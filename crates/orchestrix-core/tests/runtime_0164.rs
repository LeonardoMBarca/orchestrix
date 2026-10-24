//! Offline 0.164 profile qualification. Processes, Windows Jobs and stdio are real;
//! every protocol execution below remains an explicitly synthetic fixture.
#![cfg(windows)]

use orchestrix_core::runtime::{
    AttemptExecution, CancellationToken, CodexAdapter, RuntimeLaunch, RuntimeReport,
    SUPPORTED_CODEX_VERSIONS, SUPPORTED_STRUCTURED_CODEX_VERSIONS, SessionBinding, native_effort,
    neutral_effort,
};
use orchestrix_core::runtime_contract::{RuntimeObserver, normalize_request};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const NATIVE_0164: &str = "0.164.0-alpha.11.1";
const SYNTHETIC: &str = "synthetic-rust-1";
const METHODS_FILE: &str = "profile-0164-request-methods";

struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchestrix-runtime-0164-{}-{}-{}-á space",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let cwd = root.join("worktree");
        let home = root.join("auth-fixture");
        fs::create_dir(&cwd).unwrap();
        fs::create_dir(&home).unwrap();
        fs::write(cwd.join("fixture.txt"), "before\n").unwrap();
        Self { root, cwd, home }
    }
    fn launch(&self, scenario: &str) -> RuntimeLaunch {
        let mut launch = RuntimeLaunch::fixture(
            PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
            vec!["--scenario".into(), scenario.into()],
            self.home.clone(),
            self.cwd.clone(),
        );
        launch.request_timeout = Duration::from_millis(500);
        launch.turn_timeout = Duration::from_secs(2);
        launch.interrupt_grace = Duration::from_millis(250);
        launch
    }
    fn connect(&self, scenario: &str) -> CodexAdapter {
        let began = Instant::now();
        let adapter = CodexAdapter::connect(
            self.launch(scenario),
            "connection",
            1,
            100,
            &CancellationToken::new(),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{scenario}: connect failed after {:?}: {error:?}; methods={:?}",
                began.elapsed(),
                fs::read_to_string(self.home.join(METHODS_FILE)).ok()
            )
        });
        assert!(adapter.metadata().synthetic);
        assert_eq!(adapter.metadata().identity_assurance(), "synthetic");
        assert_eq!(
            adapter.metadata().descriptor.as_json()["runtimeVersion"],
            SYNTHETIC
        );
        adapter
    }
    fn methods(&self) -> Vec<String> {
        fs::read_to_string(self.home.join(METHODS_FILE))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    fn reset_routing(&self) {
        for (file, value) in [
            ("route-account", "synthetic-workspace-a"),
            ("route-backend", "https://chatgpt.com/backend-api/"),
            ("route-policy", "NO_CONSTRAINT"),
        ] {
            fs::write(self.home.join(file), value).unwrap();
        }
    }
    fn dispatch_events(&self) -> usize {
        fs::read_dir(&self.home)
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("dispatch-event-")
            })
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Retained owned synthetic fixture: {}", self.root.display());
            return;
        }
        // Resolve the exact owned target before any recursive removal on Windows.
        let root = self.root.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert!(root.starts_with(temp));
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchestrix-runtime-0164-")
        );
        let _ = fs::remove_dir_all(root);
    }
}
fn execution(attempt: &str) -> AttemptExecution {
    AttemptExecution {
        request: normalize_request(&json!({
            "version":{"major":1,"minor":1},"connectionId":"connection",
            "connectionRevision":1,"sessionId":"session","attemptId":attempt,
            "turnId":null,"billingMode":"subscription-only","requested":{
                "modelPolicy":"runtime-default","reasoning":"runtime-default",
                "sandbox":"workspace-write","network":"disabled","approval":"never"
            }
        }))
        .unwrap(),
        prompt: "fixture-edit".into(),
        base_instructions: "Only edit the authorized fixture.".into(),
        developer_instructions: "No network or delegation.".into(),
        resume: None,
    }
}
fn snapshot(report: &RuntimeReport) -> Value {
    RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot()
}
fn assert_closed(report: &RuntimeReport) {
    assert!(report.synthetic);
    assert_eq!(report.tree.as_ref().unwrap().active_processes, 0);
    assert!(report.streams.stdin_closed);
    assert!(report.streams.stdout_closed);
    assert!(report.streams.stderr_closed);
    assert_eq!(
        snapshot(report)["process"]["tree"]["termination"],
        "confirmed"
    );
}
fn assert_read_state_private(events: &[Value], report: &RuntimeReport) {
    let normalized = json!({
        "events":events,
        "observer":report.observer_checkpoint,
        "adapterDiagnostic":report.adapter_diagnostic
    })
    .to_string();
    for private in [
        "thread/readState/changed",
        "do-not-persist-read-state",
        "do-not-adopt-historical-turn",
        "do-not-adopt-foreign-thread",
    ] {
        assert!(
            !normalized.contains(private),
            "private native metadata retained: {private}"
        );
    }
}
fn expected_execution_methods(thread_method: &str) -> Vec<String> {
    [
        "initialize",
        "account/read",
        "account/rateLimits/read",
        "config/read",
        "model/list",
        thread_method,
        "turn/start",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn exact_native_version_and_structured_profile_gates_do_not_relabel_fixtures() {
    assert!(SUPPORTED_CODEX_VERSIONS.contains(&NATIVE_0164));
    assert_eq!(
        SUPPORTED_STRUCTURED_CODEX_VERSIONS,
        &["0.162.0-alpha.17.2", NATIVE_0164]
    );
    let fixture = Fixture::new();
    // Static profile inspection only. This nonexistent native path is never run.
    let native_path = fixture.root.join("not-executed-native-codex.exe");
    for (version, supported) in [
        ("0.162.0-alpha.2", false),
        ("0.162.0-alpha.17.2", true),
        (NATIVE_0164, true),
        ("0.164.0-alpha.11.2", false),
        ("0.164.0", false),
        ("unknown", false),
    ] {
        let launch = RuntimeLaunch::codex(
            native_path.clone(),
            fixture.home.clone(),
            fixture.cwd.clone(),
            version.into(),
        );
        assert!(!launch.is_synthetic());
        assert_eq!(
            launch.structured_results_supported(),
            supported,
            "{version}"
        );
        if !SUPPORTED_CODEX_VERSIONS.contains(&version) {
            let error =
                CodexAdapter::connect(launch, "connection", 1, 100, &CancellationToken::new())
                    .err()
                    .expect("unknown version must fail before opening a process");
            assert_eq!(error.code(), "version");
            assert_eq!(error.dispatch(), "not-sent");
        }
    }
    let synthetic = fixture.launch("profile-0164-read-state");
    assert!(synthetic.is_synthetic());
    assert!(synthetic.structured_results_supported());
    assert!(!fixture.home.join(METHODS_FILE).exists());
}

#[test]
fn native_0164_effort_mapping_remains_explicit_and_model_values_are_not_universal() {
    for (native, neutral) in [
        ("minimal", "minimal"),
        ("low", "low"),
        ("medium", "medium"),
        ("high", "high"),
        ("xhigh", "maximum"),
    ] {
        assert_eq!(neutral_effort(NATIVE_0164, native), Some(neutral));
        assert_eq!(native_effort(NATIVE_0164, neutral), Some(native));
    }
    for native in ["maximum", "ultra", "none", "", "future-effort"] {
        assert_eq!(neutral_effort(NATIVE_0164, native), None);
    }
    assert_eq!(native_effort(NATIVE_0164, "xhigh"), None);
    for version in ["0.164.0-alpha.11.2", "0.164.0", "other-provider"] {
        assert_eq!(neutral_effort(version, "xhigh"), None);
        assert_eq!(native_effort(version, "maximum"), None);
    }
}

#[test]
fn read_state_with_historical_unread_turn_is_information_without_extra_queries() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("profile-0164-read-state");
    let metadata = adapter.metadata().descriptor.as_json().clone();
    let controls = adapter.metadata().controls.clone();
    let fingerprint = adapter.metadata().observation_fingerprint().to_owned();
    let mut events = Vec::new();
    let report = adapter
        .execute(
            execution("read-state-success"),
            &CancellationToken::new(),
            |event, _| {
                events.push(event.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.fault, None);
    assert_eq!(report.adapter_diagnostic, None);
    assert_eq!(report.dispatch, "sent");
    assert_eq!(snapshot(&report)["terminal"]["outcome"], "completed");
    assert_eq!(snapshot(&report)["terminalConfirmed"], true);
    assert_eq!(*adapter.metadata().descriptor.as_json(), metadata);
    assert_eq!(adapter.metadata().controls, controls);
    assert_eq!(adapter.metadata().observation_fingerprint(), fingerprint);
    assert!(
        adapter
            .metadata()
            .matches_binding(report.binding.as_ref().unwrap())
    );
    for kind in ["attempt.start-observed", "attempt.terminal-observed"] {
        assert_eq!(
            events.iter().filter(|event| event["type"] == kind).count(),
            1,
            "{kind}"
        );
    }
    assert_read_state_private(&events, &report);
    assert_eq!(
        fixture.methods(),
        expected_execution_methods("thread/start")
    );
    assert_eq!(
        fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
        "1"
    );
    assert_eq!(
        fs::read_to_string(fixture.cwd.join("fixture.txt")).unwrap(),
        "after\n"
    );
    assert_closed(&report);
}

#[test]
fn read_state_without_a_real_terminal_never_confirms_completion_or_retries() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("profile-0164-read-state-no-terminal");
    let mut events = Vec::new();
    let report = adapter
        .execute(
            execution("read-state-no-terminal"),
            &CancellationToken::new(),
            |event, _| {
                events.push(event.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.fault.as_ref().unwrap().code(), "crash");
    assert_eq!(report.dispatch, "sent");
    let state = snapshot(&report);
    assert_eq!(state["startObserved"], false);
    assert_eq!(state["terminalConfirmed"], false);
    assert_eq!(state["outcome"], "unknown");
    assert!(events.iter().all(|event| !matches!(
        event["type"].as_str(),
        Some("attempt.start-observed" | "attempt.terminal-observed")
    )));
    assert_eq!(report.tree.as_ref().unwrap().root_exit_code, Some(23));
    assert_eq!(
        fixture.methods(),
        expected_execution_methods("thread/start")
    );
    assert_eq!(
        fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
        "1"
    );
    assert_read_state_private(&events, &report);
    assert_closed(&report);
}

#[test]
fn foreign_alias_and_malformed_read_state_cannot_become_green() {
    for (scenario, code) in [
        ("profile-0164-read-state-foreign", "ownership"),
        ("profile-0164-read-state-alias", "ownership"),
        ("profile-0164-read-state-invalid", "protocol"),
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let mut events = Vec::new();
        let report = adapter
            .execute(
                execution("read-state-invalid"),
                &CancellationToken::new(),
                |event, _| {
                    events.push(event.clone());
                    Ok(())
                },
            )
            .unwrap();
        let fault = report
            .fault
            .as_ref()
            .expect("invalid metadata must fail closed");
        assert_eq!(fault.code(), code, "{scenario}");
        assert_eq!(fault.dispatch(), "unknown", "{scenario}");
        // The RPC acknowledgement proves submission; terminal outcome remains unknown.
        assert_eq!(report.dispatch, "sent", "{scenario}");
        assert_eq!(snapshot(&report)["terminalConfirmed"], false, "{scenario}");
        assert!(
            events
                .iter()
                .all(|event| event["type"] != "attempt.terminal-observed")
        );
        assert_eq!(
            fixture.methods(),
            expected_execution_methods("thread/start"),
            "{scenario}"
        );
        assert_eq!(
            fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1"
        );
        assert_read_state_private(&events, &report);
        assert_closed(&report);
    }
}

#[test]
fn api_bedrock_and_gcp_are_rejected_before_catalog_thread_or_turn() {
    for scenario in [
        "profile-0164-api",
        "profile-0164-bedrock",
        "profile-0164-gcp",
    ] {
        let fixture = Fixture::new();
        let error = CodexAdapter::connect(
            fixture.launch(scenario),
            "connection",
            1,
            100,
            &CancellationToken::new(),
        )
        .err()
        .expect("only the existing ChatGPT subscription modality is supported");
        assert_eq!(error.code(), "authentication", "{scenario}");
        assert_eq!(error.dispatch(), "not-sent", "{scenario}");
        assert_eq!(
            fixture.methods(),
            ["initialize", "account/read"],
            "{scenario}"
        );
        assert!(!fixture.home.join("dispatch-count").exists(), "{scenario}");
        assert_eq!(
            fs::read_to_string(fixture.cwd.join("fixture.txt")).unwrap(),
            "before\n"
        );
    }
}

#[test]
fn incomplete_workspace_routing_is_protocol_failure_before_dispatch() {
    let fixture = Fixture::new();
    let error = CodexAdapter::connect(
        fixture.launch("profile-0164-routing-invalid"),
        "connection",
        1,
        100,
        &CancellationToken::new(),
    )
    .err()
    .expect("an incomplete observed routing object cannot form a binding");
    assert_eq!(error.code(), "protocol");
    assert_eq!(error.dispatch(), "not-sent");
    assert_eq!(fixture.methods(), ["initialize", "account/read"]);
    assert!(!fixture.home.join("dispatch-count").exists());
}

#[test]
fn unchanged_routing_preserves_the_opaque_binding_across_a_real_process_restart() {
    let fixture = Fixture::new();
    fixture.reset_routing();
    let mut first = fixture.connect("profile-0164-routing");
    let baseline_metadata = first.metadata().clone();
    let original = first
        .execute(
            execution("routing-original"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(original.fault, None);
    assert_eq!(snapshot(&original)["terminal"]["outcome"], "completed");
    assert_closed(&original);
    let saved = original.binding.as_ref().unwrap().checkpoint();
    drop(first);
    let mut resumed = fixture.connect("profile-0164-routing");
    let binding = SessionBinding::restore(&saved).unwrap();
    assert!(resumed.metadata().same_identity(&baseline_metadata));
    assert!(resumed.metadata().matches_binding(&binding));
    let mut next = execution("routing-resumed");
    next.resume = Some(binding);
    let report = resumed
        .execute(next, &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    assert_eq!(report.fault, None);
    assert_eq!(snapshot(&report)["terminal"]["outcome"], "completed");
    assert_eq!(
        report.binding.as_ref().unwrap().checkpoint()["nativeThread"],
        saved["nativeThread"]
    );
    let expected = [
        expected_execution_methods("thread/start"),
        expected_execution_methods("thread/resume"),
    ]
    .concat();
    assert_eq!(fixture.methods(), expected);
    // The pre-existing durable ledger spans processes; dispatch-count is
    // intentionally per-process and therefore restarts at one on resume.
    assert_eq!(fixture.dispatch_events(), 2);
    assert_eq!(
        fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
        "1"
    );
    assert_eq!(
        fs::read_to_string(fixture.cwd.join("fixture.txt")).unwrap(),
        "after\n"
    );
    assert_closed(&report);
}

#[test]
fn changed_workspace_backend_or_policy_blocks_old_binding_before_resume_dispatch() {
    let fixture = Fixture::new();
    fixture.reset_routing();
    let mut original = fixture.connect("profile-0164-routing");
    let baseline_metadata = original.metadata().clone();
    let first = original
        .execute(
            execution("routing-source"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(first.fault, None);
    assert_closed(&first);
    let saved = first.binding.as_ref().unwrap().checkpoint();
    drop(original);
    for (file, value) in [
        ("route-account", "synthetic-workspace-b"),
        ("route-backend", "https://chatgpt.com/backend-api/changed"),
        ("route-policy", "us"),
    ] {
        fixture.reset_routing();
        fs::write(fixture.home.join(file), value).unwrap();
        let mut changed = fixture.connect("profile-0164-routing");
        let binding = SessionBinding::restore(&saved).unwrap();
        assert!(
            !changed.metadata().same_identity(&baseline_metadata),
            "{file}"
        );
        assert_ne!(
            changed.metadata().observation_fingerprint(),
            baseline_metadata.observation_fingerprint(),
            "{file}"
        );
        assert!(!changed.metadata().matches_binding(&binding), "{file}");
        let methods_before_dispatch = fixture.methods();
        let mut next = execution("routing-changed");
        next.resume = Some(binding);
        let report = changed
            .execute(next, &CancellationToken::new(), |_, _| Ok(()))
            .unwrap();
        assert_eq!(report.fault.as_ref().unwrap().code(), "ownership", "{file}");
        assert_eq!(report.dispatch, "not-sent", "{file}");
        assert_eq!(snapshot(&report)["terminalConfirmed"], false, "{file}");
        assert_eq!(
            fixture.methods(),
            methods_before_dispatch,
            "no thread/resume or turn/start for {file}"
        );
        assert_eq!(
            fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(fixture.cwd.join("fixture.txt")).unwrap(),
            "after\n"
        );
        assert_eq!(fixture.dispatch_events(), 1);
        assert_closed(&report);
    }
}
