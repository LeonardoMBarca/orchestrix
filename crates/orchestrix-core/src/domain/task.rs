//! Guarded task lifecycle, independently of runtime terminal and final delivery.
use super::{
    ArtifactVersion, AttemptState, CheckResult, DomainError, DomainResult, IntegrationEvidence,
    ReviewEvidence, ReviewResult, TaskAttempt, TaskId, VerificationEvidence, require,
};
use crate::runtime_contract::EventOrigin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Code,
    Analysis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Created,
    Waiting,
    Ready,
    Assigned,
    Running,
    NeedsReview,
    NeedsWork,
    Approved,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

/// The result consumed by a successor, rather than a generic runtime success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyResult {
    CodeIntegrated,
    AnalysisArtifactAccepted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDependency {
    pub task: TaskId,
    pub result: DependencyResult,
}

pub struct Task {
    pub(super) id: TaskId,
    pub(super) objective: String,
    pub(super) kind: TaskKind,
    pub(super) acceptance_criteria: Vec<String>,
    pub(super) dependencies: Vec<TaskDependency>,
    pub(super) state: TaskState,
    pub(super) attempts: Vec<TaskAttempt>,
    pub(super) artifact: Option<ArtifactVersion>,
    pub(super) verifications: Vec<VerificationEvidence>,
    pub(super) reviews: Vec<ReviewEvidence>,
    pub(super) integrations: Vec<IntegrationEvidence>,
}

impl Task {
    pub fn new(
        id: TaskId,
        objective: impl Into<String>,
        kind: TaskKind,
        acceptance_criteria: Vec<String>,
        dependencies: Vec<TaskDependency>,
    ) -> DomainResult<Self> {
        let objective = objective.into();
        require(
            !objective.trim().is_empty()
                && !acceptance_criteria.is_empty()
                && acceptance_criteria.iter().all(|c| !c.trim().is_empty()),
            DomainError::EmptyContent,
        )?;
        let mut seen = std::collections::BTreeSet::new();
        for dependency in &dependencies {
            require(dependency.task != id, DomainError::DependencyPending)?;
            require(
                seen.insert(dependency.task.clone()),
                DomainError::DuplicateIdentity,
            )?;
        }
        Ok(Self {
            id,
            objective,
            kind,
            acceptance_criteria,
            dependencies,
            state: TaskState::Created,
            attempts: Vec::new(),
            artifact: None,
            verifications: Vec::new(),
            reviews: Vec::new(),
            integrations: Vec::new(),
        })
    }
    pub fn id(&self) -> &TaskId {
        &self.id
    }
    pub fn objective(&self) -> &str {
        &self.objective
    }
    pub fn kind(&self) -> TaskKind {
        self.kind
    }
    pub fn state(&self) -> TaskState {
        self.state
    }
    pub fn acceptance_criteria(&self) -> &[String] {
        &self.acceptance_criteria
    }
    pub fn dependencies(&self) -> &[TaskDependency] {
        &self.dependencies
    }
    pub fn attempts(&self) -> &[TaskAttempt] {
        &self.attempts
    }
    pub fn current_attempt(&self) -> Option<&TaskAttempt> {
        self.attempts.last()
    }
    pub fn artifact(&self) -> Option<&ArtifactVersion> {
        self.artifact.as_ref()
    }
    pub fn verifications(&self) -> &[VerificationEvidence] {
        &self.verifications
    }
    pub fn reviews(&self) -> &[ReviewEvidence] {
        &self.reviews
    }
    pub fn integrations(&self) -> &[IntegrationEvidence] {
        &self.integrations
    }

