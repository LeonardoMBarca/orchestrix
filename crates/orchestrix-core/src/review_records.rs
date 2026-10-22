//! Independent review identities and structured results; no provider or database effects.
use crate::{
    domain::*,
    git_records::{is_hash, relative_file_path},
    policy::{PolicyDocument, TaskClassification},
    worker::{ManualTaskRequest, WorkerError},
    worker_records::exact_safe,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewBudget {
    pub max_corrections: u32,
    pub max_reviews: u32,
    pub max_elapsed_ms: u64,
}
impl ReviewBudget {
    pub fn validate(&self) -> bool {
        self.max_corrections <= 3
            && self.max_reviews > 0
            && self.max_reviews <= 4
            && (1_000..=3_600_000).contains(&self.max_elapsed_ms)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    pub schema_version: u32,
    pub id: String,
    pub worker_id: String,
    pub artifact_revision: String,
    pub authorization_ref: String,
    pub policy: PolicyDocument,
    pub classification: TaskClassification,
    pub budget: ReviewBudget,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionRequest {
    pub schema_version: u32,
    pub id: String,
    pub review_id: String,
    pub authorization_ref: String,
    pub task: ManualTaskRequest,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Accepted,
    Rejected,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub path: String,
    pub artifact_revision: String,
    pub line_start: u32,
    pub line_end: u32,
    pub explanation: String,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewOutput {
    pub schema_version: u32,
    pub artifact_revision: String,
    pub verdict: Verdict,
    pub summary: String,
    pub findings: Vec<Finding>,
}
impl ReviewOutput {
    pub fn parse(
        text: &str,
        revision: &str,
        artifact: &crate::git_records::CapturedArtifact,
    ) -> Result<Self, WorkerError> {
        if text.len() > 65_536 {
            return Err(WorkerError("review-result-budget"));
        }
        let value: Self =
            serde_json::from_str(text).map_err(|_| WorkerError("invalid-review-result"))?;
        if !value.validate(revision)
            || value.findings.iter().any(|f| {
                !artifact.files.iter().any(|file| {
                    file.path == f.path
                        && file
                            .worktree
                            .as_ref()
                            .or(file.base.as_ref())
                            .is_some_and(|c| {
                                f.line_end as usize <= c.content.lines().count().max(1)
                            })
                })
            })
        {
            return Err(WorkerError("invalid-review-result"));
        }
        Ok(value)
    }
    pub fn validate(&self, revision: &str) -> bool {
        self.schema_version == 1
            && self.artifact_revision == revision
            && is_hash(revision)
            && !self.summary.trim().is_empty()
            && self.summary.len() <= 4096
            && self.findings.len() <= 32
            && (self.verdict != Verdict::Rejected || !self.findings.is_empty())
            && (self.verdict != Verdict::Accepted || self.findings.is_empty())
            && exact_safe(&json!(self), 65_536)
            && {
                let mut ids = std::collections::BTreeSet::new();
                self.findings.iter().all(|f| {
                    LocalRef::new(&f.id).is_ok()
                        && ids.insert(&f.id)
                        && relative_file_path(&f.path)
                        && f.artifact_revision == revision
                        && f.line_start > 0
                        && f.line_end >= f.line_start
                        && !f.explanation.trim().is_empty()
                        && f.explanation.len() <= 4096
                        && !f.evidence.trim().is_empty()
                        && f.evidence.len() <= 4096
                })
            }
    }
    pub fn schema() -> Value {
        json!({"type":"object","additionalProperties":false,"required":["schema_version","artifact_revision","verdict","summary","findings"],"properties":{"schema_version":{"type":"integer","enum":[1]},"artifact_revision":{"type":"string"},"verdict":{"type":"string","enum":["accepted","rejected"]},"summary":{"type":"string"},"findings":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["id","severity","path","artifact_revision","line_start","line_end","explanation","evidence"],"properties":{"id":{"type":"string"},"severity":{"type":"string","enum":["critical","high","medium","low"]},"path":{"type":"string"},"artifact_revision":{"type":"string"},"line_start":{"type":"integer"},"line_end":{"type":"integer"},"explanation":{"type":"string"},"evidence":{"type":"string"}}}}}})
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewStatus {
    Reserved,
    Preparing,
    SupervisorReady,
    DispatchPlanned,
    Accepted,
    Rejected,
    Invalid,
    Unavailable,
    Cancelled,
    Uncertain,
    Invalidated,
    SetAside,
}
impl ReviewStatus {
    pub fn terminal(self) -> bool {
        !matches!(
            self,
            Self::Reserved | Self::Preparing | Self::SupervisorReady | Self::DispatchPlanned
        )
    }
    pub fn blocks_admission(self) -> bool {
        !self.terminal() || self == Self::Uncertain
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewIntent {
    pub id: String,
    pub request_hash: String,
    pub worker_id: String,
    pub run_id: String,
    pub task_id: String,
    pub source_attempt_id: String,
    pub artifact_id: String,
    pub artifact_revision: String,
    pub base_oid: String,
    pub session_id: String,
    pub attempt_id: String,
    pub authorization_ref: String,
    pub budget: ReviewBudget,
    pub started_ms: u64,
    pub deadline_ms: u64,
    pub policy: Value,
    pub context: Value,
    pub runtime_request: Value,
    pub checks: Value,
}
impl ReviewIntent {
    pub fn validate(&self) -> bool {
        [
            &self.id,
            &self.worker_id,
            &self.run_id,
            &self.task_id,
            &self.source_attempt_id,
            &self.artifact_id,
            &self.session_id,
            &self.attempt_id,
            &self.authorization_ref,
        ]
        .iter()
        .all(|v| LocalRef::new(v.as_str()).is_ok())
            && self.source_attempt_id != self.attempt_id
            && is_hash(&self.request_hash)
            && is_hash(&self.artifact_revision)
            && self.budget.validate()
            && self.deadline_ms > self.started_ms
            && self.deadline_ms - self.started_ms <= self.budget.max_elapsed_ms
            && exact_safe(&json!(self), 1_048_576)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewObservation {
    pub status: ReviewStatus,
    pub details: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    pub intent: ReviewIntent,
    pub observations: Vec<ReviewObservation>,
}
impl ReviewRecord {
    pub fn status(&self) -> ReviewStatus {
        self.observations
            .last()
            .map_or(ReviewStatus::Reserved, |o| o.status)
    }
    pub fn output(&self) -> Option<ReviewOutput> {
        self.observations.iter().rev().find_map(|o| {
            o.details
                .get("output")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
        })
    }
    pub fn validate(&self) -> bool {
        let mut prior = ReviewStatus::Reserved;
        self.intent.validate()
            && self.observations.len() <= 128
            && self.observations.iter().all(|o| {
                let valid = transition(prior, o.status) && exact_safe(&o.details, 131_072);
                prior = o.status;
                valid
            })
    }
    pub fn summary(&self) -> Value {
        let effective = self.observations.iter().rev().find_map(|observation| {
            observation
                .details
                .get("observer")
                .and_then(|observer| observer["snapshot"].get("effective"))
                .or_else(|| {
                    observation
                        .details
                        .get("runtimeEvent")
                        .and_then(|event| event.get("effective"))
                })
        });
        let synthetic = self.observations.iter().find_map(|observation| {
            observation
                .details
                .get("synthetic")
                .and_then(Value::as_bool)
        });
        json!({"id":self.intent.id,"worker_id":self.intent.worker_id,"run_id":self.intent.run_id,"task_id":self.intent.task_id,"artifact_revision":self.intent.artifact_revision,"reviewer_session_id":self.intent.session_id,"status":self.status(),"budget":self.intent.budget,"deadline_ms":self.intent.deadline_ms,"result":self.output(),"requested":self.intent.runtime_request["requested"],"effective":effective,"policy_revision":crate::git_records::content_hash(self.intent.policy.to_string().as_bytes()),"fallback_reasons":self.intent.policy["route"]["value"]["fallback_reasons"],"synthetic":synthetic,"historical":true,"delivered":false})
    }
}
pub(crate) fn transition(from: ReviewStatus, to: ReviewStatus) -> bool {
    use ReviewStatus::*;
    match from {
        Reserved => matches!(to, Preparing | Unavailable | Cancelled | Uncertain),
        Preparing => matches!(to, SupervisorReady | Unavailable | Cancelled | Uncertain),
        SupervisorReady => matches!(to, DispatchPlanned | Unavailable | Cancelled | Uncertain),
        DispatchPlanned => matches!(
            to,
            DispatchPlanned
                | Accepted
                | Rejected
                | Invalid
                | Unavailable
                | Cancelled
                | Uncertain
                | Invalidated
        ),
        Accepted | Rejected => to == Invalidated,
        Uncertain => to == SetAside,
        _ => false,
    }
}
pub enum ReviewCommand {
    Reserve {
        record: Box<ReviewRecord>,
        session: RuntimeSessionRef,
    },
    Observe {
        id: String,
        status: ReviewStatus,
        details: Value,
    },
}
impl ReviewCommand {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Reserve { record, .. } => &record.intent.id,
            Self::Observe { id, .. } => id,
        }
    }
    pub(crate) fn body(&self) -> Value {
        match self {
            Self::Reserve { record, session } => {
                json!(["review.reserve", record, session.checkpoint()])
            }
            Self::Observe {
                id,
                status,
                details,
            } => json!(["review.observe", id, status, details]),
        }
    }
    pub(crate) fn bounded(&self) -> bool {
        exact_safe(&self.body(), 1_048_576)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_records::{CapturedArtifact, CapturedFile, FileContent, content_hash};
    fn artifact() -> CapturedArtifact {
        let before = FileContent::new("before\n".into(), "100644".into()).unwrap();
        let after = FileContent::new("after\n".into(), "100644".into()).unwrap();
        let mut artifact = CapturedArtifact {
            preservation: None,
            id: "artifact".into(),
            revision: "0".repeat(64),
            operation_id: "operation".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "implementation".into(),
            base_oid: "a".repeat(40),
            origin: std::env::temp_dir().to_string_lossy().into_owned(),
            files: vec![CapturedFile {
                path: "fixture.txt".into(),
                base: Some(before.clone()),
                index: Some(before),
                worktree: Some(after),
                untracked: false,
            }],
            exclusions: vec![],
            patch: String::new(),
            patch_sha256: content_hash(b""),
        };
        artifact.revision = artifact.content_revision();
        assert!(artifact.validate());
        artifact
    }
    #[test]
    fn structured_findings_reject_wrong_version_paths_lines_duplicates_and_unstructured_approval() {
        let artifact = artifact();
        let revision = &artifact.revision;
        let output = json!({"schema_version":1,"artifact_revision":revision,"verdict":"rejected","summary":"Exact review","findings":[{"id":"finding","severity":"high","path":"fixture.txt","artifact_revision":revision,"line_start":1,"line_end":1,"explanation":"Concrete issue","evidence":"file content is after"}]});
        assert!(ReviewOutput::parse(&output.to_string(), revision, &artifact).is_ok());
        assert!(ReviewOutput::parse("Looks good", revision, &artifact).is_err());
        for (mut invalid, path, value) in [
            (output.clone(), "/artifact_revision", json!("b".repeat(64))),
            (output.clone(), "/schema_version", json!(2)),
            (output.clone(), "/verdict", json!("accepted")),
            (output.clone(), "/findings/0/path", json!("../private.txt")),
            (output.clone(), "/findings/0/path", json!("missing.txt")),
            (output.clone(), "/findings/0/line_start", json!(0)),
            (output.clone(), "/findings/0/line_end", json!(2)),
            (output.clone(), "/findings/0/evidence", json!("")),
        ] {
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(
                ReviewOutput::parse(&invalid.to_string(), revision, &artifact).is_err(),
                "{path}"
            );
        }
        let mut duplicate = output.clone();
        let finding = duplicate["findings"][0].clone();
        duplicate["findings"].as_array_mut().unwrap().push(finding);
        assert!(ReviewOutput::parse(&duplicate.to_string(), revision, &artifact).is_err());
        let mut unknown = output.clone();
        unknown["approval"] = json!(true);
        assert!(ReviewOutput::parse(&unknown.to_string(), revision, &artifact).is_err());
        let mut accepted = output;
        accepted["verdict"] = json!("accepted");
        accepted["findings"] = json!([]);
        assert!(ReviewOutput::parse(&accepted.to_string(), revision, &artifact).is_ok());
    }
    #[test]
    fn correction_profile_rejects_staged_source_instead_of_losing_index_snapshot() {
        let mut artifact = artifact();
        assert!(crate::review::supported_correction_source(&artifact));
        artifact.files[0].index =
            Some(FileContent::new("staged intermediate\n".into(), "100644".into()).unwrap());
        artifact.revision = artifact.content_revision();
        assert!(artifact.validate());
        assert!(!crate::review::supported_correction_source(&artifact));
    }
    #[test]
    fn review_budget_is_bounded_and_never_implicitly_unlimited() {
        assert!(
            ReviewBudget {
                max_corrections: 0,
                max_reviews: 1,
                max_elapsed_ms: 1_000
            }
            .validate()
        );
        for budget in [
            ReviewBudget {
                max_corrections: 4,
                max_reviews: 1,
                max_elapsed_ms: 1_000,
            },
            ReviewBudget {
                max_corrections: 1,
                max_reviews: 0,
                max_elapsed_ms: 1_000,
            },
            ReviewBudget {
                max_corrections: 1,
                max_reviews: 5,
                max_elapsed_ms: 1_000,
            },
            ReviewBudget {
                max_corrections: 1,
                max_reviews: 1,
                max_elapsed_ms: 0,
            },
            ReviewBudget {
                max_corrections: 1,
                max_reviews: 1,
                max_elapsed_ms: 3_600_001,
            },
        ] {
            assert!(!budget.validate());
        }
    }
}
