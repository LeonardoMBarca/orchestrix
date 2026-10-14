//! First production runtime boundary. Provider protocol and OS ownership stay outside the domain.
mod codex;
mod metadata;
mod metadata_schedule;
pub mod refresh;
mod rpc;
pub use codex::{
    AttemptExecution, CodexAdapter, RuntimeLaunch, RuntimeMetadata, RuntimeReport,
    SUPPORTED_CODEX_VERSIONS, SessionBinding, native_effort, neutral_effort,
};
pub use metadata::{
    CodexMetadataService, ManualRefreshClock, MetadataRead, RefreshClock, SystemRefreshClock,
};
pub use metadata_schedule::{
    MetadataRefreshCoordinator, MetadataRefreshTrigger, MetadataRegistration,
    MetadataScheduleBounds, ScheduledMetadataOutcome, ScheduledMetadataUpdate,
};

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    code: &'static str,
    dispatch: &'static str,
    adapter_diagnostic: Option<serde_json::Value>,
}
impl RuntimeError {
    /// The host failed to durably record an observation. No raw database error crosses this boundary.
    pub fn observation_failed() -> Self {
        Self::new("transport-loss", "unknown")
    }
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub fn dispatch(&self) -> &'static str {
        self.dispatch
    }
    pub(crate) fn new(code: &'static str, dispatch: &'static str) -> Self {
        Self {
            code,
            dispatch,
            adapter_diagnostic: None,
        }
    }
    fn with_adapter_diagnostic(mut self, diagnostic: serde_json::Value) -> Self {
        self.adapter_diagnostic = Some(diagnostic);
        self
    }
}
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Runtime operation rejected: {} ({})",
            self.code, self.dispatch
        )
    }
}
impl std::error::Error for RuntimeError {}
impl From<crate::runtime_contract::ContractError> for RuntimeError {
    fn from(error: crate::runtime_contract::ContractError) -> Self {
        Self::new(error.code(), "not-sent")
    }
}
impl From<orchestrix_process::ProcessError> for RuntimeError {
    fn from(error: orchestrix_process::ProcessError) -> Self {
        use orchestrix_process::ProcessError as E;
        Self::new(
            match error {
                E::Deadline => "timeout",
                E::Budget => "budget",
                E::Ownership => "ownership",
                E::Unsupported => "unsupported",
                E::Arguments => "policy",
                _ => "transport-loss",
            },
            "unknown",
        )
    }
}
pub type RuntimeResult<T> = std::result::Result<T, RuntimeError>;
