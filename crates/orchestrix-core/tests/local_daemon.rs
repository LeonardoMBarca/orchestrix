//! Offline daemon acceptance: actual CLI, named pipes, Git, SQLite and owned runtime processes.
#![cfg(windows)]
use orchestrix_core::{
    daemon::{API_VERSION, Client, Operation, Request, RuntimeChoice},
    evaluator::CheckSpec,
    persistence::{DaemonCursor, Store},
    policy::{ContextBudget, Fallback, PolicyDocument, TaskClassification, TaskKind},
    worker::{ContextFileRequest, ManualTaskRequest},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
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
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-daemon-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("repository");
        let authorized = root.join("attempts");
        let home = root.join("fixture-home");
        for p in [&repository, &authorized, &home] {
            fs::create_dir(p).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), "before\n").unwrap();
        git(&repository, &["add", "--all"]);
        git(&repository, &["commit", "--quiet", "-m", "daemon fixture"]);
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]))
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
    fn start(&self) -> Daemon {
        let child = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
            .args(["daemon", "--database"])
            .arg(&self.database)
            .env("ORCHESTRIX_LOCAL_CONTROL_DIAGNOSTICS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(self.root.join("daemon-stderr.log")).unwrap())
            .spawn()
            .unwrap();
        let mut daemon = Daemon {
            child,
            client: Client::new(&self.database).unwrap(),
        };
        // Startup revalidates retained Git artifacts before publishing readiness.
        // Use the bounded host-operation envelope, not a guessed process boot delay.
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            if daemon
                .client
                .request(&Request {
                    api_version: API_VERSION,
                    operation: Operation::Snapshot,
                })
                .is_ok()
            {
                break;
            }
            assert!(
                daemon.child.try_wait().unwrap().is_none(),
                "daemon exited before ready"
            );
            assert!(Instant::now() < deadline, "daemon start deadline");
        }
        daemon
    }
    fn task(&self, id: &str) -> ManualTaskRequest {
        let mut policy = PolicyDocument::default();
        policy.mode_defaults.fallback = Some(Fallback::Compatible);
        ManualTaskRequest {
            editable_paths: None,
            schema_version: 1,
            id: id.into(),
            project_id: "daemon-project".into(),
            repository_id: "daemon-repository".into(),
            repository_directory: self.repository.clone(),
            authorized_worktree_directory: self.authorized.clone(),
            base_commit: self.base.clone(),
            connection_id: "fixture-account".into(),
            connection_revision: 1,
            objective: "fixture-edit: change only fixture.txt".into(),
            acceptance_criteria: vec!["fixture.txt is after followed by newline".into()],
            policy,
            classification: TaskClassification::routine(TaskKind::Implementation, "manual-user"),
            context_files: vec![ContextFileRequest {
                path: "fixture.txt".into(),
                authorization_ref: "read-fixture".into(),
                required: true,
            }],
            instructions: vec!["Use only owned worktree; no delegation.".into()],
            checks: vec![CheckSpec {
                id: "fixture-check".into(),
                executable: binary(),
                arguments: vec!["--check-fixture".into(), "after\n".into()],
                authorization_ref: "authorized-fixture-check".into(),
                authorized: true,
                timeout_ms: 2000,
                executable_sha256: None,
            }],
        }
    }
    fn submit(&self, id: &str, scenario: &str) -> Operation {
        Operation::Submit {
            command_id: format!("submit-{id}"),
            authorization_ref: "explicit-offline-test".into(),
            task: Box::new(self.task(id)),
            runtime: RuntimeChoice::Synthetic {
                executable: binary(),
                home: self.home.clone(),
                scenario: scenario.into(),
            },
        }
    }
    fn dispatches(&self) -> u32 {
        fs::read_to_string(self.home.join("dispatch-count"))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }
    fn native_threads(&self) -> usize {
        fs::read_dir(&self.home)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("fixture-thread-")
            })
            .count()
    }
    fn dispatch_events(&self) -> std::collections::BTreeMap<String, Vec<u8>> {
        fs::read_dir(&self.home)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("dispatch-event-")
            })
            .map(|entry| {
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // A failed process test retains its own fixture for diagnosis; cleanup
        // must not panic during unwinding and conceal the original assertion.
        if std::thread::panicking() {
            return;
        }
        let root = self.root.canonicalize().unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-daemon-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
struct Daemon {
    child: Child,
    client: Client,
}

#[test]
fn exact_serial_application_is_journaled_shared_and_durable_without_redispatch() {
    use orchestrix_core::{
        domain::{DeliveryCandidate, RunId, RunState, TaskId},
        git_records::content_hash,
        integration_records::{ApplicationRequest, ApprovalRequest, IntegrationRequest},
        review::ReviewPipeline,
        review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
        runtime::{CancellationToken, RuntimeLaunch},
        worker::ManualWorker,
        worker_records::WorkerStatus,
    };
    let f = Fixture::new();
    let task = f.task("implementation");
    let launch = |scenario: &str| {
        RuntimeLaunch::fixture(
            binary(),
            vec!["--scenario".into(), scenario.into()],
            f.home.clone(),
            f.repository.clone(),
        )
    };
    let mut store = Store::open(&f.database).unwrap();
    let cancel = CancellationToken::new();
    let source =
        ManualWorker::run(&mut store, &task, launch("review-implementation"), &cancel).unwrap();
    assert_eq!(source.status(), WorkerStatus::Verified);
    let state = store.load().unwrap();
    let original = state.runs()[&RunId::new(&source.intent.run_id).unwrap()]
        .task(&TaskId::new(&source.intent.task_id).unwrap())
        .unwrap()
        .artifact()
        .unwrap()
        .clone();
    let original_capture = state.captured_artifacts()[&(
        original.id().as_str().to_owned(),
        original.revision().as_str().to_owned(),
    )]
        .clone();
    drop(state);
    let mut review_request = ReviewRequest {
        schema_version: 1,
        id: "review".into(),
        worker_id: source.intent.id.clone(),
        artifact_revision: original.revision().as_str().to_owned(),
        authorization_ref: "explicit-synthetic-review".into(),
        policy: task.policy.clone(),
        classification: task.classification.clone(),
        budget: ReviewBudget {
            max_corrections: 1,
            max_reviews: 2,
            max_elapsed_ms: 600_000,
        },
    };
    let rejected = ReviewPipeline::review(
        &mut store,
        &review_request,
        launch("review-reject"),
        &cancel,
    )
    .unwrap();
    assert_eq!(rejected.status(), ReviewStatus::Rejected);
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
    let corrected = ReviewPipeline::correct(
        &mut store,
        &correction,
        launch("review-correction"),
        &cancel,
    )
    .unwrap();
    assert_eq!(corrected.status(), WorkerStatus::Verified);
    assert_eq!(corrected.intent.run_id, source.intent.run_id);
    assert_eq!(corrected.intent.task_id, source.intent.task_id);
    assert_ne!(corrected.intent.attempt_id, source.intent.attempt_id);
    let state = store.load().unwrap();
    assert_eq!(state.runs().len(), 1);
    let current = state.runs()[&RunId::new(&source.intent.run_id).unwrap()]
        .task(&TaskId::new(&source.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(current.attempts().len(), 2);
    assert_eq!(
        current.attempts()[1].inputs().correction_of.as_ref(),
        Some(&original)
    );
    let corrected_artifact = current.artifact().unwrap().clone();
    assert_ne!(corrected_artifact, original);
    assert_eq!(corrected_artifact.base(), original.revision());
    assert_ne!(corrected_artifact.base().as_str(), f.base);
    let corrected_capture = &state.captured_artifacts()[&(
        corrected_artifact.id().as_str().to_owned(),
        corrected_artifact.revision().as_str().to_owned(),
    )];
    assert_eq!(corrected_capture.attempt_id, corrected.intent.attempt_id);
    assert_eq!(corrected_capture.base_oid, f.base);
    assert_eq!(
        &state.captured_artifacts()[&(
            original.id().as_str().to_owned(),
            original.revision().as_str().to_owned(),
        )],
        &original_capture
    );
    assert!(
        !original_capture
            .files
            .iter()
            .any(|file| file.path == "correction.txt")
    );
    for (path, expected) in [
        ("fixture.txt", "after\n"),
        ("implementation.txt", "preserve prior implementation\n"),
        ("correction.txt", "finding-one addressed\n"),
    ] {
        assert_eq!(
            corrected_capture
                .files
                .iter()
                .find(|file| file.path == path)
                .unwrap()
                .worktree
                .as_ref()
                .unwrap()
                .content,
            expected
        );
    }
    drop(state);
    let artifact = corrected_artifact.revision().as_str().to_owned();
    review_request.id = "corrected-review".into();
    review_request.worker_id = corrected.intent.id.clone();
    review_request.artifact_revision = artifact.clone();
    let reviewed = ReviewPipeline::review(
        &mut store,
        &review_request,
        launch("review-accept"),
        &cancel,
    )
    .unwrap();
    assert_eq!(reviewed.status(), ReviewStatus::Accepted);
    // Separate implementer, rejecting reviewer, correction and accepting reviewer.
    let dispatch_events = f.dispatch_events();
    assert_eq!(dispatch_events.len(), 4);
    drop(store);
    let integration_root = f.root.join("integrations");
    fs::create_dir(&integration_root).unwrap();
    let daemon = f.start();
    let second = Client::new(&f.database).unwrap();
    daemon.resume("resume-integration");
    let revision = || Store::open(&f.database).unwrap().load().unwrap().revision();
    let mut prepare = Operation::PrepareIntegration {
        command_id: "prepare-result".into(),
        expected_revision: revision(),
        request: Box::new(IntegrationRequest {
            schema_version: 1,
            id: "result".into(),
            worker_id: corrected.intent.id.clone(),
            artifact_revision: artifact.clone(),
            authorization_ref: "explicit-integration-and-checks".into(),
            authorized_integration_directory: integration_root,
            target_directory: f.repository.clone(),
            checks: task.checks.clone(),
        }),
    };
    let wait = |operation: &Operation| {
        let command = match operation {
            Operation::PrepareIntegration { command_id, .. }
            | Operation::ApproveIntegration { command_id, .. }
            | Operation::ApplyIntegration { command_id, .. }
            | Operation::RevalidateIntegration { command_id, .. } => command_id,
            _ => unreachable!(),
        };
        let fingerprint = content_hash(&serde_json::to_vec(operation).unwrap());
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let snapshot = daemon.ok(Operation::Snapshot);
            let receipt = daemon.ok(Operation::CommandReceipt {
                command_id: command.clone(),
                fingerprint: fingerprint.clone(),
            });
            if !receipt["receipt"]["outcome"].is_null() {
                return receipt["receipt"]["outcome"].clone();
            }
            assert!(Instant::now() < deadline, "integration deadline {snapshot}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    // A current Task artifact cannot be attributed to its previous worker.
    let refs_before = git(
        &f.repository,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    let mut wrong_worker = prepare.clone();
    if let Operation::PrepareIntegration {
        command_id,
        expected_revision,
        request,
    } = &mut wrong_worker
    {
        *command_id = "wrong-source-worker".into();
        *expected_revision = revision();
        request.id = "wrong-source-binding".into();
        request.worker_id = source.intent.id.clone();
    }
    daemon.ok(wrong_worker.clone());
    let rejected_binding = wait(&wrong_worker);
    assert_eq!(rejected_binding["state"], "rejected-before-reservation");
    assert_eq!(rejected_binding["reservationPersisted"], false);
    assert!(
        !Store::open(&f.database)
            .unwrap()
            .load()
            .unwrap()
            .integrations()
            .contains_key("wrong-source-binding")
    );
    assert_eq!(
        fs::read_dir(f.root.join("integrations")).unwrap().count(),
        0
    );
    assert_eq!(
        git(
            &f.repository,
            &["for-each-ref", "--format=%(refname) %(objectname)"]
        ),
        refs_before
    );
    assert_eq!(
        String::from_utf8(git(&f.repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim(),
        f.base
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    assert!(!f.repository.join("implementation.txt").exists());
    assert!(!f.repository.join("correction.txt").exists());
    assert_eq!(f.dispatch_events(), dispatch_events);
    if let Operation::PrepareIntegration {
        expected_revision, ..
    } = &mut prepare
    {
        *expected_revision = revision();
    }
    let first = daemon.ok(prepare.clone());
    assert_eq!(first["replayed"], false);
    let replay = second
        .request(&Request {
            api_version: API_VERSION,
            operation: prepare.clone(),
        })
        .unwrap();
    assert!(replay.ok);
    assert_eq!(replay.result["replayed"], true);
    // Historical inspection remains available while preparation/checks own the worker lease.
    let snapshot = daemon.ok(Operation::Snapshot);
    assert!(
        snapshot["activeOperationKind"].is_null()
            || snapshot["activeOperationKind"] == "integration-prepare"
    );
    assert_eq!(wait(&prepare)["status"], "prepared");
    let inspect = || {
        daemon.ok(Operation::InspectIntegration {
            integration_id: "result".into(),
        })
    };
    let view = inspect();
    assert_eq!(view["historical"], true);
    assert_eq!(view["currentVerification"], "not-revalidated");
    assert_eq!(view["summary"]["delivered"], false);
    assert_eq!(view["summary"]["worker_id"], corrected.intent.id);
    assert_eq!(view["summary"]["source_artifact_revision"], artifact);
    let bound_candidate =
        DeliveryCandidate::restore_checkpoint(&view["summary"]["candidate"]).unwrap();
    assert_eq!(
        bound_candidate.artifacts(),
        std::slice::from_ref(&corrected_artifact)
    );
    let state = Store::open(&f.database).unwrap().load().unwrap();
    let integrated = &state.integrations()["result"];
    assert_eq!(integrated.intent.worker_id, corrected.intent.id);
    assert_eq!(
        integrated.intent.worktree.attempt_id,
        corrected.intent.attempt_id
    );
    assert_eq!(integrated.intent.capture_revision, artifact);
    assert_eq!(
        integrated.snapshot().unwrap().attempt_id,
        corrected.intent.attempt_id
    );
    assert_eq!(
        &state.captured_artifacts()[&(
            original.id().as_str().to_owned(),
            original.revision().as_str().to_owned()
        )],
        &original_capture
    );
    drop(state);
    assert_eq!(f.dispatch_events(), dispatch_events);
    let candidate = view["summary"]["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    // A physical recheck owns the same bounded slot, while historical reads stay cheap.
    // Replaying it retains the command identity instead of reserving a new operation.
    let revalidate = Operation::RevalidateIntegration {
        command_id: "revalidate-result".into(),
        integration_id: "result".into(),
    };
    daemon.ok(revalidate.clone());
    assert_eq!(wait(&revalidate)["status"], "prepared");
    assert_eq!(daemon.ok(revalidate)["replayed"], true);
    assert_eq!(inspect()["summary"]["candidate_revision"], candidate);
    let stale = Operation::ApproveIntegration {
        command_id: "stale-approval".into(),
        expected_revision: 0,
        request: Box::new(ApprovalRequest {
            schema_version: 1,
            id: "stale".into(),
            integration_id: "result".into(),
            candidate_revision: candidate.clone(),
            authorization_ref: "explicit-human-approval".into(),
            valid_for_ms: 600_000,
        }),
    };
    daemon.ok(stale.clone());
    let rejected = wait(&stale);
    assert_eq!(rejected["reason"], "stale-revision-resync");
    assert_eq!(rejected["reservationPersisted"], false);
    assert_eq!(inspect()["summary"]["status"], "prepared");
    let approve = Operation::ApproveIntegration {
        command_id: "approve-result".into(),
        expected_revision: revision(),
        request: Box::new(ApprovalRequest {
            schema_version: 1,
            id: "approval".into(),
            integration_id: "result".into(),
            candidate_revision: candidate.clone(),
            authorization_ref: "explicit-human-approval".into(),
            valid_for_ms: 600_000,
        }),
    };
    daemon.ok(approve.clone());
    assert_eq!(wait(&approve)["status"], "approved");
    let apply = Operation::ApplyIntegration {
        command_id: "apply-result".into(),
        expected_revision: revision(),
        request: Box::new(ApplicationRequest {
            schema_version: 1,
            id: "application".into(),
            integration_id: "result".into(),
            candidate_revision: candidate,
            authorization_ref: "explicit-destination-application".into(),
        }),
    };
    daemon.ok(apply.clone());
    let delivered = wait(&apply);
    assert_eq!(delivered["status"], "delivered");
    assert_eq!(delivered["delivered"], true);
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("implementation.txt")).unwrap(),
        "preserve prior implementation\n"
    );
    assert_eq!(
        fs::read_to_string(f.repository.join("correction.txt")).unwrap(),
        "finding-one addressed\n"
    );
    assert_eq!(
        fs::read_to_string(Path::new(&original_capture.origin).join("implementation.txt")).unwrap(),
        "preserve prior implementation\n"
    );
    assert!(
        !Path::new(&original_capture.origin)
            .join("correction.txt")
            .exists()
    );
    assert_eq!(f.dispatch_events(), dispatch_events);
    let head = git(&f.repository, &["rev-parse", "HEAD"]);
    daemon.stop();
    let reopened = f.start();
    let replay = reopened.ok(apply);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["outcome"], delivered);
    assert_eq!(git(&f.repository, &["rev-parse", "HEAD"]), head);
    let state = Store::open(&f.database).unwrap().load().unwrap();
    assert_eq!(
        state.runs()[&RunId::new(&source.intent.run_id).unwrap()].state(),
        RunState::Delivered
    );
    assert_eq!(f.dispatch_events(), dispatch_events);
    assert_eq!(reopened.ok(Operation::Snapshot)["state"]["suspended"], true);
    // Startup deliberately suspends new admissions. Replay above needs no
    // resume; this new collision probe does, and must not bypass that gate.
    reopened.resume("resume-collision-probe");
    let mut collision = prepare.clone();
    if let Operation::PrepareIntegration {
        command_id,
        expected_revision,
        request,
    } = &mut collision
    {
        *command_id = "colliding-worker-id".into();
        *expected_revision = revision();
        request.id = source.intent.id.clone();
    }
    reopened.ok(collision.clone());
    let fingerprint = content_hash(&serde_json::to_vec(&collision).unwrap());
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let receipt = reopened.ok(Operation::CommandReceipt {
            command_id: "colliding-worker-id".into(),
            fingerprint: fingerprint.clone(),
        });
        if !receipt["receipt"]["outcome"].is_null() {
            assert_eq!(
                receipt["receipt"]["outcome"]["state"],
                "rejected-before-reservation"
            );
            assert_eq!(receipt["receipt"]["outcome"]["reservationPersisted"], false);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(git(&f.repository, &["rev-parse", "HEAD"]), head);
    assert_eq!(f.dispatch_events(), dispatch_events);
    reopened.stop();
}

#[test]
fn desktop_two_clients_replay_resync_and_reopen_use_the_same_durable_conversation() {
    use orchestrix_core::desktop::DesktopRequest;
    let fixture = Fixture::new();
    let daemon = fixture.start();
    let op = |request| Operation::Desktop {
        request: Box::new(request),
    };
    let bootstrap = daemon.ok(op(DesktopRequest::Bootstrap));
    assert_eq!(bootstrap["conversations"], serde_json::json!([]));
    let original = DesktopRequest::CreateProject {
        command_id: "desktop-project".into(),
        expected_revision: 0,
        id: "desktop-project".into(),
        name: "Parser".into(),
        directory: fixture.repository.to_string_lossy().to_string(),
    };
    daemon.ok(op(original.clone()));
    let replay = daemon.ok(op(original));
    assert_eq!(replay["replayed"], true);
    let snapshot = daemon.ok(op(DesktopRequest::Bootstrap));
    let revision = snapshot["revision"].as_u64().unwrap();
    let second = Client::new(&fixture.database).unwrap();
    let message = DesktopRequest::SendMessage {
        command_id: "desktop-message".into(),
        expected_revision: revision,
        conversation_id: "chat-one".into(),
        project_id: Some("desktop-project".into()),
        content: "Investigate the parser".into(),
    };
    daemon.ok(op(message.clone()));
    let replay = second
        .request(&Request {
            api_version: API_VERSION,
            operation: op(message),
        })
        .unwrap();
    assert!(replay.ok);
    assert_eq!(replay.result["replayed"], true);
    let stale = second
        .request(&Request {
            api_version: API_VERSION,
            operation: op(DesktopRequest::SendMessage {
                command_id: "stale-message".into(),
                expected_revision: revision,
                conversation_id: "chat-one".into(),
                project_id: None,
                content: "Stale client".into(),
            }),
        })
        .unwrap();
    assert_eq!(stale.error.as_deref(), Some("stale-revision-resync"));
    let cursor: DaemonCursor = serde_json::from_value(bootstrap["eventCursor"].clone()).unwrap();
    let events = daemon.ok(Operation::Events { cursor, limit: 128 });
    assert_eq!(events["resyncRequired"], false);
    assert!(events["events"].as_array().unwrap().len() >= 2);
    let page = daemon.ok(op(DesktopRequest::Conversation {
        conversation_id: "chat-one".into(),
        after_ordinal: 0,
        limit: 16,
    }));
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    daemon.stop();
    let reopened = fixture.start();
    let state = reopened.ok(op(DesktopRequest::Bootstrap));
    assert_eq!(state["projects"][0]["name"], "Parser");
    assert_eq!(state["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(state["admissionsSuspended"], true);
    reopened.stop();
}
impl Daemon {
    fn send(&self, operation: Operation) -> orchestrix_core::daemon::Response {
        self.client
            .request(&Request {
                api_version: API_VERSION,
                operation,
            })
            .unwrap_or_else(|error| {
                panic!(
                    "API request failed: {error}; native={:?}",
                    orchestrix_process::ipc::last_diagnostic()
                )
            })
    }
    fn ok(&self, operation: Operation) -> Value {
        let r = self.send(operation);
        assert!(r.ok, "{:?}", r.error);
        r.result
    }
    fn resume(&self, id: &str) {
        self.ok(Operation::Resume {
            command_id: id.into(),
        });
    }
    fn wait_worker(&self, id: &str) -> Value {
        // The worker contains bounded Git preparation and supervised Jobs; this
        // outer fixture guard is not a guessed model-start delay or a new runtime timeout.
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let s = self.ok(Operation::Snapshot);
            if let Some(w) = s["workers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["workerId"] == id)
                && s["activeWorkerId"].is_null()
                && [
                    "verified",
                    "failed",
                    "cancelled",
                    "uncertain",
                    "blocked",
                    "no-checks",
                    "needs-work",
                    "inspection-required",
                    "set-aside",
                ]
                .contains(&w["status"].as_str().unwrap())
            {
                return self.ok(Operation::Status {
                    worker_id: id.into(),
                })["worker"]
                    .clone();
            }
            assert!(Instant::now() < deadline, "worker deadline {s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn stop(self) {
        let status = self.shutdown_status();
        assert!(
            status.success(),
            "expected graceful daemon exit0, got {status}"
        );
    }
    fn shutdown_status(mut self) -> std::process::ExitStatus {
        self.ok(Operation::Shutdown {
            command_id: format!("shutdown-final-{}", self.child.id()),
        });
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "shutdown deadline");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn repository_reads_are_shared_nonblocking_bounded_and_never_dispatch_a_runtime() {
    use orchestrix_core::{
        mechanical::{Query, QueryKind},
        repository_inspection::InspectionRequest,
    };
    let f = Fixture::new();
    let daemon = f.start();
    let other = Client::new(&f.database).unwrap();
    let query = Query {
        authorized: true,
        authorization_ref: "local-read-consent".into(),
        query: QueryKind::RepositoryPreflight {
            editable_paths: None,
            repository: f.repository.clone(),
            base_commit: Some(f.base.clone()),
        },
    };
    let start = Operation::RepositoryInspection {
        request: InspectionRequest::Start {
            id: "inspect-one".into(),
            query: query.clone(),
        },
    };
    let mut denied = query.clone();
    denied.authorized = false;
    assert!(
        !daemon
            .send(Operation::RepositoryInspection {
                request: InspectionRequest::Start {
                    id: "not-authorized".into(),
                    query: denied
                }
            })
            .ok
    );
    assert_eq!(daemon.ok(start.clone())["state"], "pending");
    let snapshot = daemon.ok(Operation::Snapshot);
    assert!(snapshot["activeWorkerId"].is_null());
    assert!(
        snapshot["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "repository-inspection-v1")
    );
    let mut conflicting = query.clone();
    conflicting.authorization_ref = "another-consent".into();
    assert_eq!(
        daemon
            .send(Operation::RepositoryInspection {
                request: InspectionRequest::Start {
                    id: "inspect-one".into(),
                    query: conflicting
                }
            })
            .error
            .as_deref(),
        Some("repository-read-id-conflict")
    );
    let deadline = Instant::now() + Duration::from_secs(110);
    let result = loop {
        let response = other
            .request(&Request {
                api_version: API_VERSION,
                operation: Operation::RepositoryInspection {
                    request: InspectionRequest::Inspect {
                        id: "inspect-one".into(),
                    },
                },
            })
            .unwrap();
        assert!(response.ok, "{:?}", response.error);
        if response.result["state"] != "pending" {
            break response.result;
        }
        // Inspection does not monopolize control, suspend worker admissions,
        // reserve an attempt, or trigger metadata/model execution.
        assert!(daemon.ok(Operation::Snapshot)["activeWorkerId"].is_null());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(result["state"], "finished", "{result}");
    assert_eq!(result["result"]["baseOid"], f.base);
    assert_eq!(result["result"]["compatible"], true);
    assert_eq!(result["result"]["applicationEligible"], true);
    assert_eq!(result["modelCalled"], false);
    assert_eq!(
        daemon.ok(start),
        result,
        "same read ID does not execute another inspection"
    );
    let status = daemon.ok(Operation::RepositoryInspection {
        request: InspectionRequest::Start {
            id: "git-status".into(),
            query: Query {
                query: QueryKind::GitStatus {
                    repository: f.repository.clone(),
                },
                ..query
            },
        },
    });
    assert_eq!(status["state"], "pending");
    let deadline = Instant::now() + Duration::from_secs(110);
    loop {
        let result = daemon.ok(Operation::RepositoryInspection {
            request: InspectionRequest::Inspect {
                id: "git-status".into(),
            },
        });
        if result["state"] != "pending" {
            assert_eq!(result["state"], "finished", "{result}");
            assert_eq!(result["result"]["output"], "");
            assert_eq!(result["result"]["modelCalled"], false);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(f.dispatches(), 0);
    assert_eq!(f.native_threads(), 0);
    assert!(
        daemon.ok(Operation::Snapshot)["workers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    daemon.stop();
    let reopened = f.start();
    assert_eq!(
        reopened
            .send(Operation::RepositoryInspection {
                request: InspectionRequest::Inspect {
                    id: "inspect-one".into()
                }
            })
            .error
            .as_deref(),
        Some("repository-read-not-found-reinspect-explicitly")
    );
    assert_eq!(f.dispatches(), 0);
    reopened.stop();
}
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn git(cwd: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Orchestrix fixture",
            "-c",
            "user.email=fixture@orchestrix.invalid",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn two_clients_reconnect_replay_events_suspend_and_exact_result() {
    let f = Fixture::new();
    let daemon = f.start();
    let client = Client::new(&f.database).unwrap();
    let initial = daemon.ok(Operation::Snapshot);
    assert_eq!(initial["state"]["suspended"], true);
    assert_eq!(initial["capacity"]["subscriptionQuota"], "unknown");
    let cursor: DaemonCursor = serde_json::from_value(initial["state"]["cursor"].clone()).unwrap();
    assert_eq!(
        daemon
            .send(f.submit("before-resume", "success"))
            .error
            .as_deref(),
        Some("admissions-suspended")
    );
    let second = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
        .args(["daemon", "--database"])
        .arg(&f.database)
        .output()
        .unwrap();
    assert!(!second.status.success());
    daemon.resume("resume-first");
    let command = f.submit("happy-worker", "success");
    let first = daemon.ok(command.clone());
    assert_eq!(first["replayed"], false);
    let duplicate = client
        .request(&Request {
            api_version: 1,
            operation: command.clone(),
        })
        .unwrap();
    assert!(duplicate.ok);
    assert_eq!(duplicate.result["replayed"], true);
    let mut conflict = command.clone();
    if let Operation::Submit { task, .. } = &mut conflict {
        task.objective.push_str(" changed");
    }
    assert_eq!(
        daemon.send(conflict).error.as_deref(),
        Some("command-id-conflict")
    );
    daemon.ok(Operation::Suspend {
        command_id: "suspend-active".into(),
    });
    assert_eq!(
        daemon
            .send(f.submit("blocked-during-suspend", "success"))
            .error
            .as_deref(),
        Some("admissions-suspended")
    );
    let result = daemon.wait_worker("happy-worker");
    assert_eq!(result["status"], "verified");
    assert_eq!(f.dispatches(), 1);
    assert_eq!(
        fs::read_to_string(f.repository.join("fixture.txt")).unwrap(),
        "before\n"
    );
    let pack = daemon.ok(Operation::Continuity {
        worker_id: "happy-worker".into(),
        budget: ContextBudget::default(),
    });
    assert_eq!(pack["inferenceUsed"], false);
    assert!(
        pack["pack"]["objective"]
            .as_str()
            .unwrap()
            .contains("fixture-edit")
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
        .args(["snapshot", "--database"])
        .arg(&f.database)
        .output()
        .unwrap();
    assert!(cli.status.success());
    let value: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(value["result"]["workers"][0]["status"], "verified");
    let current: DaemonCursor =
        serde_json::from_value(value["result"]["state"]["cursor"].clone()).unwrap();
    let page = daemon.ok(Operation::Workers {
        cursor: current,
        after_worker: None,
        limit: 1,
    });
    assert_eq!(page["workers"][0]["workerId"], "happy-worker");
    assert!(page["nextWorker"].is_null());
    assert_eq!(page["resyncRequired"], false);
    let stale = daemon.ok(Operation::Workers {
        cursor: cursor.clone(),
        after_worker: None,
        limit: 1,
    });
    assert_eq!(stale["resyncRequired"], true);
    let status = daemon.ok(Operation::Status {
        worker_id: "happy-worker".into(),
    });
    assert_eq!(status["historical"], true);
    assert_eq!(status["currentVerification"], "not-revalidated");
    let mut next = cursor;
    let mut seen = 0;
    loop {
        let events = daemon.ok(Operation::Events {
            cursor: next.clone(),
            limit: 3,
        });
        for event in events["events"].as_array().unwrap() {
            next.sequence += 1;
            assert_eq!(event["sequence"], next.sequence);
            seen += 1;
        }
        assert_eq!(events["cursor"]["sequence"], next.sequence);
        if events["more"] == false {
            break;
        }
    }
    assert!(seen > 5);
    let invalid = daemon.ok(Operation::Events {
        cursor: DaemonCursor {
            database: "foreign".into(),
            sequence: 0,
        },
        limit: 1,
    });
    assert_eq!(invalid["resyncRequired"], true);
    let invalid = daemon.ok(Operation::Events {
        cursor: DaemonCursor {
            database: next.database.clone(),
            sequence: u64::MAX,
        },
        limit: 1,
    });
    assert_eq!(invalid["resyncRequired"], true);
    let final_replay = daemon.ok(command);
    assert_eq!(final_replay["replayed"], true);
    assert_eq!(final_replay["outcome"]["state"], "finished");
    assert_eq!(f.dispatches(), 1);
    daemon.stop();
    let restarted = f.start();
    let snapshot = restarted.ok(Operation::Snapshot);
    assert_eq!(snapshot["state"]["suspended"], true);
    assert_eq!(snapshot["workers"][0]["status"], "verified");
    assert_eq!(f.dispatches(), 1);
    restarted.stop();
    let saved = Store::open(&f.database).unwrap().load().unwrap();
    assert_eq!(saved.workers().len(), 1);
    let record = &saved.workers()["happy-worker"];
    let run = &saved.runs()[&orchestrix_core::domain::RunId::new(&record.intent.run_id).unwrap()];
    assert_eq!(
        run.task(&orchestrix_core::domain::TaskId::new(&record.intent.task_id).unwrap())
            .unwrap()
            .state(),
        orchestrix_core::domain::TaskState::NeedsReview
    );
}

#[test]
fn diagnostics_default_never_blocks_on_an_undrained_stderr_pipe() {
    use std::io::Read;
    let root = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "orchestrix-daemon-no-log-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&root).unwrap();
    let database = root.join("state.sqlite");
    let child = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
        .args(["daemon", "--database"])
        .arg(&database)
        .env_remove("ORCHESTRIX_LOCAL_CONTROL_DIAGNOSTICS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut daemon = Daemon {
        child,
        client: Client::new(&database).unwrap(),
    };
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if daemon
            .client
            .request(&Request {
                api_version: API_VERSION,
                operation: Operation::Snapshot,
            })
            .is_ok()
        {
            break;
        }
        assert!(daemon.child.try_wait().unwrap().is_none());
        assert!(Instant::now() < deadline);
    }
    let scope = orchestrix_core::daemon::endpoint_scope(&database).unwrap();
    // Deliberate disconnects fill the in-memory ring and error-emission budget.
    // Nobody drains stderr until the dedicated host exits; no runtime or Git.
    for _ in 0..24 {
        let stream =
            orchestrix_process::ipc::Stream::connect(&scope, Duration::from_secs(2)).unwrap();
        drop(stream);
        daemon.ok(Operation::Snapshot);
    }
    let mut stderr = daemon.child.stderr.take().unwrap();
    daemon.stop();
    let mut bytes = vec![];
    stderr.read_to_end(&mut bytes).unwrap();
    assert!(
        bytes.is_empty(),
        "default diagnostics unexpectedly wrote stderr"
    );
    let canonical = root.canonicalize().unwrap();
    assert_eq!(
        canonical.parent(),
        Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
    );
    assert!(
        canonical
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("orchestrix-daemon-no-log-")
    );
    fs::remove_dir_all(canonical).unwrap();
}
#[test]
fn cancel_is_owned_and_quota_failure_keeps_local_controls_available() {
    let f = Fixture::new();
    let daemon = f.start();
    daemon.resume("resume-hang");
    daemon.ok(f.submit("hanging-worker", "hang"));
    let deadline = Instant::now() + Duration::from_secs(300);
    while f.dispatches() == 0 {
        assert!(Instant::now() < deadline);
        let state = daemon.ok(Operation::Snapshot);
        assert!(
            !state["activeWorkerId"].is_null(),
            "worker finished without dispatch: {state}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        daemon
            .send(Operation::Cancel {
                command_id: "wrong-worker".into(),
                worker_id: "some-other-worker".into()
            })
            .error
            .as_deref(),
        Some("worker-not-active-owned-by-daemon")
    );
    daemon.ok(Operation::Suspend {
        command_id: "suspend-hang".into(),
    });
    daemon.ok(Operation::Cancel {
        command_id: "cancel-own".into(),
        worker_id: "hanging-worker".into(),
    });
    let status = daemon.wait_worker("hanging-worker");
    assert_eq!(status["status"], "cancelled");
    let recovery = daemon.ok(Operation::Recovery);
    let stopped = recovery["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["worker_id"] == "hanging-worker")
        .unwrap();
    assert_eq!(stopped["local_tree_stopped"], true);
    assert_eq!(stopped["streams_closed"], true);
    daemon.resume("resume-after-cancel");
    daemon.ok(f.submit("quota-worker", "rate-limit"));
    let quota = daemon.wait_worker("quota-worker");
    assert_eq!(quota["status"], "failed");
    let snapshot = daemon.ok(Operation::Snapshot);
    assert_eq!(snapshot["capacity"]["subscriptionQuota"], "unknown");
    daemon.ok(Operation::Suspend {
        command_id: "suspend-quota".into(),
    });
    daemon.ok(Operation::Recovery);
    assert_eq!(f.native_threads(), 2);
    daemon.resume("resume-preflight-quota");
    let denied = f.submit("unavailable-before-reservation", "quota-failure");
    daemon.ok(denied.clone());
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        let receipt = daemon.ok(denied.clone());
        if !receipt["outcome"].is_null() {
            assert_eq!(receipt["outcome"]["state"], "rejected-before-reservation");
            assert_eq!(receipt["outcome"]["reason"], "unavailable");
            assert_eq!(receipt["outcome"]["reservationPersisted"], false);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(f.native_threads(), 2);
    daemon.ok(Operation::Suspend {
        command_id: "still-local-after-quota".into(),
    });
    daemon.stop();
}

#[test]
fn lost_client_reply_and_daemon_crash_never_redispatch_accepted_command() {
    let f = Fixture::new();
    let mut daemon = f.start();
    daemon.resume("resume-crash");
    let operation = f.submit("crashed-worker", "hang");
    // Disconnect after write, before reading reply. The durable command remains authoritative.
    let scope = orchestrix_core::daemon::endpoint_scope(&f.database).unwrap();
    let mut stream =
        orchestrix_process::ipc::Stream::connect(&scope, Duration::from_secs(2)).unwrap();
    stream
        .write(
            &serde_json::to_vec(&Request {
                api_version: 1,
                operation: operation.clone(),
            })
            .unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
    drop(stream);
    let deadline = Instant::now() + Duration::from_secs(300);
    while f.dispatches() == 0 {
        assert!(Instant::now() < deadline);
        let state = daemon.ok(Operation::Snapshot);
        assert!(
            !state["activeWorkerId"].is_null(),
            "worker finished without dispatch: {state}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    daemon.child.kill().unwrap();
    daemon.child.wait().unwrap();
    drop(daemon);
    let restarted = f.start();
    let replay = restarted.ok(operation);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["outcome"]["state"], "interrupted-before-response");
    let recovery = restarted.ok(Operation::Recovery);
    assert_eq!(recovery["has_blocking_cases"], true);
    assert_eq!(recovery["cases"][0]["status"], "uncertain");
    assert_eq!(
        restarted
            .send(Operation::Resume {
                command_id: "resume-after-crash".into()
            })
            .error
            .as_deref(),
        Some("recovery-inspection-required")
    );
    assert_eq!(f.dispatches(), 1);
    restarted.ok(Operation::Continuity {
        worker_id: "crashed-worker".into(),
        budget: ContextBudget::default(),
    });
    for index in 0..32 {
        restarted.ok(Operation::RecheckRecovery {
            command_id: format!("recheck-owned-closure-{index}"),
            worker_id: "crashed-worker".into(),
        });
        let recovery = restarted.ok(Operation::Recovery);
        if recovery["cases"][0]["local_scope_exhausted"] == true {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let closure = restarted.ok(Operation::Recovery);
    assert_eq!(closure["cases"][0]["local_scope_exhausted"], true);
    assert_eq!(closure["cases"][0]["status"], "uncertain");
    assert_eq!(closure["cases"][0]["remote_outcome"], "unknown");
    restarted.ok(Operation::Intervene {
        command_id: "review-owned-crash".into(),
        worker_id: "crashed-worker".into(),
        expected_revision: closure["revision"].as_u64().unwrap(),
        authorization_ref: "fixture-reviewed-uncertainty".into(),
        action: orchestrix_core::recovery::RecoveryAction::SetAside,
    });
    restarted.resume("resume-after-explicit-inspection");
    let mut old = f.submit("crashed-worker", "hang");
    if let Operation::Submit { command_id, .. } = &mut old {
        *command_id = "new-command-must-not-repeat-old-worker".into();
    }
    assert_eq!(
        restarted.send(old).error.as_deref(),
        Some("worker-id-already-recorded-inspect-only")
    );
    restarted.ok(f.submit("new-task-after-recovery", "success"));
    assert_eq!(
        restarted.wait_worker("new-task-after-recovery")["status"],
        "verified"
    );
    assert_eq!(f.native_threads(), 2);
    restarted.stop();
}

#[test]
fn stalled_and_oversized_clients_have_bounded_lifetimes() {
    let f = Fixture::new();
    let daemon = f.start();
    let scope = orchestrix_core::daemon::endpoint_scope(&f.database).unwrap();
    let stalled = orchestrix_process::ipc::Stream::connect(&scope, Duration::from_secs(2)).unwrap();
    let before = Instant::now();
    std::thread::sleep(Duration::from_millis(2200));
    drop(stalled);
    daemon.ok(Operation::Snapshot);
    assert!(before.elapsed() < Duration::from_secs(5));
    let mut stream =
        orchestrix_process::ipc::Stream::connect(&scope, Duration::from_secs(2)).unwrap();
    assert_eq!(
        stream.write(
            &vec![0; orchestrix_process::ipc::MAX_FRAME + 1],
            Duration::from_secs(1)
        ),
        Err(orchestrix_process::ipc::Error::Budget)
    );
    drop(stream);
    daemon.stop();
}

#[test]
fn shutdown_deadline_preserves_uncertainty_without_automatic_reexecution() {
    let f = Fixture::new();
    let daemon = f.start();
    daemon.resume("resume-before-shutdown");
    let operation = f.submit("shutdown-worker", "shutdown-stall");
    daemon.ok(operation.clone());
    let deadline = Instant::now() + Duration::from_secs(300);
    while f.dispatches() == 0 {
        let snapshot = daemon.ok(Operation::Snapshot);
        assert!(
            !snapshot["activeWorkerId"].is_null(),
            "worker ended without dispatch"
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    let shutdown_id = format!("shutdown-final-{}", daemon.child.id());
    let started = Instant::now();
    let status = daemon.shutdown_status();
    assert_eq!(status.code(), Some(3), "hard shutdown must be exact exit3");
    assert!(started.elapsed() >= Duration::from_secs(19));
    assert!(started.elapsed() < Duration::from_secs(23));
    let store = Store::open(&f.database).unwrap();
    let state = store.load().unwrap();
    assert_eq!(
        state.workers()["shutdown-worker"].status(),
        orchestrix_core::worker_records::WorkerStatus::RuntimeObserved
    );
    let recovery = orchestrix_core::recovery::snapshot(&store).unwrap();
    assert!(recovery.cases[0].local_tree_stopped);
    assert!(recovery.cases[0].streams_closed);
    assert_eq!(recovery.cases[0].remote_outcome, "interrupted");
    assert_eq!(state.worktrees().len(), 1);
    assert!(state.captured_artifacts().is_empty());
    let worktree = state.worktrees().values().next().unwrap();
    assert_eq!(
        fs::read_to_string(Path::new(&worktree.intent.path).join("fixture.txt")).unwrap(),
        "before\n"
    );
    drop(store);
    let reopened = f.start();
    let replay = reopened.ok(operation);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["outcome"]["state"], "interrupted-before-response");
    let shutdown = reopened.ok(Operation::Shutdown {
        command_id: shutdown_id,
    });
    assert_eq!(shutdown["replayed"], true);
    assert_eq!(shutdown["outcome"]["state"], "interrupted-before-response");
    for index in 0..32 {
        reopened.ok(Operation::RecheckRecovery {
            command_id: format!("recheck-shutdown-closure-{index}"),
            worker_id: "shutdown-worker".into(),
        });
        if reopened.ok(Operation::Recovery)["cases"][0]["local_scope_exhausted"] == true {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let recovered = reopened.ok(Operation::Recovery);
    assert_eq!(recovered["cases"][0]["status"], "uncertain");
    assert_eq!(recovered["cases"][0]["remote_outcome"], "interrupted");
    assert_eq!(recovered["cases"][0]["local_scope_exhausted"], true);
    assert_eq!(recovered["has_blocking_cases"], true);
    reopened.ok(Operation::Continuity {
        worker_id: "shutdown-worker".into(),
        budget: ContextBudget::default(),
    });
    reopened.stop();
    assert_eq!(f.dispatches(), 1);
}
