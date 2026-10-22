//! Real child-process exits, Windows Jobs, Git and SQLite; synthetic provider only.
#![cfg(windows)]
use orchestrix_core::{
    domain::{AttemptState, RunId, TaskId},
    persistence::Store,
    policy::{ContextBudget, Fallback, PolicyDocument, TaskClassification, TaskKind},
    recovery::{self, DispatchKnowledge, RecoveryAction},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker, WorkerCheckpoint},
    worker_records::WorkerStatus,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    home: PathBuf,
    database: PathBuf,
    authorized: PathBuf,
    base: String,
}
struct ForeignChild(std::process::Child);
impl Drop for ForeignChild {
    fn drop(&mut self) {
        if self.0.try_wait().is_ok_and(|s| s.is_none()) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
            "orchestrix-recovery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("source");
        let home = root.join("synthetic-home");
        let authorized = root.join("attempts");
        for path in [&repository, &home, &authorized] {
            fs::create_dir(path).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), "before\n").unwrap();
        git(&repository, &["add", "--all"]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "Recovery fixture"],
        );
        let base = git(&repository, &["rev-parse", "HEAD"]);
        Self {
            database: root.join("state.sqlite"),
            root,
            repository,
            home,
            authorized,
            base,
        }
    }
    fn request(&self) -> ManualTaskRequest {
        let mut policy = PolicyDocument::default();
        policy.mode_defaults.fallback = Some(Fallback::Compatible);
        ManualTaskRequest {
            editable_paths: None,
            schema_version: 1,
            id: "recovery-worker".into(),
            project_id: "recovery-project".into(),
            repository_id: "recovery-repository".into(),
            repository_directory: self.repository.clone(),
            authorized_worktree_directory: self.authorized.clone(),
            base_commit: self.base.clone(),
            connection_id: "recovery-account".into(),
            connection_revision: 1,
            objective: "fixture-edit: correct only the owned fixture".into(),
            acceptance_criteria: vec!["fixture.txt contains after followed by newline".into()],
            policy,
            classification: TaskClassification::routine(TaskKind::Implementation, "explicit-user"),
            context_files: vec![ContextFileRequest {
                path: "fixture.txt".into(),
                authorization_ref: "read-fixture".into(),
                required: true,
            }],
            instructions: vec!["Only edit the owned worktree and never delegate.".into()],
            checks: vec![],
        }
    }
    fn threads(&self) -> usize {
        fs::read_dir(&self.home)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fixture-thread-")
            })
            .count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.root).unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(env::temp_dir()).unwrap().as_path())
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-recovery-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
fn git(path: &Path, args: &[&str]) -> String {
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
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = command
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
        .args(args)
        .current_dir(path)
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
#[ignore = "child helper executed explicitly by recovery parent"]
fn worker_crash_child() {
    let manifest = env::var_os("ORCHESTRIX_RECOVERY_MANIFEST").expect("parent manifest");
    let request: ManualTaskRequest = serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
    let phase = env::var("ORCHESTRIX_RECOVERY_PHASE").unwrap();
    let home = PathBuf::from(env::var_os("ORCHESTRIX_RECOVERY_HOME").unwrap());
    let database = PathBuf::from(env::var_os("ORCHESTRIX_RECOVERY_DATABASE").unwrap());
    let mut store = Store::open(database).unwrap();
    let mut launch = RuntimeLaunch::fixture(
        PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
        vec!["--scenario".into(), "success".into()],
        home,
        request.repository_directory.clone(),
    );
    launch.request_timeout = Duration::from_secs(2);
    launch.turn_timeout = Duration::from_secs(3);
    let callback = |checkpoint| {
        if format!("{checkpoint:?}") == phase {
            std::process::exit(86);
        }
    };
    let outcome = if phase == "KeeperLostAfterRuntimeConnect" {
        ManualWorker::run_with_keeper_failure(
            &mut store,
            &request,
            launch,
            &CancellationToken::new(),
            callback,
        )
    } else {
        ManualWorker::run_with_checkpoint(
            &mut store,
            &request,
            launch,
            &CancellationToken::new(),
            callback,
        )
    };
    panic!("requested checkpoint {phase} not reached: {outcome:?}");
}

#[test]
fn crash_windows_reconcile_without_pid_adoption_or_automatic_dispatch_and_restore_context() {
    for phase in [
        WorkerCheckpoint::Reserved,
        WorkerCheckpoint::WorktreeCreated,
        WorkerCheckpoint::SupervisorIntentPersisted,
        WorkerCheckpoint::SupervisorReady,
        WorkerCheckpoint::SupervisorRecorded,
        WorkerCheckpoint::RuntimeConnected,
        WorkerCheckpoint::DispatchIntentPersisted,
        WorkerCheckpoint::DispatchConfirmed,
        WorkerCheckpoint::TerminalObserved,
        WorkerCheckpoint::RuntimeObserved,
        WorkerCheckpoint::ArtifactCaptured,
        WorkerCheckpoint::Checking,
        WorkerCheckpoint::CheckStarted,
        WorkerCheckpoint::KeeperLostAfterRuntimeConnect,
    ] {
        eprintln!("recovery crash checkpoint: {phase:?}");
        let fixture = Fixture::new();
        let mut request = fixture.request();
        if matches!(
            phase,
            WorkerCheckpoint::Checking | WorkerCheckpoint::CheckStarted
        ) {
            request.checks = vec![orchestrix_core::evaluator::CheckSpec {
                id: "crash-check".into(),
                executable: PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
                arguments: vec!["--check-hang".into()],
                authorization_ref: "explicit-crash-fixture-check".into(),
                authorized: true,
                timeout_ms: 2000,
                executable_sha256: None,
            }];
        }
        let manifest = fixture.root.join("task.json");
        fs::write(&manifest, serde_json::to_vec(&request).unwrap()).unwrap();
        let child = Command::new(env::current_exe().unwrap())
            .args(["--ignored", "--exact", "worker_crash_child", "--nocapture"])
            .env("ORCHESTRIX_RECOVERY_MANIFEST", &manifest)
            .env("ORCHESTRIX_RECOVERY_DATABASE", &fixture.database)
            .env("ORCHESTRIX_RECOVERY_HOME", &fixture.home)
            .env("ORCHESTRIX_RECOVERY_PHASE", format!("{phase:?}"))
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(86),
            "{phase:?}: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        // The unrelated process remains owned by the test, never by recovery.
        // It could occupy any PID formerly mentioned in external diagnostics.
        let mut foreign = ForeignChild(
            Command::new(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
                .args(["--scenario", "child-hold"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let count = fixture.threads();
        let dispatched = fs::read(fixture.home.join("dispatch-count")).ok();
        let mut store = Store::open(&fixture.database).unwrap();
        let before = store.load().unwrap();
        assert_eq!(before.workers().len(), 1);
        let original = before.workers()["recovery-worker"].clone();
        let original_context = original.intent.context.clone();
        let original_policy = original.intent.policy.clone();
        let artifacts = before.captured_artifacts().len();
        drop(before);
        let summary =
            recovery::reconcile(&mut store).unwrap_or_else(|error| panic!("{phase:?}: {error}"));
        let mut case = summary.cases[0].clone();
        if !matches!(
            phase,
            WorkerCheckpoint::Reserved
                | WorkerCheckpoint::WorktreeCreated
                | WorkerCheckpoint::SupervisorIntentPersisted
                | WorkerCheckpoint::KeeperLostAfterRuntimeConnect
        ) {
            let waiting = std::time::Instant::now();
            while !case.local_scope_exhausted && waiting.elapsed() < Duration::from_secs(5) {
                case = recovery::refresh_closure_evidence(&mut store, "recovery-worker").unwrap();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                case.local_scope_exhausted,
                "keeper must prove its own exact fenced Job after owner exit: {phase:?} {case:?}"
            );
        }
        assert!(summary.has_blocking_cases);
        assert!(!summary.ready_for_admission);
        let never = matches!(
            phase,
            WorkerCheckpoint::Reserved
                | WorkerCheckpoint::WorktreeCreated
                | WorkerCheckpoint::SupervisorIntentPersisted
                | WorkerCheckpoint::SupervisorReady
                | WorkerCheckpoint::SupervisorRecorded
                | WorkerCheckpoint::RuntimeConnected
                | WorkerCheckpoint::KeeperLostAfterRuntimeConnect
        );
        assert_eq!(
            case.dispatch == DispatchKnowledge::NeverSent,
            never,
            "{phase:?}: {case:?}"
        );
        assert_eq!(
            case.status,
            if never {
                WorkerStatus::Blocked
            } else {
                WorkerStatus::Uncertain
            }
        );
        if phase == WorkerCheckpoint::DispatchIntentPersisted {
            assert_eq!(case.dispatch, DispatchKnowledge::IntentOnly);
            assert!(!case.local_tree_stopped);
        }
        if phase == WorkerCheckpoint::RuntimeConnected {
            assert!(!case.local_tree_stopped);
            assert!(
                case.local_scope_exhausted && case.set_aside_eligible,
                "only keeper receipt supplies this independent local closure proof"
            );
        }
        if phase == WorkerCheckpoint::DispatchConfirmed {
            assert_eq!(case.dispatch, DispatchKnowledge::Confirmed);
            assert_eq!(case.remote_outcome, "unknown");
        }
        if matches!(
            phase,
            WorkerCheckpoint::RuntimeObserved | WorkerCheckpoint::ArtifactCaptured
        ) {
            assert_eq!(case.dispatch, DispatchKnowledge::OutcomeKnown);
            assert_eq!(case.remote_outcome, "completed");
            assert!(case.local_tree_stopped && case.streams_closed);
        }
        if phase == WorkerCheckpoint::TerminalObserved {
            assert_eq!(case.dispatch, DispatchKnowledge::OutcomeKnown);
            assert_eq!(case.remote_outcome, "completed");
            assert!(!case.local_tree_stopped);
            assert!(
                case.local_scope_exhausted && case.set_aside_eligible,
                "remote terminal alone is insufficient; keeper proof is separate"
            );
        }
        if phase == WorkerCheckpoint::KeeperLostAfterRuntimeConnect {
            assert!(!case.local_scope_exhausted);
            assert!(!case.local_tree_stopped);
            assert!(
                !case.set_aside_eligible,
                "lost keeper without receipt must preserve uncertainty"
            );
        }
        let pack =
            recovery::compile_continuity(&store, "recovery-worker", ContextBudget::default())
                .unwrap();
        assert_eq!(pack.as_json()["objective"], request.objective);
        assert_eq!(pack.as_json()["files"][0]["content"], "before\n");
        assert!(pack.rendered().contains("durable-continuity-checkpoint"));
        let facts: serde_json::Value = serde_json::from_str(
            pack.as_json()["sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["identity"]["reference"] == "durable-continuity-checkpoint")
                .unwrap()["content"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let state = store.load().unwrap();
        let task = state.runs()[&RunId::new(&original.intent.run_id).unwrap()]
            .task(&TaskId::new(&original.intent.task_id).unwrap())
            .unwrap();
        assert_eq!(
            facts["planRevision"],
            task.attempts()[0].inputs().plan_revision.as_str()
        );
        assert!(!facts["planRevision"].is_null());
        assert!(!pack.rendered().contains("fixture-thread-"));
        assert!(!pack.rendered().contains("adapterBinding"));
        assert_eq!(
            pack.as_json()["budget"]["actualPromptBytes"],
            pack.rendered().len()
        );
        let small = recovery::compile_continuity(
            &store,
            "recovery-worker",
            ContextBudget {
                max_files: 0,
                ..ContextBudget::default()
            },
        )
        .unwrap();
        assert_eq!(small.as_json()["files"].as_array().unwrap().len(), 0);
        assert_eq!(
            small.as_json()["omissions"][0]["reason"],
            "file-count-budget"
        );
        let after = store.load().unwrap();
        assert_eq!(
            after.workers()["recovery-worker"].intent.context,
            original_context
        );
        assert_eq!(
            after.workers()["recovery-worker"].intent.policy,
            original_policy
        );
        assert_eq!(after.captured_artifacts().len(), artifacts);
        let task = after.runs()[&RunId::new(&original.intent.run_id).unwrap()]
            .task(&TaskId::new(&original.intent.task_id).unwrap())
            .unwrap();
        if never {
            assert_eq!(task.attempts()[0].state(), AttemptState::Created);
        } else if matches!(
            phase,
            WorkerCheckpoint::Checking
                | WorkerCheckpoint::CheckStarted
                | WorkerCheckpoint::TerminalObserved
                | WorkerCheckpoint::RuntimeObserved
                | WorkerCheckpoint::ArtifactCaptured
        ) {
            assert_eq!(task.attempts()[0].state(), AttemptState::Succeeded);
        } else {
            assert_eq!(task.attempts()[0].state(), AttemptState::Uncertain);
        }
        assert_eq!(task.attempts().len(), 1);
        drop(after);
        let eligible = case.set_aside_eligible;
        let revision = store.load().unwrap().revision();
        assert!(
            recovery::intervene(
                &mut store,
                "recovery-worker",
                revision.saturating_sub(1),
                "explicit-inspection",
                RecoveryAction::RecordInspection
            )
            .is_err()
        );
        recovery::intervene(
            &mut store,
            "recovery-worker",
            revision,
            "explicit-inspection",
            RecoveryAction::RecordInspection,
        )
        .unwrap();
        assert!(
            recovery::snapshot(&store).unwrap().has_blocking_cases,
            "inspection alone cannot unlock"
        );
        let revision = store.load().unwrap().revision();
        let set_aside = recovery::intervene(
            &mut store,
            "recovery-worker",
            revision,
            "explicit-set-aside",
            RecoveryAction::SetAside,
        );
        assert_eq!(set_aside.is_ok(), eligible, "{phase:?}: {set_aside:?}");
        if !eligible {
            let revision = reopened_revision(&store);
            let forged=store.execute(&orchestrix_core::persistence::Request{
                scope:orchestrix_core::domain::LocalRef::new("recovery-worker").unwrap(),
                key:orchestrix_core::domain::LocalRef::new("forged-closure").unwrap(),expected_revision:revision,
                command:orchestrix_core::persistence::Command::Worker(orchestrix_core::worker_records::WorkerCommand::Observe{
                    id:"recovery-worker".into(),status:WorkerStatus::SetAside,
                    details:serde_json::json!({"authorizationRef":"claimed-inspection","resumeAutomatically":false,"remoteResultUnchanged":true,"treeStopped":true,"streamsClosed":true})
                })
            });
            assert!(
                forged.is_err(),
                "caller-provided booleans cannot replace supervisor evidence"
            );
            assert_eq!(reopened_revision(&store), revision);
        }
        assert_eq!(
            recovery::snapshot(&store).unwrap().ready_for_admission,
            eligible
        );
        drop(store);
        let mut reopened = Store::open(&fixture.database).unwrap();
        let second = recovery::reconcile(&mut reopened).unwrap();
        assert_eq!(second.ready_for_admission, eligible);
        let replay = ManualWorker::run(
            &mut reopened,
            &request,
            RuntimeLaunch::fixture(
                PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
                vec!["--scenario".into(), "quota-failure".into()],
                fixture.home.clone(),
                fixture.repository.clone(),
            ),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            replay.status(),
            if eligible {
                WorkerStatus::SetAside
            } else {
                WorkerStatus::InspectionRequired
            }
        );
        assert_eq!(fixture.threads(), count);
        assert_eq!(
            fs::read(fixture.home.join("dispatch-count")).ok(),
            dispatched
        );
        assert_eq!(
            fs::read_to_string(fixture.repository.join("fixture.txt")).unwrap(),
            "before\n"
        );
        assert!(
            foreign.0.try_wait().unwrap().is_none(),
            "recovery must not touch unrelated process"
        );
    }
}
fn reopened_revision(store: &Store) -> u64 {
    store.load().unwrap().revision()
}

// Build faithful legacy-contract rows through the normal reservation API. This
// is not an assertion that an old executable was run: no existing row, trigger,
// digest or snapshot is edited to manufacture a recovery proof.
fn legacy_reservation(
    state: &orchestrix_core::persistence::SavedState,
    protocol: Option<orchestrix_core::worker_records::WorkerLaunchProtocol>,
) -> orchestrix_core::worker_records::WorkerReservation {
    use orchestrix_core::domain::*;
    let mut intent = state.workers()["recovery-worker"].intent.clone();
    intent.launch_protocol = protocol;
    let original = state.runs()[&RunId::new(&intent.run_id).unwrap()]
        .task(&TaskId::new(&intent.task_id).unwrap())
        .unwrap();
    let attempt = &original.attempts()[0];
    assert_eq!(attempt.state(), AttemptState::Created);
    let project = Project::restore_checkpoint(
        &state.projects()[&ProjectId::new(&intent.project_id).unwrap()].checkpoint(),
    )
    .unwrap();
    let mut run = Run::new(
        RunId::new(&intent.run_id).unwrap(),
        Some(&project),
        RevisionId::new(&intent.base_oid).unwrap(),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(
        Task::new(
            original.id().clone(),
            original.objective(),
            original.kind(),
            original.acceptance_criteria().to_vec(),
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    run.make_ready(original.id()).unwrap();
    let session = attempt.session();
    orchestrix_core::worker_records::WorkerReservation {
        intent,
        project,
        connection: Connection::restore_checkpoint(
            &state.connections()[&(
                session.connection_id().clone(),
                session.connection_revision(),
            )]
                .checkpoint(),
        )
        .unwrap(),
        session: RuntimeSessionRef::restore_checkpoint(&session.checkpoint()).unwrap(),
        run,
        attempt: TaskAttempt::restore_checkpoint(&attempt.checkpoint()).unwrap(),
        snapshot: state.snapshots()[attempt.id()].clone(),
        definitions: state.records().definitions().to_vec(),
        diagnostics: state.records().diagnostics().to_vec(),
    }
}

fn effect(store: &mut Store, key: &str, command: orchestrix_core::persistence::Command) {
    let result = store
        .execute(&orchestrix_core::persistence::Request {
            scope: orchestrix_core::domain::LocalRef::new("legacy-proof-test").unwrap(),
            key: orchestrix_core::domain::LocalRef::new(key).unwrap(),
            expected_revision: store.load().unwrap().revision(),
            command,
        })
        .unwrap();
    assert_eq!(
        result.result.disposition,
        orchestrix_core::persistence::Disposition::Applied
    );
}

#[test]
fn legacy_launch_and_unrecorded_checks_never_gain_closure_from_missing_barriers() {
    use orchestrix_core::{
        domain::*,
        git::{CreateWorktree, WorktreeService},
        persistence::{Command as DurableCommand, RunAction},
        recovery_guardian::GuardianBinding,
        runtime::{AttemptExecution, CodexAdapter, RuntimeError},
        runtime_contract::normalize_request,
        worker_records::{WorkerCommand, WorkerLaunchProtocol},
    };
    use orchestrix_process::{OwnedProcess, ProcessLimits, SpawnSpec};
    use serde_json::json;

    let fixture = Fixture::new();
    let mut request = fixture.request();
    let marker = fixture.root.join("legacy-check-started");
    request.checks = vec![orchestrix_core::evaluator::CheckSpec {
        id: "legacy-check".into(),
        executable: PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
        arguments: vec!["--check-hang".into(), marker.to_string_lossy().into()],
        authorization_ref: "explicit-legacy-check-fixture".into(),
        authorized: true,
        timeout_ms: 90_000,
        executable_sha256: None,
    }];
    let manifest = fixture.root.join("seed-task.json");
    fs::write(&manifest, serde_json::to_vec(&request).unwrap()).unwrap();
    let child = Command::new(env::current_exe().unwrap())
        .args(["--ignored", "--exact", "worker_crash_child", "--nocapture"])
        .env("ORCHESTRIX_RECOVERY_MANIFEST", &manifest)
        .env("ORCHESTRIX_RECOVERY_DATABASE", &fixture.database)
        .env("ORCHESTRIX_RECOVERY_HOME", &fixture.home)
        .env("ORCHESTRIX_RECOVERY_PHASE", "Reserved")
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(86), "{child:?}");
    let seed = Store::open(&fixture.database).unwrap().load().unwrap();

    for case in [
        "legacy-reserved",
        "legacy-worktree",
        "guarded-reserved",
        "schema5-preparing",
        "unproved-preparing",
        "legacy-runtime-check",
    ] {
        eprintln!("legacy compatibility case: {case}");
        let database = fixture.root.join(format!("{case}.sqlite"));
        let mut store = Store::open(&database).unwrap();
        let protocol =
            (case == "guarded-reserved").then_some(WorkerLaunchProtocol::GuardedRuntimeAndChecksV1);
        let reservation = legacy_reservation(&seed, protocol);
        let intent_bytes = serde_json::to_string(&reservation.intent).unwrap();
        if protocol.is_none() {
            assert!(!intent_bytes.contains("launch_protocol"));
        }
        let snapshot_before = serde_json::to_value(&reservation.snapshot).unwrap();
        effect(
            &mut store,
            "reserve",
            DurableCommand::Worker(WorkerCommand::Reserve(Box::new(reservation))),
        );
        if matches!(case, "schema5-preparing" | "unproved-preparing") {
            let details = if case == "schema5-preparing" {
                let binding = GuardianBinding::prepare(
                    store.path(),
                    &seed.workers()["recovery-worker"].intent.attempt_id,
                    &orchestrix_core::recovery_guardian::default_helper().unwrap(),
                )
                .unwrap();
                json!({"guardian":binding,"runtimeSpawnRequiresSupervisorReady":true})
            } else {
                json!({"runtimeSpawnRequiresSupervisorReady":true})
            };
            effect(
                &mut store,
                "prepare",
                DurableCommand::Worker(WorkerCommand::Observe {
                    id: "recovery-worker".into(),
                    status: WorkerStatus::Preparing,
                    details,
                }),
            );
        }
        let mut own_check = None;
        if matches!(case, "legacy-worktree" | "legacy-runtime-check") {
            // The two independent databases must not claim the same Git refs
            // or checkout. Clone only this owned fixture locally, preserving
            // its exact commit while using a separate physical Git registry.
            let repository = if case == "legacy-runtime-check" {
                let repository = fixture.root.join("legacy-runtime-source");
                let ordinary = |path: &Path| {
                    path.to_string_lossy()
                        .strip_prefix(r"\\?\")
                        .unwrap_or(path.to_str().unwrap())
                        .to_owned()
                };
                git(
                    &fixture.root,
                    &[
                        "clone",
                        "--quiet",
                        "--no-hardlinks",
                        "--",
                        &ordinary(&fixture.repository),
                        &ordinary(&repository),
                    ],
                );
                repository
            } else {
                fixture.repository.clone()
            };
            let authorized = fixture.authorized.join(case);
            fs::create_dir(&authorized).unwrap();
            let state = store.load().unwrap();
            let intent = &state.workers()["recovery-worker"].intent;
            let task = state.runs()[&RunId::new(&intent.run_id).unwrap()]
                .task(&TaskId::new(&intent.task_id).unwrap())
                .unwrap();
            let attempt = &task.attempts()[0];
            let runtime_request = normalize_request(&json!({
                "version":{"major":1,"minor":1},"connectionId":request.connection_id,
                "connectionRevision":request.connection_revision,"sessionId":attempt.session().id().as_str(),
                "attemptId":intent.attempt_id,"contextPackRef":attempt.inputs().context.reference.as_str(),
                "billingMode":"subscription-only","paidFallback":"forbidden",
                "requested":attempt.runtime_snapshot()["requested"]
            }))
            .unwrap();
            let worktree = WorktreeService::new()
                .unwrap()
                .create(
                    &mut store,
                    &CreateWorktree {
                        scope: LocalRef::new("legacy-proof-test").unwrap(),
                        key: LocalRef::new(format!("create-{case}")).unwrap(),
                        expected_revision: state.revision(),
                        project: ProjectId::new(&intent.project_id).unwrap(),
                        repository: RepositoryId::new(&intent.repository_id).unwrap(),
                        run: RunId::new(&intent.run_id).unwrap(),
                        task: TaskId::new(&intent.task_id).unwrap(),
                        attempt: AttemptId::new(&intent.attempt_id).unwrap(),
                        directory_reference: LocalRef::new(format!(
                            "directory-{}",
                            request.project_id
                        ))
                        .unwrap(),
                        worktree_reference: attempt.inputs().worktree.clone(),
                        repository_path: repository,
                        authorized_directory: authorized,
                        base_commit: fixture.base.clone(),
                    },
                )
                .unwrap();
            if case == "legacy-runtime-check" {
                let diagnostic = &state.snapshots()[attempt.id()].diagnostics[0];
                effect(
                    &mut store,
                    "dispatch-intent",
                    DurableCommand::Worker(WorkerCommand::Observe {
                        id: "recovery-worker".into(),
                        status: WorkerStatus::DispatchPlanned,
                        details: json!({"admissionDiagnostic":diagnostic,"worktreeOperation":worktree.intent.operation_id}),
                    }),
                );
                let path = PathBuf::from(&worktree.intent.path);
                let launch = RuntimeLaunch::fixture(
                    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
                    vec!["--scenario".into(), "success".into()],
                    fixture.home.clone(),
                    path.clone(),
                );
                let mut adapter = CodexAdapter::connect(
                    launch,
                    &request.connection_id,
                    request.connection_revision,
                    100,
                    &CancellationToken::new(),
                )
                .unwrap();
                let mut sequence = 0;
                let report = adapter
                    .execute(
                        AttemptExecution {
                            request: runtime_request,
                            prompt: request.objective.clone(),
                            base_instructions: "Synthetic legacy compatibility proof".into(),
                            developer_instructions: "Only the owned fixture".into(),
                            resume: None,
                        },
                        &CancellationToken::new(),
                        |frame, origin| {
                            sequence += 1;
                            let receipt = store
                                .execute(&orchestrix_core::persistence::Request {
                                    scope: LocalRef::new("legacy-proof-test").unwrap(),
                                    key: LocalRef::new(format!("frame-{sequence}")).unwrap(),
                                    expected_revision: store.load().unwrap().revision(),
                                    command: DurableCommand::run(
                                        RunId::new(&intent.run_id).unwrap(),
                                        RunAction::Observe {
                                            task: TaskId::new(&intent.task_id).unwrap(),
                                            frame: frame.to_string(),
                                            origin,
                                        },
                                    ),
                                })
                                .map_err(|_| RuntimeError::observation_failed())?;
                            if receipt.result.disposition
                                != orchestrix_core::persistence::Disposition::Applied
                            {
                                return Err(RuntimeError::observation_failed());
                            }
                            Ok(())
                        },
                    )
                    .unwrap();
                assert_eq!(report.fault, None);
                assert!(report.tree.is_some_and(|tree| tree.active_processes == 0));
                assert!(
                    report.streams.stdin_closed
                        && report.streams.stdout_closed
                        && report.streams.stderr_closed
                );
                effect(
                    &mut store,
                    "runtime-observed",
                    DurableCommand::Worker(WorkerCommand::Observe {
                        id: "recovery-worker".into(),
                        status: WorkerStatus::RuntimeObserved,
                        details: json!({"treeStopped":true,"streamsClosed":true,"worktreeOperation":worktree.intent.operation_id}),
                    }),
                );
                // V4 could now start its authorized check without recording
                // Checking. Its old runtime closure is not closure of this Job.
                request.checks[0].validate().unwrap();
                let mut check = OwnedProcess::spawn(SpawnSpec {
                    executable: request.checks[0].executable.clone(),
                    arguments: request.checks[0].arguments.iter().map(Into::into).collect(),
                    cwd: path,
                    environment: env::vars_os()
                        .filter(|(key, _)| {
                            ["PATH", "SystemRoot", "SystemDrive", "WINDIR", "TEMP", "TMP"]
                                .iter()
                                .any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))
                        })
                        .collect(),
                    limits: ProcessLimits {
                        lifetime: Duration::from_millis(request.checks[0].timeout_ms),
                        ..ProcessLimits::default()
                    },
                })
                .unwrap();
                check.close_input();
                let start = std::time::Instant::now();
                while !marker.is_file() {
                    assert!(start.elapsed() < Duration::from_secs(3));
                    std::thread::sleep(Duration::from_millis(5));
                }
                assert!(check.tree().unwrap().active_processes > 0);
                own_check = Some(check);
            }
        }
        let summary = recovery::reconcile(&mut store).unwrap();
        let case_result = &summary.cases[0];
        let eligible = matches!(case, "guarded-reserved" | "schema5-preparing");
        assert_eq!(case_result.set_aside_eligible, eligible, "{case}");
        assert!(!case_result.local_scope_exhausted);
        if case == "legacy-runtime-check" {
            assert!(case_result.local_tree_stopped && case_result.streams_closed);
        }
        let revision = store.load().unwrap().revision();
        assert_eq!(
            recovery::intervene(
                &mut store,
                "recovery-worker",
                revision,
                "explicit-legacy-inspection",
                RecoveryAction::SetAside,
            )
            .is_ok(),
            eligible,
            "{case}"
        );
        if let Some(check) = &mut own_check {
            assert!(check.tree().unwrap().active_processes > 0);
            assert_eq!(
                check
                    .terminate(Duration::from_secs(3))
                    .unwrap()
                    .active_processes,
                0
            );
        }
        let after = store.load().unwrap();
        assert_eq!(
            serde_json::to_string(&after.workers()["recovery-worker"].intent).unwrap(),
            intent_bytes
        );
        assert_eq!(
            serde_json::to_value(after.snapshots().values().next().unwrap()).unwrap(),
            snapshot_before
        );
        drop(store);
        let reopened = Store::open(&database).unwrap();
        assert_eq!(
            serde_json::to_string(&reopened.load().unwrap().workers()["recovery-worker"].intent)
                .unwrap(),
            intent_bytes
        );
        assert_eq!(
            recovery::snapshot(&reopened).unwrap().ready_for_admission,
            eligible
        );
    }
}
