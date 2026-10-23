//! Opt-in, bounded local timing of one integration preparation. Never authority.
//! No stdout/stderr sink: a full pipe must not stall a supervised operation.
use serde::Serialize;
use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

const MAX_EVENTS: usize = 256;
const MAX_BYTES: usize = 128 * 1024;
const MAX_TRACES: u64 = 32;
static NEXT_TRACE: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static ACTIVE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Request,
    ValidateRequest,
    LookupExisting,
    InspectSource,
    AcquireLease,
    LoadState,
    InitialTarget,
    Reserve,
    LoadIntent,
    PrepareSupervisor,
    RecordSupervisor,
    StartSupervisor,
    CreateExecutor,
    TargetInJob,
    Materialize,
    MaterializeBase,
    MaterializeCapture,
    RecordMaterialized,
    RecordChecking,
    StartWatch,
    InspectBeforeCheck,
    Check,
    InspectAfterChecks,
    RecordChecks,
    PlanCommit,
    RecordCommitPlan,
    InspectBeforeCommit,
    WriteReference,
    CloseJob,
    ReadClosure,
    RecordIntegrated,
    ConfirmCandidate,
    RecordFailure,
    ReadResult,
    ValidateReadOnly,
    ValidateInspect,
    ValidateCommit,
    ValidateTree,
    ValidateTarget,
}
impl Phase {
    fn label(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::ValidateRequest => "validate-request",
            Self::LookupExisting => "lookup-existing",
            Self::InspectSource => "inspect-source",
            Self::AcquireLease => "acquire-lease",
            Self::LoadState => "load-state",
            Self::InitialTarget => "initial-target",
            Self::Reserve => "reserve",
            Self::LoadIntent => "load-intent",
            Self::PrepareSupervisor => "prepare-supervisor",
            Self::RecordSupervisor => "record-supervisor",
            Self::StartSupervisor => "start-supervisor",
            Self::CreateExecutor => "create-executor",
            Self::TargetInJob => "target-in-job",
            Self::Materialize => "materialize",
            Self::MaterializeBase => "materialize-base",
            Self::MaterializeCapture => "materialize-capture",
            Self::RecordMaterialized => "record-materialized",
            Self::RecordChecking => "record-checking",
            Self::StartWatch => "start-watch",
            Self::InspectBeforeCheck => "inspect-before-check",
            Self::Check => "check",
            Self::InspectAfterChecks => "inspect-after-checks",
            Self::RecordChecks => "record-checks",
            Self::PlanCommit => "plan-commit",
            Self::RecordCommitPlan => "record-commit-plan",
            Self::InspectBeforeCommit => "inspect-before-commit",
            Self::WriteReference => "write-reference",
            Self::CloseJob => "close-job",
            Self::ReadClosure => "read-closure",
            Self::RecordIntegrated => "record-integrated",
            Self::ConfirmCandidate => "confirm-candidate",
            Self::RecordFailure => "record-failure",
            Self::ReadResult => "read-result",
            Self::ValidateReadOnly => "validate-read-only",
            Self::ValidateInspect => "validate-inspect",
            Self::ValidateCommit => "validate-commit",
            Self::ValidateTree => "validate-tree",
            Self::ValidateTarget => "validate-target",
        }
    }
}

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Counters {
    git_process_attempts: u64,
    git_transport_micros: u64,
    git_input_bytes: u64,
    git_stdout_bytes: u64,
    capture_file_bytes: u64,
    store_loads: u64,
    store_load_micros: u64,
    store_commands: u64,
    store_command_micros: u64,
}
impl Counters {
    fn since(self, before: Self) -> Self {
        Self {
            git_process_attempts: self
                .git_process_attempts
                .saturating_sub(before.git_process_attempts),
            git_transport_micros: self
                .git_transport_micros
                .saturating_sub(before.git_transport_micros),
            git_input_bytes: self.git_input_bytes.saturating_sub(before.git_input_bytes),
            git_stdout_bytes: self
                .git_stdout_bytes
                .saturating_sub(before.git_stdout_bytes),
            capture_file_bytes: self
                .capture_file_bytes
                .saturating_sub(before.capture_file_bytes),
            store_loads: self.store_loads.saturating_sub(before.store_loads),
            store_load_micros: self
                .store_load_micros
                .saturating_sub(before.store_load_micros),
            store_commands: self.store_commands.saturating_sub(before.store_commands),
            store_command_micros: self
                .store_command_micros
                .saturating_sub(before.store_command_micros),
        }
    }
}
struct State {
    file: Option<File>,
    start: Instant,
    operation_hash: String,
    trace: u64,
    next_span: u64,
    stack: Vec<u64>,
    events: usize,
    bytes: usize,
    counters: Counters,
}
impl State {
    fn emit(&mut self, mut event: serde_json::Value) {
        if self.file.is_none() {
            return;
        }
        event["schemaVersion"] = 1.into();
        event["processId"] = std::process::id().into();
        event["trace"] = self.trace.into();
        event["operationHash"] = self.operation_hash.clone().into();
        event["elapsedMicros"] = micros(self.start).into();
        // Counters on nested spans are inclusive; never sum parent and child.
        event["inclusive"] = true.into();
        event["authority"] = false.into();
        let Ok(mut line) = serde_json::to_vec(&event) else {
            self.file = None;
            return;
        };
        line.push(b'\n');
        if self.events >= MAX_EVENTS
            || line.len() > 2048
            || self.bytes.saturating_add(line.len()) > MAX_BYTES
        {
            // A missing/unmatched end is an incomplete diagnostic, never success.
            self.file = None;
            return;
        }
        self.events += 1;
        self.bytes += line.len();
        if self
            .file
            .as_mut()
            .is_some_and(|file| file.write_all(&line).is_err())
        {
            self.file = None;
        }
    }
}
fn micros(start: Instant) -> u64 {
    start.elapsed().as_micros().try_into().unwrap_or(u64::MAX)
}

