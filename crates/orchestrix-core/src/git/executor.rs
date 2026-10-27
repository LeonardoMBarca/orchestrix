//! Bounded Git built-ins, without a shell or inherited Git configuration.
//! This does not implement process-tree supervision or a command sandbox.
use super::paths::{canonical_directory, git_path, reject_links};
use super::read_deadline::{CommandDeadline, DeadlineBoundary, WaitObservation};
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
#[path = "executor_probe.rs"]
mod probe;
#[cfg(test)]
#[path = "read_deadline_tests.rs"]
mod read_deadline_tests;
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
const OBJECT_ENTRIES: usize = 16_384;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
#[cfg(test)]
thread_local! {
    static OBJECT_STORE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn owned_index_refresh(args: &[OsString], scoped: bool) -> bool {
    scoped && args == strings(&["update-index", "--refresh"])
}

fn diagnostic_builtin(args: &[OsString]) -> &'static str {
    const LABELS: &[&str] = &[
        "--version",
        "rev-parse",
        "symbolic-ref",
        "config",
        "ls-files",
        "ls-tree",
        "cat-file",
        "status",
        "diff",
        "worktree",
        "branch",
        "update-ref",
        "for-each-ref",
        "check-ref-format",
        "check-attr",
        "hash-object",
        "show-ref",
        "rev-list",
        "merge-base",
        "read-tree",
        "update-index",
    ];
    LABELS
        .iter()
        .copied()
        .find(|label| Some(*label) == args.first().and_then(|v| v.to_str()))
        .unwrap_or("unsupported")
}

fn scoped_run(
    command: Command,
    scope: &orchestrix_process::named_job::JobScope,
    input: Option<&[u8]>,
    deadline: Duration,
    limit: usize,
) -> GitResult<GitOutput> {
    use crate::integration_diagnostics::{CounterKind, CounterTimer};
    use orchestrix_process::{OwnedProcess, ProcessEvent, ProcessLimits, SpawnSpec};
    let mut process = {
        let _spawn_timing = CounterTimer::start(CounterKind::GitSpawn);
        OwnedProcess::spawn_identified_backpressured(
            SpawnSpec {
                executable: PathBuf::from(command.get_program()),
                arguments: command.get_args().map(OsString::from).collect(),
                cwd: command.get_current_dir().ok_or(GitError::Path)?.into(),
                environment: command
                    .get_envs()
                    .filter_map(|(k, v)| v.map(|v| (k.into(), v.into())))
                    .collect(),
                limits: ProcessLimits {
                    lifetime: deadline,
                    max_output_bytes: limit as u64,
                    max_write_bytes: 1_048_576,
                },
            },
            scope,
        )
        .map_err(|_| GitError::Ownership)?
    };
    // Git's batch protocol may produce a large blob before reading the next
    // request. Do not await a completed pipe write while its output queue needs
    // this same consumer: retain one bounded write ticket and drain both sides.
    let mut pending_input = input
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| process.begin_send(bytes).map_err(scoped_io_error))
        .transpose()?;
    process.close_input();
    let started = Instant::now();
    let mut stdout = Vec::new();
    loop {
        if started.elapsed() >= deadline {
            return Err(GitError::Deadline);
        }
        if let Some(pending) = &mut pending_input
            && pending.try_complete().map_err(scoped_io_error)?.is_some()
        {
            pending_input = None;
        }
        match process.receive(Duration::from_millis(5)) {
            Ok(Some(ProcessEvent::Stdout(bytes))) => {
                crate::integration_diagnostics::git_bytes(0, bytes.len());
                stdout.extend(bytes);
            }
            Ok(Some(ProcessEvent::Stderr(_))) | Ok(None) => {}
            Err(error) => return Err(scoped_io_error(error)),
        }
        // Drain a bounded burst before querying OS tree state. Raw local blob
        // reads can fill the same small event queue much faster than per-chunk
        // Job queries; neither the queue nor byte/deadline budgets are enlarged.
        for _ in 0..64 {
            match process.receive(Duration::ZERO).map_err(scoped_io_error)? {
                Some(ProcessEvent::Stdout(bytes)) => {
                    crate::integration_diagnostics::git_bytes(0, bytes.len());
                    stdout.extend(bytes);
                }
                Some(ProcessEvent::Stderr(_)) => {}
                None => break,
            }
        }
        let tree = {
            let _tree_timing = CounterTimer::start(CounterKind::GitTree);
            process.tree().map_err(|_| GitError::Ownership)?
        };
        let streams = process.streams();
        if let Some(code) = tree.root_exit_code
            && tree.owned
            && tree.active_processes == 0
            && streams.stdout_closed
            && streams.stderr_closed
            && pending_input.is_none()
        {
            loop {
                match process
                    .receive(Duration::from_millis(1))
                    .map_err(scoped_io_error)?
                {
                    Some(ProcessEvent::Stdout(bytes)) => {
                        crate::integration_diagnostics::git_bytes(0, bytes.len());
                        stdout.extend(bytes);
                    }
                    Some(ProcessEvent::Stderr(_)) => {}
                    None => break,
                }
            }
            process.health().map_err(scoped_io_error)?;
            let _close_timing = CounterTimer::start(CounterKind::GitClose);
            let stopped = process
                .terminate(Duration::from_secs(3))
                .map_err(|_| GitError::Ownership)?;
            let streams = process.wait_streams(Duration::from_secs(2));
            if !stopped.owned
                || stopped.active_processes != 0
                || !streams.stdin_closed
                || !streams.stdout_closed
                || !streams.stderr_closed
            {
                return Err(GitError::Ownership);
            }
            // Include the same normal RAII cleanup in the close counter.
            drop(process);
            return Ok(GitOutput {
                stdout,
                status: code as i32,
            });
        }
    }
}

