//! User-restricted, bounded local request/reply transport. No background I/O threads.
//! Windows security and overlapped ownership stay outside the portable Core.
use std::time::Duration;

pub const MAX_FRAME: usize = 1_048_576;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    Identity,
    Busy,
    Unavailable,
    Deadline,
    Closed,
    Budget,
    Io,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Local IPC: {self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(windows)]
#[path = "ipc_windows.rs"]
mod native;
#[cfg(windows)]
pub use native::{Listener, Stream};

#[cfg(not(windows))]
pub struct Listener;
#[cfg(not(windows))]
pub struct Stream;
#[cfg(not(windows))]
impl Listener {
    pub fn bind(_: &str) -> Result<Self> {
        Err(Error::Unsupported)
    }
    pub fn accept(&mut self, _: Duration) -> Result<Option<Stream>> {
        Err(Error::Unsupported)
    }
}
#[cfg(not(windows))]
impl Stream {
    pub fn connect(_: &str, _: Duration) -> Result<Self> {
        Err(Error::Unsupported)
    }
    pub fn read(&mut self, _: Duration) -> Result<Vec<u8>> {
        Err(Error::Unsupported)
    }
    pub fn write(&mut self, _: &[u8], _: Duration) -> Result<()> {
        Err(Error::Unsupported)
    }
}

/// SID is included in the endpoint's derivation, never accepted from a client.
pub fn current_user_scope() -> Result<String> {
    #[cfg(windows)]
    {
        native::current_user_scope()
    }
    #[cfg(not(windows))]
    {
        Err(Error::Unsupported)
    }
}

fn checked_deadline(timeout: Duration) -> Result<std::time::Instant> {
    if timeout.is_zero() || timeout > Duration::from_secs(10) {
        return Err(Error::Deadline);
    }
    Ok(std::time::Instant::now() + timeout)
}