    pub(crate) fn wait(&mut self) -> DomainResult<()> {
        require(
            self.state == TaskState::Created,
            DomainError::InvalidTransition,
        )?;
        self.state = TaskState::Waiting;
        Ok(())
    }
    pub(crate) fn make_ready(&mut self) -> DomainResult<()> {
        require(
            matches!(
                self.state,
                TaskState::Created
                    | TaskState::Waiting
                    | TaskState::NeedsWork
                    | TaskState::Failed
                    | TaskState::Blocked
            ),
            DomainError::InvalidTransition,
        )?;
        if let Some(attempt) = self.current_attempt() {
            require(
                matches!(
                    attempt.state(),
                    AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                ),
                DomainError::InvalidTransition,
            )?;
        }
        self.state = TaskState::Ready;
        Ok(())
    }
    pub(crate) fn assign(&mut self, attempt: TaskAttempt) -> DomainResult<()> {
        require(
            self.state == TaskState::Ready && attempt.state() == AttemptState::Created,
            DomainError::InvalidTransition,
        )?;
        require(
            !self.attempts.iter().any(|a| a.id() == attempt.id()),
            DomainError::DuplicateIdentity,
        )?;
        if let Some(previous) = self.current_attempt() {
            require(
                matches!(
                    previous.state(),
                    AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                ),
                DomainError::InvalidTransition,
            )?;
            if let Some(artifact) = self.artifact.as_ref() {
                require(
                    attempt.inputs().correction_of.as_ref() == Some(artifact),
                    DomainError::StaleEvidence,
                )?;
                require(
                    &attempt.inputs().base == artifact.revision(),
                    DomainError::StaleEvidence,
                )?;
            } else {
                require(
                    attempt.inputs().correction_of.is_none(),
                    DomainError::StaleEvidence,
                )?;
            }
        } else {
            require(
                attempt.inputs().correction_of.is_none(),
                DomainError::StaleEvidence,
            )?;
        }
        if let Some(previous) = self.attempts.last_mut() {
            previous.seal()?;
        }
        self.attempts.push(attempt);
        self.artifact = None;
        self.state = TaskState::Assigned;
        Ok(())
    }
    pub(crate) fn observe(&mut self, frame: &str, origin: EventOrigin) -> DomainResult<()> {
        let attempt = self
            .attempts
            .last_mut()
            .ok_or(DomainError::InvalidTransition)?;
        let result = attempt.accept_frame(frame, origin);
        self.reconcile_observation();
        result
    }
    pub(crate) fn transport_lost(&mut self, reason: &str) -> DomainResult<()> {
        let attempt = self
            .attempts
            .last_mut()
            .ok_or(DomainError::InvalidTransition)?;
        attempt.transport_lost(reason)?;
        self.reconcile_observation();
        Ok(())
    }
    fn reconcile_observation(&mut self) {
        let Some(attempt) = self.attempts.last() else {
            return;
        };
        match attempt.state() {
            AttemptState::Created | AttemptState::Admitted => {}
            AttemptState::Running => {
                if matches!(self.state, TaskState::Assigned | TaskState::Blocked) {
                    self.state = TaskState::Running;
                }
            }
            AttemptState::Succeeded => {
                if matches!(
                    self.state,
                    TaskState::Assigned | TaskState::Running | TaskState::Blocked
                ) {
                    self.state = TaskState::NeedsReview;
                }
            }
            AttemptState::Failed => {
                if matches!(
                    self.state,
                    TaskState::Assigned | TaskState::Running | TaskState::Blocked
                ) {
                    self.state = TaskState::Failed;
                }
            }
            AttemptState::Cancelled => {
                if matches!(
                    self.state,
                    TaskState::Assigned | TaskState::Running | TaskState::Blocked
                ) {
                    self.state = TaskState::Cancelled;
                }
            }
            AttemptState::Uncertain => self.state = TaskState::Blocked,
        }
    }
    pub(crate) fn request_cancel(&mut self) -> DomainResult<()> {
        require(
            !matches!(self.state, TaskState::Completed | TaskState::Cancelled),
            DomainError::InvalidTransition,
        )?;
        if let Some(attempt) = self.attempts.last_mut()
            && matches!(
                attempt.state(),
                AttemptState::Created
                    | AttemptState::Admitted
                    | AttemptState::Running
                    | AttemptState::Uncertain
            )
        {
            return attempt.request_cancel();
        }
        self.state = TaskState::Cancelled;
        Ok(())
    }
    pub(crate) fn publish(&mut self, artifact: ArtifactVersion) -> DomainResult<()> {
        require(
            self.state == TaskState::NeedsReview,
            DomainError::InvalidTransition,
        )?;
        require(
            !self
                .attempts
                .iter()
                .any(|attempt| attempt.artifacts().contains(&artifact)),
            DomainError::DuplicateIdentity,
        )?;
        self.attempts
            .last_mut()
            .ok_or(DomainError::InvalidTransition)?
            .record_artifact(artifact.clone())?;
        self.artifact = Some(artifact);
        Ok(())
    }
    fn require_current(&self, artifact: &ArtifactVersion) -> DomainResult<()> {
        require(
            self.artifact.as_ref() == Some(artifact),
            DomainError::StaleEvidence,
        )
    }
    fn checks_passed(&self) -> bool {
        let Some(artifact) = self.artifact.as_ref() else {
            return false;
        };
        let checks: Vec<_> = self
            .verifications
            .iter()
            .filter(|v| &v.artifact == artifact)
            .collect();
        !checks.is_empty() && checks.iter().all(|v| v.result == CheckResult::Passed)
    }
    fn review_accepted(&self) -> bool {
        self.reviews
            .iter()
            .rev()
            .find(|r| Some(&r.artifact) == self.artifact.as_ref())
            .is_some_and(|r| r.result == ReviewResult::Accepted)
    }
    fn promote_approval(&mut self) {
        if self.review_accepted() && (self.kind == TaskKind::Analysis || self.checks_passed()) {
            self.state = TaskState::Approved;
        }
    }
    pub(crate) fn verify(&mut self, evidence: VerificationEvidence) -> DomainResult<()> {
        require(
            matches!(self.state, TaskState::NeedsReview | TaskState::Approved),
            DomainError::InvalidTransition,
        )?;
        self.require_current(&evidence.artifact)?;
        require(
            !self
                .verifications
                .iter()
                .any(|v| v.reference == evidence.reference),
            DomainError::DuplicateIdentity,
        )?;
        let failed = evidence.result == CheckResult::Failed;
        self.verifications.push(evidence);
        if failed {
            self.state = TaskState::NeedsWork;
        } else {
            self.promote_approval();
        }
        Ok(())
    }
    pub(crate) fn review(&mut self, evidence: ReviewEvidence) -> DomainResult<()> {
        require(
            matches!(self.state, TaskState::NeedsReview | TaskState::Approved),
            DomainError::InvalidTransition,
        )?;
        self.require_current(&evidence.artifact)?;
        require(
            !self
                .reviews
                .iter()
                .any(|r| r.reference == evidence.reference),
            DomainError::DuplicateIdentity,
        )?;
        let rejected = evidence.result == ReviewResult::Rejected;
        self.reviews.push(evidence);
        if rejected {
            self.state = TaskState::NeedsWork;
        } else {
            self.promote_approval();
        }
        Ok(())
    }
    pub(crate) fn integrate(&mut self, evidence: IntegrationEvidence) -> DomainResult<()> {
        require(
            self.kind == TaskKind::Code && self.state == TaskState::Approved,
            DomainError::InvalidTransition,
        )?;
        self.require_current(&evidence.artifact)?;
        require(
            self.checks_passed() && self.review_accepted(),
            DomainError::MissingEvidence,
        )?;
        require(evidence.task == self.id, DomainError::Ownership)?;
        require(
            !self
                .integrations
                .iter()
                .any(|i| i.reference == evidence.reference),
            DomainError::DuplicateIdentity,
        )?;
        let passed = evidence.validation == CheckResult::Passed;
        self.integrations.push(evidence);
        if passed {
            self.state = TaskState::Completed;
        }
        Ok(())
    }
    pub(crate) fn complete_analysis(&mut self) -> DomainResult<()> {
        require(
            self.kind == TaskKind::Analysis && self.state == TaskState::Approved,
            DomainError::InvalidTransition,
        )?;
        require(self.review_accepted(), DomainError::MissingEvidence)?;
        self.state = TaskState::Completed;
        Ok(())
    }
    pub(crate) fn releases(&self, result: DependencyResult) -> bool {
        if self.state != TaskState::Completed {
            return false;
        }
        match result {
            DependencyResult::CodeIntegrated => {
                self.kind == TaskKind::Code
                    && self.integrations.last().is_some_and(|i| {
                        i.validation == CheckResult::Passed
                            && Some(&i.artifact) == self.artifact.as_ref()
                    })
            }
            DependencyResult::AnalysisArtifactAccepted => {
                self.kind == TaskKind::Analysis && self.review_accepted()
            }
        }
    }
}
