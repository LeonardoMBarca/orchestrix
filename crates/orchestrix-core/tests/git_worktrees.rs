//! Real Git + SQLite in disposable repositories. Fixture edits are deterministic,
//! never a simulated live agent response or a project check invocation.
use orchestrix_core::{
    domain::*,
    git::{CreateWorktree, GitError, GitFaultPoint, WorktreeService},
    git_records::{WorktreeRecord, WorktreeState},
    persistence::{AttemptSnapshot, Command, Disposition, Request, RunAction, Store},
    persistence_records::{
        DefinitionInput, DefinitionKind, DefinitionRef, RecordsCommand, SnapshotInput,
    },
    runtime_contract::{EventOrigin, normalize_descriptor, normalize_request},
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    main: PathBuf,
    authorized: PathBuf,
    db: PathBuf,
    base: String,
}
impl Fixture {
    fn new() -> Self {
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        let root = temp.join(format!(
            "orchestrix-worktrees-espa\u{e7}o-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let main = root.join("main repository acentua\u{e7}\u{e3}o");
        let authorized = root.join("authorized attempts");
        fs::create_dir(&main).unwrap();
        fs::create_dir(&authorized).unwrap();
        git(&main, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(main.join("source.txt"), "base\n").unwrap();
        fs::write(main.join("delete.txt"), "delete base\n").unwrap();
        fs::write(main.join("constant.md"), "constant base\n").unwrap();
        fs::write(main.join(".gitignore"), "ignored/\n").unwrap();
        git(&main, &["add", "--all"]);
        git(
            &main,
            &["commit", "--quiet", "-m", "Owned Git worktree fixture"],
        );
        let base = git_text(&main, &["rev-parse", "HEAD"]);
        let fixture = Self {
            db: root.join("state.sqlite"),
            root,
            main,
            authorized,
            base,
        };
        let mut store = Store::open(&fixture.db).unwrap();
        seed(&mut store);
        fixture
    }
    fn store(&self) -> Store {
        Store::open(&self.db).unwrap()
    }
    fn request(&self, store: &mut Store, number: u64) -> CreateWorktree {
        reserve(store, &self.base, number);
        CreateWorktree {
            scope: local("git-fixture"),
            key: local(&format!("create-{number}")),
            expected_revision: store.load().unwrap().revision(),
            project: ProjectId::new("project").unwrap(),
            repository: RepositoryId::new("repository").unwrap(),
            run: RunId::new("run").unwrap(),
            task: task(number),
            attempt: attempt(number),
            directory_reference: local("granted-directory"),
            worktree_reference: local(&format!("worktree-{number}")),
            repository_path: self.main.clone(),
            authorized_directory: self.authorized.clone(),
            base_commit: self.base.clone(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only recursively remove this test's verified unique temporary directory.
        let root = fs::canonicalize(&self.root).unwrap();
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        assert!(
            root.parent() == Some(temp.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchestrix-worktrees-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
fn git_command(root: &Path, args: &[&str]) -> ProcessCommand {
    let mut command = ProcessCommand::new("git");
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
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
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
        .current_dir(root);
    command
}
fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let result = git_command(root, args).output().unwrap();
    assert!(
        result.status.success(),
        "fixture Git failed: {:?} {}",
        args,
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}
fn git_text(root: &Path, args: &[&str]) -> String {
    String::from_utf8(git(root, args))
        .unwrap()
        .trim_end_matches(['\r', '\n'])
        .into()
}
fn local(s: &str) -> LocalRef {
    LocalRef::new(s).unwrap()
}
fn task(n: u64) -> TaskId {
    TaskId::new(format!("task-{n}")).unwrap()
}
fn attempt(n: u64) -> AttemptId {
    AttemptId::new(format!("attempt-{n}")).unwrap()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}
fn project() -> Project {
    Project::new(
        ProjectId::new("project").unwrap(),
        Repository::new(
            RepositoryId::new("repository").unwrap(),
            local("granted-directory"),
        ),
    )
}
fn execute(store: &mut Store, key: &str, command: Command) {
    let expected_revision = store.load().unwrap().revision();
    let request = Request {
        scope: local("seed"),
        key: local(key),
        expected_revision,
        command,
    };
    assert_eq!(
        store.execute(&request).unwrap().result.disposition,
        Disposition::Applied,
        "seed {key}"
    );
}
fn definition(kind: DefinitionKind, id: &str) -> DefinitionRef {
    DefinitionRef {
        kind,
        id: id.into(),
        version: 1,
    }
}
fn seed(store: &mut Store) {
    execute(store, "project", Command::AddProject(project()));
    execute(
        store,
        "connection",
        Command::AddConnection(
            Connection::new(normalize_descriptor(&fixture()["descriptors"]["base"]).unwrap())
                .unwrap(),
        ),
    );
    for (kind, id, body) in [
        (
            DefinitionKind::Mode,
            "development",
            json!({"name":"Development","purpose":"Produce checked work"}),
        ),
        (
            DefinitionKind::Profile,
            "implementer",
            json!({"name":"Implementer","role":"Write code","instructions_ref":"instructions"}),
        ),
        (
            DefinitionKind::Context,
            "context-pack-1",
            json!({"manifest_ref":"manifest"}),
        ),
        (
            DefinitionKind::Configuration,
            "configuration",
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
fn reserve(store: &mut Store, base: &str, n: u64) {
    if store.load().unwrap().runs().is_empty() {
        execute(
            store,
            "run",
            Command::create_run(
                Run::new(
                    RunId::new("run").unwrap(),
                    Some(&project()),
                    RevisionId::new(base).unwrap(),
                    DeliveryRequirement::FinalApplication,
                )
                .unwrap(),
            ),
        );
    }
    let connection =
        Connection::new(normalize_descriptor(&fixture()["descriptors"]["base"]).unwrap()).unwrap();
    let session = RuntimeSessionRef::new(
        RuntimeSessionId::new(format!("session-{n}")).unwrap(),
        &connection,
    );
    execute(
        store,
        &format!("session-{n}"),
        Command::RegisterSession(session.clone()),
    );
    execute(
        store,
        &format!("task-{n}"),
        Command::run(
            RunId::new("run").unwrap(),
            RunAction::AddTask(
                Task::new(
                    task(n),
                    "Deterministic fixture change",
                    TaskKind::Code,
                    vec!["Fixture bytes match".into()],
                    vec![],
                )
                .unwrap(),
            ),
        ),
    );
    execute(
        store,
        &format!("ready-{n}"),
        Command::run(RunId::new("run").unwrap(), RunAction::Ready(task(n))),
    );
    let mut request = fixture()["requests"]["base"].clone();
    request["attemptId"] = json!(attempt(n).as_str());
    request["sessionId"] = json!(session.id().as_str());
    let value = TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&request).unwrap(),
        AttemptInputs {
            plan_revision: RevisionId::new("plan").unwrap(),
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
                local("instructions"),
            )
            .unwrap(),
            base: RevisionId::new(base).unwrap(),
            worktree: local(&format!("worktree-{n}")),
            project: Some(ProjectId::new("project").unwrap()),
            correction_of: None,
        },
    )
    .unwrap();
    let snapshot = AttemptSnapshot {
        definition: SnapshotInput {
            id: format!("snapshot-{n}"),
            attempt_id: attempt(n).as_str().into(),
            mode: definition(DefinitionKind::Mode, "development"),
            profile: definition(DefinitionKind::Profile, "implementer"),
            context: definition(DefinitionKind::Context, "context-pack-1"),
            configuration: definition(DefinitionKind::Configuration, "configuration"),
        },
        diagnostics: vec![],
    };
    execute(
        store,
        &format!("reserve-{n}"),
        Command::run(
            RunId::new("run").unwrap(),
            RunAction::assign(task(n), value, snapshot),
        ),
    );
}
fn current_revision(store: &Store) -> u64 {
    store.load().unwrap().revision()
}
fn operation(store: &Store) -> WorktreeRecord {
    store
        .load()
        .unwrap()
        .worktrees()
        .values()
        .next()
        .unwrap()
        .clone()
}

#[test]
fn two_attempts_isolate_real_git_and_preserve_dirty_main_then_reopen_in_another_process() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    fs::write(fixture.main.join("source.txt"), "staged main\n").unwrap();
    git(&fixture.main, &["add", "source.txt"]);
    fs::write(fixture.main.join("source.txt"), "unstaged main\n").unwrap();
    fs::write(fixture.main.join("main-untracked.txt"), "keep main new\n").unwrap();
    let status = git(
        &fixture.main,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    );
    let index = fs::read(fixture.main.join(".git/index")).unwrap();
    let head = git(&fixture.main, &["rev-parse", "HEAD"]);
    let branch = git(&fixture.main, &["symbolic-ref", "HEAD"]);
    let request1 = fixture.request(&mut store, 1);
    let first = service.create(&mut store, &request1).unwrap();
    let request2 = fixture.request(&mut store, 2);
    let second = service.create(&mut store, &request2).unwrap();
    assert_ne!(first.intent.path, second.intent.path);
    assert_ne!(first.intent.branch, second.intent.branch);
    assert_eq!(first.intent.base_oid, second.intent.base_oid);
    let work = Path::new(&first.intent.path);
    fs::write(work.join("source.txt"), "staged attempt\n").unwrap();
    git(work, &["add", "source.txt"]);
    fs::write(work.join("source.txt"), "unstaged attempt\n").unwrap();
    fs::remove_file(work.join("delete.txt")).unwrap();
    fs::write(work.join("new.md"), "new attempt\n").unwrap();
    let revision = current_revision(&store);
    let artifact = service
        .capture(
            &mut store,
            &local("git-fixture"),
            &local("capture-1"),
            revision,
            &first.intent.operation_id,
        )
        .unwrap();
    assert_eq!(artifact.files.len(), 3);
    let source = artifact
        .files
        .iter()
        .find(|f| f.path == "source.txt")
        .unwrap();
    assert_eq!(source.base.as_ref().unwrap().content, "base\n");
    assert_eq!(source.index.as_ref().unwrap().content, "staged attempt\n");
    assert_eq!(
        source.worktree.as_ref().unwrap().content,
        "unstaged attempt\n"
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|f| f.path == "new.md" && f.untracked && f.base.is_none())
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|f| f.path == "delete.txt" && f.worktree.is_none() && f.base.is_some())
    );
    assert_eq!(
        fs::read_to_string(Path::new(&second.intent.path).join("source.txt")).unwrap(),
        "base\n"
    );
    assert_eq!(
        git(
            &fixture.main,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"]
        ),
        status
    );
    assert_eq!(fs::read(fixture.main.join(".git/index")).unwrap(), index);
    assert_eq!(git(&fixture.main, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&fixture.main, &["symbolic-ref", "HEAD"]), branch);
    assert_eq!(
        fs::read_to_string(fixture.main.join("main-untracked.txt")).unwrap(),
        "keep main new\n"
    );
    assert_eq!(service.create(&mut store, &request1).unwrap(), first);
    let event_count = store.event_count().unwrap();
    assert_eq!(
        service
            .capture(
                &mut store,
                &local("git-fixture"),
                &local("capture-1"),
                revision,
                &first.intent.operation_id
            )
            .unwrap(),
        artifact
    );
    assert_eq!(event_count, store.event_count().unwrap());
    assert!(
        service
            .capture(
                &mut store,
                &local("git-fixture"),
                &local("capture-1"),
                revision,
                &second.intent.operation_id
            )
            .is_err()
    );
    assert!(
        service
            .capture(
                &mut store,
                &local("git-fixture"),
                &local("capture-1"),
                revision + 1,
                &first.intent.operation_id
            )
            .is_err()
    );
    let artifact_id = artifact.id.clone();
    let artifact_revision = artifact.revision.clone();
    drop(store);
    let child = ProcessCommand::new(env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "reopen_git_provenance_child",
            "--nocapture",
        ])
        .env("ORCHESTRIX_GIT_DB", &fixture.db)
        .env("ORCHESTRIX_GIT_ARTIFACT_ID", artifact_id)
        .env("ORCHESTRIX_GIT_ARTIFACT_REVISION", artifact_revision)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{} {}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    let mut store = fixture.store();
    assert_eq!(service.create(&mut store, &request1).unwrap(), first);
    // A committed capture replays its original immutable bytes after the filesystem changes.
    fs::write(work.join("source.txt"), "later changes\n").unwrap();
    assert_eq!(
        service
            .capture(
                &mut store,
                &local("git-fixture"),
                &local("capture-1"),
                revision,
                &first.intent.operation_id
            )
            .unwrap(),
        artifact
    );
    assert_eq!(
        store.load().unwrap().runs()[&RunId::new("run").unwrap()]
            .task(&task(1))
            .unwrap()
            .state(),
        TaskState::Assigned
    );
}

#[test]
#[ignore = "Invoked by parent with a private file database path"]
fn reopen_git_provenance_child() {
    let store = Store::open(env::var_os("ORCHESTRIX_GIT_DB").unwrap()).unwrap();
    let state = store.load().unwrap();
    let artifact = &state.captured_artifacts()[&(
        env::var("ORCHESTRIX_GIT_ARTIFACT_ID").unwrap(),
        env::var("ORCHESTRIX_GIT_ARTIFACT_REVISION").unwrap(),
    )];
    assert!(artifact.validate());
    assert_eq!(artifact.revision, artifact.content_revision());
    assert_eq!(
        artifact.base_oid,
        state.worktrees()[&artifact.operation_id].intent.base_oid
    );
    assert_eq!(state.worktrees().len(), 2);
    WorktreeService::new()
        .unwrap()
        .inspect(&store, &artifact.operation_id)
        .unwrap();
}

#[test]
fn git_effect_confirmation_failure_retries_owned_resources_without_duplicate() {
    for fault in [
        GitFaultPoint::AfterRefs,
        GitFaultPoint::AfterWorktree,
        GitFaultPoint::CreateConfirmationRollback,
    ] {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let service = WorktreeService::new().unwrap();
        let request = fixture.request(&mut store, 1);
        assert!(
            service
                .create_with_fault(&mut store, &request, fault)
                .is_err()
        );
        let pending = operation(&store);
        assert_eq!(pending.state, WorktreeState::Planned);
        drop(store);
        let mut store = fixture.store();
        let created = service.create(&mut store, &request).unwrap();
        assert_eq!(created.intent, pending.intent);
        assert_eq!(created.state, WorktreeState::Created);
        assert_eq!(store.load().unwrap().worktrees().len(), 1);
        assert_eq!(
            git_text(
                &fixture.main,
                &[
                    "for-each-ref",
                    "--format=%(refname)",
                    &format!("refs/heads/{}", created.intent.branch)
                ]
            )
            .lines()
            .count(),
            1
        );
        assert_eq!(service.create(&mut store, &request).unwrap(), created);
    }
}

#[test]
fn foreign_branch_path_and_ownership_collisions_are_preserved() {
    for path_collision in [false, true] {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let service = WorktreeService::new().unwrap();
        let request = fixture.request(&mut store, 1);
        assert_eq!(
            service.create_with_fault(&mut store, &request, GitFaultPoint::AfterIntent),
            Err(GitError::Injected)
        );
        let pending = operation(&store);
        if path_collision {
            fs::create_dir(&pending.intent.path).unwrap();
            fs::write(
                Path::new(&pending.intent.path).join("foreign.txt"),
                "foreign bytes",
            )
            .unwrap();
        } else {
            git(
                &fixture.main,
                &["branch", &pending.intent.branch, &fixture.base],
            );
        }
        assert!(service.create(&mut store, &request).is_err());
        assert_eq!(operation(&store).state, WorktreeState::Planned);
        if path_collision {
            assert_eq!(
                fs::read_to_string(Path::new(&pending.intent.path).join("foreign.txt")).unwrap(),
                "foreign bytes"
            );
        } else {
            assert_eq!(
                git_text(
                    &fixture.main,
                    &[
                        "rev-parse",
                        &format!("refs/heads/{}", pending.intent.branch)
                    ]
                ),
                fixture.base
            );
        }
    }
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    let mut process = git_command(&fixture.main, &["hash-object", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(b"foreign owner record")
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    let blob = String::from_utf8(output.stdout).unwrap();
    git(
        &fixture.main,
        &[
            "update-ref",
            &format!("refs/orchestrix/ownership/{}", owned.intent.operation_id),
            blob.trim(),
        ],
    );
    assert!(service.inspect(&store, &owned.intent.operation_id).is_err());
    let revision = current_revision(&store);
    assert!(
        service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("foreign-owner-remove"),
                revision,
                &owned.intent.operation_id
            )
            .is_err()
    );
    assert!(Path::new(&owned.intent.path).exists());
}

#[test]
fn capture_race_is_rejected_and_nothing_is_committed() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    let source = Path::new(&owned.intent.path).join("source.txt");
    fs::write(&source, "first change\n").unwrap();
    let before = store.event_count().unwrap();
    let revision = current_revision(&store);
    let result = service.capture_with_observer(
        &mut store,
        &local("git-fixture"),
        &local("race-capture"),
        revision,
        &owned.intent.operation_id,
        || {
            fs::write(source, "concurrent second change\n").unwrap();
            Ok(())
        },
    );
    assert_eq!(result, Err(GitError::InconsistentCapture));
    assert_eq!(store.event_count().unwrap(), before);
    assert!(store.load().unwrap().captured_artifacts().is_empty());
}

#[test]
fn conservative_cleanup_preserves_dirty_ignored_and_exclusive_commit_work() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    let root = Path::new(&owned.intent.path);
    fs::write(root.join("source.txt"), "not preserved\n").unwrap();
    let revision = current_revision(&store);
    assert!(
        service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("dirty-remove"),
                revision,
                &owned.intent.operation_id
            )
            .is_err()
    );
    assert!(root.exists());
    service
        .capture(
            &mut store,
            &local("git-fixture"),
            &local("save-before-remove"),
            revision,
            &owned.intent.operation_id,
        )
        .unwrap();
    let revision = current_revision(&store);
    assert!(
        service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("captured-dirty-remove"),
                revision,
                &owned.intent.operation_id
            )
            .is_err()
    );
    assert!(root.exists());
    fs::write(root.join("source.txt"), "base\n").unwrap();
    fs::create_dir(root.join("ignored")).unwrap();
    fs::write(root.join("ignored/output.txt"), "ignored unsaved work\n").unwrap();
    assert!(
        service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("ignored-remove"),
                revision,
                &owned.intent.operation_id
            )
            .is_err()
    );
    assert!(root.join("ignored/output.txt").exists());
    fs::remove_file(root.join("ignored/output.txt")).unwrap();
    fs::remove_dir(root.join("ignored")).unwrap();
    fs::write(root.join("source.txt"), "exclusive commit\n").unwrap();
    git(root, &["add", "source.txt"]);
    git(
        root,
        &["commit", "--quiet", "-m", "Exclusive fixture commit"],
    );
    assert!(git(root, &["status", "--porcelain"]).is_empty());
    assert!(
        service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("exclusive-remove"),
                revision,
                &owned.intent.operation_id
            )
            .is_err()
    );
    assert!(root.exists());
    assert_ne!(git_text(root, &["rev-parse", "HEAD"]), fixture.base);
}

