//! Bounded Git built-ins, without a shell or inherited Git configuration.
//! This does not implement process-tree supervision or a command sandbox.
use super::paths::{canonical_directory, git_path, reject_links};
use super::{GitError, GitResult};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEADLINE: Duration = Duration::from_secs(15);
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
const OBJECT_ENTRIES: usize = 16_384;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

pub(crate) struct GitOutput {
    pub stdout: Vec<u8>,
    pub status: i32,
}
impl GitOutput {
    pub(crate) fn checked(self) -> GitResult<Vec<u8>> {
        if self.status == 0 {
            Ok(self.stdout)
        } else {
            Err(GitError::Process)
        }
    }
}

pub struct GitExecutor {
    binary: PathBuf,
    private: PathBuf,
}
impl GitExecutor {
    /// Read only, fixed built-in for a host-granted repository. No arbitrary arguments.
    pub fn inspect_head(&self, repository: &Path) -> GitResult<String> {
        let output = self
            .run_bounded(
                repository,
                &strings(&["rev-parse", "--verify", "HEAD^{commit}"]),
                None,
                DEADLINE,
                256,
            )?
            .checked()?;
        let oid = std::str::from_utf8(&output)
            .map_err(|_| GitError::Unsupported)?
            .trim();
        if !matches!(oid.len(), 40 | 64)
            || !oid
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(GitError::Unsupported);
        }
        Ok(oid.to_owned())
    }
    pub fn new() -> GitResult<Self> {
        let binary = find_git()?;
        let parent = canonical_directory(&env::temp_dir())?;
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GitError::Io)?
            .as_nanos();
        let mut private = None;
        for _ in 0..16 {
            let candidate = parent.join(format!(
                "orchestrix-git-{}-{epoch}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    private = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(GitError::Io),
            }
        }
        let executor = Self {
            binary,
            private: private.ok_or(GitError::Collision)?,
        };
        let version = executor.run(&parent, &strings(&["--version"]))?.checked()?;
        let version = std::str::from_utf8(&version).map_err(|_| GitError::Unsupported)?;
        let number = version
            .strip_prefix("git version ")
            .ok_or(GitError::Unsupported)?;
        let mut parts = number.trim().split('.');
        let major: u64 = parts
            .next()
            .ok_or(GitError::Unsupported)?
            .parse()
            .map_err(|_| GitError::Unsupported)?;
        let minor: u64 = parts
            .next()
            .ok_or(GitError::Unsupported)?
            .parse()
            .map_err(|_| GitError::Unsupported)?;
        // Before 2.36, `core.fsmonitor=false` could be interpreted as a hook
        // pathname instead of a disabling boolean. Fail before repository use.
        if major < 2 || major == 2 && minor < 36 {
            return Err(GitError::Unsupported);
        }
        Ok(executor)
    }

    pub(crate) fn run(&self, cwd: &Path, args: &[OsString]) -> GitResult<GitOutput> {
        self.run_bounded(cwd, args, None, DEADLINE, OUTPUT_LIMIT)
    }

    pub(crate) fn run_with_input(
        &self,
        cwd: &Path,
        args: &[OsString],
        input: &[u8],
    ) -> GitResult<GitOutput> {
        if input.len() > 1024 * 1024 {
            return Err(GitError::Limit);
        }
        self.run_bounded(cwd, args, Some(input), DEADLINE, OUTPUT_LIMIT)
    }

    fn run_bounded(
        &self,
        cwd: &Path,
        args: &[OsString],
        input: Option<&[u8]>,
        deadline: Duration,
        limit: usize,
    ) -> GitResult<GitOutput> {
        let cwd = canonical_directory(cwd)?;
        reject_links(&self.private)?;
        if fs::read_dir(&self.private)
            .map_err(|_| GitError::Io)?
            .next()
            .is_some()
        {
            return Err(GitError::Unsupported);
        }
        let Some(name) = args.first().and_then(|name| name.to_str()) else {
            return Err(GitError::Unsupported);
        };
        if !matches!(
            name,
            "--version"
                | "rev-parse"
                | "symbolic-ref"
                | "config"
                | "ls-files"
                | "ls-tree"
                | "cat-file"
                | "status"
                | "diff"
                | "worktree"
                | "branch"
                | "update-ref"
                | "for-each-ref"
                | "check-ref-format"
                | "check-attr"
                | "hash-object"
                | "show-ref"
                | "rev-list"
                | "merge-base"
                | "read-tree"
        ) {
            return Err(GitError::Unsupported);
        }
        // Block editor aliases and unrelated worktree/branch maintenance even
        // though only this module's trusted callers can supply these arguments.
        if name == "config"
            && args != strings(&["config", "--local", "--no-includes", "--null", "--list"])
            || name == "branch"
                && args.iter().any(|arg| {
                    matches!(
                        arg.to_str(),
                        Some("--edit-description" | "--force" | "-f" | "-D")
                    )
                })
            || name == "worktree"
                && !matches!(
                    args.get(1).and_then(|arg| arg.to_str()),
                    Some("add" | "list" | "remove" | "lock" | "unlock")
                )
            || matches!(name, "worktree" | "branch")
                && args
                    .iter()
                    .any(|arg| matches!(arg.to_str(), Some("--force" | "-f")))
        {
            return Err(GitError::Unsupported);
        }
        let mut command = Command::new(&self.binary);
        command.env_clear();
        for name in [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "SystemDrive",
            "TEMP",
            "TMP",
            "TMPDIR",
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
            .env("GIT_CONFIG_SYSTEM", null)
            .env("GIT_CONFIG_GLOBAL", null)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_PAGER", "cat")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .arg("--no-pager")
            .arg("--no-optional-locks")
            .arg("-c")
            .arg(format!(
                "core.hooksPath={}",
                git_path(&self.private)?.to_string_lossy()
            ))
            .args([
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "-c",
                "core.pager=",
                "-c",
                "pager.diff=false",
                "-c",
                "diff.external=",
                "-c",
                "submodule.recurse=false",
                "-c",
                "protocol.allow=never",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-c",
                "core.autocrlf=false",
            ])
            .arg("-c")
            .arg(format!("core.attributesFile={null}"))
            .arg(&args[0]);
        if name == "diff" {
            command.args(["--no-ext-diff", "--no-textconv"]);
        }
        command
            .args(&args[1..])
            .current_dir(cwd)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let started = Instant::now();
        let mut child = command.spawn().map_err(|_| GitError::Process)?;
        let stdout = child.stdout.take().ok_or(GitError::Process)?;
        let stderr = child.stderr.take().ok_or(GitError::Process)?;
        let (sender, receiver) = mpsc::sync_channel(8);
        drain(stdout, false, sender.clone());
        drain(stderr, true, sender.clone());
        if let Some(input) = input {
            let mut stdin = child.stdin.take().ok_or(GitError::Process)?;
            let input = input.to_vec();
            let sender = sender.clone();
            thread::spawn(move || {
                let result = stdin.write_all(&input);
                drop(stdin);
                let _ = sender.send(if result.is_ok() {
                    Pipe::InputClosed
                } else {
                    Pipe::Error
                });
            });
        }
        drop(sender);
        let mut result = Vec::new();
        let mut count = 0usize;
        let mut closed = 0;
        let mut status = None;
        let mut input_closed = input.is_none();
        loop {
            if started.elapsed() >= deadline {
                let _ = child.kill();
                reap_bounded(&mut child);
                return Err(GitError::Deadline);
            }
            match receiver.recv_timeout(Duration::from_millis(5)) {
                Ok(Pipe::Data(is_stderr, bytes)) => {
                    count = count.saturating_add(bytes.len());
                    if count > limit {
                        let _ = child.kill();
                        reap_bounded(&mut child);
                        return Err(GitError::OutputLimit);
                    }
                    if !is_stderr {
                        result.extend_from_slice(&bytes);
                    }
                }
                Ok(Pipe::Closed) => closed += 1,
                Ok(Pipe::InputClosed) => input_closed = true,
                Ok(Pipe::Error) => {
                    let _ = child.kill();
                    reap_bounded(&mut child);
                    return Err(GitError::Process);
                }
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
            }
            if status.is_none() {
                status = child.try_wait().map_err(|_| GitError::Process)?;
            }
            if closed == 2
                && input_closed
                && let Some(exit) = status
            {
                return Ok(GitOutput {
                    stdout: result,
                    status: exit.code().unwrap_or(-1),
                });
            }
        }
    }

    /// Inspect only the repository's direct local config, without includes.
    /// Unsupported features are refused before checkout/capture rather than
    /// running their configured programs or fetching missing objects.
    pub(crate) fn ensure_supported(&self, repo: &Path) -> GitResult<()> {
        // Check the worktree marker too, not only the directory returned after
        // Git resolves it. A normal linked-worktree .git file is supported.
        let marker = repo.join(".git");
        reject_links(&marker)?;
        if marker.is_dir() {
            reject_links(&marker.join("config"))?;
        }
        let config = self
            .run(
                repo,
                &strings(&["config", "--local", "--no-includes", "--null", "--list"]),
            )?
            .checked()?;
        for entry in config
            .split(|value| *value == 0)
            .filter(|entry| !entry.is_empty())
        {
            let key = entry
                .split(|value| *value == b'\n')
                .next()
                .ok_or(GitError::Unsupported)?;
            let key = std::str::from_utf8(key)
                .map_err(|_| GitError::Unsupported)?
                .to_ascii_lowercase();
            if key.starts_with("include.")
                || key.starts_with("includeif.")
                || key.starts_with("filter.")
                || key.starts_with("extensions.")
                || key.starts_with("remote.")
                    && (key.ends_with(".promisor") || key.ends_with(".partialclonefilter"))
                || matches!(
                    key.as_str(),
                    "core.worktree"
                        | "core.sparsecheckout"
                        | "core.sparsecheckoutcone"
                        | "core.alternaterefscommand"
                )
            {
                return Err(GitError::Unsupported);
            }
        }
        for arg in ["--absolute-git-dir", "--git-common-dir"] {
            let raw = self.run(repo, &strings(&["rev-parse", arg]))?.checked()?;
            let text = std::str::from_utf8(&raw)
                .map_err(|_| GitError::Path)?
                .trim_end_matches(['\n', '\r']);
            let path = Path::new(text);
            let dir = canonical_directory(&if path.is_absolute() {
                path.to_path_buf()
            } else {
                repo.join(path)
            })?;
            for name in [
                "config",
                "config.worktree",
                "commondir",
                "objects",
                "refs",
                "HEAD",
                "index",
                "packed-refs",
                "shallow",
                "logs",
                "worktrees",
                "gitdir",
                "locked",
                "objects/info/alternates",
                "objects/info/http-alternates",
                "info/attributes",
                "info/exclude",
                "info/grafts",
            ] {
                let candidate = dir.join(name);
                reject_links(&candidate)?;
                if (name.starts_with("objects/info/") || name == "info/grafts")
                    && candidate.exists()
                {
                    return Err(GitError::Unsupported);
                }
            }
            inspect_object_store(&dir.join("objects"))?;
        }
        Ok(())
    }
}