fn scoped_io_error(error: orchestrix_process::ProcessError) -> GitError {
    match error {
        orchestrix_process::ProcessError::Budget => GitError::OutputLimit,
        orchestrix_process::ProcessError::Deadline => GitError::Deadline,
        _ => GitError::Process,
    }
}

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
    scope: Option<orchestrix_process::named_job::JobScope>,
    read_budget: Option<(Instant, Duration, crate::runtime::CancellationToken)>,
}

pub(super) struct CaptureHead {
    pub head: Vec<u8>,
    pub index: PathBuf,
}

/// Fresh paths from the same support observation, after local configuration,
/// link and object-store checks. These facts are not retained by the executor.
pub(super) struct SupportedDirectories {
    pub admin: PathBuf,
    pub common: PathBuf,
}

impl GitExecutor {
    /// Filesystem phases consume the same optional inspection budget as Git.
    /// A checkpoint cannot interrupt an OS call already in progress.
    pub(super) fn check_read_budget(&self) -> GitResult<()> {
        if let Some((started, lifetime, cancel)) = &self.read_budget {
            if cancel.is_cancelled() {
                crate::integration_diagnostics::read_budget_rejected(
                    DeadlineBoundary::FilesystemCheckpoint,
                    true,
                    self.read_budget_observation(),
                    None,
                );
                return Err(GitError::Pending);
            }
            let elapsed = started.elapsed();
            if elapsed >= *lifetime {
                crate::integration_diagnostics::read_budget_rejected(
                    DeadlineBoundary::FilesystemCheckpoint,
                    false,
                    Some((elapsed, *lifetime)),
                    None,
                );
                return Err(GitError::Deadline);
            }
        }
        Ok(())
    }
    fn read_budget_observation(&self) -> Option<(Duration, Duration)> {
        self.read_budget
            .as_ref()
            .map(|(start, lifetime, _)| (start.elapsed(), *lifetime))
    }
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
        Self::construct(None)
    }
    /// Integration effects and their reads share the durable keeper's owned Job.
    /// The legacy worktree executor retains its existing transport and contract.
    pub(crate) fn scoped(scope: orchestrix_process::named_job::JobScope) -> GitResult<Self> {
        Self::construct(Some(scope))
    }
    fn construct(scope: Option<orchestrix_process::named_job::JobScope>) -> GitResult<Self> {
        let resolution = probe::start_preparation();
        let binary = find_git()?;
        probe::selected(&binary, resolution.map(|start| start.elapsed()))?;
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
            scope,
            // Only the explicit private probe has a budget at construction.
            // Ordinary executors retain their existing admission semantics.
            read_budget: probe::read_budget(),
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
        probe::version_validated(version);
        Ok(executor)
    }

    /// Private fixed read sequence, opt-in and confined to an owned TEMP root.
    pub(crate) fn diagnostic_probe(
        repository: &Path,
        cancel: &crate::runtime::CancellationToken,
    ) -> GitResult<serde_json::Value> {
        probe::execute(repository, cancel)
    }

    pub(crate) fn run(&self, cwd: &Path, args: &[OsString]) -> GitResult<GitOutput> {
        self.run_bounded(cwd, args, None, DEADLINE, OUTPUT_LIMIT)
    }
    /// Fixed, bounded commit read used only by the opt-in preservation profile.
    pub(super) fn read_preservation_commit(&self, root: &Path, oid: &str) -> GitResult<Vec<u8>> {
        if !super::worktrees::valid_oid(oid) {
            return Err(GitError::Base);
        }
        self.run_bounded(
            root,
            &strings(&["cat-file", "commit", oid]),
            None,
            DEADLINE,
            1_048_576,
        )?
        .checked()
    }
    pub(super) fn with_read_budget(
        mut self,
        start: Instant,
        lifetime: Duration,
        cancel: crate::runtime::CancellationToken,
    ) -> Self {
        self.read_budget = Some((start, lifetime, cancel));
        self
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
        crate::integration_diagnostics::measure(
            crate::integration_diagnostics::Phase::GitCommand,
            || self.run_bounded_inner(cwd, args, input, deadline, limit),
        )
    }
    fn run_bounded_inner(
        &self,
        cwd: &Path,
        args: &[OsString],
        input: Option<&[u8]>,
        deadline: Duration,
        limit: usize,
    ) -> GitResult<GitOutput> {
        let preparation = probe::start_preparation();
        let command_limit = deadline;
        let mut observation_remaining = None;
        let deadline = if let Some((started, lifetime, cancel)) = &self.read_budget {
            if cancel.is_cancelled() {
                crate::integration_diagnostics::read_budget_rejected(
                    DeadlineBoundary::BeforeCommand,
                    true,
                    self.read_budget_observation(),
                    None,
                );
                return Err(GitError::Pending);
            }
            let elapsed = started.elapsed();
            let remaining = lifetime
                .checked_sub(elapsed)
                .filter(|d| !d.is_zero())
                .ok_or_else(|| {
                    crate::integration_diagnostics::read_budget_rejected(
                        DeadlineBoundary::BeforeCommand,
                        false,
                        Some((elapsed, *lifetime)),
                        None,
                    );
                    GitError::Deadline
                })?;
            observation_remaining = Some(remaining);
            deadline.min(remaining)
        } else {
            deadline
        };
        #[cfg(all(test, windows))]
        let _diagnostic_timing = test_timing::Call::start(args);
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
                | "update-index"
        ) {
            return Err(GitError::Unsupported);
        }
        // Block editor aliases and unrelated worktree/branch maintenance even
        // though only this module's trusted callers can supply these arguments.
        if name == "update-index" && !owned_index_refresh(args, self.scope.is_some())
            || name == "config"
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
        let _transport_timing = crate::integration_diagnostics::CounterTimer::start(
            crate::integration_diagnostics::CounterKind::GitTransport,
        );
        let command_facts = |elapsed| {
            CommandDeadline::new(
                diagnostic_builtin(args),
                command_limit,
                deadline,
                observation_remaining,
                elapsed,
            )
        };
        crate::integration_diagnostics::git_command_ready(
            command_facts(None),
            self.read_budget_observation(),
        );
        crate::integration_diagnostics::git_bytes(input.map_or(0, <[u8]>::len), 0);
        if let Some(scope) = &self.scope {
            let observed = Instant::now();
            let result = scoped_run(command, scope, input, deadline, limit);
            if matches!(result, Err(GitError::Deadline)) {
                crate::integration_diagnostics::read_budget_rejected(
                    DeadlineBoundary::CommandWait,
                    false,
                    self.read_budget_observation(),
                    Some(command_facts(Some(observed.elapsed()))),
                );
            }
            return result;
        }
        let mut probe = probe::CommandProbe::begin(
            &command,
            false,
            limit,
            deadline,
            preparation.map(|start| start.elapsed()),
        );
        let started = Instant::now();
        let child = {
            // The existing command deadline still starts before synchronous
            // process creation. This counter only separates its observed cost.
            let _spawn_timing = crate::integration_diagnostics::CounterTimer::start(
                crate::integration_diagnostics::CounterKind::GitSpawn,
            );
            command.spawn()
        };
        let mut child = match child {
            Ok(child) => child,
            Err(_) => {
                if let Some(probe) = &mut probe {
                    probe.spawn_failed(started.elapsed());
                }
                return Err(GitError::Process);
            }
        };
        if let Some(probe) = &mut probe {
            probe.spawned(child.id(), started.elapsed());
        }
        let result = (|| {
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
            let mut status: Option<std::process::ExitStatus> = None;
            let mut input_closed = input.is_none();
            if let Some(probe) = &mut probe {
                probe.observe_created(&child, started.elapsed(), deadline);
            }
            loop {
                let elapsed = started.elapsed();
                if elapsed >= deadline {
                    // Capture detection facts without IO. A diagnostic sink cannot
                    // delay the existing kill/reap at a command deadline.
                    let observation = self.read_budget_observation();
                    let command = command_facts(Some(elapsed));
                    // These are the loop's last observed facts, not a fresh OS
                    // probe. Do not poll the child or drain another stream before
                    // the mandatory kill/reap. In particular, None does not prove
                    // that the child is still running at this later instant.
                    let wait = u8::try_from(closed).ok().and_then(|closed| {
                        WaitObservation::new(
                            status.is_some(),
                            status.as_ref().and_then(std::process::ExitStatus::code),
                            closed,
                            input_closed,
                            result.len() as u64,
                            count as u64,
                        )
                    });
                    let command = wait.map_or(command, |wait| command.with_wait_observation(wait));
                    if let Some(probe) = &mut probe {
                        probe.cleanup_start();
                    }
                    let _ = child.kill();
                    reap_bounded(&mut child);
                    if let Some(probe) = &mut probe {
                        probe.cleanup_returned();
                    }
                    crate::integration_diagnostics::read_budget_rejected(
                        DeadlineBoundary::CommandWait,
                        false,
                        observation,
                        Some(command),
                    );
                    return Err(GitError::Deadline);
                }
                match receive_pipe(&receiver, Duration::from_millis(5)) {
                    Ok(Pipe::Data(is_stderr, bytes)) => {
                        if let Some(probe) = &mut probe {
                            probe.output();
                        }
                        count = count.saturating_add(bytes.len());
                        if count > limit {
                            if let Some(probe) = &mut probe {
                                probe.cleanup_start();
                            }
                            let _ = child.kill();
                            reap_bounded(&mut child);
                            if let Some(probe) = &mut probe {
                                probe.cleanup_returned();
                            }
                            return Err(GitError::OutputLimit);
                        }
                        if !is_stderr {
                            crate::integration_diagnostics::git_bytes(0, bytes.len());
                            result.extend_from_slice(&bytes);
                        }
                    }
                    Ok(Pipe::Closed) => {
                        closed += 1;
                        if closed == 2
                            && let Some(probe) = &mut probe
                        {
                            probe.streams_closed();
                        }
                    }
                    Ok(Pipe::InputClosed) => input_closed = true,
                    Ok(Pipe::Error) => {
                        if let Some(probe) = &mut probe {
                            probe.cleanup_start();
                        }
                        let _ = child.kill();
                        reap_bounded(&mut child);
                        if let Some(probe) = &mut probe {
                            probe.cleanup_returned();
                        }
                        return Err(GitError::Process);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                    }
                }
                if status.is_none() {
                    status = child.try_wait().map_err(|_| GitError::Process)?;
                    if let Some(status) = &status
                        && let Some(probe) = &mut probe
                    {
                        probe.exit(status);
                    }
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
        })();
        if let Some(probe) = &mut probe {
            probe.finish(&child, result.is_ok());
        }
        result
    }

    /// Inspect only the repository's direct local config, without includes.
    /// Unsupported features are refused before checkout/capture rather than
    /// running their configured programs or fetching missing objects.
    pub(crate) fn ensure_supported(&self, repo: &Path) -> GitResult<()> {
        self.ensure_supported_directories(repo).map(|_| ())
    }

    pub(super) fn ensure_supported_directories(
        &self,
        repo: &Path,
    ) -> GitResult<SupportedDirectories> {
        #[cfg(all(test, windows))]
        let _timing = test_timing::Call::phase("phase-support-directories");
        self.ensure_local_config(repo)?;
        let raw = self
            .run(
                repo,
                &strings(&[
                    "rev-parse",
                    "--path-format=absolute",
                    "--absolute-git-dir",
                    "--git-common-dir",
                ]),
            )?
            .checked()?;
        self.ensure_admin_stores(repository_directories(&raw)?)
    }

    /// One fresh capture observation; symbolic HEAD remains a separate native
    /// symbolic-ref query because rev-parse's DWIM name output is not equivalent.
    pub(super) fn capture_head(&self, repo: &Path) -> GitResult<CaptureHead> {
        #[cfg(all(test, windows))]
        let _timing = test_timing::Call::phase("phase-capture-head-support");
        self.ensure_local_config(repo)?;
        let output = self.run(
            repo,
            &strings(&[
                "rev-parse",
                "--path-format=absolute",
                "--absolute-git-dir",
                "--git-common-dir",
                "--git-path",
                "index",
                "--verify",
                "HEAD",
            ]),
        )?;
        if output.status != 0 {
            // A missing/unborn HEAD can fail after Git has printed its three
            // paths. Keep the old support-check precedence without repeating
            // a Git process or treating partial output as a successful frame.
            if let Ok([admin, common, _]) = repository_paths::<3>(&output.stdout) {
                self.ensure_admin_stores([admin, common])?;
            }
            return Err(GitError::Process);
        }
        let (directories, observation) = capture_head_fields(&output.stdout)?;
        self.ensure_admin_stores(directories)?;
        Ok(observation)
    }

    fn ensure_local_config(&self, repo: &Path) -> GitResult<()> {
        #[cfg(all(test, windows))]
        let _timing = test_timing::Call::phase("phase-support-config");
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
        Ok(())
    }

    fn ensure_admin_stores(
        &self,
        mut directories: [PathBuf; 2],
    ) -> GitResult<SupportedDirectories> {
        #[cfg(all(test, windows))]
        let _timing = test_timing::Call::phase("phase-support-admin-stores");
        for dir in &mut directories {
            *dir = canonical_directory(dir)?;
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
        }
        let [admin, common] = directories;
        // These are two roles in one fresh support observation, not two
        // independent repositories. A main checkout reports the very same
        // canonical directory for both. Validate both roles' paths above, then
        // inspect each distinct current object store once. Nothing is retained
        // across calls, captures or effects; linked-worktree stores stay separate.
        inspect_object_store(&admin.join("objects"))?;
        if common != admin {
            inspect_object_store(&common.join("objects"))?;
        }
        Ok(SupportedDirectories { admin, common })
    }
}

