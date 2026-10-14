//! File-backed acceptance of the guarded domain through the durable command boundary.
use orchestrix_core::domain::*;
use orchestrix_core::git_records::{
    CapturedArtifact, CapturedFile, FileContent, GitCommand, WorktreeIntent, WorktreeObservation,
    WorktreeState, content_hash,
};
use orchestrix_core::persistence::{
    AttemptSnapshot, Command, Disposition, FaultPoint, Receipt, Request, RunAction, Store,
    StoreError,
};
use orchestrix_core::persistence_records::{
    DefinitionInput, DefinitionKind, DefinitionRef, RecordsCommand, SnapshotInput,
};
use orchestrix_core::runtime_contract::{EventOrigin, normalize_descriptor, normalize_request};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn git_intent(directory: &Path) -> WorktreeIntent {
    WorktreeIntent {
        operation_id: "worktree-operation-1".into(),
        project_id: "project-1".into(),
        repository_id: "repository-1".into(),
        run_id: "git-run".into(),
        task_id: "task-1".into(),
        attempt_id: "attempt-git".into(),
        base_oid: "a".repeat(40),
        domain_base: "base-1".into(),
        worktree_ref: "worktree-1".into(),
        authorized_root: directory.to_str().unwrap().into(),
        path: directory.join("attempt-worktree").to_str().unwrap().into(),
        main_repository: directory.join("repository").to_str().unwrap().into(),
        branch: "orchestrix/run-task-attempt-git".into(),
        common_git_dir: directory.join("repository/.git").to_str().unwrap().into(),
        ownership_token: content_hash(b"synthetic-noncredential-ownership"),
    }
}
fn git_observation(kind: &str) -> WorktreeObservation {
    WorktreeObservation {
        kind: kind.into(),
        evidence_sha256: content_hash(kind.as_bytes()),
    }
}
fn captured(intent: &WorktreeIntent) -> CapturedArtifact {
    let mut artifact = CapturedArtifact {
        id: "artifact-git".into(),
        revision: String::new(),
        operation_id: intent.operation_id.clone(),
        repository_id: intent.repository_id.clone(),
        run_id: intent.run_id.clone(),
        task_id: intent.task_id.clone(),
        attempt_id: intent.attempt_id.clone(),
        base_oid: intent.base_oid.clone(),
        origin: intent.path.clone(),
        files: vec![CapturedFile {
            path: "readme.md".into(),
            base: Some(FileContent::new("old\n".into(), "100644".into()).unwrap()),
            index: Some(FileContent::new("old\n".into(), "100644".into()).unwrap()),
            worktree: Some(FileContent::new("new\n".into(), "100644".into()).unwrap()),
            untracked: false,
        }],
        exclusions: vec![],
        patch: "exact patch fixture\n".into(),
        patch_sha256: content_hash(b"exact patch fixture\n"),
    };
    artifact.revision = artifact.content_revision();
    artifact
}

