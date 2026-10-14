//! Durable manual-worker lifecycle; no automatic dispatch on restoration.
use crate::git_records::{content_hash, is_hash};
use crate::persistence_privacy::{sanitize_checkpoint, sanitize_json};
use crate::{domain::*, persistence::AttemptSnapshot, persistence_records::DefinitionInput};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkerIntent {
    pub id: String,
    pub request_hash: String,
    pub run_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub project_id: String,
    pub repository_id: String,
    pub base_oid: String,
    pub policy: Value,
    pub context: Value,
    pub authorized_checks: Value,
}
impl WorkerIntent {
    pub fn validate(&self) -> bool {
        LocalRef::new(&self.id).is_ok()
            && is_hash(&self.request_hash)
            && RunId::new(&self.run_id).is_ok()
            && TaskId::new(&self.task_id).is_ok()
            && AttemptId::new(&self.attempt_id).is_ok()
            && ProjectId::new(&self.project_id).is_ok()
            && RepositoryId::new(&self.repository_id).is_ok()
            && matches!(self.base_oid.len(), 40 | 64)
            && self
                .base_oid
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && self
                .authorized_checks
                .as_array()
                .is_some_and(|a| a.len() <= 16)
            && exact_safe(&json!(self), 1_048_576)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerStatus {
    Reserved,
    DispatchPlanned,
    RuntimeObserved,
    Verified,
    NeedsWork,
    NoChecks,
    Failed,
    Cancelled,
    Uncertain,
    Blocked,
    Invalidated,
}
impl WorkerStatus {
    pub fn terminal(self) -> bool {
        !matches!(
            self,
            Self::Reserved | Self::DispatchPlanned | Self::RuntimeObserved
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkerObservation {
    pub status: WorkerStatus,
    pub details: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkerRecord {
    pub intent: WorkerIntent,
    pub observations: Vec<WorkerObservation>,
}
impl WorkerRecord {
    pub fn status(&self) -> WorkerStatus {
        self.observations
            .last()
            .map_or(WorkerStatus::Reserved, |o| o.status)
    }
    pub fn validate(&self) -> bool {
        let mut previous = WorkerStatus::Reserved;
        self.intent.validate()
            && self.observations.len() <= 64
            && exact_safe(&json!(self), 4 * 1_048_576)
            && self.observations.iter().all(|o| {
                let valid = transition(previous, o.status) && exact_safe(&o.details, 65_536);
                previous = o.status;
                valid
            })
    }
    pub fn summary(&self) -> Value {
        let mut details = serde_json::Map::new();
        if let Some(last) = self.observations.last() {
            for key in [
                "reason",
                "inspectionRequired",
                "synthetic",
                "artifactRevision",
                "checks",
                "checksOmitted",
                "review",
                "delivered",
                "treeStopped",
                "streamsClosed",
                "dispatch",
                "fault",
                "worktreeOperation",
            ] {
                if let Some(value) = last.details.get(key) {
                    details.insert(key.into(), value.clone());
                }
            }
        }
        json!({"workerId":self.intent.id,"runId":self.intent.run_id,"taskId":self.intent.task_id,"attemptId":self.intent.attempt_id,
            "status":self.status(),"resumeAutomatically":false,"details":details})
    }
}
pub(crate) fn transition(from: WorkerStatus, to: WorkerStatus) -> bool {
    match from {
        WorkerStatus::Reserved => matches!(
            to,
            WorkerStatus::DispatchPlanned | WorkerStatus::Blocked | WorkerStatus::Uncertain
        ),
        WorkerStatus::DispatchPlanned => matches!(
            to,
            WorkerStatus::RuntimeObserved | WorkerStatus::Uncertain | WorkerStatus::Blocked
        ),
        WorkerStatus::RuntimeObserved => matches!(
            to,
            WorkerStatus::Verified
                | WorkerStatus::NeedsWork
                | WorkerStatus::NoChecks
                | WorkerStatus::Failed
                | WorkerStatus::Cancelled
                | WorkerStatus::Uncertain
                | WorkerStatus::Invalidated
        ),
        WorkerStatus::Verified => to == WorkerStatus::Invalidated,
        _ => false,
    }
}
pub(crate) fn exact_safe(value: &Value, maximum: usize) -> bool {
    serde_json::to_vec(value).is_ok_and(|b| b.len() <= maximum)
        && sanitize_json(value).is_ok_and(|v| v == *value)
}
pub struct WorkerReservation {
    pub intent: WorkerIntent,
    pub project: Project,
    pub connection: Connection,
    pub session: RuntimeSessionRef,
    /// Contains the single ready task, before assignment.
    pub run: Run,
    pub attempt: TaskAttempt,
    pub snapshot: AttemptSnapshot,
    pub definitions: Vec<DefinitionInput>,
    pub diagnostics: Vec<crate::persistence_records::DiagnosticInput>,
}
pub enum WorkerCommand {
    Reserve(Box<WorkerReservation>),
    Observe {
        id: String,
        status: WorkerStatus,
        details: Value,
    },
}
impl WorkerCommand {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Reserve(r) => &r.intent.id,
            Self::Observe { id, .. } => id,
        }
    }
    pub(crate) fn body(&self) -> Value {
        match self {
            Self::Reserve(r) => json!([
                "worker.reserve",
                r.intent,
                r.project.checkpoint(),
                r.connection.checkpoint(),
                r.session.checkpoint(),
                r.run.checkpoint(),
                r.attempt.checkpoint(),
                r.snapshot,
                r.definitions,
                r.diagnostics
            ]),
            Self::Observe {
                id,
                status,
                details,
            } => json!(["worker.observe", id, status, details]),
        }
    }
    pub(crate) fn bounded(&self) -> bool {
        let body = self.body();
        match self {
            Self::Reserve(r) => {
                // This command composes independently bounded documents. The
                // same descriptor occurs in the connection, observer, policy
                // and diagnostic; it is not one provider document's node budget.
                let components = [
                    json!(r.intent),
                    r.project.checkpoint(),
                    r.connection.checkpoint(),
                    r.session.checkpoint(),
                    r.run.checkpoint(),
                    r.attempt.checkpoint(),
                    json!(r.snapshot),
                ];
                bounded_reservation(
                    &body,
                    components
                        .into_iter()
                        .chain(r.definitions.iter().map(|d| json!(d)))
                        .chain(r.diagnostics.iter().map(|d| json!(d))),
                )
            }
            Self::Observe { .. } => exact_safe(&body, 1_048_576),
        }
    }
    pub(crate) fn fingerprint(&self) -> Value {
        json!(["worker", content_hash(self.body().to_string().as_bytes())])
    }
}

fn bounded_reservation(body: &Value, components: impl IntoIterator<Item = Value>) -> bool {
    // The existing aggregate profile still scans every key/string and rejects
    // redaction, omission and collisions. Individual documents retain the
    // ordinary depth/node/text budgets; the command retains its 1 MiB ceiling.
    serde_json::to_vec(body).is_ok_and(|bytes| bytes.len() <= 1_048_576)
        && components
            .into_iter()
            .all(|part| exact_safe(&part, 1_048_576))
        && sanitize_checkpoint(body).is_ok_and(|cleaned| cleaned == *body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composed_documents_do_not_share_one_documents_node_budget() {
        let part = json!(vec![Value::Null; 3_000]);
        let parts = vec![part.clone(), part];
        let body = json!(parts);
        assert!(sanitize_json(&body).is_err());
        assert!(bounded_reservation(&body, parts));
    }

    #[test]
    fn every_component_keeps_the_ordinary_node_and_depth_limits() {
        let oversized = json!(vec![Value::Null; 4_096]);
        assert!(!bounded_reservation(&json!([oversized]), [oversized]));
        let mut deep = Value::Null;
        for _ in 0..33 {
            deep = json!([deep]);
        }
        assert!(!bounded_reservation(&json!([deep]), [deep]));
    }

    #[test]
    fn secrets_and_transcripts_remain_rejected_without_silent_changes() {
        for part in [
            json!({"api_key":"synthetic-credential"}),
            json!({"value":"password=synthetic-credential"}),
            json!({"providerTranscript":"synthetic private conversation"}),
        ] {
            assert!(!bounded_reservation(&json!([part]), [part]));
        }
    }

    #[test]
    fn total_reservation_byte_budget_is_not_expanded_by_component_validation() {
        let part = json!("x".repeat(600_000));
        assert!(exact_safe(&part, 1_048_576));
        let parts = vec![part.clone(), part];
        assert!(!bounded_reservation(&json!(parts), parts));
    }
}
