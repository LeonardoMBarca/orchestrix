use super::{SavedState, StoreError, StoreResult};
use crate::domain::*;
use crate::runtime_contract::EventOrigin;
use serde_json::{Value, json};

/// Local commands only. None dispatches a process, performs Git or contacts a provider.
pub enum Command {
    AddProject(Project),
    AddConnection(Connection),
    SaveCapacityGroup(CapacityGroup),
    RegisterSession(RuntimeSessionRef),
    CreateRun(Box<Run>),
    Run {
        id: RunId,
        action: Box<RunAction>,
    },
    Send {
        conversation: ConversationId,
        project: Option<ProjectId>,
        content: String,
    },
    Reply {
        conversation: ConversationId,
        content: String,
    },
    Attach {
        conversation: ConversationId,
        project: ProjectId,
    },
    SaveDraft {
        conversation: ConversationId,
        content: String,
    },
    LinkWork {
        conversation: ConversationId,
        run: RunId,
        task: Option<TaskId>,
        attempt: Option<AttemptId>,
    },
    Records(crate::persistence_records::RecordsCommand),
    Git(crate::git_records::GitCommand),
}

pub enum RunAction {
    AddTask(Task),
    Wait(TaskId),
    Ready(TaskId),
    Assign {
        task: TaskId,
        attempt: Box<TaskAttempt>,
        snapshot: super::AttemptSnapshot,
    },
    Observe {
        task: TaskId,
        frame: String,
        origin: EventOrigin,
    },
    TransportLost {
        task: TaskId,
        reason: String,
    },
    CancelTask(TaskId),
    Publish {
        task: TaskId,
        artifact: ArtifactVersion,
    },
    Verify {
        task: TaskId,
        evidence: VerificationEvidence,
    },
    Review {
        task: TaskId,
        evidence: ReviewEvidence,
    },
    Integrate(IntegrationEvidence),
    CompleteAnalysis(TaskId),
    Prepare(RevisionId),
    Approve {
        candidate: DeliveryCandidate,
        target_base: RevisionId,
        reference: EvidenceId,
    },
    ObserveTarget(RevisionId),
    ApplicationConflict {
        candidate: DeliveryCandidate,
        target_base: RevisionId,
        reference: EvidenceId,
    },
    Apply {
        candidate: DeliveryCandidate,
        evidence: ApplicationEvidence,
    },
    Deliver,
    RequestCancel,
    ConfirmCancel,
}

fn artifact(value: &ArtifactVersion) -> Value {
    json!({"id":value.id().as_str(),"revision":value.revision().as_str(),"base":value.base().as_str()})
}
fn integration(value: &IntegrationEvidence) -> Value {
    json!({"run":value.run.as_str(),"task":value.task.as_str(),"artifact":artifact(&value.artifact),
        "inputBase":value.input_base.as_str(),"resultRevision":value.result_revision.as_str(),
        "reference":value.reference.as_str(),"validation":format!("{:?}",value.validation)})
}
fn application(value: &ApplicationEvidence) -> Value {
    json!({"run":value.run.as_str(),"repository":value.repository.as_str(),"result":value.result.as_str(),
        "targetBase":value.target_base.as_str(),"appliedRevision":value.applied_revision.as_str(),
        "reference":value.reference.as_str(),"validation":format!("{:?}",value.validation)})
}