#[test]
fn clean_cleanup_retains_base_anchor_and_recovers_confirmation_failure() {
    for fault in [
        GitFaultPoint::None,
        GitFaultPoint::AfterRemoval,
        GitFaultPoint::RemovalConfirmationRollback,
    ] {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let service = WorktreeService::new().unwrap();
        let request = fixture.request(&mut store, 1);
        let owned = service.create(&mut store, &request).unwrap();
        let revision = current_revision(&store);
        let result = service.remove_with_fault(
            &mut store,
            &local("git-fixture"),
            &local("remove-clean"),
            revision,
            &owned.intent.operation_id,
            fault,
        );
        if fault == GitFaultPoint::None {
            assert_eq!(result.unwrap().state, WorktreeState::Removed);
        } else {
            assert!(result.is_err());
            assert_eq!(operation(&store).state, WorktreeState::CleanupPlanned);
        }
        assert!(!Path::new(&owned.intent.path).exists());
        drop(store);
        let mut store = fixture.store();
        let removed = service
            .remove(
                &mut store,
                &local("git-fixture"),
                &local("remove-clean"),
                revision,
                &owned.intent.operation_id,
            )
            .unwrap();
        assert_eq!(removed.state, WorktreeState::Removed);
        assert!(
            service
                .remove(
                    &mut store,
                    &local("git-fixture"),
                    &local("remove-clean"),
                    revision + 1,
                    &owned.intent.operation_id
                )
                .is_err()
        );
        assert!(
            service
                .remove(
                    &mut store,
                    &local("git-fixture"),
                    &local("unrelated-remove-key"),
                    revision,
                    &owned.intent.operation_id
                )
                .is_err()
        );
        service.inspect(&store, &owned.intent.operation_id).unwrap();
        assert_eq!(
            git_text(
                &fixture.main,
                &[
                    "rev-parse",
                    &format!("refs/orchestrix/bases/{}", owned.intent.operation_id)
                ]
            ),
            fixture.base
        );
        assert!(
            !git_command(
                &fixture.main,
                &[
                    "show-ref",
                    "--verify",
                    &format!("refs/heads/{}", owned.intent.branch)
                ]
            )
            .output()
            .unwrap()
            .status
            .success()
        );
    }
}

