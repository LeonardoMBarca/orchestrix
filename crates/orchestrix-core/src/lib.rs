//! Portable domain and explicit local persistence boundary; no provider execution.
#![forbid(unsafe_code)]

pub mod domain;
pub mod git;
pub mod git_records;
pub mod persistence;
pub mod persistence_privacy;
pub mod persistence_records;
pub mod policy;
pub mod runtime_contract;
