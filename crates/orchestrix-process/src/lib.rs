//! Bounded owned-process transport. Windows Job ownership is independent of protocol success.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod ipc;
pub mod lease;
pub mod named_job;
pub mod startup_pipe;
pub mod watch;
#[cfg(windows)]
pub mod windows_file_read;

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
    stop_output: AtomicBool,
    #[cfg(test)]
    queue_full: AtomicBool,
    #[cfg(test)]
    queue_full_count: AtomicU64,
    #[cfg(test)]
    queue_full_wait_micros: AtomicU64,
}
impl PumpState {
    fn fail(&self, error: ProcessError) {
        if let Ok(mut fault) = self.fault.lock() {
            fault.get_or_insert(error);
        }
    }
    fn health(&self) -> Result<()> {
        self.fault
            .lock()
            .map_err(|_| ProcessError::Io)?
            .map_or(Ok(()), Err)
    }
}
enum Input {
    Write(Vec<u8>, mpsc::SyncSender<Result<()>>),
    Close,
}

#[derive(Clone, Copy)]
enum OutputDelivery {
    FailFast,
    Backpressured,
}

/// Receipt for one bounded, already queued stdin write. Polling never resends
/// bytes and never waits for the child to consume input. Callers must continue
/// draining output and observing the owned process while a write is pending.
pub struct PendingWrite {
    acknowledgement: mpsc::Receiver<Result<()>>,
    state: Arc<PumpState>,
    completion: Option<Result<()>>,
}
impl PendingWrite {
    pub fn try_complete(&mut self) -> Result<Option<()>> {
        if let Some(result) = self.completion {
            return result.map(Some);
        }
        let result = match self.acknowledgement.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                self.state.health()?;
                return Ok(None);
            }
            Err(mpsc::TryRecvError::Disconnected) => Err(ProcessError::Closed),
        };
        self.completion = Some(result);
        result.map(Some)
    }
    fn wait(self, timeout: Duration) -> Result<()> {
        self.acknowledgement
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => ProcessError::Deadline,
                mpsc::RecvTimeoutError::Disconnected => ProcessError::Closed,
            })?
    }
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
        Self::spawn_inner(spec, None, OutputDelivery::FailFast)
    }
    pub fn spawn_identified(spec: SpawnSpec, scope: &named_job::JobScope) -> Result<Self> {
        Self::spawn_inner(spec, Some(scope), OutputDelivery::FailFast)
    }
    /// Bulk local output may wait for this consumer without turning the fixed
    /// event queue into an additional byte budget. Readers retain at most one
    /// pending chunk each; ownership, byte and lifetime limits are unchanged.
    /// Consumers must drain output and observe stream closure independently of
    /// process exit. Explicit termination cancels pending reader delivery.
    pub fn spawn_identified_backpressured(
        spec: SpawnSpec,
        scope: &named_job::JobScope,
    ) -> Result<Self> {
        Self::spawn_inner(spec, Some(scope), OutputDelivery::Backpressured)
    }
    fn spawn_inner(
        spec: SpawnSpec,
        scope: Option<&named_job::JobScope>,
        delivery: OutputDelivery,
    ) -> Result<Self> {
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
            stop_output: AtomicBool::new(false),
            #[cfg(test)]
            queue_full: AtomicBool::new(false),
            #[cfg(test)]
            queue_full_count: AtomicU64::new(0),
            #[cfg(test)]
            queue_full_wait_micros: AtomicU64::new(0),
        });
        let (events_tx, events) = mpsc::sync_channel(128);
        pump(
            launched.output,
            events_tx.clone(),
            Arc::clone(&state),
            true,
            limits.max_output_bytes,
            delivery,
        );
        pump(
            launched.error,
            events_tx,
            Arc::clone(&state),
            false,
            limits.max_output_bytes,
            delivery,
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
                    watchdog_state.stop_output.store(true, Ordering::Release);
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
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(ProcessError::Arguments);
        }
        self.begin_send(bytes)?.wait(timeout)
    }
    /// Queue one write without waiting for pipe capacity. This uses the same
    /// eight-entry input queue and per-write budget as `send`; a full queue is
    /// still rejected. Dropping the receipt does not cancel or repeat a write.
    pub fn begin_send(&self, bytes: &[u8]) -> Result<PendingWrite> {
        if bytes.is_empty() || bytes.len() > self.limits.max_write_bytes {
            return Err(ProcessError::Arguments);
        }
        self.health()?;
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        self.input
            .as_ref()
            .ok_or(ProcessError::Closed)?
            .try_send(Input::Write(bytes.to_vec(), ack_tx))
            .map_err(|_| ProcessError::Budget)?;
        Ok(PendingWrite {
            acknowledgement: ack_rx,
            state: Arc::clone(&self.state),
            completion: None,
        })
    }
    pub fn receive(&self, timeout: Duration) -> Result<Option<ProcessEvent>> {
        if timeout > Duration::from_secs(1) {
            return Err(ProcessError::Arguments);
        }
        let started = Instant::now();
        self.health()?;
        match self.events.recv_timeout(timeout) {
            Ok(value) => Ok(Some(value)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // EOF does not imply the owned process tree has exited. A
                // disconnected receiver returns immediately, so honor this
                // call's remaining wait instead of spinning until OS closure.
                let remaining = timeout.saturating_sub(started.elapsed());
                if !remaining.is_zero() {
                    thread::sleep(remaining);
                }
                self.health()?;
                Ok(None)
            }
        }
    }
    pub fn health(&self) -> Result<()> {
        self.state.health()
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
        self.state.stop_output.store(true, Ordering::Release);
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
        self.state.stop_output.store(true, Ordering::Release);
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
    delivery: OutputDelivery,
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
                    if !deliver(&tx, &state, delivery, event) {
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

fn deliver(
    tx: &mpsc::SyncSender<ProcessEvent>,
    state: &PumpState,
    delivery: OutputDelivery,
    mut event: ProcessEvent,
) -> bool {
    loop {
        if matches!(delivery, OutputDelivery::Backpressured)
            && state.stop_output.load(Ordering::Acquire)
        {
            return false;
        }
        match tx.try_send(event) {
            Ok(()) => return true,
            Err(mpsc::TrySendError::Full(pending)) => {
                #[cfg(test)]
                {
                    state.queue_full.store(true, Ordering::Release);
                    state.queue_full_count.fetch_add(1, Ordering::Relaxed);
                }
                if matches!(delivery, OutputDelivery::FailFast) {
                    state.fail(ProcessError::Budget);
                    return false;
                }
                event = pending;
                // Only the output reader waits here. The independent watchdog
                // and all control operations remain runnable; cancellation is
                // checked again without waiting for the consumer to drain.
                #[cfg(test)]
                let wait_started = Instant::now();
                thread::sleep(Duration::from_millis(1));
                #[cfg(test)]
                state
                    .queue_full_wait_micros
                    .fetch_add(wait_started.elapsed().as_micros() as u64, Ordering::Relaxed);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                state.fail(if matches!(delivery, OutputDelivery::FailFast) {
                    ProcessError::Budget
                } else {
                    ProcessError::Closed
                });
                return false;
            }
        }
    }
}

#[cfg(all(test, windows))]
mod bulk_output_tests {
    use super::*;
    use std::ffi::c_void;
    use std::process::{Command, Stdio};

    const CHILD: &str = "ORCHESTRIX_PROCESS_OUTPUT_CHILD";
    const CHILD_TEST: &str = "bulk_output_tests::owned_output_child";
    const PAYLOAD: &[u8] = b"ORCHESTRIX_BULK_PAYLOAD\n";
    const INPUT_PAYLOAD: &[u8] = b"ORCHESTRIX_BULK_INPUT\n";
    const MIB: usize = 1_048_576;
    const INPUT_BYTES: usize = 65_536;

    #[derive(Default)]
    #[repr(C)]
    struct ThreadFileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetThreadTimes(
            thread: *mut c_void,
            creation: *mut ThreadFileTime,
            exit: *mut ThreadFileTime,
            kernel: *mut ThreadFileTime,
            user: *mut ThreadFileTime,
        ) -> i32;
        fn GetStdHandle(handle: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    fn thread_cpu_100ns() -> u64 {
        let mut creation = ThreadFileTime::default();
        let mut exit = ThreadFileTime::default();
        let mut kernel = ThreadFileTime::default();
        let mut user = ThreadFileTime::default();
        // SAFETY: the pseudo-handle belongs to this thread and every output
        // pointer names a live, correctly sized FILETIME value.
        unsafe {
            assert_ne!(
                GetThreadTimes(
                    GetCurrentThread(),
                    &mut creation,
                    &mut exit,
                    &mut kernel,
                    &mut user,
                ),
                0
            );
        }
        let value = |time: ThreadFileTime| (u64::from(time.high) << 32) | u64::from(time.low);
        value(kernel) + value(user)
    }

    fn close_child_output_and_wait_for_input() -> ! {
        std::io::stdout().flush().unwrap();
        std::io::stderr().flush().unwrap();
        // SAFETY: this private fixture owns its two distinct inherited output
        // handles. It performs no later writes and exits directly after the
        // parent releases stdin, avoiding test-harness writes to closed handles.
        unsafe {
            let output = GetStdHandle(-11_i32 as u32);
            let error = GetStdHandle(-12_i32 as u32);
            assert!(!output.is_null() && output != -1_isize as *mut c_void);
            assert!(!error.is_null() && error != -1_isize as *mut c_void);
            assert_ne!(output, error);
            assert_ne!(CloseHandle(output), 0);
            assert_ne!(CloseHandle(error), 0);
        }
        let mut release = [0u8];
        std::io::stdin().read_exact(&mut release).unwrap();
        assert_eq!(release, [1]);
        std::process::exit(0);
    }

    fn wait_for_disconnected_output(process: &OwnedProcess) -> Vec<ProcessEvent> {
        let mut startup = Vec::new();
        wait_for(|| {
            loop {
                match process.events.try_recv() {
                    Ok(event) => startup.push(event),
                    Err(mpsc::TryRecvError::Empty) => return false,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
            let streams = process.streams();
            streams.stdout_closed && streams.stderr_closed
        });
        startup
    }

    fn duplex_input() -> Vec<u8> {
        (0..INPUT_BYTES).map(|index| (index % 251) as u8).collect()
    }

    fn spec(scenario: &str, bytes: usize, lifetime: Duration, maximum: u64) -> SpawnSpec {
        let mut environment = [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "SystemDrive",
            "TEMP",
            "TMP",
            "COMSPEC",
            "PATHEXT",
        ]
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (OsString::from(key), value)))
        .collect::<Vec<_>>();
        environment.push((CHILD.into(), scenario.into()));
        environment.push((
            "ORCHESTRIX_PROCESS_OUTPUT_BYTES".into(),
            bytes.to_string().into(),
        ));
        SpawnSpec {
            executable: std::env::current_exe().unwrap(),
            arguments: [
                "--exact",
                CHILD_TEST,
                "--ignored",
                "--nocapture",
                "--quiet",
                "--test-threads=1",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
            cwd: std::env::current_dir().unwrap().canonicalize().unwrap(),
            environment,
            limits: ProcessLimits {
                lifetime,
                max_output_bytes: maximum,
                max_write_bytes: 65_536,
            },
        }
    }

    fn scope() -> named_job::JobScope {
        named_job::JobScope::create(&named_job::nonce().unwrap()).unwrap()
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "owned fixture observation deadline"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn assert_closed(state: &PumpState) {
        wait_for(|| {
            state.input.load(Ordering::Acquire)
                && state.output.load(Ordering::Acquire)
                && state.error.load(Ordering::Acquire)
        });
    }

    #[test]
    #[ignore = "explicit installed Git entrypoints; owned --version processes only"]
    fn installed_git_entrypoints_have_equivalent_bounded_owned_version_output() {
        let executable = |key| {
            let path = PathBuf::from(std::env::var_os(key).expect("explicit Git executable"));
            assert!(path.is_absolute() && path.is_file());
            assert!(!std::fs::symlink_metadata(&path).unwrap().is_symlink());
            path.canonicalize().unwrap()
        };
        let wrapper = executable("ORCHESTRIX_GIT_ENTRYPOINT_WRAPPER");
        let native = executable("ORCHESTRIX_GIT_ENTRYPOINT_NATIVE");
        assert_ne!(wrapper, native, "comparison requires distinct entrypoints");
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let mut environment = [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "SystemDrive",
            "TEMP",
            "TMP",
            "COMSPEC",
            "PATHEXT",
        ]
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (OsString::from(key), value)))
        .collect::<Vec<_>>();
        for (key, value) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "NUL"),
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
        ] {
            environment.push((key.into(), value.into()));
        }
        let mut expected_output: Option<Vec<u8>> = None;
        for sample in 0..4 {
            // Interleave and alternate the first role rather than running two batches.
            let roles = if sample % 2 == 0 {
                [("wrapper", &wrapper), ("native", &native)]
            } else {
                [("native", &native), ("wrapper", &wrapper)]
            };
            for (role, executable) in roles {
                let scope = scope();
                let total_started = Instant::now();
                let spawn_started = Instant::now();
                let mut process = OwnedProcess::spawn_identified_backpressured(
                    SpawnSpec {
                        executable: executable.clone(),
                        arguments: vec!["--version".into()],
                        cwd: cwd.clone(),
                        environment: environment.clone(),
                        limits: ProcessLimits {
                            lifetime: Duration::from_secs(15),
                            max_output_bytes: 8 * MIB as u64,
                            max_write_bytes: INPUT_BYTES,
                        },
                    },
                    &scope,
                )
                .unwrap();
                let spawn_micros = spawn_started.elapsed().as_micros();
                process.close_input();
                let drain_started = Instant::now();
                let mut receive_micros = 0;
                let mut tree_micros = 0;
                let mut tree_queries = 0;
                let mut output = Vec::new();
                let mut error = Vec::new();
                loop {
                    let receive_started = Instant::now();
                    let event = process.receive(Duration::from_millis(5)).unwrap();
                    receive_micros += receive_started.elapsed().as_micros();
                    if let Some(event) = event {
                        match event {
                            ProcessEvent::Stdout(bytes) => output.extend(bytes),
                            ProcessEvent::Stderr(bytes) => error.extend(bytes),
                        }
                    }
                    let tree_started = Instant::now();
                    let tree = process.tree().unwrap();
                    tree_micros += tree_started.elapsed().as_micros();
                    tree_queries += 1;
                    let streams = process.streams();
                    if tree.root_exit_code.is_some()
                        && tree.active_processes == 0
                        && streams.stdin_closed
                        && streams.stdout_closed
                        && streams.stderr_closed
                    {
                        assert!(tree.owned);
                        assert_eq!(tree.root_exit_code, Some(0));
                        while let Some(event) = process.receive(Duration::ZERO).unwrap() {
                            match event {
                                ProcessEvent::Stdout(bytes) => output.extend(bytes),
                                ProcessEvent::Stderr(bytes) => error.extend(bytes),
                            }
                        }
                        break;
                    }
                    assert!(total_started.elapsed() < Duration::from_secs(15));
                }
                let drain_micros = drain_started.elapsed().as_micros();
                assert!(output.starts_with(b"git version "));
                assert!(
                    error.is_empty(),
                    "--version must not emit diagnostic output"
                );
                if let Some(expected) = &expected_output {
                    assert_eq!(&output, expected, "all eight outputs must match exactly");
                } else {
                    expected_output = Some(output.clone());
                }
                assert_eq!(process.health(), Ok(()));
                let state = Arc::clone(&process.state);
                let close_started = Instant::now();
                let closed_tree = process.terminate(Duration::from_secs(3)).unwrap();
                assert!(closed_tree.owned && closed_tree.active_processes == 0);
                assert_closed(&state);
                let terminate_micros = close_started.elapsed().as_micros();
                let drop_started = Instant::now();
                drop(process);
                let drop_micros = drop_started.elapsed().as_micros();
                assert_eq!(scope.active().unwrap(), 0);
                println!(
                    "ORCHESTRIX_GIT_ENTRYPOINT_TIMING={{\"role\":\"{role}\",\"sample\":{sample},\"spawnMicros\":{spawn_micros},\"drainMicros\":{drain_micros},\"receiveMicros\":{receive_micros},\"treeObservationMicros\":{tree_micros},\"treeQueries\":{tree_queries},\"terminateMicros\":{terminate_micros},\"dropMicros\":{drop_micros},\"totalMicros\":{},\"stdoutBytes\":{},\"stderrBytes\":{},\"queueFullCount\":{},\"queueFullWaitMicros\":{}}}",
                    total_started.elapsed().as_micros(),
                    output.len(),
                    error.len(),
                    state.queue_full_count.load(Ordering::Relaxed),
                    state.queue_full_wait_micros.load(Ordering::Relaxed),
                );
            }
        }
        println!(
            "ORCHESTRIX_GIT_ENTRYPOINT_EQUALITY={{\"samples\":8,\"exactStdoutEqual\":true,\"stderrEmpty\":true}}"
        );
    }

    #[test]
    #[ignore = "private owned process fixture; no shell, network or provider"]
    fn owned_output_child() {
        let scenario = std::env::var(CHILD).expect("owned child scenario");
        if scenario == "closed-output-live" {
            close_child_output_and_wait_for_input();
        }
        if scenario == "linger" {
            loop {
                thread::sleep(Duration::from_millis(50));
            }
        }
        let mut descendant = if scenario == "descendant" {
            use std::os::windows::process::CommandExt;
            Some(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        CHILD_TEST,
                        "--ignored",
                        "--nocapture",
                        "--quiet",
                        "--test-threads=1",
                    ])
                    .env(CHILD, "linger")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .creation_flags(0x0800_0000)
                    .spawn()
                    .unwrap(),
            )
        } else {
            None
        };
        let count = std::env::var("ORCHESTRIX_PROCESS_OUTPUT_BYTES")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut output: Box<dyn Write> = if scenario == "stderr" {
            Box::new(std::io::stderr())
        } else {
            Box::new(std::io::stdout())
        };
        output.write_all(PAYLOAD).unwrap();
        let chunk = [b'z'; 8192];
        let mut sent = 0;
        while sent < count {
            let size = (count - sent).min(chunk.len());
            output.write_all(&chunk[..size]).unwrap();
            sent += size;
        }
        output.flush().unwrap();
        if scenario == "duplex" {
            let mut input = Vec::new();
            std::io::stdin()
                .take((INPUT_BYTES + 1) as u64)
                .read_to_end(&mut input)
                .unwrap();
            assert_eq!(input, duplex_input(), "stdin must arrive exactly once");
            output.write_all(INPUT_PAYLOAD).unwrap();
            output.write_all(&input).unwrap();
            output.flush().unwrap();
        }
        if scenario == "mixed" {
            let mut stderr = std::io::stderr();
            stderr.write_all(PAYLOAD).unwrap();
            for _ in 0..MIB / chunk.len() {
                stderr.write_all(&chunk).unwrap();
            }
            stderr.flush().unwrap();
        }
        if let Some(child) = &mut descendant {
            let _ = child.wait();
        }
        std::process::exit(0);
    }

    #[test]
    fn pending_input_allows_draining_full_duplex_bulk_output_without_repeating_writes() {
        let scope = scope();
        let total_started = Instant::now();
        let spawn_started = Instant::now();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec("duplex", 4 * MIB, Duration::from_secs(15), 8 * MIB as u64),
            &scope,
        )
        .unwrap();
        let spawn_micros = spawn_started.elapsed().as_micros();
        assert!(matches!(
            process.begin_send(&[]),
            Err(ProcessError::Arguments)
        ));
        assert!(matches!(
            process.begin_send(&vec![0; INPUT_BYTES + 1]),
            Err(ProcessError::Arguments)
        ));
        let input = duplex_input();
        let mut pending = Some(process.begin_send(&input).unwrap());
        let queue_ready_started = Instant::now();
        wait_for(|| process.state.queue_full.load(Ordering::Acquire));
        let queue_ready_micros = queue_ready_started.elapsed().as_micros();
        assert_eq!(pending.as_mut().unwrap().try_complete(), Ok(None));
        let started = Instant::now();
        let mut receive_micros = 0;
        let mut tree_micros = 0;
        let mut tree_queries = 0;
        let mut output = Vec::new();
        let mut error = Vec::new();
        loop {
            let receive_started = Instant::now();
            let event = process.receive(Duration::from_millis(5)).unwrap();
            receive_micros += receive_started.elapsed().as_micros();
            if let Some(event) = event {
                match event {
                    ProcessEvent::Stdout(bytes) => output.extend(bytes),
                    ProcessEvent::Stderr(bytes) => error.extend(bytes),
                }
            }
            if let Some(write) = pending.as_mut()
                && write.try_complete().unwrap().is_some()
            {
                assert_eq!(write.try_complete(), Ok(Some(())));
                pending = None;
                process.close_input();
            }
            let streams = process.streams();
            let tree_started = Instant::now();
            let tree = process.tree().unwrap();
            tree_micros += tree_started.elapsed().as_micros();
            tree_queries += 1;
            if pending.is_none()
                && tree.root_exit_code == Some(0)
                && tree.active_processes == 0
                && streams.stdin_closed
                && streams.stdout_closed
                && streams.stderr_closed
            {
                while let Some(event) = process.receive(Duration::ZERO).unwrap() {
                    match event {
                        ProcessEvent::Stdout(bytes) => output.extend(bytes),
                        ProcessEvent::Stderr(bytes) => error.extend(bytes),
                    }
                }
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(10));
        }
        let drain_micros = started.elapsed().as_micros();
        let offset = output
            .windows(PAYLOAD.len())
            .position(|window| window == PAYLOAD)
            .unwrap()
            + PAYLOAD.len();
        assert_eq!(
            output.len() - offset,
            4 * MIB + INPUT_PAYLOAD.len() + input.len()
        );
        assert!(
            output[offset..offset + 4 * MIB]
                .iter()
                .all(|byte| *byte == b'z')
        );
        assert_eq!(
            &output[offset + 4 * MIB..offset + 4 * MIB + INPUT_PAYLOAD.len()],
            INPUT_PAYLOAD
        );
        assert_eq!(&output[offset + 4 * MIB + INPUT_PAYLOAD.len()..], input);
        assert!(error.is_empty());
        assert_eq!(process.health(), Ok(()));
        let state = Arc::clone(&process.state);
        let close_started = Instant::now();
        assert!(process.terminate(Duration::from_secs(3)).unwrap().owned);
        assert_closed(&process.state);
        let terminate_micros = close_started.elapsed().as_micros();
        let drop_started = Instant::now();
        drop(process);
        let drop_micros = drop_started.elapsed().as_micros();
        assert_eq!(scope.active().unwrap(), 0);
        println!(
            "ORCHESTRIX_BULK_TIMING={{\"scenario\":\"duplex\",\"spawnMicros\":{spawn_micros},\"queueReadyMicros\":{queue_ready_micros},\"drainMicros\":{drain_micros},\"receiveMicros\":{receive_micros},\"treeObservationMicros\":{tree_micros},\"treeQueries\":{tree_queries},\"terminateMicros\":{terminate_micros},\"dropMicros\":{drop_micros},\"totalMicros\":{},\"stdoutBytes\":{},\"stderrBytes\":{},\"queueFullCount\":{},\"queueFullWaitMicros\":{}}}",
            total_started.elapsed().as_micros(),
            output.len(),
            error.len(),
            state.queue_full_count.load(Ordering::Relaxed),
            state.queue_full_wait_micros.load(Ordering::Relaxed),
        );
    }

    #[test]
    fn pending_full_duplex_input_remains_cancellable_and_watchdog_bounded() {
        for cancel in [true, false] {
            let scope = scope();
            let mut process = OwnedProcess::spawn_identified_backpressured(
                spec(
                    "duplex",
                    4 * MIB,
                    Duration::from_secs(if cancel { 15 } else { 1 }),
                    8 * MIB as u64,
                ),
                &scope,
            )
            .unwrap();
            let mut pending = process.begin_send(&duplex_input()).unwrap();
            wait_for(|| process.state.queue_full.load(Ordering::Acquire));
            assert_eq!(pending.try_complete(), Ok(None));
            if cancel {
                let tree = process.terminate(Duration::from_secs(3)).unwrap();
                assert!(tree.owned && tree.root_exit_code.is_some() && tree.active_processes == 0);
            } else {
                wait_for(|| process.health() == Err(ProcessError::Deadline));
            }
            assert_closed(&process.state);
            wait_for(|| scope.active().unwrap() == 0);
            assert!(matches!(
                pending.try_complete(),
                Err(ProcessError::Io | ProcessError::Closed | ProcessError::Deadline)
            ));
        }
    }

    #[test]
    fn disconnected_output_waits_without_claiming_owned_tree_exit() {
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec("closed-output-live", 0, Duration::from_secs(3), MIB as u64),
            &scope,
        )
        .unwrap();
        let startup = wait_for_disconnected_output(&process);
        assert_eq!(process.health(), Ok(()));
        let before = process.tree().unwrap();
        assert!(
            before.owned && before.active_processes > 0,
            "fixture must still be alive: {before:?}; startup: {startup:?}"
        );
        assert_eq!(before.root_exit_code, None, "EOF is not process-tree exit");

        let zero_started = Instant::now();
        assert!(process.receive(Duration::ZERO).unwrap().is_none());
        let zero_micros = zero_started.elapsed().as_micros();
        let timeout = Duration::from_millis(5);
        let started = Instant::now();
        let cpu_before = thread_cpu_100ns();
        let mut empty = 0u64;
        let mut disconnected = 0u64;
        let mut early = 0u64;
        let mut minimum_wait = Duration::MAX;
        let mut maximum_wait = Duration::ZERO;
        while started.elapsed() < Duration::from_millis(250) {
            let receive_started = Instant::now();
            assert!(process.receive(timeout).unwrap().is_none());
            let elapsed = receive_started.elapsed();
            empty += 1;
            if elapsed < timeout {
                early += 1;
            }
            minimum_wait = minimum_wait.min(elapsed);
            maximum_wait = maximum_wait.max(elapsed);
            assert!(matches!(
                process.events.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ));
            disconnected += 1;
            let tree = process.tree().unwrap();
            assert!(tree.owned && tree.active_processes > 0);
            assert_eq!(tree.root_exit_code, None);
        }
        let cpu_100ns = thread_cpu_100ns() - cpu_before;
        let elapsed_micros = started.elapsed().as_micros();
        let alive = process.tree().unwrap();
        process.send(&[1], Duration::from_secs(1)).unwrap();
        process.close_input();
        wait_for(|| {
            let tree = process.tree().unwrap();
            tree.root_exit_code == Some(0) && tree.active_processes == 0
        });
        let closed = process.terminate(Duration::from_secs(3)).unwrap();
        assert!(closed.owned && closed.active_processes == 0);
        assert!(closed.root_exit_code.is_some());
        assert_closed(&process.state);
        drop(process);
        assert_eq!(scope.active().unwrap(), 0);
        println!(
            "ORCHESTRIX_DISCONNECTED_RECEIVE={{\"requestedMicros\":{},\"elapsedMicros\":{elapsed_micros},\"threadCpu100ns\":{cpu_100ns},\"emptyReturns\":{empty},\"disconnectedObservations\":{disconnected},\"earlyReturns\":{early},\"minimumWaitMicros\":{},\"maximumWaitMicros\":{},\"zeroWaitMicros\":{zero_micros},\"rootStillAlive\":{},\"activeBeforeClose\":{},\"ownedTreeClosed\":true}}",
            timeout.as_micros(),
            minimum_wait.as_micros(),
            maximum_wait.as_micros(),
            alive.root_exit_code.is_none(),
            alive.active_processes,
        );
        assert_eq!(
            early, 0,
            "a disconnected channel must honor the requested wait"
        );
        assert!(
            zero_micros < 50_000,
            "nonblocking receive must remain immediate"
        );
    }

    #[test]
    fn open_output_timeout_preserves_the_live_owned_tree() {
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec("linger", 0, Duration::from_secs(15), MIB as u64),
            &scope,
        )
        .unwrap();
        process.close_input();
        let timeout = Duration::from_millis(25);
        let started = Instant::now();
        loop {
            let receive_started = Instant::now();
            if process.receive(timeout).unwrap().is_none() {
                assert!(receive_started.elapsed() >= timeout);
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3));
        }
        assert!(matches!(
            process.events.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        let streams = process.streams();
        assert!(!streams.stdout_closed && !streams.stderr_closed);
        let tree = process.tree().unwrap();
        assert!(tree.owned && tree.active_processes > 0 && tree.root_exit_code.is_none());
        let closed = process.terminate(Duration::from_secs(3)).unwrap();
        assert!(closed.owned && closed.active_processes == 0);
        assert_closed(&process.state);
    }

    #[test]
    fn disconnected_output_wait_rechecks_watchdog_health() {
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec("closed-output-live", 0, Duration::from_secs(1), MIB as u64),
            &scope,
        )
        .unwrap();
        let _startup = wait_for_disconnected_output(&process);
        assert_eq!(process.health(), Ok(()));
        let started = Instant::now();
        assert!(matches!(
            process.receive(Duration::from_secs(1)),
            Err(ProcessError::Deadline)
        ));
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert!(matches!(
            process.receive(Duration::ZERO),
            Err(ProcessError::Deadline)
        ));
        let closed = process.terminate(Duration::from_secs(3)).unwrap();
        assert!(closed.owned && closed.active_processes == 0);
        assert_closed(&process.state);
    }

    #[test]
    fn backpressured_slow_consumer_receives_exact_large_stdout_and_stderr() {
        for (stream, size) in [
            ("stdout", MIB + 123),
            ("mixed", 4 * MIB),
            ("stderr", MIB + 321),
        ] {
            let scope = scope();
            let mut process = OwnedProcess::spawn_identified_backpressured(
                spec(stream, size, Duration::from_secs(15), 8 * MIB as u64),
                &scope,
            )
            .unwrap();
            process.close_input();
            wait_for(|| process.state.queue_full.load(Ordering::Acquire));
            assert_eq!(
                process.health(),
                Ok(()),
                "queue pressure is not a byte-budget failure"
            );
            let started = Instant::now();
            let mut output = Vec::new();
            let mut error = Vec::new();
            loop {
                if let Some(event) = process.receive(Duration::from_millis(100)).unwrap() {
                    match event {
                        ProcessEvent::Stdout(bytes) => output.extend(bytes),
                        ProcessEvent::Stderr(bytes) => error.extend(bytes),
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                let streams = process.streams();
                let tree = process.tree().unwrap();
                if tree.root_exit_code == Some(0)
                    && tree.active_processes == 0
                    && streams.stdout_closed
                    && streams.stderr_closed
                {
                    while let Some(event) = process.receive(Duration::ZERO).unwrap() {
                        match event {
                            ProcessEvent::Stdout(bytes) => output.extend(bytes),
                            ProcessEvent::Stderr(bytes) => error.extend(bytes),
                        }
                    }
                    break;
                }
                assert!(started.elapsed() < Duration::from_secs(10));
            }
            let assert_payload = |bytes: &[u8], size| {
                let offset = bytes
                    .windows(PAYLOAD.len())
                    .position(|window| window == PAYLOAD)
                    .unwrap()
                    + PAYLOAD.len();
                assert_eq!(bytes.len() - offset, size);
                assert!(bytes[offset..].iter().all(|byte| *byte == b'z'));
            };
            if stream != "stderr" {
                assert_payload(&output, size);
            }
            if stream == "mixed" {
                assert_payload(&error, MIB);
            }
            if stream == "stderr" {
                assert_payload(&error, size);
            }
            assert_eq!(process.health(), Ok(()));
            assert!(process.terminate(Duration::from_secs(3)).unwrap().owned);
            assert_closed(&process.state);
        }
    }

    #[test]
    fn backpressure_keeps_the_aggregate_byte_budget() {
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec(
                "mixed",
                4 * MIB,
                Duration::from_secs(15),
                (4 * MIB + MIB / 2) as u64,
            ),
            &scope,
        )
        .unwrap();
        process.close_input();
        wait_for(|| process.state.queue_full.load(Ordering::Acquire));
        assert_eq!(process.health(), Ok(()));
        let started = Instant::now();
        loop {
            match process.receive(Duration::from_millis(100)) {
                Err(ProcessError::Budget) => break,
                Ok(_) => {}
                other => panic!("expected byte-budget rejection, got {other:?}"),
            }
            assert!(started.elapsed() < Duration::from_secs(3));
        }
        let tree = process.terminate(Duration::from_secs(3)).unwrap();
        assert!(tree.owned && tree.active_processes == 0);
        assert_closed(&process.state);
    }

    #[test]
    fn cancelling_a_full_queue_closes_readers_and_only_the_owned_tree() {
        let unrelated_scope = scope();
        let mut unrelated = OwnedProcess::spawn_identified(
            spec("linger", 0, Duration::from_secs(15), 8 * MIB as u64),
            &unrelated_scope,
        )
        .unwrap();
        unrelated.close_input();
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec(
                "descendant",
                4 * MIB,
                Duration::from_secs(15),
                8 * MIB as u64,
            ),
            &scope,
        )
        .unwrap();
        process.close_input();
        wait_for(|| {
            process.state.queue_full.load(Ordering::Acquire) && scope.active().unwrap() >= 2
        });
        let began = Instant::now();
        let tree = process.terminate(Duration::from_secs(3)).unwrap();
        assert!(tree.owned && tree.root_exit_code.is_some() && tree.active_processes == 0);
        assert_closed(&process.state);
        assert!(began.elapsed() < Duration::from_secs(3));
        assert!(unrelated_scope.active().unwrap() > 0);
        assert_eq!(unrelated.tree().unwrap().root_exit_code, None);
        unrelated.terminate(Duration::from_secs(3)).unwrap();
    }

    #[test]
    fn drop_releases_a_backpressured_reader_without_consuming_the_queue() {
        let scope = scope();
        let process = OwnedProcess::spawn_identified_backpressured(
            spec(
                "descendant",
                4 * MIB,
                Duration::from_secs(15),
                8 * MIB as u64,
            ),
            &scope,
        )
        .unwrap();
        wait_for(|| {
            process.state.queue_full.load(Ordering::Acquire) && scope.active().unwrap() >= 2
        });
        let state = Arc::clone(&process.state);
        let began = Instant::now();
        drop(process);
        assert_closed(&state);
        wait_for(|| scope.active().unwrap() == 0);
        assert!(began.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn watchdog_remains_independent_of_a_saturated_output_queue() {
        let scope = scope();
        let mut process = OwnedProcess::spawn_identified_backpressured(
            spec(
                "descendant",
                4 * MIB,
                Duration::from_secs(1),
                8 * MIB as u64,
            ),
            &scope,
        )
        .unwrap();
        process.close_input();
        wait_for(|| process.state.queue_full.load(Ordering::Acquire));
        wait_for(|| process.health() == Err(ProcessError::Deadline));
        assert_closed(&process.state);
        wait_for(|| scope.active().unwrap() == 0);
    }

    #[test]
    fn both_existing_spawn_apis_still_fail_fast_on_a_full_event_queue() {
        for identified in [false, true] {
            let scope = scope();
            let request = spec("stdout", 4 * MIB, Duration::from_secs(15), 8 * MIB as u64);
            let mut process = if identified {
                OwnedProcess::spawn_identified(request, &scope)
            } else {
                OwnedProcess::spawn(request)
            }
            .unwrap();
            process.close_input();
            wait_for(|| process.health() == Err(ProcessError::Budget));
            assert!(process.state.queue_full.load(Ordering::Acquire));
            assert!(process.state.bytes.load(Ordering::Acquire) < 8 * MIB as u64);
            let tree = process.terminate(Duration::from_secs(3)).unwrap();
            assert!(tree.owned && tree.active_processes == 0);
        }
    }
}
