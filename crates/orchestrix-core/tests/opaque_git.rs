//! Opt-in mixed bases use real owned Git repositories, without provider calls.
use orchestrix_core::{
    domain::*,
    git::{self, CaptureLimits, GitError, GitExecutor},
    git_records::{WorktreeIntent, content_hash},
    persistence::{
        AttemptSnapshot, Command as StoredCommand, Disposition, Request, RunAction, Store,
    },
    persistence_records::{
        DefinitionInput, DefinitionKind, DefinitionRef, RecordsCommand, SnapshotInput,
    },
    runtime::CancellationToken,
    runtime_contract::{normalize_descriptor, normalize_request},
};
use serde_json::json;
use std::{
    env, fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    main: PathBuf,
    authorized: PathBuf,
    base: String,
}
impl Fixture {
    fn new() -> Self {
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        let root = temp.join(format!(
            "orchestrix-opaque-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let main = root.join("main repository");
        let authorized = root.join("attempts");
        fs::create_dir(&main).unwrap();
        fs::create_dir(&authorized).unwrap();
        git_cmd(&main, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(main.join("source.txt"), b"original editable text\n").unwrap();
        fs::write(main.join("mixed.bin"), [0, 1, 2, 255, 254, 3]).unwrap();
        fs::write(main.join("large.txt"), vec![b'x'; 5 * 1_048_576]).unwrap();
        fs::write(
            main.join("pattern-fixture.txt"),
            b"API_KEY=fixture-only-secret-canary\n",
        )
        .unwrap();
        git_cmd(&main, &["add", "--all"]);
        git_cmd(&main, &["commit", "--quiet", "-m", "Owned opaque fixture"]);
        let base = git_text(&main, &["rev-parse", "HEAD"]);
        Self {
            root,
            main,
            authorized,
            base,
        }
    }
    fn paths() -> Vec<String> {
        vec!["added.txt".into(), "source.txt".into()]
    }
    fn intent(&self) -> WorktreeIntent {
        let manifest = git::plan_base_preservation(
            &self.main,
            &self.base,
            &Self::paths(),
            &CancellationToken::new(),
        )
        .unwrap();
        let target = self.authorized.join("owned");
        let target_arg = target
            .to_str()
            .unwrap()
            .strip_prefix("\\\\?\\")
            .unwrap_or(target.to_str().unwrap());
        git_cmd(
            &self.main,
            &[
                "worktree",
                "add",
                "--quiet",
                "--no-checkout",
                "--lock",
                "-b",
                "orchestrix/opaque-fixture",
                target_arg,
                &self.base,
            ],
        );
        for entry in &manifest.entries {
            let path = target.join(&entry.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, fs::read(self.main.join(&entry.path)).unwrap()).unwrap();
        }
        git_cmd(&target, &["read-tree", &self.base]);
        let text = |path: &Path| fs::canonicalize(path).unwrap().to_str().unwrap().to_owned();
        WorktreeIntent {
            preservation: Some(manifest),
            operation_id: "opaque-owned".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: self.base.clone(),
            domain_base: "base".into(),
            worktree_ref: "owned-reference".into(),
            authorized_root: text(&self.authorized),
            path: text(&target),
            main_repository: text(&self.main),
            branch: "orchestrix/opaque-fixture".into(),
            common_git_dir: text(&self.main.join(".git")),
            ownership_token: "a".repeat(64),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.root).unwrap();
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        assert!(
            root.parent() == Some(temp.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchestrix-opaque-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
fn git_output(root: &Path, args: &[&str]) -> std::process::Output {
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
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_ATTR_NOSYSTEM", "1")
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
            "-c",
            "protocol.allow=never",
            "-c",
            "gc.auto=0",
        ])
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command.output().unwrap()
}
fn git_cmd(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = git_output(root, args);
    assert!(
        output.status.success(),
        "fixture Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn git_text(root: &Path, args: &[&str]) -> String {
    String::from_utf8(git_cmd(root, args))
        .unwrap()
        .trim()
        .into()
}

fn fixture_loose_object(fixture: &Fixture, oid: &str) -> PathBuf {
    assert_eq!(oid.len(), 40);
    assert!(oid.bytes().all(|byte| byte.is_ascii_hexdigit()));
    fixture
        .main
        .join(".git/objects")
        .join(&oid[..2])
        .join(&oid[2..])
}

fn writable_owned_loose_object(fixture: &Fixture, path: &Path) {
    assert!(path.starts_with(fixture.main.join(".git/objects")));
    assert!(path.is_file());
    #[cfg(windows)]
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "Windows-only owned loose-object fixture; never a developer repository"
    )]
    {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn store_valid_native_fixture_object(fixture: &Fixture, kind: &str, body: &[u8]) -> String {
    assert!(matches!(kind, "blob" | "commit" | "tree"));
    let payload = fixture.root.join(format!("valid-{kind}-payload"));
    fs::write(&payload, body).unwrap();
    let payload = payload.to_str().unwrap();
    let payload = payload.strip_prefix("\\\\?\\").unwrap_or(payload);
    let oid = git_text(
        &fixture.main,
        &["hash-object", "-w", "-t", kind, "--", payload],
    );
    assert_eq!(git_cmd(&fixture.main, &["cat-file", kind, &oid]), body);
    assert!(fixture_loose_object(fixture, &oid).is_file());
    oid
}

fn working_hashes(
    root: &Path,
    manifest: &orchestrix_core::git_records::BasePreservation,
) -> Vec<(String, String)> {
    manifest
        .entries
        .iter()
        .map(|entry| {
            (
                entry.path.clone(),
                content_hash(&fs::read(root.join(&entry.path)).unwrap()),
            )
        })
        .collect()
}

fn assert_native_identity_mismatch_is_rejected(kind: &str) {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let manifest = intent.preservation.as_ref().unwrap();
    let cancel = CancellationToken::new();
    let positive = git::inspect_repository_with_editable(
        &fixture.main,
        Some(&fixture.base),
        &Fixture::paths(),
        &cancel,
    )
    .unwrap();
    assert!(positive.compatible && positive.application_eligible);
    let expected_oid = match kind {
        "blob" => manifest
            .entries
            .iter()
            .find(|entry| entry.path == "mixed.bin")
            .unwrap()
            .git_oid
            .clone(),
        "commit" => fixture.base.clone(),
        "tree" => manifest.tree_oid.clone(),
        _ => panic!("explicit native object fixture kind"),
    };
    let mut changed_body = git_cmd(&fixture.main, &["cat-file", kind, &expected_oid]);
    match kind {
        "blob" => changed_body[2] ^= 0x40,
        "commit" => {
            let message = changed_body
                .windows(2)
                .position(|bytes| bytes == b"\n\n")
                .unwrap()
                + 2;
            changed_body[message] = b'P';
        }
        "tree" => {
            let name = changed_body
                .windows(b"mixed.bin\0".len())
                .position(|bytes| bytes == b"mixed.bin\0")
                .unwrap();
            // Still a structurally valid, sorted tree with the same modes,
            // OIDs and byte length. Only its native identity is incompatible.
            changed_body[name] = b'n';
        }
        _ => unreachable!(),
    }
    let other_oid = store_valid_native_fixture_object(&fixture, kind, &changed_body);
    assert_ne!(other_oid, expected_oid);
    let expected_object = fixture_loose_object(&fixture, &expected_oid);
    let original_object = fs::read(&expected_object).unwrap();
    let other_object = fixture_loose_object(&fixture, &other_oid);

    // Keep an independently valid HEAD. A rejected explicit base must never be
    // replaced by this commit or by the intact main/attempt working bytes.
    fs::write(fixture.main.join("mixed.bin"), [0, 1, 9, 255, 254, 3]).unwrap();
    git_cmd(&fixture.main, &["add", "--", "mixed.bin"]);
    git_cmd(
        &fixture.main,
        &["commit", "--quiet", "-m", "Independent valid HEAD fixture"],
    );
    let healthy_head = git_text(&fixture.main, &["rev-parse", "HEAD"]);
    assert_ne!(healthy_head, fixture.base);
    assert_ne!(
        git_text(&fixture.main, &["rev-parse", "HEAD^{tree}"]),
        manifest.tree_oid
    );
    let root = Path::new(&intent.path);
    let non_head = git::inspect_repository_with_editable(
        &fixture.main,
        Some(&fixture.base),
        &Fixture::paths(),
        &cancel,
    )
    .unwrap();
    assert!(!non_head.compatible && !non_head.application_eligible);
    assert!(
        non_head
            .blockers
            .iter()
            .any(|blocker| blocker.code == "base-not-head")
    );
    let main_before = working_hashes(&fixture.main, manifest);
    let attempt_before = working_hashes(root, manifest);
    let main_index = fs::read(fixture.main.join(".git/index")).unwrap();
    let attempt_index_path = PathBuf::from(git_text(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    ));
    let attempt_index = fs::read(&attempt_index_path).unwrap();
    let references = git_cmd(&fixture.main, &["show-ref"]);
    let db = fixture.root.join("identity-rejection.sqlite");
    let mut store = Store::open(&db).unwrap();
    seed_attempt(&mut store, &fixture.base);
    let revision = store.load().unwrap().revision();
    let request = git::CreateWorktree {
        scope: local("identity-rejection"),
        key: local("create"),
        expected_revision: revision,
        project: ProjectId::new("project").unwrap(),
        repository: RepositoryId::new("repository").unwrap(),
        run: RunId::new("run").unwrap(),
        task: TaskId::new("opaque-task").unwrap(),
        attempt: AttemptId::new("opaque-attempt").unwrap(),
        directory_reference: local("repository-grant"),
        worktree_reference: local("opaque-worktree"),
        repository_path: fixture.main.clone(),
        authorized_directory: fixture.authorized.clone(),
        base_commit: fixture.base.clone(),
    };
    writable_owned_loose_object(&fixture, &expected_object);
    fs::copy(&other_object, &expected_object).unwrap();
    assert_eq!(
        fs::read(&expected_object).unwrap(),
        fs::read(&other_object).unwrap()
    );
    // Git can read this valid zlib/header/body under the wrong filename. Its
    // parser succeeding does not prove the native object identity.
    assert_eq!(
        git_cmd(&fixture.main, &["cat-file", kind, &expected_oid]),
        changed_body
    );
    let expected = if kind == "tree" {
        let observed = git_output(
            &fixture.main,
            &["ls-tree", "-r", "-l", "-z", "--full-tree", &fixture.base],
        );
        assert!(observed.stdout.len() <= 65_536);
        assert!(observed.stderr.len() <= 8192);
        let diagnostic = json!({
            "stage":"native-ls-tree-after-wrong-native-oid",
            "success":observed.status.success(),
            "status":observed.status.code(),
            "stdoutBytes":observed.stdout.len(),
            "stdoutSha256":content_hash(&observed.stdout),
            "stderr":String::from_utf8_lossy(&observed.stderr),
            "base":fixture.base,
            "expectedObject":expected_oid,
            "differentValidObject":other_oid,
        });
        eprintln!("OPAQUE_NATIVE_TREE_DIAGNOSTIC={diagnostic}");
        fs::write(
            fixture.root.join("native-tree-stage.json"),
            serde_json::to_vec(&diagnostic).unwrap(),
        )
        .unwrap();
        assert_eq!(observed.status.code(), Some(128));
        assert!(observed.stdout.is_empty());
        let stderr = std::str::from_utf8(&observed.stderr).unwrap();
        assert!(stderr.contains(&format!("error: hash mismatch {expected_oid}")));
        assert!(stderr.contains("fatal: not a tree object"));
        // Native ls-tree checks this tree's identity before the Core can
        // reconstruct its inventory. Only this demonstrated native refusal is
        // classified as Process; arbitrary transport/process failure is not a
        // passing substitute for a native integrity check.
        GitError::Process
    } else {
        GitError::InconsistentCapture
    };
    let (expected_preflight, expected_create) = if kind == "commit" {
        let base_expression = format!("{}^{{commit}}", fixture.base);
        for (stage, directory, arguments) in [
            (
                "preflight-head",
                root,
                vec!["rev-parse", "--verify", "HEAD^{commit}"],
            ),
            (
                "create-base",
                fixture.main.as_path(),
                vec![
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &base_expression,
                ],
            ),
        ] {
            let observed = git_output(directory, &arguments);
            assert!(observed.stdout.len() <= 256);
            assert!(observed.stderr.len() <= 8192);
            let diagnostic = json!({
                "stage":stage,
                "arguments":arguments,
                "success":observed.status.success(),
                "status":observed.status.code(),
                "stdoutBytes":observed.stdout.len(),
                "stdoutSha256":content_hash(&observed.stdout),
                "stderr":String::from_utf8_lossy(&observed.stderr),
                "base":fixture.base,
                "expectedObject":expected_oid,
                "differentValidObject":other_oid,
            });
            eprintln!("OPAQUE_NATIVE_COMMIT_DIAGNOSTIC={diagnostic}");
            fs::write(
                fixture.root.join(format!("native-{stage}-stage.json")),
                serde_json::to_vec(&diagnostic).unwrap(),
            )
            .unwrap();
            assert_eq!(observed.status.code(), Some(128));
            let stderr = std::str::from_utf8(&observed.stderr).unwrap();
            assert!(stderr.contains(&format!("error: hash mismatch {expected_oid}")));
        }
        // These exact native revision resolvers precede raw commit validation
        // in preflight/create. Plan and capture still require the Core's typed
        // InconsistentCapture; Process is specific to the proven two stages.
        (GitError::Process, GitError::Process)
    } else {
        (expected.clone(), expected.clone())
    };
    assert_eq!(
        git::plan_base_preservation(&fixture.main, &fixture.base, &Fixture::paths(), &cancel),
        Err(expected.clone())
    );
    assert_eq!(
        git::inspect_repository_with_editable(
            root,
            Some(&fixture.base),
            &Fixture::paths(),
            &cancel,
        )
        .unwrap_err(),
        expected_preflight
    );
    let executor = GitExecutor::new().unwrap();
    assert_eq!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(expected.clone())
    );
    let service = git::WorktreeService::new().unwrap();
    assert_eq!(
        service
            .create_with_preservation(&mut store, &request, &Fixture::paths())
            .unwrap_err(),
        expected_create
    );
    assert!(store.load().unwrap().worktrees().is_empty());
    assert_eq!(store.load().unwrap().revision(), revision);
    assert_eq!(fs::read_dir(&fixture.authorized).unwrap().count(), 1);
    assert_eq!(working_hashes(&fixture.main, manifest), main_before);
    assert_eq!(working_hashes(root, manifest), attempt_before);
    assert_eq!(
        fs::read(fixture.main.join(".git/index")).unwrap(),
        main_index
    );
    assert_eq!(fs::read(&attempt_index_path).unwrap(), attempt_index);
    assert_eq!(git_cmd(&fixture.main, &["show-ref"]), references);
    assert_eq!(
        git_text(&fixture.main, &["rev-parse", "HEAD"]),
        healthy_head
    );
    assert_eq!(
        git::plan_base_preservation(&fixture.main, &healthy_head, &Fixture::paths(), &cancel)
            .unwrap()
            .base_oid,
        healthy_head
    );

    writable_owned_loose_object(&fixture, &expected_object);
    fs::write(&expected_object, original_object).unwrap();
    assert_eq!(
        git::plan_base_preservation(&fixture.main, &fixture.base, &Fixture::paths(), &cancel)
            .unwrap(),
        *manifest
    );
    assert!(
        git::capture(&executor, root, &intent, CaptureLimits::default())
            .unwrap()
            .files
            .is_empty()
    );
    assert_eq!(working_hashes(&fixture.main, manifest), main_before);
    assert_eq!(working_hashes(root, manifest), attempt_before);
}

#[test]
fn native_oid_valid_zlib_wrong_blob_body_is_rejected_without_head_fallback() {
    assert_native_identity_mismatch_is_rejected("blob");
}

#[test]
fn native_oid_valid_zlib_wrong_commit_body_is_rejected_without_head_fallback() {
    assert_native_identity_mismatch_is_rejected("commit");
}

#[test]
fn native_oid_valid_zlib_wrong_tree_body_is_rejected_without_head_fallback() {
    assert_native_identity_mismatch_is_rejected("tree");
}

#[test]
fn native_oid_empty_tree_directory_outside_recursive_inventory_is_explicitly_rejected() {
    let fixture = Fixture::new();
    let cancel = CancellationToken::new();
    let baseline =
        git::plan_base_preservation(&fixture.main, &fixture.base, &Fixture::paths(), &cancel)
            .unwrap();
    let index = fs::read(fixture.main.join(".git/index")).unwrap();
    let checkout = working_hashes(&fixture.main, &baseline);
    let references = git_cmd(&fixture.main, &["show-ref"]);
    let empty = store_valid_native_fixture_object(&fixture, "tree", b"");
    let original_tree = git_cmd(&fixture.main, &["cat-file", "tree", &baseline.tree_oid]);
    let mut tree = b"40000 empty\0".to_vec();
    for pair in empty.as_bytes().as_chunks::<2>().0 {
        tree.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    tree.extend(original_tree);
    let tree_oid = store_valid_native_fixture_object(&fixture, "tree", &tree);
    let commit = String::from_utf8(git_cmd(
        &fixture.main,
        &["cat-file", "commit", &fixture.base],
    ))
    .unwrap()
    .replacen(&baseline.tree_oid, &tree_oid, 1);
    let base = store_valid_native_fixture_object(&fixture, "commit", commit.as_bytes());
    assert_ne!(base, fixture.base);
    // Recursive ls-tree omits an empty directory. Native root-tree validation
    // must reject the incomplete flat projection rather than invent support.
    assert_eq!(
        git_cmd(&fixture.main, &["ls-tree", "-r", "-z", &base]),
        git_cmd(&fixture.main, &["ls-tree", "-r", "-z", &fixture.base])
    );
    assert_eq!(
        git::plan_base_preservation(&fixture.main, &base, &Fixture::paths(), &cancel),
        Err(GitError::InconsistentCapture)
    );
    let non_head = git::inspect_repository_with_editable(
        &fixture.main,
        Some(&base),
        &Fixture::paths(),
        &cancel,
    )
    .unwrap();
    assert!(!non_head.compatible && !non_head.application_eligible);
    assert!(
        non_head
            .blockers
            .iter()
            .any(|blocker| blocker.code == "base-not-head")
    );
    // Prepare only this fixture's ref so the real native-tree check is reached
    // with HEAD=base. Keep checkout/index untouched and restore the exact ref
    // by CAS before asserting the result, including an unexpected error.
    git_cmd(
        &fixture.main,
        &["update-ref", "refs/heads/main", &base, &fixture.base],
    );
    let inspected = git::inspect_repository_with_editable(
        &fixture.main,
        Some(&base),
        &Fixture::paths(),
        &cancel,
    );
    git_cmd(
        &fixture.main,
        &["update-ref", "refs/heads/main", &fixture.base, &base],
    );
    assert!(matches!(inspected, Err(GitError::InconsistentCapture)));
    assert_eq!(fs::read(fixture.main.join(".git/index")).unwrap(), index);
    assert_eq!(working_hashes(&fixture.main, &baseline), checkout);
    assert_eq!(git_cmd(&fixture.main, &["show-ref"]), references);
    assert_eq!(fs::read_dir(&fixture.authorized).unwrap().count(), 0);
}

#[test]
fn native_oid_valid_blob_changed_during_capture_is_rejected_and_recoverable() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let manifest = intent.preservation.as_ref().unwrap();
    let root = Path::new(&intent.path);
    let executor = GitExecutor::new().unwrap();
    let baseline = git::capture(&executor, root, &intent, CaptureLimits::default()).unwrap();
    assert!(baseline.files.is_empty());
    let baseline_bytes = serde_json::to_vec(&baseline).unwrap();
    let manifest_bytes = serde_json::to_vec(manifest).unwrap();
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.path == "mixed.bin")
        .unwrap();
    let expected_object = fixture_loose_object(&fixture, &entry.git_oid);
    let original_object = fs::read(&expected_object).unwrap();
    let mut changed_body = git_cmd(&fixture.main, &["cat-file", "blob", &entry.git_oid]);
    changed_body[2] ^= 0x40;
    let different_oid = store_valid_native_fixture_object(&fixture, "blob", &changed_body);
    assert_ne!(different_oid, entry.git_oid);
    let different_object = fixture_loose_object(&fixture, &different_oid);
    let wrong_object_bytes = fs::read(&different_object).unwrap();
    let main_before = working_hashes(&fixture.main, manifest);
    let attempt_before = working_hashes(root, manifest);
    let main_index = fs::read(fixture.main.join(".git/index")).unwrap();
    let attempt_index_path = PathBuf::from(git_text(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    ));
    let attempt_index = fs::read(&attempt_index_path).unwrap();
    let references = git_cmd(&fixture.main, &["show-ref"]);
    let mut observer_reached = false;
    let result =
        git::capture_with_observer(&executor, root, &intent, CaptureLimits::default(), || {
            // This hook runs after native base verification AND the first
            // complete working observation. Alter only the owned loose object;
            // working bytes, size and timestamps cannot reveal this mutation.
            observer_reached = true;
            writable_owned_loose_object(&fixture, &expected_object);
            fs::copy(&different_object, &expected_object)?;
            Ok(())
        });
    assert!(
        observer_reached,
        "initial complete observation must be reached"
    );
    assert_eq!(fs::read(&expected_object).unwrap(), wrong_object_bytes);
    let native = git_output(&fixture.main, &["cat-file", "blob", &entry.git_oid]);
    assert!(native.stdout.len() <= 65_536 && native.stderr.len() <= 8192);
    let diagnostic = json!({
        "stage":"blob-changed-after-first-capture-observation",
        "nativeStatus":native.status.code(),
        "nativeStdoutSha256":content_hash(&native.stdout),
        "nativeStderr":String::from_utf8_lossy(&native.stderr),
        "expectedObject":entry.git_oid,
        "differentValidObject":different_oid,
        "coreResult":result.as_ref().err().map(ToString::to_string),
    });
    eprintln!("OPAQUE_NATIVE_CAPTURE_INTERVAL_DIAGNOSTIC={diagnostic}");
    fs::write(
        fixture.root.join("native-capture-interval.json"),
        serde_json::to_vec(&diagnostic).unwrap(),
    )
    .unwrap();
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(native.stdout, changed_body);
    assert!(native.stderr.is_empty());
    assert_eq!(working_hashes(&fixture.main, manifest), main_before);
    assert_eq!(working_hashes(root, manifest), attempt_before);
    assert_eq!(
        fs::read(fixture.main.join(".git/index")).unwrap(),
        main_index
    );
    assert_eq!(fs::read(&attempt_index_path).unwrap(), attempt_index);
    assert_eq!(git_cmd(&fixture.main, &["show-ref"]), references);
    assert_eq!(serde_json::to_vec(manifest).unwrap(), manifest_bytes);
    assert_eq!(serde_json::to_vec(&baseline).unwrap(), baseline_bytes);

    // Restore explicitly in the test; capture must neither repair the object
    // nor return a partial/successful artifact from the interval above.
    writable_owned_loose_object(&fixture, &expected_object);
    fs::write(&expected_object, &original_object).unwrap();
    assert_eq!(result, Err(GitError::InconsistentCapture));
    assert_eq!(
        git::capture(&executor, root, &intent, CaptureLimits::default()).unwrap(),
        baseline
    );
    assert_eq!(fs::read(&expected_object).unwrap(), original_object);
    assert_eq!(working_hashes(root, manifest), attempt_before);
    assert_eq!(fs::read(&attempt_index_path).unwrap(), attempt_index);
    assert_eq!(git_cmd(&fixture.main, &["show-ref"]), references);
}
#[test]
fn mixed_base_manifest_and_preflight_keep_legacy_blocked_and_require_explicit_selection() {
    let fixture = Fixture::new();
    let cancel = CancellationToken::new();
    let legacy = git::inspect_repository(&fixture.main, None, &cancel).unwrap();
    assert!(!legacy.compatible);
    assert!(legacy.preservation.is_none());
    let empty = git::inspect_repository_with_editable(&fixture.main, None, &[], &cancel).unwrap();
    assert_eq!(empty.schema_version, 2);
    assert!(!empty.compatible);
    assert!(empty.preservation.unwrap().selection_required);
    let report =
        git::inspect_repository_with_editable(&fixture.main, None, &Fixture::paths(), &cancel)
            .unwrap();
    assert!(report.compatible && report.complete && report.application_eligible);
    assert_eq!(report.profile, "opaque-base-v2");
    assert!(!report.model_called);
    let summary = report.preservation.unwrap();
    assert_eq!(summary.base_files, 4);
    assert_eq!(summary.preserved_files, 3);
    assert!(summary.preserved_bytes > 5 * 1_048_576);
    assert_eq!(summary.editable_paths, Fixture::paths());
    let manifest =
        git::plan_base_preservation(&fixture.main, &fixture.base, &Fixture::paths(), &cancel)
            .unwrap();
    assert!(manifest.validate());
    assert_eq!(manifest.entries.len(), 4);
    assert!(manifest.entries.iter().all(|e| e.sha256.len() == 64));
    let wire = serde_json::to_string(&manifest).unwrap();
    assert!(!wire.contains("fixture-only-secret-canary"));
    assert!(!wire.contains(&"x".repeat(256)));
    for path in ["mixed.bin", "pattern-fixture.txt", "large.txt"] {
        let blocked =
            git::inspect_repository_with_editable(&fixture.main, None, &[path.into()], &cancel)
                .unwrap();
        assert!(!blocked.compatible && !blocked.application_eligible);
    }
}
#[test]
fn captured_edits_preserve_full_base_and_only_explicit_text_enters_artifact() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let root = Path::new(&intent.path);
    let executor = GitExecutor::new().unwrap();
    let began = Instant::now();
    let clean = git::capture(&executor, root, &intent, CaptureLimits::default()).unwrap();
    eprintln!(
        "opaque_timing phase=clean_capture elapsed_ms={} base_files=4",
        began.elapsed().as_millis()
    );
    assert!(clean.files.is_empty());
    assert_eq!(clean.preservation, intent.preservation);
    fs::write(root.join("source.txt"), b"verified editable change\n").unwrap();
    fs::write(root.join("added.txt"), b"explicit added file\n").unwrap();
    let began = Instant::now();
    let artifact = git::capture(&executor, root, &intent, CaptureLimits::default()).unwrap();
    eprintln!(
        "opaque_timing phase=changed_capture elapsed_ms={} base_files=4 changed_files=2",
        began.elapsed().as_millis()
    );
    assert!(artifact.validate());
    assert_eq!(artifact.files.len(), 2);
    assert_eq!(artifact.preservation, intent.preservation);
    assert!(artifact.patch.contains("verified editable change"));
    let wire = serde_json::to_string(&artifact).unwrap();
    assert!(!wire.contains("fixture-only-secret-canary"));
    assert!(!wire.contains(&"x".repeat(256)));
    for name in ["mixed.bin", "large.txt", "pattern-fixture.txt"] {
        assert_eq!(
            content_hash(&fs::read(root.join(name)).unwrap()),
            content_hash(&fs::read(fixture.main.join(name)).unwrap())
        );
    }
    assert_eq!(
        fs::read(fixture.main.join("source.txt")).unwrap(),
        b"original editable text\n"
    );
}
#[test]
fn same_size_opaque_edit_staged_edit_and_unlisted_creation_are_rejected() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let root = Path::new(&intent.path);
    let executor = GitExecutor::new().unwrap();
    for name in ["mixed.bin", "large.txt", "pattern-fixture.txt"] {
        let original = fs::read(root.join(name)).unwrap();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(root.join(name))
            .unwrap();
        file.write_all(&[original[0] ^ 1]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(matches!(
            git::capture(&executor, root, &intent, CaptureLimits::default()),
            Err(GitError::UnpreservedWork)
        ));
        fs::write(root.join(name), &original).unwrap();
    }
    fs::write(root.join("outside.txt"), b"not authorized\n").unwrap();
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::Scope)
    ));
    fs::remove_file(root.join("outside.txt")).unwrap();
    fs::write(root.join("mixed.bin"), b"staged unauthorized edit\n").unwrap();
    git_cmd(root, &["add", "--", "mixed.bin"]);
    fs::write(
        root.join("mixed.bin"),
        fs::read(fixture.main.join("mixed.bin")).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::UnpreservedWork)
    ));
}
#[test]
fn opaque_hash_is_checked_again_after_capture_interval_and_size_budget_is_separate() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let root = Path::new(&intent.path);
    let executor = GitExecutor::new().unwrap();
    let result =
        git::capture_with_observer(&executor, root, &intent, CaptureLimits::default(), || {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(root.join("large.txt"))?;
            file.seek(SeekFrom::Start(10))?;
            file.write_all(b"y")?;
            file.sync_all()?;
            Ok(())
        });
    assert!(matches!(result, Err(GitError::UnpreservedWork)));
    fs::write(
        fixture.main.join("too-large.bin"),
        vec![0; 6 * 1_048_576 + 1],
    )
    .unwrap();
    git_cmd(&fixture.main, &["add", "--", "too-large.bin"]);
    git_cmd(
        &fixture.main,
        &["commit", "--quiet", "-m", "Oversized own fixture"],
    );
    let head = git_text(&fixture.main, &["rev-parse", "HEAD"]);
    assert!(matches!(
        git::plan_base_preservation(
            &fixture.main,
            &head,
            &Fixture::paths(),
            &CancellationToken::new()
        ),
        Err(GitError::Limit)
    ));
}

