//! Actual guard branches with a private bounded diagnostic sink. No provider,
//! environment mutation, artificial sleep or executable replacement is used.
use super::*;
use crate::integration_diagnostics::{self as diagnostic, PreparationTrace, TraceOperation};
use crate::runtime::CancellationToken;
use serde_json::Value;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
const PRIVATE_CANARY: &str = "read-deadline-private-canary";

struct TraceFixture {
    root: PathBuf,
    logs: PathBuf,
    cwd: PathBuf,
    temporary: PathBuf,
}

impl TraceFixture {
    fn new() -> Self {
        let temporary = canonical_directory(&env::temp_dir()).unwrap();
        let root = temporary.join(format!(
            "orchestrix-read-deadline-private-canary-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let logs = root.join("diagnostics");
        let cwd = root.join("empty-cwd");
        fs::create_dir(&logs).unwrap();
        fs::create_dir(&cwd).unwrap();
        Self {
            root,
            logs,
            cwd,
            temporary,
        }
    }

    fn trace(&self) -> PreparationTrace {
        PreparationTrace::in_directory_for_operation(
            Some(&self.logs),
            PRIVATE_CANARY,
            TraceOperation::RepositoryInspection,
        )
    }

    fn events(&self) -> Vec<Value> {
        let files = fs::read_dir(&self.logs)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 1);
        assert!(fs::metadata(&files[0]).unwrap().len() <= 128 * 1024);
        let text = fs::read_to_string(&files[0]).unwrap();
        assert!(!text.contains(PRIVATE_CANARY));
        assert!(!text.contains("private-binary-canary"));
        assert!(!text.contains("private-argument-canary"));
        assert!(!text.contains("private-content-canary"));
        let events = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert!(!events.is_empty());
        assert!(events.len() <= 256);
        assert!(events.iter().all(|event| event["authority"] == false));
        events
    }

    fn inert_executor(&self, cancel: CancellationToken) -> GitExecutor {
        GitExecutor {
            // Neither path exists: an expired/cancelled read must stop before
            // validating cwd/private storage or attempting to spawn anything.
            binary: self.root.join("private-binary-canary.exe"),
            private: self.root.join("nonexistent-private-storage"),
            scope: None,
            read_budget: Some((Instant::now(), Duration::ZERO, cancel)),
        }
    }
}

impl Drop for TraceFixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.root).unwrap();
        assert_eq!(root.parent(), Some(self.temporary.as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchestrix-read-deadline-private-canary-")
        );
        reject_links(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}

fn rejections(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| event["event"] == "read-budget-rejected")
        .collect()
}

fn assert_no_command_ready(events: &[Value]) {
    assert!(
        events
            .iter()
            .all(|event| event["event"] != "git-command-ready")
    );
    for event in events.iter().filter(|event| event["event"] == "end") {
        assert_eq!(event["counters"]["gitProcessAttempts"], 0);
        assert_eq!(event["counters"]["gitSpawnMicros"], 0);
    }
}

#[test]
fn expired_observation_rejects_before_command_or_private_path_access() {
    let fixture = TraceFixture::new();
    let executor = fixture.inert_executor(CancellationToken::new());
    let trace = fixture.trace();
    let result = executor.run_bounded(
        &fixture.root.join("nonexistent-cwd"),
        &strings(&["--version", "private-argument-canary"]),
        Some(b"private-content-canary"),
        DEADLINE,
        OUTPUT_LIMIT,
    );
    assert!(matches!(result, Err(GitError::Deadline)));
    drop(trace);
    let events = fixture.events();
    let rejected = rejections(&events);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["boundary"], "before-command");
    assert_eq!(rejected[0]["reason"], "deadline");
    assert_eq!(rejected[0]["observationLimitMs"], 0);
    assert!(rejected[0]["command"].is_null());
    assert_no_command_ready(&events);
}

#[test]
fn cancellation_keeps_priority_over_an_expired_observation_before_command() {
    let fixture = TraceFixture::new();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let executor = fixture.inert_executor(cancel);
    let trace = fixture.trace();
    let result = executor.run_bounded(
        &fixture.root.join("nonexistent-cwd"),
        &strings(&["--version", "private-argument-canary"]),
        None,
        DEADLINE,
        OUTPUT_LIMIT,
    );
    assert!(matches!(result, Err(GitError::Pending)));
    drop(trace);
    let events = fixture.events();
    let rejected = rejections(&events);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["boundary"], "before-command");
    assert_eq!(rejected[0]["reason"], "cancelled");
    assert_eq!(rejected[0]["observationLimitMs"], 0);
    assert!(rejected[0]["command"].is_null());
    assert_no_command_ready(&events);
}

