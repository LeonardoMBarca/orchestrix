//! Bounded local Git worktrees and exact artifact capture; no agent execution.
mod capture;
mod executor;
pub(crate) mod integration;
pub(crate) mod paths;
mod preflight;
mod worktrees;

use crate::persistence::StoreError;
pub use capture::*;
pub use executor::GitExecutor;
pub use preflight::{RepositoryPreflight, inspect_repository};
use std::fmt;
pub use worktrees::{CreateWorktree, GitFaultPoint, WorktreeService};
const SOURCE_FILES: usize = 4096;
const SOURCE_BYTES: usize = 64 * crate::git_records::MAX_FILE_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitError {
    Io,
    Path,
    Scope,
    Link,
    Collision,
    Unsupported,
    NotRepository,
    Base,
    Ownership,
    MissingAttempt,
    Conflict,
    Process,
    Deadline,
    OutputLimit,
    Limit,
    InconsistentCapture,
    SecretContent,
    Pending,
    UnpreservedWork,
    Injected,
    Storage(StoreError),
}
impl fmt::Display for GitError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::Io => "io",
            Self::Path => "path",
            Self::Scope => "scope",
            Self::Link => "link",
            Self::Collision => "collision",
            Self::Unsupported => "unsupported",
            Self::NotRepository => "not-repository",
            Self::Base => "base",
            Self::Ownership => "ownership",
            Self::MissingAttempt => "missing-attempt",
            Self::Conflict => "conflict",
            Self::Process => "process",
            Self::Deadline => "deadline",
            Self::OutputLimit => "output-limit",
            Self::Limit => "limit",
            Self::InconsistentCapture => "inconsistent-capture",
            Self::SecretContent => "secret-content",
            Self::Pending => "pending",
            Self::UnpreservedWork => "unpreserved-work",
            Self::Injected => "injected",
            Self::Storage(_) => "storage",
        };
        write!(out, "Local Git operation rejected: {code}")
    }
}
impl std::error::Error for GitError {}
impl From<std::io::Error> for GitError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<StoreError> for GitError {
    fn from(value: StoreError) -> Self {
        Self::Storage(value)
    }
}
pub type GitResult<T> = Result<T, GitError>;