fn local(value: &str) -> LocalRef {
    LocalRef::new(value).unwrap()
}
fn store_command(store: &mut Store, key: &str, command: StoredCommand) {
    let expected_revision = store.load().unwrap().revision();
    assert_eq!(
        store
            .execute(&Request {
                scope: local("opaque-seed"),
                key: local(key),
                expected_revision,
                command
            })
            .unwrap()
            .result
            .disposition,
        Disposition::Applied
    );
}
fn seed_attempt(store: &mut Store, base: &str) {
    let project = Project::new(
        ProjectId::new("project").unwrap(),
        Repository::new(
            RepositoryId::new("repository").unwrap(),
            local("repository-grant"),
        ),
    );
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap();
    let connection =
        Connection::new(normalize_descriptor(&fixture["descriptors"]["base"]).unwrap()).unwrap();
    store_command(store, "project", StoredCommand::AddProject(project.clone()));
    store_command(
        store,
        "connection",
        StoredCommand::AddConnection(connection.clone()),
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
        store_command(
            store,
            &format!("definition-{id}"),
            StoredCommand::Records(RecordsCommand::AddDefinition(DefinitionInput {
                kind,
                id: id.into(),
                version: 1,
                body,
                references: vec![],
            })),
        );
    }
    store_command(
        store,
        "run",
        StoredCommand::create_run(
            Run::new(
                RunId::new("run").unwrap(),
                Some(&project),
                RevisionId::new(base).unwrap(),
                DeliveryRequirement::FinalApplication,
            )
            .unwrap(),
        ),
    );
    let session = RuntimeSessionRef::new(
        RuntimeSessionId::new("opaque-session").unwrap(),
        &connection,
    );
    store_command(
        store,
        "session",
        StoredCommand::RegisterSession(session.clone()),
    );
    let task = TaskId::new("opaque-task").unwrap();
    let attempt = AttemptId::new("opaque-attempt").unwrap();
    store_command(
        store,
        "task",
        StoredCommand::run(
            RunId::new("run").unwrap(),
            RunAction::AddTask(
                Task::new(
                    task.clone(),
                    "Exact mixed base test",
                    TaskKind::Code,
                    vec!["Opaque bytes unchanged".into()],
                    vec![],
                )
                .unwrap(),
            ),
        ),
    );
    store_command(
        store,
        "ready",
        StoredCommand::run(RunId::new("run").unwrap(), RunAction::Ready(task.clone())),
    );
    let mut request = fixture["requests"]["base"].clone();
    request["attemptId"] = json!(attempt.as_str());
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
            worktree: local("opaque-worktree"),
            project: Some(project.id().clone()),
            correction_of: None,
        },
    )
    .unwrap();
    let definition = |kind, id: &str| DefinitionRef {
        kind,
        id: id.into(),
        version: 1,
    };
    store_command(
        store,
        "reserve",
        StoredCommand::run(
            RunId::new("run").unwrap(),
            RunAction::assign(
                task,
                value,
                AttemptSnapshot {
                    definition: SnapshotInput {
                        id: "opaque-snapshot".into(),
                        attempt_id: attempt.as_str().into(),
                        mode: definition(DefinitionKind::Mode, "development"),
                        profile: definition(DefinitionKind::Profile, "implementer"),
                        context: definition(DefinitionKind::Context, "context-pack-1"),
                        configuration: definition(DefinitionKind::Configuration, "configuration"),
                    },
                    diagnostics: vec![],
                },
            ),
        ),
    );
}
#[test]
fn production_materialization_recovers_after_effect_and_reopens_exact_persisted_evidence() {
    let fixture = Fixture::new();
    let db = fixture.root.join("state.sqlite");
    let mut store = Store::open(&db).unwrap();
    seed_attempt(&mut store, &fixture.base);
    let service = git::WorktreeService::new().unwrap();
    let request = git::CreateWorktree {
        scope: local("opaque-create"),
        key: local("create"),
        expected_revision: store.load().unwrap().revision(),
        project: ProjectId::new("project").unwrap(),
        repository: RepositoryId::new("repository").unwrap(),
        run: RunId::new("run").unwrap(),
        task: TaskId::new("opaque-task").unwrap(),
        attempt: AttemptId::new("opaque-attempt").unwrap(),
        directory_reference: local("repository-grant"),
        worktree_reference: local("opaque-worktree"),
        repository_path: fixture.main.clone(),
        authorized_directory: fixture.authorized.clone(),
        base_commit: fixture.base.clone(),
    };
    let materialization_started = Instant::now();
    assert!(matches!(
        service.create_with_preservation_and_fault(
            &mut store,
            &request,
            &Fixture::paths(),
            git::GitFaultPoint::AfterWorktree
        ),
        Err(GitError::Injected)
    ));
    eprintln!(
        "opaque_timing phase=create_through_materialization_and_verification elapsed_ms={} base_files=4",
        materialization_started.elapsed().as_millis()
    );
    let began = Instant::now();
    let created = service
        .create_with_preservation(&mut store, &request, &Fixture::paths())
        .unwrap();
    eprintln!(
        "opaque_timing phase=resume_confirmation elapsed_ms={}",
        began.elapsed().as_millis()
    );
    assert!(created.intent.preservation.is_some());
    let root = Path::new(&created.intent.path);
    for name in [
        "source.txt",
        "mixed.bin",
        "large.txt",
        "pattern-fixture.txt",
    ] {
        assert_eq!(
            content_hash(&fs::read(root.join(name)).unwrap()),
            content_hash(&fs::read(fixture.main.join(name)).unwrap())
        );
    }
    fs::write(root.join("source.txt"), b"persisted edit\n").unwrap();
    let revision = store.load().unwrap().revision();
    let began = Instant::now();
    let artifact = service
        .capture(
            &mut store,
            &local("opaque-capture"),
            &local("capture"),
            revision,
            &created.intent.operation_id,
        )
        .unwrap();
    eprintln!(
        "opaque_timing phase=service_capture_including_persistence elapsed_ms={} changed_files=1",
        began.elapsed().as_millis()
    );
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.preservation, created.intent.preservation);
    drop(store);
    let reopened = Store::open(&db).unwrap();
    assert_eq!(
        reopened
            .load()
            .unwrap()
            .captured_artifacts()
            .get(&(artifact.id.clone(), artifact.revision.clone())),
        Some(&artifact)
    );
    service
        .inspect(&reopened, &created.intent.operation_id)
        .unwrap();
}

