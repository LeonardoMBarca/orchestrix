//! Synthetic protocol runtime; Windows Job, OS processes and stdio are real.
#![cfg(windows)]
use orchestrix_core::runtime::{
    AttemptExecution, CancellationToken, CodexAdapter, RuntimeLaunch, SessionBinding,
    native_effort, neutral_effort,
};
use orchestrix_core::runtime_contract::{RuntimeObserver, normalize_request};
use orchestrix_process::{OwnedProcess, ProcessError, ProcessEvent, ProcessLimits, SpawnSpec};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchestrix-runtime-{}-{}-á space",
            std::process::id(),
            COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let cwd = root.join("worktree");
        let home = root.join("auth-fixture");
        std::fs::create_dir(&cwd).unwrap();
        std::fs::create_dir(&home).unwrap();
        std::fs::write(cwd.join("fixture.txt"), "before\n").unwrap();
        Self { root, cwd, home }
    }
    fn launch(&self, scenario: &str) -> RuntimeLaunch {
        let mut launch = RuntimeLaunch::fixture(
            binary(),
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
        CodexAdapter::connect(
            self.launch(scenario),
            "connection",
            1,
            100,
            &CancellationToken::new(),
        )
        .unwrap()
    }
    fn spawn(&self, scenario: &str, limits: ProcessLimits) -> OwnedProcess {
        OwnedProcess::spawn(SpawnSpec {
            executable: binary(),
            arguments: vec!["--scenario".into(), scenario.into()],
            cwd: self.cwd.clone(),
            environment: environment(),
            limits,
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn environment() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    std::env::vars_os()
        .filter(|(key, _)| {
            ["PATH", "SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "TEMP", "TMP"]
                .iter()
                .any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))
        })
        .collect()
}
fn request(attempt: &str) -> orchestrix_core::runtime_contract::RuntimeRequest {
    normalize_request(&json!({"version":{"major":1,"minor":1},"connectionId":"connection","connectionRevision":1,"sessionId":"session","attemptId":attempt,"turnId":null,"billingMode":"subscription-only","requested":{"modelPolicy":"runtime-default","reasoning":"runtime-default","sandbox":"workspace-write","network":"disabled","approval":"never"}})).unwrap()
}
fn execution(attempt: &str) -> AttemptExecution {
    AttemptExecution {
        request: request(attempt),
        prompt: "fixture-edit".into(),
        base_instructions: "Only edit the authorized fixture.".into(),
        developer_instructions: "No network or delegation.".into(),
        resume: None,
    }
}

#[test]
fn rust_adapter_edits_only_fixture_emits_normalized_observations_and_resume_binding() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("success");
    assert!(adapter.metadata().synthetic);
    assert_eq!(
        adapter.metadata().descriptor.as_json()["models"][0]["availability"],
        "unknown"
    );
    assert_eq!(
        adapter.metadata().descriptor.as_json()["models"][0]["reasoningLevels"],
        json!(["low", "medium", "high", "maximum"])
    );
    let mut events = Vec::new();
    let report = adapter
        .execute(
            execution("attempt-one"),
            &CancellationToken::new(),
            |event, _| {
                events.push(event.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.fault, None);
    assert_eq!(report.dispatch, "sent");
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["outcome"], "completed");
    assert_eq!(snapshot["process"]["tree"]["termination"], "confirmed");
    assert_eq!(
        std::fs::read_to_string(fixture.cwd.join("fixture.txt")).unwrap(),
        "after\n"
    );
    assert!(
        events
            .iter()
            .any(|event| event["type"] == "attempt.start-observed")
    );
    let normalized = serde_json::to_string(&events).unwrap();
    assert!(!normalized.contains("fixture-thread"));
    assert!(!normalized.contains("synthetic-identity"));
    let saved_binding = report.binding.unwrap().checkpoint();
    drop(adapter);
    let mut resumed = fixture.connect("success");
    let mut next = execution("attempt-two");
    next.resume = Some(SessionBinding::restore(&saved_binding).unwrap());
    let report = resumed
        .execute(next, &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    assert_eq!(report.fault, None);
    assert_eq!(
        report.binding.unwrap().checkpoint()["nativeThread"],
        saved_binding["nativeThread"]
    );
}
#[test]
fn strict_model_entitlement_blocks_dispatch_and_subscription_gates_fail_closed() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("success");
    let mut value = request("attempt").as_json().clone();
    value["requested"]["modelPolicy"] = json!("specific");
    value["requested"]["modelId"] =
        adapter.metadata().descriptor.as_json()["models"][0]["id"].clone();
    let mut run = execution("attempt");
    run.request = normalize_request(&value).unwrap();
    let result = adapter
        .execute(run, &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    assert_eq!(result.fault.unwrap().code(), "availability-unknown");
    assert_eq!(result.dispatch, "not-sent");
    assert!(!fixture.home.join("dispatch-count").exists());
    for scenario in [
        "auth-failure",
        "api",
        "quota-failure",
        "delegation-enabled",
        "agents-enabled",
    ] {
        assert!(
            CodexAdapter::connect(
                fixture.launch(scenario),
                "connection",
                1,
                100,
                &CancellationToken::new()
            )
            .is_err()
        );
    }
    let mut adapter = fixture.connect("wrong-sandbox");
    let result = adapter
        .execute(
            execution("other-attempt"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(result.fault.unwrap().code(), "policy");
    assert_eq!(result.dispatch, "not-sent");
}
#[test]
fn cooperative_cancellation_is_distinct_from_remote_and_process_termination() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("hang");
    let token = CancellationToken::new();
    let trigger = token.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        trigger.cancel();
    });
    let report = adapter
        .execute(execution("attempt"), &token, |_, _| Ok(()))
        .unwrap();
    thread.join().unwrap();
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["terminal"]["outcome"], "interrupted");
    assert_eq!(snapshot["interrupt"]["acknowledged"], true);
    assert_eq!(report.tree.unwrap().active_processes, 0);
    assert!(report.streams.stdout_closed);
}
#[test]
fn durable_callback_latency_does_not_discard_an_already_buffered_interrupted_terminal() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("hang");
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let report = adapter
        .execute(execution("slow-durable-observer"), &cancel, |event, _| {
            if event["type"] == "attempt.admission-observed" {
                trigger.cancel();
            }
            if event["type"] == "attempt.start-observed" {
                // Simulate a durable commit taking longer than the unchanged
                // interrupt grace after the runtime already emitted its terminal.
                std::thread::sleep(Duration::from_millis(300));
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(report.fault, None);
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["terminalConfirmed"], true);
    assert_eq!(snapshot["terminal"]["outcome"], "interrupted");
    assert_eq!(snapshot["interrupt"]["acknowledged"], true);
    assert_eq!(report.tree.unwrap().active_processes, 0);
}
#[test]
fn forced_cancel_crash_and_lost_reply_remain_uncertain_without_retry() {
    for scenario in [
        "ignore-interrupt",
        "crash",
        "lost-reply",
        "malformed",
        "contradictory-delta",
        "contradictory-thread",
        "contradictory-thread-start",
        "changed-global-auth",
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let token = CancellationToken::new();
        let trigger = token.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            trigger.cancel();
        });
        let report = adapter
            .execute(execution("attempt"), &token, |_, _| Ok(()))
            .unwrap();
        thread.join().unwrap();
        assert!(report.fault.is_some(), "{scenario}");
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["terminalConfirmed"], false, "{scenario}");
        if scenario.starts_with("contradictory-") {
            assert_eq!(report.fault.as_ref().unwrap().code(), "ownership");
        }
        if scenario == "changed-global-auth" {
            assert_eq!(report.fault.as_ref().unwrap().code(), "authentication");
        }
        if scenario == "lost-reply" {
            assert_eq!(report.dispatch, "sent"); // Pipe write confirmed; remote admission remains unknown.
            assert!(
                snapshot["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["type"] == "runtime.error"
                        && event["error"]["dispatch"] == "unknown")
            );
        }
        assert_eq!(report.tree.unwrap().active_processes, 0, "{scenario}");
        assert_eq!(
            std::fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1"
        );
        assert!(
            adapter
                .execute(
                    execution("attempt-retry"),
                    &CancellationToken::new(),
                    |_, _| Ok(())
                )
                .is_err()
        );
    }
}
#[test]
fn global_metadata_does_not_impersonate_turn_evidence_and_native_rate_limit_is_terminal() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("global-notifications");
    let report = adapter
        .execute(
            execution("global-update"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.fault, None);
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["terminalConfirmed"], true);
    assert_eq!(snapshot["terminal"]["outcome"], "completed");
    let mut adapter = fixture.connect("rate-limit");
    let report = adapter
        .execute(
            execution("quota-boundary"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.fault, None);
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["terminalConfirmed"], true);
    assert_eq!(snapshot["terminal"]["outcome"], "failed");
    assert_eq!(snapshot["terminal"]["error"]["code"], "rate-limit");
    assert_eq!(report.tree.unwrap().active_processes, 0);
}
#[test]
fn nullable_or_matching_metadata_scope_is_not_attempt_execution_evidence() {
    for scenario in [
        "warning-null",
        "warning-absent",
        "warning-thread",
        "mcp-metadata-null",
        "mcp-metadata-thread",
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let mut events = Vec::new();
        let report = adapter
            .execute(
                execution("warning-scope"),
                &CancellationToken::new(),
                |event, _| {
                    events.push(event.clone());
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(report.fault, None, "{scenario}");
        assert_eq!(report.adapter_diagnostic, None, "{scenario}");
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["terminalConfirmed"], true, "{scenario}");
        assert_eq!(snapshot["terminal"]["outcome"], "completed", "{scenario}");
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "attempt.start-observed")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "attempt.terminal-observed")
                .count(),
            1
        );
        let normalized = serde_json::to_string(&events).unwrap();
        assert!(!normalized.contains("do-not-persist-provider-message"));
        assert!(!normalized.contains("do-not-persist-server-name"));
        assert!(!normalized.contains("fixture-thread"));
        assert_eq!(report.tree.unwrap().active_processes, 0);
    }
}
#[test]
fn metadata_with_foreign_contradictory_or_invalid_scope_still_fails_closed() {
    for scenario in [
        "warning-foreign",
        "warning-turn",
        "warning-nested-thread",
        "mcp-metadata-foreign",
        "mcp-metadata-turn",
        "mcp-metadata-absent",
        "mcp-metadata-nonobject",
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let report = adapter
            .execute(
                execution("warning-contradiction"),
                &CancellationToken::new(),
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(
            report.fault.as_ref().unwrap().code(),
            "ownership",
            "{scenario}"
        );
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["startObserved"], false);
        assert_eq!(snapshot["terminalConfirmed"], false);
        let diagnostic = report.adapter_diagnostic.unwrap();
        assert_eq!(diagnostic["stage"], "notification");
        assert_eq!(
            diagnostic["reason"],
            if scenario.starts_with("warning-") {
                "warning-scope"
            } else {
                "mcp-metadata-scope"
            }
        );
        assert_eq!(
            diagnostic["methodSha256"],
            orchestrix_core::git_records::content_hash(if scenario.starts_with("warning-") {
                b"warning"
            } else {
                b"mcpServer/startupStatus/updated"
            })
        );
        assert_eq!(
            diagnostic["identityShapes"]["threadId"],
            if scenario.ends_with("foreign") {
                "string"
            } else if scenario.ends_with("absent") || scenario.ends_with("nonobject") {
                "absent"
            } else {
                "null"
            }
        );
        assert_eq!(
            diagnostic["identityShapes"]["turnId"],
            if scenario == "mcp-metadata-turn" {
                "null"
            } else if scenario == "warning-turn" {
                "string"
            } else {
                "absent"
            }
        );
        assert_eq!(
            diagnostic["identityShapes"]["thread.id"],
            if scenario == "warning-nested-thread" {
                "string"
            } else {
                "absent"
            }
        );
        for private in [
            "private-foreign-thread",
            "private-foreign-turn",
            "do-not-persist-provider-message",
            "do-not-persist-server-name",
        ] {
            assert!(!diagnostic.to_string().contains(private));
        }
        assert_eq!(report.tree.unwrap().active_processes, 0);
    }
}
#[test]
fn protocol_diagnostic_contains_only_closed_local_routes_shapes_and_method_hash() {
    for scenario in ["unscoped-private", "malformed"] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let report = adapter
            .execute(
                execution("private-protocol"),
                &CancellationToken::new(),
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(
            report.fault.as_ref().unwrap().code(),
            "protocol",
            "{scenario}"
        );
        assert_eq!(report.dispatch, "sent");
        let diagnostic = report.adapter_diagnostic.unwrap();
        assert_eq!(diagnostic["version"], 1);
        let keys: Vec<_> = diagnostic
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "idShape",
                "identityShapes",
                "methodSha256",
                "paramsShape",
                "reason",
                "stage",
                "version"
            ]
        );
        assert_eq!(diagnostic["idShape"], "absent");
        assert!(diagnostic.to_string().len() <= 1024);
        for private in [
            "private.method.fixture-value",
            "do-not-persist-provider-message",
            "fixture-thread",
            "synthetic-identity",
            "malformed",
        ] {
            assert!(!diagnostic.to_string().contains(private));
        }
        if scenario == "unscoped-private" {
            assert_eq!(diagnostic["stage"], "notification");
            assert_eq!(diagnostic["reason"], "unsupported-global");
            assert_eq!(
                diagnostic["methodSha256"],
                orchestrix_core::git_records::content_hash(b"private.method.fixture-value")
            );
            assert_eq!(diagnostic["paramsShape"], "object");
        } else {
            assert_eq!(diagnostic["stage"], "framing");
            assert_eq!(diagnostic["reason"], "invalid-json");
            assert_eq!(diagnostic["methodSha256"], serde_json::Value::Null);
            assert_eq!(diagnostic["paramsShape"], "absent");
        }
        assert!(
            diagnostic["identityShapes"]
                .as_object()
                .unwrap()
                .values()
                .all(|shape| shape == "absent")
        );
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["terminalConfirmed"], false);
        if scenario == "unscoped-private" {
            assert_eq!(snapshot["startObserved"], false);
        }
        assert_eq!(report.tree.unwrap().active_processes, 0);
    }
}
#[test]
fn remote_control_global_status_is_only_information_and_never_attempt_evidence() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("remote-status-global");
    let descriptor = adapter.metadata().descriptor.as_json().clone();
    let controls = adapter.metadata().controls.clone();
    let mut events = Vec::new();
    let report = adapter
        .execute(
            execution("remote-info"),
            &CancellationToken::new(),
            |event, _| {
                events.push(event.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.fault, None);
    assert_eq!(report.adapter_diagnostic, None);
    assert_eq!(*adapter.metadata().descriptor.as_json(), descriptor);
    assert_eq!(adapter.metadata().controls, controls);
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["terminalConfirmed"], true);
    assert_eq!(snapshot["terminal"]["outcome"], "completed");
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "attempt.start-observed")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "attempt.terminal-observed")
            .count(),
        1
    );
    let normalized = serde_json::to_string(&events).unwrap();
    for private in [
        "remoteControl/status/changed",
        "do-not-persist-server-name",
        "do-not-persist-installation-id",
        "do-not-persist-environment-id",
    ] {
        assert!(!normalized.contains(private));
    }
    assert_eq!(report.tree.unwrap().active_processes, 0);
}
#[test]
fn remote_control_status_with_any_execution_identity_or_invalid_params_fails_closed() {
    for scenario in [
        "remote-status-thread",
        "remote-status-thread-null",
        "remote-status-turn-null",
        "remote-status-nested-thread",
        "remote-status-nested-turn",
        "remote-status-nonobject",
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let report = adapter
            .execute(
                execution("remote-contradiction"),
                &CancellationToken::new(),
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(
            report.fault.as_ref().unwrap().code(),
            "ownership",
            "{scenario}"
        );
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["startObserved"], false);
        assert_eq!(snapshot["terminalConfirmed"], false);
        let diagnostic = report.adapter_diagnostic.unwrap();
        assert_eq!(diagnostic["stage"], "notification");
        assert_eq!(diagnostic["reason"], "global-scope");
        assert_eq!(
            diagnostic["methodSha256"],
            "fc90be8d31a9f1bb24948acd35f025e91f103bcde03fb735388a7169f1262d24"
        );
        assert_eq!(
            diagnostic["paramsShape"],
            if scenario == "remote-status-nonobject" {
                "array"
            } else {
                "object"
            }
        );
        for private in [
            "remoteControl/status/changed",
            "private-foreign-thread",
            "private-foreign-turn",
            "do-not-persist-server-name",
            "do-not-persist-installation-id",
            "do-not-persist-environment-id",
            "do-not-persist-provider-message",
        ] {
            assert!(!diagnostic.to_string().contains(private));
        }
        assert_eq!(report.tree.unwrap().active_processes, 0);
    }
}
#[test]
fn skills_changed_global_is_only_information_without_queries_or_attempt_evidence() {
    for scenario in ["skills-changed-global", "skills-changed-canary"] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let descriptor = adapter.metadata().descriptor.as_json().clone();
        let controls = adapter.metadata().controls.clone();
        let mut events = Vec::new();
        let report = adapter
            .execute(
                execution("skills-info"),
                &CancellationToken::new(),
                |event, _| {
                    events.push(event.clone());
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(report.fault, None, "{scenario}");
        assert_eq!(report.adapter_diagnostic, None);
        assert_eq!(report.dispatch, "sent");
        assert_eq!(*adapter.metadata().descriptor.as_json(), descriptor);
        assert_eq!(adapter.metadata().controls, controls);
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["terminalConfirmed"], true);
        assert_eq!(snapshot["terminal"]["outcome"], "completed");
        for kind in ["attempt.start-observed", "attempt.terminal-observed"] {
            assert_eq!(
                events.iter().filter(|event| event["type"] == kind).count(),
                1,
                "{scenario}: {kind}"
            );
        }
        let normalized = serde_json::to_string(&json!({
            "events":events,"observer":report.observer_checkpoint
        }))
        .unwrap();
        for private in [
            "skills/changed",
            "do-not-persist-skill-path",
            "do-not-persist-skill-content",
            "do-not-adopt-skill-model",
            "do-not-adopt-skill-effort",
        ] {
            assert!(!normalized.contains(private), "{scenario}: {private}");
        }
        let methods =
            std::fs::read_to_string(fixture.home.join("skills-changed-request-methods")).unwrap();
        assert_eq!(
            methods.lines().collect::<Vec<_>>(),
            [
                "initialize",
                "account/read",
                "account/rateLimits/read",
                "config/read",
                "model/list",
                "thread/start",
                "turn/start"
            ]
        );
        assert_eq!(
            std::fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1"
        );
        assert_eq!(report.tree.unwrap().active_processes, 0);
        assert!(report.streams.stdin_closed);
        assert!(report.streams.stdout_closed);
        assert!(report.streams.stderr_closed);
    }
}
#[test]
fn skills_changed_without_real_terminal_does_not_impersonate_completion() {
    let fixture = Fixture::new();
    let mut adapter = fixture.connect("skills-changed-no-terminal");
    let mut events = Vec::new();
    let report = adapter
        .execute(
            execution("skills-no-terminal"),
            &CancellationToken::new(),
            |event, _| {
                events.push(event.clone());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.fault.as_ref().unwrap().code(), "crash");
    assert_eq!(report.dispatch, "sent");
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(snapshot["startObserved"], false);
    assert_eq!(snapshot["terminalConfirmed"], false);
    assert_eq!(snapshot["outcome"], "unknown");
    assert!(events.iter().all(|event| {
        !matches!(
            event["type"].as_str(),
            Some("attempt.start-observed" | "attempt.terminal-observed")
        )
    }));
    let methods =
        std::fs::read_to_string(fixture.home.join("skills-changed-request-methods")).unwrap();
    assert!(!methods.lines().any(|method| method == "skills/list"));
    assert_eq!(
        methods
            .lines()
            .filter(|method| *method == "turn/start")
            .count(),
        1
    );
    let tree = report.tree.unwrap();
    assert_eq!(tree.root_exit_code, Some(23));
    assert_eq!(tree.active_processes, 0);
    assert!(report.streams.stdin_closed);
    assert!(report.streams.stdout_closed);
    assert!(report.streams.stderr_closed);
}
#[test]
fn skills_changed_with_any_execution_identity_or_invalid_params_fails_closed() {
    for (scenario, params_shape, thread_id, nested_thread, turn_id, nested_turn) in [
        (
            "skills-changed-thread",
            "object",
            "string",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-thread-foreign",
            "object",
            "string",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-thread-null",
            "object",
            "null",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-nested-thread",
            "object",
            "absent",
            "string",
            "absent",
            "absent",
        ),
        (
            "skills-changed-nested-thread-foreign",
            "object",
            "absent",
            "string",
            "absent",
            "absent",
        ),
        (
            "skills-changed-nested-thread-null",
            "object",
            "absent",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-turn",
            "object",
            "absent",
            "absent",
            "string",
            "absent",
        ),
        (
            "skills-changed-turn-foreign",
            "object",
            "absent",
            "absent",
            "string",
            "absent",
        ),
        (
            "skills-changed-turn-null",
            "object",
            "absent",
            "absent",
            "null",
            "absent",
        ),
        (
            "skills-changed-nested-turn",
            "object",
            "absent",
            "absent",
            "absent",
            "string",
        ),
        (
            "skills-changed-nested-turn-foreign",
            "object",
            "absent",
            "absent",
            "absent",
            "string",
        ),
        (
            "skills-changed-nested-turn-null",
            "object",
            "absent",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-contradictory-thread",
            "object",
            "string",
            "string",
            "absent",
            "absent",
        ),
        (
            "skills-changed-contradictory-turn",
            "object",
            "absent",
            "absent",
            "string",
            "string",
        ),
        (
            "skills-changed-array",
            "array",
            "absent",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-null",
            "null",
            "absent",
            "absent",
            "absent",
            "absent",
        ),
        (
            "skills-changed-string",
            "string",
            "absent",
            "absent",
            "absent",
            "absent",
        ),
    ] {
        let fixture = Fixture::new();
        let mut adapter = fixture.connect(scenario);
        let report = adapter
            .execute(
                execution("skills-contradiction"),
                &CancellationToken::new(),
                |_, _| Ok(()),
            )
            .unwrap();
        assert_eq!(
            report.fault.as_ref().unwrap().code(),
            "ownership",
            "{scenario}"
        );
        assert_eq!(report.dispatch, "sent");
        let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot();
        assert_eq!(snapshot["startObserved"], false, "{scenario}");
        assert_eq!(snapshot["terminalConfirmed"], false, "{scenario}");
        let diagnostic = report.adapter_diagnostic.unwrap();
        assert_eq!(diagnostic["stage"], "notification");
        assert_eq!(diagnostic["reason"], "global-scope");
        assert_eq!(
            diagnostic["methodSha256"],
            "4781880021f88b798d54dd94cc42c068b5cf66286ad7d4d22dbee38cb418a01e"
        );
        assert_eq!(diagnostic["paramsShape"], params_shape, "{scenario}");
        for (field, expected) in [
            ("threadId", thread_id),
            ("thread.id", nested_thread),
            ("turnId", turn_id),
            ("turn.id", nested_turn),
        ] {
            assert_eq!(
                diagnostic["identityShapes"][field], expected,
                "{scenario}: {field}"
            );
        }
        for private in [
            "skills/changed",
            "private-foreign-thread",
            "private-foreign-turn",
            "do-not-persist-skill-content",
        ] {
            assert!(!diagnostic.to_string().contains(private));
        }
        let methods =
            std::fs::read_to_string(fixture.home.join("skills-changed-request-methods")).unwrap();
        assert!(!methods.lines().any(|method| method == "skills/list"));
        assert_eq!(
            methods
                .lines()
                .filter(|method| *method == "turn/start")
                .count(),
            1
        );
        assert_eq!(report.tree.unwrap().active_processes, 0);
    }
}
#[test]
fn owns_descendant_after_root_exit_and_never_terminates_unrelated_process() {
    struct Unrelated(std::process::Child);
    impl Drop for Unrelated {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let fixture = Fixture::new();
    use std::os::windows::process::CommandExt;
    let mut unrelated = Unrelated(
        std::process::Command::new(binary())
            .creation_flags(0x0800_0000)
            .args(["--scenario", "child-hold"])
            .spawn()
            .unwrap(),
    );
    let mut process = fixture.spawn("parent-exit", ProcessLimits::default());
    let began = Instant::now();
    let mut announced = Vec::new();
    loop {
        if let Some(ProcessEvent::Stdout(bytes)) =
            process.receive(Duration::from_millis(5)).unwrap()
        {
            announced.extend(bytes);
        }
        let state = process.tree().unwrap();
        let child_pid = std::str::from_utf8(&announced)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok());
        // CREATE_NO_WINDOW may have additional owned console descendants. Prove
        // that the actual child announced by our fixture remains in our Job;
        // neither an arbitrary count of one nor the root's exit proves this.
        if state.root_exit_code == Some(0)
            && child_pid.is_some_and(|pid| process.member_ids().unwrap().contains(&pid))
        {
            assert!(state.active_processes > 0);
            assert!(!process.streams().stdout_closed);
            break;
        }
        assert!(
            began.elapsed() < Duration::from_secs(3),
            "state={state:?}; members={:?}",
            process.member_ids()
        );
    }
    let state = process.terminate(Duration::from_secs(3)).unwrap();
    assert!(state.owned);
    assert_eq!(state.active_processes, 0);
    assert_eq!(state.root_exit_code, Some(0));
    assert!(unrelated.0.try_wait().unwrap().is_none());
    assert!(process.wait_streams(Duration::from_secs(1)).stdout_closed);
}

#[test]
fn wrong_account_binding_and_failed_observation_storage_block_green_result() {
    let fixture = Fixture::new();
    let mut original = fixture.connect("success");
    let report = original
        .execute(execution("original"), &CancellationToken::new(), |_, _| {
            Ok(())
        })
        .unwrap();
    let binding = report.binding.unwrap();
    drop(original);
    let mut other = fixture.connect("wrong-account");
    let mut resumed = execution("different");
    resumed.resume = Some(binding);
    let report = other
        .execute(resumed, &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    assert_eq!(report.fault.unwrap().code(), "ownership");
    assert_eq!(report.dispatch, "not-sent");
    let mut adapter = fixture.connect("success");
    let report = adapter
        .execute(
            execution("storage-failed"),
            &CancellationToken::new(),
            |_, _| {
                // A real host maps a failed SQLite commit into a fixed boundary error.
                Err(orchestrix_core::runtime::RuntimeError::observation_failed())
            },
        )
        .unwrap();
    assert!(report.fault.is_some());
    assert_eq!(report.dispatch, "not-sent");
    assert_eq!(report.tree.unwrap().active_processes, 0);
}
#[test]
fn bounded_stdin_output_and_watchdog_do_not_block_controller() {
    let fixture = Fixture::new();
    let mut process = fixture.spawn(
        "blocked-stdin",
        ProcessLimits {
            max_write_bytes: 1_048_576,
            ..ProcessLimits::default()
        },
    );
    assert_eq!(
        process.send(&vec![b'x'; 1_048_576], Duration::from_millis(50)),
        Err(ProcessError::Deadline)
    );
    assert_eq!(
        process
            .terminate(Duration::from_secs(3))
            .unwrap()
            .active_processes,
        0
    );
    let mut process = fixture.spawn(
        "oversized",
        ProcessLimits {
            max_output_bytes: 16_384,
            ..ProcessLimits::default()
        },
    );
    let began = Instant::now();
    loop {
        match process.receive(Duration::from_millis(5)) {
            Err(ProcessError::Budget) => break,
            Ok(_) => {}
            other => panic!("{other:?}"),
        }
        assert!(began.elapsed() < Duration::from_secs(3));
    }
    assert_eq!(
        process
            .terminate(Duration::from_secs(3))
            .unwrap()
            .active_processes,
        0
    );
    let process = fixture.spawn(
        "child-hold",
        ProcessLimits {
            lifetime: Duration::from_millis(100),
            ..ProcessLimits::default()
        },
    );
    let began = Instant::now();
    while process.health().is_ok() {
        std::thread::sleep(Duration::from_millis(5));
        assert!(began.elapsed() < Duration::from_secs(3));
    }
    assert_eq!(process.health(), Err(ProcessError::Deadline));
}
#[test]
fn native_effort_mapping_is_versioned_and_never_universal() {
    assert_eq!(neutral_effort("0.162.0-alpha.2", "xhigh"), Some("maximum"));
    assert_eq!(
        native_effort("0.162.0-alpha.17.2", "maximum"),
        Some("xhigh")
    );
    assert_eq!(neutral_effort("other-provider", "xhigh"), None);
    assert_eq!(neutral_effort("0.162.0-alpha.2", "maximum"), None);
    assert_eq!(native_effort("future-version", "maximum"), None);
}
#[test]
fn owner_process_crash_closes_private_job_without_rust_drop() {
    let fixture = Fixture::new();
    let mut process = fixture.spawn("owner-crash", ProcessLimits::default());
    let began = Instant::now();
    loop {
        let _ = process.receive(Duration::from_millis(5));
        let state = process.tree().unwrap();
        if state.root_exit_code == Some(42) && state.active_processes == 0 {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(3), "{state:?}");
    }
    process.close_input();
    assert!(process.wait_streams(Duration::from_secs(1)).stdout_closed);
}
#[test]
fn explicit_environment_does_not_inherit_keys_or_endpoint_overrides() {
    let fixture = Fixture::new();
    let mut process = fixture.spawn("env", ProcessLimits::default());
    process.close_input();
    let mut bytes = Vec::new();
    let began = Instant::now();
    loop {
        if let Some(ProcessEvent::Stdout(chunk)) =
            process.receive(Duration::from_millis(5)).unwrap()
        {
            bytes.extend(chunk);
        }
        if process.streams().stdout_closed && process.tree().unwrap().root_exit_code.is_some() {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(3));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["openai"], false);
    assert_eq!(value["route"], false);
}

#[test]
fn authorized_metadata_service_uses_scoped_cache_without_new_process_or_inference() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{CodexMetadataService, ManualRefreshClock, MetadataRead};
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    assert!(
        service
            .read(
                fixture.launch("success"),
                "connection",
                1,
                false,
                &CancellationToken::new()
            )
            .is_err()
    );
    assert!(!fixture.home.join("metadata-reads").exists());
    let MetadataRead::Ready {
        metadata: first,
        cached: false,
        complete: true,
    } = service
        .read(
            fixture.launch("success"),
            "connection",
            1,
            true,
            &CancellationToken::new(),
        )
        .unwrap()
    else {
        panic!()
    };
    let MetadataRead::Ready {
        metadata: cached,
        cached: true,
        complete: true,
    } = service
        .read(
            fixture.launch("success"),
            "connection",
            1,
            true,
            &CancellationToken::new(),
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(first.same_identity(&cached));
    assert_eq!(
        std::fs::read_to_string(fixture.home.join("metadata-reads")).unwrap(),
        "1"
    );
    assert!(!fixture.home.join("dispatch-count").exists());
    let other = fixture.root.join("other-worktree");
    std::fs::create_dir(&other).unwrap();
    assert!(matches!(
        service
            .read(
                fixture.launch("success").with_cwd(other),
                "connection",
                1,
                true,
                &CancellationToken::new()
            )
            .unwrap(),
        MetadataRead::Ready { cached: false, .. }
    ));
    service.invalidate_connection("connection", 2);
    assert!(
        service
            .read(
                fixture.launch("success"),
                "connection",
                1,
                true,
                &CancellationToken::new()
            )
            .is_err()
    );
    assert!(matches!(
        service
            .read(
                fixture.launch("success"),
                "connection",
                2,
                true,
                &CancellationToken::new()
            )
            .unwrap(),
        MetadataRead::Ready { cached: false, .. }
    ));
}
#[test]
fn capacity_expiration_between_thread_configuration_and_dispatch_sends_no_turn() {
    let fixture = Fixture::new();
    std::fs::write(fixture.home.join("synthetic-capacity-response.json"),json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":20,"resetsAt":1}}}).to_string()).unwrap();
    let mut adapter = fixture.connect("success");
    let mut delayed = false;
    let report = adapter
        .execute(
            execution("expiry-before-dispatch"),
            &CancellationToken::new(),
            |event, _| {
                if event["type"] == "configuration.observation-recorded" {
                    delayed = true;
                    std::thread::sleep(Duration::from_millis(1000));
                }
                Ok(())
            },
        )
        .unwrap();
    assert!(delayed);
    assert_eq!(report.dispatch, "not-sent");
    assert_eq!(report.fault.unwrap().code(), "availability-unknown");
    assert!(!fixture.home.join("dispatch-count").exists());
}
#[test]
fn capacity_metadata_negative_is_durable_input_not_an_executable_adapter() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{CodexMetadataService, ManualRefreshClock, MetadataRead};
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    let response = fixture.home.join("synthetic-capacity-response.json");
    let read = |launch| {
        service
            .read(launch, "connection", 1, true, &CancellationToken::new())
            .unwrap()
    };
    std::fs::write(&response,json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":0,"resetsAt":1}}}).to_string()).unwrap();
    let MetadataRead::Ready {
        metadata: first, ..
    } = read(fixture.launch("success"))
    else {
        panic!("positive read")
    };
    assert!(first.usage_available_at(999, 60_100));
    assert!(!first.usage_available_at(1_000, 60_100));
    let initial_reads = metadata_read_count(&fixture);
    let MetadataRead::Ready {
        metadata: cached,
        cached: true,
        ..
    } = read(fixture.launch("success"))
    else {
        panic!("cached")
    };
    assert_eq!(cached.capacity, first.capacity);
    assert_eq!(metadata_read_count(&fixture), initial_reads);
    std::fs::write(&response,json!({"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":100,"resetsAt":1}}}).to_string()).unwrap();
    clock.set(1_001);
    let MetadataRead::Ready {
        metadata: negative,
        complete: false,
        cached: false,
    } = read(fixture.launch("success"))
    else {
        panic!("negative observation")
    };
    assert_eq!(negative.capacity.usage_allowed(), Some(false));
    assert_eq!(
        negative.descriptor.as_json()["connection"]["authentication"],
        "authenticated"
    );
    assert_eq!(
        negative.descriptor.as_json()["connection"]["availability"],
        "unavailable"
    );
    assert_eq!(
        negative.descriptor.as_json()["connection"]["billing"],
        "unknown"
    );
    assert!(negative.controls.is_empty());
    assert!(!negative.usage_available_at(1_002, 61_001));
    let blocked = CodexAdapter::connect(
        fixture.launch("success"),
        "connection",
        1,
        1_002,
        &CancellationToken::new(),
    )
    .err()
    .unwrap();
    assert_eq!(blocked.code(), "unavailable");
    assert_eq!(blocked.dispatch(), "not-sent");
    assert!(!fixture.home.join("dispatch-count").exists());
    clock.set(3_000);
    std::fs::write(&response,json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":"invalid-optional"}}}).to_string()).unwrap();
    let MetadataRead::Ready {
        metadata: eligible, ..
    } = read(fixture.launch("success"))
    else {
        panic!("new positive")
    };
    assert_eq!(eligible.capacity.completeness, "partial");
    assert!(eligible.usage_available_at(3_000, 63_000));
    let mut adapter = CodexAdapter::connect(
        fixture.launch("success"),
        "connection",
        1,
        3_000,
        &CancellationToken::new(),
    )
    .unwrap();
    let report = adapter
        .execute(
            execution("capacity-eligible"),
            &CancellationToken::new(),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(report.dispatch, "sent");
    assert!(report.fault.is_none());
    assert_eq!(
        RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
            .unwrap()
            .snapshot()["outcome"],
        "completed"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
        "1"
    );
}
#[test]
fn real_metadata_reads_coalesce_cancel_and_backoff_with_controlled_clock() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{CodexMetadataService, ManualRefreshClock, MetadataRead};
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    let background = service.clone();
    let launch = fixture.launch("metadata-pause");
    let thread = std::thread::spawn(move || {
        background.read(launch, "connection", 1, true, &CancellationToken::new())
    });
    let began = Instant::now();
    while !fixture.home.join("metadata-reads").exists() {
        assert!(began.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(
        service
            .read(
                fixture.launch("metadata-pause"),
                "connection",
                1,
                true,
                &CancellationToken::new()
            )
            .unwrap(),
        MetadataRead::Coalesced
    ));
    service.invalidate_connection("connection", 2);
    assert!(thread.join().unwrap().is_err());
    let short = CodexMetadataService::new(
        clock,
        RefreshBounds {
            deadline_ms: 100,
            ..RefreshBounds::default()
        },
    )
    .unwrap();
    assert!(
        short
            .read(
                fixture.launch("metadata-hang"),
                "connection",
                1,
                true,
                &CancellationToken::new()
            )
            .is_err()
    );
    assert!(matches!(
        short
            .read(
                fixture.launch("metadata-hang"),
                "connection",
                1,
                true,
                &CancellationToken::new()
            )
            .unwrap(),
        MetadataRead::Backoff { .. }
    ));
    assert!(!fixture.home.join("dispatch-count").exists());
}

fn schedule_registration(
    fixture: &Fixture,
    scenario: &str,
    revision: u64,
) -> orchestrix_core::runtime::MetadataRegistration {
    orchestrix_core::runtime::MetadataRegistration {
        connection_id: "connection".into(),
        revision,
        launch: fixture.launch(scenario),
        authorized: true,
        enabled: true,
        interval_ms: 1_000,
    }
}
fn metadata_read_count(fixture: &Fixture) -> u64 {
    std::fs::read_to_string(fixture.home.join("metadata-reads"))
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}
fn wait_metadata_count(fixture: &Fixture, count: u64) {
    let began = Instant::now();
    while metadata_read_count(fixture) < count {
        assert!(began.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn host_metadata_schedule_is_authorized_bounded_periodic_and_preserves_observation_age() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{
        CodexMetadataService, ManualRefreshClock, MetadataRead, MetadataRefreshCoordinator,
        MetadataRefreshTrigger, MetadataScheduleBounds, ScheduledMetadataOutcome,
    };
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    let schedule = MetadataRefreshCoordinator::new(
        service,
        clock.clone(),
        MetadataScheduleBounds {
            max_connections: 1,
            ..MetadataScheduleBounds::default()
        },
    )
    .unwrap();
    let mut registration = schedule_registration(&fixture, "success", 1);
    registration.authorized = false;
    schedule.register(registration).unwrap();
    assert!(schedule.tick(&CancellationToken::new()).unwrap().is_empty());
    assert_eq!(metadata_read_count(&fixture), 0); // Registration has no provider IO.
    let mut extra = schedule_registration(&fixture, "success", 1);
    extra.connection_id = "second-connection".into();
    assert!(schedule.register(extra).is_err());
    assert!(!schedule.trigger("connection", 2, MetadataRefreshTrigger::Reconnected));
    schedule
        .register(schedule_registration(&fixture, "success", 1))
        .unwrap();
    let first = schedule.tick(&CancellationToken::new()).unwrap();
    assert!(matches!(
        &first[0].outcome,
        ScheduledMetadataOutcome::Read(MetadataRead::Ready { cached: false, .. })
    ));
    assert!(
        schedule
            .register(schedule_registration(&fixture, "metadata-pause", 1))
            .is_err()
    );
    clock.set(1_100);
    let cached = schedule.tick(&CancellationToken::new()).unwrap();
    let ScheduledMetadataOutcome::Read(MetadataRead::Ready {
        metadata,
        cached: true,
        ..
    }) = &cached[0].outcome
    else {
        panic!()
    };
    assert_eq!(metadata.observed_at_ms, 100);
    assert_eq!(metadata_read_count(&fixture), 1);
    clock.set(60_100);
    let refreshed = schedule.tick(&CancellationToken::new()).unwrap();
    assert!(matches!(
        &refreshed[0].outcome,
        ScheduledMetadataOutcome::Read(MetadataRead::Ready { cached: false, .. })
    ));
    assert_eq!(metadata_read_count(&fixture), 2);
    clock.set(99);
    assert!(schedule.tick(&CancellationToken::new()).is_err());
    assert!(!fixture.home.join("dispatch-count").exists());
}
#[test]
fn metadata_resource_error_events_cannot_reset_backoff() {
    use orchestrix_core::runtime::refresh::{MetadataResource, RefreshBounds};
    use orchestrix_core::runtime::{
        CodexMetadataService, ManualRefreshClock, MetadataRead, MetadataRefreshCoordinator,
        MetadataRefreshTrigger, MetadataScheduleBounds, ScheduledMetadataOutcome,
    };
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(
        clock.clone(),
        RefreshBounds {
            initial_backoff_ms: 5_000,
            max_backoff_ms: 5_000,
            ..RefreshBounds::default()
        },
    )
    .unwrap();
    let schedule =
        MetadataRefreshCoordinator::new(service, clock.clone(), MetadataScheduleBounds::default())
            .unwrap();
    schedule
        .register(schedule_registration(&fixture, "quota-failure", 1))
        .unwrap();
    // This case proves backoff after a known quota error. A short wall-clock
    // startup timeout could fail before initialize increments metadata-reads;
    // dedicated timeout tests cover that independent, valid outcome.
    assert!(matches!(
        &schedule.tick(&CancellationToken::new()).unwrap()[0].outcome,
        ScheduledMetadataOutcome::Read(MetadataRead::Ready {metadata,complete:false,cached:false}) if metadata.capacity.usage_allowed()==Some(false)
    ));
    for now in [1_100, 2_100] {
        clock.set(now);
        assert!(schedule.trigger(
            "connection",
            1,
            MetadataRefreshTrigger::ResourceError(MetadataResource::Quota)
        ));
        let update = schedule.tick(&CancellationToken::new()).unwrap();
        assert!(matches!(
            &update[0].outcome,
            ScheduledMetadataOutcome::Read(MetadataRead::Backoff { until_ms: 5_100 })
        ));
        assert_eq!(metadata_read_count(&fixture), 1);
    }
    clock.set(5_100);
    assert!(matches!(
        &schedule.tick(&CancellationToken::new()).unwrap()[0].outcome,
        ScheduledMetadataOutcome::Read(MetadataRead::Ready {metadata,complete:false,cached:false}) if metadata.capacity.usage_allowed()==Some(false)
    ));
    assert_eq!(metadata_read_count(&fixture), 2);
}
#[test]
fn cloned_metadata_ticks_coalesce_and_revised_or_removed_reads_never_publish_late_data() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{
        CodexMetadataService, ManualRefreshClock, MetadataRefreshCoordinator,
        MetadataScheduleBounds, ScheduledMetadataOutcome,
    };
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    let schedule =
        MetadataRefreshCoordinator::new(service, clock.clone(), MetadataScheduleBounds::default())
            .unwrap();
    schedule
        .register(schedule_registration(&fixture, "metadata-pause", 1))
        .unwrap();
    let background = schedule.clone();
    let first = std::thread::spawn(move || background.tick(&CancellationToken::new()).unwrap());
    wait_metadata_count(&fixture, 1);
    assert!(schedule.tick(&CancellationToken::new()).unwrap().is_empty());
    schedule
        .register(schedule_registration(&fixture, "metadata-pause", 2))
        .unwrap();
    assert!(matches!(
        &first.join().unwrap()[0].outcome,
        ScheduledMetadataOutcome::Discarded
    ));
    clock.set(1_100);
    let current = schedule.tick(&CancellationToken::new()).unwrap();
    assert_eq!(current[0].revision, 2);
    assert!(matches!(
        &current[0].outcome,
        ScheduledMetadataOutcome::Read(_)
    ));
    assert_eq!(metadata_read_count(&fixture), 2);
    schedule
        .register(schedule_registration(&fixture, "metadata-pause", 3))
        .unwrap();
    clock.set(2_100);
    let background = schedule.clone();
    let third = std::thread::spawn(move || background.tick(&CancellationToken::new()).unwrap());
    wait_metadata_count(&fixture, 3);
    assert!(!schedule.remove("connection", 2));
    assert!(schedule.remove("connection", 3));
    assert!(matches!(
        &third.join().unwrap()[0].outcome,
        ScheduledMetadataOutcome::Discarded
    ));
    clock.set(3_100);
    assert!(schedule.tick(&CancellationToken::new()).unwrap().is_empty());
    assert!(!fixture.home.join("dispatch-count").exists());
}
#[test]
fn failed_auth_refresh_requires_host_event_and_revocation_cancels_pending_read() {
    use orchestrix_core::runtime::refresh::RefreshBounds;
    use orchestrix_core::runtime::{
        CodexMetadataService, ManualRefreshClock, MetadataRefreshCoordinator,
        MetadataRefreshTrigger, MetadataScheduleBounds, ScheduledMetadataOutcome,
    };
    let fixture = Fixture::new();
    let clock = std::sync::Arc::new(ManualRefreshClock::new(100));
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    let schedule =
        MetadataRefreshCoordinator::new(service, clock.clone(), MetadataScheduleBounds::default())
            .unwrap();
    schedule
        .register(schedule_registration(&fixture, "auth-failure", 1))
        .unwrap();
    assert!(
        matches!(&schedule.tick(&CancellationToken::new()).unwrap()[0].outcome, ScheduledMetadataOutcome::Failed(error) if error.code() == "authentication")
    );
    clock.set(60_100);
    assert!(schedule.tick(&CancellationToken::new()).unwrap().is_empty());
    assert_eq!(metadata_read_count(&fixture), 1);
    assert!(schedule.trigger("connection", 1, MetadataRefreshTrigger::Reconnected));
    assert!(matches!(
        &schedule.tick(&CancellationToken::new()).unwrap()[0].outcome,
        ScheduledMetadataOutcome::Failed(_)
    ));
    assert_eq!(metadata_read_count(&fixture), 2);
    clock.set(61_100);
    schedule
        .register(schedule_registration(&fixture, "metadata-pause", 2))
        .unwrap();
    let background = schedule.clone();
    let pending = std::thread::spawn(move || background.tick(&CancellationToken::new()).unwrap());
    wait_metadata_count(&fixture, 3);
    let mut revoked = schedule_registration(&fixture, "metadata-pause", 2);
    revoked.authorized = false;
    schedule.register(revoked).unwrap();
    assert!(matches!(
        &pending.join().unwrap()[0].outcome,
        ScheduledMetadataOutcome::Discarded
    ));
    clock.set(62_100);
    assert!(schedule.tick(&CancellationToken::new()).unwrap().is_empty());
}

#[test]
#[ignore = "Explicit read-only Codex metadata opt-in; no inference or login"]
fn codex_metadata_read_only_opt_in() {
    use orchestrix_core::runtime::{RefreshClock, SystemRefreshClock};
    assert_eq!(
        std::env::var("ORCHESTRIX_ALLOW_METADATA").as_deref(),
        Ok("yes")
    );
    let path = |name| PathBuf::from(std::env::var_os(name).expect("explicit metadata path"));
    let launch = RuntimeLaunch::codex(
        path("ORCHESTRIX_METADATA_CODEX_EXE"),
        path("ORCHESTRIX_METADATA_CODEX_HOME"),
        path("ORCHESTRIX_METADATA_CWD"),
        "0.162.0-alpha.17.2".into(),
    );
    let mut adapter = CodexAdapter::connect(
        launch,
        "metadata-preflight",
        1,
        SystemRefreshClock.now_ms(),
        &CancellationToken::new(),
    )
    .expect("bounded metadata gates");
    let descriptor = adapter.metadata().descriptor.as_json();
    let summary = json!({"inference":false,"login":false,"synthetic":adapter.metadata().synthetic,"runtimeVersion":descriptor["runtimeVersion"],"authentication":descriptor["connection"]["authentication"],"billing":descriptor["connection"]["billing"],"availability":descriptor["connection"]["availability"],"catalog":descriptor["catalog"],"modelCount":descriptor["models"].as_array().unwrap().len(),"controls":adapter.metadata().controls});
    let tree = adapter.shutdown().unwrap();
    let summary = json!({"metadata":summary,"tree":{"owned":tree.owned,"rootExitCode":tree.root_exit_code,"activeProcesses":tree.active_processes}});
    if let Some(report) = std::env::var_os("ORCHESTRIX_METADATA_REPORT") {
        std::fs::write(report, serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
    }
    println!("{summary}");
    assert!(tree.owned);
    assert_eq!(tree.active_processes, 0);
}
