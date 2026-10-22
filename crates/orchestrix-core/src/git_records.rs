//! Durable Git provenance. These records contain no executor and cannot approve work.
use crate::domain::{
    ArtifactId, AttemptId, LocalRef, ProjectId, RepositoryId, RevisionId, RunId, TaskId,
};
use crate::persistence_privacy::sanitize_text;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

pub const MAX_CAPTURE_BYTES: usize = 4 * 1_048_576;
pub const MAX_FILE_BYTES: usize = 1_048_576;
pub const MAX_CAPTURE_FILES: usize = 128;
// Base preservation budgets are independent of textual edits and Context Packs.
pub const MAX_BASE_FILES: usize = 4096;
pub const MAX_BASE_BYTES: u64 = 64 * 1_048_576;
pub const MAX_OPAQUE_FILE_BYTES: u64 = 6 * 1_048_576;
pub const MAX_PRESERVATION_BYTES: usize = 1_048_576;
pub const MAX_WORKTREE_INTENT_BYTES: usize = 1_048_576;
pub const MAX_EDITABLE_FILES: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreservedBaseEntry {
    pub path: String,
    pub mode: String,
    pub size: u64,
    pub git_oid: String,
    pub sha256: String,
}

/// V2 contains identities and integrity, never the opaque file bodies. Absence
/// on historical records means the original textual profile, with no new defaults.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BasePreservation {
    pub schema_version: u32,
    pub base_oid: String,
    pub tree_oid: String,
    pub entries: Vec<PreservedBaseEntry>,
    pub editable_paths: Vec<String>,
}
impl BasePreservation {
    pub(crate) fn privacy_identity(&self) -> serde_json::Value {
        serde_json::json!({"schema_version":2,"manifest_sha256":content_hash(&serde_json::to_vec(self).expect("typed preservation serialization"))})
    }
    pub fn input_bounded(&self) -> bool {
        self.entries.len() <= MAX_BASE_FILES
            && self.editable_paths.len() <= MAX_EDITABLE_FILES
            && self
                .entries
                .iter()
                .map(|e| e.path.len().saturating_add(256))
                .sum::<usize>()
                <= MAX_PRESERVATION_BYTES
            && self.editable_paths.iter().all(|p| p.len() <= 4096)
            && self.base_oid.len() <= 64
            && self.tree_oid.len() <= 64
            && self.entries.iter().all(|e| {
                e.path.len() <= 4096
                    && e.mode.len() <= 6
                    && e.git_oid.len() <= 64
                    && e.sha256.len() <= 64
            })
    }
    pub fn validate(&self) -> bool {
        if !self.input_bounded()
            || self.schema_version != 2
            || !oid(&self.base_oid)
            || !oid(&self.tree_oid)
            || self.entries.len() > MAX_BASE_FILES
            || !editable_paths_valid(&self.editable_paths)
        {
            return false;
        }
        let mut previous = None;
        let mut aliases = BTreeSet::new();
        let mut bytes = 0u64;
        let mut editable_bytes = 0u64;
        for entry in &self.entries {
            if !relative_file_path(&entry.path)
                || entry
                    .path
                    .chars()
                    .any(|c| c.is_control() || "<>\"|?*".contains(c))
                || previous.is_some_and(|p: &String| p >= &entry.path)
                || !aliases.insert(entry.path.to_lowercase())
                || entry.mode != "100644"
                || entry.size > MAX_OPAQUE_FILE_BYTES
                || (self.editable_paths.binary_search(&entry.path).is_ok()
                    && entry.size > MAX_FILE_BYTES as u64)
                || !oid(&entry.git_oid)
                || !is_hash(&entry.sha256)
            {
                return false;
            }
            previous = Some(&entry.path);
            bytes = bytes.saturating_add(entry.size);
            if self.editable_paths.binary_search(&entry.path).is_ok() {
                editable_bytes = editable_bytes.saturating_add(entry.size);
            }
        }
        bytes <= MAX_BASE_BYTES
            && serde_json::to_vec(self).is_ok_and(|v| {
                v.len() <= MAX_PRESERVATION_BYTES
                    && (v.len() as u64).saturating_add(editable_bytes) <= MAX_CAPTURE_BYTES as u64
            })
    }
}
pub fn editable_paths_valid(paths: &[String]) -> bool {
    !paths.is_empty()
        && paths.len() <= MAX_EDITABLE_FILES
        && paths.windows(2).all(|p| p[0] < p[1])
        && paths.iter().all(|path| {
            relative_file_path(path)
                && !path
                    .chars()
                    .any(|c| c.is_control() || "<>\"|?*".contains(c))
        })
        && paths
            .iter()
            .map(|p| p.to_lowercase())
            .collect::<BTreeSet<_>>()
            .len()
            == paths.len()
}

