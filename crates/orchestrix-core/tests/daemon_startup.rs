//! Actual startup/IPC/SQLite/children and one never-dispatched reservation.
//! A tiny owned Git fixture seeds that reservation; no provider inference.
#![cfg(windows)]
#[path = "../../../apps/desktop/src-tauri/src/daemon_startup.rs"]
mod startup_client;
use orchestrix_core::{
    daemon::{self, API_VERSION, Client, Operation, Request},
    desktop::DesktopRequest,
    evaluator::CheckSpec,
    persistence::Store,
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker, WorkerCheckpoint},
    worker_records::{WorkerRecord, WorkerStatus},
};
use serde_json::{Value, json};
use startup_client::{StartupController, StartupLimits};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Barrier},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Scope {
    root: PathBuf,
    database: PathBuf,
}
impl Scope {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-startup-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        Self {
            database: root.join("state.sqlite"),
            root,
        }
    }
    fn launches(&self) -> usize {
        fs::read_to_string(self.root.join("startup-launches.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
    }
    fn stop(&self, key: &str) {
        // Release only this fixture's gate if an assertion failed while waiting.
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.filter_map(Result::ok) {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("startup-entered-")
                    && let Ok(bytes) = fs::read(entry.path())
                    && let Ok(marker) = serde_json::from_slice::<Value>(&bytes)
                {
                    let _ = fs::write(
                        self.root.join("startup.release.json"),
                        json!({"nonce":marker["nonce"]}).to_string(),
                    );
                }
            }
        }
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            if let Ok(lease) = daemon::owner_lease(&self.database) {
                drop(lease);
                break;
            }
            if let Ok(client) = Client::new(&self.database) {
                let _ = client.request(&Request {
                    api_version: API_VERSION,
                    operation: Operation::Shutdown {
                        command_id: key.into(),
                    },
                });
            }
            assert!(Instant::now() < until, "own startup host failed to stop");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        self.stop("startup-cleanup");
        let root = self.root.canonicalize().unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-startup-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
fn controller() -> StartupController {
    StartupController::new(StartupLimits::new(Duration::from_secs(15)).unwrap())
}
fn compatible(snapshot: Value) -> Result<Value, String> {
    if !snapshot["features"]
        .as_array()
        .is_some_and(|f| f.iter().any(|v| v == "desktop-v1") && f.iter().any(|v| v == "review-v1"))
    {
        return Err("incompatible".into());
    }
    Ok(snapshot)
}
fn connect(controller: &mut StartupController, database: &Path) -> Value {
    let started = Instant::now();
    let value = controller
        .connect(
            database,
            Path::new(env!("CARGO_BIN_EXE_orchestrix-startup-fixture")),
            Path::new(env!("CARGO_BIN_EXE_orchestrix-supervisor")),
            compatible,
        )
        .unwrap()
        .into_value();
    eprintln!(
        "startup-measure connectMs={} state={} phase={} hostElapsedMs={} daemonElapsedMs={}",
        started.elapsed().as_millis(),
        value["startup"]["state"],
        value["startup"]["phase"],
        value["startup"]["hostElapsedMs"],
        value["startup"]["daemonElapsedMs"]
    );
    value
}
fn observe(controller: &mut StartupController, database: &Path) -> Value {
    controller
        .observe(database, compatible)
        .unwrap()
        .into_value()
}
fn ready(controller: &mut StartupController, database: &Path, mut value: Value) -> Value {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(30);
    let mut last = Value::Null;
    loop {
        if value.get("startup").is_none() {
            eprintln!(
                "startup-measure readyFollowUpMs={} diagnostics={}",
                started.elapsed().as_millis(),
                value["startupDiagnostics"]
            );
            return value;
        }
        let phase = json!({"state":value["startup"]["state"],"phase":value["startup"]["phase"]});
        if phase != last {
            eprintln!(
                "startup-measure state={} phase={} hostElapsedMs={} daemonElapsedMs={}",
                value["startup"]["state"],
                value["startup"]["phase"],
                value["startup"]["hostElapsedMs"],
                value["startup"]["daemonElapsedMs"]
            );
            last = phase;
        }
        assert_ne!(value["startup"]["state"], "failed", "{value}");
        assert!(
            Instant::now() < deadline,
            "startup readiness not confirmed: {value}"
        );
        std::thread::sleep(Duration::from_millis(20));
        value = observe(controller, database);
    }
}
fn api(database: &Path, operation: Operation) -> Value {
    let response = Client::new(database)
        .unwrap()
        .request(&Request {
            api_version: API_VERSION,
            operation,
        })
        .unwrap();
    assert!(response.ok, "{:?}", response.error);
    response.result
}

#[test]
fn cold_history_two_clients_and_reconnection_preserve_one_generation_per_start() {
    let scope = Scope::new();
    let barrier = Arc::new(Barrier::new(3));
    let mut clients = vec![];
    for _ in 0..2 {
        let database = scope.database.clone();
        let barrier = barrier.clone();
        clients.push(std::thread::spawn(move || {
            let mut c = controller();
            barrier.wait();
            let first = connect(&mut c, &database);
            let launched = first["startup"]["launchedByThisHost"] == true;
            let snapshot = ready(&mut c, &database, first);
            (c, snapshot, launched)
        }));
    }
    barrier.wait();
    let (mut first, a, first_launched) = clients.remove(0).join().unwrap();
    let (mut second, b, second_launched) = clients.remove(0).join().unwrap();
    assert_eq!(
        a["state"]["cursor"]["database"],
        b["state"]["cursor"]["database"]
    );
    assert_eq!(a["state"]["generation"], 1);
    assert_eq!(b["state"]["generation"], 1);
    // Readiness from the winner does not wait for the losing child to enter
    // main. Account for every explicit start before taking the ledger baseline.
    let launches = usize::from(first_launched) + usize::from(second_launched);
    assert!((1..=2).contains(&launches));
    let ledger_deadline = Instant::now() + Duration::from_secs(30);
    while scope.launches() < launches {
        assert!(
            Instant::now() < ledger_deadline,
            "an explicitly launched helper never entered main"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(scope.launches(), launches);
    let warm = connect(&mut second, &scope.database);
    assert!(warm.get("startup").is_none());
    assert_eq!(scope.launches(), launches);
    let bootstrap = api(
        &scope.database,
        Operation::Desktop {
            request: Box::new(DesktopRequest::Bootstrap),
        },
    );
    api(
        &scope.database,
        Operation::Desktop {
            request: Box::new(DesktopRequest::SendMessage {
                command_id: "startup-message".into(),
                expected_revision: bootstrap["revision"].as_u64().unwrap(),
                conversation_id: "startup-conversation".into(),
                project_id: None,
                content: "Startup history fixture".into(),
            }),
        },
    );
    scope.stop("startup-stop-first");
    let first_update = connect(&mut first, &scope.database);
    let reopened = ready(&mut first, &scope.database, first_update);
    assert_eq!(reopened["state"]["generation"], 2);
    assert_eq!(
        reopened["state"]["cursor"]["database"],
        a["state"]["cursor"]["database"]
    );
    assert_eq!(reopened["state"]["suspended"], true);
    assert_eq!(
        reopened["startupDiagnostics"]["phases"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    assert_eq!(
        reopened["startupDiagnostics"]["daemonBindingConfirmed"],
        false
    );
    assert!(
        reopened["startupDiagnostics"]["reportedProcessPid"]
            .as_u64()
            .is_some()
    );
    assert_eq!(scope.launches(), launches + 1);
    let next = observe(&mut second, &scope.database);
    assert!(next.get("startup").is_none());
    assert_eq!(next["state"]["generation"], 2);
    assert_eq!(scope.launches(), launches + 1);
    let bootstrap = api(
        &scope.database,
        Operation::Desktop {
            request: Box::new(DesktopRequest::Bootstrap),
        },
    );
    assert_eq!(bootstrap["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(bootstrap["conversations"][0]["messageCount"], 1);
    assert!(reopened["workers"].as_array().unwrap().is_empty());
    scope.stop("startup-stop-history");
}

#[test]
fn actual_database_failure_is_failed_and_observation_does_not_respawn() {
    let scope = Scope::new();
    fs::write(&scope.database, b"Deliberately invalid SQLite test bytes").unwrap();
    let mut c = controller();
    let mut value = connect(&mut c, &scope.database);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if value["startup"]["state"] == "failed" {
            break;
        }
        assert!(Instant::now() < deadline, "{value}");
        value = observe(&mut c, &scope.database);
    }
    assert_eq!(value["startup"]["phase"], "database");
    assert_eq!(value["startup"]["ownChildExitCode"], 2);
    assert_eq!(scope.launches(), 1);
    for _ in 0..2 {
        assert_eq!(
            observe(&mut c, &scope.database)["startup"]["state"],
            "failed"
        );
    }
    assert_eq!(scope.launches(), 1);
    assert_eq!(
        fs::read(&scope.database).unwrap(),
        b"Deliberately invalid SQLite test bytes"
    );
}

#[test]
fn controlled_recovery_past_old_window_remains_observable_and_rechecks_without_spawn() {
    let scope = Scope::new();
    fs::write(
        scope.root.join("startup-fixture.json"),
        r#"{"gate_worker_recovery":true}"#,
    )
    .unwrap();
    let mut c = controller();
    let mut value = connect(&mut c, &scope.database);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut recovering = false;
    let mut recovery_seen_at = None;
    loop {
        recovering |= value["startup"]["state"] == "recovering";
        if value["startup"]["state"] == "recovering" && recovery_seen_at.is_none() {
            recovery_seen_at = Some(Instant::now());
        }
        if value["startup"]["state"] == "blocked" {
            break;
        }
        assert_ne!(value["startup"]["state"], "failed", "{value}");
        assert!(Instant::now() < deadline, "{value}");
        value = observe(&mut c, &scope.database);
    }
    assert!(recovering);
    assert_eq!(value["startup"]["phases"].as_array().unwrap().len(), 4);
    eprintln!(
        "startup-measure controlledState={} phase={} hostElapsedMs={} daemonElapsedMs={} phases={}",
        value["startup"]["state"],
        value["startup"]["phase"],
        value["startup"]["hostElapsedMs"],
        value["startup"]["daemonElapsedMs"],
        value["startup"]["phases"]
    );
    assert_eq!(value["startup"]["phase"], "worker-recovery");
    assert_eq!(value["startup"]["phaseSource"], "own-startup-report");
    assert_eq!(value["startup"]["pastInitialWindow"], true);
    assert_eq!(value["startup"]["automaticFollowUp"], false);
    assert_eq!(value["startup"]["explicitConnectAvailable"], false);
    assert!(value["startup"]["ownChildExitCode"].is_null());
    let nonce = value["startup"]["launchNonce"].as_str().unwrap().to_owned();
    let pid = value["startup"]["ownChildPid"].clone();
    let marker: Value = serde_json::from_slice(
        &fs::read(scope.root.join(format!("startup-entered-{nonce}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(marker["nonce"], nonce);
    assert_eq!(marker["pid"], pid);
    assert_eq!(scope.launches(), 1);
    let explicit_while_alive = connect(&mut c, &scope.database);
    assert_eq!(explicit_while_alive["startup"]["state"], "blocked");
    assert_eq!(explicit_while_alive["startup"]["ownChildPid"], pid);
    assert_eq!(explicit_while_alive["startup"]["launchNonce"], nonce);
    assert_eq!(
        explicit_while_alive["startup"]["explicitConnectAvailable"],
        false
    );
    assert_eq!(scope.launches(), 1);
    let mut other = controller();
    let second = connect(&mut other, &scope.database);
    assert_eq!(second["startup"]["state"], "initializing");
    assert_eq!(second["startup"]["phaseSource"], "other-owner");
    assert!(second["startup"]["phase"].is_null());
    assert_eq!(second["startup"]["launchedByThisHost"], false);
    let readonly = rusqlite::Connection::open_with_flags(
        &scope.database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let workers: i64 = readonly
        .query_row("SELECT count(*) FROM manual_workers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(workers, 0);
    drop(readonly);
    assert_eq!(
        observe(&mut c, &scope.database)["startup"]["state"],
        "blocked"
    );
    assert_eq!(scope.launches(), 1);
    // The barrier deliberately holds actual recovery for the old 15 s window,
    // independently of a slow Windows spawn before that phase was entered.
    while recovery_seen_at.unwrap().elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(20));
    }
    fs::write(
        scope.root.join("startup.release.json"),
        json!({"nonce":nonce}).to_string(),
    )
    .unwrap();
    let next = observe(&mut c, &scope.database);
    let snapshot = ready(&mut c, &scope.database, next);
    assert_eq!(snapshot["state"]["generation"], 1);
    assert_eq!(snapshot["state"]["suspended"], true);
    let phases = snapshot["startupDiagnostics"]["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 9);
    assert!(
        phases[4]["elapsedMs"].as_u64().unwrap() - phases[3]["elapsedMs"].as_u64().unwrap()
            >= 15000
    );
    let next = observe(&mut other, &scope.database);
    let remote = ready(&mut other, &scope.database, next);
    assert_eq!(
        remote["state"]["cursor"]["database"],
        snapshot["state"]["cursor"]["database"]
    );
    assert_eq!(scope.launches(), 1);
    scope.stop("startup-stop-controlled");
}

#[test]
fn vanished_owner_requires_explicit_connect_after_observation_budget() {
    let scope = Scope::new();
    // This real lease models an owner that disappears before exposing IPC.
    // No database, runtime, fake terminal result or original history is edited.
    let owner = daemon::owner_lease(&scope.database).unwrap();
    let mut c = controller();
    let first = connect(&mut c, &scope.database);
    assert_eq!(first["startup"]["state"], "initializing");
    assert_eq!(first["startup"]["launchedByThisHost"], false);
    assert_eq!(first["startup"]["explicitConnectAvailable"], false);
    drop(owner);
    let mut value = observe(&mut c, &scope.database);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert_ne!(
            value["startup"]["state"], "failed",
            "absence of an unowned process is not a confirmed Child exit"
        );
        assert_eq!(scope.launches(), 0, "observation must not launch");
        if value["startup"]["state"] == "blocked" {
            break;
        }
        assert!(Instant::now() < deadline, "{value}");
        value = observe(&mut c, &scope.database);
    }
    assert_eq!(value["startup"]["explicitConnectAvailable"], true);
    let checked = observe(&mut c, &scope.database);
    assert_eq!(checked["startup"]["state"], "blocked");
    assert_eq!(checked["startup"]["explicitConnectAvailable"], true);
    assert_eq!(scope.launches(), 0);
    let explicit = connect(&mut c, &scope.database);
    let snapshot = ready(&mut c, &scope.database, explicit);
    assert_eq!(snapshot["state"]["generation"], 1);
    assert_eq!(scope.launches(), 1);
    assert!(connect(&mut c, &scope.database).get("startup").is_none());
    assert_eq!(scope.launches(), 1);
    scope.stop("startup-vanished-owner-stop");
}

#[test]
fn invalid_optional_progress_nonce_does_not_prevent_real_core_readiness() {
    let scope = Scope::new();
    eprintln!(
        "core-cli-binary-sha256={}",
        orchestrix_core::git_records::content_hash(
            &fs::read(env!("CARGO_BIN_EXE_orchestrix")).unwrap()
        )
    );
    // Exercise the actual CLI, not the fixture that validates its own nonce.
    // Optional progress is not an authentication or admission requirement.
    let mut child = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
        .args(["daemon", "--database"])
        .arg(&scope.database)
        .env(
            orchestrix_core::daemon_startup::NONCE_ENV,
            "invalid-private-progress-nonce",
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let snapshot = loop {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("Core exited before readiness: {status}");
        }
        if let Ok(response) = Client::new(&scope.database).unwrap().request(&Request {
            api_version: API_VERSION,
            operation: Operation::Snapshot,
        }) {
            assert!(response.ok, "{:?}", response.error);
            break compatible(response.result).unwrap();
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "Core readiness with invalid optional telemetry was not confirmed"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(snapshot["state"]["generation"], 1);
    assert_eq!(snapshot["state"]["suspended"], true);
    scope.stop("startup-invalid-nonce-stop");
    let stopped = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        // Includes the existing 20 s daemon deadline and bounded IPC scheduling.
        assert!(
            stopped.elapsed() < Duration::from_secs(30),
            "own idle Core failed to stop"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
    eprintln!(
        "startup-measure invalidOptionalNonceReadyAndShutdownMs={}",
        started.elapsed().as_millis()
    );
}

#[test]
fn real_core_cli_reports_owned_startup_phases_without_granting_trace_authority() {
    let scope = Scope::new();
    let core = Path::new(env!("CARGO_BIN_EXE_orchestrix"));
    eprintln!(
        "core-cli-binary-sha256={}",
        orchestrix_core::git_records::content_hash(&fs::read(core).unwrap())
    );
    let mut c = controller();
    let update = c
        .connect(
            &scope.database,
            core,
            Path::new(env!("CARGO_BIN_EXE_orchestrix-supervisor")),
            compatible,
        )
        .unwrap()
        .into_value();
    let snapshot = ready(&mut c, &scope.database, update);
    assert_eq!(snapshot["state"]["generation"], 1);
    assert_eq!(snapshot["state"]["suspended"], true);
    let trace = &snapshot["startupDiagnostics"];
    assert_eq!(trace["reportCompleted"], true);
    assert_eq!(trace["reportedFailure"], false);
    assert_eq!(trace["daemonBindingConfirmed"], false);
    assert!(
        trace["reportedProcessPid"]
            .as_u64()
            .is_some_and(|pid| pid > 0)
    );
    let phases = trace["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 9);
    assert_eq!(phases.first().unwrap()["phase"], "ownership");
    assert_eq!(phases.last().unwrap()["phase"], "ipc-available");
    assert!(
        phases
            .windows(2)
            .all(|pair| pair[0]["elapsedMs"].as_u64() <= pair[1]["elapsedMs"].as_u64())
    );
    scope.stop("startup-core-cli-progress-stop");
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let mut command = Command::new("git");
    command.env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = command
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "user.name=Orchestrix Fixture",
            "-c",
            "user.email=fixture@orchestrix.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
#[test]
#[ignore = "own subprocess seed, explicitly invoked by startup test"]
fn pending_reservation_child() {
    let root =
        PathBuf::from(std::env::var_os("ORCHESTRIX_STARTUP_SEED_ROOT").expect("own fixture path"));
    let request: ManualTaskRequest =
        serde_json::from_slice(&fs::read(root.join("task.json")).unwrap()).unwrap();
    let mut store = Store::open(root.join("state.sqlite")).unwrap();
    let mut launch = RuntimeLaunch::fixture(
        PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
        vec!["--scenario".into(), "success".into()],
        root.join("synthetic-home"),
        request.repository_directory.clone(),
    );
    launch.request_timeout = Duration::from_secs(2);
    launch.turn_timeout = Duration::from_secs(3);
    let result = ManualWorker::run_with_checkpoint(
        &mut store,
        &request,
        launch,
        &CancellationToken::new(),
        |phase| {
            if phase == WorkerCheckpoint::Reserved {
                std::process::exit(86);
            }
        },
    );
    panic!("reservation checkpoint not reached: {result:?}");
}
#[test]
fn legitimate_pending_history_is_reconciled_before_readiness_without_dispatch() {
    let scope = Scope::new();
    let repository = scope.root.join("source");
    let home = scope.root.join("synthetic-home");
    let worktrees = scope.root.join("attempts");
    for path in [&repository, &home, &worktrees] {
        fs::create_dir(path).unwrap();
    }
    git(&repository, &["init", "--quiet", "--initial-branch=main"]);
    fs::write(repository.join("fixture.txt"), "before\n").unwrap();
    git(&repository, &["add", "--all"]);
    git(
        &repository,
        &["commit", "--quiet", "-m", "Startup reservation fixture"],
    );
    let mut policy = PolicyDocument::default();
    policy.mode_defaults.fallback = Some(Fallback::Compatible);
    let task = ManualTaskRequest {
        editable_paths: None,
        schema_version: 1,
        id: "startup-pending".into(),
        project_id: "startup-project".into(),
        repository_id: "startup-repository".into(),
        repository_directory: repository.clone(),
        authorized_worktree_directory: worktrees.clone(),
        base_commit: git(&repository, &["rev-parse", "HEAD"]),
        connection_id: "startup-account".into(),
        connection_revision: 1,
        objective: "fixture-edit: correct only owned fixture".into(),
        acceptance_criteria: vec!["fixture.txt contains after followed by newline".into()],
        policy,
        classification: TaskClassification::routine(TaskKind::Implementation, "explicit-user"),
        context_files: vec![ContextFileRequest {
            path: "fixture.txt".into(),
            authorization_ref: "startup-read-fixture".into(),
            required: true,
        }],
        instructions: vec!["Only edit the owned worktree and never delegate.".into()],
        checks: vec![],
    };
    fs::write(
        scope.root.join("task.json"),
        serde_json::to_vec(&task).unwrap(),
    )
    .unwrap();
    let seeded = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "pending_reservation_child",
            "--nocapture",
        ])
        .env("ORCHESTRIX_STARTUP_SEED_ROOT", &scope.root)
        .output()
        .unwrap();
    assert_eq!(
        seeded.status.code(),
        Some(86),
        "{}",
        String::from_utf8_lossy(&seeded.stderr)
    );
    let before = Store::open(&scope.database).unwrap().load().unwrap();
    let original = before.workers()["startup-pending"].clone();
    assert_eq!(original.status(), WorkerStatus::Reserved);
    drop(before);
    assert!(!home.join("dispatch-count").exists());
    assert_eq!(fs::read_dir(&worktrees).unwrap().count(), 0);
    let mut c = controller();
    let value = connect(&mut c, &scope.database);
    let snapshot = ready(&mut c, &scope.database, value);
    assert_eq!(snapshot["state"]["suspended"], true);
    let recovery = api(&scope.database, Operation::Recovery);
    assert_eq!(recovery["has_blocking_cases"], true);
    assert_eq!(recovery["ready_for_admission"], false);
    assert_eq!(recovery["cases"][0]["worker_id"], "startup-pending");
    assert_eq!(recovery["cases"][0]["dispatch"], "never-sent");
    let resume = Client::new(&scope.database)
        .unwrap()
        .request(&Request {
            api_version: API_VERSION,
            operation: Operation::Resume {
                command_id: "startup-pending-resume".into(),
            },
        })
        .unwrap();
    assert!(!resume.ok);
    assert_eq!(scope.launches(), 1);
    scope.stop("startup-pending-stop");
    let after = Store::open(&scope.database).unwrap().load().unwrap();
    let restored = &after.workers()["startup-pending"];
    assert_eq!(restored.status(), WorkerStatus::Blocked);
    assert_eq!(restored.intent, original.intent);
    assert!(!home.join("dispatch-count").exists());
    assert_eq!(fs::read_dir(&worktrees).unwrap().count(), 0);
}

fn seed_history(scope: &Scope, scenario: &str, checked: bool) -> WorkerRecord {
    let repository = scope.root.join("source");
    let home = scope.root.join("synthetic-home");
    let worktrees = scope.root.join("attempts");
    for directory in [&repository, &home, &worktrees] {
        fs::create_dir(directory).unwrap();
    }
    git(&repository, &["init", "--quiet", "--initial-branch=main"]);
    fs::write(repository.join("fixture.txt"), "before\n").unwrap();
    git(&repository, &["add", "--all"]);
    git(
        &repository,
        &[
            "commit",
            "--quiet",
            "-m",
            "Startup historical worker fixture",
        ],
    );
    let mut policy = PolicyDocument::default();
    policy.mode_defaults.fallback = Some(Fallback::Compatible);
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"));
    let request = ManualTaskRequest {
        editable_paths: None,
        schema_version: 1,
        id: "startup-history".into(),
        project_id: "startup-project".into(),
        repository_id: "startup-repository".into(),
        repository_directory: repository.clone(),
        authorized_worktree_directory: worktrees,
        base_commit: git(&repository, &["rev-parse", "HEAD"]),
        connection_id: "startup-account".into(),
        connection_revision: 1,
        objective: "fixture-edit: correct only the owned historical fixture".into(),
        acceptance_criteria: vec!["fixture.txt contains after followed by newline".into()],
        policy,
        classification: TaskClassification::routine(TaskKind::Implementation, "explicit-user"),
        context_files: vec![ContextFileRequest {
            path: "fixture.txt".into(),
            authorization_ref: "startup-history-read-fixture".into(),
            required: true,
        }],
        instructions: vec!["Only edit the owned worktree and never delegate.".into()],
        checks: if checked {
            vec![CheckSpec {
                id: "startup-history-check".into(),
                executable: binary.clone(),
                arguments: vec!["--check-fixture".into(), "after\n".into()],
                authorization_ref: "explicit-startup-history-check".into(),
                authorized: true,
                timeout_ms: 2000,
                executable_sha256: None,
            }]
        } else {
            vec![]
        },
    };
    let mut launch = RuntimeLaunch::fixture(
        binary,
        vec!["--scenario".into(), scenario.into()],
        home,
        repository,
    );
    launch.request_timeout = Duration::from_secs(2);
    launch.turn_timeout = Duration::from_secs(3);
    ManualWorker::run(
        &mut Store::open(&scope.database).unwrap(),
        &request,
        launch,
        &CancellationToken::new(),
    )
    .unwrap()
}

struct HistoryDaemon<'a> {
    scope: &'a Scope,
    child: std::process::Child,
}
impl<'a> HistoryDaemon<'a> {
    fn start(scope: &'a Scope, without_git: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_orchestrix"));
        command
            .arg("daemon")
            .arg("--database")
            .arg(&scope.database)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(scope.root.join("history-daemon.stderr")).unwrap());
        if without_git {
            // Only this absolute Core child's PATH changes. GitExecutor::new
            // cannot find Git, so readiness proves the discarded historical
            // enrichment did not run. No parent/global environment is edited.
            let empty = scope.root.join("empty-executable-path");
            fs::create_dir(&empty).unwrap();
            command.env("PATH", empty);
        }
        Self {
            scope,
            child: command.spawn().unwrap(),
        }
    }
    fn ready(&mut self) -> Value {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!(
                    "historical startup exited {status}: {}",
                    fs::read_to_string(self.scope.root.join("history-daemon.stderr")).unwrap()
                );
            }
            if let Ok(client) = Client::new(&self.scope.database)
                && let Ok(response) = client.request(&Request {
                    api_version: API_VERSION,
                    operation: Operation::Snapshot,
                })
            {
                assert!(response.ok, "{:?}", response.error);
                eprintln!("startup-history readyMs={}", started.elapsed().as_millis());
                return compatible(response.result).unwrap();
            }
            assert!(Instant::now() < deadline, "historical startup not ready");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn stop(mut self) {
        self.scope.stop("history-startup-stop");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(0));
                return;
            }
            assert!(
                Instant::now() < deadline,
                "own historical daemon did not exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for HistoryDaemon<'_> {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        // Failure cleanup targets only this Child. This fixture daemon never
        // accepts runtime work: it reads retained history and starts suspended.
        let _ = Client::new(&self.scope.database).and_then(|client| {
            client.request(&Request {
                api_version: API_VERSION,
                operation: Operation::Shutdown {
                    command_id: "history-startup-failure-cleanup".into(),
                },
            })
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn observe_verified_history(scope: &Scope) -> Value {
    // The native Host's 15-second window is an observation boundary, not an
    // assertion that mandatory Verified recapture must finish in that time.
    // This test's separate finite cap covers explicit Check-again observations
    // after the exhausted window: the previous 30-second guard interrupted
    // recapture, while the recorded native recovery phase alone took 39.688s.
    // It changes no production, Git, runtime, check or keeper deadline.
    let mut controller = controller();
    let started = Instant::now();
    let cap = Duration::from_secs(120);
    let mut value = controller
        .connect(
            &scope.database,
            Path::new(env!("CARGO_BIN_EXE_orchestrix")),
            Path::new(env!("CARGO_BIN_EXE_orchestrix-supervisor")),
            compatible,
        )
        .unwrap()
        .into_value();
    let pid = value["startup"]["ownChildPid"].clone();
    let nonce = value["startup"]["launchNonce"].clone();
    assert!(pid.as_u64().is_some(), "one owned startup Child required");
    assert!(nonce.as_str().is_some(), "one startup attempt required");
    let mut last = Value::Null;
    let mut followed_blocked_window = false;
    loop {
        let elapsed = started.elapsed();
        assert!(
            elapsed < cap,
            "Verified startup observation cap exhausted: elapsedMs={} state={} phase={}",
            elapsed.as_millis(),
            value["startup"]["state"],
            value["startup"]["phase"]
        );
        if value.get("startup").is_none() {
            // A private phase trace never grants readiness. The compatible IPC
            // Snapshot must already contain the negative recapture outcome.
            let worker = value["workers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|worker| worker["workerId"] == "startup-history")
                .unwrap();
            assert_eq!(worker["status"], "invalidated");
            if value["startupDiagnostics"].is_object() {
                assert_eq!(value["startupDiagnostics"]["reportedProcessPid"], pid);
                assert_eq!(value["startupDiagnostics"]["daemonBindingConfirmed"], false);
            }
            eprintln!(
                "startup-verified-history readyMs={} followedBlockedWindow={} diagnostics={}",
                elapsed.as_millis(),
                followed_blocked_window,
                value["startupDiagnostics"]
            );
            return value;
        }
        let pending = &value["startup"];
        assert_eq!(pending["ownChildPid"], pid);
        assert!(
            pending["launchNonce"] == nonce,
            "same startup attempt required"
        );
        assert_eq!(pending["followUpBudgetMs"], 15_000);
        assert_ne!(pending["state"], "failed");
        if pending["state"] == "blocked" {
            assert_eq!(pending["pastInitialWindow"], true);
            assert_eq!(pending["automaticFollowUp"], false);
            followed_blocked_window = true;
        }
        let phase = json!({"state":pending["state"],"phase":pending["phase"]});
        if phase != last {
            eprintln!(
                "startup-verified-history observation={}",
                json!({"elapsedMs":elapsed.as_millis(),"state":pending["state"],"phase":pending["phase"],
                    "phaseSource":pending["phaseSource"],"daemonElapsedMs":pending["daemonElapsedMs"],"ownChildPid":pid})
            );
            last = phase;
        }
        std::thread::sleep(Duration::from_millis(500).min(cap.saturating_sub(elapsed)));
        // This is explicit continued observation of the retained attempt,
        // including after Blocked. Never call connect or launch another Child.
        value = controller
            .observe(&scope.database, compatible)
            .unwrap()
            .into_value();
    }
}

#[test]
fn startup_recovery_historical_worktrees_defer_git_but_public_reconcile_inspects() {
    let scope = Scope::new();
    let original = seed_history(&scope, "success", false);
    assert_eq!(original.status(), WorkerStatus::NoChecks);
    let dispatch = fs::read(scope.root.join("synthetic-home/dispatch-count")).ok();
    let mut daemon = HistoryDaemon::start(&scope, true);
    let snapshot = daemon.ready();
    assert_eq!(snapshot["state"]["suspended"], true);
    assert_eq!(snapshot["activeWorkerId"], Value::Null);
    let historical = api(&scope.database, Operation::Recovery);
    assert_eq!(historical["ready_for_admission"], true);
    assert_eq!(historical["has_blocking_cases"], false);
    assert_eq!(historical["cases"][0]["status"], "no-checks");
    assert_eq!(
        historical["cases"][0]["worktrees"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        historical["cases"][0]["worktrees"][0]["inspection"],
        "historical-not-revalidated"
    );
    daemon.stop();
    // The existing public API still performs its physical diagnostic pass.
    let full =
        orchestrix_core::recovery::reconcile(&mut Store::open(&scope.database).unwrap()).unwrap();
    assert!(full.ready_for_admission);
    assert_eq!(full.cases[0].status, WorkerStatus::NoChecks);
    assert_eq!(full.cases[0].worktrees[0]["inspection"], "confirmed-owned");
    assert_eq!(
        fs::read(scope.root.join("synthetic-home/dispatch-count")).ok(),
        dispatch
    );
}

#[test]
fn startup_recovery_uncertain_history_without_git_blocks_admission_without_dispatch() {
    let scope = Scope::new();
    let original = seed_history(&scope, "lost-reply", false);
    assert_eq!(original.status(), WorkerStatus::Uncertain);
    let dispatch = fs::read(scope.root.join("synthetic-home/dispatch-count")).ok();
    let mut daemon = HistoryDaemon::start(&scope, true);
    let snapshot = daemon.ready();
    assert_eq!(snapshot["state"]["suspended"], true);
    assert_eq!(snapshot["activeWorkerId"], Value::Null);
    let recovery = api(&scope.database, Operation::Recovery);
    assert_eq!(recovery["has_blocking_cases"], true);
    assert_eq!(recovery["ready_for_admission"], false);
    assert_eq!(recovery["cases"][0]["status"], "uncertain");
    assert_eq!(
        recovery["cases"][0]["worktrees"][0]["inspection"],
        "historical-not-revalidated"
    );
    let resume = Client::new(&scope.database)
        .unwrap()
        .request(&Request {
            api_version: API_VERSION,
            operation: Operation::Resume {
                command_id: "uncertain-history-resume-must-block".into(),
            },
        })
        .unwrap();
    assert!(!resume.ok);
    assert_eq!(
        resume.error.as_deref(),
        Some("recovery-inspection-required")
    );
    daemon.stop();
    let restored = Store::open(&scope.database).unwrap().load().unwrap();
    assert_eq!(
        restored.workers()["startup-history"].intent,
        original.intent
    );
    assert_eq!(
        restored.workers()["startup-history"].status(),
        WorkerStatus::Uncertain
    );
    assert_eq!(
        fs::read(scope.root.join("synthetic-home/dispatch-count")).ok(),
        dispatch
    );
}

#[test]
fn startup_recovery_recaptures_changed_verified_artifacts_and_preserves_historical_checks() {
    let scope = Scope::new();
    let original = seed_history(&scope, "success", true);
    assert_eq!(original.status(), WorkerStatus::Verified);
    let before = Store::open(&scope.database).unwrap().load().unwrap();
    let historical = original.observations.last().unwrap().details.clone();
    let original_revision = historical["artifactRevision"].as_str().unwrap();
    let original_artifact = serde_json::to_value(
        before
            .captured_artifacts()
            .values()
            .find(|artifact| artifact.revision == original_revision)
            .unwrap(),
    )
    .unwrap();
    let worktree = PathBuf::from(
        &before
            .worktrees()
            .values()
            .find(|worktree| worktree.intent.attempt_id == original.intent.attempt_id)
            .unwrap()
            .intent
            .path,
    )
    .canonicalize()
    .unwrap();
    assert!(worktree.starts_with(scope.root.join("attempts")));
    drop(before);
    let dispatch = fs::read(scope.root.join("synthetic-home/dispatch-count")).ok();
    fs::write(worktree.join("fixture.txt"), "changed after verification\n").unwrap();
    let snapshot = observe_verified_history(&scope);
    assert_eq!(snapshot["state"]["suspended"], true);
    assert_eq!(snapshot["activeWorkerId"], Value::Null);
    let worker = api(
        &scope.database,
        Operation::Desktop {
            request: Box::new(DesktopRequest::InspectWorker {
                worker_id: "startup-history".into(),
            }),
        },
    );
    assert_eq!(worker["summary"]["status"], "invalidated");
    assert_eq!(
        worker["summary"]["details"]["reason"],
        "changed-after-verification"
    );
    assert_eq!(worker["artifact"]["revision"], original_revision);
    assert_eq!(worker["summary"]["details"]["checks"], historical["checks"]);
    scope.stop("verified-history-startup-stop");
    let restored = Store::open(&scope.database).unwrap().load().unwrap();
    assert_eq!(
        serde_json::to_value(
            restored
                .captured_artifacts()
                .values()
                .find(|artifact| artifact.revision == original_revision)
                .unwrap()
        )
        .unwrap(),
        original_artifact
    );
    assert_eq!(
        fs::read(scope.root.join("synthetic-home/dispatch-count")).ok(),
        dispatch
    );
}
