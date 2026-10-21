//! Read-only repository admission evidence over real, owned Git fixtures.
//! No runtime, provider, project script, worktree effect or database is used.
use orchestrix_core::{
    git::{GitError, RepositoryPreflight, inspect_repository},
    runtime::CancellationToken,
};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const FILE_BYTES: usize = 1_048_576;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    repository: PathBuf,
}

impl Fixture {
    fn unborn() -> Self {
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        let root = temp.join(format!(
            "orchestrix-preflight-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("repository espa\u{e7}o");
        fs::create_dir(&repository).unwrap();
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        Self { root, repository }
    }

    fn new() -> Self {
        let fixture = Self::unborn();
        fixture.write("source.txt", b"committed source canary\n");
        fixture.write(
            "README.md",
            "# Fixture\nText with acentua\u{e7}\u{e3}o.\n".as_bytes(),
        );
        fixture.write(".gitignore", b"ignored/\n");
        fixture.commit();
        fixture
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.repository.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn commit(&self) {
        git(&self.repository, &["add", "--all"]);
        self.commit_index();
    }

    fn commit_index(&self) {
        git(
            &self.repository,
            &["commit", "--quiet", "-m", "Owned preflight fixture"],
        );
    }

    fn head(&self) -> String {
        git_text(&self.repository, &["rev-parse", "HEAD"])
    }

    fn inspect(&self) -> RepositoryPreflight {
        inspect_repository(&self.repository, None, &CancellationToken::new()).unwrap()
    }

    fn index(&self) -> Vec<u8> {
        fs::read(self.repository.join(".git/index")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Removal is restricted to this test's resolved, unique temporary root.
        let root = fs::canonicalize(&self.root).unwrap();
        let temp = fs::canonicalize(env::temp_dir()).unwrap();
        assert!(
            root.parent() == Some(temp.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchestrix-preflight-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

fn git_command(root: &Path, args: &[&str]) -> Command {
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
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null)
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
            "core.attributesFile=/dev/null",
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
            "-c",
            "maintenance.auto=false",
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
    command
}

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let result = git_command(root, args).output().unwrap();
    assert!(
        result.status.success(),
        "fixture Git failed: {args:?}: {}",
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

fn git_input(root: &Path, args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = git_command(root, args)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "fixture Git input failed: {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn has_blocker(report: &RepositoryPreflight, code: &str, count: usize) {
    let blocker = report
        .blockers
        .iter()
        .find(|item| item.code == code)
        .unwrap_or_else(|| panic!("missing {code}: {:?}", report.blockers));
    assert_eq!(blocker.count, count, "blocker {code}");
    assert!(blocker.examples.len() <= 3);
    assert!(
        blocker
            .examples
            .iter()
            .all(|path| path.chars().count() <= 180)
    );
}

#[test]
fn compatible_utf8_inventory_is_advisory_application_evidence_without_changes() {
    let fixture = Fixture::new();
    let index = fixture.index();
    let head = fixture.head();
    let source = fs::read(fixture.repository.join("source.txt")).unwrap();
    let report = fixture.inspect();

    assert_eq!(report.schema_version, 1);
    assert_eq!(report.profile, "regular-utf8-v1");
    assert_eq!(report.base_oid, head);
    assert_eq!(report.head_oid, head);
    assert!(report.compatible && report.application_eligible && report.complete);
    assert!(report.checkout.clean);
    assert_eq!(report.checkout.ignored, 0);
    assert_eq!(report.checkout.untracked, 0);
    assert!(report.blockers.is_empty());
    assert_eq!(report.inventory.files, 3);
    assert_eq!(report.inventory.regular, 3);
    assert_eq!(report.inventory.content_inspected, 3);
    assert_eq!(report.inventory.content_skipped, 0);
    assert_eq!(report.limits.files, 4096);
    assert_eq!(report.limits.file_bytes, FILE_BYTES);
    assert_eq!(report.limits.read_bytes, 64 * FILE_BYTES);
    assert!(!report.model_called);
    assert_eq!(
        report.authority,
        "inspection-only-revalidate-before-effects"
    );
    let wire = serde_json::to_string(&report).unwrap();
    assert!(wire.len() <= 65_536);
    assert!(!wire.contains("committed source canary"));
    assert_eq!(fixture.index(), index);
    assert_eq!(fixture.head(), head);
    assert_eq!(
        fs::read(fixture.repository.join("source.txt")).unwrap(),
        source
    );
}

#[test]
fn dirty_untracked_and_ignored_checkout_is_preserved_and_not_application_eligible() {
    let fixture = Fixture::new();
    fixture.write("source.txt", b"unsaved project change canary\n");
    fixture.write("notes.txt", b"untracked project notes canary\n");
    fixture.write("ignored/state.txt", b"ignored local state canary\n");
    let index = fixture.index();
    let head = fixture.head();
    let files = ["source.txt", "notes.txt", "ignored/state.txt"]
        .map(|path| (path, fs::read(fixture.repository.join(path)).unwrap()));
    let report = fixture.inspect();

    assert!(report.complete && report.compatible);
    assert!(!report.application_eligible);
    assert!(!report.checkout.clean);
    assert_eq!(report.checkout.untracked, 1);
    assert_eq!(report.checkout.ignored, 1);
    assert!(report.checkout.status.contains("source.txt"));
    assert!(report.checkout.status.contains("notes.txt"));
    // Inventory describes the committed base, not untracked file contents.
    assert_eq!(report.inventory.files, 3);
    assert_eq!(fixture.index(), index);
    assert_eq!(fixture.head(), head);
    for (path, before) in files {
        assert_eq!(fs::read(fixture.repository.join(path)).unwrap(), before);
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains(std::str::from_utf8(&before).unwrap().trim())
        );
    }
}

#[test]
fn unsupported_inventory_counts_sizes_binary_non_utf8_secrets_and_exclusions_without_content() {
    let fixture = Fixture::new();
    fixture.write("at-limit.txt", &vec![b'x'; FILE_BYTES]);
    fixture.write("over-limit.txt", &vec![b'y'; FILE_BYTES + 1]);
    fixture.write("binary-zero.bin", b"nul payload canary\0binary tail canary");
    fixture.write("non-utf8.txt", b"non-utf8 canary\xff\xfe");
    // Synthetic policy-pattern fixture, never an actual credential.
    fixture.write(
        "private-pattern.txt",
        b"API_KEY=fixture-only-secret-canary\n",
    );
    fixture.write(".env", b"excluded environment content canary\n");
    fixture.write(
        "target/generated.txt",
        b"excluded generated content canary\n",
    );
    fixture.commit();
    let index = fixture.index();
    let report = fixture.inspect();

    assert!(report.complete);
    assert!(!report.compatible && !report.application_eligible);
    assert_eq!(report.inventory.files, 10);
    assert_eq!(report.inventory.regular, 10);
    assert_eq!(report.inventory.oversized, 1);
    assert_eq!(report.inventory.largest_file_bytes, (FILE_BYTES + 1) as u64);
    assert_eq!(report.inventory.binary, 1);
    assert_eq!(report.inventory.non_utf8, 1);
    assert_eq!(report.inventory.secret_pattern, 1);
    assert_eq!(report.inventory.excluded, 2);
    assert_eq!(report.inventory.content_inspected, 9);
    assert_eq!(report.inventory.content_skipped, 1);
    for (code, count) in [
        ("file-size-limit", 1),
        ("binary-content", 1),
        ("non-utf8", 1),
        ("secret-pattern", 1),
        ("excluded-content", 2),
    ] {
        has_blocker(&report, code, count);
    }
    let wire = serde_json::to_string(&report).unwrap();
    assert!(wire.len() < 65_536);
    for canary in [
        "nul payload canary",
        "binary tail canary",
        "non-utf8 canary",
        "fixture-only-secret-canary",
        "excluded environment content canary",
        "excluded generated content canary",
        &"x".repeat(256),
        &"y".repeat(256),
    ] {
        assert!(!wire.contains(canary), "content leaked into preflight");
    }
    assert_eq!(fixture.index(), index);
    assert_eq!(
        fs::metadata(fixture.repository.join("over-limit.txt"))
            .unwrap()
            .len(),
        (FILE_BYTES + 1) as u64
    );
}

#[test]
fn executable_symlink_and_gitlink_modes_are_reported_without_materializing_links() {
    let fixture = Fixture::new();
    fixture.write("executable.txt", b"executable blob canary\n");
    fixture.write("link.txt", b"source.txt\n");
    git(&fixture.repository, &["add", "--all"]);
    git(
        &fixture.repository,
        &["update-index", "--chmod=+x", "executable.txt"],
    );
    let link_blob = git_text(&fixture.repository, &["rev-parse", ":link.txt"]);
    git(
        &fixture.repository,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "120000",
            &link_blob,
            "link.txt",
        ],
    );
    let base = fixture.head();
    git(
        &fixture.repository,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &base,
            "module",
        ],
    );
    fixture.commit_index();
    let index = fixture.index();
    let report = fixture.inspect();

    assert!(report.complete);
    assert!(!report.compatible && !report.application_eligible);
    assert_eq!(report.inventory.files, 6);
    assert_eq!(report.inventory.regular, 3);
    assert_eq!(report.inventory.executables, 1);
    assert_eq!(report.inventory.links, 1);
    assert_eq!(report.inventory.submodules, 1);
    assert_eq!(report.inventory.content_inspected, 5);
    assert_eq!(report.inventory.content_skipped, 1);
    has_blocker(&report, "executable-mode", 1);
    has_blocker(&report, "symbolic-link", 1);
    has_blocker(&report, "submodule", 1);
    assert!(
        !fs::symlink_metadata(fixture.repository.join("link.txt"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fixture.index(), index);
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains("executable blob canary")
    );
}

#[test]
fn checkout_transform_attributes_are_blocked_even_with_text_only_inventory() {
    for attribute in [
        "filter=fixture-unused",
        "working-tree-encoding=UTF-8",
        "ident",
    ] {
        let fixture = Fixture::new();
        fixture.write(
            ".gitattributes",
            format!("source.txt {attribute}\n").as_bytes(),
        );
        // Only this attribute file is staged; no transform driver is installed.
        git(&fixture.repository, &["add", "--", ".gitattributes"]);
        fixture.commit_index();
        let index = fixture.index();
        let report = fixture.inspect();
        assert!(report.complete, "{attribute}");
        assert!(
            !report.compatible && !report.application_eligible,
            "{attribute}"
        );
        has_blocker(&report, "unsupported-attributes", 1);
        has_blocker(&report, "unsupported-base-attributes", 1);
        assert_eq!(fixture.index(), index, "{attribute}");
    }
}

#[test]
fn dirty_checkout_cannot_hide_unsupported_attributes_in_the_pinned_base() {
    let fixture = Fixture::new();
    fixture.write(".gitattributes", b"source.txt ident\n");
    git(&fixture.repository, &["add", "--", ".gitattributes"]);
    fixture.commit_index();
    let head = fixture.head();
    // check-attr without a source would see this local version only. The owned
    // worker worktree materializes the pinned, committed attributes instead.
    fixture.write(".gitattributes", b"");
    let index = fixture.index();
    let report = fixture.inspect();
    assert_eq!(report.base_oid, head);
    assert!(report.complete);
    assert!(!report.compatible && !report.application_eligible);
    has_blocker(&report, "unsupported-base-attributes", 1);
    assert!(
        !report
            .blockers
            .iter()
            .any(|b| b.code == "unsupported-attributes"),
        "the local checkout no longer contains the incompatible attribute"
    );
    assert_eq!(fixture.index(), index);
    assert_eq!(fixture.head(), head);
    assert_eq!(
        fs::read(fixture.repository.join(".gitattributes")).unwrap(),
        b""
    );
}

#[test]
fn repository_filter_configuration_is_refused_before_driver_execution() {
    let fixture = Fixture::new();
    git(
        &fixture.repository,
        &[
            "config",
            "--local",
            "filter.fixture.process",
            "orchestrix-helper-must-not-run",
        ],
    );
    let index = fixture.index();
    let config = fs::read(fixture.repository.join(".git/config")).unwrap();
    assert!(matches!(
        inspect_repository(&fixture.repository, None, &CancellationToken::new()),
        Err(GitError::Unsupported)
    ));
    assert_eq!(fixture.index(), index);
    assert_eq!(
        fs::read(fixture.repository.join(".git/config")).unwrap(),
        config
    );
}

#[test]
fn unsupported_index_flags_and_unmerged_stages_fail_closed_without_rewriting_index() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let fixture = Fixture::new();
        git(&fixture.repository, &["update-index", flag, "source.txt"]);
        let index = fixture.index();
        let report = fixture.inspect();
        assert!(
            !report.compatible && !report.application_eligible && !report.complete,
            "{flag}"
        );
        has_blocker(&report, "capture-unsupported", 1);
        assert_eq!(fixture.index(), index, "{flag}");
    }
    let fixture = Fixture::new();
    let blob = git_text(&fixture.repository, &["rev-parse", "HEAD:source.txt"]);
    let input = format!(
        "0 {}\tsource.txt\n100644 {blob} 1\tsource.txt\n100644 {blob} 2\tsource.txt\n100644 {blob} 3\tsource.txt\n",
        "0".repeat(blob.len())
    );
    git_input(
        &fixture.repository,
        &["update-index", "--index-info"],
        input.as_bytes(),
    );
    let index = fixture.index();
    let report = fixture.inspect();
    assert!(!report.compatible && !report.application_eligible && !report.complete);
    has_blocker(&report, "capture-unsupported", 1);
    assert_eq!(fixture.index(), index);
}

#[test]
fn cancellation_is_not_compatible_evidence_and_preserves_repository() {
    let fixture = Fixture::new();
    let index = fixture.index();
    let head = fixture.head();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        inspect_repository(&fixture.repository, None, &cancel),
        Err(GitError::Pending)
    ));
    assert_eq!(fixture.index(), index);
    assert_eq!(fixture.head(), head);
}

#[test]
fn split_commit_graph_is_supported_but_arbitrary_or_deeper_object_directories_are_not() {
    let fixture = Fixture::new();
    git(
        &fixture.repository,
        &["commit-graph", "write", "--split", "--reachable"],
    );
    let graphs = fixture.repository.join(".git/objects/info/commit-graphs");
    let contents = fs::read_dir(&graphs)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            assert!(path.is_file());
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    assert!(
        contents.len() >= 2,
        "Git must produce a real chain and graph"
    );
    let index = fixture.index();
    let report = fixture.inspect();
    assert!(report.compatible && report.application_eligible && report.complete);
    assert_eq!(fixture.index(), index);
    for (path, bytes) in contents {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    let deeper = graphs.join("unsupported-child");
    fs::create_dir(&deeper).unwrap();
    assert!(matches!(
        inspect_repository(&fixture.repository, None, &CancellationToken::new()),
        Err(GitError::Unsupported)
    ));
    fs::remove_dir(deeper).unwrap();
    let arbitrary = fixture
        .repository
        .join(".git/objects/info/arbitrary-directory");
    fs::create_dir(&arbitrary).unwrap();
    assert!(matches!(
        inspect_repository(&fixture.repository, None, &CancellationToken::new()),
        Err(GitError::Unsupported)
    ));
    fs::remove_dir(arbitrary).unwrap();
    assert_eq!(fixture.index(), index);

    // Cacheinfo above exercises committed symlink modes on Windows too. This
    // separate Unix-only check rejects an actual object-store filesystem link
    // without requiring Windows Developer Mode or elevation to create one.
    #[cfg(unix)]
    {
        let target = fixture.root.join("owned-object-target");
        fs::create_dir(&target).unwrap();
        let link = graphs.join("linked-child");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(matches!(
            inspect_repository(&fixture.repository, None, &CancellationToken::new()),
            Err(GitError::Link)
        ));
        fs::remove_file(link).unwrap();
        assert!(target.is_dir());
    }
}

#[test]
fn unborn_head_invalid_base_and_old_commit_cannot_authorize_current_effects() {
    let unborn = Fixture::unborn();
    assert!(matches!(
        inspect_repository(&unborn.repository, None, &CancellationToken::new()),
        Err(GitError::Process)
    ));
    let fixture = Fixture::new();
    let first = fixture.head();
    let index = fixture.index();
    for base in ["HEAD", "refs/heads/main", "HEAD~1", "-bad", ""] {
        assert!(
            matches!(
                inspect_repository(&fixture.repository, Some(base), &CancellationToken::new()),
                Err(GitError::Base)
            ),
            "{base}"
        );
    }
    let absent = "f".repeat(first.len());
    assert!(matches!(
        inspect_repository(
            &fixture.repository,
            Some(&absent),
            &CancellationToken::new()
        ),
        Err(GitError::Process)
    ));
    assert_eq!(fixture.index(), index);
    fixture.write("source.txt", b"second committed revision\n");
    fixture.commit();
    let head = fixture.head();
    let index = fixture.index();
    let report =
        inspect_repository(&fixture.repository, Some(&first), &CancellationToken::new()).unwrap();
    assert_eq!(report.base_oid, first);
    assert_eq!(report.head_oid, head);
    assert!(report.complete);
    assert!(!report.compatible && !report.application_eligible);
    has_blocker(&report, "base-not-head", 1);
    assert_eq!(fixture.index(), index);
    assert_eq!(fixture.head(), head);
}