// Missing is legacy; explicit null is not a version discriminator.
fn present_preservation<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<BasePreservation>, D::Error> {
    BasePreservation::deserialize(d).map(Some)
}

pub fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn safe(value: &str) -> bool {
    sanitize_text(value).is_ok_and(|clean| clean == value)
}
fn absolute(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && safe(value)
        && Path::new(value).is_absolute()
        && !Path::new(value)
            .components()
            .any(|part| matches!(part, Component::ParentDir))
}
pub fn relative_file_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && safe(value)
        && !value.contains(['\\', '\0', '\n', '\r'])
        && !Path::new(value).is_absolute()
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.eq_ignore_ascii_case(".git"))
        && !value.contains(':')
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorktreeIntent {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_preservation"
    )]
    pub preservation: Option<BasePreservation>,
    pub operation_id: String,
    pub project_id: String,
    pub repository_id: String,
    pub run_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub base_oid: String,
    pub domain_base: String,
    pub worktree_ref: String,
    pub authorized_root: String,
    pub path: String,
    pub main_repository: String,
    pub branch: String,
    pub common_git_dir: String,
    pub ownership_token: String,
}
impl WorktreeIntent {
    pub fn validate(&self) -> bool {
        self.preservation
            .as_ref()
            .is_none_or(|p| p.validate() && p.base_oid == self.base_oid)
            && LocalRef::new(&self.operation_id).is_ok()
            && ProjectId::new(&self.project_id).is_ok()
            && RepositoryId::new(&self.repository_id).is_ok()
            && RunId::new(&self.run_id).is_ok()
            && TaskId::new(&self.task_id).is_ok()
            && AttemptId::new(&self.attempt_id).is_ok()
            && RevisionId::new(&self.domain_base).is_ok()
            && LocalRef::new(&self.worktree_ref).is_ok()
            && oid(&self.base_oid)
            && is_hash(&self.ownership_token)
            && [
                &self.authorized_root,
                &self.path,
                &self.main_repository,
                &self.common_git_dir,
            ]
            .into_iter()
            .all(|v| absolute(v))
            && self.path != self.main_repository
            && self.path != self.authorized_root
            && Path::new(&self.path).starts_with(Path::new(&self.authorized_root))
            && self.branch.starts_with("orchestrix/")
            && !self.branch.ends_with('/')
            && self.branch.len() <= 240
            && self
                .branch
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/-_".contains(&b))
            && !self.branch.contains("//")
            && [
                &self.operation_id,
                &self.project_id,
                &self.repository_id,
                &self.run_id,
                &self.task_id,
                &self.attempt_id,
                &self.domain_base,
                &self.worktree_ref,
                &self.branch,
            ]
            .into_iter()
            .all(|v| safe(v))
            && (self.preservation.is_none()
                || serde_json::to_vec(self)
                    .is_ok_and(|body| body.len() <= MAX_WORKTREE_INTENT_BYTES))
    }

