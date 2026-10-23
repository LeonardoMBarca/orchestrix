//! Complete mixed trees, real Git/SQLite/processes; only the runtime is synthetic.
#![cfg(windows)]
use orchestrix_core::{
    daemon::{API_VERSION, Client, Operation, Request},
    desktop::DesktopRequest,
    evaluator::CheckSpec,
    git::inspect_repository_with_editable,
    git_records::content_hash,
    integration::{IntegrationCheckpoint, IntegrationService},
    integration_records::*,
    persistence::{DaemonCursor, Store},
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    recovery_guardian::{GuardianBinding, read_proof},
    review::ReviewPipeline,
    review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ManualTaskRequest, ManualWorker, WorkerCheckpoint},
    worker_records::WorkerStatus,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// Test-only byte inventory of a confirmed own object store. Never authority for
// the product, and never a scan of the developer's repository or credentials.
fn own_object_bytes(root: &Path) -> BTreeMap<PathBuf, (u64, String)> {
    let directory = root.join(".git/objects");
    let mut pending = vec![directory];
    let mut inventory = BTreeMap::new();
    let mut total = 0u64;
    let mut entries = 0usize;
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        entries += 1;
        assert!(entries <= 16_384);
        if metadata.is_dir() {
            pending.extend(
                fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else {
            assert!(metadata.is_file());
            let mut file = fs::File::open(&path).unwrap();
            let mut hash = Sha256::new();
            let mut bytes = 0u64;
            let mut buffer = [0u8; 65_536];
            loop {
                let count = file.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                bytes += count as u64;
                total += count as u64;
                assert!(total <= 160 * 1_048_576);
                hash.update(&buffer[..count]);
            }
            assert_eq!(bytes, metadata.len());
            inventory.insert(path, (bytes, format!("{:x}", hash.finalize())));
        }
    }
    inventory
}

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    let out = command
        .current_dir(root)
        .args(["-c", "core.hooksPath=NUL", "-c", "core.autocrlf=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
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
            // Transport preserves the full reachable history in packfiles;
            // it neither prunes the source nor weakens object-store validation.
            git(
                &root,
                &[
                    "clone",
                    "--quiet",
                    "--no-local",
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
    execute_fixture(f, correction, fault, None);
}
#[derive(Clone, Copy, Debug)]
enum PreparationMeasurement {
    Direct,
    Daemon,
    DaemonDesktop,
}

struct MeasurementDaemon {
    child: Child,
    client: Client,
    clock: Instant,
    rpc_log: fs::File,
    sequence: u64,
}
impl MeasurementDaemon {
    fn start(f: &Fixture) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
            .creation_flags(0x0800_0000)
            .args(["daemon", "--database"])
            .arg(&f.db)
            .stdin(Stdio::null())
            .stdout(fs::File::create_new(f.root.join("measurement-daemon.stdout.log")).unwrap())
            .stderr(fs::File::create_new(f.root.join("measurement-daemon.stderr.log")).unwrap())
            .spawn()
            .unwrap();
        Self {
            child,
            client: Client::new(&f.db).unwrap(),
            clock: Instant::now(),
            rpc_log: fs::File::create_new(f.root.join("measurement-rpc.jsonl")).unwrap(),
            sequence: 0,
        }
    }
    fn request(&mut self, operation: Operation, label: &'static str) -> Result<Value, String> {
        self.sequence += 1;
        let sequence = self.sequence;
        let span = Instant::now();
        writeln!(
            self.rpc_log,
            "{}",
            json!({"schemaVersion":1,"sequence":sequence,
            "phase":label,"event":"begin","elapsedMicros":self.clock.elapsed().as_micros()})
        )
        .unwrap();
        self.rpc_log.flush().unwrap();
        let result = self
            .client
            .request(&Request {
                api_version: API_VERSION,
                operation,
            })
            .and_then(|response| {
                if response.ok {
                    Ok(response.result)
                } else {
                    Err(response.error.unwrap_or_else(|| "daemon-rejected".into()))
                }
            });
        writeln!(
            self.rpc_log,
            "{}",
            json!({"schemaVersion":1,"sequence":sequence,
            "phase":label,"event":"end","elapsedMicros":self.clock.elapsed().as_micros(),
            "spanMicros":span.elapsed().as_micros(),"ok":result.is_ok(),
            "error":result.as_ref().err()})
        )
        .unwrap();
        self.rpc_log.flush().unwrap();
        result
    }
    fn stop(mut self) {
        let snapshot = self
            .request(Operation::Snapshot, "shutdown-snapshot")
            .unwrap();
        assert!(snapshot["activeWorkerId"].is_null());
        assert!(snapshot["activeOperationKind"].is_null());
        self.request(
            Operation::Shutdown {
                command_id: "measurement-shutdown".into(),
            },
            "shutdown",
        )
        .unwrap();
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "own measurement daemon exit {status}");
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(25),
                "own daemon shutdown deadline"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for MeasurementDaemon {
    fn drop(&mut self) {
        // This handle belongs only to the child created by this test. Keep all
        // fixture data, and never terminate by executable name or discover PID.
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn measure_daemon_preparation(
    f: &Fixture,
    request: &IntegrationRequest,
    desktop_poll: bool,
) -> IntegrationRecord {
    let startup = Instant::now();
    let mut daemon = MeasurementDaemon::start(f);
    loop {
        if daemon
            .request(Operation::Snapshot, "startup-snapshot")
            .is_ok()
        {
            break;
        }
        assert!(
            daemon.child.try_wait().unwrap().is_none(),
            "own daemon exited during startup"
        );
        assert!(
            startup.elapsed() < Duration::from_secs(300),
            "measurement startup deadline"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    let startup_ms = startup.elapsed().as_millis();
    daemon
        .request(
            Operation::Resume {
                command_id: "measurement-resume".into(),
            },
            "resume",
        )
        .unwrap();
    let mut bootstrap = daemon
        .request(
            Operation::Desktop {
                request: Box::new(DesktopRequest::Bootstrap),
            },
            "bootstrap-revision",
        )
        .unwrap();
    // Normal Desktop messages establish a conversation, never worker authority.
    // This measured profile uses the same event/snapshot/bootstrap/message-page
    // reads as App.tsx, rather than attributing a Snapshot-only probe to the UI.
    let conversation_id = "measurement-desktop-conversation";
    if desktop_poll {
        daemon
            .request(
                Operation::Desktop {
                    request: Box::new(DesktopRequest::SendMessage {
                        command_id: "measurement-desktop-message".into(),
                        expected_revision: bootstrap["revision"].as_u64().unwrap(),
                        conversation_id: conversation_id.into(),
                        project_id: Some("opaque-project".into()),
                        content: "Inspect the complete-snapshot result before integration.".into(),
                    }),
                },
                "desktop-send-message",
            )
            .unwrap();
        bootstrap = daemon
            .request(
                Operation::Desktop {
                    request: Box::new(DesktopRequest::Bootstrap),
                },
                "desktop-bootstrap",
            )
            .unwrap();
    }
    let mut cursor: DaemonCursor =
        serde_json::from_value(bootstrap["eventCursor"].clone()).unwrap();
    let operation = Operation::PrepareIntegration {
        command_id: "measurement-prepare".into(),
        expected_revision: bootstrap["revision"].as_u64().unwrap(),
        request: Box::new(request.clone()),
    };
    let fingerprint = content_hash(&serde_json::to_vec(&operation).unwrap());
    fs::write(
        f.root.join("measurement-prepare-request.json"),
        serde_json::to_vec_pretty(&operation).unwrap(),
    )
    .unwrap();
    let start = Instant::now();
    // A missing response never generates another preparation operation. Only
    // the exact original receipt identity is queried below.
    let acknowledgement = daemon.request(operation, "prepare-submit");
    let mut poll_count = 0_u64;
    let outcome = loop {
        assert!(
            daemon.child.try_wait().unwrap().is_none(),
            "own daemon exited during preparation"
        );
        poll_count += 1;
        if desktop_poll {
            let events = daemon
                .request(
                    Operation::Events {
                        cursor: cursor.clone(),
                        limit: 128,
                    },
                    "desktop-poll-events",
                )
                .unwrap();
            cursor = serde_json::from_value(events["cursor"].clone()).unwrap();
            if events["resyncRequired"] == true
                || events["events"].as_array().is_some_and(|e| !e.is_empty())
            {
                daemon
                    .request(
                        Operation::Desktop {
                            request: Box::new(DesktopRequest::Bootstrap),
                        },
                        "desktop-event-bootstrap",
                    )
                    .unwrap();
                daemon
                    .request(
                        Operation::Desktop {
                            request: Box::new(DesktopRequest::Conversation {
                                conversation_id: conversation_id.into(),
                                after_ordinal: 0,
                                limit: 16,
                            }),
                        },
                        "desktop-message-page",
                    )
                    .unwrap();
            }
            if events["events"]
                .as_array()
                .is_some_and(|e| e.iter().any(|event| event["kind"] == "command-finished"))
            {
                daemon
                    .request(
                        Operation::Desktop {
                            request: Box::new(DesktopRequest::InspectWorker {
                                worker_id: request.worker_id.clone(),
                            }),
                        },
                        "desktop-completion-worker",
                    )
                    .unwrap();
                daemon
                    .request(
                        Operation::InspectIntegration {
                            integration_id: request.id.clone(),
                        },
                        "desktop-completion-integration",
                    )
                    .unwrap();
            }
        }
        let _snapshot = daemon.request(Operation::Snapshot, "poll-snapshot");
        if let Ok(receipt) = daemon.request(
            Operation::CommandReceipt {
                command_id: "measurement-prepare".into(),
                fingerprint: fingerprint.clone(),
            },
            "poll-command-receipt",
        ) && !receipt["receipt"]["outcome"].is_null()
        {
            break receipt["receipt"]["outcome"].clone();
        }
        // An outer observation guard covers unbounded pre-Job work and the
        // unchanged 300-second owned Job. It does not alter a production budget.
        assert!(
            start.elapsed() < Duration::from_secs(600),
            "preparation observation deadline"
        );
        std::thread::sleep(Duration::from_millis(1200));
    };
    let request_envelope_ms = start.elapsed().as_millis();
    let inspection = daemon
        .request(
            Operation::InspectIntegration {
                integration_id: request.id.clone(),
            },
            "historical-integration-inspection",
        )
        .unwrap();
    fs::write(f.root.join("daemon-preparation-measurement.json"),
        serde_json::to_vec_pretty(&json!({"schemaVersion":1,
            "startupMs":startup_ms,"requestEnvelopeMs":request_envelope_ms,
            "commandId":"measurement-prepare","fingerprint":fingerprint,
            "acknowledgement":acknowledgement,"outcome":outcome,
            "pollCount":poll_count,"pollIntervalMs":1200,
            "clientProfile":if desktop_poll {"Desktop Events/Snapshot plus event-triggered Bootstrap/Conversation and completion-triggered InspectWorker; exact original CommandReceipt; no renderer or native bridge"} else {"Snapshot plus CommandReceipt; not the complete Desktop event/bootstrap profile"},
            "inspection":inspection,"providerCalled":false})).unwrap()).unwrap();
    daemon.stop();
    let reopened = Store::open(&f.db).unwrap();
    reopened.load().unwrap().integrations()[&request.id].clone()
}

fn execute_fixture(
    f: Fixture,
    correction: bool,
    fault: Option<&str>,
    measurement: Option<PreparationMeasurement>,
) {
    let document = f.document;
    println!("OPAQUE_FIXTURE={}", f.root.display());
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
    assert_eq!(
        review.status(),
        if correction {
            ReviewStatus::Rejected
        } else {
            ReviewStatus::Accepted
        },
        "{}",
        review.summary()
    );
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
    let mut request = IntegrationRequest {
        schema_version: 1,
        id: "opaque-integration".into(),
        worker_id: final_worker.intent.id.clone(),
        artifact_revision: final_artifact.revision.clone(),
        authorization_ref: "prepare-exact-isolated-result".into(),
        authorized_integration_directory: integration_root,
        target_directory: f.repo.clone(),
        checks: task.checks.clone(),
    };
    let restored_marker = f.home.join("integration-check-restored.json");
    if fault == Some("integration-transient-check") {
        request.checks[0].arguments = vec![
            "--mutate-restore-fixture-check".into(),
            "--restore-metadata".into(),
            restored_marker.to_str().unwrap().into(),
        ];
        request.checks[0].authorization_ref = "explicit-transient-check-test".into();
    }
    let prepared = if matches!(
        measurement,
        Some(PreparationMeasurement::Daemon | PreparationMeasurement::DaemonDesktop)
    ) {
        drop(store);
        let record = measure_daemon_preparation(
            &f,
            &request,
            matches!(measurement, Some(PreparationMeasurement::DaemonDesktop)),
        );
        // The daemon has stopped and its ownership lease has closed before the
        // diagnostic reopens SQLite. There is no concurrent second Store owner.
        store = Store::open(&f.db).unwrap();
        record
    } else {
        service
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
            .unwrap()
    };
    fs::write(f.root.join("stage-progress.json"), serde_json::to_vec_pretty(&json!({"workerMs":worker_ms,"workerPhases":worker_phases,"integrationMs":start.elapsed().as_millis(),"integrationPhases":integration_phases,"status":prepared.status(),"lastObservation":prepared.observations.last()})).unwrap()).unwrap();
    println!(
        "OPAQUE_INTEGRATION_PREPARE_MS={}",
        start.elapsed().as_millis()
    );
    if let Some(path) = measurement {
        assert!(document && !correction && fault.is_none());
        let unchanged = git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]) == source_before;
        let evidence = json!({
            "schemaVersion":1,
            "scope":"synthetic complete-snapshot preparation measurement; not native UI or provider evidence",
            "path":format!("{path:?}"),"fixture":f.root,"database":f.db,
            "base":f.base,"editablePaths":f.editable,
            "workerId":final_worker.intent.id,"reviewId":review.intent.id,
            "artifactRevision":final_artifact.revision,
            "workerMs":worker_ms,"workerPhases":worker_phases,
            "integrationPathMs":start.elapsed().as_millis(),
            "integrationPhases":integration_phases,"integration":prepared,
            "sourceTreeUnchanged":unchanged,
            "traceDirectory":std::env::var_os("ORCHESTRIX_INTEGRATION_DIAGNOSTICS_DIRECTORY")
                .map(|value| value.to_string_lossy().into_owned()),
            "applied":false,"providerCalled":false
        });
        fs::write(
            f.root.join("preparation-measurement.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
        println!("OPAQUE_PREPARATION_MEASUREMENT={}", f.root.display());
        assert!(unchanged);
        assert_eq!(
            prepared.status(),
            IntegrationStatus::Prepared,
            "{}",
            prepared.summary()
        );
        assert!(prepared.candidate_revision().is_some());
        assert!(prepared.detail("approval").is_none());
        assert!(prepared.detail("application").is_none());
        return;
    }
    if fault == Some("integration-transient-check") {
        assert_eq!(prepared.status(), IntegrationStatus::Uncertain);
        assert_eq!(
            prepared.detail("reason"),
            Some(&json!("combined-artifact-changed"))
        );
        assert!(prepared.candidate_revision().is_none());
        let marker: Value = serde_json::from_slice(&fs::read(&restored_marker).unwrap()).unwrap();
        assert_eq!(
            marker,
            json!({"bytesRestored":true,"modifiedTimeRestored":true})
        );
        assert_eq!(
            fs::read(Path::new(&prepared.intent.worktree.path).join("fixture.txt")).unwrap(),
            b"after\n"
        );
        assert_eq!(
            git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]),
            source_before
        );
        let ledger = fs::read(f.home.join("dispatch-count")).unwrap();
        drop(store);
        let mut reopened = Store::open(&f.db).unwrap();
        let replay = service.prepare(&mut reopened, &request, &cancel).unwrap();
        assert_eq!(
            serde_json::to_value(&replay).unwrap(),
            serde_json::to_value(&prepared).unwrap()
        );
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
        assert_eq!(
            reopened.load().unwrap().captured_artifacts()
                [&(final_artifact.id.clone(), final_artifact.revision.clone())],
            final_artifact
        );
        return;
    }
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
    let approval = ApprovalRequest {
        schema_version: 1,
        id: "opaque-approval".into(),
        integration_id: request.id.clone(),
        candidate_revision: candidate.clone(),
        authorization_ref: "human-approved-exact-candidate".into(),
        valid_for_ms: 120_000,
    };
    if fault == Some("missing-candidate-blob") {
        let result = prepared.detail("resultRevision").unwrap().as_str().unwrap();
        let oid = String::from_utf8(git(
            &f.repo,
            &["rev-parse", "--verify", &format!("{result}:fixture.txt")],
        ))
        .unwrap();
        let oid = oid.trim();
        assert_eq!(oid.len(), 40);
        assert!(oid.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let objects = fs::canonicalize(f.repo.join(".git/objects")).unwrap();
        let object = fs::canonicalize(objects.join(&oid[..2]).join(&oid[2..])).unwrap();
        assert!(object.starts_with(&objects) && object.is_file());
        // Git may mark its loose objects read-only on Windows. Only this exact
        // owned fixture object is made writable for the missing-object fault.
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Windows-only owned loose object; never a developer repository"
        )]
        {
            let mut permissions = fs::metadata(&object).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&object, permissions).unwrap();
        }
        fs::remove_file(&object).unwrap();
        let before = own_object_bytes(&f.repo);
        let ledger = fs::read(f.home.join("dispatch-count")).unwrap();
        let revision = store.load().unwrap().revision();
        assert_eq!(
            service.approve(&mut store, &approval).unwrap_err().0,
            "git-inspection-required"
        );
        assert_eq!(
            own_object_bytes(&f.repo),
            before,
            "candidate verification wrote/repaired Git objects"
        );
        assert!(!object.exists());
        assert_eq!(store.load().unwrap().revision(), revision);
        assert_eq!(
            serde_json::to_value(&store.load().unwrap().integrations()[&request.id]).unwrap(),
            serde_json::to_value(&prepared).unwrap()
        );
        assert_eq!(fs::read(f.home.join("dispatch-count")).unwrap(), ledger);
        assert_eq!(
            git(&f.repo, &["ls-tree", "-r", "-l", "-z", "HEAD"]),
            source_before
        );
        assert_eq!(fs::read(f.repo.join("fixture.txt")).unwrap(), b"before\n");
        println!("OPAQUE_MISSING_CANDIDATE_BLOCKED={}", f.root.display());
        return;
    }
    let objects_before_approval = own_object_bytes(&f.repo);
    service.approve(&mut store, &approval).unwrap();
    assert_eq!(
        own_object_bytes(&f.repo),
        objects_before_approval,
        "approval verification wrote Git objects"
    );
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
    execute_fixture(f, false, None, None);
}
#[test]
#[ignore = "explicit complete local snapshot, no provider access"]
fn complete_orchestrix_snapshot_offline() {
    execute(true, false, None)
}

#[test]
#[ignore = "explicit complete local snapshot preparation measurement; no provider access"]
fn complete_snapshot_direct_preparation_measurement() {
    execute_fixture(
        Fixture::new(true),
        false,
        None,
        Some(PreparationMeasurement::Direct),
    );
}

#[test]
#[ignore = "explicit complete local snapshot daemon preparation measurement; no provider access"]
fn complete_snapshot_daemon_preparation_measurement() {
    execute_fixture(
        Fixture::new(true),
        false,
        None,
        Some(PreparationMeasurement::Daemon),
    );
}

#[test]
#[ignore = "explicit full snapshot, normal daemon API and Desktop polling; synthetic only"]
fn complete_snapshot_daemon_desktop_preparation_measurement() {
    execute_fixture(
        Fixture::new(true),
        false,
        None,
        Some(PreparationMeasurement::DaemonDesktop),
    );
}

#[test]
fn daemon_desktop_measurement_polling_uses_real_pages_without_runtime_calls() {
    let f = Fixture::new(false);
    let mut daemon = MeasurementDaemon::start(&f);
    let start = Instant::now();
    let bootstrap = loop {
        if let Ok(value) = daemon.request(
            Operation::Desktop {
                request: Box::new(DesktopRequest::Bootstrap),
            },
            "poll-smoke-bootstrap",
        ) {
            break value;
        }
        assert!(start.elapsed() < Duration::from_secs(300));
        assert!(daemon.child.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(250));
    };
    let cursor: DaemonCursor = serde_json::from_value(bootstrap["eventCursor"].clone()).unwrap();
    daemon
        .request(
            Operation::Desktop {
                request: Box::new(DesktopRequest::SendMessage {
                    command_id: "poll-smoke-message".into(),
                    expected_revision: bootstrap["revision"].as_u64().unwrap(),
                    conversation_id: "poll-smoke-conversation".into(),
                    project_id: None,
                    content: "A conversation message is not runtime authorization.".into(),
                }),
            },
            "poll-smoke-send",
        )
        .unwrap();
    let events = daemon
        .request(
            Operation::Events { cursor, limit: 128 },
            "poll-smoke-events",
        )
        .unwrap();
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "domain-change")
    );
    let snapshot = daemon
        .request(Operation::Snapshot, "poll-smoke-snapshot")
        .unwrap();
    assert_eq!(snapshot["workerCount"], 0);
    assert!(snapshot["activeWorkerId"].is_null());
    let current = daemon
        .request(
            Operation::Desktop {
                request: Box::new(DesktopRequest::Bootstrap),
            },
            "poll-smoke-event-bootstrap",
        )
        .unwrap();
    assert_eq!(current["conversations"].as_array().unwrap().len(), 1);
    let page = daemon
        .request(
            Operation::Desktop {
                request: Box::new(DesktopRequest::Conversation {
                    conversation_id: "poll-smoke-conversation".into(),
                    after_ordinal: 0,
                    limit: 16,
                }),
            },
            "poll-smoke-conversation",
        )
        .unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert!(page["work"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
    daemon.stop();
}

#[test]
fn protected_change_after_approval_blocks_application_without_restoration() {
    execute(false, false, Some("protected-after-approval"))
}

#[test]
fn missing_candidate_blob_blocks_approval_without_repair_or_git_writes() {
    execute(false, false, Some("missing-candidate-blob"));
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
fn integration_check_restoring_bytes_and_timestamp_still_invalidates_without_replay() {
    execute(false, false, Some("integration-transient-check"))
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