#[test]
fn missing_and_corrupt_opaque_objects_never_fall_back_to_head_or_working_bytes() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let manifest = intent.preservation.as_ref().unwrap();
    let entry = manifest
        .entries
        .iter()
        .find(|e| e.path == "mixed.bin")
        .unwrap();
    let object = fixture
        .main
        .join(".git/objects")
        .join(&entry.git_oid[..2])
        .join(&entry.git_oid[2..]);
    assert!(object.is_file());
    #[cfg(windows)]
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "Windows-only owned fixture: clear its read-only attribute; Unix permissions are not changed"
    )]
    {
        let mut permissions = fs::metadata(&object).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&object, permissions).unwrap();
    }
    let original = fs::read(&object).unwrap();
    let executor = GitExecutor::new().unwrap();
    fs::remove_file(&object).unwrap();
    // `ls-tree -l` reports an unavailable blob size as BAD. The profile rejects
    // that metadata before reading blobs; intact worktree bytes are no fallback.
    assert_eq!(
        git::plan_base_preservation(
            &fixture.main,
            &fixture.base,
            &Fixture::paths(),
            &CancellationToken::new()
        ),
        Err(GitError::Unsupported)
    );
    assert_eq!(
        git::capture(
            &executor,
            Path::new(&intent.path),
            &intent,
            CaptureLimits::default()
        ),
        Err(GitError::Unsupported)
    );
    fs::write(&object, b"owned fixture corrupt object, not zlib").unwrap();
    // Corrupt object metadata is equally rejected, rather than reconstructed
    // from HEAD, an artifact or the file still present in the worktree.
    assert_eq!(
        git::plan_base_preservation(
            &fixture.main,
            &fixture.base,
            &Fixture::paths(),
            &CancellationToken::new()
        ),
        Err(GitError::Unsupported)
    );
    assert_eq!(
        git::capture(
            &executor,
            Path::new(&intent.path),
            &intent,
            CaptureLimits::default()
        ),
        Err(GitError::Unsupported)
    );
    fs::write(object, original).unwrap();
    assert_eq!(
        git::plan_base_preservation(
            &fixture.main,
            &fixture.base,
            &Fixture::paths(),
            &CancellationToken::new()
        )
        .unwrap(),
        *manifest
    );
}