/// Git prints one canonical absolute directory for each fixed rev-parse option.
/// Reject truncation/extra fields rather than interpreting them as paths. This
/// batches only discovery; every support check still rereads config, paths and
/// object-store metadata at its original observation boundary.
fn repository_directories(raw: &[u8]) -> GitResult<[PathBuf; 2]> {
    repository_paths(raw)
}

fn metadata_fields<const N: usize>(raw: &[u8]) -> GitResult<[&[u8]; N]> {
    let body = raw.strip_suffix(b"\n").ok_or(GitError::Path)?;
    let mut fields = Vec::new();
    for line in body.split(|byte| *byte == b'\n') {
        if fields.len() == N {
            return Err(GitError::Path);
        }
        fields.push(line.strip_suffix(b"\r").unwrap_or(line));
    }
    fields.try_into().map_err(|_| GitError::Path)
}

fn metadata_path(raw: &[u8]) -> GitResult<PathBuf> {
    let text = std::str::from_utf8(raw).map_err(|_| GitError::Path)?;
    let path = PathBuf::from(text);
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err(GitError::Path);
    }
    Ok(path)
}

fn repository_paths<const N: usize>(raw: &[u8]) -> GitResult<[PathBuf; N]> {
    let mut directories = Vec::new();
    for field in metadata_fields::<N>(raw)? {
        directories.push(metadata_path(field)?);
    }
    directories.try_into().map_err(|_| GitError::Path)
}