#[test]
fn adverse_inherited_git_variables_do_not_redirect_service_operations() {
    let fixture = Fixture::new();
    let child = ProcessCommand::new(env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "adverse_git_environment_child",
            "--nocapture",
        ])
        .env("ORCHESTRIX_GIT_FIXTURE_ROOT", &fixture.root)
        .env("GIT_DIR", fixture.root.join("missing.git"))
        .env("GIT_WORK_TREE", fixture.root.join("wrong-tree"))
        .env("GIT_INDEX_FILE", fixture.root.join("wrong-index"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.worktree")
        .env("GIT_CONFIG_VALUE_0", fixture.root.join("wrong-config-tree"))
        .env("GIT_CONFIG", fixture.root.join("missing-config"))
        .env(
            "GIT_CONFIG_PARAMETERS",
            "'core.worktree'='wrong-parameter-tree'",
        )
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{} {}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(!fixture.root.join("wrong-index").exists());
    assert!(!fixture.root.join("wrong-tree").exists());
    assert!(!fixture.root.join("wrong-config-tree").exists());
}

#[test]
#[ignore = "Invoked in isolated child so Rust's parallel tests never mutate environment"]
fn adverse_git_environment_child() {
    let root = PathBuf::from(env::var_os("ORCHESTRIX_GIT_FIXTURE_ROOT").unwrap());
    let main = root.join("main repository acentua\u{e7}\u{e3}o");
    let fixture = std::mem::ManuallyDrop::new(Fixture {
        base: git_text(&main, &["rev-parse", "HEAD"]),
        db: root.join("state.sqlite"),
        authorized: root.join("authorized attempts"),
        main,
        root,
    });
    let mut store = fixture.store();
    let request = fixture.request(&mut store, 1);
    let service = WorktreeService::new().unwrap();
    let created = service.create(&mut store, &request).unwrap();
    assert_eq!(created.state, WorktreeState::Created);
    service
        .inspect(&store, &created.intent.operation_id)
        .unwrap();
}

#[test]
fn linked_source_cannot_place_attempts_in_common_git_metadata_or_another_checkout() {
    let fixture = Fixture::new();
    let linked = fixture.root.join("linked source");
    let linked_arg = linked
        .to_string_lossy()
        .strip_prefix("\\\\?\\")
        .unwrap_or(&linked.to_string_lossy())
        .to_owned();
    git(
        &fixture.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "fixture-linked-source",
            &linked_arg,
            &fixture.base,
        ],
    );
    let mut store = fixture.store();
    let mut request = fixture.request(&mut store, 1);
    request.repository_path = linked;
    let before = store.event_count().unwrap();
    let service = WorktreeService::new().unwrap();
    for root in [fixture.main.join(".git/worktrees"), fixture.main.clone()] {
        request.authorized_directory = root;
        assert_eq!(service.create(&mut store, &request), Err(GitError::Scope));
        assert!(store.load().unwrap().worktrees().is_empty());
        assert_eq!(store.event_count().unwrap(), before);
    }
    assert!(git(&fixture.main, &["status", "--porcelain"]).is_empty());
}

