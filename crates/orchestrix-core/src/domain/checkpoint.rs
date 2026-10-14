//! Versioned, validated local state checkpoints. No SQL or external execution.
//! Entities retain private fields; this module is their internal restoration boundary.
use super::*;
use crate::runtime_contract::{RuntimeObserver, normalize_descriptor};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn wrap(kind: &str, data: Value) -> Value {
    json!({"checkpointVersion": 1, "kind": kind, "data": data})
}
fn keys(value: &Value, names: &[&str]) -> DomainResult<()> {
    require(
        value.as_object().is_some_and(|map| {
            map.len() == names.len() && names.iter().all(|name| map.contains_key(*name))
        }),
        DomainError::InvalidReference,
    )
}
fn data<'a>(value: &'a Value, kind: &str, names: &[&str]) -> DomainResult<&'a Value> {
    keys(value, &["checkpointVersion", "kind", "data"])?;
    require(value["checkpointVersion"] == 1, DomainError::InvalidVersion)?;
    require(value["kind"] == kind, DomainError::InvalidReference)?;
    keys(&value["data"], names)?;
    Ok(&value["data"])
}
fn string<'a>(value: &'a Value, name: &str) -> DomainResult<&'a str> {
    value[name].as_str().ok_or(DomainError::InvalidReference)
}
fn number(value: &Value, name: &str) -> DomainResult<u64> {
    value[name].as_u64().ok_or(DomainError::InvalidVersion)
}
fn boolean(value: &Value, name: &str) -> DomainResult<bool> {
    value[name].as_bool().ok_or(DomainError::InvalidReference)
}
fn array<'a>(value: &'a Value, name: &str) -> DomainResult<&'a [Value]> {
    value[name]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(DomainError::InvalidReference)
}
fn optional<T>(
    value: &Value,
    name: &str,
    parse: impl FnOnce(&Value) -> DomainResult<T>,
) -> DomainResult<Option<T>> {
    if value[name].is_null() {
        Ok(None)
    } else {
        parse(&value[name]).map(Some)
    }
}
fn local<T>(value: &Value, parse: impl FnOnce(String) -> DomainResult<T>) -> DomainResult<T> {
    parse(
        value
            .as_str()
            .ok_or(DomainError::InvalidReference)?
            .to_owned(),
    )
}
macro_rules! enumeration {
    ($value:expr, $ty:ident, $($variant:ident),+ $(,)?) => {
        match $value.as_str() { $(Some(stringify!($variant)) => Ok($ty::$variant),)+ _ => Err(DomainError::InvalidReference) }
    }
}
fn check_result(value: &Value) -> DomainResult<CheckResult> {
    enumeration!(value, CheckResult, Passed, Failed)
}

