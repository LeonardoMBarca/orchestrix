//! Capture checks use only owned, disposable Git fixtures, never a live agent.
use orchestrix_core::{
    git::{CaptureLimits, GitError, GitExecutor, capture, capture_with_observer},
    git_records::{WorktreeIntent, content_hash},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    root: PathBuf,
    work: PathBuf,
    intent: WorktreeIntent,
    executor: GitExecutor,
}
impl Fixture {
    fn new() -> Self {
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        let root = temp.join(format!(
            "orchestrix-capture-espaço-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let main = root.join("main repository");
        let work = root.join("tentativa acentuação");
        fs::create_dir(&main).unwrap();
        fixture_git(&main, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(main.join("source.txt"), "base\n").unwrap();
        fs::write(main.join("delete.txt"), "remove me\n").unwrap();
        fs::write(main.join("unchanged.txt"), "constant\n").unwrap();
        fs::write(main.join(".gitignore"), "ignored/\n").unwrap();
        fixture_git(&main, &["add", "--all"]);
        fixture_git(&main, &["commit", "--quiet", "-m", "Owned capture fixture"]);
        let base = fixture_git(&main, &["rev-parse", "HEAD"]);
        let base = std::str::from_utf8(&base).unwrap().trim().to_owned();
        fixture_git(
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "orchestrix/capture",
                &git_path(&work),
                &base,
            ],
        );
        let common = fs::canonicalize(main.join(".git")).unwrap();
        let main = fs::canonicalize(main).unwrap();
        let work = fs::canonicalize(work).unwrap();
        let intent = WorktreeIntent {
            preservation: None,
            operation_id: "capture_operation".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: base,
            domain_base: "base".into(),
            worktree_ref: "worktree".into(),
            authorized_root: root.to_str().unwrap().into(),
            path: work.to_str().unwrap().into(),
            main_repository: main.to_str().unwrap().into(),
            branch: "orchestrix/capture".into(),
            common_git_dir: common.to_str().unwrap().into(),
            ownership_token: content_hash(b"owned-test-operation"),
        };
        assert!(intent.validate());
        Self {
            root,
            work,
            intent,
            executor: GitExecutor::new().unwrap(),
        }
    }
    fn capture(&self) -> Result<orchestrix_core::git_records::CapturedArtifact, GitError> {
        capture(
            &self.executor,
            &self.work,
            &self.intent,
            CaptureLimits::default(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        let resolved = fs::canonicalize(&self.root).unwrap();
        assert!(
            resolved.starts_with(temp)
                && resolved
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchestrix-capture-")
        );
        fs::remove_dir_all(resolved).unwrap();
    }
}
fn fixture_git(root: &Path, args: &[&str]) -> Vec<u8> {
    let mut command = Command::new("git");
    command.env_clear();
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
    ] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null)
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
        .current_dir(root);
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "fixture Git operation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn git_path(path: &Path) -> String {
    let text = path.to_str().unwrap();
    #[cfg(windows)]
    if let Some(ordinary) = text.strip_prefix("\\\\?\\") {
        assert!(
            !ordinary.starts_with("UNC\\"),
            "fixture requires a local drive"
        );
        return ordinary.to_owned();
    }
    text.to_owned()
}

#[test]
fn exact_base_index_worktree_new_delete_and_explicit_omissions() {
    let fixture = Fixture::new();
    fs::write(fixture.work.join("source.txt"), "staged\n").unwrap();
    fixture_git(&fixture.work, &["add", "source.txt"]);
    fs::write(fixture.work.join("source.txt"), "working\r\nlast line").unwrap();
    fs::remove_file(fixture.work.join("delete.txt")).unwrap();
    fs::write(fixture.work.join("novo espaço.txt"), "new\n").unwrap();
    fs::write(
        fixture.work.join(".env"),
        "API_KEY=sk-synthetic-secret-123456789\n",
    )
    .unwrap();
    fs::create_dir(fixture.work.join("ignored")).unwrap();
    fs::write(fixture.work.join("ignored/hidden.bin"), [0, 1, 2]).unwrap();
    fs::create_dir(fixture.work.join("node_modules")).unwrap();
    fs::write(
        fixture.work.join("node_modules/module.txt"),
        "not a deliverable\n",
    )
    .unwrap();
    let artifact = fixture.capture().unwrap();
    assert!(artifact.validate());
    assert_eq!(artifact.files.len(), 3);
    let changed = artifact
        .files
        .iter()
        .find(|file| file.path == "source.txt")
        .unwrap();
    assert_eq!(changed.base.as_ref().unwrap().content, "base\n");
    assert_eq!(changed.index.as_ref().unwrap().content, "staged\n");
    assert_eq!(
        changed.worktree.as_ref().unwrap().content,
        "working\r\nlast line"
    );
    assert!(!changed.untracked);
    let deleted = artifact
        .files
        .iter()
        .find(|file| file.path == "delete.txt")
        .unwrap();
    assert!(deleted.base.is_some() && deleted.index.is_some() && deleted.worktree.is_none());
    let added = artifact
        .files
        .iter()
        .find(|file| file.path == "novo espaço.txt")
        .unwrap();
    assert!(added.base.is_none() && added.index.is_none() && added.untracked);
    assert_eq!(
        added.worktree.as_ref().unwrap().sha256,
        content_hash(b"new\n")
    );
    for expected in [".env", "ignored", "node_modules"] {
        assert!(
            artifact
                .exclusions
                .iter()
                .any(|exclusion| exclusion.path == expected)
        );
    }
    assert!(
        artifact
            .patch
            .contains("+working\r\n+last line\n\\ No newline at end of file\n")
    );
    assert!(artifact.patch.contains("deleted file mode 100644"));
    assert!(
        !serde_json::to_string(&artifact)
            .unwrap()
            .contains("sk-synthetic")
    );
    assert_eq!(
        artifact.patch_sha256,
        content_hash(artifact.patch.as_bytes())
    );
    assert_eq!(fixture.capture().unwrap(), artifact);
}

#[test]
fn index_only_changes_are_preserved_even_when_final_worktree_equals_base() {
    let fixture = Fixture::new();
    fs::write(fixture.work.join("source.txt"), "intermediate\n").unwrap();
    fixture_git(&fixture.work, &["add", "source.txt"]);
    fs::write(fixture.work.join("source.txt"), "base\n").unwrap();
    let artifact = fixture.capture().unwrap();
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.files[0].base, artifact.files[0].worktree);
    assert_eq!(
        artifact.files[0].index.as_ref().unwrap().content,
        "intermediate\n"
    );
    assert!(artifact.patch.is_empty());
}

#[test]
fn deterministic_edit_during_snapshot_is_rejected_instead_of_mixed_artifact() {
    let fixture = Fixture::new();
    let result = capture_with_observer(
        &fixture.executor,
        &fixture.work,
        &fixture.intent,
        CaptureLimits::default(),
        || fs::write(fixture.work.join("unchanged.txt"), "concurrent\n").map_err(|_| GitError::Io),
    );
    assert_eq!(result, Err(GitError::InconsistentCapture));
}

#[test]
fn binary_secret_and_nondefault_index_flags_fail_without_redacting_exact_bytes() {
    let fixture = Fixture::new();
    fs::write(fixture.work.join("binary.bin"), [0, 1, 2]).unwrap();
    assert_eq!(fixture.capture(), Err(GitError::Unsupported));
    fs::remove_file(fixture.work.join("binary.bin")).unwrap();
    fs::write(
        fixture.work.join("secret.txt"),
        "sk-synthetic-secret-123456789\n",
    )
    .unwrap();
    assert_eq!(fixture.capture(), Err(GitError::SecretContent));
    fs::remove_file(fixture.work.join("secret.txt")).unwrap();
    fixture_git(
        &fixture.work,
        &["update-index", "--assume-unchanged", "source.txt"],
    );
    assert_eq!(fixture.capture(), Err(GitError::Unsupported));
}

#[test]
fn configured_filter_attributes_and_small_limits_are_refused() {
    let fixture = Fixture::new();
    fs::write(
        fixture.work.join(".gitattributes"),
        "*.txt filter=not-executed\n",
    )
    .unwrap();
    assert_eq!(fixture.capture(), Err(GitError::Unsupported));
    fs::remove_file(fixture.work.join(".gitattributes")).unwrap();
    fs::write(fixture.work.join("large.txt"), "many bytes\n").unwrap();
    let limits = CaptureLimits {
        file_bytes: 4,
        ..CaptureLimits::default()
    };
    assert_eq!(
        capture(&fixture.executor, &fixture.work, &fixture.intent, limits),
        Err(GitError::Limit)
    );
}

#[test]
fn staged_rename_is_an_explicit_delete_and_add_without_guessing_identity() {
    let fixture = Fixture::new();
    fixture_git(&fixture.work, &["mv", "source.txt", "renamed.txt"]);
    let artifact = fixture.capture().unwrap();
    assert_eq!(artifact.files.len(), 2);
    let deleted = artifact
        .files
        .iter()
        .find(|file| file.path == "source.txt")
        .unwrap();
    let added = artifact
        .files
        .iter()
        .find(|file| file.path == "renamed.txt")
        .unwrap();
    assert!(deleted.base.is_some() && deleted.index.is_none() && deleted.worktree.is_none());
    assert!(
        added.base.is_none()
            && added.index.is_some()
            && added.worktree.is_some()
            && !added.untracked
    );
    assert!(artifact.patch.contains("deleted file mode 100644"));
    assert!(artifact.patch.contains("new file mode 100644"));
}

#[test]
fn exact_generated_patch_is_accepted_as_syntax_by_git_without_applying_it() {
    let fixture = Fixture::new();
    fs::write(fixture.work.join("source.txt"), "working\r\nlast").unwrap();
    fs::remove_file(fixture.work.join("delete.txt")).unwrap();
    fs::write(fixture.work.join("arquivo espaço.txt"), "new\n").unwrap();
    fs::write(fixture.work.join("empty.txt"), "").unwrap();
    let artifact = fixture.capture().unwrap();
    let patch = fixture.root.join("capture.patch");
    fs::write(&patch, &artifact.patch).unwrap();
    fixture_git(
        Path::new(&fixture.intent.main_repository),
        &["apply", "--check", &git_path(&patch)],
    );
    assert_eq!(
        fs::read(Path::new(&fixture.intent.main_repository).join("source.txt")).unwrap(),
        b"base\n"
    );
    assert!(
        !Path::new(&fixture.intent.main_repository)
            .join("empty.txt")
            .exists()
    );
}
