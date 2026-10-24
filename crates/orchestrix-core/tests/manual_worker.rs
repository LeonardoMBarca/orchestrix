//! Integrated synthetic runtime with real Windows processes, Git and SQLite.
//! No provider account, inference or project scripts are used by these fixtures.
#![cfg(windows)]
use orchestrix_core::domain::{AttemptState, RunId, TaskId, TaskState};
use orchestrix_core::evaluator::CheckSpec;
use orchestrix_core::persistence::Store;
use orchestrix_core::policy::{Fallback, PolicyDocument, TaskClassification, TaskKind};
use orchestrix_core::runtime::{CancellationToken, RuntimeLaunch};
use orchestrix_core::worker::{ContextFileRequest, ManualTaskRequest, ManualWorker};
use orchestrix_core::worker_records::WorkerStatus;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    authorized: PathBuf,
    home: PathBuf,
    database: PathBuf,
    base: String,
}
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
            "orchestrix-worker-espa\u{e7}o-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("main repository");
        let authorized = root.join("authorized attempts");
        let home = root.join("synthetic auth store");
        for path in [&repository, &authorized, &home] {
            fs::create_dir(path).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), "before\n").unwrap();
        fs::write(
            repository.join("instructions.md"),
            "Only edit fixture.txt.\n",
        )
        .unwrap();
        git(&repository, &["add", "--all"]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "Manual worker fixture"],
        );
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        Self {
            database: root.join("state.sqlite"),
            root,
            repository,
            authorized,
            home,
            base,
        }
    }
    fn store(&self) -> Store {
        Store::open(&self.database).unwrap()
    }
    fn launch(&self, scenario: &str) -> RuntimeLaunch {
        let mut launch = RuntimeLaunch::fixture(
            binary(),
            vec!["--scenario".into(), scenario.into()],
            self.home.clone(),
            self.repository.clone(),
        );
        launch.request_timeout = Duration::from_secs(2);
        launch.turn_timeout = Duration::from_secs(3);
        launch.interrupt_grace = Duration::from_millis(300);
        launch
    }
    fn request(&self, id: &str) -> ManualTaskRequest {
        let mut policy = PolicyDocument::default();
        // Explicit policy permission, not inferred model entitlement.
        policy.mode_defaults.fallback = Some(Fallback::Compatible);
        ManualTaskRequest {
            editable_paths: None,
            schema_version: 1,
            id: id.into(),
            project_id: "fixture-project".into(),
            repository_id: "fixture-repository".into(),
            repository_directory: self.repository.clone(),
            authorized_worktree_directory: self.authorized.clone(),
            base_commit: self.base.clone(),
            connection_id: "fixture-account".into(),
            connection_revision: 1,
            objective: "fixture-edit: correct only the owned fixture".into(),
            acceptance_criteria: vec![
                "fixture.txt contains exactly after followed by newline".into(),
            ],
            policy,
            classification: TaskClassification::routine(TaskKind::Implementation, "manual-user"),
            context_files: vec![ContextFileRequest {
                path: "fixture.txt".into(),
                authorization_ref: "read-task-file".into(),
                required: true,
            }],
            instructions: vec![
                "Use only the owned worktree; do not delegate or use the network.".into(),
            ],
            checks: vec![],
        }
    }
    fn thread_count(&self) -> usize {
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
        let temporary = fs::canonicalize(env::temp_dir()).unwrap();
        assert_eq!(root.parent(), Some(temporary.as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-worker-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn cli(fixture: &Fixture, arguments: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
        .args(arguments)
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn check(arguments: &[&str]) -> CheckSpec {
    CheckSpec {
        id: "fixture-check".into(),
        executable: binary(),
        arguments: arguments.iter().map(|s| (*s).into()).collect(),
        authorization_ref: "explicit-check-authorization".into(),
        authorized: true,
        timeout_ms: 2000,
        executable_sha256: None,
    }
}
fn git(root: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new("git");
    command.env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let result = command
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
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "fixture Git failed: {arguments:?} {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[test]
fn manual_worker_runs_policy_context_reservation_worktree_runtime_capture_and_checks_durably() {
    let fixture = Fixture::new();
    // Preserve real pre-existing staged, unstaged and untracked work in the main tree.
    fs::write(fixture.repository.join("fixture.txt"), "local staged\n").unwrap();
    git(&fixture.repository, &["add", "fixture.txt"]);
    fs::write(fixture.repository.join("fixture.txt"), "local unstaged\n").unwrap();
    fs::write(
        fixture.repository.join("untracked.txt"),
        "local untracked\n",
    )
    .unwrap();
    let main_status = git(&fixture.repository, &["status", "--porcelain=v1", "-z"]).stdout;
    let main_index = fs::read(fixture.repository.join(".git/index")).unwrap();
    let mut request = fixture.request("full-worker");
    request.checks = vec![check(&["--check-fixture", "after\n"])];
    request
        .policy
        .mode_requirements
        .required_checks
        .insert("fixture-check".into());
    request.policy.mode_requirements.require_review = true;
    let manifest = fixture.root.join("manual-task.json");
    fs::write(&manifest, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
    let output = cli(
        &fixture,
        &[
            "run",
            "--database",
            fixture.database.to_str().unwrap(),
            "--task",
            manifest.to_str().unwrap(),
            "--synthetic-runtime",
            binary().to_str().unwrap(),
            "--codex-home",
            fixture.home.to_str().unwrap(),
            "--scenario",
            // The real pinned provider emits this global notification before
            // turn start; it must also pass the full durable CLI/check path.
            "remote-status-global",
        ],
    );
    assert_eq!(output["status"], "verified");
    let store = fixture.store();
    let record = store
        .load()
        .unwrap()
        .workers()
        .get("full-worker")
        .unwrap()
        .clone();
    assert_eq!(
        record.status(),
        WorkerStatus::Verified,
        "{}",
        record.summary()
    );
    let state = store.load().unwrap();
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id).unwrap())
        .unwrap();
    let task = run
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::NeedsReview);
    assert_eq!(task.verifications().len(), 1);
    assert!(task.artifact().is_some());
    let artifact = state
        .captured_artifacts()
        .values()
        .find(|a| a.attempt_id == record.intent.attempt_id)
        .unwrap();
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.files[0].path, "fixture.txt");
    assert_eq!(artifact.files[0].base.as_ref().unwrap().content, "before\n");
    assert_eq!(
        artifact.files[0].worktree.as_ref().unwrap().content,
        "after\n"
    );
    assert_eq!(
        task.verifications()[0].artifact.revision().as_str(),
        artifact.revision
    );
    assert_eq!(
        record.intent.policy["route"]["value"]["model_id"],
        serde_json::Value::Null
    );
    assert_eq!(record.intent.context["files"].as_array().unwrap().len(), 1);
    assert_eq!(
        record.intent.context["files"][0]["content"],
        "local unstaged\n"
    );
    assert_eq!(
        record.intent.context["files"][0]["source"]["revision"],
        orchestrix_core::git_records::content_hash(b"local unstaged\n")
    );
    assert_eq!(
        main_index,
        fs::read(fixture.repository.join(".git/index")).unwrap()
    );
    assert_eq!(
        main_status,
        git(&fixture.repository, &["status", "--porcelain=v1", "-z"]).stdout
    );
    assert_eq!(
        fs::read_to_string(fixture.repository.join("fixture.txt")).unwrap(),
        "local unstaged\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.repository.join("untracked.txt")).unwrap(),
        "local untracked\n"
    );
    let worker_summary = record.summary();
    assert!(!worker_summary.to_string().contains("fixture-thread-"));
    let mut observed_view = record.clone();
    let runtime_observation = observed_view
        .observations
        .iter()
        .position(|o| o.status == WorkerStatus::RuntimeObserved)
        .unwrap();
    assert!(
        observed_view.observations[runtime_observation]
            .details
            .get("adapterBinding")
            .is_some()
    );
    observed_view.observations.truncate(runtime_observation + 1);
    let observed_summary = observed_view.summary().to_string();
    assert!(!observed_summary.contains("adapterBinding"));
    assert!(!observed_summary.contains("fixture-thread-"));
    assert!(!observed_summary.contains("nativeThread"));
    drop(state);
    drop(store);
    let inspected_output = cli(
        &fixture,
        &[
            "status",
            "--database",
            fixture.database.to_str().unwrap(),
            "--id",
            "full-worker",
        ],
    );
    assert_eq!(inspected_output["status"], "verified");
    let replay_output = cli(
        &fixture,
        &[
            "run",
            "--database",
            fixture.database.to_str().unwrap(),
            "--task",
            manifest.to_str().unwrap(),
        ],
    );
    assert_eq!(replay_output["status"], "verified");
    let reopened = fixture.store();
    let inspected = reopened
        .load()
        .unwrap()
        .workers()
        .get("full-worker")
        .unwrap()
        .clone();
    assert_eq!(inspected.status(), WorkerStatus::Verified);
    assert_eq!(fixture.thread_count(), 1);
    assert_eq!(inspected.intent.context, record.intent.context);
    assert_eq!(inspected.intent.policy, record.intent.policy);
    for entry in fs::read_dir(&fixture.root).unwrap() {
        let entry = entry.unwrap();
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("state.sqlite")
        {
            let bytes = fs::read(entry.path()).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            for private in [
                "remoteControl/status/changed",
                "do-not-persist-server-name",
                "do-not-persist-installation-id",
                "do-not-persist-environment-id",
            ] {
                assert!(!text.contains(private));
            }
        }
    }
}

#[test]
fn seven_model_catalog_reserves_executes_and_reopens_without_truncating_evidence() {
    let fixture = Fixture::new();
    let mut request = fixture.request("seven-model-worker");
    request.checks = vec![check(&["--check-fixture", "after\n"])];
    let mut store = fixture.store();
    let record = ManualWorker::run(
        &mut store,
        &request,
        fixture.launch("catalog-seven"),
        &CancellationToken::new(),
    )
    .expect("seven-model catalog must fit the bounded atomic worker reservation");
    assert_eq!(
        record.status(),
        WorkerStatus::Verified,
        "{}",
        record.summary()
    );
    let state = store.load().unwrap();
    assert_eq!(state.workers().len(), 1);
    assert_eq!(state.connections().len(), 1);
    assert_eq!(state.worktrees().len(), 1);
    let descriptor = state
        .connections()
        .values()
        .next()
        .unwrap()
        .descriptor()
        .as_json()
        .clone();
    let models = descriptor["models"].as_array().unwrap();
    assert_eq!(models.len(), 7);
    assert_eq!(descriptor["catalog"]["completeness"], "complete");
    for model in models {
        assert_eq!(model["capabilities"].as_object().unwrap().len(), 11);
        assert_eq!(model["availability"], "unknown");
    }
    let policy_observation = &record.intent.policy["runtimeObservations"][0];
    assert_eq!(policy_observation["descriptor"], descriptor);
    assert_eq!(
        record.intent.policy["candidates"].as_array().unwrap().len(),
        8,
        "retain all seven model exclusions and the explicit runtime-default candidate"
    );
    let restored_policy =
        orchestrix_core::policy::EffectivePolicySnapshot::from_json(&record.intent.policy).unwrap();
    assert_eq!(restored_policy.as_json(), &record.intent.policy);
    let run = &state.runs()[&RunId::new(&record.intent.run_id).unwrap()];
    let task = run
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::NeedsReview);
    assert_eq!(task.attempts().len(), 1);
    assert_eq!(task.verifications().len(), 1);
    assert_eq!(
        task.verifications()[0].result,
        orchestrix_core::domain::CheckResult::Passed
    );
    let snapshot = state.snapshots().values().next().unwrap().clone();
    assert_eq!(snapshot.definition.attempt_id, record.intent.attempt_id);
    assert_eq!(snapshot.diagnostics.len(), 1);
    assert_eq!(
        state
            .records()
            .definition(&snapshot.definition.configuration)
            .unwrap()
            .body,
        record.intent.policy
    );
    let diagnostics = state.records().diagnostics().to_vec();
    assert_eq!(
        diagnostics.len(),
        2,
        "decision and fresh admission are retained"
    );
    for diagnostic in &diagnostics {
        assert!(diagnostic.complete);
        assert_eq!(
            diagnostic.outcome,
            orchestrix_core::persistence_records::DiagnosticOutcome::Success
        );
        assert_eq!(diagnostic.values["descriptor"], descriptor);
    }
    assert_eq!(
        state
            .records()
            .diagnostic(&snapshot.diagnostics[0])
            .unwrap()
            .values["observations"],
        policy_observation["observations"]
    );
    drop(state);
    drop(store);
    let mut reopened = fixture.store();
    let inspected = ManualWorker::inspect(&mut reopened, &request.id).unwrap();
    assert_eq!(inspected, record);
    let replayed = ManualWorker::run(
        &mut reopened,
        &request,
        fixture.launch("catalog-seven"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(replayed, record);
    let saved = reopened.load().unwrap();
    assert_eq!(saved.workers().len(), 1);
    assert_eq!(saved.worktrees().len(), 1);
    assert_eq!(saved.snapshots().values().next().unwrap(), &snapshot);
    assert_eq!(saved.records().diagnostics(), diagnostics.as_slice());
    assert_eq!(
        saved
            .connections()
            .values()
            .next()
            .unwrap()
            .descriptor()
            .as_json(),
        &descriptor
    );
    assert_eq!(fixture.thread_count(), 1);
    assert_eq!(
        fs::read_to_string(fixture.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
}

#[test]
fn failing_authorized_check_prevents_green_result_and_retains_artifact() {
    let fixture = Fixture::new();
    let mut request = fixture.request("failed-check");
    let omitted_marker = fixture.home.join("omitted-check-must-not-start");
    let mut second = check(&["--check-hang", omitted_marker.to_str().unwrap()]);
    second.id = "second-check".into();
    request.checks = vec![check(&["--check-fixture", "after\n"]), second];
    let mut store = fixture.store();
    let record = ManualWorker::run(
        &mut store,
        &request,
        fixture.launch("check-failed"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(
        record.status(),
        WorkerStatus::NeedsWork,
        "{}",
        record.summary()
    );
    let details = &record.observations.last().unwrap().details;
    assert_eq!(details["checks"].as_array().unwrap().len(), 1);
    assert_eq!(details["checksOmitted"].as_array().unwrap().len(), 1);
    assert_eq!(details["checksOmitted"][0]["id"], "second-check");
    assert!(
        !omitted_marker.exists(),
        "a failed check cannot silently launch another evaluator"
    );
    let state = store.load().unwrap();
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id).unwrap())
        .unwrap();
    let task = run
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::NeedsWork);
    assert_eq!(task.verifications().len(), 1);
    assert_eq!(
        task.verifications()[0].result,
        orchestrix_core::domain::CheckResult::Failed
    );
    assert!(
        state
            .captured_artifacts()
            .values()
            .any(|a| a.attempt_id == record.intent.attempt_id)
    );
    drop(state);
    drop(store);
    let mut reopened = fixture.store();
    assert_eq!(
        ManualWorker::inspect(&mut reopened, "failed-check")
            .unwrap()
            .status(),
        WorkerStatus::NeedsWork
    );
    assert_eq!(fixture.thread_count(), 1);
}

#[test]
fn check_mutations_including_write_and_restore_invalidate_the_bound_artifact() {
    for mode in ["--mutate-fixture-check", "--mutate-restore-fixture-check"] {
        let fixture = Fixture::new();
        let mut request = fixture.request("mutating-check");
        request.checks = vec![check(&[mode])];
        let mut store = fixture.store();
        let record = ManualWorker::run(
            &mut store,
            &request,
            fixture.launch("success"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            record.status(),
            WorkerStatus::Invalidated,
            "{mode} {}",
            record.summary()
        );
        let check_report = &record.observations.last().unwrap().details["checks"][0];
        assert_eq!(check_report["exit_code"], 0);
        assert_eq!(
            check_report["result"], "invalidated",
            "successful process exit cannot verify bytes changed by the check"
        );
        let state = store.load().unwrap();
        let run = state
            .runs()
            .get(&RunId::new(&record.intent.run_id).unwrap())
            .unwrap();
        let task = run
            .task(&TaskId::new(&record.intent.task_id).unwrap())
            .unwrap();
        assert!(!matches!(
            task.state(),
            TaskState::Approved | TaskState::Completed
        ));
        assert!(
            !task
                .verifications()
                .iter()
                .any(|v| v.result == orchestrix_core::domain::CheckResult::Passed)
        );
    }
}

#[test]
fn crash_and_lost_response_reopen_without_automatic_model_execution() {
    for scenario in ["crash", "lost-reply"] {
        let fixture = Fixture::new();
        let request = fixture.request("uncertain-worker");
        let mut store = fixture.store();
        let record = ManualWorker::run(
            &mut store,
            &request,
            fixture.launch(scenario),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            record.status(),
            WorkerStatus::Uncertain,
            "{scenario} {}",
            record.summary()
        );
        assert_eq!(fixture.thread_count(), 1);
        let state = store.load().unwrap();
        let run = state
            .runs()
            .get(&RunId::new(&record.intent.run_id).unwrap())
            .unwrap();
        let task = run
            .task(&TaskId::new(&record.intent.task_id).unwrap())
            .unwrap();
        assert_eq!(task.attempts()[0].state(), AttemptState::Uncertain);
        assert!(task.verifications().is_empty());
        let attempts = task.attempts().len();
        drop(state);
        drop(store);
        let mut reopened = fixture.store();
        let inspected = ManualWorker::inspect(&mut reopened, "uncertain-worker").unwrap();
        assert_eq!(inspected.status(), WorkerStatus::Uncertain);
        let replay = ManualWorker::run(
            &mut reopened,
            &request,
            fixture.launch("success"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(replay.status(), WorkerStatus::Uncertain);
        assert_eq!(fixture.thread_count(), 1);
        let mut other = request.clone();
        other.id = "another-manual-task".into();
        assert!(
            ManualWorker::run(
                &mut reopened,
                &other,
                fixture.launch("success"),
                &CancellationToken::new()
            )
            .is_err(),
            "an unresolved attempt must retain the durable serial admission slot"
        );
        assert_eq!(fixture.thread_count(), 1);
        assert_eq!(
            fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1"
        );
        let state = reopened.load().unwrap();
        let run = state
            .runs()
            .get(&RunId::new(&record.intent.run_id).unwrap())
            .unwrap();
        assert_eq!(
            run.task(&TaskId::new(&record.intent.task_id).unwrap())
                .unwrap()
                .attempts()
                .len(),
            attempts
        );
    }
}

#[test]
fn protocol_diagnostic_is_durable_private_and_does_not_authorize_reexecution() {
    let fixture = Fixture::new();
    let request = fixture.request("diagnostic-worker");
    let mut store = fixture.store();
    let record = ManualWorker::run(
        &mut store,
        &request,
        fixture.launch("unscoped-private"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(record.status(), WorkerStatus::Uncertain);
    let observed = record
        .observations
        .iter()
        .find(|entry| entry.status == WorkerStatus::RuntimeObserved)
        .unwrap();
    assert_eq!(observed.details["fault"], "protocol");
    assert_eq!(observed.details["dispatch"], "sent");
    assert_eq!(observed.details["treeStopped"], true);
    assert_eq!(observed.details["streamsClosed"], true);
    let diagnostic = observed.details["adapterDiagnostic"].clone();
    assert!(diagnostic.is_object());
    assert!(diagnostic.to_string().len() <= 4_096);
    let method_hash = orchestrix_core::git_records::content_hash(b"private.method.fixture-value");
    assert_eq!(diagnostic["methodSha256"], method_hash);
    for private in [
        "private.method.fixture-value",
        "do-not-persist-provider-message",
    ] {
        assert!(!diagnostic.to_string().contains(private));
    }
    assert!(!record.summary().to_string().contains(&method_hash));
    let state = store.load().unwrap();
    let task = state.runs()[&RunId::new(&record.intent.run_id).unwrap()]
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::Blocked);
    assert!(task.verifications().is_empty());
    drop(state);
    drop(store);
    let mut reopened = fixture.store();
    let inspected = ManualWorker::inspect(&mut reopened, &request.id).unwrap();
    assert_eq!(inspected, record);
    let replay = ManualWorker::run(
        &mut reopened,
        &request,
        fixture.launch("success"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(replay, record);
    assert_eq!(fixture.thread_count(), 1);
    assert_eq!(
        fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
        "1"
    );
    drop(reopened);
    for entry in fs::read_dir(&fixture.root).unwrap() {
        let entry = entry.unwrap();
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("state.sqlite")
        {
            let bytes = fs::read(entry.path()).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains("private.method.fixture-value"));
            assert!(!text.contains("do-not-persist-provider-message"));
        }
    }
    assert_eq!(
        fs::read_to_string(fixture.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
}

#[test]
fn capture_failures_preserve_stage_codes_and_never_redispatch() {
    use orchestrix_core::domain::LocalRef;
    use orchestrix_core::persistence::{Command as StoreCommand, Disposition, Request};
    use orchestrix_core::persistence_records::{DefinitionInput, DefinitionKind, RecordsCommand};
    use orchestrix_core::worker::WorkerCheckpoint;
    use serde_json::json;

    for (id, phase, stage, git_code, storage_code, scenario, reason) in [
        (
            "capture-inspect-failure",
            WorkerCheckpoint::CaptureInspecting,
            "inspect",
            "process",
            None,
            "success",
            "git-inspection-required",
        ),
        (
            "capture-read-failure",
            WorkerCheckpoint::CaptureReading,
            "capture",
            "unsupported",
            None,
            "success",
            "git-inspection-required",
        ),
        (
            "capture-persist-failure",
            WorkerCheckpoint::CapturePersisting,
            "persist",
            "storage",
            Some("stale-revision"),
            "success",
            "git-inspection-required",
        ),
        (
            "capture-runtime-uncertain",
            WorkerCheckpoint::CaptureReading,
            "capture",
            "unsupported",
            None,
            "lost-reply",
            "runtime-outcome-not-confirmed",
        ),
    ] {
        let fixture = Fixture::new();
        let request = fixture.request(id);
        let mut store = fixture.store();
        let mut injected = false;
        let mut capture_stages = vec![];
        let record = ManualWorker::run_with_checkpoint(
            &mut store,
            &request,
            fixture.launch(scenario),
            &CancellationToken::new(),
            |checkpoint| {
                if matches!(
                    checkpoint,
                    WorkerCheckpoint::CaptureInspecting
                        | WorkerCheckpoint::CaptureReading
                        | WorkerCheckpoint::CapturePersisting
                ) {
                    capture_stages.push(checkpoint);
                }
                if checkpoint != phase || injected {
                    return;
                }
                injected = true;
                let worktree = fs::read_dir(&fixture.authorized)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| path.is_dir())
                    .unwrap();
                match checkpoint {
                    WorkerCheckpoint::CaptureInspecting => {
                        fs::write(
                            worktree.join(".git"),
                            "gitdir: not-for-diagnostics-CAPTURE-CANARY\n",
                        )
                        .unwrap();
                    }
                    WorkerCheckpoint::CaptureReading => {
                        fs::write(
                            worktree.join("private-binary.txt"),
                            b"not-for-diagnostics-CAPTURE-CANARY\0",
                        )
                        .unwrap();
                    }
                    WorkerCheckpoint::CapturePersisting => {
                        // A separate writer can commit here because Git and
                        // capture hold no SQLite writer transaction. The old
                        // expected revision must still fail, without a retry.
                        let mut other = fixture.store();
                        let revision = other.load().unwrap().revision();
                        let receipt = other
                            .execute(&Request {
                                scope: LocalRef::new("capture-test").unwrap(),
                                key: LocalRef::new("concurrent-definition").unwrap(),
                                expected_revision: revision,
                                command: StoreCommand::Records(RecordsCommand::AddDefinition(
                                    DefinitionInput {
                                        kind: DefinitionKind::Configuration,
                                        id: "capture-concurrent-definition".into(),
                                        version: 1,
                                        body: json!({"purpose":"capture-stage-proof"}),
                                        references: vec![],
                                    },
                                )),
                            })
                            .unwrap();
                        assert_eq!(receipt.result.disposition, Disposition::Applied);
                    }
                    _ => unreachable!(),
                }
            },
        )
        .unwrap();
        assert!(
            injected,
            "{id}: the requested stage must actually be reached"
        );
        let stages = [
            WorkerCheckpoint::CaptureInspecting,
            WorkerCheckpoint::CaptureReading,
            WorkerCheckpoint::CapturePersisting,
        ];
        let reached = stages.iter().position(|stage| *stage == phase).unwrap() + 1;
        assert_eq!(
            capture_stages,
            stages[..reached],
            "{id}: capture never retries"
        );
        assert_eq!(record.status(), WorkerStatus::Uncertain, "{id}");
        let details = &record.observations.last().unwrap().details;
        assert_eq!(details["reason"], reason, "{id}");
        let mut diagnostic = json!({"schemaVersion":1,"stage":stage,"gitCode":git_code});
        if let Some(code) = storage_code {
            diagnostic["storageCode"] = json!(code);
        }
        assert_eq!(details["captureDiagnostic"], diagnostic, "{id}");
        assert_eq!(
            record.summary()["details"]["captureDiagnostic"],
            diagnostic,
            "{id}"
        );
        assert!(!details.to_string().contains("CAPTURE-CANARY"));
        assert!(
            !details
                .to_string()
                .contains(fixture.root.to_string_lossy().as_ref())
        );
        let state = store.load().unwrap();
        assert!(state.captured_artifacts().is_empty(), "{id}");
        let task = state.runs()[&RunId::new(&record.intent.run_id).unwrap()]
            .task(&TaskId::new(&record.intent.task_id).unwrap())
            .unwrap();
        assert_eq!(task.attempts().len(), 1, "{id}");
        assert!(task.verifications().is_empty(), "{id}");
        assert!(!matches!(
            task.state(),
            TaskState::Approved | TaskState::Completed
        ));
        drop(state);
        drop(store);
        let mut reopened = fixture.store();
        assert_eq!(ManualWorker::inspect(&mut reopened, id).unwrap(), record);
        let replay = ManualWorker::run(
            &mut reopened,
            &request,
            fixture.launch("success"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(replay, record, "{id}");
        assert_eq!(fixture.thread_count(), 1, "{id}");
        assert_eq!(
            fs::read_to_string(fixture.home.join("dispatch-count")).unwrap(),
            "1",
            "{id}"
        );
        assert!(reopened.load().unwrap().captured_artifacts().is_empty());
        drop(reopened);
        for entry in fs::read_dir(&fixture.root).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("state.sqlite")
            {
                assert!(
                    !String::from_utf8_lossy(&fs::read(entry.path()).unwrap())
                        .contains("CAPTURE-CANARY"),
                    "{id}: rejected content and Git stderr must not enter SQLite"
                );
            }
        }
    }
}

#[test]
fn required_sensitive_context_is_rejected_before_worktree_and_model_dispatch() {
    let fixture = Fixture::new();
    let secret = "API_KEY=sk-syntheticfixturecredential123456";
    fs::write(fixture.repository.join(".env"), secret).unwrap();
    let mut request = fixture.request("unsafe-context");
    request.context_files = vec![ContextFileRequest {
        path: ".env".into(),
        authorization_ref: "explicit-read".into(),
        required: true,
    }];
    let mut store = fixture.store();
    assert!(
        ManualWorker::run(
            &mut store,
            &request,
            fixture.launch("success"),
            &CancellationToken::new()
        )
        .is_err()
    );
    assert_eq!(fixture.thread_count(), 0);
    assert!(store.load().unwrap().worktrees().is_empty());
    drop(store);
    let database = fs::read(&fixture.database).unwrap();
    assert!(!String::from_utf8_lossy(&database).contains(secret));
    assert_eq!(
        fs::read_to_string(fixture.repository.join(".env")).unwrap(),
        secret
    );
}

#[test]
fn cooperative_cancellation_is_durable_and_preserves_the_owned_worktree() {
    let fixture = Fixture::new();
    let request = fixture.request("cancelled-worker");
    let token = CancellationToken::new();
    let trigger = token.clone();
    let dispatched = fixture.home.join("dispatch-count");
    let status_database = fixture.database.clone();
    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    let canceller = std::thread::spawn(move || {
        // Observe the actual dispatch, not a guessed duration of Git preparation.
        // The worker's bounded operations and finished flag bound this observer.
        while !done.load(Ordering::Acquire) {
            if dispatched.is_file() {
                let output = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
                    .args(["status", "--database"])
                    .arg(&status_database)
                    .args(["--id", "cancelled-worker"])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "active CLI status failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(
                    status["status"], "dispatch-planned",
                    "status must inspect the running host without marking it lost"
                );
                trigger.cancel();
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    });
    let mut store = fixture.store();
    let result = ManualWorker::run(&mut store, &request, fixture.launch("hang"), &token);
    finished.store(true, Ordering::Release);
    let cancellation_sent = canceller.join().unwrap();
    let record = result.unwrap();
    assert!(
        cancellation_sent,
        "worker never reached authorized synthetic dispatch"
    );
    assert_eq!(
        record.status(),
        WorkerStatus::Cancelled,
        "{}",
        record.summary()
    );
    let state = store.load().unwrap();
    let worktree = state
        .worktrees()
        .values()
        .find(|w| w.intent.attempt_id == record.intent.attempt_id)
        .unwrap();
    assert!(Path::new(&worktree.intent.path).is_dir());
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id).unwrap())
        .unwrap();
    let task = run
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.attempts()[0].state(), AttemptState::Cancelled);
    assert!(task.verifications().is_empty());
    drop(state);
    drop(store);
    let mut reopened = fixture.store();
    assert_eq!(
        ManualWorker::inspect(&mut reopened, "cancelled-worker")
            .unwrap()
            .status(),
        WorkerStatus::Cancelled
    );
    assert_eq!(fixture.thread_count(), 1);
}

#[test]
fn a_later_worktree_change_invalidates_verification_without_rerunning_the_runtime() {
    let fixture = Fixture::new();
    let mut request = fixture.request("later-change");
    request.checks = vec![check(&["--check-fixture", "after\n"])];
    let mut store = fixture.store();
    let record = ManualWorker::run(
        &mut store,
        &request,
        fixture.launch("success"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(record.status(), WorkerStatus::Verified);
    let state = store.load().unwrap();
    let worktree = state
        .worktrees()
        .values()
        .find(|w| w.intent.attempt_id == record.intent.attempt_id)
        .unwrap();
    let path = PathBuf::from(&worktree.intent.path);
    drop(state);
    fs::write(path.join("fixture.txt"), "changed after verification\n").unwrap();
    drop(store);
    let mut reopened = fixture.store();
    let inspected = ManualWorker::inspect(&mut reopened, "later-change").unwrap();
    assert_eq!(inspected.status(), WorkerStatus::Invalidated);
    let observations = inspected.observations.len();
    let inspected_again = ManualWorker::inspect(&mut reopened, "later-change").unwrap();
    assert_eq!(inspected_again.status(), WorkerStatus::Invalidated);
    assert_eq!(inspected_again.observations.len(), observations);
    assert_eq!(fixture.thread_count(), 1);
    assert_eq!(
        fs::read_to_string(path.join("fixture.txt")).unwrap(),
        "changed after verification\n"
    );
}

#[test]
fn project_scripts_are_not_authorized_by_discovery_or_check_configuration_alone() {
    let fixture = Fixture::new();
    let mut request = fixture.request("unapproved-check");
    let mut spec = check(&["--check-fixture", "after\n"]);
    spec.authorized = false;
    request.checks = vec![spec];
    let mut store = fixture.store();
    assert!(
        ManualWorker::run(
            &mut store,
            &request,
            fixture.launch("success"),
            &CancellationToken::new()
        )
        .is_err()
    );
    assert_eq!(fixture.thread_count(), 0);
    assert!(!fixture.home.join("dispatch-count").exists());
    assert!(store.load().unwrap().worktrees().is_empty());
}

#[test]
fn check_executable_preflight_blocks_before_metadata_reservation_and_owned_processes() {
    let fixture = Fixture::new();
    let oversized = fixture.root.join("oversized-check.exe");
    let file = fs::File::create(&oversized).unwrap();
    file.set_len(64 * 1_048_576 + 1).unwrap();
    drop(file);
    let changed = fixture.root.join("changed-check.exe");
    fs::write(&changed, b"before\n").unwrap();
    let mut changed_check = check(&[]);
    changed_check.executable = changed;
    changed_check.executable_sha256 = Some(changed_check.preflight_executable().unwrap());
    fs::write(&changed_check.executable, b"edited\n").unwrap();
    let mut oversized_check = check(&[]);
    oversized_check.executable = oversized;
    let before_index = fs::read(fixture.repository.join(".git/index")).unwrap();
    let before_status = git(&fixture.repository, &["status", "--porcelain=v1", "-z"]).stdout;
    let mut store = fixture.store();
    let before_revision = store.load().unwrap().revision();
    let before_events = store.event_count().unwrap();
    for (id, spec, reason) in [
        (
            "oversized-check-preflight",
            oversized_check,
            "check-executable-size",
        ),
        (
            "changed-check-preflight",
            changed_check,
            "check-executable-changed",
        ),
    ] {
        let mut request = fixture.request(id);
        request.checks = vec![spec];
        let error = ManualWorker::run(
            &mut store,
            &request,
            fixture.launch("success"),
            &CancellationToken::new(),
        )
        .unwrap_err();
        assert_eq!(error.0, reason, "{id}");
        assert!(!fixture.home.join("metadata-reads").exists(), "{id}");
        assert!(!fixture.home.join("dispatch-count").exists(), "{id}");
        assert_eq!(fixture.thread_count(), 0, "{id}");
        assert_eq!(
            fs::read_dir(&fixture.authorized).unwrap().count(),
            0,
            "{id}"
        );
        let state = store.load().unwrap();
        assert_eq!(state.revision(), before_revision, "{id}");
        assert!(state.workers().is_empty(), "{id}");
        assert!(state.runs().is_empty(), "{id}");
        assert!(state.worktrees().is_empty(), "{id}");
        assert!(state.captured_artifacts().is_empty(), "{id}");
        assert_eq!(store.event_count().unwrap(), before_events, "{id}");
        assert!(
            fs::read_dir(&fixture.root).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("orchestrix-supervisor-")),
            "{id}"
        );
    }
    drop(store);
    let reopened = fixture.store();
    let state = reopened.load().unwrap();
    assert_eq!(state.revision(), before_revision);
    assert!(state.workers().is_empty());
    assert!(state.runs().is_empty());
    assert!(state.worktrees().is_empty());
    assert_eq!(reopened.event_count().unwrap(), before_events);
    assert_eq!(
        fs::read(fixture.repository.join(".git/index")).unwrap(),
        before_index
    );
    assert_eq!(
        git(&fixture.repository, &["status", "--porcelain=v1", "-z"]).stdout,
        before_status
    );
}

#[test]
fn cancelling_an_authorized_check_terminates_its_job_and_cannot_mark_green() {
    let fixture = Fixture::new();
    let marker = fixture.home.join("check-started");
    let mut request = fixture.request("cancelled-check");
    let mut spec = check(&["--check-hang", marker.to_str().unwrap()]);
    spec.timeout_ms = 10_000;
    request.checks = vec![spec];
    let token = CancellationToken::new();
    let trigger = token.clone();
    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    let canceller = std::thread::spawn(move || {
        // Preparation may legitimately outlast the evaluator's own timeout. Its
        // deadline starts when that process exists, and is unchanged by this wait.
        while !done.load(Ordering::Acquire) {
            if marker.is_file() {
                trigger.cancel();
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    });
    let mut store = fixture.store();
    let result = ManualWorker::run(&mut store, &request, fixture.launch("success"), &token);
    finished.store(true, Ordering::Release);
    let cancellation_sent = canceller.join().unwrap();
    let record = result.unwrap();
    assert!(
        cancellation_sent,
        "check never reached its authorized process"
    );
    assert_eq!(
        record.status(),
        WorkerStatus::NeedsWork,
        "{}",
        record.summary()
    );
    let details = &record.observations.last().unwrap().details;
    assert_eq!(details["checks"][0]["result"], "cancelled");
    assert_eq!(details["checks"][0]["tree_confirmed"], true);
    assert_eq!(details["checks"][0]["streams_closed"], true);
    assert_eq!(fixture.thread_count(), 1);
}

#[test]
fn missing_checks_are_durable_and_never_become_a_green_or_completed_task() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let request = fixture.request("without-checks");
    let record = ManualWorker::run(
        &mut store,
        &request,
        fixture.launch("success"),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(record.status(), WorkerStatus::NoChecks);
    let state = store.load().unwrap();
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id).unwrap())
        .unwrap();
    let task = run
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::NeedsReview);
    assert_eq!(task.attempts()[0].state(), AttemptState::Succeeded);
    assert!(task.verifications().is_empty());
    assert_eq!(fixture.thread_count(), 1);
    drop(state);
    drop(store);
    let mut reopened = fixture.store();
    // Existing work must be inspected without even validating/spawning a launch.
    let invalid_launch = RuntimeLaunch::fixture(
        fixture.root.join("does-not-exist.exe"),
        vec![],
        fixture.home.clone(),
        fixture.repository.clone(),
    );
    let replay = ManualWorker::run(
        &mut reopened,
        &request,
        invalid_launch,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(replay.status(), WorkerStatus::NoChecks);
    assert_eq!(fixture.thread_count(), 1);
    assert_eq!(
        fs::read_to_string(fixture.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
}