impl Command {
    pub fn create_run(run: Run) -> Self {
        Self::CreateRun(Box::new(run))
    }
    pub fn run(id: RunId, action: RunAction) -> Self {
        Self::Run {
            id,
            action: Box::new(action),
        }
    }
    pub(super) fn work_references(
        &self,
        state: &SavedState,
    ) -> (Option<String>, Option<String>, Option<String>) {
        if let Self::Git(command) = self {
            return state
                .worktrees
                .get(command.operation_id())
                .map(|record| {
                    (
                        Some(record.intent.run_id.clone()),
                        Some(record.intent.task_id.clone()),
                        Some(record.intent.attempt_id.clone()),
                    )
                })
                .unwrap_or_default();
        }
        let (run, task, attempt) = match self {
            Self::CreateRun(run) => (Some(run.id()), None, None),
            Self::Run { id, action } => {
                let task = match action.as_ref() {
                    RunAction::AddTask(task) => Some(task.id()),
                    RunAction::Wait(task)
                    | RunAction::Ready(task)
                    | RunAction::CancelTask(task)
                    | RunAction::CompleteAnalysis(task)
                    | RunAction::Assign { task, .. }
                    | RunAction::Observe { task, .. }
                    | RunAction::TransportLost { task, .. }
                    | RunAction::Publish { task, .. }
                    | RunAction::Verify { task, .. }
                    | RunAction::Review { task, .. } => Some(task),
                    RunAction::Integrate(evidence) => Some(&evidence.task),
                    _ => None,
                };
                let attempt = task.and_then(|task| {
                    state
                        .runs
                        .get(id)?
                        .task(task)
                        .ok()?
                        .current_attempt()
                        .map(TaskAttempt::id)
                });
                (Some(id), task, attempt)
            }
            Self::LinkWork {
                run, task, attempt, ..
            } => (Some(run), task.as_ref(), attempt.as_ref()),
            _ => (None, None, None),
        };
        (
            run.map(|id| id.as_str().to_owned()),
            task.map(|id| id.as_str().to_owned()),
            attempt.map(|id| id.as_str().to_owned()),
        )
    }
    pub(super) fn body(&self) -> Value {
        match self {
            Self::AddProject(value) => json!(["project.add", value.checkpoint()]),
            Self::AddConnection(value) => json!(["connection.add", value.checkpoint()]),
            Self::SaveCapacityGroup(value) => json!(["capacity-group.save", value.checkpoint()]),
            Self::RegisterSession(value) => json!(["session.register", value.checkpoint()]),
            Self::CreateRun(value) => json!(["run.create", value.checkpoint()]),
            Self::Run { id, action } => json!(["run.command", id.as_str(), action.body()]),
            Self::Send {
                conversation,
                project,
                content,
            } => json!([
                "conversation.send",
                conversation.as_str(),
                project.as_ref().map(ProjectId::as_str),
                content
            ]),
            Self::Reply {
                conversation,
                content,
            } => json!(["conversation.reply", conversation.as_str(), content]),
            Self::Attach {
                conversation,
                project,
            } => json!([
                "conversation.attach",
                conversation.as_str(),
                project.as_str()
            ]),
            Self::SaveDraft {
                conversation,
                content,
            } => json!(["conversation.draft", conversation.as_str(), content]),
            Self::LinkWork {
                conversation,
                run,
                task,
                attempt,
            } => json!([
                "conversation.link",
                conversation.as_str(),
                run.as_str(),
                task.as_ref().map(TaskId::as_str),
                attempt.as_ref().map(AttemptId::as_str)
            ]),
            Self::Records(value) => json!(["records", value]),
            Self::Git(value) => value.fingerprint_body(),
        }
    }
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::AddProject(_) => "project.add",
            Self::AddConnection(_) => "connection.add",
            Self::SaveCapacityGroup(_) => "capacity-group.save",
            Self::RegisterSession(_) => "session.register",
            Self::CreateRun(_) => "run.create",
            Self::Run { action, .. } => action.name(),
            Self::Send { .. } => "conversation.send",
            Self::Reply { .. } => "conversation.reply",
            Self::Attach { .. } => "conversation.attach",
            Self::SaveDraft { .. } => "conversation.draft",
            Self::LinkWork { .. } => "conversation.link",
            Self::Records(_) => "records",
            Self::Git(value) => value.name(),
        }
    }
    pub(super) fn observed_failure(&self, error: &DomainError) -> bool {
        let Self::Run { action, .. } = self else {
            return false;
        };
        matches!(
            (action.as_ref(), error),
            (
                RunAction::Integrate(_) | RunAction::Apply { .. },
                DomainError::VerificationFailed
            ) | (
                RunAction::Approve { .. }
                    | RunAction::ApplicationConflict { .. }
                    | RunAction::Apply { .. },
                DomainError::StaleEvidence
            ) | (RunAction::Observe { .. }, DomainError::Runtime(_))
        )
    }
    pub(super) fn apply(&self, state: &mut SavedState) -> StoreResult<Value> {
        if let Self::Git(command) = self {
            super::git::apply(state, command)?;
            return Ok(match command {
                crate::git_records::GitCommand::RecordArtifact(artifact) => {
                    json!({"gitArtifact":{"id":artifact.id,"revision":artifact.revision,"operationId":artifact.operation_id}})
                }
                _ => Value::Null,
            });
        }
        match self {
            Self::AddProject(value) => {
                if state.projects.values().any(|p| {
                    p.repository().id() == value.repository().id()
                        && p.repository().location() != value.repository().location()
                }) {
                    return Err(DomainError::Ownership.into());
                }
                state.conversations.add_project(value.clone())?;
                state.projects.insert(value.id().clone(), value.clone());
            }
            Self::AddConnection(value) => {
                let key = (value.id().clone(), value.revision());
                if state.connections.contains_key(&key) {
                    return Err(DomainError::DuplicateIdentity.into());
                }
                if state
                    .connections
                    .keys()
                    .any(|(id, revision)| id == value.id() && *revision > value.revision())
                {
                    return Err(DomainError::InvalidVersion.into());
                }
                state
                    .records
                    .register_connection(value.id(), value.revision())
                    .map_err(StoreError::Records)?;
                state.connections.insert(key, value.clone());
            }
            Self::SaveCapacityGroup(value) => {
                for member in value.members() {
                    if !state.connections.keys().any(|(id, _)| id == member) {
                        return Err(DomainError::UnknownEntity.into());
                    }
                }
                let group = CapacityGroup::restore_checkpoint(&value.checkpoint())?;
                state.capacity_groups.insert(group.id().clone(), group);
            }
            Self::RegisterSession(value) => {
                if state.sessions.contains_key(value.id()) {
                    return Err(DomainError::DuplicateIdentity.into());
                }
                let connection = state
                    .connections
                    .get(&(value.connection_id().clone(), value.connection_revision()))
                    .ok_or(DomainError::UnknownEntity)?;
                if value.adapter_id() != connection.adapter_id() {
                    return Err(DomainError::Ownership.into());
                }
                state.sessions.insert(value.id().clone(), value.clone());
            }
            Self::CreateRun(value) => {
                if state.runs.contains_key(value.id()) {
                    return Err(DomainError::DuplicateIdentity.into());
                }
                if value.state() != RunState::Working
                    || !value.tasks().is_empty()
                    || value.cancellation_requested()
                {
                    return Err(DomainError::InvalidTransition.into());
                }
                if let Some(id) = value.project() {
                    let project = state.projects.get(id).ok_or(DomainError::UnknownEntity)?;
                    if Some(project.repository().id()) != value.repository() {
                        return Err(DomainError::Ownership.into());
                    }
                }
                state.runs.insert(
                    value.id().clone(),
                    Run::restore_checkpoint(&value.checkpoint())?,
                );
            }
            Self::Run { id, action } => {
                // Global session ownership is checked before detaching a new reservation.
                if let RunAction::Assign {
                    attempt, snapshot, ..
                } = action.as_ref()
                {
                    let session = state
                        .sessions
                        .get(attempt.session().id())
                        .ok_or(DomainError::UnknownEntity)?;
                    if session != attempt.session() {
                        return Err(DomainError::Ownership.into());
                    }
                    for previous in state
                        .runs
                        .values()
                        .flat_map(|r| r.tasks().values())
                        .flat_map(Task::attempts)
                    {
                        if previous.id() == attempt.id() {
                            return Err(DomainError::DuplicateIdentity.into());
                        }
                        if previous.session().id() == session.id() && !resolved(previous.state()) {
                            return Err(DomainError::InvalidTransition.into());
                        }
                    }
                    state.validate_snapshot(attempt, snapshot)?;
                    let mut checked = state.records.clone();
                    checked
                        .add_snapshot(snapshot.definition.clone())
                        .map_err(StoreError::Records)?;
                }
                let result =
                    action.apply(state.runs.get_mut(id).ok_or(DomainError::UnknownEntity)?);
                if result.is_ok()
                    && let RunAction::Assign {
                        attempt, snapshot, ..
                    } = action.as_ref()
                {
                    state
                        .records
                        .add_snapshot(snapshot.definition.clone())
                        .map_err(StoreError::Records)?;
                    state
                        .snapshots
                        .insert(attempt.id().clone(), snapshot.clone());
                }
                return result.map_err(Into::into);
            }
            Self::Send {
                conversation,
                project,
                content,
            } => {
                if state.conversations.conversation(conversation).is_some() {
                    // Existing conversation: user and orchestrator messages remain distinct.
                    state.conversations.append_message(
                        conversation,
                        MessageRole::User,
                        content.clone(),
                    )?;
                } else {
                    let mut pending = state.conversations.prepare(project.clone())?;
                    pending.set_draft(content.clone());
                    state
                        .conversations
                        .send(&mut pending, conversation.clone())?;
                }
                state.drafts.remove(conversation);
            }
            Self::Reply {
                conversation,
                content,
            } => state
                .conversations
                .append_reply(conversation, content.clone())?,
            Self::Attach {
                conversation,
                project,
            } => state
                .conversations
                .attach_project(conversation, project.clone())?,
            Self::SaveDraft {
                conversation,
                content,
            } => {
                if state.conversations.conversation(conversation).is_none() {
                    return Err(DomainError::UnknownEntity.into());
                }
                if content.trim().is_empty() {
                    state.drafts.remove(conversation);
                } else {
                    state.drafts.insert(conversation.clone(), content.clone());
                }
            }
            Self::LinkWork {
                conversation,
                run,
                task,
                attempt,
            } => state.conversations.link_work(
                conversation,
                state.runs.get(run).ok_or(DomainError::UnknownEntity)?,
                task.as_ref(),
                attempt.as_ref(),
            )?,
            Self::Records(value) => {
                use crate::persistence_records::RecordsCommand;
                match value {
                    // These two records are managed atomically by AddConnection/Assign.
                    RecordsCommand::RegisterConnection { .. } | RecordsCommand::AddSnapshot(_) => {
                        return Err(DomainError::Policy.into());
                    }
                    RecordsCommand::AddDefinition(input) if input.version > i64::MAX as u64 => {
                        return Err(DomainError::InvalidVersion.into());
                    }
                    RecordsCommand::Diagnostic(input) if input.version > i64::MAX as u64 => {
                        return Err(DomainError::InvalidVersion.into());
                    }
                    _ => state.records.apply(value).map_err(StoreError::Records)?,
                }
            }
            Self::Git(_) => unreachable!("Git commands handled before domain commands"),
        }
        Ok(Value::Null)
    }
}