#[test]
fn git_records_are_atomic_owned_idempotent_and_preserve_exact_content() {
    let temp = TestDatabase::new();
    let mut store = temp.reopen();
    seed(&mut store);
    create_run(&mut store, "git-run", TaskKind::Code);
    let reserve = reservation(
        &store,
        "reserve-git",
        "git-run",
        "attempt-git",
        TaskKind::Code,
    );
    store.execute(&reserve).unwrap();
    let intent = git_intent(&temp.directory);
    assert!(intent.validate());
    let plan = request(
        &store,
        "plan-git",
        Command::Git(GitCommand::Plan(intent.clone())),
    );
    let receipt = store.execute(&plan).unwrap();
    assert_eq!(receipt.result.disposition, Disposition::Applied);
    assert!(store.execute(&plan).unwrap().replayed);
    let before_events = store.event_count().unwrap();
    let confirm = request(
        &store,
        "confirm-git",
        Command::Git(GitCommand::ConfirmCreated {
            operation_id: intent.operation_id.clone(),
            observation: git_observation("created"),
        }),
    );
    assert_eq!(
        store.execute_with_fault(&confirm, FaultPoint::AfterEvent),
        Err(StoreError::InjectedRollback)
    );
    assert_eq!(
        store.load().unwrap().worktrees()[&intent.operation_id].state,
        WorktreeState::Planned
    );
    assert_eq!(store.event_count().unwrap(), before_events);
    store.execute(&confirm).unwrap();
    let artifact = captured(&intent);
    assert!(artifact.validate());
    let save = request(
        &store,
        "capture-git",
        Command::Git(GitCommand::RecordArtifact(artifact.clone())),
    );
    assert_eq!(
        store.execute_with_fault(&save, FaultPoint::AfterState),
        Err(StoreError::InjectedRollback)
    );
    assert!(store.load().unwrap().captured_artifacts().is_empty());
    assert_eq!(
        store.execute_with_fault(&save, FaultPoint::AfterCommit),
        Err(StoreError::ReplyLost)
    );
    drop(store);
    let mut store = temp.reopen();
    assert!(store.execute(&save).unwrap().replayed);
    assert!(store.execute(&plan).unwrap().replayed);
    let state = store.load().unwrap();
    assert_eq!(
        state.captured_artifacts()[&(artifact.id.clone(), artifact.revision.clone())],
        artifact
    );
    assert_eq!(
        store
            .command_result(
                &LocalRef::new("workflow").unwrap(),
                &LocalRef::new("capture-git").unwrap()
            )
            .unwrap()
            .unwrap()
            .output,
        json!({"gitArtifact":{"id":artifact.id,"revision":artifact.revision,"operationId":artifact.operation_id}})
    );
    assert_eq!(
        state.runs()[&run_id("git-run")]
            .task(&task_id())
            .unwrap()
            .state(),
        TaskState::Assigned
    );
    assert!(
        state.runs()[&run_id("git-run")]
            .task(&task_id())
            .unwrap()
            .artifact()
            .is_none()
    );
    let mut altered = artifact.clone();
    altered.files[0].worktree.as_mut().unwrap().content = "changed bytes\n".into();
    let invalid = request(
        &store,
        "invalid-hash",
        Command::Git(GitCommand::RecordArtifact(altered)),
    );
    assert_eq!(
        store.execute(&invalid).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    let mut wrong = intent.clone();
    wrong.operation_id = "different-operation".into();
    wrong.attempt_id = "unknown-attempt".into();
    let invalid = request(&store, "wrong-owner", Command::Git(GitCommand::Plan(wrong)));
    assert_eq!(
        store.execute(&invalid).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    let mut secret = artifact.clone();
    secret.files[0].worktree.as_mut().unwrap().content = "api_key=synthetic-private-value\n".into();
    secret.files[0].worktree.as_mut().unwrap().sha256 = content_hash(
        secret.files[0]
            .worktree
            .as_ref()
            .unwrap()
            .content
            .as_bytes(),
    );
    secret.revision = secret.content_revision();
    let invalid = request(
        &store,
        "secret-content",
        Command::Git(GitCommand::RecordArtifact(secret)),
    );
    assert_eq!(
        store.execute(&invalid).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    let mut oversized = artifact.clone();
    oversized.patch = "x".repeat(orchestrix_core::git_records::MAX_FILE_BYTES + 1);
    let invalid = request(
        &store,
        "oversized-exact-content",
        Command::Git(GitCommand::RecordArtifact(oversized)),
    );
    assert!(matches!(
        store.execute(&invalid),
        Err(StoreError::Privacy(_))
    ));
    for (key, command) in [
        (
            "cleanup-plan",
            GitCommand::PlanCleanup {
                operation_id: intent.operation_id.clone(),
                observation: git_observation("cleanup-planned"),
            },
        ),
        (
            "cleanup-removed",
            GitCommand::ConfirmRemoved {
                operation_id: intent.operation_id.clone(),
                observation: git_observation("removed"),
            },
        ),
    ] {
        execute(&mut store, key, Command::Git(command));
    }
    assert_eq!(
        store.load().unwrap().worktrees()[&intent.operation_id].state,
        WorktreeState::Removed
    );
    drop(store);
    let bytes = std::fs::read(temp.path()).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-private-value"));
    let sql = rusqlite::Connection::open(temp.path()).unwrap();
    assert!(
        sql.execute("UPDATE captured_artifacts SET patch_sha256='tampered'", [])
            .is_err()
    );
    assert!(sql.execute("DELETE FROM git_observations", []).is_err());
    assert!(
        sql.execute("UPDATE git_worktrees SET path='foreign'", [])
            .is_err()
    );
}

struct TestDatabase {
    directory: PathBuf,
}
impl TestDatabase {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "orchestrix-persistence-workflow-{}-{unique}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self { directory }
    }
    fn path(&self) -> PathBuf {
        self.directory.join("state.sqlite")
    }
    fn reopen(&self) -> Store {
        Store::open(self.path()).unwrap()
    }
}
impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Cleanup is limited to this test's explicitly created temporary directory.
        let safe_parent = self.directory.parent() == Some(std::env::temp_dir().as_path());
        let owned_name = self
            .directory
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("orchestrix-persistence-workflow-"));
        if safe_parent && owned_name {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}
fn rev(value: &str) -> RevisionId {
    RevisionId::new(value).unwrap()
}
fn task_id() -> TaskId {
    TaskId::new("task-1").unwrap()
}
fn run_id(value: &str) -> RunId {
    RunId::new(value).unwrap()
}
fn proof(value: &str) -> EvidenceId {
    EvidenceId::new(value).unwrap()
}
fn project() -> Project {
    Project::new(
        ProjectId::new("project-1").unwrap(),
        Repository::new(
            RepositoryId::new("repository-1").unwrap(),
            LocalRef::new("granted-directory-1").unwrap(),
        ),
    )
}
fn connection() -> Connection {
    Connection::new(normalize_descriptor(&fixture()["descriptors"]["base"]).unwrap()).unwrap()
}
fn definition_ref(kind: DefinitionKind, id: &str) -> DefinitionRef {
    DefinitionRef {
        kind,
        id: id.into(),
        version: 1,
    }
}
fn request(store: &Store, key: &str, command: Command) -> Request {
    Request {
        scope: LocalRef::new("workflow").unwrap(),
        key: LocalRef::new(key).unwrap(),
        expected_revision: store.load().unwrap().revision(),
        command,
    }
}
fn execute(store: &mut Store, key: &str, command: Command) -> Receipt {
    let request = request(store, key, command);
    let receipt = store.execute(&request).unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.result.disposition, Disposition::Applied, "{key}");
    receipt
}
fn action(store: &mut Store, key: &str, run: &str, action: RunAction) -> Receipt {
    execute(store, key, Command::run(run_id(run), action))
}
fn seed(store: &mut Store) {
    execute(store, "project", Command::AddProject(project()));
    let connection = connection();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &connection);
    execute(store, "connection", Command::AddConnection(connection));
    execute(store, "session", Command::RegisterSession(session));
    for (kind, id, body) in [
        (
            DefinitionKind::Mode,
            "development",
            json!({"name":"Development","purpose":"Produce checked work"}),
        ),
        (
            DefinitionKind::Profile,
            "implementer",
            json!({"name":"Implementer","role":"Write code","instructions_ref":"instructions-1"}),
        ),
        (
            DefinitionKind::Context,
            "context-pack-1",
            json!({"manifest_ref":"manifest-1"}),
        ),
        (
            DefinitionKind::Configuration,
            "configuration-1",
            json!({"strategy":"balanced","billing":"subscription-only"}),
        ),
    ] {
        execute(
            store,
            &format!("definition-{id}"),
            Command::Records(RecordsCommand::AddDefinition(DefinitionInput {
                kind,
                id: id.into(),
                version: 1,
                body,
                references: vec![],
            })),
        );
    }
}
fn create_run(store: &mut Store, name: &str, kind: TaskKind) {
    let project = project();
    let run = Run::new(
        run_id(name),
        (kind == TaskKind::Code).then_some(&project),
        rev("base-1"),
        if kind == TaskKind::Code {
            DeliveryRequirement::FinalApplication
        } else {
            DeliveryRequirement::AcceptedArtifacts
        },
    )
    .unwrap();
    execute(store, &format!("create-{name}"), Command::create_run(run));
    let task = Task::new(
        task_id(),
        "Produce a checked result",
        kind,
        vec!["Meets the objective".into()],
        vec![],
    )
    .unwrap();
    action(
        store,
        &format!("task-{name}"),
        name,
        RunAction::AddTask(task),
    );
    action(
        store,
        &format!("ready-{name}"),
        name,
        RunAction::Ready(task_id()),
    );
}
fn reservation(store: &Store, key: &str, run: &str, attempt: &str, kind: TaskKind) -> Request {
    let connection = connection();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &connection);
    let mut runtime_request = fixture()["requests"]["base"].clone();
    runtime_request["attemptId"] = json!(attempt);
    let attempt_value = TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&runtime_request).unwrap(),
        AttemptInputs {
            plan_revision: rev("plan-1"),
            context: ContextVersion {
                reference: ContextPackId::new("context-pack-1").unwrap(),
                version: DefinitionVersion::new(1).unwrap(),
            },
            mode: ActionMode::new(
                ModeId::new("development").unwrap(),
                DefinitionVersion::new(1).unwrap(),
                "Development",
                "Produce checked work",
            )
            .unwrap(),
            profile: AgentProfile::new(
                ProfileId::new("implementer").unwrap(),
                DefinitionVersion::new(1).unwrap(),
                "Implementer",
                "Write code",
                LocalRef::new("instructions-1").unwrap(),
            )
            .unwrap(),
            base: rev("base-1"),
            worktree: LocalRef::new("worktree-1").unwrap(),
            project: (kind == TaskKind::Code).then(|| project().id().clone()),
            correction_of: None,
        },
    )
    .unwrap();
    let snapshot = AttemptSnapshot {
        definition: SnapshotInput {
            id: format!("snapshot-{attempt}"),
            attempt_id: attempt.into(),
            mode: definition_ref(DefinitionKind::Mode, "development"),
            profile: definition_ref(DefinitionKind::Profile, "implementer"),
            context: definition_ref(DefinitionKind::Context, "context-pack-1"),
            configuration: definition_ref(DefinitionKind::Configuration, "configuration-1"),
        },
        diagnostics: vec![],
    };
    request(
        store,
        key,
        Command::run(
            run_id(run),
            RunAction::assign(task_id(), attempt_value, snapshot),
        ),
    )
}
fn frame(attempt: &str, sequence: u64, kind: &str, extra: Value) -> String {
    let mut value = json!({"version":{"major":1,"minor":1},"connectionId":"connection-1","connectionRevision":1,
        "sessionId":"session-1","attemptId":attempt,"sequence":sequence,"type":kind});
    for (key, item) in extra.as_object().unwrap() {
        value[key] = item.clone();
    }
    value.to_string()
}
fn complete_runtime(store: &mut Store, run: &str, attempt: &str) {
    for (sequence, kind, extra) in [
        (1, "attempt.admission-observed", json!({})),
        (2, "attempt.start-observed", json!({})),
        (
            3,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
    ] {
        action(
            store,
            &format!("{attempt}-frame-{sequence}"),
            run,
            RunAction::Observe {
                task: task_id(),
                frame: frame(attempt, sequence, kind, extra),
                origin: EventOrigin::Runtime,
            },
        );
    }
}
fn artifact(revision: &str) -> ArtifactVersion {
    ArtifactVersion::new(
        ArtifactId::new("output-1").unwrap(),
        rev(revision),
        rev("base-1"),
    )
}
fn approve_artifact(store: &mut Store, run: &str, artifact: &ArtifactVersion) {
    action(
        store,
        &format!("verify-{}", artifact.revision().as_str()),
        run,
        RunAction::Verify {
            task: task_id(),
            evidence: VerificationEvidence {
                artifact: artifact.clone(),
                reference: proof(&format!("check-{}", artifact.revision().as_str())),
                result: CheckResult::Passed,
            },
        },
    );
    action(
        store,
        &format!("review-{}", artifact.revision().as_str()),
        run,
        RunAction::Review {
            task: task_id(),
            evidence: ReviewEvidence {
                artifact: artifact.clone(),
                reference: proof(&format!("review-{}", artifact.revision().as_str())),
                result: ReviewResult::Accepted,
            },
        },
    );
}
fn produced_code(store: &mut Store) -> ArtifactVersion {
    seed(store);
    create_run(store, "run-1", TaskKind::Code);
    let request = reservation(store, "reserve", "run-1", "attempt-1", TaskKind::Code);
    assert_eq!(
        store.execute(&request).unwrap().result.disposition,
        Disposition::Applied
    );
    complete_runtime(store, "run-1", "attempt-1");
    let artifact = artifact("output-v1");
    action(
        store,
        "publish",
        "run-1",
        RunAction::Publish {
            task: task_id(),
            artifact: artifact.clone(),
        },
    );
    artifact
}
fn prepared_code(store: &mut Store) -> ArtifactVersion {
    let artifact = produced_code(store);
    approve_artifact(store, "run-1", &artifact);
    artifact
}
fn integration(artifact: ArtifactVersion, validation: CheckResult) -> IntegrationEvidence {
    IntegrationEvidence {
        run: run_id("run-1"),
        task: task_id(),
        artifact,
        input_base: rev("base-1"),
        result_revision: rev("run-v1"),
        reference: proof("integration-1"),
        validation,
    }
}
fn evidence_count(path: &Path, kind: &str) -> i64 {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .query_row(
            "SELECT count(*) FROM evidence WHERE kind=?1",
            [kind],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn durable_reservation_replays_after_reopen_then_domain_continues_to_delivery() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    seed(&mut store);
    create_run(&mut store, "run-1", TaskKind::Code);
    let reservation = reservation(&store, "reserve", "run-1", "attempt-1", TaskKind::Code);
    let original = store.execute(&reservation).unwrap();
    assert_eq!(original.result.disposition, Disposition::Applied);
    let events = store.event_count().unwrap();
    let commands = store.command_count().unwrap();
    assert!(store.execute(&reservation).unwrap().replayed);
    drop(store);

    let mut store = database.reopen();
    let replay = store.execute(&reservation).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result, original.result);
    assert_eq!(store.event_count().unwrap(), events);
    assert_eq!(store.command_count().unwrap(), commands);
    let state = store.load().unwrap();
    let task = state.runs()[&run_id("run-1")].task(&task_id()).unwrap();
    assert_eq!(task.state(), TaskState::Assigned);
    assert_eq!(task.attempts().len(), 1);
    assert_eq!(
        task.current_attempt().unwrap().state(),
        AttemptState::Created
    );
    assert_eq!(state.snapshots().len(), 1);
    assert_eq!(state.records().snapshots().len(), 1);

    complete_runtime(&mut store, "run-1", "attempt-1");
    let output = artifact("output-v1");
    action(
        &mut store,
        "publish",
        "run-1",
        RunAction::Publish {
            task: task_id(),
            artifact: output.clone(),
        },
    );
    approve_artifact(&mut store, "run-1", &output);
    drop(store);

    let mut store = database.reopen();
    let state = store.load().unwrap();
    let task = state.runs()[&run_id("run-1")].task(&task_id()).unwrap();
    assert_eq!(task.state(), TaskState::Approved);
    assert_eq!(task.artifact(), Some(&output));
    assert_eq!(
        task.current_attempt().unwrap().runtime_snapshot()["events"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    action(
        &mut store,
        "integrate",
        "run-1",
        RunAction::Integrate(integration(output, CheckResult::Passed)),
    );
    let prepared = action(
        &mut store,
        "prepare",
        "run-1",
        RunAction::Prepare(rev("target-1")),
    );
    let candidate = DeliveryCandidate::restore_checkpoint(&prepared.result.output).unwrap();
    action(
        &mut store,
        "approve",
        "run-1",
        RunAction::Approve {
            candidate: candidate.clone(),
            target_base: rev("target-1"),
            reference: proof("approval-1"),
        },
    );
    drop(store);

    let mut store = database.reopen();
    action(
        &mut store,
        "apply",
        "run-1",
        RunAction::Apply {
            candidate,
            evidence: ApplicationEvidence {
                run: run_id("run-1"),
                repository: RepositoryId::new("repository-1").unwrap(),
                result: rev("run-v1"),
                target_base: rev("target-1"),
                applied_revision: rev("application-v1"),
                reference: proof("application-1"),
                validation: CheckResult::Passed,
            },
        },
    );
    action(&mut store, "deliver", "run-1", RunAction::Deliver);
    drop(store);

    let store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    assert_eq!(run.state(), RunState::Delivered);
    assert_eq!(run.task(&task_id()).unwrap().integrations().len(), 1);
    assert_eq!(
        run.application().unwrap().applied_revision,
        rev("application-v1")
    );
    assert_eq!(evidence_count(&database.path(), "integrations"), 1);
    assert_eq!(evidence_count(&database.path(), "application"), 1);
}

#[test]
fn negative_integration_commits_its_evidence_and_block_then_replays_after_reopen() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    let output = prepared_code(&mut store);
    let request = request(
        &store,
        "failed-integration",
        Command::run(
            run_id("run-1"),
            RunAction::Integrate(integration(output, CheckResult::Failed)),
        ),
    );
    let events = store.event_count().unwrap();
    let result = store.execute(&request).unwrap();
    assert_eq!(
        result.result.disposition,
        Disposition::ObservedFailureCommitted
    );
    assert!(result.result.error_code.is_some());
    assert_eq!(store.event_count().unwrap(), events + 1);
    drop(store);

    let mut store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    assert_eq!(run.state(), RunState::Blocked);
    assert_eq!(run.result_revision(), &rev("base-1"));
    assert!(run.candidate().is_none());
    assert!(run.approval().is_none());
    let evidence = run.task(&task_id()).unwrap().integrations();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].validation, CheckResult::Failed);
    assert_eq!(evidence_count(&database.path(), "integrations"), 1);
    let replay = store.execute(&request).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result, result.result);
    assert_eq!(store.event_count().unwrap(), events + 1);
}

