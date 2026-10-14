use super::{ArtifactId, EvidenceId, RevisionId, RunId, TaskId};

/// Exact content and producer base; approvals never refer to an ID alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactVersion {
    pub(super) id: ArtifactId,
    pub(super) revision: RevisionId,
    pub(super) base: RevisionId,
}
impl ArtifactVersion {
    pub fn new(id: ArtifactId, revision: RevisionId, base: RevisionId) -> Self {
        Self { id, revision, base }
    }
    pub fn id(&self) -> &ArtifactId {
        &self.id
    }
    pub fn revision(&self) -> &RevisionId {
        &self.revision
    }
    pub fn base(&self) -> &RevisionId {
        &self.base
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckResult {
    Passed,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationEvidence {
    pub artifact: ArtifactVersion,
    pub reference: EvidenceId,
    pub result: CheckResult,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewResult {
    Accepted,
    Rejected,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewEvidence {
    pub artifact: ArtifactVersion,
    pub reference: EvidenceId,
    pub result: ReviewResult,
}

/// Host-supplied combined validation, not a Git operation or an inferred success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationEvidence {
    pub run: RunId,
    pub task: TaskId,
    pub artifact: ArtifactVersion,
    pub input_base: RevisionId,
    pub result_revision: RevisionId,
    pub reference: EvidenceId,
    pub validation: CheckResult,
}
