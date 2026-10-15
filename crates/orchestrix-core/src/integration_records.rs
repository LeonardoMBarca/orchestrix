//! Durable, provider-independent serial integration and application records.
use crate::{
    domain::LocalRef,
    evaluator::CheckSpec,
    git_records::{CapturedArtifact, WorktreeIntent, content_hash, is_hash},
    worker_records::exact_safe,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRequest {
    pub schema_version: u32,
    pub id: String,
    pub worker_id: String,
    pub artifact_revision: String,
    pub authorization_ref: String,
    pub authorized_integration_directory: PathBuf,
    pub target_directory: PathBuf,
    pub checks: Vec<CheckSpec>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequest {
    pub schema_version: u32,
    pub id: String,
    pub integration_id: String,
    pub candidate_revision: String,
    pub authorization_ref: String,
    pub valid_for_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRequest {
    pub schema_version: u32,
    pub id: String,
    pub integration_id: String,
    pub candidate_revision: String,
    pub authorization_ref: String,
}
/// The canonical checkout and exact current bytes are separate from a commit OID.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetState {
    pub directory: String,
    pub common_git_dir: String,
    pub symbolic_ref: String,
    pub head: String,
    pub index_sha256: String,
    pub working_sha256: String,
}
impl TargetState {
    pub fn validate(&self) -> bool {
        Path::new(&self.directory).is_absolute()
            && Path::new(&self.common_git_dir).is_absolute()
            && self.symbolic_ref.starts_with("refs/heads/")
            && !self.symbolic_ref.contains(['\0', '\r', '\n'])
            && oid(&self.head)
            && is_hash(&self.index_sha256)
            && is_hash(&self.working_sha256)
            && exact_safe(&json!(self), 16_384)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationIntent {
    pub launch_protocol: String,
    pub id: String,
    pub request_hash: String,
    pub worker_id: String,
    pub run_id: String,
    pub task_id: String,
    pub artifact: Value,
    pub capture_id: String,
    pub capture_revision: String,
    pub input_base: String,
    pub target: TargetState,
    pub worktree: WorktreeIntent,
    pub checks: Vec<CheckSpec>,
    pub authorization_ref: String,
    pub created_ms: u64,
}
impl IntegrationIntent {
    pub fn validate(&self) -> bool {
        self.launch_protocol == "guarded-git-and-checks-v1"
            && [
                &self.id,
                &self.worker_id,
                &self.run_id,
                &self.task_id,
                &self.authorization_ref,
            ]
            .iter()
            .all(|v| LocalRef::new(*v).is_ok())
            && is_hash(&self.request_hash)
            && is_hash(&self.capture_revision)
            && oid(&self.input_base)
            && self.target.validate()
            && self.target.head == self.input_base
            && self.worktree.validate()
            && self.worktree.run_id == self.run_id
            && self.worktree.task_id == self.task_id
            && self.worktree.base_oid == self.input_base
            && self.worktree.main_repository == self.target.directory
            && self.worktree.common_git_dir == self.target.common_git_dir
            && !self.checks.is_empty()
            && self.checks.len() <= 16
            && self
                .checks
                .iter()
                .map(|c| &c.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.checks.len()
            && self.checks.iter().all(|c| {
                LocalRef::new(&c.id).is_ok()
                    && LocalRef::new(&c.authorization_ref).is_ok()
                    && c.authorized
                    && c.executable.is_absolute()
                    && (1..=90_000).contains(&c.timeout_ms)
                    && c.arguments.len() <= 64
                    && c.arguments.iter().all(|v| !v.contains('\0'))
                    && c.executable_sha256.as_ref().is_none_or(|v| is_hash(v))
            })
            && self.created_ms > 0
            && exact_safe(&json!(self), 1_048_576)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationStatus {
    Reserved,
    Preparing,
    Materialized,
    Checking,
    Checked,
    CommitPlanned,
    Integrated,
    Prepared,
    Approved,
    Applying,
    Applied,
    Delivered,
    Blocked,
    Invalidated,
    Uncertain,
}
impl IntegrationStatus {
    pub fn blocks_admission(self) -> bool {
        matches!(
            self,
            Self::Reserved
                | Self::Preparing
                | Self::Materialized
                | Self::Checking
                | Self::Checked
                | Self::CommitPlanned
                | Self::Applying
                | Self::Uncertain
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationObservation {
    pub status: IntegrationStatus,
    pub details: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRecord {
    pub intent: IntegrationIntent,
    pub observations: Vec<IntegrationObservation>,
}
impl IntegrationRecord {
    pub fn status(&self) -> IntegrationStatus {
        self.observations
            .last()
            .map_or(IntegrationStatus::Reserved, |o| o.status)
    }
    pub fn detail(&self, key: &str) -> Option<&Value> {
        self.observations
            .iter()
            .rev()
            .find_map(|o| o.details.get(key))
    }
    pub fn snapshot(&self) -> Option<CapturedArtifact> {
        self.detail("snapshot")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    pub fn candidate_revision(&self) -> Option<String> {
        self.detail("candidate")
            .map(|v| content_hash(v.to_string().as_bytes()))
    }
    pub fn validate(&self) -> bool {
        let mut old = IntegrationStatus::Reserved;
        self.intent.validate()
            && self.observations.len() <= 128
            && self.observations.iter().all(|o| {
                let result = transition(old, o.status) && exact_safe(&o.details, 4_194_304);
                old = o.status;
                result
            })
    }
    pub fn summary(&self) -> Value {
        json!({"id":self.intent.id,"worker_id":self.intent.worker_id,"run_id":self.intent.run_id,"task_id":self.intent.task_id,
            "source_artifact_revision":self.intent.capture_revision,"status":self.status(),"input_base":self.intent.input_base,
            "result_revision":self.detail("resultRevision"),"candidate":self.detail("candidate"),"candidate_revision":self.candidate_revision(),
            "target":self.intent.target,"checks":self.detail("checks"),"approval":self.detail("approval"),
            "reason":self.observations.last().map(|o|&o.details["reason"]),"delivered":self.status()==IntegrationStatus::Delivered,
            "profile":"single-code-task-clean-target-v1"})
    }
}
pub(crate) fn transition(from: IntegrationStatus, to: IntegrationStatus) -> bool {
    use IntegrationStatus::*;
    if matches!(to, Blocked | Invalidated | Uncertain) {
        return !matches!(from, Delivered | Blocked | Invalidated);
    }
    matches!(
        (from, to),
        (Reserved, Preparing)
            | (Preparing, Materialized)
            | (Materialized, Checking)
            | (Checking, Checked)
            | (Checked, CommitPlanned)
            | (CommitPlanned, Integrated)
            | (Integrated, Prepared)
            | (Prepared, Approved)
            | (Approved, Applying)
            | (Applying, Applied)
            | (Applied, Delivered)
            | (Uncertain, Integrated)
            | (Uncertain, Applied)
    )
}
pub enum IntegrationCommand {
    Reserve(Box<IntegrationRecord>),
    Observe {
        id: String,
        status: IntegrationStatus,
        details: Value,
    },
}
impl IntegrationCommand {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Reserve(r) => &r.intent.id,
            Self::Observe { id, .. } => id,
        }
    }
    pub(crate) fn body(&self) -> Value {
        match self {
            Self::Reserve(r) => json!(["integration.reserve", r]),
            Self::Observe {
                id,
                status,
                details,
            } => json!(["integration.observe", id, status, details]),
        }
    }
    pub(crate) fn bounded(&self) -> bool {
        match self {
            Self::Reserve(r) => r.validate() && r.observations.is_empty(),
            Self::Observe { id, details, .. } => {
                LocalRef::new(id).is_ok() && exact_safe(details, 4_194_304)
            }
        }
    }
}