#[test]
fn observed_integration_failure_is_atomic_at_each_injected_write_boundary() {
    for fault in [
        FaultPoint::AfterState,
        FaultPoint::AfterEvent,
        FaultPoint::AfterResult,
    ] {
        let database = TestDatabase::new();
        let mut store = database.reopen();
        let output = prepared_code(&mut store);
        let detached = store.load().unwrap();
        let before = detached.runs()[&run_id("run-1")].checkpoint();
        let revision = detached.revision();
        let events = store.event_count().unwrap();
        let commands = store.command_count().unwrap();
        let command = request(
            &store,
            "failed-integration",
            Command::run(
                run_id("run-1"),
                RunAction::Integrate(integration(output, CheckResult::Failed)),
            ),
        );
        assert_eq!(
            store.execute_with_fault(&command, fault),
            Err(StoreError::InjectedRollback)
        );
        assert_eq!(detached.runs()[&run_id("run-1")].checkpoint(), before);
        assert_eq!(
            detached.runs()[&run_id("run-1")]
                .task(&task_id())
                .unwrap()
                .state(),
            TaskState::Approved
        );
        drop(store);

        let mut store = database.reopen();
        let state = store.load().unwrap();
        assert_eq!(state.revision(), revision);
        assert_eq!(state.runs()[&run_id("run-1")].checkpoint(), before);
        assert!(
            state.runs()[&run_id("run-1")]
                .task(&task_id())
                .unwrap()
                .integrations()
                .is_empty()
        );
        assert_eq!(evidence_count(&database.path(), "integrations"), 0);
        assert_eq!(store.event_count().unwrap(), events);
        assert_eq!(store.command_count().unwrap(), commands);
        let committed = store.execute(&command).unwrap();
        assert_eq!(
            committed.result.disposition,
            Disposition::ObservedFailureCommitted
        );
        assert_eq!(committed.result.revision, revision + 1);
        assert_eq!(
            store.load().unwrap().runs()[&run_id("run-1")].state(),
            RunState::Blocked
        );
        assert_eq!(evidence_count(&database.path(), "integrations"), 1);
        assert_eq!(store.event_count().unwrap(), events + 1);
        assert_eq!(store.command_count().unwrap(), commands + 1);
        assert!(store.execute(&command).unwrap().replayed);
        assert_eq!(store.event_count().unwrap(), events + 1);
    }
}

