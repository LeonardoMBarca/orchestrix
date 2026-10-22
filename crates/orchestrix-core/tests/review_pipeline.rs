//! Real Windows/Git/SQLite review/correction path with an explicitly synthetic runtime.
#![cfg(windows)]
use orchestrix_core::{
    domain::{RunId, TaskId, TaskState},
    evaluator::CheckSpec,
    persistence::Store,
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    review::{ReviewCheckpoint, ReviewPipeline},
    review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker},
    worker_records::WorkerStatus,
};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
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
        if std::thread::panicking() {
            eprintln!("Preserved review fixture: {}", root.display());
            return;
        }
        fs::remove_dir_all(root).unwrap();
    }
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
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

fn review_request(
    worker: &orchestrix_core::worker_records::WorkerRecord,
    revision: &str,
    id: &str,
    policy: &PolicyDocument,
) -> ReviewRequest {
    ReviewRequest {
        schema_version: 1,
        id: id.into(),
        worker_id: worker.intent.id.clone(),
        artifact_revision: revision.into(),
        authorization_ref: "explicit-review-authorization".into(),
        policy: policy.clone(),
        classification: TaskClassification::routine(TaskKind::Implementation, "manual-review"),
        budget: ReviewBudget {
            max_corrections: 1,
            max_reviews: 4,
            max_elapsed_ms: 600_000,
        },
    }
}
#[test]
fn review_refreshes_admission_after_capture_and_blocks_transient_changes_before_dispatch() {
    let f = Fixture::new();
    let mut store = f.store();
    let cancel = CancellationToken::new();
    let mut task = f.request("review-admission-order");
    task.checks = vec![check(&["--check-fixture", "after\n"])];
    let implemented = ManualWorker::run(&mut store, &task, f.launch("success"), &cancel).unwrap();
    assert_eq!(
        implemented.status(),
        WorkerStatus::Verified,
        "{}",
        implemented.summary()
    );
    let state = store.load().unwrap();
    let original = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap()
        .artifact()
        .unwrap()
        .clone();
    let captured_key = (
        original.id().as_str().to_owned(),
        original.revision().as_str().to_owned(),
    );
    let captured_bytes = serde_json::to_vec(&state.captured_artifacts()[&captured_key]).unwrap();
    let worktree = state
        .worktrees()
        .values()
        .find(|worktree| worktree.intent.attempt_id == implemented.intent.attempt_id)
        .unwrap();
    let path = PathBuf::from(&worktree.intent.path);
    let common = PathBuf::from(&worktree.intent.common_git_dir);
    drop(state);
    let working_file = path.join("fixture.txt");
    let configuration_file = common.join("config");
    let working_bytes = fs::read(&working_file).unwrap();
    let configuration_bytes = fs::read(&configuration_file).unwrap();
    let index = PathBuf::from(
        String::from_utf8(
            git(
                &path,
                &["rev-parse", "--path-format=absolute", "--git-path", "index"],
            )
            .stdout,
        )
        .unwrap()
        .trim(),
    );
    let index_bytes = fs::read(&index).unwrap();
    let references = git(&f.repository, &["show-ref"]).stdout;
    let metadata_reads = || {
        fs::read_to_string(f.home.join("metadata-reads"))
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    // dispatch-count is per process, so equal contents alone cannot prove that
    // another reviewer did not receive a first turn. Compare the durable ledger.
    let dispatches = || {
        fs::read_dir(&f.home)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().unwrap();
                (name.starts_with("dispatch-event-") && name.ends_with(".json"))
                    .then(|| (name, fs::read(entry.path()).unwrap()))
            })
            .collect::<BTreeMap<_, _>>()
    };
    let initial_dispatches = dispatches();
    assert_eq!(initial_dispatches.len(), 1, "only the implementation turn");
    assert!(!f.home.join("review-sandbox").exists());

    for (id, mutation) in [
        ("review-worktree-transient", Some(&working_file)),
        ("review-common-transient", Some(&configuration_file)),
        ("review-admission-current", None),
    ] {
        let baseline_reads = metadata_reads();
        let baseline_dispatches = dispatches();
        let dispatch_count = fs::read(f.home.join("dispatch-count")).unwrap();
        let mut artifact_validated = false;
        let mut admission_confirmed = false;
        let request = review_request(&implemented, original.revision().as_str(), id, &task.policy);
        let reviewed = ReviewPipeline::review_with_checkpoint(
            &mut store,
            &request,
            f.launch("review-accept"),
            &cancel,
            &mut |checkpoint| match checkpoint {
                ReviewCheckpoint::ArtifactValidated => {
                    assert!(!artifact_validated && !admission_confirmed);
                    artifact_validated = true;
                    assert_eq!(
                        metadata_reads(),
                        baseline_reads + 1,
                        "the full capture precedes the second metadata connection"
                    );
                    assert_eq!(dispatches(), baseline_dispatches);
                }
                ReviewCheckpoint::AdmissionMetadataConfirmed => {
                    assert!(artifact_validated && !admission_confirmed);
                    admission_confirmed = true;
                    assert_eq!(metadata_reads(), baseline_reads + 2);
                    assert_eq!(dispatches(), baseline_dispatches);
                    if let Some(target) = mutation {
                        let original = fs::read(target).unwrap();
                        let mut transient = original.clone();
                        transient.extend_from_slice(b"\ntransient owned review race\n");
                        fs::write(target, transient).unwrap();
                        fs::write(target, &original).unwrap();
                        assert_eq!(fs::read(target).unwrap(), original);
                    }
                }
            },
        )
        .unwrap();
        assert!(artifact_validated && admission_confirmed);
        assert_eq!(metadata_reads(), baseline_reads + 2);
        assert_eq!(fs::read(&working_file).unwrap(), working_bytes);
        assert_eq!(fs::read(&configuration_file).unwrap(), configuration_bytes);
        assert_eq!(fs::read(&index).unwrap(), index_bytes);
        assert_eq!(git(&f.repository, &["show-ref"]).stdout, references);
        assert_eq!(
            serde_json::to_vec(&store.load().unwrap().captured_artifacts()[&captured_key]).unwrap(),
            captured_bytes
        );
        if mutation.is_some() {
            assert_eq!(
                reviewed.status(),
                ReviewStatus::Unavailable,
                "{}",
                reviewed.summary()
            );
            assert!(reviewed.output().is_none());
            assert_eq!(
                reviewed.observations.last().unwrap().details["reason"],
                "review-artifact-changed"
            );
            assert!(
                reviewed
                    .observations
                    .iter()
                    .all(|observation| observation.status != ReviewStatus::DispatchPlanned),
                "a watch signal must stop review before dispatch intent"
            );
            assert_eq!(dispatches(), baseline_dispatches);
            assert_eq!(
                fs::read(f.home.join("dispatch-count")).unwrap(),
                dispatch_count
            );
            assert!(!f.home.join("review-sandbox").exists());
            let state = store.load().unwrap();
            let current_task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
                .task(&TaskId::new(&implemented.intent.task_id).unwrap())
                .unwrap();
            assert!(current_task.reviews().is_empty());
            assert_eq!(current_task.state(), TaskState::NeedsReview);
        } else {
            assert_eq!(
                reviewed.status(),
                ReviewStatus::Accepted,
                "{}",
                reviewed.summary()
            );
            assert!(reviewed.output().is_some());
            assert_eq!(
                reviewed.observations.last().unwrap().details["readOnlyConfirmed"],
                true
            );
            let sandbox: serde_json::Value =
                serde_json::from_slice(&fs::read(f.home.join("review-sandbox")).unwrap()).unwrap();
            assert_eq!(sandbox["type"], "readOnly");
            let final_dispatches = dispatches();
            assert_eq!(final_dispatches.len(), baseline_dispatches.len() + 1);
            for (name, bytes) in baseline_dispatches {
                assert_eq!(final_dispatches.get(&name), Some(&bytes));
            }
        }
    }
    assert_eq!(dispatches().len(), initial_dispatches.len() + 1);
}
#[test]
fn independent_review_rejection_correction_preserves_task_artifact_and_rechecks_exact_version() {
    let f = Fixture::new();
    let mut store = f.store();
    let cancel = CancellationToken::new();
    let mut task = f.request("implementation");
    task.checks = vec![check(&["--check-fixture", "after\n"])];
    let implemented = ManualWorker::run(
        &mut store,
        &task,
        f.launch("review-implementation"),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        implemented.status(),
        WorkerStatus::Verified,
        "{}",
        implemented.summary()
    );
    let state = store.load().unwrap();
    let original = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap()
        .artifact()
        .unwrap()
        .clone();
    drop(state);
    let rejected_input = review_request(
        &implemented,
        original.revision().as_str(),
        "independent-rejection",
        &task.policy,
    );
    let rejected = ReviewPipeline::review(
        &mut store,
        &rejected_input,
        f.launch("review-reject"),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        rejected.status(),
        ReviewStatus::Rejected,
        "{}",
        rejected.summary()
    );
    let before = store.load().unwrap();
    let oldtask = before.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert_ne!(
        rejected.intent.session_id,
        oldtask.attempts()[0].session().id().as_str()
    );
    assert_eq!(oldtask.state(), TaskState::NeedsWork);
    drop(before);
    drop(store);
    let mut store = f.store();
    let correction = ReviewPipeline::prepare_correction(
        &store,
        &rejected.intent.id,
        "correction-one",
        "explicit-one-correction",
        task.policy.clone(),
        task.classification.clone(),
    )
    .unwrap();
    assert_eq!(correction.task.instructions, task.instructions);
    assert!(correction.task.context_files.is_empty());
    let corrected = ReviewPipeline::correct(
        &mut store,
        &correction,
        f.launch("review-correction"),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        corrected.status(),
        WorkerStatus::Verified,
        "{}",
        corrected.summary()
    );
    assert_eq!(corrected.intent.run_id, implemented.intent.run_id);
    assert_eq!(corrected.intent.task_id, implemented.intent.task_id);
    let mut exhausted = correction.clone();
    exhausted.id = "correction-two".into();
    exhausted.task.id = exhausted.id.clone();
    assert_eq!(
        ReviewPipeline::correct(&mut store, &exhausted, f.launch("crash"), &cancel)
            .unwrap_err()
            .0,
        "review-budget-exhausted"
    );

    let state = store.load().unwrap();
    assert_eq!(state.runs().len(), 1);
    let current = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(current.attempts().len(), 2);
    assert_eq!(
        current.attempts()[1].inputs().correction_of.as_ref(),
        Some(&original)
    );
    assert_eq!(current.attempts()[1].inputs().base, *original.revision());
    let corrected_artifact = current.artifact().unwrap().clone();
    assert_ne!(&corrected_artifact, &original);
    let artifact = state
        .captured_artifacts()
        .get(&(
            corrected_artifact.id().as_str().to_owned(),
            corrected_artifact.revision().as_str().to_owned(),
        ))
        .unwrap();
    assert_eq!(artifact.base_oid, f.base);
    assert_eq!(
        artifact
            .files
            .iter()
            .find(|file| file.path == "correction.txt")
            .unwrap()
            .worktree
            .as_ref()
            .unwrap()
            .content,
        "finding-one addressed\n"
    );
    assert!(
        !state
            .captured_artifacts()
            .get(&(
                original.id().as_str().to_owned(),
                original.revision().as_str().to_owned()
            ))
            .unwrap()
            .files
            .iter()
            .any(|file| file.path == "correction.txt")
    );
    assert_eq!(
        artifact
            .files
            .iter()
            .find(|file| file.path == "fixture.txt")
            .unwrap()
            .worktree
            .as_ref()
            .unwrap()
            .content,
        "after\n"
    );

    assert_eq!(
        artifact
            .files
            .iter()
            .find(|f| f.path == "implementation.txt")
            .unwrap()
            .worktree
            .as_ref()
            .unwrap()
            .content,
        "preserve prior implementation\n"
    );
    assert_eq!(current.verifications().len(), 2);
    assert_eq!(current.reviews().len(), 1);
    assert_eq!(current.state(), TaskState::NeedsReview);
    drop(state);
    let approved_input = review_request(
        &corrected,
        corrected_artifact.revision().as_str(),
        "corrected-review",
        &task.policy,
    );
    let approved = ReviewPipeline::review(
        &mut store,
        &approved_input,
        f.launch("review-accept"),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        approved.status(),
        ReviewStatus::Accepted,
        "{}",
        approved.summary()
    );
    drop(store);
    let mut reopened = f.store();
    let state = reopened.load().unwrap();
    let current = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(current.state(), TaskState::Approved);
    assert_eq!(current.reviews().len(), 2);
    assert_eq!(state.reviews().len(), 2);
    assert!(current.integrations().is_empty());
    drop(state);
    assert_retained_and_validate_rehashed_corruption(&f, &approved.intent.id);
    let replay =
        ReviewPipeline::review(&mut reopened, &approved_input, f.launch("crash"), &cancel).unwrap();
    assert_eq!(replay, approved);
    let replayed_correction =
        ReviewPipeline::correct(&mut reopened, &correction, f.launch("crash"), &cancel).unwrap();
    assert_eq!(replayed_correction.intent, corrected.intent);
    assert_eq!(reopened.load().unwrap().workers().len(), 2);
    let state = reopened.load().unwrap();
    let old_path = state
        .worktrees()
        .values()
        .find(|w| w.intent.attempt_id == implemented.intent.attempt_id)
        .unwrap()
        .intent
        .path
        .clone();
    let new_path = state
        .worktrees()
        .values()
        .find(|w| w.intent.attempt_id == corrected.intent.attempt_id)
        .unwrap()
        .intent
        .path
        .clone();
    drop(state);
    fs::write(
        Path::new(&old_path).join("fixture.txt"),
        "historical mutation\n",
    )
    .unwrap();
    assert_eq!(
        ManualWorker::inspect(&mut reopened, &implemented.intent.id)
            .unwrap()
            .status(),
        WorkerStatus::Invalidated
    );
    assert_eq!(
        reopened.load().unwrap().reviews()[&rejected.intent.id].status(),
        ReviewStatus::Invalidated
    );
    assert_eq!(
        reopened.load().unwrap().reviews()[&approved.intent.id].status(),
        ReviewStatus::Accepted
    );
    assert_eq!(
        reopened.load().unwrap().runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
            .task(&TaskId::new(&implemented.intent.task_id).unwrap())
            .unwrap()
            .state(),
        TaskState::Approved
    );
    fs::write(
        Path::new(&new_path).join("fixture.txt"),
        "current mutation\n",
    )
    .unwrap();
    assert_eq!(
        ManualWorker::inspect(&mut reopened, &corrected.intent.id)
            .unwrap()
            .status(),
        WorkerStatus::Invalidated
    );
    assert_eq!(
        reopened.load().unwrap().reviews()[&approved.intent.id].status(),
        ReviewStatus::Invalidated
    );
    assert_eq!(
        reopened.load().unwrap().runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
            .task(&TaskId::new(&implemented.intent.task_id).unwrap())
            .unwrap()
            .state(),
        TaskState::NeedsWork
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
}
#[test]
fn invalid_and_unavailable_review_never_approve_and_file_mutation_withdraws_checks() {
    let f = Fixture::new();
    let mut store = f.store();
    let cancel = CancellationToken::new();
    let mut task = f.request("review-errors");
    task.checks = vec![check(&["--check-fixture", "after\n"])];
    let implemented = ManualWorker::run(&mut store, &task, f.launch("success"), &cancel).unwrap();
    assert_eq!(implemented.status(), WorkerStatus::Verified);
    let state = store.load().unwrap();
    let revision = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap()
        .artifact()
        .unwrap()
        .revision()
        .as_str()
        .to_owned();
    drop(state);
    for (id, scenario, expected) in [
        ("invalid-review", "review-invalid", ReviewStatus::Invalid),
        ("no-quota", "rate-limit", ReviewStatus::Unavailable),
        (
            "mutating-review",
            "review-mutate",
            ReviewStatus::Invalidated,
        ),
    ] {
        let request = review_request(&implemented, &revision, id, &task.policy);
        let result =
            ReviewPipeline::review(&mut store, &request, f.launch(scenario), &cancel).unwrap();
        assert_eq!(result.status(), expected, "{}", result.summary());
        assert!(result.output().is_none());
        let state = store.load().unwrap();
        let task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
            .task(&TaskId::new(&implemented.intent.task_id).unwrap())
            .unwrap();
        assert!(task.reviews().is_empty());
        assert_ne!(task.state(), TaskState::Approved);
    }
    let invalidated = ManualWorker::inspect(&mut store, &implemented.intent.id).unwrap();
    assert_eq!(invalidated.status(), WorkerStatus::Invalidated);
}

fn assert_retained_and_validate_rehashed_corruption(f: &Fixture, review_id: &str) {
    use orchestrix_core::git_records::content_hash;
    use rusqlite::{Connection, params};
    let source = Connection::open(&f.database).unwrap();
    for sql in [
        "UPDATE independent_reviews SET intent=intent",
        "DELETE FROM independent_reviews",
        "UPDATE review_observations SET body=body",
        "DELETE FROM review_observations",
    ] {
        assert!(source.execute(sql, []).is_err(), "must retain {sql}");
    }
    for case in [
        "missing-output",
        "missing-observer",
        "missing-binding",
        "readonly-flag",
        "effective-sandbox",
        "streams",
        "missing-technical-row",
    ] {
        let copy = f.root.join(format!("corrupt-{case}.sqlite"));
        source
            .execute("VACUUM INTO ?1", [copy.to_str().unwrap()])
            .unwrap();
        let corrupt = Connection::open(&copy).unwrap();
        corrupt.execute_batch("DROP TRIGGER review_observations_no_update;DROP TRIGGER review_observations_no_delete;").unwrap();
        let(ordinal,body):(i64,String)=corrupt.query_row("SELECT ordinal,body FROM review_observations WHERE review_id=?1 AND json_extract(body,'$.status')='accepted'",[review_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let mut observation: serde_json::Value = serde_json::from_str(&body).unwrap();
        match case {
            "missing-output" => {
                observation["details"]
                    .as_object_mut()
                    .unwrap()
                    .remove("output");
            }
            "missing-observer" => {
                observation["details"]
                    .as_object_mut()
                    .unwrap()
                    .remove("observer");
            }
            "missing-binding" => {
                observation["details"]
                    .as_object_mut()
                    .unwrap()
                    .remove("adapterBinding");
            }
            "readonly-flag" => {
                observation["details"]["readOnlyConfirmed"] = serde_json::json!(false)
            }
            "effective-sandbox" => {
                observation["details"]["observer"]["snapshot"]["effective"]["sandbox"]["value"] =
                    serde_json::json!("workspace-write")
            }
            "streams" => observation["details"]["streamsClosed"] = serde_json::json!(false),
            "missing-technical-row" => {}
            _ => unreachable!(),
        }
        if case == "missing-technical-row" {
            corrupt
                .execute(
                    "DELETE FROM review_observations WHERE review_id=?1 AND ordinal=?2",
                    params![review_id, ordinal],
                )
                .unwrap();
        } else {
            let body = observation.to_string();
            corrupt.execute("UPDATE review_observations SET body=?3,digest=?4 WHERE review_id=?1 AND ordinal=?2",params![review_id,ordinal,body,content_hash(body.as_bytes())]).unwrap();
        }
        drop(corrupt);
        match Store::open(&copy) {
            Err(orchestrix_core::persistence::StoreError::InvalidState) => {}
            Ok(restored) => assert_eq!(
                restored.load().err(),
                Some(orchestrix_core::persistence::StoreError::InvalidState),
                "must reject rehashed {case}"
            ),
            Err(other) => panic!("unexpected corruption failure {case}: {other}"),
        }
    }
}
#[test]
fn review_crash_is_durable_uncertain_and_only_exact_local_proof_allows_set_aside() {
    let f = Fixture::new();
    let mut store = f.store();
    let cancel = CancellationToken::new();
    let mut task = f.request("crash-source");
    task.checks = vec![check(&["--check-fixture", "after\n"])];
    let implemented = ManualWorker::run(&mut store, &task, f.launch("success"), &cancel).unwrap();
    assert_eq!(implemented.status(), WorkerStatus::Verified);
    let state = store.load().unwrap();
    let revision = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap()
        .artifact()
        .unwrap()
        .revision()
        .as_str()
        .to_owned();
    drop(state);
    let request = review_request(&implemented, &revision, "crashed-review", &task.policy);
    let uncertain =
        ReviewPipeline::review(&mut store, &request, f.launch("crash"), &cancel).unwrap();
    assert_eq!(uncertain.status(), ReviewStatus::Uncertain);
    assert!(uncertain.output().is_none());
    drop(store);
    let mut reopened = f.store();
    ReviewPipeline::reconcile(&mut reopened).unwrap();
    let replay =
        ReviewPipeline::review(&mut reopened, &request, f.launch("review-accept"), &cancel)
            .unwrap();
    assert_eq!(replay, uncertain);
    let mut other = task.clone();
    other.id = "blocked-after-review-crash".into();
    assert_eq!(
        ManualWorker::run(&mut reopened, &other, f.launch("success"), &cancel)
            .unwrap_err()
            .0,
        "serial-admission-blocked"
    );
    let exact_revision = reopened.load().unwrap().revision();
    assert!(
        ReviewPipeline::set_aside(
            &mut reopened,
            &request.id,
            exact_revision - 1,
            "authorize-closed-review"
        )
        .is_err()
    );
    let closed = ReviewPipeline::set_aside(
        &mut reopened,
        &request.id,
        exact_revision,
        "authorize-closed-review",
    )
    .unwrap();
    assert_eq!(closed.status(), ReviewStatus::SetAside);
    assert!(closed.output().is_none());
    let state = reopened.load().unwrap();
    let task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert!(task.reviews().is_empty());
    assert_eq!(task.state(), TaskState::NeedsReview);
    drop(state);
    let cancelled_request = review_request(
        &implemented,
        &revision,
        "cancelled-review",
        &task_policy(&f),
    );
    let token = CancellationToken::new();
    let worker_token = token.clone();
    let marker = f.home.join("review-started");
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let watcher = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            if marker.is_file() {
                worker_token.cancel();
                return true;
            }
            if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(120) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let result = ReviewPipeline::review(
        &mut reopened,
        &cancelled_request,
        f.launch("review-hang"),
        &token,
    );
    let _ = stop_tx.send(());
    assert!(
        watcher.join().unwrap(),
        "fixture must confirm turn start before cancellation"
    );
    let cancelled = result.unwrap();
    assert_eq!(
        cancelled.status(),
        ReviewStatus::Cancelled,
        "{}",
        cancelled.summary()
    );
    assert!(cancelled.output().is_none());
    assert_eq!(
        cancelled.observations.last().unwrap().details["treeStopped"],
        true
    );
    assert_eq!(
        cancelled.observations.last().unwrap().details["streamsClosed"],
        true
    );
}

fn task_policy(f: &Fixture) -> PolicyDocument {
    f.request("policy-template").policy
}