pub(crate) struct PreparationTrace {
    state: Option<Rc<RefCell<State>>>,
}
impl PreparationTrace {
    pub(crate) fn disabled() -> Self {
        Self { state: None }
    }
    pub(crate) fn new(operation_id: &str) -> Self {
        let started = Instant::now();
        if std::env::var("ORCHESTRIX_INTEGRATION_DIAGNOSTICS").as_deref() != Ok("1") {
            return Self { state: None };
        }
        let directory = std::env::var_os("ORCHESTRIX_INTEGRATION_DIAGNOSTICS_DIRECTORY");
        Self::in_directory_at(directory.as_deref().map(Path::new), operation_id, started)
    }
    #[cfg(test)]
    fn in_directory(directory: Option<&Path>, operation_id: &str) -> Self {
        Self::in_directory_at(directory, operation_id, Instant::now())
    }
    fn in_directory_at(directory: Option<&Path>, operation_id: &str, started: Instant) -> Self {
        // Diagnostics precede admission validation. Do not hash an unbounded
        // invalid direct-call identifier just to produce an optional trace.
        if operation_id.len() > 80 {
            return Self::disabled();
        }
        let state = (|| {
            if ACTIVE.with(|slot| slot.borrow().is_some()) {
                return None;
            }
            let directory = crate::git::paths::canonical_directory(directory?).ok()?;
            let trace = NEXT_TRACE.fetch_add(1, Ordering::Relaxed);
            if trace > MAX_TRACES {
                return None;
            }
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(format!("prepare-{}-{trace}.jsonl", std::process::id())))
                .ok()?;
            if !file.metadata().ok()?.is_file() {
                return None;
            }
            let state = Rc::new(RefCell::new(State {
                file: Some(file),
                start: started,
                operation_hash: crate::git_records::content_hash(operation_id.as_bytes()),
                trace,
                next_span: 1,
                stack: Vec::new(),
                events: 0,
                bytes: 0,
                counters: Counters::default(),
            }));
            ACTIVE.with(|slot| {
                *slot.borrow_mut() = Some(Rc::clone(&state));
            });
            Some(state)
        })();
        Self { state }
    }
    pub(crate) fn measure<T, E>(
        &self,
        phase: Phase,
        check: Option<usize>,
        action: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let span = self
            .state
            .as_ref()
            .and_then(|state| Span::start(Rc::clone(state), phase, check));
        let result = action();
        if let Some(span) = span {
            span.finish(if result.is_ok() { "ok" } else { "error" });
        }
        result
    }
    pub(crate) fn result(&self, status: PreparationResult) {
        if let Some(state) = &self.state {
            let status = match status {
                PreparationResult::Prepared => "prepared",
                PreparationResult::Blocked => "blocked",
                PreparationResult::Uncertain => "uncertain",
                PreparationResult::Other => "other-record",
                PreparationResult::Error => "error",
            };
            state
                .borrow_mut()
                .emit(serde_json::json!({"event":"result","status":status}));
        }
    }
}
pub(crate) enum PreparationResult {
    Prepared,
    Blocked,
    Uncertain,
    Other,
    Error,
}
impl Drop for PreparationTrace {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let _ = ACTIVE.try_with(|slot| {
                if slot
                    .borrow()
                    .as_ref()
                    .is_some_and(|active| Rc::ptr_eq(active, state))
                {
                    *slot.borrow_mut() = None;
                }
            });
        }
    }
}
pub(crate) fn measure<T, E>(phase: Phase, action: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let span = ACTIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|state| Span::start(Rc::clone(state), phase, None))
    });
    let result = action();
    if let Some(span) = span {
        span.finish(if result.is_ok() { "ok" } else { "error" });
    }
    result
}
struct Span {
    state: Rc<RefCell<State>>,
    phase: Phase,
    check: Option<usize>,
    id: u64,
    parent: Option<u64>,
    start: Instant,
    before: Counters,
    finished: bool,
}
impl Span {
    fn start(state: Rc<RefCell<State>>, phase: Phase, check: Option<usize>) -> Option<Self> {
        if check.is_some_and(|index| index >= 16) {
            return None;
        }
        let mut inner = state.borrow_mut();
        if inner.file.is_none() || inner.stack.len() >= 16 {
            return None;
        }
        let id = inner.next_span;
        inner.next_span += 1;
        let parent = inner.stack.last().copied();
        inner.stack.push(id);
        let before = inner.counters;
        inner.emit(serde_json::json!({"event":"begin","phase":phase.label(),"check":check,"span":id,"parent":parent}));
        drop(inner);
        Some(Self {
            state,
            phase,
            check,
            id,
            parent,
            start: Instant::now(),
            before,
            finished: false,
        })
    }
    fn finish(mut self, outcome: &'static str) {
        self.end(outcome);
    }
    fn end(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        let elapsed = micros(self.start);
        let mut inner = self.state.borrow_mut();
        let counters = inner.counters.since(self.before);
        inner.stack.retain(|id| *id != self.id);
        inner.emit(serde_json::json!({"event":"end","phase":self.phase.label(),"check":self.check,"span":self.id,"parent":self.parent,"spanMicros":elapsed,"outcome":outcome,"counters":counters}));
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        self.end("aborted");
    }
}