impl Project {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "Project",
            json!({"id": self.id.as_str(), "repositoryId": self.repository.id.as_str(), "location": self.repository.location.as_str()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "Project", &["id", "repositoryId", "location"])?;
        Ok(Self::new(
            ProjectId::new(string(value, "id")?)?,
            Repository::new(
                RepositoryId::new(string(value, "repositoryId")?)?,
                LocalRef::new(string(value, "location")?)?,
            ),
        ))
    }
}
impl Connection {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "Connection",
            json!({"descriptor": self.descriptor.as_json()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "Connection", &["descriptor"])?;
        let descriptor = normalize_descriptor(&value["descriptor"])?;
        require(
            descriptor.as_json() == &value["descriptor"],
            DomainError::InvalidReference,
        )?;
        Self::new(descriptor)
    }
}
impl CapacityGroup {
    pub fn checkpoint(&self) -> Value {
        let capacity = match &self.capacity {
            SharedCapacity::Unknown => json!({"state":"Unknown"}),
            SharedCapacity::Observed(value) => {
                json!({"state":"Observed","limit":value.limit,"remaining":value.remaining,"evidence":value.evidence.as_str()})
            }
        };
        wrap(
            "CapacityGroup",
            json!({"id":self.id.as_str(),"members":self.members.iter().map(ConnectionId::as_str).collect::<Vec<_>>(),"capacity":capacity}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "CapacityGroup", &["id", "members", "capacity"])?;
        let capacity = &value["capacity"];
        let capacity = match capacity["state"].as_str() {
            Some("Unknown") => {
                keys(capacity, &["state"])?;
                SharedCapacity::Unknown
            }
            Some("Observed") => {
                keys(capacity, &["state", "limit", "remaining", "evidence"])?;
                SharedCapacity::Observed(CapacityObservation::new(
                    number(capacity, "limit")?,
                    number(capacity, "remaining")?,
                    EvidenceId::new(string(capacity, "evidence")?)?,
                )?)
            }
            _ => return Err(DomainError::InvalidReference),
        };
        let mut group = Self::new(CapacityGroupId::new(string(value, "id")?)?, capacity);
        for member in array(value, "members")? {
            group.add_member(local(member, ConnectionId::new)?)?;
        }
        Ok(group)
    }
}
impl RuntimeSessionRef {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "RuntimeSessionRef",
            json!({"id": self.id.as_str(), "connectionId": self.connection_id.as_str(), "connectionRevision": self.connection_revision, "adapterId": self.adapter_id}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "RuntimeSessionRef",
            &["id", "connectionId", "connectionRevision", "adapterId"],
        )?;
        let revision = number(value, "connectionRevision")?;
        require(
            (1..=9_007_199_254_740_991).contains(&revision),
            DomainError::InvalidVersion,
        )?;
        let adapter_id = LocalRef::new(string(value, "adapterId")?)?;
        Ok(Self {
            id: RuntimeSessionId::new(string(value, "id")?)?,
            connection_id: ConnectionId::new(string(value, "connectionId")?)?,
            connection_revision: revision,
            adapter_id: adapter_id.as_str().to_owned(),
        })
    }
}
impl ActionMode {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "ActionMode",
            json!({"id": self.id.as_str(), "version": self.version.get(), "name": self.name, "purpose": self.purpose}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "ActionMode", &["id", "version", "name", "purpose"])?;
        Self::new(
            ModeId::new(string(value, "id")?)?,
            DefinitionVersion::new(number(value, "version")?)?,
            string(value, "name")?,
            string(value, "purpose")?,
        )
    }
}
impl AgentProfile {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "AgentProfile",
            json!({"id": self.id.as_str(), "version": self.version.get(), "name": self.name, "role": self.role, "instructions": self.instructions.as_str()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "AgentProfile",
            &["id", "version", "name", "role", "instructions"],
        )?;
        Self::new(
            ProfileId::new(string(value, "id")?)?,
            DefinitionVersion::new(number(value, "version")?)?,
            string(value, "name")?,
            string(value, "role")?,
            LocalRef::new(string(value, "instructions")?)?,
        )
    }
}
impl ArtifactVersion {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "ArtifactVersion",
            json!({"id": self.id.as_str(), "revision": self.revision.as_str(), "base": self.base.as_str()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "ArtifactVersion", &["id", "revision", "base"])?;
        Ok(Self::new(
            ArtifactId::new(string(value, "id")?)?,
            RevisionId::new(string(value, "revision")?)?,
            RevisionId::new(string(value, "base")?)?,
        ))
    }
}
impl AttemptInputs {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "AttemptInputs",
            json!({"planRevision": self.plan_revision.as_str(), "context": {"reference": self.context.reference.as_str(), "version": self.context.version.get()}, "mode": self.mode.checkpoint(), "profile": self.profile.checkpoint(), "base": self.base.as_str(), "worktree": self.worktree.as_str(), "project": self.project.as_ref().map(ProjectId::as_str), "correctionOf": self.correction_of.as_ref().map(ArtifactVersion::checkpoint)}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "AttemptInputs",
            &[
                "planRevision",
                "context",
                "mode",
                "profile",
                "base",
                "worktree",
                "project",
                "correctionOf",
            ],
        )?;
        keys(&value["context"], &["reference", "version"])?;
        Ok(Self {
            plan_revision: RevisionId::new(string(value, "planRevision")?)?,
            context: ContextVersion {
                reference: ContextPackId::new(string(&value["context"], "reference")?)?,
                version: DefinitionVersion::new(number(&value["context"], "version")?)?,
            },
            mode: ActionMode::restore_checkpoint(&value["mode"])?,
            profile: AgentProfile::restore_checkpoint(&value["profile"])?,
            base: RevisionId::new(string(value, "base")?)?,
            worktree: LocalRef::new(string(value, "worktree")?)?,
            project: optional(value, "project", |v| local(v, ProjectId::new))?,
            correction_of: optional(value, "correctionOf", ArtifactVersion::restore_checkpoint)?,
        })
    }
}
impl TaskAttempt {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "TaskAttempt",
            json!({"id": self.id.as_str(), "session": self.session.checkpoint(), "inputs": self.inputs.checkpoint(), "observer": self.observer.checkpoint(), "cancellationRequested": self.cancellation_requested, "sealed": self.sealed, "artifacts": self.artifacts.iter().map(ArtifactVersion::checkpoint).collect::<Vec<_>>()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "TaskAttempt",
            &[
                "id",
                "session",
                "inputs",
                "observer",
                "cancellationRequested",
                "sealed",
                "artifacts",
            ],
        )?;
        let observer = RuntimeObserver::restore_checkpoint(&value["observer"])?;
        let connection = Connection::new(observer.descriptor().clone())?;
        let session = RuntimeSessionRef::restore_checkpoint(&value["session"])?;
        let inputs = AttemptInputs::restore_checkpoint(&value["inputs"])?;
        let mut attempt = Self::new(&connection, &session, observer.request().clone(), inputs)?;
        require(
            attempt.id.as_str() == string(value, "id")?,
            DomainError::Ownership,
        )?;
        attempt.observer = observer;
        attempt.cancellation_requested = boolean(value, "cancellationRequested")?;
        let artifacts = array(value, "artifacts")?
            .iter()
            .map(ArtifactVersion::restore_checkpoint)
            .collect::<DomainResult<Vec<_>>>()?;
        for artifact in artifacts {
            attempt.record_artifact(artifact)?;
        }
        let sealed = boolean(value, "sealed")?;
        require(
            sealed == (attempt.runtime_snapshot()["observation"] == "closed"),
            DomainError::InvalidTransition,
        )?;
        if sealed {
            require(
                attempt.runtime_snapshot()["observation"] == "closed",
                DomainError::InvalidTransition,
            )?;
            attempt.seal()?;
        }
        Ok(attempt)
    }
}
fn verification(value: &VerificationEvidence) -> Value {
    json!({"artifact": value.artifact.checkpoint(), "reference": value.reference.as_str(), "result": format!("{:?}", value.result)})
}
fn restore_verification(value: &Value) -> DomainResult<VerificationEvidence> {
    keys(value, &["artifact", "reference", "result"])?;
    Ok(VerificationEvidence {
        artifact: ArtifactVersion::restore_checkpoint(&value["artifact"])?,
        reference: EvidenceId::new(string(value, "reference")?)?,
        result: check_result(&value["result"])?,
    })
}
fn review(value: &ReviewEvidence) -> Value {
    json!({"artifact": value.artifact.checkpoint(), "reference": value.reference.as_str(), "result": format!("{:?}", value.result)})
}
fn restore_review(value: &Value) -> DomainResult<ReviewEvidence> {
    keys(value, &["artifact", "reference", "result"])?;
    Ok(ReviewEvidence {
        artifact: ArtifactVersion::restore_checkpoint(&value["artifact"])?,
        reference: EvidenceId::new(string(value, "reference")?)?,
        result: enumeration!(&value["result"], ReviewResult, Accepted, Rejected)?,
    })
}
fn integration(value: &IntegrationEvidence) -> Value {
    json!({"run": value.run.as_str(), "task": value.task.as_str(), "artifact": value.artifact.checkpoint(), "inputBase": value.input_base.as_str(), "resultRevision": value.result_revision.as_str(), "reference": value.reference.as_str(), "validation": format!("{:?}", value.validation)})
}
fn restore_integration(value: &Value) -> DomainResult<IntegrationEvidence> {
    keys(
        value,
        &[
            "run",
            "task",
            "artifact",
            "inputBase",
            "resultRevision",
            "reference",
            "validation",
        ],
    )?;
    Ok(IntegrationEvidence {
        run: RunId::new(string(value, "run")?)?,
        task: TaskId::new(string(value, "task")?)?,
        artifact: ArtifactVersion::restore_checkpoint(&value["artifact"])?,
        input_base: RevisionId::new(string(value, "inputBase")?)?,
        result_revision: RevisionId::new(string(value, "resultRevision")?)?,
        reference: EvidenceId::new(string(value, "reference")?)?,
        validation: check_result(&value["validation"])?,
    })
}
impl Task {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "Task",
            json!({"id": self.id.as_str(), "objective": self.objective, "kind": format!("{:?}", self.kind), "acceptanceCriteria": self.acceptance_criteria, "dependencies": self.dependencies.iter().map(|d| json!({"task": d.task.as_str(), "result": format!("{:?}", d.result)})).collect::<Vec<_>>(), "state": format!("{:?}", self.state), "attempts": self.attempts.iter().map(TaskAttempt::checkpoint).collect::<Vec<_>>(), "artifact": self.artifact.as_ref().map(ArtifactVersion::checkpoint), "verifications": self.verifications.iter().map(verification).collect::<Vec<_>>(), "reviews": self.reviews.iter().map(review).collect::<Vec<_>>(), "integrations": self.integrations.iter().map(integration).collect::<Vec<_>>()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "Task",
            &[
                "id",
                "objective",
                "kind",
                "acceptanceCriteria",
                "dependencies",
                "state",
                "attempts",
                "artifact",
                "verifications",
                "reviews",
                "integrations",
            ],
        )?;
        let criteria = array(value, "acceptanceCriteria")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or(DomainError::EmptyContent)
            })
            .collect::<DomainResult<Vec<_>>>()?;
        let dependencies = array(value, "dependencies")?
            .iter()
            .map(|v| {
                keys(v, &["task", "result"])?;
                Ok(TaskDependency {
                    task: TaskId::new(string(v, "task")?)?,
                    result: enumeration!(
                        &v["result"],
                        DependencyResult,
                        CodeIntegrated,
                        AnalysisArtifactAccepted
                    )?,
                })
            })
            .collect::<DomainResult<Vec<_>>>()?;
        let mut task = Self::new(
            TaskId::new(string(value, "id")?)?,
            string(value, "objective")?,
            enumeration!(&value["kind"], TaskKind, Code, Analysis)?,
            criteria,
            dependencies,
        )?;
        task.state = enumeration!(
            &value["state"],
            TaskState,
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
            Cancelled
        )?;
        task.attempts = array(value, "attempts")?
            .iter()
            .map(TaskAttempt::restore_checkpoint)
            .collect::<DomainResult<Vec<_>>>()?;
        task.artifact = optional(value, "artifact", ArtifactVersion::restore_checkpoint)?;
        task.verifications = array(value, "verifications")?
            .iter()
            .map(restore_verification)
            .collect::<DomainResult<Vec<_>>>()?;
        task.reviews = array(value, "reviews")?
            .iter()
            .map(restore_review)
            .collect::<DomainResult<Vec<_>>>()?;
        task.integrations = array(value, "integrations")?
            .iter()
            .map(restore_integration)
            .collect::<DomainResult<Vec<_>>>()?;
        task.validate_checkpoint()?;
        Ok(task)
    }
    fn validate_checkpoint(&self) -> DomainResult<()> {
        let mut ids = BTreeSet::new();
        let mut artifacts = Vec::new();
        for (index, attempt) in self.attempts.iter().enumerate() {
            require(ids.insert(attempt.id()), DomainError::DuplicateIdentity)?;
            require(
                index + 1 == self.attempts.len() || attempt.sealed,
                DomainError::InvalidTransition,
            )?;
            if index == 0 {
                require(
                    attempt.inputs.correction_of.is_none(),
                    DomainError::StaleEvidence,
                )?;
            } else {
                let previous = &self.attempts[index - 1];
                if let Some(source) = &attempt.inputs.correction_of {
                    require(
                        previous.artifacts.last() == Some(source)
                            && &attempt.inputs.base == source.revision(),
                        DomainError::StaleEvidence,
                    )?;
                } else {
                    require(previous.artifacts.is_empty(), DomainError::StaleEvidence)?;
                }
            }
            for artifact in &attempt.artifacts {
                require(
                    !artifacts.contains(artifact),
                    DomainError::DuplicateIdentity,
                )?;
                artifacts.push(artifact.clone());
            }
        }
        require(
            self.artifact.as_ref() == self.attempts.last().and_then(|a| a.artifacts.last()),
            DomainError::Ownership,
        )?;
        let mut checks = BTreeSet::new();
        for evidence in &self.verifications {
            require(
                artifacts.contains(&evidence.artifact),
                DomainError::Ownership,
            )?;
            require(
                checks.insert(&evidence.reference),
                DomainError::DuplicateIdentity,
            )?;
        }
        let mut reviews = BTreeSet::new();
        for evidence in &self.reviews {
            require(
                artifacts.contains(&evidence.artifact),
                DomainError::Ownership,
            )?;
            require(
                reviews.insert(&evidence.reference),
                DomainError::DuplicateIdentity,
            )?;
        }
        let mut integrations = BTreeSet::new();
        for (index, evidence) in self.integrations.iter().enumerate() {
            require(
                self.kind == TaskKind::Code
                    && evidence.task == self.id
                    && artifacts.contains(&evidence.artifact),
                DomainError::Ownership,
            )?;
            require(
                integrations.insert(&evidence.reference),
                DomainError::DuplicateIdentity,
            )?;
            require(
                self.verifications
                    .iter()
                    .any(|v| v.artifact == evidence.artifact && v.result == CheckResult::Passed)
                    && self.reviews.iter().any(|r| {
                        r.artifact == evidence.artifact && r.result == ReviewResult::Accepted
                    }),
                DomainError::MissingEvidence,
            )?;
            if evidence.validation == CheckResult::Passed {
                require(
                    index + 1 == self.integrations.len()
                        && self.state == TaskState::Completed
                        && Some(&evidence.artifact) == self.artifact.as_ref(),
                    DomainError::InvalidTransition,
                )?;
            }
        }
        let attempt = self.current_attempt();
        require(
            attempt.is_none_or(|a| !a.sealed),
            DomainError::InvalidTransition,
        )?;
        if matches!(self.state, TaskState::Assigned | TaskState::Running) {
            require(self.artifact.is_none(), DomainError::InvalidTransition)?;
        }
        match self.state {
            TaskState::Created | TaskState::Waiting => require(
                self.attempts.is_empty() && self.artifact.is_none(),
                DomainError::InvalidTransition,
            )?,
            TaskState::Ready => require(
                attempt.is_none_or(|a| {
                    matches!(
                        a.state(),
                        AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                    )
                }),
                DomainError::InvalidTransition,
            )?,
            TaskState::Assigned => require(
                attempt.is_some_and(|a| {
                    matches!(a.state(), AttemptState::Created | AttemptState::Admitted)
                }),
                DomainError::InvalidTransition,
            )?,
            TaskState::Running => require(
                attempt.is_some_and(|a| a.state() == AttemptState::Running),
                DomainError::InvalidTransition,
            )?,
            TaskState::NeedsReview
            | TaskState::NeedsWork
            | TaskState::Approved
            | TaskState::Completed => require(
                attempt.is_some_and(|a| a.state() == AttemptState::Succeeded),
                DomainError::InvalidTransition,
            )?,
            TaskState::Failed => require(
                attempt.is_some_and(|a| a.state() == AttemptState::Failed),
                DomainError::InvalidTransition,
            )?,
            TaskState::Blocked => require(
                attempt.is_some_and(|a| a.state() == AttemptState::Uncertain),
                DomainError::InvalidTransition,
            )?,
            TaskState::Cancelled => require(
                attempt.is_none_or(|a| {
                    matches!(
                        a.state(),
                        AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                    )
                }),
                DomainError::InvalidTransition,
            )?,
        }
        if self.state == TaskState::NeedsWork {
            let artifact = self.artifact.as_ref().ok_or(DomainError::MissingEvidence)?;
            require(
                self.verifications
                    .iter()
                    .any(|v| &v.artifact == artifact && v.result == CheckResult::Failed)
                    || self
                        .reviews
                        .iter()
                        .rev()
                        .find(|r| &r.artifact == artifact)
                        .is_some_and(|r| r.result == ReviewResult::Rejected),
                DomainError::MissingEvidence,
            )?;
        }
        if matches!(self.state, TaskState::Approved | TaskState::Completed) {
            require(self.artifact.is_some(), DomainError::MissingEvidence)?;
            let accepted = self
                .reviews
                .iter()
                .rev()
                .find(|r| Some(&r.artifact) == self.artifact.as_ref())
                .is_some_and(|r| r.result == ReviewResult::Accepted);
            require(accepted, DomainError::MissingEvidence)?;
            require(
                self.verifications
                    .iter()
                    .filter(|v| Some(&v.artifact) == self.artifact.as_ref())
                    .all(|v| v.result == CheckResult::Passed),
                DomainError::VerificationFailed,
            )?;
            if self.kind == TaskKind::Code {
                let matching: Vec<_> = self
                    .verifications
                    .iter()
                    .filter(|v| Some(&v.artifact) == self.artifact.as_ref())
                    .collect();
                require(
                    !matching.is_empty()
                        && matching.iter().all(|v| v.result == CheckResult::Passed),
                    DomainError::MissingEvidence,
                )?;
                if self.state == TaskState::Completed {
                    require(
                        self.integrations.last().is_some_and(|i| {
                            i.validation == CheckResult::Passed
                                && Some(&i.artifact) == self.artifact.as_ref()
                        }),
                        DomainError::MissingEvidence,
                    )?;
                }
            }
        }
        Ok(())
    }
}
impl DeliveryCandidate {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "DeliveryCandidate",
            json!({"run": self.run.as_str(), "repository": self.repository.as_str(), "result": self.result.as_str(), "targetBase": self.target_base.as_str(), "generation": self.generation, "artifacts": self.artifacts.iter().map(ArtifactVersion::checkpoint).collect::<Vec<_>>()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "DeliveryCandidate",
            &[
                "run",
                "repository",
                "result",
                "targetBase",
                "generation",
                "artifacts",
            ],
        )?;
        let generation = number(value, "generation")?;
        require(generation > 0, DomainError::InvalidVersion)?;
        Ok(Self {
            run: RunId::new(string(value, "run")?)?,
            repository: RepositoryId::new(string(value, "repository")?)?,
            result: RevisionId::new(string(value, "result")?)?,
            target_base: RevisionId::new(string(value, "targetBase")?)?,
            generation,
            artifacts: array(value, "artifacts")?
                .iter()
                .map(ArtifactVersion::restore_checkpoint)
                .collect::<DomainResult<Vec<_>>>()?,
        })
    }
}
fn application(value: &ApplicationEvidence) -> Value {
    json!({"run": value.run.as_str(), "repository": value.repository.as_str(), "result": value.result.as_str(), "targetBase": value.target_base.as_str(), "appliedRevision": value.applied_revision.as_str(), "reference": value.reference.as_str(), "validation": format!("{:?}", value.validation)})
}
fn restore_application(value: &Value) -> DomainResult<ApplicationEvidence> {
    keys(
        value,
        &[
            "run",
            "repository",
            "result",
            "targetBase",
            "appliedRevision",
            "reference",
            "validation",
        ],
    )?;
    Ok(ApplicationEvidence {
        run: RunId::new(string(value, "run")?)?,
        repository: RepositoryId::new(string(value, "repository")?)?,
        result: RevisionId::new(string(value, "result")?)?,
        target_base: RevisionId::new(string(value, "targetBase")?)?,
        applied_revision: RevisionId::new(string(value, "appliedRevision")?)?,
        reference: EvidenceId::new(string(value, "reference")?)?,
        validation: check_result(&value["validation"])?,
    })
}
impl Run {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "Run",
            json!({"id": self.id.as_str(), "project": self.project.as_ref().map(ProjectId::as_str), "repository": self.repository.as_ref().map(RepositoryId::as_str), "initialRevision": self.initial_revision.as_str(), "integrationOrder": self.integration_order.iter().map(|(task, evidence)| json!({"task": task.as_str(), "evidence": evidence.as_str()})).collect::<Vec<_>>(), "resultRevision": self.result_revision.as_str(), "delivery": format!("{:?}", self.delivery), "state": format!("{:?}", self.state), "tasks": self.tasks.values().map(Task::checkpoint).collect::<Vec<_>>(), "generation": self.generation, "candidate": self.candidate.as_ref().map(DeliveryCandidate::checkpoint), "approval": self.approval.as_ref().map(EvidenceId::as_str), "application": self.application.as_ref().map(application), "applicationFailures": self.application_failures.iter().map(application).collect::<Vec<_>>(), "conflicts": self.conflicts.iter().map(|c| json!({"candidate": c.candidate.checkpoint(), "reference": c.reference.as_str()})).collect::<Vec<_>>(), "cancellationRequested": self.cancellation_requested}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(
            value,
            "Run",
            &[
                "id",
                "project",
                "repository",
                "initialRevision",
                "integrationOrder",
                "resultRevision",
                "delivery",
                "state",
                "tasks",
                "generation",
                "candidate",
                "approval",
                "application",
                "applicationFailures",
                "conflicts",
                "cancellationRequested",
            ],
        )?;
        let mut tasks = BTreeMap::new();
        for task in array(value, "tasks")? {
            let task = Task::restore_checkpoint(task)?;
            require(
                tasks.insert(task.id().clone(), task).is_none(),
                DomainError::DuplicateIdentity,
            )?;
        }
        let conflicts = array(value, "conflicts")?
            .iter()
            .map(|v| {
                keys(v, &["candidate", "reference"])?;
                Ok(ApplicationConflict {
                    candidate: DeliveryCandidate::restore_checkpoint(&v["candidate"])?,
                    reference: EvidenceId::new(string(v, "reference")?)?,
                })
            })
            .collect::<DomainResult<Vec<_>>>()?;
        let run = Self {
            id: RunId::new(string(value, "id")?)?,
            project: optional(value, "project", |v| local(v, ProjectId::new))?,
            repository: optional(value, "repository", |v| local(v, RepositoryId::new))?,
            initial_revision: RevisionId::new(string(value, "initialRevision")?)?,
            integration_order: array(value, "integrationOrder")?
                .iter()
                .map(|v| {
                    keys(v, &["task", "evidence"])?;
                    Ok((
                        TaskId::new(string(v, "task")?)?,
                        EvidenceId::new(string(v, "evidence")?)?,
                    ))
                })
                .collect::<DomainResult<Vec<_>>>()?,
            result_revision: RevisionId::new(string(value, "resultRevision")?)?,
            delivery: enumeration!(
                &value["delivery"],
                DeliveryRequirement,
                FinalApplication,
                AcceptedArtifacts
            )?,
            state: enumeration!(
                &value["state"],
                RunState,
                Working,
                ReadyToApply,
                Applied,
                Blocked,
                Cancelled,
                Delivered
            )?,
            tasks,
            generation: number(value, "generation")?,
            candidate: optional(value, "candidate", DeliveryCandidate::restore_checkpoint)?,
            approval: optional(value, "approval", |v| local(v, EvidenceId::new))?,
            application: optional(value, "application", restore_application)?,
            application_failures: array(value, "applicationFailures")?
                .iter()
                .map(restore_application)
                .collect::<DomainResult<Vec<_>>>()?,
            conflicts,
            cancellation_requested: boolean(value, "cancellationRequested")?,
        };
        run.validate_checkpoint()?;
        Ok(run)
    }
    fn validate_checkpoint(&self) -> DomainResult<()> {
        require(
            self.project.is_some() == self.repository.is_some(),
            DomainError::Ownership,
        )?;
        require(
            self.delivery != DeliveryRequirement::FinalApplication || self.project.is_some(),
            DomainError::MissingEvidence,
        )?;
        let mut attempts = BTreeSet::new();
        let mut sessions = BTreeMap::new();
        let mut unresolved = BTreeSet::new();
        let mut pending = self.tasks.keys().cloned().collect::<BTreeSet<_>>();
        let mut resolved = BTreeSet::new();
        while !pending.is_empty() {
            let ready: Vec<_> = pending
                .iter()
                .filter(|id| {
                    self.tasks[*id]
                        .dependencies
                        .iter()
                        .all(|d| resolved.contains(&d.task))
                })
                .cloned()
                .collect();
            require(!ready.is_empty(), DomainError::DependencyPending)?;
            for id in ready {
                pending.remove(&id);
                resolved.insert(id);
            }
        }
        for task in self.tasks.values() {
            require(
                task.kind != TaskKind::Code
                    || self.delivery == DeliveryRequirement::FinalApplication,
                DomainError::Policy,
            )?;
            for dependency in &task.dependencies {
                let prerequisite = self.task(&dependency.task)?;
                require(
                    matches!(
                        (dependency.result, prerequisite.kind),
                        (DependencyResult::CodeIntegrated, TaskKind::Code)
                            | (
                                DependencyResult::AnalysisArtifactAccepted,
                                TaskKind::Analysis
                            )
                    ),
                    DomainError::Policy,
                )?;
                if !matches!(
                    task.state,
                    TaskState::Created | TaskState::Waiting | TaskState::Cancelled
                ) {
                    require(
                        prerequisite.releases(dependency.result),
                        DomainError::DependencyPending,
                    )?;
                }
            }
            for attempt in &task.attempts {
                require(
                    attempt.inputs.project == self.project,
                    DomainError::Ownership,
                )?;
                require(
                    attempts.insert(attempt.id()),
                    DomainError::DuplicateIdentity,
                )?;
                if let Some(previous) = sessions.insert(attempt.session.id(), &attempt.session) {
                    require(previous == &attempt.session, DomainError::Ownership)?;
                }
                if !matches!(
                    attempt.state(),
                    AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                ) {
                    require(
                        unresolved.insert(attempt.session.id()),
                        DomainError::InvalidTransition,
                    )?;
                }
            }
            for evidence in &task.integrations {
                require(evidence.run == self.id, DomainError::Ownership)?;
            }
        }
        let mut revision = &self.initial_revision;
        let mut integrated = BTreeSet::new();
        for (task_id, reference) in &self.integration_order {
            require(
                integrated.insert((task_id, reference)),
                DomainError::DuplicateIdentity,
            )?;
            let evidence = self
                .task(task_id)?
                .integrations
                .iter()
                .find(|e| &e.reference == reference)
                .ok_or(DomainError::MissingEvidence)?;
            require(&evidence.input_base == revision, DomainError::StaleEvidence)?;
            if evidence.validation == CheckResult::Passed {
                revision = &evidence.result_revision;
            }
        }
        require(
            integrated.len()
                == self
                    .tasks
                    .values()
                    .map(|t| t.integrations.len())
                    .sum::<usize>()
                && revision == &self.result_revision,
            DomainError::StaleEvidence,
        )?;
        // Every operation that observes these current blocking facts blocks the
        // Run. Historical failures may remain after an explicit cancellation.
        if self.tasks.values().any(|task| {
            task.state == TaskState::Blocked
                || task
                    .integrations
                    .last()
                    .is_some_and(|e| e.validation == CheckResult::Failed)
        }) {
            require(
                matches!(self.state, RunState::Blocked | RunState::Cancelled),
                DomainError::InvalidTransition,
            )?;
        }
        let all_completed =
            !self.tasks.is_empty() && self.tasks.values().all(|t| t.state == TaskState::Completed);
        if let Some(candidate) = &self.candidate {
            require(
                self.delivery == DeliveryRequirement::FinalApplication,
                DomainError::Policy,
            )?;
            require(
                candidate.run == self.id
                    && Some(&candidate.repository) == self.repository.as_ref()
                    && candidate.result == self.result_revision
                    && candidate.generation == self.generation
                    && all_completed,
                DomainError::StaleEvidence,
            )?;
            let artifacts = self
                .tasks
                .values()
                .filter_map(|t| t.artifact.clone())
                .collect::<Vec<_>>();
            require(candidate.artifacts == artifacts, DomainError::StaleEvidence)?;
            require(
                matches!(
                    self.state,
                    RunState::ReadyToApply | RunState::Applied | RunState::Delivered
                ),
                DomainError::InvalidTransition,
            )?;
        }
        require(
            self.approval.is_none() || self.candidate.is_some(),
            DomainError::MissingEvidence,
        )?;
        if self.state == RunState::ReadyToApply && !self.cancellation_requested {
            require(self.candidate.is_some(), DomainError::MissingEvidence)?;
        }
        for evidence in self.application.iter().chain(&self.application_failures) {
            require(
                evidence.run == self.id && Some(&evidence.repository) == self.repository.as_ref(),
                DomainError::Ownership,
            )?;
        }
        require(
            self.application_failures
                .iter()
                .all(|e| e.validation == CheckResult::Failed),
            DomainError::InvalidTransition,
        )?;
        if let Some(application) = &self.application {
            let candidate = self
                .candidate
                .as_ref()
                .ok_or(DomainError::MissingEvidence)?;
            require(
                application.validation == CheckResult::Passed
                    && application.result == candidate.result
                    && application.target_base == candidate.target_base
                    && self.approval.is_some()
                    && matches!(self.state, RunState::Applied | RunState::Delivered),
                DomainError::StaleEvidence,
            )?;
        }
        if self.state == RunState::Applied
            || (self.state == RunState::Delivered
                && self.delivery == DeliveryRequirement::FinalApplication)
        {
            require(
                self.application.is_some() && all_completed,
                DomainError::MissingEvidence,
            )?;
        }
        if self.state == RunState::Delivered {
            require(
                all_completed && !self.cancellation_requested,
                DomainError::MissingEvidence,
            )?;
        }
        if self.state == RunState::Cancelled {
            require(
                self.cancellation_requested && unresolved.is_empty(),
                DomainError::MissingEvidence,
            )?;
        }
        if self.cancellation_requested {
            require(
                self.candidate.is_none() && self.approval.is_none(),
                DomainError::InvalidTransition,
            )?;
        }
        let mut references = BTreeSet::new();
        for conflict in &self.conflicts {
            require(
                conflict.candidate.run == self.id
                    && Some(&conflict.candidate.repository) == self.repository.as_ref()
                    && conflict.candidate.generation <= self.generation,
                DomainError::Ownership,
            )?;
            require(
                references.insert(&conflict.reference),
                DomainError::DuplicateIdentity,
            )?;
        }
        Ok(())
    }
}
impl ConversationStore {
    pub fn checkpoint(&self) -> Value {
        wrap(
            "ConversationStore",
            json!({"projects": self.projects.values().map(Project::checkpoint).collect::<Vec<_>>(), "conversations": self.conversations.values().map(|c| json!({"id": c.id.as_str(), "project": c.project_id.as_ref().map(ProjectId::as_str), "messages": c.messages.iter().map(|m| json!({"order": m.order, "role": format!("{:?}", m.role), "content": m.content, "projectAtSend": m.project_at_send.as_ref().map(ProjectId::as_str)})).collect::<Vec<_>>(), "workReferences": c.work_references.iter().map(|w| json!({"runId": w.run_id.as_str(), "runResultRevision": w.run_result_revision.as_str(), "projectAtLink": w.project_at_link.as_ref().map(ProjectId::as_str), "taskId": w.task_id.as_ref().map(TaskId::as_str), "attemptId": w.attempt_id.as_ref().map(AttemptId::as_str), "planRevision": w.plan_revision.as_ref().map(RevisionId::as_str)})).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
        )
    }
    pub fn restore_checkpoint(value: &Value) -> DomainResult<Self> {
        let value = data(value, "ConversationStore", &["projects", "conversations"])?;
        let mut store = Self::new();
        for project in array(value, "projects")? {
            store.add_project(Project::restore_checkpoint(project)?)?;
        }
        for v in array(value, "conversations")? {
            keys(v, &["id", "project", "messages", "workReferences"])?;
            let project_id = optional(v, "project", |v| local(v, ProjectId::new))?;
            if let Some(id) = &project_id {
                require(store.projects.contains_key(id), DomainError::UnknownEntity)?;
            }
            let mut messages = Vec::new();
            for m in array(v, "messages")? {
                keys(m, &["order", "role", "content", "projectAtSend"])?;
                let order = number(m, "order")?;
                require(
                    order == messages.len() as u64 + 1,
                    DomainError::InvalidTransition,
                )?;
                let content = string(m, "content")?.to_owned();
                require(!content.trim().is_empty(), DomainError::EmptyContent)?;
                let project_at_send = optional(m, "projectAtSend", |v| local(v, ProjectId::new))?;
                if let Some(id) = &project_at_send {
                    require(store.projects.contains_key(id), DomainError::UnknownEntity)?;
                }
                messages.push(ConversationMessage {
                    order,
                    role: enumeration!(&m["role"], MessageRole, User, Orchestrator)?,
                    content,
                    project_at_send,
                });
            }
            require(
                messages
                    .first()
                    .is_some_and(|m| m.role == MessageRole::User),
                DomainError::EmptyContent,
            )?;
            let mut work_references = Vec::new();
            for w in array(v, "workReferences")? {
                keys(
                    w,
                    &[
                        "runId",
                        "runResultRevision",
                        "projectAtLink",
                        "taskId",
                        "attemptId",
                        "planRevision",
                    ],
                )?;
                let reference = ConversationWorkRef {
                    run_id: RunId::new(string(w, "runId")?)?,
                    run_result_revision: RevisionId::new(string(w, "runResultRevision")?)?,
                    project_at_link: optional(w, "projectAtLink", |v| local(v, ProjectId::new))?,
                    task_id: optional(w, "taskId", |v| local(v, TaskId::new))?,
                    attempt_id: optional(w, "attemptId", |v| local(v, AttemptId::new))?,
                    plan_revision: optional(w, "planRevision", |v| local(v, RevisionId::new))?,
                };
                require(
                    reference.attempt_id.is_none() || reference.task_id.is_some(),
                    DomainError::Ownership,
                )?;
                require(
                    reference.attempt_id.is_some() == reference.plan_revision.is_some(),
                    DomainError::MissingEvidence,
                )?;
                if let Some(id) = &reference.project_at_link {
                    require(store.projects.contains_key(id), DomainError::UnknownEntity)?;
                }
                require(
                    !work_references.contains(&reference),
                    DomainError::DuplicateIdentity,
                )?;
                work_references.push(reference);
            }
            let conversation = Conversation {
                id: ConversationId::new(string(v, "id")?)?,
                project_id,
                messages,
                work_references,
            };
            require(
                store
                    .conversations
                    .insert(conversation.id.clone(), conversation)
                    .is_none(),
                DomainError::DuplicateIdentity,
            )?;
        }
        Ok(store)
    }
    /// Reopens an already saved composer; HOME drafts have no such ID.
    pub fn composer(
        &self,
        id: &ConversationId,
        draft: impl Into<String>,
    ) -> DomainResult<PendingComposer> {
        let conversation = self.conversation(id).ok_or(DomainError::UnknownEntity)?;
        Ok(PendingComposer {
            project_id: conversation.project_id.clone(),
            draft: draft.into(),
            conversation_id: Some(id.clone()),
        })
    }
}
