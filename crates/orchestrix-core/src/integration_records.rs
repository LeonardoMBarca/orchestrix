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
            && self.privacy_valid()
    }

    fn privacy_valid(&self) -> bool {
        let mut body = json!(self);
        if self.worktree.preservation.is_some() {
            // Preserve the existing physical envelope bound as well as privacy
            // checks on all non-manifest fields. Every manifest field has been
            // checked by WorktreeIntent::validate above.
            if !serde_json::to_vec(&body).is_ok_and(|bytes| bytes.len() <= 1_048_576) {
                return false;
            }
            body["worktree"] = self.worktree.privacy_projection();
        }
        exact_safe(&body, 1_048_576)
    }
}

fn has_preserved_snapshot(details: &Value) -> bool {
    details
        .get("snapshot")
        .and_then(|snapshot| snapshot.get("preservation"))
        .is_some()
}

/// Project exactly one recognized, validated typed manifest. Arbitrary fields,
/// transcripts and text remain subject to the original privacy/node budgets.
fn details_safe(details: &Value) -> bool {
    if !has_preserved_snapshot(details) {
        return exact_safe(details, 4_194_304);
    }
    if !serde_json::to_vec(details).is_ok_and(|bytes| bytes.len() <= 4_194_304) {
        return false;
    }
    let Some(value) = details.get("snapshot") else {
        return false;
    };
    let Ok(snapshot) = serde_json::from_value::<CapturedArtifact>(value.clone()) else {
        return false;
    };
    if !snapshot.validate() {
        return false;
    }
    let Some(preservation) = &snapshot.preservation else {
        return false;
    };
    let mut projected = details.clone();
    projected["snapshot"]["preservation"] = preservation.privacy_identity();
    exact_safe(&projected, 4_194_304)
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
                let result = transition(old, o.status) && details_safe(&o.details);
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
            Self::Reserve(r) if r.intent.worktree.preservation.is_some() => json!([
                "integration.reserve",
                "opaque-base-v2",
                content_hash(&serde_json::to_vec(r).expect("typed integration serialization"))
            ]),
            Self::Reserve(r) => json!(["integration.reserve", r]),
            Self::Observe {
                id,
                status,
                details,
            } if has_preserved_snapshot(details) => json!([
                "integration.observe",
                id,
                status,
                "opaque-base-v2",
                content_hash(
                    &serde_json::to_vec(details).expect("typed observation serialization")
                )
            ]),
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
            Self::Observe { id, details, .. } => LocalRef::new(id).is_ok() && details_safe(details),
        }
    }
}

#[cfg(test)]
mod preservation_tests {
    use super::*;
    use crate::git_records::{BasePreservation, PreservedBaseEntry};