#[test]
fn installed_git_command_deadline_is_distinct_while_observation_remains_valid() {
    let fixture = TraceFixture::new();
    // Construct/version-check the installed executor before collecting this
    // trace, so only the explicit zero-limit call is part of the evidence.
    let executor = GitExecutor::new().unwrap().with_read_budget(
        Instant::now(),
        Duration::from_secs(90),
        CancellationToken::new(),
    );
    assert_eq!(fs::read_dir(&fixture.cwd).unwrap().count(), 0);
    let trace = fixture.trace();
    let result = executor.run_bounded(
        &fixture.cwd,
        &strings(&["--version"]),
        None,
        Duration::ZERO,
        4096,
    );
    assert!(matches!(result, Err(GitError::Deadline)));
    drop(trace);
    assert_eq!(fs::read_dir(&fixture.cwd).unwrap().count(), 0);
    let events = fixture.events();
    let ready = events
        .iter()
        .filter(|event| event["event"] == "git-command-ready")
        .collect::<Vec<_>>();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0]["command"]["builtin"], "--version");
    assert_eq!(ready[0]["command"]["commandLimitMs"], 0);
    assert_eq!(ready[0]["command"]["effectiveLimitMs"], 0);
    assert_eq!(ready[0]["command"]["limitingBudget"], "command");
    assert!(ready[0]["command"]["transportElapsedMs"].is_null());
    assert!(
        ready[0]["command"]["observationRemainingAtSelectionMs"]
            .as_u64()
            .unwrap()
            > 0
    );
    let rejected = rejections(&events);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["boundary"], "command-wait");
    assert_eq!(rejected[0]["reason"], "deadline");
    assert_eq!(rejected[0]["command"]["builtin"], "--version");
    assert_eq!(rejected[0]["command"]["commandLimitMs"], 0);
    assert_eq!(rejected[0]["command"]["effectiveLimitMs"], 0);
    assert_eq!(rejected[0]["command"]["limitingBudget"], "command");
    assert!(rejected[0]["command"]["transportElapsedMs"].is_u64());
    // Obtaining Deadline here independently requires a successful real spawn:
    // spawn failure returns Process. The existing counter stays within its
    // enclosing transport/span without imposing a minimum process duration.
    let command_end = events
        .iter()
        .find(|event| {
            event["event"] == "end"
                && event["phase"] == "git-command"
                && event["span"] == rejected[0]["parent"]
        })
        .unwrap();
    let spawn_micros = command_end["counters"]["gitSpawnMicros"].as_u64().unwrap();
    let transport_micros = command_end["counters"]["gitTransportMicros"]
        .as_u64()
        .unwrap();
    assert_eq!(command_end["counters"]["gitProcessAttempts"], 1);
    // A real Windows Git spawn must register its measured duration. This
    // detects missing instrumentation, not a performance budget or threshold.
    assert!(spawn_micros > 0);
    assert!(spawn_micros <= transport_micros);
    assert!(spawn_micros <= command_end["spanMicros"].as_u64().unwrap());
    // The zero deadline rejects before this loop's first receive/try_wait.
    // These are LAST local facts, even if Git exited concurrently in the OS.
    assert!(ready[0]["command"].get("waitObservation").is_none());
    let wait = &rejected[0]["command"]["waitObservation"];
    assert_eq!(wait["statusLastObserved"], false);
    assert!(wait["exitCodeLastObserved"].is_null());
    assert_eq!(wait["closedStreamsLastObserved"], 0);
    assert_eq!(wait["inputClosedLastObserved"], true);
    assert_eq!(wait["stdoutBytesObserved"], 0);
    assert_eq!(wait["combinedOutputBytesObserved"], 0);
    assert!(
        rejected[0]["observationElapsedMs"].as_u64().unwrap()
            < rejected[0]["observationLimitMs"].as_u64().unwrap()
    );
}

#[test]
fn filesystem_rejection_preserves_deadline_then_cancellation_without_a_command() {
    let fixture = TraceFixture::new();
    let cancel = CancellationToken::new();
    let executor = fixture.inert_executor(cancel.clone());
    let trace = fixture.trace();
    assert_eq!(executor.check_read_budget(), Err(GitError::Deadline));
    cancel.cancel();
    assert_eq!(executor.check_read_budget(), Err(GitError::Pending));
    drop(trace);
    let events = fixture.events();
    let rejected = rejections(&events);
    assert_eq!(rejected.len(), 2);
    assert_eq!(rejected[0]["reason"], "deadline");
    assert_eq!(rejected[1]["reason"], "cancelled");
    for event in rejected {
        assert_eq!(event["boundary"], "filesystem-checkpoint");
        assert_eq!(event["observationLimitMs"], 0);
        assert!(event["command"].is_null());
    }
    assert_no_command_ready(&events);
}