#[test]
fn rejected_secret_frame_preserves_quarantine_and_only_owned_cleanup_after_reopen() {
    const SECRET: &str = "sk-orchestrix_SYNTHETIC_NEVER_STORE_123456";
    const RAW_MARKER: &str = "synthetic-malformed-frame-only";
    let database = TestDatabase::new();
    let mut store = database.reopen();
    seed(&mut store);
    create_run(&mut store, "run-1", TaskKind::Code);
    let reserve = reservation(&store, "reserve", "run-1", "attempt-1", TaskKind::Code);
    store.execute(&reserve).unwrap();
    let malformed = format!("{{\"api_key\":\"{SECRET}\",\"raw\":\"{RAW_MARKER}\"");
    let rejected = request(
        &store,
        "bad-runtime-frame",
        Command::run(
            run_id("run-1"),
            RunAction::Observe {
                task: task_id(),
                frame: malformed,
                origin: EventOrigin::Runtime,
            },
        ),
    );
    let result = store.execute(&rejected).unwrap();
    assert_eq!(
        result.result.disposition,
        Disposition::ObservedFailureCommitted
    );
    assert!(
        !serde_json::to_string(&result.result)
            .unwrap()
            .contains(SECRET)
    );
    drop(store);

    let mut store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    let task = run.task(&task_id()).unwrap();
    assert_eq!(run.state(), RunState::Blocked);
    assert_eq!(task.state(), TaskState::Blocked);
    assert_eq!(task.attempts().len(), 1);
    let snapshot = task.current_attempt().unwrap().runtime_snapshot();
    assert_eq!(snapshot["fatal"], true);
    assert_eq!(snapshot["fault"]["code"], "protocol");
    assert_eq!(snapshot["observation"], "uncertain");
    assert_eq!(snapshot["terminalConfirmed"], false);
    assert!(!run.checkpoint().to_string().contains(SECRET));
    assert!(!run.checkpoint().to_string().contains(RAW_MARKER));
    assert!(store.execute(&rejected).unwrap().replayed);
    let retry = request(
        &store,
        "retry-is-forbidden",
        Command::run(run_id("run-1"), RunAction::Ready(task_id())),
    );
    assert_eq!(
        store.execute(&retry).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    let spoofed_cleanup = request(
        &store,
        "runtime-is-not-supervisor",
        Command::run(
            run_id("run-1"),
            RunAction::Observe {
                task: task_id(),
                frame: frame(
                    "attempt-1",
                    1,
                    "process.exited",
                    json!({"role":"runtime","code":137}),
                ),
                origin: EventOrigin::Runtime,
            },
        ),
    );
    assert_eq!(
        store.execute(&spoofed_cleanup).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    action(
        &mut store,
        "owned-cleanup",
        "run-1",
        RunAction::Observe {
            task: task_id(),
            frame: frame(
                "attempt-1",
                1,
                "process.exited",
                json!({"role":"runtime","code":137}),
            ),
            origin: EventOrigin::Supervisor,
        },
    );
    let late_terminal = request(
        &store,
        "runtime-terminal-is-quarantined",
        Command::run(
            run_id("run-1"),
            RunAction::Observe {
                task: task_id(),
                frame: frame(
                    "attempt-1",
                    2,
                    "attempt.terminal-observed",
                    json!({"outcome":"completed"}),
                ),
                origin: EventOrigin::Runtime,
            },
        ),
    );
    assert_eq!(
        store.execute(&late_terminal).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    drop(store);

    let store = database.reopen();
    let state = store.load().unwrap();
    let task = state.runs()[&run_id("run-1")].task(&task_id()).unwrap();
    assert_eq!(task.attempts().len(), 1);
    assert_eq!(
        task.current_attempt().unwrap().state(),
        AttemptState::Uncertain
    );
    let snapshot = task.current_attempt().unwrap().runtime_snapshot();
    assert_eq!(snapshot["process"]["runtime"]["exited"], true);
    assert_eq!(snapshot["terminalConfirmed"], false);
    assert_eq!(snapshot["outcome"], "unknown");
    assert_eq!(snapshot["budget"]["frames"], 1);
    drop(store);
    for entry in std::fs::read_dir(&database.directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            for needle in [SECRET.as_bytes(), RAW_MARKER.as_bytes()] {
                assert!(
                    !bytes.windows(needle.len()).any(|window| window == needle),
                    "rejected raw frame reached a SQLite file"
                );
            }
        }
    }
}

#[test]
fn failed_application_keeps_negative_evidence_after_reopen_and_idempotent_replay() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    let output = prepared_code(&mut store);
    action(
        &mut store,
        "integrate",
        "run-1",
        RunAction::Integrate(integration(output, CheckResult::Passed)),
    );
    let prepared = action(
        &mut store,
        "prepare",
        "run-1",
        RunAction::Prepare(rev("target-1")),
    );
    let candidate = DeliveryCandidate::restore_checkpoint(&prepared.result.output).unwrap();
    action(
        &mut store,
        "approve",
        "run-1",
        RunAction::Approve {
            candidate: candidate.clone(),
            target_base: rev("target-1"),
            reference: proof("approval-1"),
        },
    );
    let failure = request(
        &store,
        "application-failed",
        Command::run(
            run_id("run-1"),
            RunAction::Apply {
                candidate,
                evidence: ApplicationEvidence {
                    run: run_id("run-1"),
                    repository: RepositoryId::new("repository-1").unwrap(),
                    result: rev("run-v1"),
                    target_base: rev("target-1"),
                    applied_revision: rev("rejected-application"),
                    reference: proof("failed-application-proof"),
                    validation: CheckResult::Failed,
                },
            },
        ),
    );
    let events = store.event_count().unwrap();
    let failed = store.execute(&failure).unwrap();
    assert_eq!(
        failed.result.disposition,
        Disposition::ObservedFailureCommitted
    );
    drop(store);

    let mut store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    assert_eq!(run.state(), RunState::Blocked);
    assert!(run.application().is_none());
    assert!(run.approval().is_none());
    assert_eq!(run.application_failures().len(), 1);
    assert_eq!(
        run.application_failures()[0].reference,
        proof("failed-application-proof")
    );
    assert_eq!(
        run.application_failures()[0].validation,
        CheckResult::Failed
    );
    assert_eq!(evidence_count(&database.path(), "applicationFailures"), 1);
    let replay = store.execute(&failure).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result, failed.result);
    assert_eq!(store.event_count().unwrap(), events + 1);
}