pub(super) fn resolved(state: AttemptState) -> bool {
    matches!(
        state,
        AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
    )
}

impl RunAction {
    pub fn assign(task: TaskId, attempt: TaskAttempt, snapshot: super::AttemptSnapshot) -> Self {
        Self::Assign {
            task,
            attempt: Box::new(attempt),
            snapshot,
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Self::AddTask(_) => "task.add",
            Self::Wait(_) => "task.wait",
            Self::Ready(_) => "task.ready",
            Self::Assign { .. } => "attempt.reserve",
            Self::Observe { .. } => "attempt.observe",
            Self::TransportLost { .. } => "attempt.transport-lost",
            Self::CancelTask(_) => "task.cancel",
            Self::Publish { .. } => "artifact.publish",
            Self::Verify { .. } => "artifact.verify",
            Self::Review { .. } => "artifact.review",
            Self::Integrate(_) => "artifact.integrate",
            Self::CompleteAnalysis(_) => "analysis.complete",
            Self::Prepare(_) => "delivery.prepare",
            Self::Approve { .. } => "delivery.approve",
            Self::ObserveTarget(_) => "delivery.target",
            Self::ApplicationConflict { .. } => "delivery.conflict",
            Self::Apply { .. } => "delivery.apply",
            Self::Deliver => "run.deliver",
            Self::RequestCancel => "run.cancel-request",
            Self::ConfirmCancel => "run.cancel-confirm",
        }
    }
    fn body(&self) -> Value {
        let value = match self {
            Self::AddTask(value) => value.checkpoint(),
            Self::Wait(id)
            | Self::Ready(id)
            | Self::CancelTask(id)
            | Self::CompleteAnalysis(id) => json!(id.as_str()),
            Self::Assign {
                task,
                attempt,
                snapshot,
            } => json!([task.as_str(), attempt.checkpoint(), snapshot]),
            // Never write the raw frame, even if malformed. Its digest binds idempotency.
            Self::Observe {
                task,
                frame,
                origin,
            } => json!([
                task.as_str(),
                super::digest(frame.as_bytes()),
                match origin {
                    EventOrigin::Runtime => "runtime",
                    EventOrigin::Supervisor => "supervisor",
                }
            ]),
            Self::TransportLost { task, reason } => json!([task.as_str(), reason]),
            Self::Publish {
                task,
                artifact: value,
            } => json!([task.as_str(), artifact(value)]),
            Self::Verify { task, evidence } => json!([
                task.as_str(),
                artifact(&evidence.artifact),
                evidence.reference.as_str(),
                format!("{:?}", evidence.result)
            ]),
            Self::Review { task, evidence } => json!([
                task.as_str(),
                artifact(&evidence.artifact),
                evidence.reference.as_str(),
                format!("{:?}", evidence.result)
            ]),
            Self::Integrate(value) => integration(value),
            Self::Prepare(rev) | Self::ObserveTarget(rev) => json!(rev.as_str()),
            Self::Approve {
                candidate,
                target_base,
                reference,
            }
            | Self::ApplicationConflict {
                candidate,
                target_base,
                reference,
            } => json!([
                candidate.checkpoint(),
                target_base.as_str(),
                reference.as_str()
            ]),
            Self::Apply {
                candidate,
                evidence,
            } => json!([candidate.checkpoint(), application(evidence)]),
            Self::Deliver | Self::RequestCancel | Self::ConfirmCancel => Value::Null,
        };
        json!([self.name(), value])
    }
    fn apply(&self, run: &mut Run) -> DomainResult<Value> {
        match self {
            Self::AddTask(value) => run.add_task(Task::restore_checkpoint(&value.checkpoint())?)?,
            Self::Wait(id) => run.wait_task(id)?,
            Self::Ready(id) => run.make_ready(id)?,
            Self::Assign { task, attempt, .. } => run.assign_attempt(
                task,
                TaskAttempt::restore_checkpoint(&attempt.checkpoint())?,
            )?,
            Self::Observe {
                task,
                frame,
                origin,
            } => run.accept_frame(task, frame, *origin)?,
            Self::TransportLost { task, reason } => run.transport_lost(task, reason)?,
            Self::CancelTask(id) => run.request_task_cancel(id)?,
            Self::Publish { task, artifact } => run.publish_artifact(task, artifact.clone())?,
            Self::Verify { task, evidence } => run.verify_artifact(task, evidence.clone())?,
            Self::Review { task, evidence } => run.review_artifact(task, evidence.clone())?,
            Self::Integrate(value) => run.integrate(value.clone())?,
            Self::CompleteAnalysis(id) => run.complete_analysis(id)?,
            Self::Prepare(rev) => return Ok(run.prepare_delivery(rev.clone())?.checkpoint()),
            Self::Approve {
                candidate,
                target_base,
                reference,
            } => run.approve_delivery(candidate, target_base, reference.clone())?,
            Self::ObserveTarget(rev) => run.observe_target_base(rev)?,
            Self::ApplicationConflict {
                candidate,
                target_base,
                reference,
            } => run.report_application_conflict(candidate, target_base, reference.clone())?,
            Self::Apply {
                candidate,
                evidence,
            } => run.confirm_application(candidate, evidence.clone())?,
            Self::Deliver => run.mark_delivered()?,
            Self::RequestCancel => run.request_cancel()?,
            Self::ConfirmCancel => run.confirm_cancel()?,
        }
        Ok(Value::Null)
    }
}
