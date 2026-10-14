use super::{
    ActionMode, AgentProfile, ArtifactVersion, AttemptId, Connection, ContextPackId,
    DefinitionVersion, DomainError, DomainResult, LocalRef, ProjectId, RevisionId,
    RuntimeSessionRef, require,
};
use crate::runtime_contract::{
    EventOrigin, Limits, RuntimeObserver, RuntimeRequest, assert_admission,
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextVersion {
    pub reference: ContextPackId,
    pub version: DefinitionVersion,
}

/// A captured configuration; edits to definitions only affect later inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptInputs {
    pub plan_revision: RevisionId,
    pub context: ContextVersion,
    pub mode: ActionMode,
    pub profile: AgentProfile,
    pub base: RevisionId,
    pub worktree: LocalRef,
    pub project: Option<ProjectId>,
    pub correction_of: Option<ArtifactVersion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptState {
    Created,
    Admitted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Uncertain,
}

/// Output production is separate from execution and from artifact acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptResult {
    Pending,
    AwaitingArtifact,
    ArtifactProduced,
    ExecutionFailed,
    Cancelled,
    Uncertain,
}

/// Owns the existing OX-003 observer, not a second runtime event parser.
pub struct TaskAttempt {
    pub(super) id: AttemptId,
    pub(super) session: RuntimeSessionRef,
    pub(super) inputs: AttemptInputs,
    pub(super) observer: RuntimeObserver,
    pub(super) cancellation_requested: bool,
    pub(super) sealed: bool,
    pub(super) artifacts: Vec<ArtifactVersion>,
}
impl std::fmt::Debug for TaskAttempt {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("TaskAttempt")
            .field("id", &self.id)
            .field("inputs", &self.inputs)
            .field("state", &self.state())
            .field("sealed", &self.sealed)
            .finish_non_exhaustive()
    }
}
impl TaskAttempt {
    pub fn new(
        connection: &Connection,
        session: &RuntimeSessionRef,
        request: RuntimeRequest,
        inputs: AttemptInputs,
    ) -> DomainResult<Self> {
        let intent = request.as_json();
        require(
            session.connection_id() == connection.id()
                && session.connection_revision() == connection.revision()
                && session.adapter_id() == connection.adapter_id()
                && intent["sessionId"] == session.id().as_str(),
            DomainError::Ownership,
        )?;
        require(
            intent["contextPackRef"] == inputs.context.reference.as_str(),
            DomainError::MissingEvidence,
        )?;
        let id = AttemptId::new(intent["attemptId"].as_str().ok_or(DomainError::Ownership)?)?;
        // Policy admission is distinct from a factual ACK/start/terminal.
        assert_admission(connection.descriptor(), &request, None)?;
        let observer =
            RuntimeObserver::new(connection.descriptor().clone(), request, Limits::default())?;
        Ok(Self {
            id,
            session: session.clone(),
            inputs,
            observer,
            cancellation_requested: false,
            sealed: false,
            artifacts: Vec::new(),
        })
    }
    pub fn id(&self) -> &AttemptId {
        &self.id
    }
    pub fn session(&self) -> &RuntimeSessionRef {
        &self.session
    }
    pub fn inputs(&self) -> &AttemptInputs {
        &self.inputs
    }
    pub fn runtime_snapshot(&self) -> Value {
        self.observer.snapshot()
    }
    pub fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }
    pub fn artifacts(&self) -> &[ArtifactVersion] {
        &self.artifacts
    }
    pub fn result(&self) -> AttemptResult {
        match self.state() {
            AttemptState::Succeeded if self.artifacts.is_empty() => AttemptResult::AwaitingArtifact,
            AttemptState::Succeeded => AttemptResult::ArtifactProduced,
            AttemptState::Failed => AttemptResult::ExecutionFailed,
            AttemptState::Cancelled => AttemptResult::Cancelled,
            AttemptState::Uncertain => AttemptResult::Uncertain,
            _ => AttemptResult::Pending,
        }
    }
    pub fn state(&self) -> AttemptState {
        let state = self.observer.snapshot();
        match state["outcome"].as_str() {
            Some("completed") => AttemptState::Succeeded,
            Some("failed") => AttemptState::Failed,
            Some("interrupted") => AttemptState::Cancelled,
            _ if state["observation"] != "healthy" || state["fatal"] == true => {
                AttemptState::Uncertain
            }
            _ if state["startObserved"] == true => AttemptState::Running,
            _ if state["admissionObserved"] == true => AttemptState::Admitted,
            _ => AttemptState::Created,
        }
    }
    pub(crate) fn accept_frame(&mut self, frame: &str, origin: EventOrigin) -> DomainResult<()> {
        require(!self.sealed, DomainError::InvalidTransition)?;
        self.observer.accept(frame, origin)?;
        Ok(())
    }
    pub(crate) fn transport_lost(&mut self, reason: &str) -> DomainResult<()> {
        require(!self.sealed, DomainError::InvalidTransition)?;
        self.observer.transport_lost(reason)?;
        Ok(())
    }
    pub(crate) fn request_cancel(&mut self) -> DomainResult<()> {
        require(
            !self.sealed
                && !matches!(
                    self.state(),
                    AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                ),
            DomainError::InvalidTransition,
        )?;
        self.cancellation_requested = true;
        Ok(())
    }
    pub(crate) fn record_artifact(&mut self, artifact: ArtifactVersion) -> DomainResult<()> {
        require(
            !self.sealed && self.state() == AttemptState::Succeeded,
            DomainError::InvalidTransition,
        )?;
        require(
            artifact.base() == &self.inputs.base,
            DomainError::StaleEvidence,
        )?;
        require(
            !self.artifacts.contains(&artifact),
            DomainError::DuplicateIdentity,
        )?;
        self.artifacts.push(artifact);
        Ok(())
    }
    pub(crate) fn seal(&mut self) -> DomainResult<()> {
        require(
            matches!(
                self.state(),
                AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
            ),
            DomainError::InvalidTransition,
        )?;
        self.observer.close();
        self.sealed = true;
        Ok(())
    }
}