#[test]
fn reloaded_artifact_and_approval_keep_exact_versions_and_stale_invalidation() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    let old = produced_code(&mut store);
    // An accepted review alone cannot approve code: verification is still missing.
    // Replacement is permitted during review, not after Task approval.
    action(
        &mut store,
        "old-artifact-review",
        "run-1",
        RunAction::Review {
            task: task_id(),
            evidence: ReviewEvidence {
                artifact: old.clone(),
                reference: proof("old-review"),
                result: ReviewResult::Accepted,
            },
        },
    );
    let current = artifact("output-v2");
    action(
        &mut store,
        "replace-artifact",
        "run-1",
        RunAction::Publish {
            task: task_id(),
            artifact: current.clone(),
        },
    );
    drop(store);

    let mut store = database.reopen();
    let event_count = store.event_count().unwrap();
    let stale_check = request(
        &store,
        "stale-check",
        Command::run(
            run_id("run-1"),
            RunAction::Verify {
                task: task_id(),
                evidence: VerificationEvidence {
                    artifact: old.clone(),
                    reference: proof("old-check"),
                    result: CheckResult::Passed,
                },
            },
        ),
    );
    let rejected = store.execute(&stale_check).unwrap();
    assert_eq!(rejected.result.disposition, Disposition::InvalidNoChange);
    assert_eq!(store.event_count().unwrap(), event_count);
    assert_eq!(
        store.load().unwrap().runs()[&run_id("run-1")]
            .task(&task_id())
            .unwrap()
            .artifact(),
        Some(&current)
    );
    approve_artifact(&mut store, "run-1", &current);
    action(
        &mut store,
        "integrate",
        "run-1",
        RunAction::Integrate(integration(current, CheckResult::Passed)),
    );
    let prepared = action(
        &mut store,
        "prepare",
        "run-1",
        RunAction::Prepare(rev("target-1")),
    );
    let candidate = DeliveryCandidate::restore_checkpoint(&prepared.result.output).unwrap();
    action(
        &mut store,
        "approve",
        "run-1",
        RunAction::Approve {
            candidate: candidate.clone(),
            target_base: rev("target-1"),
            reference: proof("approval-1"),
        },
    );
    drop(store);

    let mut store = database.reopen();
    assert!(
        store.load().unwrap().runs()[&run_id("run-1")]
            .approval()
            .is_some()
    );
    let stale_approval = request(
        &store,
        "stale-approval",
        Command::run(
            run_id("run-1"),
            RunAction::Approve {
                candidate,
                target_base: rev("target-2"),
                reference: proof("stale-approval"),
            },
        ),
    );
    let rejected = store.execute(&stale_approval).unwrap();
    assert_eq!(
        rejected.result.disposition,
        Disposition::ObservedFailureCommitted
    );
    drop(store);

    let store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    assert_eq!(run.state(), RunState::Blocked);
    assert!(run.approval().is_none());
    assert!(run.candidate().is_none());
    assert_eq!(run.task(&task_id()).unwrap().verifications().len(), 1);
    assert_eq!(run.task(&task_id()).unwrap().reviews().len(), 2);
    assert_eq!(
        run.task(&task_id())
            .unwrap()
            .current_attempt()
            .unwrap()
            .artifacts(),
        &[old, artifact("output-v2")]
    );
}

