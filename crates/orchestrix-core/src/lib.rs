//! Portable domain/policy with separate persistence, Git, runtime and manual-worker infrastructure.
#![forbid(unsafe_code)]

pub mod daemon;
pub mod daemon_startup;
pub mod desktop;
pub mod domain;
pub mod evaluator;
pub mod git;
pub mod git_records;
pub mod integration;
pub mod integration_records;
pub mod mechanical;
pub mod persistence;
pub mod persistence_privacy;
pub mod persistence_records;
pub mod policy;
pub mod recovery;
pub mod recovery_guardian;
pub mod review;
pub mod review_records;
pub mod runtime;
pub mod runtime_contract;
pub mod worker;
pub mod worker_records;