pub(crate) enum CounterKind {
    GitTransport,
    StoreLoad,
    StoreCommand,
}
pub(crate) struct CounterTimer {
    state: Option<Rc<RefCell<State>>>,
    kind: CounterKind,
    start: Instant,
}
impl CounterTimer {
    pub(crate) fn start(kind: CounterKind) -> Self {
        let state = ACTIVE.with(|slot| slot.borrow().clone());
        if let Some(state) = &state {
            let mut inner = state.borrow_mut();
            let count = match kind {
                CounterKind::GitTransport => &mut inner.counters.git_process_attempts,
                CounterKind::StoreLoad => &mut inner.counters.store_loads,
                CounterKind::StoreCommand => &mut inner.counters.store_commands,
            };
            *count = count.saturating_add(1);
        }
        Self {
            state,
            kind,
            start: Instant::now(),
        }
    }
}
impl Drop for CounterTimer {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let mut inner = state.borrow_mut();
            let value = match self.kind {
                CounterKind::GitTransport => &mut inner.counters.git_transport_micros,
                CounterKind::StoreLoad => &mut inner.counters.store_load_micros,
                CounterKind::StoreCommand => &mut inner.counters.store_command_micros,
            };
            *value = value.saturating_add(micros(self.start));
        }
    }
}
pub(crate) fn git_bytes(input: usize, output: usize) {
    update(|c| {
        c.git_input_bytes = c.git_input_bytes.saturating_add(input as u64);
        c.git_stdout_bytes = c.git_stdout_bytes.saturating_add(output as u64);
    });
}
pub(crate) fn capture_bytes(bytes: usize) {
    update(|c| {
        c.capture_file_bytes = c.capture_file_bytes.saturating_add(bytes as u64);
    });
}
fn update(action: impl FnOnce(&mut Counters)) {
    let _ = ACTIVE.try_with(|slot| {
        if let Some(state) = slot.borrow().as_ref() {
            action(&mut state.borrow_mut().counters);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "orchestrix-prepare-trace-{}-{}",
            std::process::id(),
            crate::git_records::content_hash(format!("{:?}", Instant::now()).as_bytes())
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn begin_is_written_before_action_and_nested_metrics_are_inclusive() {
        let root = directory();
        let trace = PreparationTrace::in_directory(Some(&root), "private-name-must-not-be-logged");
        let result: Result<(), ()> = trace.measure(Phase::Request, None, || {
            let filename = std::fs::read_dir(&root)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let begin = std::fs::read_to_string(filename).unwrap();
            assert!(begin.contains("\"event\":\"begin\""));
            assert!(!begin.contains("private-name"));
            trace.measure(Phase::Check, Some(0), || {
                let _timer = CounterTimer::start(CounterKind::StoreLoad);
                capture_bytes(123);
                Ok(())
            })
        });
        assert!(result.is_ok());
        drop(trace);
        let filename = std::fs::read_dir(&root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(filename).unwrap();
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 4);
        assert_eq!(events[2]["counters"]["captureFileBytes"], 123);
        assert_eq!(events[3]["counters"]["captureFileBytes"], 123);
        assert_eq!(events[2]["parent"], events[0]["span"]);
        assert_eq!(events[3]["outcome"], "ok");
        assert!(
            events
                .windows(2)
                .all(|w| w[1]["elapsedMicros"].as_u64() >= w[0]["elapsedMicros"].as_u64())
        );
    }
    #[test]
    fn missing_sink_never_changes_results_and_errors_are_not_success() {
        let disabled = PreparationTrace::in_directory(None, "unused");
        assert_eq!(
            disabled.measure(Phase::Request, None, || Err::<(), _>("sentinel")),
            Err("sentinel")
        );
        let root = directory();
        let trace = PreparationTrace::in_directory(Some(&root), "error");
        assert_eq!(
            trace.measure(Phase::InspectBeforeCheck, Some(0), || Err::<(), _>(7)),
            Err(7)
        );
        drop(trace);
        let filename = std::fs::read_dir(root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(filename).unwrap();
        assert!(text.contains("\"outcome\":\"error\""));
    }
    #[test]
    fn trace_size_is_bounded_and_aborted_span_stays_visible() {
        let root = directory();
        let trace = PreparationTrace::in_directory(Some(&root), "bounded");
        let state = Rc::clone(trace.state.as_ref().unwrap());
        {
            let _span = Span::start(Rc::clone(&state), Phase::Check, Some(0)).unwrap();
        }
        for _ in 0..MAX_EVENTS {
            trace
                .measure::<(), ()>(Phase::Check, Some(1), || Ok(()))
                .unwrap();
        }
        assert!(state.borrow().events <= MAX_EVENTS);
        assert!(state.borrow().bytes <= MAX_BYTES);
        drop(trace);
        let filename = std::fs::read_dir(root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(filename).unwrap();
        assert!(text.contains("\"outcome\":\"aborted\""));
        assert!(text.lines().count() <= MAX_EVENTS);
    }
    #[test]
    fn invalid_sink_and_oversized_identity_do_not_create_trace_or_change_action() {
        let root = directory();
        let ordinary = root.join("ordinary-file");
        std::fs::write(&ordinary, b"unchanged").unwrap();
        for (sink, id) in [(&ordinary, "test".to_owned()), (&root, "x".repeat(81))] {
            let trace = PreparationTrace::in_directory(Some(sink), &id);
            assert!(trace.state.is_none());
            assert_eq!(
                trace.measure(Phase::Request, None, || Ok::<_, ()>(42)),
                Ok(42)
            );
        }
        assert_eq!(std::fs::read(ordinary).unwrap(), b"unchanged");
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
    }
    #[test]
    fn returning_an_uncertain_record_is_not_prepared_authority() {
        let root = directory();
        let trace = PreparationTrace::in_directory(Some(&root), "uncertain-record");
        trace
            .measure::<(), ()>(Phase::Request, None, || Ok(()))
            .unwrap();
        trace.result(PreparationResult::Uncertain);
        drop(trace);
        let filename = std::fs::read_dir(root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let events: Vec<serde_json::Value> = std::fs::read_to_string(filename)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.last().unwrap()["event"], "result");
        assert_eq!(events.last().unwrap()["status"], "uncertain");
        assert!(events.iter().all(|event| event["authority"] == false));
    }
}
