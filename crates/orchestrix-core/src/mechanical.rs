//! Explicitly authorized deterministic reads. No model, provider protocol or discovered script is invoked.
use crate::{
    domain::LocalRef,
    git::{self, GitExecutor},
    persistence::Store,
    persistence_privacy::sanitize_text,
    worker::{ManualWorker, WorkerError},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", deny_unknown_fields)]
pub enum QueryKind {
    RepositoryPreflight {
        repository: PathBuf,
        base_commit: Option<String>,
    },
    GitStatus {
        repository: PathBuf,
    },
    GitDiff {
        repository: PathBuf,
    },
    FileExists {
        authorized_directory: PathBuf,
        path: String,
    },
    CheckResults {
        worker_id: String,
    },
    ProcessStatus {
        worker_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub authorization_ref: String,
    pub authorized: bool,
    pub query: QueryKind,
}
pub fn execute(store: Option<&mut Store>, input: &Query) -> Result<Value, WorkerError> {
    if !input.authorized || LocalRef::new(&input.authorization_ref).is_err() {
        return Err(WorkerError("query-not-authorized"));
    }
    match &input.query {
        QueryKind::RepositoryPreflight {
            repository,
            base_commit,
        } => {
            let report = git::inspect_repository(
                repository,
                base_commit.as_deref(),
                &crate::runtime::CancellationToken::new(),
            )
            .map_err(|error| {
                WorkerError(match error {
                    git::GitError::Deadline => "repository-inspection-deadline",
                    git::GitError::Limit => "repository-inspection-limit",
                    git::GitError::OutputLimit => "repository-inspection-output-limit",
                    git::GitError::Unsupported => "repository-inspection-unsupported",
                    git::GitError::InconsistentCapture => "repository-inspection-inconsistent",
                    git::GitError::Process => "repository-inspection-process",
                    git::GitError::Base => "repository-inspection-base",
                    git::GitError::Path => "repository-inspection-path",
                    git::GitError::Pending => "repository-inspection-cancelled",
                    _ => "repository-inspection-rejected",
                })
            })?;
            serde_json::to_value(report).map_err(|_| WorkerError("query-output"))
        }
        QueryKind::GitStatus { repository } | QueryKind::GitDiff { repository } => {
            let root = git::paths::canonical_directory(repository)?;
            let executor = GitExecutor::new()?;
            executor.ensure_supported(&root)?;
            let args = if matches!(&input.query, QueryKind::GitStatus { .. }) {
                vec!["status", "--porcelain=v1", "--untracked-files=normal"]
            } else {
                vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color"]
            };
            let output = executor
                .run(&root, &args.into_iter().map(Into::into).collect::<Vec<_>>())?
                .checked()?;
            let text = String::from_utf8_lossy(&output);
            let bounded = text.chars().take(8192).collect::<String>();
            Ok(
                json!({"route":"deterministic","authorizationRef":input.authorization_ref,"output":sanitize_text(&bounded).map_err(|_|WorkerError("query-output"))?,"truncated":bounded.len()!=text.len(),"modelCalled":false}),
            )
        }
        QueryKind::FileExists {
            authorized_directory,
            path,
        } => {
            if !crate::git_records::relative_file_path(path) {
                return Err(WorkerError("query-file-path"));
            }
            let root = git::paths::canonical_directory(authorized_directory)?;
            let candidate = root.join(path);
            git::paths::reject_links(&candidate)?;
            let exists = if candidate.exists() {
                git::paths::authorized_child(&root, &candidate, true)?.is_file()
            } else {
                false
            };
            Ok(json!({"route":"deterministic","exists":exists,"modelCalled":false}))
        }
        QueryKind::CheckResults { worker_id } | QueryKind::ProcessStatus { worker_id } => {
            let record =
                ManualWorker::inspect(store.ok_or(WorkerError("query-database"))?, worker_id)?;
            Ok(
                json!({"route":"deterministic","result":record.summary(),"processObservation":"historical-not-current-liveness","modelCalled":false}),
            )
        }
    }
}
