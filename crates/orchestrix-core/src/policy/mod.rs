//! Versioned deterministic policy and explicitly selected context; no I/O or providers.
mod context;
mod resolve;
mod schema;

pub use context::*;
pub use resolve::*;
pub use schema::*;

use crate::persistence_privacy::sanitize_text;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyError {
    Schema,
    Version,
    Limit,
    Conflict,
    UnsafeContent,
    Ownership,
}
impl fmt::Display for PolicyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::Schema => "schema",
            Self::Version => "version",
            Self::Limit => "limit",
            Self::Conflict => "conflict",
            Self::UnsafeContent => "unsafe-content",
            Self::Ownership => "ownership",
        };
        write!(out, "Policy rejected: {code}")
    }
}
impl std::error::Error for PolicyError {}
pub type PolicyResult<T> = Result<T, PolicyError>;

pub(crate) fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
pub(crate) fn safe_text(value: &str, max: usize) -> PolicyResult<()> {
    if value.len() > max {
        return Err(PolicyError::Limit);
    }
    if value.contains('\0') || sanitize_text(value).map_err(|_| PolicyError::Limit)? != value {
        return Err(PolicyError::UnsafeContent);
    }
    Ok(())
}
pub(crate) fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