#[test]
fn opaque_deletion_rename_file_type_and_index_modes_are_not_authorized_edits() {
    let fixture = Fixture::new();
    let intent = fixture.intent();
    let root = Path::new(&intent.path);
    let executor = GitExecutor::new().unwrap();
    let path = root.join("mixed.bin");
    let original = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::UnpreservedWork)
    ));
    fs::write(&path, &original).unwrap();
    fs::rename(&path, root.join("renamed.bin")).unwrap();
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::Scope)
    ));
    fs::rename(root.join("renamed.bin"), &path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::Unsupported)
    ));
    fs::remove_dir(&path).unwrap();
    fs::write(&path, &original).unwrap();
    git_cmd(root, &["update-index", "--chmod=+x", "mixed.bin"]);
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::UnpreservedWork)
    ));
    git_cmd(root, &["read-tree", &fixture.base]);
    let oid = &intent
        .preservation
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|e| e.path == "mixed.bin")
        .unwrap()
        .git_oid;
    git_cmd(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "120000",
            oid,
            "mixed.bin",
        ],
    );
    assert!(matches!(
        git::capture(&executor, root, &intent, CaptureLimits::default()),
        Err(GitError::UnpreservedWork)
    ));
    git_cmd(root, &["read-tree", &fixture.base]);
    assert!(
        git::capture(&executor, root, &intent, CaptureLimits::default())
            .unwrap()
            .files
            .is_empty()
    );
}