fn capture_head_fields(raw: &[u8]) -> GitResult<([PathBuf; 2], CaptureHead)> {
    let [admin, common, index, oid] = metadata_fields::<4>(raw)?;
    if !matches!(oid.len(), 40 | 64)
        || !oid
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(GitError::Base);
    }
    // Preserve the standalone rev-parse HEAD bytes used by Snapshot equality.
    let mut head = oid.to_vec();
    head.push(b'\n');
    Ok((
        [metadata_path(admin)?, metadata_path(common)?],
        CaptureHead {
            head,
            index: metadata_path(index)?,
        },
    ))
}

#[cfg(test)]
mod metadata_batch_tests {
    use super::*;

    #[test]
    fn directory_response_requires_exact_absolute_fields() {
        let directory = canonical_directory(&env::temp_dir()).unwrap();
        let text = directory.to_str().unwrap();
        assert_eq!(
            repository_directories(format!("{text}\n{text}\n").as_bytes()).unwrap(),
            [directory.clone(), directory.clone()]
        );
        assert_eq!(
            repository_directories(format!("{text}\r\n{text}\r\n").as_bytes()).unwrap(),
            [directory.clone(), directory.clone()]
        );
        for bad in [
            format!("{text}\n"),
            format!("{text}\n{text}"),
            format!("{text}\n{text}\nextra\n"),
            format!("{text}\nrelative\n"),
            format!("{text}\n\n"),
            format!("{text}\0\n{text}\n"),
        ] {
            assert_eq!(repository_directories(bad.as_bytes()), Err(GitError::Path));
        }
    }

