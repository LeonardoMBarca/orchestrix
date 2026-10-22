#![cfg(windows)]
//! Real disposable Git/SQLite/check processes; source agents are explicitly synthetic.
use orchestrix_core::{
    domain::{RunId, RunState, TaskId, TaskState},
    evaluator::CheckSpec,
    git_records::content_hash,
    integration::{IntegrationCheckpoint, IntegrationClock, IntegrationService},
    integration_records::*,
    persistence::Store,
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    recovery_guardian::{GuardianBinding, read_proof},
    review::ReviewPipeline,
    review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker},
    worker_records::WorkerStatus,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct Clock(AtomicU64);
impl IntegrationClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct SteppingClock(AtomicU64);
impl IntegrationClock for SteppingClock {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(100, Ordering::SeqCst)
    }
}
struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    database: PathBuf,
    integration: PathBuf,
    base: String,
    source: String,
    revision: String,
}
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
            "orchestrix-integration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("source");
        let attempts = root.join("attempts");
        let home = root.join("synthetic-auth");
        let integration = root.join("integration");
        for p in [&repository, &attempts, &home, &integration] {
            fs::create_dir(p).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), "before\n").unwrap();
        fs::write(
            repository.join("preserve.txt"),
            "original untouched bytes\n",
        )
        .unwrap();
        fs::create_dir(repository.join("nested")).unwrap();
        fs::write(
            repository.join("nested/keep.md"),
            "# Preserved nested file\n",
        )
        .unwrap();
        git(&repository, &["add", "--all"]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "Disposable integration fixture"],
        );
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        let database = root.join("state.sqlite");
        let mut store = Store::open(&database).unwrap();
        let mut policy = PolicyDocument::default();
        policy.mode_defaults.fallback = Some(Fallback::Compatible);
        let check = check("after\n");
        let task = ManualTaskRequest {
            editable_paths: None,
            schema_version: 1,
            id: "implementation".into(),
            project_id: "integration-project".into(),
            repository_id: "integration-repository".into(),
            repository_directory: repository.clone(),
            authorized_worktree_directory: attempts,
            base_commit: base.clone(),
            connection_id: "synthetic-account".into(),
            connection_revision: 1,
            objective: "fixture-edit: change only fixture.txt to after".into(),
            acceptance_criteria: vec!["fixture.txt contains after followed by newline".into()],
            policy: policy.clone(),
            classification: TaskClassification::routine(
                TaskKind::Implementation,
                "explicit-fixture-authorization",
            ),
            context_files: vec![ContextFileRequest {
                path: "fixture.txt".into(),
                authorization_ref: "read-fixture".into(),
                required: true,
            }],
            instructions: vec!["Only edit the owned fixture; no network or delegation".into()],
            checks: vec![check],
        };
        let launch = |scenario: &str| {
            let mut l = RuntimeLaunch::fixture(
                binary(),
                vec!["--scenario".into(), scenario.into()],
                home.clone(),
                repository.clone(),
            );
            l.request_timeout = Duration::from_secs(2);
            l.turn_timeout = Duration::from_secs(3);
            l
        };
        let implemented = ManualWorker::run(
            &mut store,
            &task,
            launch("success"),
            &CancellationToken::new(),
        )
        .unwrap();
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
        let reviewed = ReviewPipeline::review(
            &mut store,
            &ReviewRequest {
                schema_version: 1,
                id: "source-review".into(),
                worker_id: implemented.intent.id.clone(),
                artifact_revision: revision.clone(),
                authorization_ref: "source-review-authorized".into(),
                policy,
                classification: TaskClassification::routine(
                    TaskKind::Implementation,
                    "independent-fixture-review",
                ),
                budget: ReviewBudget {
                    max_corrections: 0,
                    max_reviews: 1,
                    max_elapsed_ms: 600_000,
                },
            },
            launch("review-accept"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(reviewed.status(), ReviewStatus::Accepted);
        Self {
            root,
            repository,
            database,
            integration,
            base,
            source: implemented.intent.id,
            revision,
        }
    }
    fn request(&self, id: &str) -> IntegrationRequest {
        IntegrationRequest {
            schema_version: 1,
            id: id.into(),
            worker_id: self.source.clone(),
            artifact_revision: self.revision.clone(),
            authorization_ref: "explicit-integration-checks-and-owned-worktree".into(),
            authorized_integration_directory: self.integration.clone(),
            target_directory: self.repository.clone(),
            checks: vec![check("after\n")],
        }
    }
    fn copy_database(&self, name: &str) -> PathBuf {
        let path = self.root.join(format!("{name}.sqlite"));
        let c = rusqlite::Connection::open(&self.database).unwrap();
        c.execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
        path
    }
    fn clock(&self) -> Arc<Clock> {
        Arc::new(Clock(AtomicU64::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Preserved integration fixture {}", self.root.display());
        }
    }
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn check(expected: &str) -> CheckSpec {
    let executable = binary();
    CheckSpec {
        id: "authorized-combined-check".into(),
        executable: executable.clone(),
        arguments: vec!["--check-fixture".into(), expected.into()],
        authorization_ref: "explicit-combined-check-authorization".into(),
        authorized: true,
        timeout_ms: 2_000,
        executable_sha256: Some(content_hash(&fs::read(executable).unwrap())),
    }
}
fn git(cwd: &Path, args: &[&str]) -> Vec<u8> {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("git");
    c.creation_flags(0x0800_0000).env_clear();
    for k in [
        "PATH",
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
    ] {
        if let Some(v) = env::var_os(k) {
            c.env(k, v);
        }
    }
    let o = c
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
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(o.status.success(), "fixture Git {args:?}");
    o.stdout
}
fn approval(record: &IntegrationRecord, id: &str) -> ApprovalRequest {
    ApprovalRequest {
        schema_version: 1,
        id: id.into(),
        integration_id: record.intent.id.clone(),
        candidate_revision: record.candidate_revision().unwrap(),
        authorization_ref: "explicit-human-exact-candidate-approval".into(),
        valid_for_ms: 600_000,
    }
}
fn application(record: &IntegrationRecord, id: &str) -> ApplicationRequest {
    ApplicationRequest {
        schema_version: 1,
        id: id.into(),
        integration_id: record.intent.id.clone(),
        candidate_revision: record.candidate_revision().unwrap(),
        authorization_ref: "explicit-user-target-application".into(),
    }
}

#[test]
fn exact_combined_checks_approval_expiry_local_work_and_final_delivery_are_distinct() {
    let f = Fixture::new();
    let cancel = CancellationToken::new();
    let service = IntegrationService::new();
    let cancelled_path = f.copy_database("cancelled-reservation");
    let mut cancelled_store = Store::open(&cancelled_path).unwrap();
    let cancelled = CancellationToken::new();
    let trigger = cancelled.clone();
    let stopped = service
        .prepare_with_checkpoint(
            &mut cancelled_store,
            &f.request("cancelled-before-effects"),
            &cancelled,
            move |point| {
                if point == IntegrationCheckpoint::Reserved {
                    trigger.cancel();
                }
            },
        )
        .unwrap();
    assert_eq!(stopped.status(), IntegrationStatus::Blocked);
    assert!(stopped.detail("guardian").is_none());
    assert!(!Path::new(&stopped.intent.worktree.path).exists());
    assert!(!stopped.status().blocks_admission());
    // Keep this failed preparation in the SAME durable Run. A later exact
    // candidate's approval must not be demanded from the historical Blocked row.
    let mut failed = Store::open(&f.database).unwrap();
    let mut req = f.request("failed-integration");
    req.checks[0] = check("wrong\n");
    let result = service.prepare(&mut failed, &req, &cancel).unwrap();
    assert_eq!(result.status(), IntegrationStatus::Blocked);
    assert_eq!(result.detail("checks").unwrap()[0]["result"], "failed");
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    let state = failed.load().unwrap();
    let task = state.runs()[&RunId::new(&result.intent.run_id).unwrap()]
        .task(&TaskId::new(&result.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::Approved);
    assert!(task.integrations().is_empty());
    drop(state);
    drop(failed);
    let mut store = Store::open(&f.database).unwrap();
    let request = f.request("serial-integration");
    let prepared = service.prepare(&mut store, &request, &cancel).unwrap();
    assert_eq!(prepared.status(), IntegrationStatus::Prepared);
    let state = store.load().unwrap();
    let run = &state.runs()[&RunId::new(&prepared.intent.run_id).unwrap()];
    assert_eq!(run.state(), RunState::ReadyToApply);
    assert_eq!(
        run.task(&TaskId::new(&prepared.intent.task_id).unwrap())
            .unwrap()
            .state(),
        TaskState::Completed
    );
    let result_oid = prepared.detail("resultRevision").unwrap().as_str().unwrap();
    assert_eq!(result_oid.len(), 40);
    assert_ne!(result_oid, f.revision);
    assert_eq!(
        prepared.detail("checks").unwrap()[0]["artifact_revision"],
        prepared.snapshot().unwrap().revision
    );
    drop(state);
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    let invalid = application(&prepared, "not-approved");
    assert_eq!(
        service.apply(&mut store, &invalid, &cancel).unwrap_err().0,
        "approved-exact-candidate-required"
    );
    let replay = service.prepare(&mut store, &request, &cancel).unwrap();
    assert_eq!(json!(replay), json!(prepared));
    let mut incompatible = request.clone();
    incompatible.artifact_revision = "a".repeat(64);
    assert_eq!(
        service
            .prepare(&mut store, &incompatible, &cancel)
            .unwrap_err()
            .0,
        "integration-id-conflict"
    );
    // Copies are private test snapshots, retaining the exact historical source
    // and combined evidence. No production binding import is authorized here.
    let expiry = f.copy_database("expired");
    let mut expired = Store::open(&expiry).unwrap();
    let clock = f.clock();
    let timed = IntegrationService::with_clock(clock.clone());
    let mut a = approval(&prepared, "expires");
    a.valid_for_ms = 1_000;
    timed.approve(&mut expired, &a).unwrap();
    clock.0.fetch_add(1_000, Ordering::SeqCst);
    let expired = timed
        .apply(
            &mut expired,
            &application(&prepared, "expired-apply"),
            &cancel,
        )
        .unwrap();
    assert_eq!(expired.status(), IntegrationStatus::Invalidated);
    let path = f.copy_database("expires-before-effects");
    let mut late = Store::open(&path).unwrap();
    let clock = f.clock();
    let timed = IntegrationService::with_clock(clock.clone());
    let mut a = approval(&prepared, "expires-late");
    a.valid_for_ms = 1_000;
    let approved = timed.approve(&mut late, &a).unwrap();
    let issued = approved.detail("approval").unwrap()["issuedMs"]
        .as_u64()
        .unwrap();
    let stepping =
        IntegrationService::with_clock(Arc::new(SteppingClock(AtomicU64::new(issued + 800))));
    let stopped = stepping
        .apply(&mut late, &application(&prepared, "late-apply"), &cancel)
        .unwrap();
    assert_eq!(stopped.status(), IntegrationStatus::Invalidated);
    assert_eq!(
        stopped.observations.last().unwrap().details["applicationNotStarted"],
        true
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    for (kind, contents) in [
        ("dirty", "personal local edit\n"),
        ("untracked", "keep my notes\n"),
        ("sensitive", "LOCAL_VALUE=keep private\n"),
    ] {
        let path = f.copy_database(kind);
        let mut case = Store::open(&path).unwrap();
        service
            .approve(&mut case, &approval(&prepared, kind))
            .unwrap();
        let changed = f.repository.join(if kind == "dirty" {
            "fixture.txt"
        } else if kind == "sensitive" {
            ".env"
        } else {
            "notes.txt"
        });
        fs::write(&changed, contents).unwrap();
        let blocked = service
            .apply(
                &mut case,
                &application(&prepared, &format!("{kind}-apply")),
                &cancel,
            )
            .unwrap();
        assert_eq!(blocked.status(), IntegrationStatus::Invalidated);
        assert_eq!(fs::read_to_string(&changed).unwrap(), contents);
        if kind == "dirty" {
            fs::write(changed, "before\n").unwrap();
        } else {
            fs::remove_file(changed).unwrap();
        }
    }
    let path = f.copy_database("branch-changed");
    let mut changed_branch = Store::open(&path).unwrap();
    service
        .approve(&mut changed_branch, &approval(&prepared, "branch-approved"))
        .unwrap();
    git(&f.repository, &["branch", "alternate", &f.base]);
    git(
        &f.repository,
        &["symbolic-ref", "HEAD", "refs/heads/alternate"],
    );
    assert_eq!(
        service
            .apply(
                &mut changed_branch,
                &application(&prepared, "branch-apply"),
                &cancel
            )
            .unwrap()
            .status(),
        IntegrationStatus::Invalidated
    );
    assert_eq!(
        String::from_utf8(git(&f.repository, &["symbolic-ref", "HEAD"]))
            .unwrap()
            .trim(),
        "refs/heads/alternate"
    );
    git(&f.repository, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    let tampered = f.copy_database("wrong-candidate");
    let mut case = Store::open(&tampered).unwrap();
    let mut bad = approval(&prepared, "wrong-hash");
    bad.candidate_revision = "b".repeat(64);
    assert_eq!(
        service.approve(&mut case, &bad).unwrap_err().0,
        "prepared-exact-candidate-required"
    );
    let path = f.copy_database("combined-worktree-changed");
    let mut changed = Store::open(&path).unwrap();
    service
        .approve(&mut changed, &approval(&prepared, "combined-approved"))
        .unwrap();
    let combined_file = Path::new(&prepared.intent.worktree.path).join("fixture.txt");
    fs::write(&combined_file, "external combined-worktree edit\n").unwrap();
    let invalidated = service.inspect(&mut changed, &prepared.intent.id).unwrap();
    assert_eq!(invalidated.status(), IntegrationStatus::Invalidated);
    assert_eq!(
        fs::read_to_string(&combined_file).unwrap(),
        "external combined-worktree edit\n"
    );
    let state = changed.load().unwrap();
    let run = &state.runs()[&RunId::new(&prepared.intent.run_id).unwrap()];
    assert!(run.approval().is_none());
    assert!(run.candidate().is_none());
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    drop(state);
    // Only this private test's edited worktree is restored. The invalidated
    // copy retains its negative evidence; it is never reused for application.
    fs::write(&combined_file, "after\n").unwrap();
    // Regression: identical bytes after a manual rewrite are the same approved
    // logical target even though the index's cached stat entry is now stale.
    fs::write(f.repository.join("fixture.txt"), "before\n").unwrap();
    let same_bytes = service.inspect(&mut store, &prepared.intent.id).unwrap();
    assert_eq!(same_bytes.status(), IntegrationStatus::Prepared);
    assert_eq!(same_bytes.intent.target, prepared.intent.target);
    let a = approval(&prepared, "approval-one");
    service.approve(&mut store, &a).unwrap();
    let approved = service.approve(&mut store, &a).unwrap();
    assert_eq!(approved.status(), IntegrationStatus::Approved);
    let app = application(&prepared, "apply-one");
    let delivered = service.apply(&mut store, &app, &cancel).unwrap();
    assert_eq!(delivered.status(), IntegrationStatus::Delivered);
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("preserve.txt")).unwrap(),
        "original untouched bytes\n"
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("nested/keep.md")).unwrap(),
        "# Preserved nested file\n"
    );
    assert_eq!(
        String::from_utf8(git(&f.repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim(),
        result_oid
    );
    assert_eq!(
        String::from_utf8(git(&f.repository, &["rev-parse", "HEAD^"]))
            .unwrap()
            .trim(),
        f.base
    );
    assert!(
        git(
            &f.repository,
            &["status", "--porcelain", "--untracked-files=all"]
        )
        .is_empty()
    );
    let replay = service.apply(&mut store, &app, &cancel).unwrap();
    assert_eq!(json!(replay), json!(delivered));
    drop(store);
    let reopened = Store::open(&f.database).unwrap();
    assert_eq!(
        reopened.load().unwrap().runs()[&RunId::new(&prepared.intent.run_id).unwrap()].state(),
        RunState::Delivered
    );
    let changed_source = f.copy_database("delivered-source-edited");
    let removed_source = f.copy_database("delivered-source-removed");
    let state = reopened.load().unwrap();
    let source_attempt = &state.workers()[&f.source].intent.attempt_id;
    let source_path = PathBuf::from(
        &state
            .worktrees()
            .values()
            .find(|w| &w.intent.attempt_id == source_attempt)
            .unwrap()
            .intent
            .path,
    )
    .join("fixture.txt");
    drop(state);
    for (database, remove) in [(changed_source, false), (removed_source, true)] {
        if remove {
            fs::remove_file(&source_path).unwrap();
        } else {
            fs::write(&source_path, "changed after delivery\n").unwrap();
        }
        let mut restored = Store::open(&database).unwrap();
        orchestrix_core::recovery::reconcile(&mut restored).unwrap();
        drop(restored);
        let restored = Store::open(&database).unwrap();
        let state = restored.load().unwrap();
        assert_eq!(
            state.workers()[&f.source].status(),
            WorkerStatus::Invalidated
        );
        assert_eq!(
            state.reviews()["source-review"].status(),
            ReviewStatus::Accepted
        );
        assert_eq!(
            state.runs()[&RunId::new(&prepared.intent.run_id).unwrap()].state(),
            RunState::Delivered
        );
        assert_eq!(
            state.integrations()[&prepared.intent.id].status(),
            IntegrationStatus::Delivered
        );
        assert_eq!(
            fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
            "after\n"
        );
        fs::write(&source_path, "after\n").unwrap();
    }
    let c = rusqlite::Connection::open(&f.database).unwrap();
    for sql in [
        "DELETE FROM serial_integrations",
        "UPDATE serial_integrations SET intent=intent",
        "DELETE FROM integration_observations",
        "UPDATE integration_observations SET body=body",
    ] {
        assert!(c.execute(sql, []).is_err());
    }
    assert_restore_rejects_rehashed_corruption(&f, &prepared.intent.id);
}

fn assert_restore_rejects_rehashed_corruption(f: &Fixture, id: &str) {
    use rusqlite::params;
    for case in [
        "missing-closure",
        "missing-checks",
        "wrong-check-version",
        "wrong-target-branch",
        "missing-terminal-row",
        "missing-application-history",
        "missing-integration-history",
    ] {
        let path = f.copy_database(&format!("corrupt-{case}"));
        let c = rusqlite::Connection::open(&path).unwrap();
        // Deliberately disable triggers only in a private copied fixture to
        // exercise structural restoration; no DB-owner adversary claim.
        c.execute_batch("DROP TRIGGER integration_observations_no_update;DROP TRIGGER integration_observations_no_delete;").unwrap();
        let status = match case {
            "missing-checks" | "wrong-check-version" => "checked",
            "missing-terminal-row" => "delivered",
            "missing-integration-history" => "integrated",
            _ => "applied",
        };
        let(ordinal,body):(i64,String)=c.query_row("SELECT ordinal,body FROM integration_observations WHERE integration_id=?1 AND json_extract(body,'$.status')=?2",params![id,status],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        if case.starts_with("missing-")
            && matches!(
                case,
                "missing-terminal-row"
                    | "missing-application-history"
                    | "missing-integration-history"
            )
        {
            c.execute(
                "DELETE FROM integration_observations WHERE integration_id=?1 AND ordinal>=?2",
                params![id, ordinal],
            )
            .unwrap();
        } else {
            let mut value: Value = serde_json::from_str(&body).unwrap();
            match case {
                "missing-closure" => {
                    value["details"]
                        .as_object_mut()
                        .unwrap()
                        .remove("localClosureProof");
                }
                "missing-checks" => {
                    value["details"].as_object_mut().unwrap().remove("checks");
                }
                "wrong-check-version" => {
                    value["details"]["checks"][0]["artifact_revision"] = json!("a".repeat(64))
                }
                "wrong-target-branch" => {
                    value["details"]["actualTarget"]["symbolic_ref"] = json!("refs/heads/foreign")
                }
                _ => unreachable!(),
            }
            let body = value.to_string();
            c.execute("UPDATE integration_observations SET body=?3,digest=?4 WHERE integration_id=?1 AND ordinal=?2",params![id,ordinal,body,content_hash(body.as_bytes())]).unwrap();
        }
        drop(c);
        match Store::open(&path) {
            Err(orchestrix_core::persistence::StoreError::InvalidState) => {}
            Ok(store) => assert_eq!(
                store.load().err(),
                Some(orchestrix_core::persistence::StoreError::InvalidState),
                "must reject {case}"
            ),
            Err(other) => panic!("unexpected error for {case}: {other}"),
        }
    }
}

fn crash_child(f: &Fixture, mode: &str, point: &str, request: Value) {
    let config = f.root.join(format!("crash-{point}.json"));
    fs::write(
        &config,
        json!({"database":f.database,"mode":mode,"point":point,"request":request}).to_string(),
    )
    .unwrap();
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(env::current_exe().unwrap());
    c.creation_flags(0x0800_0000)
        .args([
            "--ignored",
            "--exact",
            "integration_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("ORCHESTRIX_INTEGRATION_CHILD", config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(f.root.join(format!("crash-{point}.stderr.log"))).unwrap(),
        ));
    let mut child = c.spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(73));
            break;
        }
        if start.elapsed() >= Duration::from_secs(600) {
            // Only this test's child is owned here. The persisted guardian still
            // governs any Git/check descendants after the parent disappears.
            let _ = child.kill();
            let _ = child.wait();
            panic!("owned test helper deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn await_closure(store: &Store, id: &str) {
    let state = store.load().unwrap();
    let binding: GuardianBinding =
        serde_json::from_value(state.integrations()[id].detail("guardian").unwrap().clone())
            .unwrap();
    drop(state);
    let start = Instant::now();
    while read_proof(&binding).is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "own scope closure proof required"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn crash_after_git_result_before_sql_confirmation_reconciles_without_duplicate_integration() {
    let f = Fixture::new();
    let request = f.request("crashed-integration");
    crash_child(&f, "prepare", "result", json!(request));
    let mut store = Store::open(&f.database).unwrap();
    await_closure(&store, &request.id);
    assert_eq!(
        store.load().unwrap().integrations()[&request.id].status(),
        IntegrationStatus::CommitPlanned
    );
    let service = IntegrationService::new();
    service.reconcile(&mut store).unwrap();
    let record = store.load().unwrap().integrations()[&request.id].clone();
    assert_eq!(record.status(), IntegrationStatus::Prepared);
    let count = record.observations.len();
    service.reconcile(&mut store).unwrap();
    assert_eq!(
        store.load().unwrap().integrations()[&request.id]
            .observations
            .len(),
        count
    );
    let source = store.load().unwrap();
    assert_eq!(
        source.runs()[&RunId::new(&record.intent.run_id).unwrap()]
            .task(&TaskId::new(&record.intent.task_id).unwrap())
            .unwrap()
            .integrations()
            .len(),
        1
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    drop(source);
    service
        .approve(&mut store, &approval(&record, "base-change-approved"))
        .unwrap();
    git(
        &f.repository,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "User advanced the target base",
        ],
    );
    let changed = String::from_utf8(git(&f.repository, &["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .to_owned();
    assert_ne!(changed, f.base);
    assert_eq!(
        service
            .apply(
                &mut store,
                &application(&record, "base-change-apply"),
                &CancellationToken::new()
            )
            .unwrap()
            .status(),
        IntegrationStatus::Invalidated
    );
    assert_eq!(
        String::from_utf8(git(&f.repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim(),
        changed
    );
}
#[test]
fn crash_after_target_cas_confirms_delivery_but_partial_file_update_stays_uncertain() {
    for point in ["files", "target"] {
        let f = Fixture::new();
        let service = IntegrationService::new();
        let mut store = Store::open(&f.database).unwrap();
        let prepared = service
            .prepare(
                &mut store,
                &f.request(&format!("apply-crash-{point}")),
                &CancellationToken::new(),
            )
            .unwrap();
        service
            .approve(&mut store, &approval(&prepared, "crash-approved"))
            .unwrap();
        drop(store);
        let request = application(&prepared, "crashed-application");
        crash_child(&f, "apply", point, json!(request));
        let mut store = Store::open(&f.database).unwrap();
        await_closure(&store, &prepared.intent.id);
        service.reconcile(&mut store).unwrap();
        let state = store.load().unwrap();
        let record = &state.integrations()[&prepared.intent.id];
        assert_eq!(
            fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
            "after\n"
        );
        if point == "target" {
            assert_eq!(record.status(), IntegrationStatus::Delivered);
            assert_eq!(
                state.runs()[&RunId::new(&prepared.intent.run_id).unwrap()].state(),
                RunState::Delivered
            );
        } else {
            assert_eq!(record.status(), IntegrationStatus::Uncertain);
            assert_ne!(
                state.runs()[&RunId::new(&prepared.intent.run_id).unwrap()].state(),
                RunState::Delivered
            );
            assert_eq!(
                String::from_utf8(git(&f.repository, &["rev-parse", "HEAD"]))
                    .unwrap()
                    .trim(),
                f.base
            );
            drop(state);
            assert_eq!(
                service
                    .apply(&mut store, &request, &CancellationToken::new())
                    .unwrap_err()
                    .0,
                "application-uncertain-reconcile-required"
            );
        }
    }
}
#[test]
#[ignore = "Invoked by parent with its own disposable fixture and crash checkpoint"]
fn integration_crash_child() {
    let path = env::var_os("ORCHESTRIX_INTEGRATION_CHILD").expect("private parent config");
    let config: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let mut store = Store::open(Path::new(config["database"].as_str().unwrap())).unwrap();
    let service = IntegrationService::new();
    let point = config["point"].as_str().unwrap();
    let started = Instant::now();
    eprintln!("integration child started; elapsed_ms=0");
    let checkpoint = |p| {
        eprintln!(
            "integration checkpoint={p:?}; elapsed_ms={}",
            started.elapsed().as_millis()
        );
        if matches!(
            (point, p),
            ("result", IntegrationCheckpoint::ResultRef)
                | ("files", IntegrationCheckpoint::FilesApplied)
                | ("target", IntegrationCheckpoint::TargetRef)
        ) {
            std::process::exit(73);
        }
    };
    if config["mode"] == "prepare" {
        let request: IntegrationRequest =
            serde_json::from_value(config["request"].clone()).unwrap();
        service
            .prepare_with_checkpoint(&mut store, &request, &CancellationToken::new(), checkpoint)
            .unwrap();
    } else {
        let request: ApplicationRequest =
            serde_json::from_value(config["request"].clone()).unwrap();
        service
            .apply_with_checkpoint(&mut store, &request, &CancellationToken::new(), checkpoint)
            .unwrap();
    }
    panic!("requested crash checkpoint was not reached");
}