#[test]
fn cleanup_refuses_observed_active_or_uncertain_attempt_without_recording_removal_intent() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    for (sequence, kind, state) in [
        (1, "attempt.admission-observed", AttemptState::Admitted),
        (2, "attempt.start-observed", AttemptState::Running),
        (3, "transport-loss", AttemptState::Uncertain),
    ] {
        let action = if kind == "transport-loss" {
            RunAction::TransportLost {
                task: task(1),
                reason: "transport-loss".into(),
            }
        } else {
            RunAction::Observe{task:task(1),frame:json!({"version":{"major":1,"minor":1},"connectionId":"connection-1","connectionRevision":1,"sessionId":"session-1","attemptId":"attempt-1","sequence":sequence,"type":kind}).to_string(),origin:EventOrigin::Runtime}
        };
        execute(
            &mut store,
            &format!("synthetic-observation-{sequence}"),
            Command::run(RunId::new("run").unwrap(), action),
        );
        assert_eq!(
            store.load().unwrap().runs()[&RunId::new("run").unwrap()]
                .task(&task(1))
                .unwrap()
                .current_attempt()
                .unwrap()
                .state(),
            state
        );
        let revision = current_revision(&store);
        let events = store.event_count().unwrap();
        let key = local(&format!("blocked-removal-{sequence}"));
        assert_eq!(
            service.remove(
                &mut store,
                &local("git-fixture"),
                &key,
                revision,
                &owned.intent.operation_id
            ),
            Err(GitError::Pending)
        );
        assert_eq!(store.event_count().unwrap(), events);
        assert!(
            store
                .command_result(&local("git-fixture"), &key)
                .unwrap()
                .is_none()
        );
        assert_eq!(operation(&store).state, WorktreeState::Created);
        assert!(Path::new(&owned.intent.path).exists());
        assert_eq!(
            git_text(
                &fixture.main,
                &["rev-parse", &format!("refs/heads/{}", owned.intent.branch)]
            ),
            fixture.base
        );
    }
}