    fn fixture() -> (IntegrationRecord, CapturedArtifact) {
        let root = std::env::temp_dir();
        let directory = |tail: &str| root.join(tail).to_str().unwrap().to_owned();
        let manifest = BasePreservation {
            schema_version: 2,
            base_oid: "a".repeat(40),
            tree_oid: "b".repeat(40),
            editable_paths: vec!["editable.txt".into()],
            entries: (0..1000)
                .map(|index| PreservedBaseEntry {
                    path: format!("file-{index:04}.bin"),
                    mode: "100644".into(),
                    size: 0,
                    git_oid: "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391".into(),
                    sha256: content_hash(b""),
                })
                .collect(),
        };
        let mut snapshot = CapturedArtifact {
            preservation: Some(manifest.clone()),
            id: "capture".into(),
            revision: String::new(),
            operation_id: "operation".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: "a".repeat(40),
            origin: directory("owned/attempt"),
            files: vec![],
            exclusions: vec![],
            patch: String::new(),
            patch_sha256: content_hash(b""),
        };
        snapshot.revision = snapshot.content_revision();
        assert!(snapshot.validate());
        let record = IntegrationRecord {
            intent: IntegrationIntent {
                launch_protocol: "guarded-git-and-checks-v1".into(),
                id: "integration".into(),
                request_hash: "d".repeat(64),
                worker_id: "worker".into(),
                run_id: "run".into(),
                task_id: "task".into(),
                artifact: json!({"reference":"artifact"}),
                capture_id: snapshot.id.clone(),
                capture_revision: snapshot.revision.clone(),
                input_base: "a".repeat(40),
                target: TargetState {
                    directory: directory("repository"),
                    common_git_dir: directory("repository/.git"),
                    symbolic_ref: "refs/heads/main".into(),
                    head: "a".repeat(40),
                    index_sha256: "d".repeat(64),
                    working_sha256: "e".repeat(64),
                },
                worktree: WorktreeIntent {
                    preservation: Some(manifest),
                    operation_id: "operation".into(),
                    project_id: "project".into(),
                    repository_id: "repository".into(),
                    run_id: "run".into(),
                    task_id: "task".into(),
                    attempt_id: "attempt".into(),
                    base_oid: "a".repeat(40),
                    domain_base: "base".into(),
                    worktree_ref: "worktree".into(),
                    authorized_root: directory("owned"),
                    path: directory("owned/attempt"),
                    main_repository: directory("repository"),
                    common_git_dir: directory("repository/.git"),
                    branch: "orchestrix/attempt".into(),
                    ownership_token: "f".repeat(64),
                },
                checks: vec![CheckSpec {
                    id: "check".into(),
                    executable: root.join("check.exe"),
                    arguments: vec![],
                    authorization_ref: "owned-check".into(),
                    authorized: true,
                    timeout_ms: 1000,
                    executable_sha256: None,
                }],
                authorization_ref: "explicit-integration".into(),
                created_ms: 1,
            },
            observations: vec![],
        };
        (record, snapshot)
    }

    #[test]
    fn typed_large_inventory_preserves_bodies_and_privacy_without_small_command_node_limit() {
        let (record, snapshot) = fixture();
        let original = serde_json::to_vec(&record).unwrap();
        assert!(!exact_safe(&json!(record.intent), 1_048_576));
        assert!(record.validate());
        let command = IntegrationCommand::Reserve(Box::new(record.clone()));
        assert!(command.bounded());
        assert!(exact_safe(&command.body(), 1_048_576));
        assert_eq!(serde_json::to_vec(&record).unwrap(), original);
        let details = json!({"snapshot":snapshot});
        assert!(!exact_safe(&details, 4_194_304));
        assert!(details_safe(&details));
        let observation = IntegrationCommand::Observe {
            id: "integration".into(),
            status: IntegrationStatus::Materialized,
            details: details.clone(),
        };
        assert!(observation.bounded());
        assert!(exact_safe(&observation.body(), 1_048_576));
        for key in ["password", "transcript"] {
            let mut invalid = details.clone();
            invalid[key] = json!("synthetic-forbidden-content");
            assert!(!details_safe(&invalid));
        }
        let mut invalid = details.clone();
        invalid["snapshot"]["extra"] = json!("not-a-typed-artifact-field");
        assert!(!details_safe(&invalid));
        let mut invalid = details.clone();
        invalid["snapshot"]["preservation"]["entries"][0]["sha256"] = json!("not-a-hash");
        assert!(!details_safe(&invalid));
        let mut invalid = details;
        invalid["extra"] = json!("x".repeat(4_194_304));
        assert!(!details_safe(&invalid));
        let mut invalid = record.clone();
        invalid.intent.artifact["password"] = json!("synthetic-forbidden-content");
        assert!(!invalid.validate());
    }

    #[test]
    fn legacy_integration_command_bodies_are_unchanged() {
        let (mut record, _) = fixture();
        record.intent.worktree.preservation = None;
        assert!(record.validate());
        assert_eq!(
            IntegrationCommand::Reserve(Box::new(record.clone())).body(),
            json!(["integration.reserve", record])
        );
        let details = json!({"reason":"original-observation"});
        assert_eq!(
            IntegrationCommand::Observe {
                id: "integration".into(),
                status: IntegrationStatus::Preparing,
                details: details.clone()
            }
            .body(),
            json!([
                "integration.observe",
                "integration",
                IntegrationStatus::Preparing,
                details
            ])
        );
    }
}