    #[test]
    fn capture_frame_is_exact_typed_and_hash_width_independent() {
        let directory = canonical_directory(&env::temp_dir()).unwrap();
        let text = directory.to_str().unwrap();
        for oid in ["a".repeat(40), "b".repeat(64)] {
            for ending in ["\n", "\r\n"] {
                let raw = format!("{text}{ending}{text}{ending}{text}{ending}{oid}{ending}");
                let (directories, frame) = capture_head_fields(raw.as_bytes()).unwrap();
                assert_eq!(directories, [directory.clone(), directory.clone()]);
                assert_eq!(frame.index, directory);
                assert_eq!(frame.head, format!("{oid}\n").as_bytes());
            }
        }
        for raw in [
            format!("{text}\n{text}\n{text}\n"),
            format!("{text}\n{text}\n{text}\n{}", "a".repeat(40)),
            format!("{text}\n{text}\n{text}\n{}\nextra\n", "a".repeat(40)),
            format!("{text}\n{text}\nrelative-index\n{}\n", "a".repeat(40)),
            format!("{text}\n{text}\n{text}\0\n{}\n", "a".repeat(40)),
        ] {
            assert!(matches!(
                capture_head_fields(raw.as_bytes()),
                Err(GitError::Path)
            ));
        }
        for oid in [
            "a".repeat(39),
            "a".repeat(41),
            "A".repeat(40),
            "z".repeat(40),
            "".into(),
        ] {
            assert!(matches!(
                capture_head_fields(format!("{text}\n{text}\n{text}\n{oid}\n").as_bytes()),
                Err(GitError::Base)
            ));
        }
    }

    #[test]
    fn installed_capture_frame_matches_native_oid_paths_and_symbolic_head() {
        let fixture = MetadataFixture::new();
        let root = fixture.repository("repository");
        assert!(matches!(
            fixture.executor.capture_head(&root),
            Err(GitError::Process)
        ));
        // Support restrictions precede the missing HEAD error without retries.
        fs::write(root.join(".git/objects/info/alternates"), b"outside\n").unwrap();
        assert!(matches!(
            fixture.executor.capture_head(&root),
            Err(GitError::Unsupported)
        ));
        fs::remove_file(root.join(".git/objects/info/alternates")).unwrap();
        let base = fixture.seed_empty(&root, "main");
        let frame = fixture.executor.capture_head(&root).unwrap();
        assert_eq!(
            frame.head,
            fixture
                .executor
                .run(&root, &strings(&["rev-parse", "--verify", "HEAD"]))
                .unwrap()
                .checked()
                .unwrap()
        );
        let standalone = fixture
            .executor
            .run(
                &root,
                &strings(&["rev-parse", "--path-format=absolute", "--git-path", "index"]),
            )
            .unwrap()
            .checked()
            .unwrap();
        assert_eq!(
            frame.index,
            metadata_path(metadata_fields::<1>(&standalone).unwrap()[0]).unwrap()
        );
        assert_eq!(
            fixture
                .executor
                .run(&root, &strings(&["symbolic-ref", "--quiet", "HEAD"]))
                .unwrap()
                .checked()
                .unwrap(),
            b"refs/heads/main\n"
        );
        // The symbolic name is deliberately read by the unchanged native
        // command: chained HEAD and an ambiguous DWIM "HEAD" remain supported.
        fs::write(
            root.join(".git/refs/heads/alias"),
            b"ref: refs/heads/main\n",
        )
        .unwrap();
        fs::write(root.join(".git/HEAD"), b"ref: refs/heads/alias\n").unwrap();
        assert_eq!(
            fixture.executor.capture_head(&root).unwrap().head,
            frame.head
        );
        assert_eq!(
            fixture
                .executor
                .run(&root, &strings(&["symbolic-ref", "--quiet", "HEAD"]))
                .unwrap()
                .checked()
                .unwrap(),
            b"refs/heads/main\n"
        );
        fixture
            .executor
            .run(&root, &strings(&["update-ref", "refs/heads/HEAD", &base]))
            .unwrap()
            .checked()
            .unwrap();
        assert_eq!(
            fixture.executor.capture_head(&root).unwrap().head,
            frame.head
        );
        assert_eq!(
            fixture
                .executor
                .run(&root, &strings(&["symbolic-ref", "--quiet", "HEAD"]))
                .unwrap()
                .checked()
                .unwrap(),
            b"refs/heads/main\n"
        );
        // Detached HEAD still has an OID, but symbolic-ref still rejects it.
        fs::write(root.join(".git/HEAD"), format!("{base}\n")).unwrap();
        assert_eq!(
            fixture.executor.capture_head(&root).unwrap().head,
            frame.head
        );
        assert_eq!(
            fixture
                .executor
                .run(&root, &strings(&["symbolic-ref", "--quiet", "HEAD"]))
                .unwrap()
                .status,
            1
        );
        let config_path = root.join(".git/config");
        let config = fs::read(&config_path).unwrap();
        let mut unsupported = config.clone();
        unsupported.extend_from_slice(
            b"\n[core]\nrepositoryFormatVersion = 1\n[extensions]\nobjectFormat = sha256\n",
        );
        fs::write(&config_path, unsupported).unwrap();
        assert!(matches!(
            fixture.executor.capture_head(&root),
            Err(GitError::Unsupported)
        ));
        fs::write(config_path, config).unwrap();
    }
}