#[test]
fn clean_reset_to_base_preserves_work_retained_only_by_owned_reflogs() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    let work = Path::new(&owned.intent.path);
    fs::write(work.join("source.txt"), "unique reflog-only fixture work\n").unwrap();
    git(work, &["add", "source.txt"]);
    git(
        work,
        &["commit", "--quiet", "-m", "Fixture work retained by reflog"],
    );
    let unique_commit = git_text(work, &["rev-parse", "HEAD"]);
    assert_ne!(unique_commit, fixture.base);
    // This reset prepares only the disposable test fixture. The production
    // worktree service never resets user work or uses reset for recovery.
    git(work, &["reset", "--hard", &fixture.base]);
    assert!(git(work, &["status", "--porcelain"]).is_empty());
    assert_eq!(git_text(work, &["rev-parse", "HEAD"]), fixture.base);
    let branch_log = fixture
        .main
        .join(".git/logs/refs/heads")
        .join(&owned.intent.branch);
    let admin = PathBuf::from(git_text(work, &["rev-parse", "--absolute-git-dir"]));
    let head_log = admin.join("logs/HEAD");
    let branch_before = fs::read(&branch_log).unwrap();
    let head_before = fs::read(&head_log).unwrap();
    assert!(String::from_utf8_lossy(&branch_before).contains(&unique_commit));
    assert!(String::from_utf8_lossy(&head_before).contains(&unique_commit));
    let revision = current_revision(&store);
    let events = store.event_count().unwrap();
    assert_eq!(
        service.remove(
            &mut store,
            &local("git-fixture"),
            &local("reflog-only-remove"),
            revision,
            &owned.intent.operation_id
        ),
        Err(GitError::UnpreservedWork)
    );
    assert!(work.exists());
    assert_eq!(fs::read(branch_log).unwrap(), branch_before);
    assert_eq!(fs::read(head_log).unwrap(), head_before);
    assert_eq!(git_text(work, &["rev-parse", "HEAD"]), fixture.base);
    assert_eq!(
        git_text(
            &fixture.main,
            &["rev-parse", &format!("refs/heads/{}", owned.intent.branch)]
        ),
        fixture.base
    );
    assert_eq!(store.event_count().unwrap(), events);
    assert_eq!(operation(&store).state, WorktreeState::Created);
}