#[test]
fn session_binding_and_unresolved_reservation_are_global_across_runs_and_reopen() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    seed(&mut store);
    create_run(&mut store, "run-a", TaskKind::Analysis);
    create_run(&mut store, "run-b", TaskKind::Analysis);
    let first = reservation(
        &store,
        "reserve-a",
        "run-a",
        "attempt-a",
        TaskKind::Analysis,
    );
    store.execute(&first).unwrap();
    drop(store);

    let mut store = database.reopen();
    let second = reservation(
        &store,
        "reserve-b-blocked",
        "run-b",
        "attempt-b",
        TaskKind::Analysis,
    );
    let result = store.execute(&second).unwrap();
    assert_eq!(result.result.disposition, Disposition::InvalidNoChange);
    assert!(
        store.load().unwrap().runs()[&run_id("run-b")]
            .task(&task_id())
            .unwrap()
            .attempts()
            .is_empty()
    );
    let mut descriptor = fixture()["descriptors"]["base"].clone();
    descriptor["connection"]["id"] = json!("connection-2");
    let other = Connection::new(normalize_descriptor(&descriptor).unwrap()).unwrap();
    let rebound = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &other);
    execute(
        &mut store,
        "other-connection",
        Command::AddConnection(other),
    );
    let rebind = request(&store, "rebind", Command::RegisterSession(rebound));
    assert_eq!(
        store.execute(&rebind).unwrap().result.disposition,
        Disposition::InvalidNoChange
    );
    assert_eq!(
        store.load().unwrap().sessions()[&RuntimeSessionId::new("session-1").unwrap()]
            .connection_id(),
        &ConnectionId::new("connection-1").unwrap()
    );
    action(
        &mut store,
        "first-terminal",
        "run-a",
        RunAction::Observe {
            task: task_id(),
            frame: frame(
                "attempt-a",
                1,
                "attempt.terminal-observed",
                json!({"outcome":"completed"}),
            ),
            origin: EventOrigin::Runtime,
        },
    );
    let second = reservation(
        &store,
        "reserve-b-after-terminal",
        "run-b",
        "attempt-b",
        TaskKind::Analysis,
    );
    assert_eq!(
        store.execute(&second).unwrap().result.disposition,
        Disposition::Applied
    );
    drop(store);

    let store = database.reopen();
    let state = store.load().unwrap();
    assert_eq!(
        state.runs()[&run_id("run-a")]
            .task(&task_id())
            .unwrap()
            .current_attempt()
            .unwrap()
            .state(),
        AttemptState::Succeeded
    );
    assert_eq!(
        state.runs()[&run_id("run-b")]
            .task(&task_id())
            .unwrap()
            .state(),
        TaskState::Assigned
    );
    assert_eq!(state.snapshots().len(), 2);
}

