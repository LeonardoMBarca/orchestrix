//! In-memory product identities and guarded workflows. No external effects.
mod artifact;
mod attempt;
mod ids;
mod profiles;
mod run;
mod task;
mod workspace;

pub use artifact::*;
pub use attempt::*;
pub use ids::*;
pub use profiles::*;
pub use run::*;
pub use task::*;
pub use workspace::*;

use crate::runtime_contract::ContractError;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    InvalidReference,
    EmptyContent,
    InvalidVersion,
    DuplicateIdentity,
    UnknownEntity,
    Ownership,
    InvalidTransition,
    DependencyPending,
    MissingEvidence,
    StaleEvidence,
    VerificationFailed,
    ReviewRejected,
    Conflict,
    Policy,
    Runtime(ContractError),
}
impl fmt::Display for DomainError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidReference => "invalid-reference",
            Self::EmptyContent => "empty-content",
            Self::InvalidVersion => "invalid-version",
            Self::DuplicateIdentity => "duplicate-identity",
            Self::UnknownEntity => "unknown-entity",
            Self::Ownership => "ownership",
            Self::InvalidTransition => "invalid-transition",
            Self::DependencyPending => "dependency-pending",
            Self::MissingEvidence => "missing-evidence",
            Self::StaleEvidence => "stale-evidence",
            Self::VerificationFailed => "verification-failed",
            Self::ReviewRejected => "review-rejected",
            Self::Conflict => "conflict",
            Self::Policy => "policy",
            Self::Runtime(e) => e.code(),
        };
        write!(out, "Domain operation rejected: {code}")
    }
}
impl std::error::Error for DomainError {}
impl From<ContractError> for DomainError {
    fn from(value: ContractError) -> Self {
        Self::Runtime(value)
    }
}
pub type DomainResult<T> = Result<T, DomainError>;
pub(crate) fn require(value: bool, error: DomainError) -> DomainResult<()> {
    if value { Ok(()) } else { Err(error) }
}
