//! Complete mixed trees, real Git/SQLite/processes; only the runtime is synthetic.
#![cfg(windows)]
use orchestrix_core::{
    evaluator::CheckSpec,
    git::inspect_repository_with_editable,
    git_records::content_hash,
    integration::{IntegrationCheckpoint, IntegrationService},
    integration_records::*,
    persistence::Store,
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    recovery_guardian::{GuardianBinding, read_proof},
    review::ReviewPipeline,
    review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ManualTaskRequest, ManualWorker, WorkerCheckpoint},
    worker_records::WorkerStatus,
};
use serde_json::json;
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["-c", "core.hooksPath=NUL", "-c", "core.autocrlf=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    attempts: PathBuf,
    home: PathBuf,
    db: PathBuf,
    base: String,
    editable: Vec<String>,
    document: bool,
}
impl Fixture {
    fn new(document: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchestrix-opaque-workflow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let repo = root.join("repository");
        fs::create_dir(&repo).unwrap();
        if document {
            let source = PathBuf::from(
                std::env::var_os("ORCHESTRIX_OFFLINE_SOURCE")
                    .expect("explicit local source repository"),
            );
            assert_eq!(
                std::env::var("ORCHESTRIX_OFFLINE_COMMIT").unwrap(),
                "b053a7f"
            );
            // A local clone transports tracked objects/history only; no worktree,
            // appdata, dependencies or uncommitted developer bytes are copied.
            git(
                &root,
                &[
                    "clone",
                    "--quiet",
                    "--no-hardlinks",
                    "--no-checkout",
                    source.to_str().unwrap(),
                    repo.to_str().unwrap(),
                ],
            );
            git(&repo, &["checkout", "--quiet", "-B", "main", "b053a7f"]);
            assert_eq!(
                git(&repo, &["ls-tree", "-r", "-z", "HEAD"]),
                git(&source, &["ls-tree", "-r", "-z", "b053a7f"])
            );
        } else {
            git(&repo, &["init", "--quiet", "--initial-branch=main"]);
            fs::write(repo.join("fixture.txt"), b"before\n").unwrap();
            fs::write(repo.join("image.bin"), [0, 1, 255, 0, 3]).unwrap();
            fs::write(repo.join("non-utf8.bin"), [255, 254, 253]).unwrap();
            fs::write(repo.join("large.txt"), vec![b'x'; 1_048_704]).unwrap();
            fs::write(
                repo.join("opaque-policy.txt"),
                b"API_KEY=synthetic-placeholder-only\n",
            )
            .unwrap();
            git(&repo, &["add", "--all"]);
            git(
                &repo,
                &[
                    "-c",
                    "user.name=Owned QA",
                    "-c",
                    "user.email=qa@invalid.example",
                    "commit",
                    "--quiet",
                    "-m",
                    "Complete mixed base",
                ],
            );
        }
        let base = String::from_utf8(git(&repo, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        let attempts = root.join("attempts");
        let home = root.join("synthetic-home");
        fs::create_dir(&attempts).unwrap();
        fs::create_dir(&home).unwrap();
        Self {
            db: root.join("state.sqlite"),
            root,
            repo,
            attempts,
            home,
            base,
            editable: if document {
                vec!["docs/CONTEXT_OWNERSHIP.md".into()]
            } else {
                vec![
                    "correction.txt".into(),
                    "fixture.txt".into(),
                    "implementation.txt".into(),
                ]
            },
            document,
        }
    }
    fn launch(&self, scenario: &str) -> RuntimeLaunch {
        let mut launch = RuntimeLaunch::fixture(
            PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
            vec!["--scenario".into(), scenario.into()],
            self.home.clone(),
            self.repo.clone(),
        );
        launch.request_timeout = Duration::from_secs(2);
        launch.turn_timeout = Duration::from_secs(3);
        launch
    }
    fn task(&self) -> ManualTaskRequest {
        let mut policy = PolicyDocument::default();
        policy.mode_defaults.fallback = Some(Fallback::Compatible);
        let arguments = if self.document {
            vec![
                "--check-opaque-document".into(),
                content_hash(&fs::read(self.repo.join("docs/CONTEXT_OWNERSHIP.md")).unwrap()),
            ]
        } else {
            vec!["--check-fixture".into(), "after\n".into()]
        };
        ManualTaskRequest{schema_version:2,editable_paths:Some(self.editable.clone()),id:"opaque-implementation".into(),project_id:"opaque-project".into(),repository_id:"opaque-repo".into(),repository_directory:self.repo.clone(),authorized_worktree_directory:self.attempts.clone(),base_commit:self.base.clone(),connection_id:"synthetic-account".into(),connection_revision:1,objective:if self.document{"opaque-document: clarify workspace access versus explicit context in the existing ownership document".into()}else{"fixture-edit: change only fixture.txt; preserve the entire mixed base".into()},acceptance_criteria:vec!["The authorized textual change passes its exact check; all other baseline bytes and modes are preserved".into()],policy,classification:TaskClassification::routine(TaskKind::Implementation,"explicit-owned-offline-proof"),context_files:vec![],instructions:vec![format!("Only these paths may change: {:?}. The Context Pack is not an OS read sandbox. No scripts or network.",self.editable)],checks:vec![CheckSpec{id:"owned-check".into(),executable:PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),arguments,authorization_ref:"explicit-owned-check".into(),authorized:true,timeout_ms:30_000,executable_sha256:None}]}
    }
}
fn execute(document: bool, correction: bool, fault: Option<&str>) {
    let f = Fixture::new(document);
    execute_fixture(f, correction, fault);
}
fn execute_fixture(f: Fixture, correction: bool, fault: Option<&str>) {
    let document = f.document;
    let cancel = CancellationToken::new();
    let all = Instant::now();
    let source_before = git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]);
    let report =
        inspect_repository_with_editable(&f.repo, Some(&f.base), &f.editable, &cancel).unwrap();
    assert!(
        report.compatible && report.application_eligible,
        "{}",
        json!(report)
    );
    if !document {
        assert_eq!(report.inventory.secret_pattern, 1);
        assert_eq!(report.inventory.binary, report.inventory.files - 4);
        assert_eq!(report.inventory.non_utf8, 1);
        assert!(report.inventory.largest_file_bytes > 1_048_576);
    }
    let mut store = Store::open(&f.db).unwrap();
    let task = f.task();
    let start = Instant::now();
    let mut worker_phases = Vec::new();
    let worker = ManualWorker::run_with_checkpoint(
        &mut store,
        &task,
        f.launch(if document {
            "opaque-document"
        } else if correction {
            "review-implementation"
        } else {
            "success"
        }),
        &cancel,
        |point| {
            let elapsed = start.elapsed().as_millis();
            println!("OPAQUE_WORKER_PHASE={point:?} elapsedMs={elapsed}");
            worker_phases.push((format!("{point:?}"), elapsed));
        },
    )
    .unwrap();
    let worker_ms = start.elapsed().as_millis();
    println!("OPAQUE_WORKER_MS={worker_ms}");
    assert!(
        !worker
            .intent
            .context
            .to_string()
            .contains("synthetic-placeholder-only")
    );
    assert_eq!(
        worker.status(),
        WorkerStatus::Verified,
        "{}",
        worker.summary()
    );
    let artifact = store
        .load()
        .unwrap()
        .captured_artifacts()
        .values()
        .find(|a| a.attempt_id == worker.intent.attempt_id)
        .unwrap()
        .clone();
    let manifest = artifact.preservation.as_ref().unwrap();
    assert_eq!(manifest.base_oid, f.base);
    assert!(artifact.files.iter().all(|v| f.editable.contains(&v.path)));
    assert_eq!(
        git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]),
        source_before
    );
    let review_request = |worker_id: String, revision: String, id: &str| ReviewRequest {
        schema_version: 1,
        id: id.into(),
        worker_id,
        artifact_revision: revision,
        authorization_ref: "independent-read-only-review".into(),
        policy: task.policy.clone(),
        classification: task.classification.clone(),
        budget: ReviewBudget {
            max_corrections: 1,
            max_reviews: 2,
            max_elapsed_ms: 1_800_000,
        },
    };
    let review = ReviewPipeline::review(
        &mut store,
        &review_request(
            worker.intent.id.clone(),
            artifact.revision.clone(),
            "opaque-review",
        ),
        f.launch(if correction {
            "review-reject"
        } else {
            "review-accept"
        }),
        &cancel,
    )
    .unwrap();
    let review_context = review.intent.context.to_string();
    assert!(review_context.contains("inventory_sha256"));
    assert!(!review_context.contains("synthetic-placeholder-only"));
    assert!(!review_context.contains("image.bin"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(f.home.join("review-sandbox")).unwrap()
        )
        .unwrap()["type"],
        "readOnly"
    );
    let (final_worker, final_artifact) = if correction {
        assert_eq!(review.status(), ReviewStatus::Rejected);
        let request = ReviewPipeline::prepare_correction(
            &store,
            &review.intent.id,
            "opaque-correction",
            "explicit-limited-correction",
            task.policy.clone(),
            task.classification.clone(),
        )
        .unwrap();
        assert_eq!(request.task.editable_paths, task.editable_paths);
        let corrected =
            ReviewPipeline::correct(&mut store, &request, f.launch("review-correction"), &cancel)
                .unwrap();
        assert_eq!(
            corrected.status(),
            WorkerStatus::Verified,
            "{}",
            corrected.summary()
        );
        let next = store
            .load()
            .unwrap()
            .captured_artifacts()
            .values()
            .find(|a| a.attempt_id == corrected.intent.attempt_id)
            .unwrap()
            .clone();
        assert_eq!(next.preservation, artifact.preservation);
        let accepted = ReviewPipeline::review(
            &mut store,
            &review_request(
                corrected.intent.id.clone(),
                next.revision.clone(),
                "opaque-revised",
            ),
            f.launch("review-accept"),
            &cancel,
        )
        .unwrap();
        assert_eq!(accepted.status(), ReviewStatus::Accepted);
        (corrected, next)
    } else {
        assert_eq!(
            review.status(),
            ReviewStatus::Accepted,
            "{}",
            review.summary()
        );
        (worker, artifact)
    };
    let integration_root = f.root.join("integration");
    fs::create_dir(&integration_root).unwrap();
    let integration_effect_root = integration_root.clone();
    let mut protected_during_materialization = None;
    let start = Instant::now();
    let service = IntegrationService::new();
    let mut integration_phases = Vec::new();
    let request = IntegrationRequest {
        schema_version: 1,
        id: "opaque-integration".into(),
        worker_id: final_worker.intent.id.clone(),
        artifact_revision: final_artifact.revision.clone(),
        authorization_ref: "prepare-exact-isolated-result".into(),
        authorized_integration_directory: integration_root,
        target_directory: f.repo.clone(),
        checks: task.checks.clone(),
    };
    let prepared = service
        .prepare_with_checkpoint(&mut store, &request, &cancel, |point| {
            let elapsed = start.elapsed().as_millis();
            println!("OPAQUE_INTEGRATION_PHASE={point:?} elapsedMs={elapsed}");
            integration_phases.push((format!("{point:?}"), elapsed));
            if point == IntegrationCheckpoint::BaseVerified
                && fault == Some("protected-during-materialization")
            {
                let directories = fs::read_dir(&integration_effect_root)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .filter(|path| path.is_dir())
                    .collect::<Vec<_>>();
                assert_eq!(directories.len(), 1);
                let protected = directories[0].join("image.bin");
                fs::write(&protected, [0, 1, 254, 0, 3]).unwrap();
                protected_during_materialization = Some(protected);
            }
        })
        .unwrap();
    fs::write(f.root.join("stage-progress.json"), serde_json::to_vec_pretty(&json!({"workerMs":worker_ms,"workerPhases":worker_phases,"integrationMs":start.elapsed().as_millis(),"integrationPhases":integration_phases,"status":prepared.status(),"lastObservation":prepared.observations.last()})).unwrap()).unwrap();
    println!(
        "OPAQUE_INTEGRATION_PREPARE_MS={}",
        start.elapsed().as_millis()
    );
    if fault == Some("protected-during-materialization") {
        assert_eq!(prepared.status(), IntegrationStatus::Uncertain);
        let diagnostic = prepared.detail("gitDiagnostic").unwrap();
        assert_eq!(diagnostic["phase"], "materialization");
        assert_eq!(
            diagnostic["classification"],
            "Local Git operation rejected: unpreserved-work"
        );
        assert!(prepared.candidate_revision().is_none());
        let protected = protected_during_materialization.unwrap();
        assert_eq!(fs::read(&protected).unwrap(), [0, 1, 254, 0, 3]);
        assert_eq!(
            fs::read(protected.with_file_name("fixture.txt")).unwrap(),
            b"after\n"
        );
        assert_eq!(fs::read(f.repo.join("fixture.txt")).unwrap(), b"before\n");
        assert_eq!(
            git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]),
            source_before
        );
        let ledger = fs::read(f.home.join("dispatch-count")).unwrap();
        drop(store);
        let reopened = Store::open(&f.db).unwrap();
        let state = reopened.load().unwrap();
        assert_eq!(
            state.integrations()[&request.id].status(),
            IntegrationStatus::Uncertain
        );
        assert_eq!(
            state.captured_artifacts()
                [&(final_artifact.id.clone(), final_artifact.revision.clone())],
            final_artifact
        );
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
        println!(
            "OPAQUE_MATERIALIZATION_BLOCKED_EVIDENCE={}",
            f.root.display()
        );
        return;
    }
    assert_eq!(
        prepared.status(),
        IntegrationStatus::Prepared,
        "{}",
        prepared.summary()
    );
    let candidate = prepared.candidate_revision().unwrap();
    assert_eq!(
        prepared.intent.worktree.preservation,
        final_artifact.preservation
    );
    service
        .approve(
            &mut store,
            &ApprovalRequest {
                schema_version: 1,
                id: "opaque-approval".into(),
                integration_id: request.id.clone(),
                candidate_revision: candidate.clone(),
                authorization_ref: "human-approved-exact-candidate".into(),
                valid_for_ms: 120_000,
            },
        )
        .unwrap();
    let apply = ApplicationRequest {
        schema_version: 1,
        id: "opaque-application".into(),
        integration_id: request.id.clone(),
        candidate_revision: candidate,
        authorization_ref: "explicit-isolated-target-application".into(),
    };
    if matches!(
        fault,
        Some("protected-after-approval" | "protected-target-after-approval")
    ) {
        let target_changed = fault == Some("protected-target-after-approval");
        let integration_root = Path::new(&prepared.intent.worktree.path);
        let source_root = Path::new(&final_artifact.origin);
        let source_text = fs::read(source_root.join("fixture.txt")).unwrap();
        let source_protected = fs::read(source_root.join("image.bin")).unwrap();
        let integration_text = fs::read(integration_root.join("fixture.txt")).unwrap();
        let integration_protected = fs::read(integration_root.join("image.bin")).unwrap();
        let target_index = fs::read(f.repo.join(".git/index")).unwrap();
        let approved = store.load().unwrap().integrations()[&request.id].clone();
        let ledger = fs::read(f.home.join("dispatch-count")).unwrap();
        let protected = if target_changed {
            f.repo.join("image.bin")
        } else {
            integration_root.join("image.bin")
        };
        let changed = [0, 1, 254, 0, 3];
        fs::write(&protected, changed).unwrap();
        let invalid = service.apply(&mut store, &apply, &cancel).unwrap();
        assert_eq!(invalid.status(), IntegrationStatus::Invalidated);
        assert_eq!(
            invalid.detail("reason"),
            Some(&json!("protected-or-unlisted-content-changed"))
        );
        assert_eq!(invalid.detail("applicationNotStarted"), Some(&json!(true)));
        assert_eq!(invalid.detail("inspectionRequired"), Some(&json!(false)));
        assert_eq!(invalid.snapshot(), approved.snapshot());
        assert_eq!(invalid.detail("checks"), approved.detail("checks"));
        assert_eq!(invalid.detail("approval"), approved.detail("approval"));
        assert_eq!(invalid.candidate_revision(), approved.candidate_revision());
        assert_eq!(
            store.load().unwrap().captured_artifacts()
                [&(final_artifact.id.clone(), final_artifact.revision.clone())],
            final_artifact
        );
        assert_eq!(fs::read(&protected).unwrap(), changed);
        assert_eq!(
            fs::read(source_root.join("fixture.txt")).unwrap(),
            source_text
        );
        assert_eq!(
            fs::read(source_root.join("image.bin")).unwrap(),
            source_protected
        );
        if target_changed {
            assert_eq!(
                fs::read(integration_root.join("fixture.txt")).unwrap(),
                integration_text
            );
            assert_eq!(
                fs::read(integration_root.join("image.bin")).unwrap(),
                integration_protected
            );
        }
        assert_eq!(fs::read(f.repo.join("fixture.txt")).unwrap(), b"before\n");
        assert_eq!(fs::read(f.repo.join(".git/index")).unwrap(), target_index);
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
        assert_eq!(
            git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]),
            source_before
        );
        drop(store);
        let mut reopened = Store::open(&f.db).unwrap();
        assert!(service.apply(&mut reopened, &apply, &cancel).is_err());
        service.reconcile(&mut reopened).unwrap();
        assert_eq!(
            reopened.load().unwrap().integrations()[&request.id].status(),
            IntegrationStatus::Invalidated
        );
        assert_eq!(
            reopened.load().unwrap().captured_artifacts()
                [&(final_artifact.id.clone(), final_artifact.revision.clone())],
            final_artifact
        );
        assert_eq!(fs::read(&protected).unwrap(), changed);
        assert_eq!(fs::read(f.repo.join("fixture.txt")).unwrap(), b"before\n");
        assert_eq!(fs::read(f.repo.join(".git/index")).unwrap(), target_index);
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
        println!("OPAQUE_INVALIDATED_EVIDENCE={}", f.root.display());
        return;
    }
    let delivered = if fault == Some("crash-after-target") {
        let config = f.root.join("application-crash.json");
        fs::write(
            &config,
            serde_json::to_vec(&json!({"database":f.db,"request":apply})).unwrap(),
        )
        .unwrap();
        drop(store);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .creation_flags(0x0800_0000)
            .args([
                "--ignored",
                "--exact",
                "opaque_application_crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("ORCHESTRIX_OPAQUE_CRASH", config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::File::create(f.root.join("application-crash.stderr.log")).unwrap(),
            ))
            .spawn()
            .unwrap();
        let child_start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(73));
                break;
            }
            if child_start.elapsed() >= Duration::from_secs(300) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned application test child deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        store = Store::open(&f.db).unwrap();
        let interrupted = store.load().unwrap().integrations()[&request.id].clone();
        assert_eq!(interrupted.status(), IntegrationStatus::Applying);
        let guardian: GuardianBinding =
            serde_json::from_value(interrupted.detail("guardian").unwrap().clone()).unwrap();
        let closure_start = Instant::now();
        while read_proof(&guardian).is_err() {
            assert!(closure_start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        service.reconcile(&mut store).unwrap();
        store.load().unwrap().integrations()[&request.id].clone()
    } else {
        service.apply(&mut store, &apply, &cancel).unwrap()
    };
    assert_eq!(
        delivered.status(),
        IntegrationStatus::Delivered,
        "{}",
        delivered.summary()
    );
    let integration_ms = start.elapsed().as_millis();
    let result_entries = String::from_utf8(git(&f.repo, &["ls-tree", "-r", "-l", "HEAD"])).unwrap();
    for entry in &final_artifact.preservation.as_ref().unwrap().entries {
        if !f.editable.contains(&entry.path) {
            assert!(
                result_entries.lines().any(|line| line
                    .starts_with(&format!("{} blob {}", entry.mode, entry.git_oid))
                    && line.ends_with(&format!("\t{}", entry.path))),
                "protected entry changed: {}",
                entry.path
            );
            assert_eq!(
                content_hash(&fs::read(f.repo.join(&entry.path)).unwrap()),
                entry.sha256
            );
        }
    }
    assert!(
        git(
            &f.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
        .is_empty()
    );
    let ledger = fs::read(f.home.join("dispatch-count")).unwrap();
    drop(store);
    let mut reopened = Store::open(&f.db).unwrap();
    assert_eq!(
        service
            .apply(&mut reopened, &apply, &cancel)
            .unwrap()
            .status(),
        IntegrationStatus::Delivered
    );
    assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
    assert_eq!(
        ManualWorker::inspect(&mut reopened, &final_worker.intent.id)
            .unwrap()
            .intent
            .editable_paths,
        task.editable_paths
    );
    fs::write(f.root.join("result.json"),serde_json::to_vec_pretty(&json!({"snapshot":f.base,"document":document,"synthetic":true,"providerReads":0,"preflight":report,"artifact":final_artifact.revision,"preservation":final_artifact.preservation,"workerMs":worker_ms,"workerPhases":worker_phases,"integrationMs":integration_ms,"integrationPhases":integration_phases,"totalMs":all.elapsed().as_millis(),"result":delivered.summary(),"untouchedObjectsAndBytesEqual":true,"reopenedWithoutDispatch":true})).unwrap()).unwrap();
    println!("OPAQUE_EVIDENCE={}", f.root.display());
}
#[test]
fn mixed_base_checks_review_apply_and_reopen() {
    execute(false, false, None)
}
#[test]
fn mixed_base_correction_retains_exact_provenance() {
    execute(false, true, None)
}
#[test]
fn thousand_preserved_entries_survive_persistence_review_application_and_reopen() {
    let mut f = Fixture::new(false);
    let directory = f.repo.join("preserved");
    fs::create_dir(&directory).unwrap();
    for index in 0..1000 {
        fs::write(directory.join(format!("file-{index:04}.bin")), [0, 255]).unwrap();
    }
    git(&f.repo, &["add", "--all"]);
    git(
        &f.repo,
        &[
            "-c",
            "user.name=Owned QA",
            "-c",
            "user.email=qa@invalid.example",
            "commit",
            "--quiet",
            "-m",
            "Bounded complete inventory",
        ],
    );
    f.base = String::from_utf8(git(&f.repo, &["rev-parse", "HEAD"]))
        .unwrap()
        .trim()
        .into();
    execute_fixture(f, false, None);
}
#[test]
#[ignore = "explicit complete local snapshot, no provider access"]
fn complete_orchestrix_snapshot_offline() {
    execute(true, false, None)
}

#[test]
fn protected_change_after_approval_blocks_application_without_restoration() {
    execute(false, false, Some("protected-after-approval"))
}
#[test]
fn protected_target_change_after_approval_invalidates_before_application_without_restoration() {
    execute(false, false, Some("protected-target-after-approval"))
}
#[test]
fn protected_change_after_fresh_base_verification_blocks_materialization_without_restoration() {
    execute(false, false, Some("protected-during-materialization"))
}
#[test]
fn mixed_base_crash_after_target_effect_reconciles_without_reapplying() {
    execute(false, false, Some("crash-after-target"))
}
#[test]
#[ignore = "private owned application crash child; no provider access"]
fn opaque_application_crash_child() {
    let config: serde_json::Value = serde_json::from_slice(
        &fs::read(std::env::var_os("ORCHESTRIX_OPAQUE_CRASH").unwrap()).unwrap(),
    )
    .unwrap();
    let mut store = Store::open(Path::new(config["database"].as_str().unwrap())).unwrap();
    let request: ApplicationRequest = serde_json::from_value(config["request"].clone()).unwrap();
    IntegrationService::new()
        .apply_with_checkpoint(&mut store, &request, &CancellationToken::new(), |point| {
            if point == IntegrationCheckpoint::TargetRef {
                std::process::exit(73);
            }
        })
        .unwrap();
    panic!("application crash checkpoint not reached");
}

#[test]
fn protected_mutation_during_check_or_capture_never_becomes_verified_or_retries() {
    for phase in [
        WorkerCheckpoint::CaptureReading,
        WorkerCheckpoint::Checking,
        WorkerCheckpoint::CheckStarted,
    ] {
        let f = Fixture::new(false);
        let mut store = Store::open(&f.db).unwrap();
        let task = f.task();
        let mut changed = None;
        let record = ManualWorker::run_with_checkpoint(
            &mut store,
            &task,
            f.launch("success"),
            &CancellationToken::new(),
            |point| {
                if point == phase && changed.is_none() {
                    let worktree = fs::read_dir(&f.attempts)
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .find(|path| path.is_dir())
                        .unwrap();
                    let path = worktree.join("image.bin");
                    fs::write(&path, [0, 1, 254, 0, 3]).unwrap();
                    changed = Some(path);
                }
            },
        )
        .unwrap();
        assert!(changed.is_some(), "requested checkpoint must be reached");
        assert_ne!(record.status(), WorkerStatus::Verified);
        assert_eq!(
            record.summary()["details"]["captureDiagnostic"]["gitCode"],
            "unpreserved-work"
        );
        assert_eq!(fs::read(changed.unwrap()).unwrap(), [0, 1, 254, 0, 3]);
        let dispatch = fs::read(f.home.join("dispatch-count")).unwrap();
        drop(store);
        let mut reopened = Store::open(&f.db).unwrap();
        let replay = ManualWorker::run(
            &mut reopened,
            &task,
            f.launch("success"),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_ne!(replay.status(), WorkerStatus::Verified);
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), dispatch);
        assert_eq!(
            fs::read(f.repo.join("image.bin")).unwrap(),
            [0, 1, 255, 0, 3]
        );
    }
}

