//! Scope-bound recovery observations, independent of root exit/stdio/remote outcome.
use crate::{ProcessError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobObservation {
    /// The named kernel object no longer exists (or was never created).
    Absent,
    Empty,
    Active(u32),
}
#[derive(Clone)]
pub struct JobScope {
    #[cfg(windows)]
    native: std::sync::Arc<native::Scope>,
}
impl std::fmt::Debug for JobScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JobScope(owned)")
    }
}
impl JobScope {
    pub fn create(scope: &str) -> Result<Self> {
        if !valid_scope(scope) {
            return Err(ProcessError::Arguments);
        }
        #[cfg(windows)]
        {
            Ok(Self {
                native: std::sync::Arc::new(native::Scope::create(scope)?),
            })
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
    /// Only the trusted keeper consumes the pre-created scope. Create never
    /// adopts a colliding name; this bounded open validates its owner/security.
    pub fn keeper_open(scope: &str) -> Result<Self> {
        if !valid_scope(scope) {
            return Err(ProcessError::Arguments);
        }
        #[cfg(windows)]
        {
            Ok(Self {
                native: std::sync::Arc::new(native::Scope::open(scope)?),
            })
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
    pub fn active(&self) -> Result<u32> {
        #[cfg(windows)]
        {
            self.native.active()
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
    pub fn terminate(&self) -> Result<()> {
        #[cfg(windows)]
        {
            self.native.terminate()
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
    pub fn seal(&self) -> Result<()> {
        #[cfg(windows)]
        {
            self.native.seal()
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
    #[cfg(windows)]
    pub(crate) fn duplicate(&self) -> Result<*mut std::ffi::c_void> {
        self.native.duplicate()
    }
}
pub struct HostWatch {
    #[cfg(windows)]
    native: native::HostWatch,
}
impl HostWatch {
    pub fn open(pid: u32, created: u64) -> Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                native: native::HostWatch::open(pid, created)?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (pid, created);
            Err(ProcessError::Unsupported)
        }
    }
    pub fn exited(&self) -> Result<bool> {
        #[cfg(windows)]
        {
            self.native.exited()
        }
        #[cfg(not(windows))]
        {
            Err(ProcessError::Unsupported)
        }
    }
}
pub fn host_identity() -> Result<(u32, u64)> {
    #[cfg(windows)]
    {
        native::host_identity()
    }
    #[cfg(not(windows))]
    {
        Err(ProcessError::Unsupported)
    }
}
pub fn private_directory(path: &std::path::Path) -> Result<()> {
    #[cfg(windows)]
    {
        native::private_directory(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(ProcessError::Unsupported)
    }
}
#[cfg(windows)]
#[path = "named_job_windows.rs"]
mod native;

pub(crate) fn valid_scope(scope: &str) -> bool {
    scope.len() == 64
        && scope
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
/// Host-generated unpredictable ownership material, not a provider credential.
pub fn nonce() -> Result<String> {
    #[cfg(windows)]
    {
        native::nonce()
    }
    #[cfg(not(windows))]
    {
        Err(ProcessError::Unsupported)
    }
}
pub fn inspect(scope: &str) -> Result<JobObservation> {
    if !valid_scope(scope) {
        return Err(ProcessError::Arguments);
    }
    #[cfg(windows)]
    {
        native::inspect(scope)
    }
    #[cfg(not(windows))]
    {
        Err(ProcessError::Unsupported)
    }
}