/// Inspect the supported loose/pack/info object layout without following links.
/// The common store may be absent in a linked worktree's administrative folder.
/// Deeper layouts (such as split commit-graph directories) are outside this
/// bounded profile; the caller must preserve them and report Unsupported.
fn inspect_object_store(root: &Path) -> GitResult<()> {
    reject_links(root)?;
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(GitError::Io),
        Ok(metadata) if !metadata.is_dir() => return Err(GitError::Unsupported),
        Ok(_) => {}
    }
    let mut count = 0usize;
    for entry in fs::read_dir(root).map_err(|_| GitError::Io)? {
        let path = entry.map_err(|_| GitError::Io)?.path();
        let metadata = object_entry(&path, &mut count)?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).map_err(|_| GitError::Io)? {
                let path = entry.map_err(|_| GitError::Io)?.path();
                if !object_entry(&path, &mut count)?.is_file() {
                    return Err(GitError::Unsupported);
                }
            }
        } else if !metadata.is_file() {
            return Err(GitError::Unsupported);
        }
    }
    Ok(())
}
fn object_entry(path: &Path, count: &mut usize) -> GitResult<fs::Metadata> {
    *count = count.saturating_add(1);
    if *count > OBJECT_ENTRIES {
        return Err(GitError::Limit);
    }
    // Check each entry before reading its type or descending. In particular,
    // objects/aa junctions cannot redirect a subsequent hash-object write.
    reject_links(path)?;
    fs::symlink_metadata(path).map_err(|_| GitError::Io)
}