const CHILD_DATABASE: &str = "ORCHESTRIX_PERSISTENCE_WORKFLOW_CHILD_DATABASE";

#[test]
fn another_process_reconstructs_attempt_evidence_and_continues_guarded_integration() {
    let database = TestDatabase::new();
    let mut store = database.reopen();
    prepared_code(&mut store);
    let revision = store.load().unwrap().revision();
    let events = store.event_count().unwrap();
    let commands = store.command_count().unwrap();
    drop(store);

    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_reopen_and_integrate_owned_workflow",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_DATABASE, database.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let store = database.reopen();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    let task = run.task(&task_id()).unwrap();
    assert_eq!(task.state(), TaskState::Completed);
    assert_eq!(task.integrations().len(), 1);
    assert_eq!(task.integrations()[0].validation, CheckResult::Passed);
    assert_eq!(run.result_revision(), &rev("run-v1"));
    assert_eq!(
        task.current_attempt().unwrap().state(),
        AttemptState::Succeeded
    );
    assert_eq!(
        task.current_attempt().unwrap().artifacts(),
        &[artifact("output-v1")]
    );
    assert_eq!(state.revision(), revision + 1);
    assert_eq!(store.event_count().unwrap(), events + 1);
    assert_eq!(store.command_count().unwrap(), commands + 1);
    assert_eq!(evidence_count(&database.path(), "integrations"), 1);
}