#[test]
fn removal_recovery_preserves_unique_branch_reflog_work_after_worktree_is_gone() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let service = WorktreeService::new().unwrap();
    let request = fixture.request(&mut store, 1);
    let owned = service.create(&mut store, &request).unwrap();
    let revision = current_revision(&store);
    assert_eq!(
        service.remove_with_fault(
            &mut store,
            &local("git-fixture"),
            &local("interrupted-remove"),
            revision,
            &owned.intent.operation_id,
            GitFaultPoint::AfterWorktreeRemoval
        ),
        Err(GitError::Injected)
    );
    assert!(!Path::new(&owned.intent.path).exists());
    assert_eq!(operation(&store).state, WorktreeState::CleanupPlanned);
    let branch = format!("refs/heads/{}", owned.intent.branch);
    assert_eq!(
        git_text(&fixture.main, &["rev-parse", &branch]),
        fixture.base
    );
    let main_head = git(&fixture.main, &["rev-parse", "HEAD"]);
    let main_index = fs::read(fixture.main.join(".git/index")).unwrap();
    // Fixture-only object/ref preparation: no checkout, HEAD or index change.
    // The production removal path must preserve this later reflog-only work.
    let tree = format!("{}^{{tree}}", fixture.base);
    let unique = git_text(
        &fixture.main,
        &[
            "commit-tree",
            &tree,
            "-p",
            &fixture.base,
            "-m",
            "Unique removal-recovery fixture commit",
        ],
    );
    assert_ne!(unique, fixture.base);
    git(
        &fixture.main,
        &["update-ref", &branch, &unique, &fixture.base],
    );
    git(
        &fixture.main,
        &["update-ref", &branch, &fixture.base, &unique],
    );
    let branch_log = fixture
        .main
        .join(".git/logs/refs/heads")
        .join(&owned.intent.branch);
    let log_before = fs::read(&branch_log).unwrap();
    assert!(String::from_utf8_lossy(&log_before).contains(&unique));
    assert_eq!(git(&fixture.main, &["rev-parse", "HEAD"]), main_head);
    assert_eq!(
        fs::read(fixture.main.join(".git/index")).unwrap(),
        main_index
    );
    let events = store.event_count().unwrap();
    drop(store);
    let mut store = fixture.store();
    assert_eq!(
        service.remove(
            &mut store,
            &local("git-fixture"),
            &local("interrupted-remove"),
            revision,
            &owned.intent.operation_id
        ),
        Err(GitError::UnpreservedWork)
    );
    assert!(!Path::new(&owned.intent.path).exists());
    assert_eq!(
        git_text(&fixture.main, &["rev-parse", &branch]),
        fixture.base
    );
    assert_eq!(fs::read(branch_log).unwrap(), log_before);
    assert_eq!(operation(&store).state, WorktreeState::CleanupPlanned);
    assert_eq!(store.event_count().unwrap(), events);
}
