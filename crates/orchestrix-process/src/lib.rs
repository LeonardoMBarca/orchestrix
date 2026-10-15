//! Bounded owned-process transport. Windows Job ownership is independent of protocol success.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod ipc;
pub mod lease;
pub mod named_job;
pub mod watch;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
use unsupported as native;
#[cfg(windows)]
use windows as native;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessError {
    Arguments,
    Unsupported,
    Launch,
    Ownership,
    Io,
    Deadline,
    Budget,
    Closed,
}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Owned process failed: {self:?}")
    }
}
impl std::error::Error for ProcessError {}
pub type Result<T> = std::result::Result<T, ProcessError>;

#[derive(Clone, Debug)]
pub struct SpawnSpec {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub cwd: PathBuf,
    /// Explicit environment; inherited credentials and routing variables are never added.
    pub environment: Vec<(OsString, OsString)>,
    pub limits: ProcessLimits,
}
#[derive(Clone, Copy, Debug)]
pub struct ProcessLimits {
    pub lifetime: Duration,
    pub max_output_bytes: u64,
    pub max_write_bytes: usize,
}
impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            lifetime: Duration::from_secs(300),
            max_output_bytes: 8 * 1024 * 1024,
            max_write_bytes: 65_536,
        }
    }
}
impl ProcessLimits {
    fn validate(self) -> Result<Self> {
        if self.lifetime.is_zero()
            || self.lifetime > Duration::from_secs(300)
            || self.max_output_bytes == 0
            || self.max_output_bytes > 16 * 1024 * 1024
            || self.max_write_bytes == 0
            || self.max_write_bytes > 1_048_576
        {
            return Err(ProcessError::Arguments);
        }
        Ok(self)
    }
}
#[derive(Clone, Debug)]
pub enum ProcessEvent {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamState {
    pub stdin_closed: bool,
    pub stdout_closed: bool,
    pub stderr_closed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeState {
    pub owned: bool,
    pub root_exit_code: Option<u32>,
    pub active_processes: u32,
}
struct PumpState {
    input: AtomicBool,
    output: AtomicBool,
    error: AtomicBool,
    fault: Mutex<Option<ProcessError>>,
    bytes: AtomicU64,
}
impl PumpState {
    fn fail(&self, error: ProcessError) {
        if let Ok(mut fault) = self.fault.lock() {
            fault.get_or_insert(error);
        }
    }
}
enum Input {
    Write(Vec<u8>, mpsc::SyncSender<Result<()>>),
    Close,
}

pub struct OwnedProcess {
    native: Arc<native::NativeProcess>,
    events: mpsc::Receiver<ProcessEvent>,
    input: Option<mpsc::SyncSender<Input>>,
    state: Arc<PumpState>,
    limits: ProcessLimits,
    stop_watchdog: Arc<AtomicBool>,
    watchdog: Option<thread::JoinHandle<()>>,
}
impl OwnedProcess {
    pub fn member_ids(&self) -> Result<Vec<u32>> {
        self.native.member_ids()
    }
    pub fn spawn(spec: SpawnSpec) -> Result<Self> {
        Self::spawn_inner(spec, None)
    }
    pub fn spawn_identified(spec: SpawnSpec, scope: &named_job::JobScope) -> Result<Self> {
        Self::spawn_inner(spec, Some(scope))
    }
    fn spawn_inner(spec: SpawnSpec, scope: Option<&named_job::JobScope>) -> Result<Self> {
        let limits = spec.limits.validate()?;
        if !spec.executable.is_absolute()
            || !spec.executable.is_file()
            || !spec.cwd.is_absolute()
            || !spec.cwd.is_dir()
            || spec.arguments.len() > 256
            || spec.environment.len() > 64
        {
            return Err(ProcessError::Arguments);
        }
        let launched = native::NativeProcess::spawn(&spec, scope)?;
        let native = Arc::new(launched.process);
        let state = Arc::new(PumpState {
            input: AtomicBool::new(false),
            output: AtomicBool::new(false),
            error: AtomicBool::new(false),
            fault: Mutex::new(None),
            bytes: AtomicU64::new(0),
        });
        let (events_tx, events) = mpsc::sync_channel(128);
        pump(
            launched.output,
            events_tx.clone(),
            Arc::clone(&state),
            true,
            limits.max_output_bytes,
        );
        pump(
            launched.error,
            events_tx,
            Arc::clone(&state),
            false,
            limits.max_output_bytes,
        );
        let (input_tx, input_rx) = mpsc::sync_channel(8);
        let input_state = Arc::clone(&state);
        thread::spawn(move || {
            let mut pipe = launched.input;
            while let Ok(Input::Write(bytes, ack)) = input_rx.recv() {
                let result = pipe
                    .write_all(&bytes)
                    .and_then(|()| pipe.flush())
                    .map_err(|_| ProcessError::Io);
                if result.is_err() {
                    input_state.fail(ProcessError::Io);
                }
                let failed = result.is_err();
                let _ = ack.try_send(result);
                if failed {
                    break;
                }
            }
            drop(pipe);
            input_state.input.store(true, Ordering::Release);
        });
        let stop_watchdog = Arc::new(AtomicBool::new(false));
        let watchdog_stop = Arc::clone(&stop_watchdog);
        let watchdog_process = Arc::clone(&native);
        let watchdog_state = Arc::clone(&state);
        let watchdog = thread::spawn(move || {
            let begin = Instant::now();
            while !watchdog_stop.load(Ordering::Acquire) {
                if begin.elapsed() >= limits.lifetime {
                    watchdog_state.fail(ProcessError::Deadline);
                    let _ = watchdog_process.terminate();
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
        });
        Ok(Self {
            native,
            events,
            input: Some(input_tx),
            state,
            limits,
            stop_watchdog,
            watchdog: Some(watchdog),
        })
    }
    pub fn send(&self, bytes: &[u8], timeout: Duration) -> Result<()> {
        if bytes.is_empty()
            || bytes.len() > self.limits.max_write_bytes
            || timeout.is_zero()
            || timeout > Duration::from_secs(30)
        {
            return Err(ProcessError::Arguments);
        }
        self.health()?;
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        self.input
            .as_ref()
            .ok_or(ProcessError::Closed)?
            .try_send(Input::Write(bytes.to_vec(), ack_tx))
            .map_err(|_| ProcessError::Budget)?;
        ack_rx.recv_timeout(timeout).map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => ProcessError::Deadline,
            mpsc::RecvTimeoutError::Disconnected => ProcessError::Closed,
        })?
    }
    pub fn receive(&self, timeout: Duration) -> Result<Option<ProcessEvent>> {
        if timeout > Duration::from_secs(1) {
            return Err(ProcessError::Arguments);
        }
        self.health()?;
        match self.events.recv_timeout(timeout) {
            Ok(value) => Ok(Some(value)),
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }
    pub fn health(&self) -> Result<()> {
        self.state
            .fault
            .lock()
            .map_err(|_| ProcessError::Io)?
            .map_or(Ok(()), Err)
    }
    pub fn streams(&self) -> StreamState {
        StreamState {
            stdin_closed: self.state.input.load(Ordering::Acquire),
            stdout_closed: self.state.output.load(Ordering::Acquire),
            stderr_closed: self.state.error.load(Ordering::Acquire),
        }
    }
    pub fn tree(&self) -> Result<TreeState> {
        self.native.observe()
    }
    pub fn close_input(&mut self) {
        if let Some(input) = self.input.take() {
            let _ = input.try_send(Input::Close);
        }
    }
    /// Terminates only the owned Job, then independently observes root exit and active count.
    pub fn terminate(&mut self, timeout: Duration) -> Result<TreeState> {
        if timeout.is_zero() || timeout > Duration::from_secs(3) {
            return Err(ProcessError::Arguments);
        }
        self.close_input();
        self.native.terminate()?;
        let began = Instant::now();
        loop {
            let state = self.native.observe()?;
            if state.root_exit_code.is_some() && state.active_processes == 0 {
                return Ok(state);
            }
            if began.elapsed() >= timeout {
                return Err(ProcessError::Deadline);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn wait_streams(&self, timeout: Duration) -> StreamState {
        let began = Instant::now();
        let timeout = timeout.min(Duration::from_secs(3));
        loop {
            let streams = self.streams();
            if (streams.stdin_closed && streams.stdout_closed && streams.stderr_closed)
                || began.elapsed() >= timeout
            {
                return streams;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        self.close_input();
        let _ = self.native.terminate();
        self.stop_watchdog.store(true, Ordering::Release);
        if let Some(watchdog) = self.watchdog.take() {
            let _ = watchdog.join();
        }
    }
}
fn pump(
    mut pipe: impl Read + Send + 'static,
    tx: mpsc::SyncSender<ProcessEvent>,
    state: Arc<PumpState>,
    output: bool,
    maximum: u64,
) {
    thread::spawn(move || {
        let mut bytes = [0; 8192];
        loop {
            match pipe.read(&mut bytes) {
                Ok(0) => break,
                Ok(size) => {
                    let prior = state.bytes.fetch_add(size as u64, Ordering::AcqRel);
                    if prior.saturating_add(size as u64) > maximum {
                        state.fail(ProcessError::Budget);
                        break;
                    }
                    let event = if output {
                        ProcessEvent::Stdout(bytes[..size].to_vec())
                    } else {
                        ProcessEvent::Stderr(bytes[..size].to_vec())
                    };
                    if tx.try_send(event).is_err() {
                        state.fail(ProcessError::Budget);
                        break;
                    }
                }
                Err(_) => {
                    state.fail(ProcessError::Io);
                    break;
                }
            }
        }
        drop(pipe);
        if output {
            state.output.store(true, Ordering::Release);
        } else {
            state.error.store(true, Ordering::Release);
        }
    });
}