/// Disposable repositories for the metadata grammar/race regressions only.
#[cfg(test)]
pub(super) struct MetadataFixture {
    pub executor: GitExecutor,
    pub root: PathBuf,
    temporary: PathBuf,
}
#[cfg(test)]
impl MetadataFixture {
    pub fn new() -> Self {
        let executor = GitExecutor::new().unwrap();
        let temporary = canonical_directory(&env::temp_dir()).unwrap();
        let root = executor.private.with_extension("metadata-fixture");
        fs::create_dir(&root).unwrap();
        Self {
            executor,
            root,
            temporary,
        }
    }
    pub fn repository(&self, name: &str) -> PathBuf {
        assert!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
        let root = self.root.join(name);
        fs::create_dir(&root).unwrap();
        let mut init = Command::new(&self.executor.binary);
        init.env_clear();
        for key in ["PATH", "SystemRoot", "WINDIR", "SystemDrive", "TEMP", "TMP"] {
            if let Some(value) = env::var_os(key) {
                init.env(key, value);
            }
        }
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        init.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", null)
            .env("GIT_CONFIG_GLOBAL", null)
            .arg("-c")
            .arg(format!(
                "init.templateDir={}",
                git_path(&self.executor.private).unwrap().to_string_lossy()
            ))
            .args(["init", "--quiet", "--initial-branch=main"])
            .current_dir(&root);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            init.creation_flags(0x0800_0000);
        }
        assert!(init.output().unwrap().status.success());
        root
    }
    pub fn seed_empty(&self, root: &Path, branch: &str) -> String {
        let tree = self
            .executor
            .run_with_input(
                root,
                &strings(&["hash-object", "-t", "tree", "-w", "--stdin"]),
                b"",
            )
            .unwrap()
            .checked()
            .unwrap();
        let tree = std::str::from_utf8(&tree).unwrap().trim();
        let commit = format!(
            "tree {tree}\nauthor Fixture <fixture@orchestrix.invalid> 1 +0000\ncommitter Fixture <fixture@orchestrix.invalid> 1 +0000\n\nCapture metadata fixture\n"
        );
        let base = self
            .executor
            .run_with_input(
                root,
                &strings(&["hash-object", "-t", "commit", "-w", "--stdin"]),
                commit.as_bytes(),
            )
            .unwrap()
            .checked()
            .unwrap();
        let base = std::str::from_utf8(&base).unwrap().trim().to_owned();
        self.executor
            .run(
                root,
                &strings(&["update-ref", &format!("refs/heads/{branch}"), &base]),
            )
            .unwrap()
            .checked()
            .unwrap();
        fs::write(
            root.join(".git/HEAD"),
            format!("ref: refs/heads/{branch}\n"),
        )
        .unwrap();
        self.executor
            .run(root, &strings(&["read-tree", &base]))
            .unwrap()
            .checked()
            .unwrap();
        base
    }
}
#[cfg(test)]
impl Drop for MetadataFixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.root).unwrap();
        assert!(
            root.parent() == Some(self.temporary.as_path())
                && root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchestrix-git-")
        );
        reject_links(&root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
pub(crate) mod test_timing {
    use std::{cell::RefCell, collections::BTreeMap, ffi::OsString, time::Instant};

    #[derive(serde::Serialize)]
    pub(crate) struct Sample {
        pub builtin: &'static str,
        pub elapsed_micros: u128,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub calls: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub measured_bytes: Option<u64>,
    }
    #[derive(Default)]
    struct Aggregate {
        elapsed_micros: u128,
        calls: u64,
        measured_bytes: Option<u64>,
    }
    thread_local! {
        static SAMPLES: RefCell<Option<Vec<Sample>>> = const { RefCell::new(None) };
        static PHASES: RefCell<Option<BTreeMap<&'static str, Aggregate>>> = const { RefCell::new(None) };
    }
    pub(crate) fn begin() {
        SAMPLES.with(|samples| assert!(samples.replace(Some(Vec::new())).is_none()));
        PHASES.with(|phases| assert!(phases.replace(Some(BTreeMap::new())).is_none()));
    }
    pub(crate) fn finish() -> Vec<Sample> {
        let mut samples =
            SAMPLES.with(|samples| samples.take().expect("diagnostic trace not enabled"));
        PHASES.with(|phases| {
            for (builtin, total) in phases.take().expect("diagnostic phases not enabled") {
                samples.push(Sample {
                    builtin,
                    elapsed_micros: total.elapsed_micros,
                    calls: Some(total.calls),
                    measured_bytes: total.measured_bytes,
                });
            }
        });
        samples
    }
    pub(crate) struct Call {
        builtin: &'static str,
        started: Instant,
        aggregate: bool,
        measured_bytes: Option<u64>,
    }
    impl Call {
        /// Test-only nested spans are aggregated by fixed labels. Their totals
        /// overlap their child phases and must not be summed as elapsed wall time.
        #[inline(never)]
        pub(crate) fn phase(label: &'static str) -> Option<Self> {
            if !SAMPLES.with(|samples| samples.borrow().is_some()) {
                return None;
            }
            Some(Self {
                builtin: label,
                started: Instant::now(),
                aggregate: true,
                measured_bytes: None,
            })
        }
        #[inline(never)]
        pub(crate) fn add_bytes(&mut self, bytes: u64) {
            self.measured_bytes = Some(self.measured_bytes.unwrap_or(0).saturating_add(bytes));
        }
        #[inline(never)]
        pub(super) fn start(args: &[OsString]) -> Option<Self> {
            if !SAMPLES.with(|samples| samples.borrow().is_some()) {
                return None;
            }
            let builtin = match args.first().and_then(|value| value.to_str()) {
                Some("--version") => "--version",
                Some("rev-parse") => "rev-parse",
                Some("symbolic-ref") => "symbolic-ref",
                Some("config") => "config",
                Some("ls-files") => "ls-files",
                Some("ls-tree") => "ls-tree",
                Some("cat-file") => "cat-file",
                Some("status") => "status",
                Some("diff") => "diff",
                Some("worktree") => "worktree",
                Some("branch") => "branch",
                Some("update-ref") => "update-ref",
                Some("for-each-ref") => "for-each-ref",
                Some("check-ref-format") => "check-ref-format",
                Some("check-attr") => "check-attr",
                Some("hash-object") => "hash-object",
                Some("show-ref") => "show-ref",
                Some("rev-list") => "rev-list",
                Some("merge-base") => "merge-base",
                Some("read-tree") => "read-tree",
                Some("update-index") => "update-index",
                _ => "unsupported",
            };
            Some(Self {
                builtin,
                started: Instant::now(),
                aggregate: false,
                measured_bytes: None,
            })
        }
    }
    impl Drop for Call {
        #[inline(never)]
        fn drop(&mut self) {
            let elapsed_micros = self.started.elapsed().as_micros();
            if self.aggregate {
                PHASES.with(|phases| {
                    if let Some(phases) = phases.borrow_mut().as_mut() {
                        assert!(phases.contains_key(self.builtin) || phases.len() < 32);
                        let total = phases.entry(self.builtin).or_default();
                        total.calls = total.calls.saturating_add(1);
                        total.elapsed_micros = total.elapsed_micros.saturating_add(elapsed_micros);
                        if let Some(bytes) = self.measured_bytes {
                            total.measured_bytes =
                                Some(total.measured_bytes.unwrap_or(0).saturating_add(bytes));
                        }
                    }
                });
                return;
            }
            SAMPLES.with(|samples| {
                if let Some(samples) = samples.borrow_mut().as_mut() {
                    samples.push(Sample {
                        builtin: self.builtin,
                        elapsed_micros,
                        calls: None,
                        measured_bytes: None,
                    });
                }
            });
        }
    }
}

/// Inspect the supported loose/pack/info object layout without following links.
/// The common store may be absent in a linked worktree's administrative folder.
/// The sole deeper layout admitted is Git's split commit-graph directory under
/// objects/info. Its children must be regular files and share the same total
/// entry budget; arbitrary/deeper directories and links remain unsupported.
fn inspect_object_store(root: &Path) -> GitResult<()> {
    #[cfg(all(test, windows))]
    let _timing = test_timing::Call::phase("phase-support-object-store");
    #[cfg(test)]
    OBJECT_STORE_READS.with(|count| count.set(count.get() + 1));
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
                let metadata = object_entry(&path, &mut count)?;
                if metadata.is_dir() && path == root.join("info/commit-graphs") {
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
        } else if !metadata.is_file() {
            return Err(GitError::Unsupported);
        }
    }
    Ok(())
}
fn object_entry(path: &Path, count: &mut usize) -> GitResult<fs::Metadata> {
    #[cfg(all(test, windows))]
    let _timing = test_timing::Call::phase("phase-support-object-entry");
    *count = count.saturating_add(1);
    if *count > OBJECT_ENTRIES {
        return Err(GitError::Limit);
    }
    // Check each entry before reading its type or descending. In particular,
    // objects/aa junctions cannot redirect a subsequent hash-object write.
    #[cfg(all(test, windows))]
    let path_timing = test_timing::Call::phase("phase-support-object-path-guard");
    reject_links(path)?;
    #[cfg(all(test, windows))]
    drop(path_timing);
    #[cfg(all(test, windows))]
    let _metadata_timing = test_timing::Call::phase("phase-support-object-metadata");
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
fn receive_pipe(
    receiver: &mpsc::Receiver<Pipe>,
    timeout: Duration,
) -> Result<Pipe, mpsc::RecvTimeoutError> {
    let started = Instant::now();
    let received = receiver.recv_timeout(timeout);
    if matches!(&received, Err(mpsc::RecvTimeoutError::Disconnected)) {
        // The readers may reach EOF before the child exits. A disconnected
        // channel has no remaining sender to wake this wait, so avoid spinning
        // try_wait while preserving the caller's existing polling interval.
        let remaining = timeout.saturating_sub(started.elapsed());
        if !remaining.is_zero() {
            thread::sleep(remaining);
        }
    }
    received
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

#[cfg(test)]
mod pipe_wait_tests {
    use super::*;

    #[test]
    fn disconnected_pipe_waits_without_synthesizing_closed_events() {
        let (sender, receiver) = mpsc::sync_channel(8);
        sender.send(Pipe::Data(false, b"out".to_vec())).unwrap();
        sender.send(Pipe::Data(true, b"err".to_vec())).unwrap();
        sender.send(Pipe::Closed).unwrap();
        sender.send(Pipe::Closed).unwrap();
        sender.send(Pipe::InputClosed).unwrap();
        drop(sender);
        for (is_stderr, expected) in [(false, b"out".as_slice()), (true, b"err".as_slice())] {
            assert!(matches!(
                receive_pipe(&receiver, Duration::ZERO),
                Ok(Pipe::Data(stream, bytes)) if stream == is_stderr && bytes.as_slice() == expected
            ));
        }
        for _ in 0..2 {
            assert!(matches!(
                receive_pipe(&receiver, Duration::ZERO),
                Ok(Pipe::Closed)
            ));
        }
        assert!(matches!(
            receive_pipe(&receiver, Duration::ZERO),
            Ok(Pipe::InputClosed)
        ));
        let timeout = Duration::from_millis(5);
        let started = Instant::now();
        assert!(matches!(
            receive_pipe(&receiver, timeout),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        assert!(started.elapsed() >= timeout);
    }

    #[test]
    fn open_pipe_timeout_remains_distinct_from_disconnection() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let timeout = Duration::from_millis(5);
        let started = Instant::now();
        assert!(matches!(
            receive_pipe(&receiver, timeout),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(started.elapsed() >= timeout);
        sender.send(Pipe::Closed).unwrap();
        assert!(matches!(
            receive_pipe(&receiver, Duration::ZERO),
            Ok(Pipe::Closed)
        ));
    }

    #[test]
    fn zero_pipe_wait_does_not_wait_for_a_disconnected_sender() {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(sender);
        let started = Instant::now();
        assert!(matches!(
            receive_pipe(&receiver, Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        assert!(started.elapsed() < Duration::from_millis(50));
    }
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
    fn filesystem_checkpoints_preserve_read_deadline_and_cancellation() {
        let cancel = crate::runtime::CancellationToken::new();
        // No executable, directory, process or provider is needed to inspect
        // the observation's existing budget.
        let mut executor = GitExecutor {
            binary: PathBuf::new(),
            private: PathBuf::new(),
            scope: None,
            read_budget: None,
        };
        assert_eq!(executor.check_read_budget(), Ok(()));
        executor.read_budget = Some((Instant::now(), Duration::from_secs(90), cancel.clone()));
        assert_eq!(executor.check_read_budget(), Ok(()));
        executor.read_budget = Some((Instant::now(), Duration::ZERO, cancel.clone()));
        assert_eq!(executor.check_read_budget(), Err(GitError::Deadline));
        cancel.cancel();
        assert_eq!(executor.check_read_budget(), Err(GitError::Pending));
    }

    #[test]
    fn aliased_metadata_roles_share_one_current_store_read_without_caching() {
        let executor = GitExecutor::new().unwrap();
        let fixture = executor.private.with_extension("metadata-roles");
        let first = fixture.join("first");
        let second = fixture.join("second");
        for directory in [&first, &second] {
            fs::create_dir_all(directory.join("objects/aa")).unwrap();
            fs::write(
                directory.join("objects/aa/entry"),
                b"owned metadata fixture",
            )
            .unwrap();
        }
        OBJECT_STORE_READS.with(|count| count.set(0));
        let same = executor
            .ensure_admin_stores([first.clone(), first.clone()])
            .unwrap();
        assert_eq!(same.admin, same.common);
        OBJECT_STORE_READS.with(|count| assert_eq!(count.get(), 1));
        let distinct = executor
            .ensure_admin_stores([first.clone(), second])
            .unwrap();
        assert_ne!(distinct.admin, distinct.common);
        OBJECT_STORE_READS.with(|count| assert_eq!(count.get(), 3));
        executor
            .ensure_admin_stores([first.clone(), first.clone()])
            .unwrap();
        OBJECT_STORE_READS.with(|count| assert_eq!(count.get(), 4));
        fs::create_dir(first.join("objects/aa/unsupported-depth")).unwrap();
        assert!(matches!(
            executor.ensure_admin_stores([first.clone(), first]),
            Err(GitError::Unsupported)
        ));
        OBJECT_STORE_READS.with(|count| assert_eq!(count.get(), 5));
    }
    #[test]
    fn index_refresh_cannot_stage_or_escape_owned_scope() {
        let refresh = strings(&["update-index", "--refresh"]);
        assert!(owned_index_refresh(&refresh, true));
        assert!(!owned_index_refresh(&refresh, false));
        for argv in [
            vec!["update-index"],
            vec!["update-index", "--really-refresh"],
            vec!["update-index", "--refresh", "--add"],
            vec!["update-index", "--refresh", "fixture.txt"],
            vec!["update-index", "--refresh", "--ignore-missing"],
            vec!["update-index", "--refresh", "-q"],
            vec!["update-index", "--index-info"],
            vec!["update-index", "--cacheinfo", "100644,object,path"],
        ] {
            assert!(!owned_index_refresh(&strings(&argv), true));
        }
    }
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
