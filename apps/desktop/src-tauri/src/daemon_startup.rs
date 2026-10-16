//! Native launch/readiness observation, not a daemon or model execution engine.
//! A connect may launch once; follow-up observation never launches or kills.
use orchestrix_core::{
    daemon::{self, API_VERSION, Client, Operation, Request},
    daemon_startup::{NONCE_ENV, Phase, PhaseObservation, ReportDecoder},
};
use orchestrix_process::{
    named_job,
    startup_pipe::{self, PipeRead, StartupReader},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct StartupLimits {
    total: Duration,
}
impl StartupLimits {
    /// The caller selects the measured finite follow-up budget. This is not an
    /// IPC timeout, a model-turn deadline, or authority to stop an owned daemon.
    pub fn new(total: Duration) -> Result<Self, String> {
        if total < Duration::from_secs(15) || total > Duration::from_secs(300) {
            return Err("startup-observation-budget".into());
        }
        Ok(Self { total })
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    Initializing,
    Recovering,
    Blocked,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionObservation {
    pub state: Availability,
    pub phase: Option<Phase>,
    pub phases: Vec<PhaseObservation>,
    pub phase_source: &'static str,
    pub host_elapsed_ms: u64,
    pub daemon_elapsed_ms: Option<u64>,
    pub past_initial_window: bool,
    pub automatic_follow_up: bool,
    pub explicit_connect_available: bool,
    pub follow_up_budget_ms: u64,
    pub launched_by_this_host: bool,
    pub launch_nonce: Option<String>,
    pub own_child_pid: Option<u32>,
    pub own_child_exit_code: Option<i32>,
    pub diagnostic: Option<&'static str>,
}
pub enum ConnectionUpdate {
    Ready(Value),
    Pending(ConnectionObservation),
}
impl ConnectionUpdate {
    pub fn into_value(self) -> Value {
        match self {
            Self::Ready(snapshot) => snapshot,
            Self::Pending(observation) => json!({"startup":observation}),
        }
    }
}
struct Launch {
    child: Child,
    reader: Option<StartupReader>,
    decoder: ReportDecoder,
    nonce: String,
    exited: bool,
    exit_code: Option<i32>,
    report_rejected: bool,
}
impl Launch {
    fn observe_report(&mut self) {
        let mut closed = false;
        if let Some(reader) = &mut self.reader {
            for _ in 0..16 {
                match reader.poll(Duration::from_millis(5)) {
                    Ok(PipeRead::Data(bytes)) => {
                        if self.decoder.feed(&bytes).is_err() {
                            self.report_rejected = true;
                            break;
                        }
                        if self.decoder.observation().report_completed {
                            break;
                        }
                    }
                    Ok(PipeRead::Pending) => break,
                    Ok(PipeRead::Closed) => {
                        closed = true;
                        break;
                    }
                    Err(_) => {
                        self.report_rejected = true;
                        break;
                    }
                }
            }
        }
        if closed || self.report_rejected || self.decoder.observation().report_completed {
            self.reader = None;
        }
    }
}
struct Attempt {
    started: Instant,
    launch: Option<Launch>,
    failed: bool,
    was_ready: bool,
}
impl Attempt {
    fn expired_observer(&self, budget: Duration) -> bool {
        self.launch.is_none() && self.started.elapsed() >= budget
    }
}
pub struct StartupController {
    limits: StartupLimits,
    attempt: Option<Attempt>,
}
impl StartupController {
    pub fn new(limits: StartupLimits) -> Self {
        Self {
            limits,
            attempt: None,
        }
    }
    pub fn connect(
        &mut self,
        database: &Path,
        core: &Path,
        keeper: &Path,
        compatible: fn(Value) -> Result<Value, String>,
    ) -> Result<ConnectionUpdate, String> {
        self.update(database, Some((core, keeper)), compatible)
    }
    pub fn observe(
        &mut self,
        database: &Path,
        compatible: fn(Value) -> Result<Value, String>,
    ) -> Result<ConnectionUpdate, String> {
        self.update(database, None, compatible)
    }
    fn update(
        &mut self,
        database: &Path,
        launch_paths: Option<(&Path, &Path)>,
        compatible: fn(Value) -> Result<Value, String>,
    ) -> Result<ConnectionUpdate, String> {
        if let Some(snapshot) = probe(database)? {
            // Only the normal compatible handshake grants readiness. The
            // private progress channel can add presentation timing afterwards.
            let mut snapshot = compatible(snapshot)?;
            if let Some(attempt) = &mut self.attempt {
                attempt.was_ready = true;
                if let Some(launch) = &mut attempt.launch {
                    launch.observe_report();
                    let report = launch.decoder.observation();
                    if !launch.report_rejected && !report.phases.is_empty() {
                        snapshot["startupDiagnostics"] = json!({"phaseSource":"own-startup-report","phases":report.phases,"daemonElapsedMs":report.daemon_elapsed_ms,"hostElapsedMs":millis(attempt.started.elapsed())});
                        // This trace is bound to the Child/nonce, not to the
                        // process serving the independently validated Snapshot.
                        snapshot["startupDiagnostics"]["reportedProcessPid"] =
                            json!(launch.child.id());
                        snapshot["startupDiagnostics"]["daemonBindingConfirmed"] = json!(false);
                        snapshot["startupDiagnostics"]["reportCompleted"] =
                            json!(report.report_completed);
                        snapshot["startupDiagnostics"]["reportedFailure"] =
                            json!(report.reported_failure);
                    }
                }
            }
            return Ok(ConnectionUpdate::Ready(snapshot));
        }
        let owner = owner_busy(database)?;
        if launch_paths.is_some()
            && self.attempt.as_ref().is_some_and(|attempt| {
                attempt.failed || attempt.was_ready || attempt.expired_observer(self.limits.total)
            })
            && !owner
        {
            // Only explicit connect may retry a confirmed failure, reconnect
            // after readiness, or replace an expired observer with no Child.
            // The current ownership check above still has to be free.
            self.attempt = None;
        }
        if self.attempt.is_none() {
            let started = Instant::now();
            let launch = if !owner {
                let Some((core, keeper)) = launch_paths else {
                    return Err("startup-not-started-use-connect".into());
                };
                Some(start(database, core, keeper)?)
            } else {
                None
            };
            self.attempt = Some(Attempt {
                started,
                launch,
                failed: false,
                was_ready: false,
            });
        }
        let attempt = self.attempt.as_mut().expect("attempt initialized");
        let mut phase = None;
        let mut phases = vec![];
        let mut daemon_elapsed_ms = None;
        let mut phase_source = "unknown";
        let mut diagnostic = None;
        let mut child_exited = false;
        let mut child_running = false;
        let mut exit_code = None;
        if let Some(launch) = &mut attempt.launch {
            launch.observe_report();
            if let Some(status) = launch
                .child
                .try_wait()
                .map_err(|_| "daemon-start-observation")?
            {
                launch.exited = true;
                launch.exit_code = status.code();
            }
            child_exited = launch.exited;
            child_running = !launch.exited;
            exit_code = launch.exit_code;
            if !launch.report_rejected {
                let observed = launch.decoder.observation();
                phase = observed.phase;
                phases = observed.phases.clone();
                daemon_elapsed_ms = phase.map(|_| observed.daemon_elapsed_ms);
                if phase.is_some() {
                    phase_source = "own-startup-report";
                }
            } else if launch.report_rejected {
                diagnostic = Some("startup-report-unavailable");
            }
        }
        // A losing child may exit while the winner is still reconciling. This
        // does not authorize another spawn or identify the winner's exact phase.
        let owner = owner_busy(database)?;
        if attempt.launch.is_none() && owner {
            phase_source = "other-owner";
            diagnostic = Some("waiting-for-existing-owner");
        }
        if child_exited && owner {
            phase = None;
            phases.clear();
            daemon_elapsed_ms = None;
            phase_source = "other-owner";
            diagnostic = Some("waiting-for-existing-owner");
        }
        let elapsed = attempt.started.elapsed();
        let state = classify(
            child_running,
            child_exited,
            owner,
            phase,
            elapsed >= self.limits.total,
        );
        attempt.failed = state == Availability::Failed;
        if state == Availability::Failed {
            diagnostic = Some("daemon-exited-before-compatible-handshake");
        }
        if state == Availability::Blocked {
            diagnostic = Some("startup-observation-budget-exhausted-check-again");
        }
        let launch = attempt.launch.as_ref();
        Ok(ConnectionUpdate::Pending(ConnectionObservation {
            state,
            phase,
            phases,
            phase_source,
            host_elapsed_ms: millis(elapsed),
            daemon_elapsed_ms,
            past_initial_window: elapsed >= Duration::from_secs(15),
            automatic_follow_up: !matches!(state, Availability::Blocked | Availability::Failed),
            explicit_connect_available: state == Availability::Blocked
                && !owner
                && attempt.expired_observer(self.limits.total),
            follow_up_budget_ms: millis(self.limits.total),
            launched_by_this_host: launch.is_some(),
            launch_nonce: launch.map(|l| l.nonce.clone()),
            own_child_pid: launch.map(|l| l.child.id()),
            own_child_exit_code: exit_code,
            diagnostic,
        }))
    }
}
fn classify(
    child_running: bool,
    child_exited: bool,
    owner: bool,
    phase: Option<Phase>,
    exhausted: bool,
) -> Availability {
    // A temporarily busy ownership probe is not proof that a daemon existed
    // or exited. Only our own observed Child exit establishes this failure.
    if !child_running && child_exited && !owner {
        return Availability::Failed;
    }
    if exhausted {
        return Availability::Blocked;
    }
    if !child_exited && phase.is_some_and(Phase::recovering) {
        Availability::Recovering
    } else {
        Availability::Initializing
    }
}
fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}
fn probe(database: &Path) -> Result<Option<Value>, String> {
    match Client::new(database)?.request(&Request {
        api_version: API_VERSION,
        operation: Operation::Snapshot,
    }) {
        Ok(response) if response.ok => Ok(Some(response.result)),
        Ok(_) => Err("daemon-handshake-rejected".into()),
        Err(code)
            if [
                "daemon-unavailable",
                "request-not-confirmed",
                "response-not-confirmed-query-before-retry",
            ]
            .contains(&code.as_str()) =>
        {
            Ok(None)
        }
        Err(code) => Err(code),
    }
}
fn owner_busy(database: &Path) -> Result<bool, String> {
    match daemon::owner_lease(database) {
        Ok(lease) => {
            drop(lease);
            Ok(false)
        }
        Err(code) if code == "daemon-already-running-or-legacy-host-active" => Ok(true),
        Err(_) => Err("startup-ownership-unavailable".into()),
    }
}
fn start(database: &Path, core: &Path, keeper: &Path) -> Result<Launch, String> {
    if !core.is_file() || !keeper.is_file() {
        return Err("core-and-supervisor-executables-required-beside-desktop".into());
    }
    let scope = daemon::endpoint_scope(database)?;
    let nonce = named_job::nonce().map_err(|_| "startup-nonce")?;
    let (reader, writer) = startup_pipe::channel().map_err(|_| "startup-channel")?;
    let mut command = Command::new(core);
    command
        .args(["daemon", "--database"])
        .arg(database)
        .env(NONCE_ENV, &nonce)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000200);
    }
    let child = command.spawn().map_err(|_| "daemon-start-failed")?;
    drop(command); // Close the parent's extra writer; the child owns its stdout.
    let decoder = ReportDecoder::new(nonce.clone(), scope, child.id()).map_err(str::to_owned)?;
    Ok(Launch {
        child,
        reader: Some(reader),
        decoder,
        nonce,
        exited: false,
        exit_code: None,
        report_rejected: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification_preserves_progress_and_never_confuses_exit_or_report_with_readiness() {
        assert_eq!(
            classify(true, false, true, Some(Phase::WorkerRecovery), false),
            Availability::Recovering
        );
        assert_eq!(
            classify(false, true, true, Some(Phase::WorkerRecovery), false),
            Availability::Initializing
        );
        assert_eq!(
            classify(true, false, true, Some(Phase::IpcAvailable), false),
            Availability::Initializing
        );
        assert_eq!(
            classify(true, false, true, Some(Phase::WorkerRecovery), true),
            Availability::Blocked
        );
        assert_eq!(
            classify(false, true, false, Some(Phase::WorkerRecovery), false),
            Availability::Failed
        );
        assert_eq!(
            classify(false, false, false, None, false),
            Availability::Initializing
        );
        assert_eq!(
            classify(false, false, false, None, true),
            Availability::Blocked
        );
    }
    #[test]
    fn observation_budget_is_explicit_finite_and_distinct_from_initial_window() {
        assert!(StartupLimits::new(Duration::from_secs(14)).is_err());
        assert!(StartupLimits::new(Duration::from_secs(301)).is_err());
        let limits = StartupLimits::new(Duration::from_secs(15)).unwrap();
        assert_eq!(limits.total, Duration::from_secs(15));
        let mut observer = Attempt {
            started: Instant::now(),
            launch: None,
            failed: false,
            was_ready: false,
        };
        assert!(!observer.expired_observer(limits.total));
        observer.started = Instant::now() - Duration::from_secs(16);
        assert!(observer.expired_observer(limits.total));
    }
}