#[test]
fn repository_scope_observation_is_private_bounded_and_rejects_invalid_oids() {
    let fixture = TraceFixture::new();
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let trace = fixture.trace();
    diagnostic::repository_scope_observed(&fixture.root, &head, &base, true);
    diagnostic::repository_scope_observed(&fixture.root, "fooNoSecret-invalid-head", &base, true);
    diagnostic::repository_scope_observed(&fixture.root, &head, "fooNoSecret-invalid-base", false);
    diagnostic::repository_scope_observed(&fixture.root, &"A".repeat(40), &base, true);
    drop(trace);

    let events = fixture.events();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event["event"], "repository-scope-observed");
    assert_eq!(event["operation"], "repository-inspection");
    assert_eq!(event["headOidObserved"], head);
    assert_eq!(event["baseOid"], base);
    assert_eq!(event["baseSource"], "request");
    assert_eq!(
        event["canonicalRootHash"],
        crate::git_records::content_hash(fixture.root.to_str().unwrap().as_bytes())
    );
    assert_eq!(event["canonicalRootHash"].as_str().unwrap().len(), 64);
    assert!(event["parent"].is_null());
    assert!(event.get("root").is_none());
    assert!(event.get("path").is_none());
    let text = serde_json::to_string(event).unwrap();
    assert!(text.len() < 2048);
    assert!(!text.contains(PRIVATE_CANARY));
    assert!(!text.contains("fooNoSecret"));
    assert!(!text.contains(fixture.root.to_str().unwrap()));
}

#[test]
fn command_detail_does_not_exhaust_existing_integration_operation_traces() {
    use diagnostic::{CounterKind, CounterTimer, Phase, PreparationResult};

    for (operation, root_phase, result, expected_operation, expected_result) in [
        (
            TraceOperation::Prepare,
            Phase::Request,
            PreparationResult::Prepared,
            "prepare",
            "prepared",
        ),
        (
            TraceOperation::Approve,
            Phase::ApproveRequest,
            PreparationResult::Approved,
            "approve",
            "approved",
        ),
        (
            TraceOperation::Apply,
            Phase::ApplyRequest,
            PreparationResult::Delivered,
            "apply",
            "delivered",
        ),
    ] {
        let fixture = TraceFixture::new();
        let trace = PreparationTrace::in_directory_for_operation(
            Some(&fixture.logs),
            PRIVATE_CANARY,
            operation,
        );
        trace
            .measure(root_phase, None, || {
                // Exercise the observational calls made by an Apply-sized
                // workload without starting Git or claiming any real effect.
                for _ in 0..267 {
                    diagnostic::measure(Phase::GitCommand, || {
                        let _transport = CounterTimer::start(CounterKind::GitTransport);
                        let command = CommandDeadline::new(
                            "status",
                            Duration::from_secs(15),
                            Duration::from_secs(15),
                            None,
                            None,
                        );
                        diagnostic::repository_scope_observed(
                            &fixture.root,
                            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                            true,
                        );
                        diagnostic::git_command_ready(command, None);
                        diagnostic::read_budget_rejected(
                            DeadlineBoundary::CommandWait,
                            false,
                            None,
                            Some(command),
                        );
                        Ok::<(), ()>(())
                    })?;
                }
                // A late ordinary span and the enclosing end/result must not
                // disappear because command detail consumed the trace budget.
                diagnostic::measure(Phase::ReadResult, || Ok::<(), ()>(()))
            })
            .unwrap();
        trace.result(result);
        drop(trace);

        let events = fixture.events();
        assert_eq!(events.len(), 5);
        assert!(events.iter().all(|event| {
            event["operation"] == expected_operation
                && event["event"] != "git-command-ready"
                && event["event"] != "read-budget-rejected"
                && event["event"] != "repository-scope-observed"
                && event["phase"] != "git-command"
        }));
        let root_end = events
            .iter()
            .find(|event| event["event"] == "end" && event["parent"].is_null())
            .unwrap();
        assert_eq!(root_end["counters"]["gitProcessAttempts"], 267);
        assert_eq!(root_end["outcome"], "ok");
        assert!(events.iter().any(|event| {
            event["event"] == "end" && event["phase"] == "read-result" && event["outcome"] == "ok"
        }));
        assert_eq!(events.last().unwrap()["event"], "result");
        assert_eq!(events.last().unwrap()["status"], expected_result);
    }
}