impl Drop for GitExecutor {
    fn drop(&mut self) {
        if reject_links(&self.private).is_ok() {
            let _ = fs::remove_dir(&self.private);
        }
    }
}
fn strings(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}
fn find_git() -> GitResult<PathBuf> {
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    for directory in env::split_paths(&env::var_os("PATH").ok_or(GitError::Process)?) {
        if directory.is_absolute() {
            let path = directory.join(name);
            if path.is_file() {
                return fs::canonicalize(path).map_err(|_| GitError::Process);
            }
        }
    }
    Err(GitError::Process)
}
enum Pipe {
    Data(bool, Vec<u8>),
    Closed,
    InputClosed,
    Error,
}
fn drain(mut pipe: impl Read + Send + 'static, stderr: bool, sender: mpsc::SyncSender<Pipe>) {
    thread::spawn(move || {
        let mut bytes = [0u8; 8192];
        loop {
            match pipe.read(&mut bytes) {
                Ok(0) => {
                    let _ = sender.send(Pipe::Closed);
                    break;
                }
                Ok(count) => {
                    if sender
                        .send(Pipe::Data(stderr, bytes[..count].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => {
                    let _ = sender.send(Pipe::Error);
                    break;
                }
            }
        }
    });
}
fn reap_bounded(child: &mut std::process::Child) {
    for _ in 0..20 {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_installed_git_and_fixed_errors() {
        let git = GitExecutor::new().unwrap();
        let cwd = canonical_directory(&env::temp_dir()).unwrap();
        assert!(
            git.run(&cwd, &strings(&["--version"]))
                .unwrap()
                .checked()
                .unwrap()
                .starts_with(b"git version ")
        );
        assert!(matches!(
            git.run(&cwd, &strings(&["fetch"])),
            Err(GitError::Unsupported)
        ));
        assert_eq!(
            git.run_with_input(&cwd, &strings(&["hash-object", "--stdin"]), b"hello\n")
                .unwrap()
                .checked()
                .unwrap(),
            b"ce013625030ba8dba906f756967f9e9ca394464a\n"
        );
        assert!(matches!(
            git.run_with_input(
                &cwd,
                &strings(&["hash-object", "--stdin"]),
                &vec![0; 1024 * 1024 + 1]
            ),
            Err(GitError::Limit)
        ));
        assert!(matches!(
            git.run_bounded(&cwd, &strings(&["--version"]), None, DEADLINE, 1),
            Err(GitError::OutputLimit)
        ));
        assert!(matches!(
            git.run_bounded(
                &cwd,
                &strings(&["--version"]),
                None,
                Duration::ZERO,
                OUTPUT_LIMIT
            ),
            Err(GitError::Deadline)
        ));
        fs::write(git.private.join("unexpected"), b"not a hook").unwrap();
        assert!(matches!(
            git.run(&cwd, &strings(&["--version"])),
            Err(GitError::Unsupported)
        ));
        fs::remove_file(git.private.join("unexpected")).unwrap();
    }

    #[test]
    fn local_feature_admission_and_fsmonitor_override() {
        let git = GitExecutor::new().unwrap();
        let root = git.private.with_extension("fixture");
        fs::create_dir(&root).unwrap();
        let result = {
            let mut init = Command::new(&git.binary);
            init.env_clear();
            for key in ["PATH", "SystemRoot", "WINDIR", "SystemDrive", "TEMP", "TMP"] {
                if let Some(value) = env::var_os(key) {
                    init.env(key, value);
                }
            }
            init.env("GIT_CONFIG_NOSYSTEM", "1").env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            );
            let initialized = init
                .arg("-c")
                .arg(format!(
                    "init.templateDir={}",
                    git_path(&git.private).unwrap().to_string_lossy()
                ))
                .args(["init", "--quiet"])
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(initialized.status.success());
            git.ensure_supported(&root).unwrap();
            let original = git
                .run_with_input(
                    &root,
                    &strings(&["hash-object", "-w", "--stdin"]),
                    b"original\n",
                )
                .unwrap()
                .checked()
                .unwrap();
            let replacement = git
                .run_with_input(
                    &root,
                    &strings(&["hash-object", "-w", "--stdin"]),
                    b"replacement\n",
                )
                .unwrap()
                .checked()
                .unwrap();
            let original = std::str::from_utf8(&original).unwrap().trim();
            let replacement = std::str::from_utf8(&replacement).unwrap().trim();
            // Actual replacement refs must not rewrite the bytes addressed by
            // the base/blob OIDs recorded in persisted provenance.
            git.run(
                &root,
                &strings(&[
                    "update-ref",
                    &format!("refs/replace/{original}"),
                    replacement,
                ]),
            )
            .unwrap()
            .checked()
            .unwrap();
            assert_eq!(
                git.run(&root, &strings(&["cat-file", "blob", original]))
                    .unwrap()
                    .checked()
                    .unwrap(),
                b"original\n"
            );
            let config_path = root.join(".git/config");
            let config = fs::read_to_string(&config_path).unwrap();
            // With a real status command, this impossible helper is never run:
            // the command-line override disables repository fsmonitor programs.
            fs::write(
                &config_path,
                format!("{config}\n[core]\nfsmonitor = orchestrix-helper-must-not-run\n"),
            )
            .unwrap();
            assert_eq!(
                git.run(&root, &strings(&["status", "--porcelain=v1", "-z"]))
                    .unwrap()
                    .status,
                0
            );
            for forbidden in [
                "[include]\npath = non-existent-config\n",
                "[includeIf \"gitdir:*/\"]\npath = non-existent-config\n",
                "[filter \"custom\"]\nprocess = orchestrix-helper-must-not-run\n",
                "[extensions]\nworktreeConfig = true\n",
                "[remote \"origin\"]\npromisor = true\n",
                "[core]\nsparseCheckout = true\n",
            ] {
                fs::write(&config_path, format!("{config}\n{forbidden}")).unwrap();
                assert_eq!(git.ensure_supported(&root), Err(GitError::Unsupported));
            }
            fs::write(&config_path, config).unwrap();
            fs::write(root.join(".git/objects/info/alternates"), b"outside\n").unwrap();
            assert_eq!(git.ensure_supported(&root), Err(GitError::Unsupported));
            fs::remove_file(root.join(".git/objects/info/alternates")).unwrap();
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let outside = root.join("outside-object-store");
                let junction = root.join(".git/objects/aa");
                fs::create_dir(&outside).unwrap();
                fs::write(outside.join("untouched.txt"), b"preserved\n").unwrap();
                let shell = PathBuf::from(env::var_os("SystemRoot").unwrap())
                    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
                let output = Command::new(shell)
                    .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:ORCHESTRIX_TEST_JUNCTION -Target $env:ORCHESTRIX_TEST_TARGET | Out-Null"])
                    .env("ORCHESTRIX_TEST_JUNCTION", git_path(&junction).unwrap())
                    .env("ORCHESTRIX_TEST_TARGET", git_path(&outside).unwrap())
                    .current_dir(&root).creation_flags(0x0800_0000).output().unwrap();
                assert!(output.status.success());
                let mut write_ran = false;
                let guarded_write = (|| {
                    git.ensure_supported(&root)?;
                    write_ran = true;
                    git.run_with_input(
                        &root,
                        &strings(&["hash-object", "-w", "--stdin"]),
                        b"write probe must not run\n",
                    )?
                    .checked()?;
                    Ok::<_, GitError>(())
                })();
                assert_eq!(guarded_write, Err(GitError::Link));
                assert!(!write_ran);
                assert_eq!(
                    fs::read(outside.join("untouched.txt")).unwrap(),
                    b"preserved\n"
                );
                assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
                // Remove the link itself before recursive owned-fixture cleanup.
                fs::remove_dir(&junction).unwrap();
                fs::remove_file(outside.join("untouched.txt")).unwrap();
                fs::remove_dir(outside).unwrap();
            }
            let deeper = root.join(".git/objects/info/deeper");
            fs::create_dir(&deeper).unwrap();
            assert_eq!(git.ensure_supported(&root), Err(GitError::Unsupported));
            fs::remove_dir(deeper).unwrap();
            fs::create_dir_all(root.join(".git/info")).unwrap();
            fs::write(
                root.join(".git/info/grafts"),
                b"untrusted-history-rewrite\n",
            )
            .unwrap();
            assert_eq!(git.ensure_supported(&root), Err(GitError::Unsupported));
            assert_eq!(
                GitError::Process.to_string(),
                "Local Git operation rejected: process"
            );
            Ok::<_, GitError>(())
        };
        // This exact fixture path was created above. Do not traverse a replaced
        // link or recursively remove any path outside the canonical TEMP root.
        let canonical = fs::canonicalize(&root).unwrap();
        let temporary = canonical_directory(&env::temp_dir()).unwrap();
        assert!(super::super::paths::strict_descendant(
            &temporary, &canonical
        ));
        reject_links(&canonical).unwrap();
        fs::remove_dir_all(&canonical).unwrap();
        result.unwrap();
    }
}