#[test]
fn v2_staged_content_is_preserved_but_not_reviewed_without_its_textual_evidence() {
    let f = Fixture::new(false);
    let mut store = Store::open(&f.db).unwrap();
    let mut staged = false;
    let worker = ManualWorker::run_with_checkpoint(
        &mut store,
        &f.task(),
        f.launch("success"),
        &CancellationToken::new(),
        |point| {
            if point == WorkerCheckpoint::RuntimeObserved && !staged {
                let worktree = fs::read_dir(&f.attempts)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| path.is_dir())
                    .unwrap();
                let path = worktree.join("fixture.txt");
                let actual = fs::read(&path).unwrap();
                fs::write(&path, "staged-only\n").unwrap();
                git(&worktree, &["add", "--", "fixture.txt"]);
                fs::write(&path, actual).unwrap();
                staged = true;
            }
        },
    )
    .unwrap();
    assert!(staged);
    assert_eq!(
        worker.status(),
        WorkerStatus::Verified,
        "{}",
        worker.summary()
    );
    let state = store.load().unwrap();
    let artifact = state
        .captured_artifacts()
        .values()
        .find(|artifact| artifact.attempt_id == worker.intent.attempt_id)
        .unwrap()
        .clone();
    assert!(artifact.files.iter().any(|file| {
        file.index
            .as_ref()
            .is_some_and(|index| index.content == "staged-only\n")
    }));
    drop(state);
    let reads = fs::read(f.home.join("metadata-reads")).unwrap();
    let dispatch = fs::read(f.home.join("dispatch-count")).unwrap();
    let error = ReviewPipeline::review(
        &mut store,
        &ReviewRequest {
            schema_version: 1,
            id: "staged-review".into(),
            worker_id: worker.intent.id,
            artifact_revision: artifact.revision.clone(),
            authorization_ref: "explicit-review".into(),
            policy: f.task().policy,
            classification: f.task().classification,
            budget: ReviewBudget {
                max_corrections: 1,
                max_reviews: 2,
                max_elapsed_ms: 1_800_000,
            },
        },
        f.launch("review-accept"),
        &CancellationToken::new(),
    )
    .unwrap_err();
    assert_eq!(error.0, "review-staged-source-outside-profile");
    assert_eq!(fs::read(f.home.join("metadata-reads")).unwrap(), reads);
    assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), dispatch);
    assert!(store.load().unwrap().reviews().is_empty());
    let reloaded = Store::open(&f.db).unwrap().load().unwrap();
    assert_eq!(
        serde_json::to_vec(
            &reloaded.captured_artifacts()[&(artifact.id.clone(), artifact.revision.clone())]
        )
        .unwrap(),
        serde_json::to_vec(&artifact).unwrap()
    );
}