#[test]
#[ignore = "invoked only by the parent file-backed process test with its own temporary path"]
fn child_reopen_and_integrate_owned_workflow() {
    let path = PathBuf::from(
        std::env::var_os(CHILD_DATABASE).expect("parent supplies its temporary database"),
    );
    let directory = path.parent().expect("database directory");
    assert_eq!(path.file_name().unwrap(), "state.sqlite");
    assert_eq!(directory.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("orchestrix-persistence-workflow-")
    );
    let mut store = Store::open(path).unwrap();
    let state = store.load().unwrap();
    let run = &state.runs()[&run_id("run-1")];
    let task = run.task(&task_id()).unwrap();
    assert_eq!(task.state(), TaskState::Approved);
    assert_eq!(task.attempts().len(), 1);
    assert_eq!(task.verifications().len(), 1);
    assert_eq!(task.reviews().len(), 1);
    assert_eq!(task.artifact(), Some(&artifact("output-v1")));
    let attempt = task.current_attempt().unwrap();
    assert_eq!(attempt.state(), AttemptState::Succeeded);
    assert_eq!(
        attempt.runtime_snapshot()["events"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(attempt.runtime_snapshot()["budget"]["frames"], 3);
    assert_eq!(state.snapshots().len(), 1);
    action(
        &mut store,
        "child-integrate",
        "run-1",
        RunAction::Integrate(integration(artifact("output-v1"), CheckResult::Passed)),
    );
}
