//! Connection API acceptance: real local daemon/SQLite/IPC with metadata-only synthetic runtimes.
#![cfg(windows)]
use orchestrix_core::{
    connections::{ConnectionChange, ConnectionRequest, runtime_executable_sha256},
    daemon::{API_VERSION, Client, Operation, Request, RuntimeChoice},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct Fixture {
    root: PathBuf,
    home: PathBuf,
    cwd: PathBuf,
    database: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-connections-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&root).unwrap();
        let home = root.join("runtime-home");
        let cwd = root.join("metadata-directory");
        std::fs::create_dir(&home).unwrap();
        std::fs::create_dir(&cwd).unwrap();
        Self {
            database: root.join("state.sqlite"),
            root,
            home,
            cwd,
        }
    }
    fn start(&self) -> Daemon {
        let mut command = Command::new(env!("CARGO_BIN_EXE_orchestrix"));
        command
            .args(["daemon", "--database"])
            .arg(&self.database)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(self.root.join("daemon.log")).unwrap());
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
        let mut d = Daemon {
            child: command.spawn().unwrap(),
            client: Client::new(&self.database).unwrap(),
        };
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            if d.raw(Operation::Snapshot).is_ok() {
                return d;
            }
            assert!(d.child.try_wait().unwrap().is_none(), "daemon exited");
            assert!(Instant::now() < until, "empty daemon readiness deadline");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    fn link(&self, id: &str, scenario: &str, allow_metadata_read: bool) -> ConnectionRequest {
        ConnectionRequest::Link {
            command_id: format!("link-{id}"),
            id: id.into(),
            expected_revision: None,
            name: "My runtime".into(),
            runtime: RuntimeChoice::Synthetic {
                executable: binary(),
                home: self.home.clone(),
                scenario: scenario.into(),
            },
            metadata_directory: self.cwd.clone(),
            executable_sha256: runtime_executable_sha256(&binary()).unwrap(),
            allow_metadata_read,
        }
    }
    fn reads(&self) -> u64 {
        std::fs::read_to_string(self.home.join("metadata-reads"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}
struct Daemon {
    child: Child,
    client: Client,
}
impl Daemon {
    fn raw(&self, operation: Operation) -> Result<Value, String> {
        let r = self.client.request(&Request {
            api_version: API_VERSION,
            operation,
        })?;
        if r.ok {
            Ok(r.result)
        } else {
            Err(r.error.unwrap_or_default())
        }
    }
    fn request(&self, r: ConnectionRequest) -> Value {
        self.raw(Operation::Connections {
            request: Box::new(r),
        })
        .unwrap()
    }
    fn diagnostic(&self, id: &str, run: Option<String>) -> Value {
        self.request(ConnectionRequest::Diagnostics {
            id: id.into(),
            diagnostic_id: run,
        })
    }
    fn wait(&self, id: &str, run: Option<String>) -> Value {
        self.wait_bounded(id, run, Duration::from_secs(20))
    }
    fn wait_bounded(&self, id: &str, run: Option<String>, bound: Duration) -> Value {
        let until = Instant::now() + bound;
        loop {
            let v = self.diagnostic(id, run.clone());
            if v["latest"]["state"]
                .as_str()
                .is_some_and(|s| s != "running")
            {
                return v;
            }
            assert!(Instant::now() < until, "metadata-only deadline");
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    fn stop(&mut self) {
        let id = format!(
            "stop-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        self.raw(Operation::Shutdown { command_id: id }).unwrap();
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(exit) = self.child.try_wait().unwrap() {
                assert!(exit.success());
                return;
            }
            assert!(Instant::now() < until, "bounded own shutdown");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.raw(Operation::Shutdown {
                command_id: format!(
                    "cleanup-{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ),
            });
        }
    }
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn diagnose(id: &str, key: &str, revision: u64) -> ConnectionRequest {
    ConnectionRequest::Diagnose {
        command_id: key.into(),
        id: id.into(),
        expected_revision: revision,
        allow_metadata_read: true,
    }
}
#[test]
fn two_clients_share_one_flight_and_cache_without_creating_threads_or_turns() {
    let f = Fixture::new();
    let mut d = f.start();
    let linked = d.request(f.link("codex", "metadata-pause", false));
    assert_eq!(linked["authenticated"], false);
    assert_eq!(f.reads(), 0);
    assert_eq!(
        d.raw(Operation::Connections {
            request: Box::new(f.link("duplicate-store", "connection-metadata", false))
        })
        .unwrap_err(),
        "connection-auth-store-already-linked"
    );
    assert_eq!(
        d.request(ConnectionRequest::List)["connections"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let second = Client::new(&f.database).unwrap();
    let first = d.request(diagnose("codex", "diagnose-first", 1));
    let coalesced_operation = Operation::Connections {
        request: Box::new(diagnose("codex", "diagnose-second-window", 1)),
    };
    let coalesced_fingerprint = orchestrix_core::git_records::content_hash(
        &serde_json::to_vec(&coalesced_operation).unwrap(),
    );
    let r = second
        .request(&Request {
            api_version: API_VERSION,
            operation: coalesced_operation.clone(),
        })
        .unwrap();
    assert!(r.ok);
    assert_eq!(r.result["diagnosticId"], first["diagnosticId"]);
    assert_eq!(r.result["state"], "coalesced");
    let observed = d.wait("codex", first["diagnosticId"].as_str().map(String::from));
    assert_eq!(observed["latest"]["state"], "complete");
    assert_eq!(observed["catalogLabels"][0]["label"], "fixture-codex");
    assert_eq!(f.reads(), 1);
    let refresh = d.request(diagnose("codex", "recheck", 1));
    let cached = d.wait("codex", refresh["diagnosticId"].as_str().map(String::from));
    assert_eq!(cached["latest"]["cached"], true);
    assert_eq!(
        cached["latest"]["observedAtMs"],
        observed["latest"]["observedAtMs"]
    );
    assert_eq!(f.reads(), 1);
    assert!(!f.home.join("dispatch-count").exists());
    assert!(std::fs::read_dir(&f.home).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("fixture-thread-")
    }));
    let list = d.request(ConnectionRequest::List);
    assert!(!list.to_string().contains("descriptor"));
    assert_eq!(list["connections"][0]["independentAccountConfirmed"], false);
    let before_select = d.raw(Operation::Snapshot).unwrap();
    let cursor = serde_json::from_value(before_select["state"]["cursor"].clone()).unwrap();
    let select = ConnectionRequest::Update {
        command_id: "select-same-link".into(),
        id: "codex".into(),
        expected_revision: 1,
        change: ConnectionChange::Select,
    };
    d.request(select.clone());
    let events = second
        .request(&Request {
            api_version: API_VERSION,
            operation: Operation::Events { cursor, limit: 128 },
        })
        .unwrap();
    assert!(events.ok);
    assert!(
        events.result["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "connection-selected" && e["body"]["connectionId"] == "codex")
    );
    let after_select = d.raw(Operation::Snapshot).unwrap()["state"]["cursor"].clone();
    d.request(select);
    assert_eq!(d.raw(Operation::Snapshot).unwrap()["state"]["cursor"], after_select);
    let receipt = d
        .raw(Operation::CommandReceipt {
            command_id: "diagnose-second-window".into(),
            fingerprint: coalesced_fingerprint.clone(),
        })
        .unwrap();
    assert_eq!(receipt["found"], true);
    assert_eq!(
        receipt["receipt"]["response"]["diagnosticId"],
        first["diagnosticId"]
    );
    d.stop();
    let mut reopened = f.start();
    let replay = reopened.raw(coalesced_operation).unwrap();
    assert_eq!(replay["diagnosticId"], first["diagnosticId"]);
    assert_eq!(f.reads(), 1);
    assert_eq!(
        reopened
            .raw(Operation::CommandReceipt {
                command_id: "diagnose-second-window".into(),
                fingerprint: coalesced_fingerprint
            })
            .unwrap()["found"],
        true
    );
    reopened.stop();
}
#[test]
fn different_task_directories_are_not_coalesced_and_identity_change_requires_reconfirmation() {
    let f = Fixture::new();
    let mut d = f.start();
    d.request(f.link("codex", "connection-metadata", true));
    d.wait("codex", None);
    let a = f.root.join("task-a");
    let b = f.root.join("task-b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let first = d.request(ConnectionRequest::DiagnoseScope {
        command_id: "scope-a".into(),
        id: "codex".into(),
        expected_revision: 1,
        metadata_directory: a.clone(),
        allow_metadata_read: true,
    });
    let second = d.raw(Operation::Connections {
        request: Box::new(ConnectionRequest::DiagnoseScope {
            command_id: "scope-b".into(),
            id: "codex".into(),
            expected_revision: 1,
            metadata_directory: b.clone(),
            allow_metadata_read: true,
        }),
    });
    assert_eq!(second.unwrap_err(), "metadata-service-busy-different-scope");
    let first_report = d.wait("codex", first["diagnosticId"].as_str().map(String::from));
    assert_eq!(first_report["latest"]["purpose"], "task-preparation");
    let second = d.request(ConnectionRequest::DiagnoseScope {
        command_id: "scope-b".into(),
        id: "codex".into(),
        expected_revision: 1,
        metadata_directory: b,
        allow_metadata_read: true,
    });
    let second_report = d.wait("codex", second["diagnosticId"].as_str().map(String::from));
    assert_ne!(
        first_report["latest"]["scopeKey"],
        second_report["latest"]["scopeKey"]
    );
    std::fs::write(
        f.home.join("connection-identity"),
        "another-synthetic-identity",
    )
    .unwrap();
    let renamed = d.request(ConnectionRequest::Update {
        command_id: "rename".into(),
        id: "codex".into(),
        expected_revision: 1,
        change: ConnectionChange::Rename {
            name: "Same store, new label".into(),
        },
    });
    assert_eq!(
        (
            renamed["revision"].as_u64(),
            renamed["bindingRevision"].as_u64()
        ),
        (Some(2), Some(1))
    );
    let changed = d.request(diagnose("codex", "identity-recheck", 2));
    let changed = d.wait("codex", changed["diagnosticId"].as_str().map(String::from));
    assert_eq!(changed["identityChanged"], true);
    assert_eq!(changed["latest"]["identityChanged"], true);
    assert_eq!(
        d.raw(Operation::Connections {
            request: Box::new(ConnectionRequest::DiagnoseScope {
                command_id: "blocked-changed-identity".into(),
                id: "codex".into(),
                expected_revision: 2,
                metadata_directory: a,
                allow_metadata_read: true
            })
        })
        .unwrap_err(),
        "connection-observation-unconfirmed"
    );
    assert!(!f.home.join("dispatch-count").exists());
    d.stop();
}
#[test]
fn cancelling_owned_metadata_preserves_controls_and_restart_never_replays_it() {
    let f = Fixture::new();
    let mut d = f.start();
    d.request(f.link("codex", "metadata-hang", false));
    let accepted = d.request(diagnose("codex", "hanging-read", 1));
    d.request(ConnectionRequest::CancelDiagnostic {
        command_id: "cancel-read".into(),
        diagnostic_id: accepted["diagnosticId"].as_str().unwrap().into(),
        id: "codex".into(),
        expected_revision: 1,
    });
    let result = d.wait("codex", accepted["diagnosticId"].as_str().map(String::from));
    assert_eq!(result["latest"]["state"], "cancelled");
    assert_eq!(result["latest"]["provenance"], "synthetic");
    assert!(d.raw(Operation::Snapshot).is_ok());
    d.stop();
    let before = f.reads();
    let mut reopened = f.start();
    assert_eq!(
        reopened.diagnostic("codex", None)["latest"]["state"],
        "cancelled"
    );
    let replay = reopened.request(diagnose("codex", "hanging-read", 1));
    assert_eq!(replay["diagnosticId"], accepted["diagnosticId"]);
    assert_eq!(before, f.reads());
    reopened.stop();
}

#[test]
fn stale_cancel_from_another_window_does_not_cancel_a_new_diagnostic() {
    let f = Fixture::new();
    let mut d = f.start();
    let first = d.request(f.link("codex", "connection-metadata", true));
    let a = d.wait("codex", None);
    let a_id = a["latest"]["id"].as_str().unwrap().to_owned();
    assert_eq!(first["diagnosticId"], a_id);
    std::fs::write(f.home.join("connection-metadata-hang"), b"fixture").unwrap();
    let scope = f.root.join("another-context");
    std::fs::create_dir(&scope).unwrap();
    let b = d.request(ConnectionRequest::DiagnoseScope {
        command_id: "second-window-new-read".into(),
        id: "codex".into(),
        expected_revision: 1,
        metadata_directory: scope,
        allow_metadata_read: true,
    });
    let b_id = b["diagnosticId"].as_str().unwrap().to_owned();
    let stale = d.request(ConnectionRequest::CancelDiagnostic {
        command_id: "stale-window-cancel".into(),
        id: "codex".into(),
        expected_revision: 1,
        diagnostic_id: a_id,
    });
    assert_eq!(stale["cancellationRequested"], false);
    assert_eq!(
        d.diagnostic("codex", Some(b_id.clone()))["latest"]["state"],
        "running"
    );
    let exact = d.request(ConnectionRequest::CancelDiagnostic {
        command_id: "current-window-cancel".into(),
        id: "codex".into(),
        expected_revision: 1,
        diagnostic_id: b_id.clone(),
    });
    assert_eq!(exact["cancellationRequested"], true);
    assert_eq!(d.wait("codex", Some(b_id))["latest"]["state"], "cancelled");
    d.stop();
}

#[test]
fn daemon_restart_does_not_renew_a_recent_success_or_automatically_recheck_it() {
    let f = Fixture::new();
    let mut d = f.start();
    d.request(f.link("codex", "connection-metadata", true));
    let before = d.wait("codex", None);
    assert_eq!(before["latest"]["current"], true);
    assert_eq!(f.reads(), 1);
    d.stop();
    let mut reopened = f.start();
    let historical = reopened.diagnostic("codex", None);
    assert_eq!(historical["latest"]["current"], false);
    assert_eq!(
        historical["lastSuccessfulAtMs"],
        before["lastSuccessfulAtMs"]
    );
    assert_eq!(
        historical["latest"]["observedAtMs"],
        before["latest"]["observedAtMs"]
    );
    assert_eq!(f.reads(), 1);
    let accepted = reopened.request(diagnose("codex", "explicit-recheck-after-restart", 1));
    let fresh = reopened.wait("codex", accepted["diagnosticId"].as_str().map(String::from));
    assert_eq!(fresh["latest"]["current"], true);
    assert_ne!(
        fresh["latest"]["daemonGeneration"],
        before["latest"]["daemonGeneration"]
    );
    assert_eq!(f.reads(), 2);
    reopened.stop();
}
#[test]
fn first_partial_catalog_keeps_positive_identity_and_failure_preserves_last_success() {
    let f = Fixture::new();
    std::fs::write(f.home.join("connection-partial"), b"fixture").unwrap();
    let mut d = f.start();
    d.request(f.link("codex", "connection-metadata", true));
    let partial = d.wait("codex", None);
    assert_eq!(partial["latest"]["state"], "partial");
    assert_eq!(partial["latest"]["complete"], false);
    assert_eq!(partial["catalog"]["models"][0]["availability"], "unknown");
    let scope = f.root.join("task-scope");
    std::fs::create_dir(&scope).unwrap();
    let allowed = d.request(ConnectionRequest::DiagnoseScope {
        command_id: "partial-known-scope".into(),
        id: "codex".into(),
        expected_revision: 1,
        metadata_directory: scope,
        allow_metadata_read: true,
    });
    let scoped = d.wait("codex", allowed["diagnosticId"].as_str().map(String::from));
    assert_eq!(scoped["latest"]["state"], "partial");
    std::fs::write(f.home.join("connection-auth-invalid"), b"fixture").unwrap();
    d.request(ConnectionRequest::Update {
        command_id: "rename".into(),
        id: "codex".into(),
        expected_revision: 1,
        change: ConnectionChange::Rename {
            name: "Invalidate owned read cache".into(),
        },
    });
    let failed = d.request(diagnose("codex", "auth-failure", 2));
    let failed = d.wait("codex", failed["diagnosticId"].as_str().map(String::from));
    assert_eq!(failed["latest"]["state"], "failed");
    assert_eq!(failed["latest"]["provenance"], "synthetic");
    let history = d.diagnostic("codex", None);
    assert_eq!(
        history["lastSuccessfulAtMs"],
        scoped["latest"]["observedAtMs"]
    );
    assert!(!history["catalog"]["models"].as_array().unwrap().is_empty());
    assert!(!f.home.join("dispatch-count").exists());
    d.stop();
}
#[test]
fn known_link_old_prepared_task_is_rejected_after_disable_enable_and_unlink_without_new_dispatch() {
    use orchestrix_core::{
        evaluator::CheckSpec,
        policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
        worker::ManualTaskRequest,
    };
    let f = Fixture::new();
    let repository = f.root.join("repository");
    let worktrees = f.root.join("worktrees");
    std::fs::create_dir(&repository).unwrap();
    std::fs::create_dir(&worktrees).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=NUL",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
                "-c",
                "user.name=Connection fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "NUL")
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(output.status.success(), "fixture Git failed");
        output.stdout
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(repository.join("fixture.txt"), "before\n").unwrap();
    git(&["add", "--all"]);
    git(&["commit", "--quiet", "-m", "connection fixture"]);
    let base = String::from_utf8(git(&["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_owned();
    let mut d = f.start();
    d.request(f.link("codex", "connection-metadata", true));
    d.wait("codex", None);
    d.raw(Operation::Resume {
        command_id: "admit-explicit-fixture".into(),
    })
    .unwrap();
    let mut policy = PolicyDocument::default();
    policy.mode_defaults.fallback = Some(Fallback::Compatible);
    let mut task = ManualTaskRequest {
        schema_version: 1,
        id: "known-connection-worker".into(),
        project_id: "project".into(),
        repository_id: "repo".into(),
        repository_directory: repository,
        authorized_worktree_directory: worktrees,
        base_commit: base,
        connection_id: "codex".into(),
        connection_revision: 1,
        objective: "fixture-edit: change only fixture.txt".into(),
        acceptance_criteria: vec!["fixture.txt contains after followed by a newline".into()],
        policy,
        classification: TaskClassification::routine(TaskKind::Implementation, "manual-user"),
        context_files: vec![],
        instructions: vec!["Edit the authorized fixture only".into()],
        checks: vec![CheckSpec {
            id: "fixture-check".into(),
            executable: binary(),
            arguments: vec!["--check-fixture".into(), "after\n".into()],
            authorization_ref: "explicit-offline-check".into(),
            authorized: true,
            timeout_ms: 2000,
            executable_sha256: None,
        }],
    };
    let runtime = RuntimeChoice::Synthetic {
        executable: binary(),
        home: f.home.clone(),
        scenario: "success".into(),
    };
    d.raw(Operation::Submit {
        command_id: "first-task".into(),
        authorization_ref: "explicit-offline-turn".into(),
        task: Box::new(task.clone()),
        runtime: runtime.clone(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let snapshot = d.raw(Operation::Snapshot).unwrap();
        if snapshot["activeWorkerId"].is_null() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "existing supervised worker envelope"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let before = d
        .raw(Operation::Status {
            worker_id: task.id.clone(),
        })
        .unwrap();
    assert_eq!(before["worker"]["status"], "verified");
    let markers = std::fs::read_dir(&f.home)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("dispatch-event-")
        })
        .map(|e| e.file_name())
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 1);
    let renamed = d.request(ConnectionRequest::Update {
        command_id: "rename-history".into(),
        id: "codex".into(),
        expected_revision: 1,
        change: ConnectionChange::Rename {
            name: "Same linked runtime".into(),
        },
    });
    assert_eq!(renamed["bindingRevision"], 1);
    let resolution = d.request(ConnectionRequest::Resolve {
        id: "codex".into(),
        expected_revision: 2,
    });
    assert_eq!(resolution["bindingRevision"], 1);
    assert_eq!(
        d.raw(Operation::Status {
            worker_id: task.id.clone()
        })
        .unwrap(),
        before
    );
    let reads_before_stale = f.reads();
    task.id = "old-prepared-unsubmitted-task".into();
    for (key, cas, change) in [
        ("disable", 2, ConnectionChange::Disable),
        ("enable", 3, ConnectionChange::Enable),
        ("unlink", 4, ConnectionChange::Unlink),
    ] {
        d.request(ConnectionRequest::Update {
            command_id: key.into(),
            id: "codex".into(),
            expected_revision: cas,
            change,
        });
        let rejected = d.raw(Operation::Submit {
            command_id: format!("stale-prepared-{key}"),
            authorization_ref: "old-prepared-authorization".into(),
            task: Box::new(task.clone()),
            runtime: runtime.clone(),
        });
        assert!(rejected.is_err(), "old binding must not admit after {key}");
        let mut alias = task.clone();
        alias.connection_id = "invented-unregistered-id".into();
        assert_eq!(
            d.raw(Operation::Submit {
                command_id: format!("alias-after-{key}"),
                authorization_ref: "old-prepared-authorization".into(),
                task: Box::new(alias),
                runtime: runtime.clone()
            })
            .unwrap_err(),
            "connection-not-registered"
        );
        assert_eq!(f.reads(), reads_before_stale);
        assert_eq!(
            d.raw(Operation::Status {
                worker_id: "known-connection-worker".into()
            })
            .unwrap(),
            before
        );
        assert_eq!(
            d.raw(Operation::Snapshot).unwrap()["workers"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let after = std::fs::read_dir(&f.home)
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("dispatch-event-")
            })
            .map(|e| e.file_name())
            .collect::<Vec<_>>();
        assert_eq!(after, markers);
    }
    d.stop();
    let mut direct = orchestrix_core::persistence::Store::open(&f.database).unwrap();
    let launch = orchestrix_core::runtime::RuntimeLaunch::fixture(
        binary(),
        vec!["--scenario".into(), "success".into()],
        f.home.clone(),
        task.repository_directory.clone(),
    );
    let denied = orchestrix_core::worker::ManualWorker::run(
        &mut direct,
        &task,
        launch,
        &orchestrix_core::runtime::CancellationToken::new(),
    );
    assert!(denied.is_err());
    assert_eq!(direct.load().unwrap().workers().values().count(), 1);
    assert_eq!(f.reads(), reads_before_stale);
}

#[test]
#[ignore = "requires explicit current authorization; up to three read-only metadata jobs, no inference"]
fn live_connection_api_metadata_only_three_authorized_rounds() {
    assert_eq!(
        std::env::var("ORCHESTRIX_CONNECTION_METADATA_AUTHORIZATION").as_deref(),
        Ok("three-read-only-rounds-no-inference")
    );
    let exe = PathBuf::from(
        std::env::var_os("ORCHESTRIX_AUTHORIZED_CODEX_EXE").expect("authorized executable"),
    );
    let home = PathBuf::from(
        std::env::var_os("ORCHESTRIX_AUTHORIZED_CODEX_HOME")
            .expect("authorized runtime-managed home"),
    );
    let version = std::env::var("ORCHESTRIX_AUTHORIZED_CODEX_VERSION").expect("inspected version");
    let expected_sha = std::env::var("ORCHESTRIX_AUTHORIZED_CODEX_SHA256")
        .expect("explicitly approved executable SHA-256");
    let actual_sha = runtime_executable_sha256(&exe).unwrap();
    assert!(
        actual_sha == expected_sha,
        "authorized executable hash changed before any metadata job"
    );
    let f = Fixture::new();
    println!("metadata-only-evidence-directory={}", f.root.display());
    let mut d = f.start();
    let began = Instant::now();
    println!("round1-link-started");
    let linked = d.request(ConnectionRequest::Link {
        command_id: "authorized-live-link".into(),
        id: "authorized-codex".into(),
        expected_revision: None,
        name: "Authorized runtime-managed Codex".into(),
        runtime: RuntimeChoice::Codex {
            executable: exe,
            home,
            version,
            allow_live: false,
        },
        metadata_directory: f.cwd.clone(),
        executable_sha256: actual_sha,
        allow_metadata_read: true,
    });
    assert_eq!(linked["authenticated"], false);
    let first = d.wait_bounded(
        "authorized-codex",
        linked["diagnosticId"].as_str().map(String::from),
        Duration::from_secs(30).saturating_sub(began.elapsed()),
    );
    println!(
        "round1-link-finished elapsedMs={} state={}",
        began.elapsed().as_millis(),
        first["latest"]["state"].as_str().unwrap_or("unknown")
    );
    assert!(matches!(
        first["latest"]["state"].as_str(),
        Some("complete" | "partial")
    ));
    let began = Instant::now();
    println!("round2-recheck-started");
    let request = d.request(diagnose("authorized-codex", "authorized-recheck", 1));
    let second = d.wait_bounded(
        "authorized-codex",
        request["diagnosticId"].as_str().map(String::from),
        Duration::from_secs(30).saturating_sub(began.elapsed()),
    );
    println!(
        "round2-recheck-finished elapsedMs={} state={} cached={}",
        began.elapsed().as_millis(),
        second["latest"]["state"].as_str().unwrap_or("unknown"),
        second["latest"]["cached"] == true
    );
    assert!(matches!(
        second["latest"]["state"].as_str(),
        Some("complete" | "partial")
    ));
    let scoped = f.root.join("authorized-task-metadata");
    std::fs::create_dir(&scoped).unwrap();
    let began = Instant::now();
    println!("round3-task-scope-started");
    let request = d.request(ConnectionRequest::DiagnoseScope {
        command_id: "authorized-task-metadata".into(),
        id: "authorized-codex".into(),
        expected_revision: 1,
        metadata_directory: scoped,
        allow_metadata_read: true,
    });
    let third = d.wait_bounded(
        "authorized-codex",
        request["diagnosticId"].as_str().map(String::from),
        Duration::from_secs(30).saturating_sub(began.elapsed()),
    );
    println!(
        "round3-task-scope-finished elapsedMs={} state={}",
        began.elapsed().as_millis(),
        third["latest"]["state"].as_str().unwrap_or("unknown")
    );
    assert!(matches!(
        third["latest"]["state"].as_str(),
        Some("complete" | "partial")
    ));
    assert_ne!(first["latest"]["scopeKey"], third["latest"]["scopeKey"]);
    let snapshot = d.raw(Operation::Snapshot).unwrap();
    assert_eq!(snapshot["activeWorkerId"], Value::Null);
    assert_eq!(snapshot["workers"], json!([]));
    let evidence = f.root.join("metadata-only-evidence.json");
    std::fs::write(&evidence,serde_json::to_vec_pretty(&json!({"first":first,"recheck":second,"scoped":third,"database":snapshot["state"]["cursor"]["database"],"turnsAuthorized":0,"maximumMetadataJobs":3})).unwrap()).unwrap();
    println!("metadata-only-evidence={}", evidence.display());
    d.stop();
}