    /// Only for privacy validation of an already validated typed record. The
    /// complete immutable manifest remains stored and hashed, never redacted.
    pub(crate) fn privacy_projection(&self) -> serde_json::Value {
        let mut body = serde_json::json!(self);
        if let Some(preservation) = &self.preservation {
            body["preservation"] = preservation.privacy_identity();
        }
        body
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeState {
    Planned,
    Created,
    CleanupPlanned,
    Removed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorktreeObservation {
    pub kind: String,
    pub evidence_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorktreeRecord {
    pub intent: WorktreeIntent,
    pub state: WorktreeState,
    pub observations: Vec<WorktreeObservation>,
}
impl WorktreeRecord {
    pub fn validate(&self) -> bool {
        if !self.intent.validate() {
            return false;
        }
        let expected: &[&str] = match self.state {
            WorktreeState::Planned => &[],
            WorktreeState::Created => &["created"],
            WorktreeState::CleanupPlanned => &["created", "cleanup-planned"],
            WorktreeState::Removed => &["created", "cleanup-planned", "removed"],
        };
        if self.observations.len() > 128
            || self.observations.iter().any(|o| {
                !is_hash(&o.evidence_sha256)
                    || !matches!(
                        o.kind.as_str(),
                        "created"
                            | "cleanup-planned"
                            | "removed"
                            | "inspection-required"
                            | "create-failed"
                            | "cleanup-failed"
                            | "recovery-observed"
                    )
            })
        {
            return false;
        }
        let progress = self
            .observations
            .iter()
            .filter(|o| matches!(o.kind.as_str(), "created" | "cleanup-planned" | "removed"))
            .map(|o| o.kind.as_str())
            .collect::<Vec<_>>();
        if progress != expected {
            return false;
        }
        // Removal is terminal even for informational observations.
        !self
            .observations
            .iter()
            .enumerate()
            .any(|(index, o)| o.kind == "removed" && index + 1 != self.observations.len())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileContent {
    pub content: String,
    pub sha256: String,
    pub mode: String,
}
impl FileContent {
    pub fn new(content: String, mode: String) -> Result<Self, &'static str> {
        let item = Self {
            sha256: content_hash(content.as_bytes()),
            content,
            mode,
        };
        if item.validate() {
            Ok(item)
        } else {
            Err("unsupported-content")
        }
    }
    pub fn validate(&self) -> bool {
        self.mode == "100644"
            && self.content.len() <= MAX_FILE_BYTES
            && !self.content.contains('\0')
            && safe(&self.content)
            && self.sha256 == content_hash(self.content.as_bytes())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapturedFile {
    pub path: String,
    pub base: Option<FileContent>,
    pub index: Option<FileContent>,
    pub worktree: Option<FileContent>,
    pub untracked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CaptureExclusion {
    pub path: String,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapturedArtifact {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_preservation"
    )]
    pub preservation: Option<BasePreservation>,
    pub id: String,
    pub revision: String,
    pub operation_id: String,
    pub repository_id: String,
    pub run_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub base_oid: String,
    pub origin: String,
    pub files: Vec<CapturedFile>,
    pub exclusions: Vec<CaptureExclusion>,
    pub patch: String,
    pub patch_sha256: String,
}
impl CapturedArtifact {
    /// Stable hash of the exact serialized manifest/content, excluding its own revision.
    pub fn content_revision(&self) -> String {
        let mut value = serde_json::to_value(self).expect("Git DTO serialization is infallible");
        value
            .as_object_mut()
            .expect("Git DTO is an object")
            .remove("revision");
        content_hash(value.to_string().as_bytes())
    }
    pub fn validate(&self) -> bool {
        if self.preservation.as_ref().is_some_and(|p| {
            !p.validate()
                || p.base_oid != self.base_oid
                || !self.exclusions.is_empty()
                || self
                    .files
                    .iter()
                    .any(|f| p.editable_paths.binary_search(&f.path).is_err())
        }) || ArtifactId::new(&self.id).is_err()
            || !is_hash(&self.revision)
            || LocalRef::new(&self.operation_id).is_err()
            || RepositoryId::new(&self.repository_id).is_err()
            || RunId::new(&self.run_id).is_err()
            || TaskId::new(&self.task_id).is_err()
            || AttemptId::new(&self.attempt_id).is_err()
            || !oid(&self.base_oid)
            || !absolute(&self.origin)
            || self.files.len() > MAX_CAPTURE_FILES
            || self.exclusions.len() > MAX_CAPTURE_FILES
            || self.patch.len() > MAX_FILE_BYTES
            || !safe(&self.patch)
            || self.patch.contains('\0')
            || self.patch_sha256 != content_hash(self.patch.as_bytes())
        {
            return false;
        }
        let mut seen = BTreeSet::new();
        let mut bytes = self.patch.len()
            + self.preservation.as_ref().map_or(0, |p| {
                serde_json::to_vec(p).map_or(MAX_CAPTURE_BYTES + 1, |v| v.len())
            });
        for file in &self.files {
            if let Some(profile) = &self.preservation {
                let entry = profile
                    .entries
                    .binary_search_by(|e| e.path.cmp(&file.path))
                    .ok()
                    .map(|i| &profile.entries[i]);
                if match (entry, &file.base) {
                    (Some(entry), Some(base)) => {
                        base.sha256 != entry.sha256
                            || base.content.len() as u64 != entry.size
                            || base.mode != entry.mode
                    }
                    (None, None) => false,
                    _ => true,
                } {
                    return false;
                }
            }
            if !relative_file_path(&file.path)
                || !seen.insert(&file.path)
                || (file.base.is_none() && file.index.is_none() && file.worktree.is_none())
                || (file.untracked && (file.base.is_some() || file.index.is_some()))
            {
                return false;
            }
            for content in [&file.base, &file.index, &file.worktree]
                .into_iter()
                .flatten()
            {
                if !content.validate() {
                    return false;
                }
                bytes = bytes.saturating_add(content.content.len());
            }
        }
        for exclusion in &self.exclusions {
            if !relative_file_path(&exclusion.path)
                || !seen.insert(&exclusion.path)
                || exclusion.reason.is_empty()
                || exclusion.reason.len() > 128
                || !safe(&exclusion.reason)
            {
                return false;
            }
        }
        bytes <= MAX_CAPTURE_BYTES
            && self.revision == self.content_revision()
            && [
                &self.id,
                &self.operation_id,
                &self.repository_id,
                &self.run_id,
                &self.task_id,
                &self.attempt_id,
            ]
            .into_iter()
            .all(|v| safe(v))
    }
}

#[derive(Clone, Debug)]
pub enum GitCommand {
    Plan(WorktreeIntent),
    ConfirmCreated {
        operation_id: String,
        observation: WorktreeObservation,
    },
    RecordArtifact(CapturedArtifact),
    PlanCleanup {
        operation_id: String,
        observation: WorktreeObservation,
    },
    ConfirmRemoved {
        operation_id: String,
        observation: WorktreeObservation,
    },
    Observe {
        operation_id: String,
        observation: WorktreeObservation,
    },
}
impl GitCommand {
    /// Cheap bounds before canonical serialization or hashing of public DTO input.
    pub fn input_bounded(&self) -> bool {
        match self {
            Self::Plan(i) => {
                [
                    &i.operation_id,
                    &i.project_id,
                    &i.repository_id,
                    &i.run_id,
                    &i.task_id,
                    &i.attempt_id,
                    &i.base_oid,
                    &i.domain_base,
                    &i.worktree_ref,
                    &i.authorized_root,
                    &i.path,
                    &i.main_repository,
                    &i.branch,
                    &i.common_git_dir,
                    &i.ownership_token,
                ]
                .into_iter()
                .all(|v| v.len() <= 4096)
                    && i.preservation
                        .as_ref()
                        .is_none_or(BasePreservation::input_bounded)
                    && (i.preservation.is_none()
                        || serde_json::to_vec(i)
                            .is_ok_and(|body| body.len() <= MAX_WORKTREE_INTENT_BYTES))
            }
            Self::RecordArtifact(a) => {
                if a.preservation.as_ref().is_some_and(|p| !p.input_bounded())
                    || a.files.len() > MAX_CAPTURE_FILES
                    || a.exclusions.len() > MAX_CAPTURE_FILES
                    || a.patch.len() > MAX_FILE_BYTES
                    || [
                        &a.id,
                        &a.revision,
                        &a.operation_id,
                        &a.repository_id,
                        &a.run_id,
                        &a.task_id,
                        &a.attempt_id,
                        &a.base_oid,
                        &a.origin,
                        &a.patch_sha256,
                    ]
                    .into_iter()
                    .any(|v| v.len() > 4096)
                {
                    return false;
                }
                let mut total = a.patch.len()
                    + a.preservation.as_ref().map_or(0, |p| {
                        serde_json::to_vec(p).map_or(MAX_CAPTURE_BYTES + 1, |v| v.len())
                    });
                for file in &a.files {
                    if file.path.len() > 4096 {
                        return false;
                    }
                    for content in [&file.base, &file.index, &file.worktree]
                        .into_iter()
                        .flatten()
                    {
                        if content.content.len() > MAX_FILE_BYTES
                            || content.sha256.len() > 64
                            || content.mode.len() > 6
                        {
                            return false;
                        }
                        total = total.saturating_add(content.content.len());
                    }
                }
                total <= MAX_CAPTURE_BYTES
                    && a.exclusions
                        .iter()
                        .all(|e| e.path.len() <= 4096 && e.reason.len() <= 128)
            }
            Self::ConfirmCreated {
                operation_id,
                observation,
            }
            | Self::PlanCleanup {
                operation_id,
                observation,
            }
            | Self::ConfirmRemoved {
                operation_id,
                observation,
            }
            | Self::Observe {
                operation_id,
                observation,
            } => {
                operation_id.len() <= 80
                    && observation.kind.len() <= 32
                    && observation.evidence_sha256.len() <= 64
            }
        }
    }
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Plan(i) => &i.operation_id,
            Self::RecordArtifact(a) => &a.operation_id,
            Self::ConfirmCreated { operation_id, .. }
            | Self::PlanCleanup { operation_id, .. }
            | Self::ConfirmRemoved { operation_id, .. }
            | Self::Observe { operation_id, .. } => operation_id,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Plan(_) => "git.plan",
            Self::ConfirmCreated { .. } => "git.created",
            Self::RecordArtifact(_) => "git.capture",
            Self::PlanCleanup { .. } => "git.cleanup-plan",
            Self::ConfirmRemoved { .. } => "git.removed",
            Self::Observe { .. } => "git.observation",
        }
    }
    /// Large exact bytes are validated and hashed separately, never passed through a redactor.
    pub fn fingerprint_body(&self) -> serde_json::Value {
        use serde_json::json;
        match self {
            Self::Plan(i) if i.preservation.is_some() => json!([
                self.name(),
                "opaque-base-v2",
                content_hash(&serde_json::to_vec(i).expect("typed worktree serialization"))
            ]),
            Self::Plan(i) => json!([self.name(), i]),
            Self::RecordArtifact(a) => json!([self.name(), a.id, a.revision, a.content_revision()]),
            Self::ConfirmCreated {
                operation_id,
                observation,
            }
            | Self::PlanCleanup {
                operation_id,
                observation,
            }
            | Self::ConfirmRemoved {
                operation_id,
                observation,
            }
            | Self::Observe {
                operation_id,
                observation,
            } => json!([self.name(), operation_id, observation]),
        }
    }
}