#[test]
fn pre_cancelled_v2_reads_preserve_index_head_and_do_not_produce_compatible_evidence() {
    let fixture = Fixture::new();
    let head = fixture.base.clone();
    let index = fs::read(fixture.main.join(".git/index")).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        git::plan_base_preservation(&fixture.main, &head, &Fixture::paths(), &cancel),
        Err(GitError::Pending)
    ));
    assert!(matches!(
        git::inspect_repository_with_editable(&fixture.main, None, &Fixture::paths(), &cancel),
        Err(GitError::Pending)
    ));
    assert_eq!(fs::read(fixture.main.join(".git/index")).unwrap(), index);
    assert_eq!(git_text(&fixture.main, &["rev-parse", "HEAD"]), head);
}

#[test]
fn selected_text_budget_is_checked_before_retaining_multiple_full_snapshots() {
    let fixture = Fixture::new();
    let mut paths = Vec::new();
    for n in 0..5 {
        let name = format!("chunk-{n}.txt");
        fs::write(fixture.main.join(&name), vec![b'z'; 900_000]).unwrap();
        paths.push(name);
    }
    git_cmd(&fixture.main, &["add", "--all"]);
    git_cmd(
        &fixture.main,
        &["commit", "--quiet", "-m", "Owned editable budget fixture"],
    );
    let base = git_text(&fixture.main, &["rev-parse", "HEAD"]);
    assert!(matches!(
        git::plan_base_preservation(&fixture.main, &base, &paths, &CancellationToken::new()),
        Err(GitError::Limit)
    ));
    let report = git::inspect_repository_with_editable(
        &fixture.main,
        None,
        &paths,
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(!report.compatible && !report.complete);
    assert!(report.blockers.iter().any(|b| b.code == "capture-limit"));
}

#[test]
fn partial_streaming_materialization_stays_planned_and_is_not_overwritten_on_replay() {
    let fixture = Fixture::new();
    let db = fixture.root.join("partial.sqlite");
    let mut store = Store::open(&db).unwrap();
    seed_attempt(&mut store, &fixture.base);
    let service = git::WorktreeService::new().unwrap();
    let request = git::CreateWorktree {
        scope: local("partial-create"),
        key: local("create"),
        expected_revision: store.load().unwrap().revision(),
        project: ProjectId::new("project").unwrap(),
        repository: RepositoryId::new("repository").unwrap(),
        run: RunId::new("run").unwrap(),
        task: TaskId::new("opaque-task").unwrap(),
        attempt: AttemptId::new("opaque-attempt").unwrap(),
        directory_reference: local("repository-grant"),
        worktree_reference: local("opaque-worktree"),
        repository_path: fixture.main.clone(),
        authorized_directory: fixture.authorized.clone(),
        base_commit: fixture.base.clone(),
    };
    assert!(matches!(
        service.create_with_preservation_and_fault(
            &mut store,
            &request,
            &Fixture::paths(),
            git::GitFaultPoint::DuringMaterialization
        ),
        Err(GitError::Injected)
    ));
    let record = store
        .load()
        .unwrap()
        .worktrees()
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(
        record.state,
        orchestrix_core::git_records::WorktreeState::Planned
    );
    assert!(record.observations.is_empty());
    let first = Path::new(&record.intent.path)
        .join(&record.intent.preservation.as_ref().unwrap().entries[0].path);
    let bytes = fs::read(&first).unwrap();
    assert!(
        service
            .create_with_preservation(&mut store, &request, &Fixture::paths())
            .is_err()
    );
    assert_eq!(fs::read(&first).unwrap(), bytes);
    assert_eq!(
        store
            .load()
            .unwrap()
            .worktrees()
            .get(&record.intent.operation_id)
            .unwrap()
            .state,
        orchestrix_core::git_records::WorktreeState::Planned
    );
    assert!(
        git_text(
            &fixture.main,
            &[
                "for-each-ref",
                "--format=%(refname)",
                &format!(
                    "refs/orchestrix/materialized/{}",
                    record.intent.operation_id
                )
            ]
        )
        .is_empty()
    );
    drop(store);
    let reopened = Store::open(&db).unwrap();
    assert_eq!(
        reopened
            .load()
            .unwrap()
            .worktrees()
            .get(&record.intent.operation_id)
            .unwrap()
            .state,
        orchestrix_core::git_records::WorktreeState::Planned
    );
    assert_eq!(fs::read(first).unwrap(), bytes);
}
