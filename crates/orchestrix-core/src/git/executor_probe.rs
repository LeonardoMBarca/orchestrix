//! Private opt-in comparison of the exact preflight-support sequence. This is
//! not repository compatibility, execution authority or a generic shell API.
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc};

const MAX_SAMPLES: u64 = 2;
const SEQUENCE_LIMIT: Duration = Duration::from_secs(90);
static SAMPLES: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static ACTIVE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) };
}
#[derive(Default)]
struct State {
    cancel: Option<crate::runtime::CancellationToken>,
    started: Option<Instant>,
    selected: Option<Value>,
    version: Option<String>,
    commands: Vec<Value>,
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|slot| *slot.borrow_mut() = None);
    }
}
fn current() -> Option<Rc<RefCell<State>>> {
    ACTIVE.with(|s| s.borrow().clone())
}
pub(super) fn read_budget() -> Option<(Instant, Duration, crate::runtime::CancellationToken)> {
    let state = current()?;
    let state = state.borrow();
    Some((state.started?, SEQUENCE_LIMIT, state.cancel.clone()?))
}
fn check_state_budget(state: &State) -> GitResult<()> {
    if state.cancel.as_ref().is_some_and(|c| c.is_cancelled()) {
        return Err(GitError::Pending);
    }
    if state
        .started
        .is_some_and(|start| start.elapsed() >= SEQUENCE_LIMIT)
    {
        return Err(GitError::Deadline);
    }
    Ok(())
}

fn file_observation(path: &Path) -> GitResult<Value> {
    let start = Instant::now();
    let path_text = path.to_str().ok_or(GitError::Path)?;
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(GitError::Path);
    }
    let mut file = fs::File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > 32 * 1_048_576 {
        return Err(GitError::Limit);
    }
    #[cfg(windows)]
    let identity = orchestrix_process::windows_file_read::identity(&file)
        .ok()
        .map(|id| format!("{id:?}"));
    #[cfg(not(windows))]
    let identity: Option<String> = None;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65_536];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > before.len() {
            return Err(GitError::InconsistentCapture);
        }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata()?;
    if bytes != before.len()
        || after.len() != before.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(GitError::InconsistentCapture);
    }
    Ok(
        json!({"path":path_text,"bytes":bytes,"sha256":format!("{:x}",hash.finalize()),
        "physicalIdentity":identity,"observedMicros":micros(start.elapsed()),
        "snapshotAtomic":false,"authority":false}),
    )
}
pub(super) fn start_preparation() -> Option<Instant> {
    current().map(|_| Instant::now())
}
pub(super) fn selected(binary: &Path, resolution: Option<Duration>) -> GitResult<()> {
    selected_with(resolution, || file_observation(binary))
}
fn selected_with(
    resolution: Option<Duration>,
    observe: impl FnOnce() -> GitResult<Value>,
) -> GitResult<()> {
    if let Some(state) = current() {
        check_state_budget(&state.borrow())?;
        let mut file = observe()?;
        check_state_budget(&state.borrow())?;
        file["resolutionMicros"] = json!(resolution.map(micros));
        state.borrow_mut().selected = Some(file);
    }
    Ok(())
}
pub(super) fn version_validated(version: &str) {
    if let Some(state) = current() {
        state.borrow_mut().version = Some(version.trim().chars().take(128).collect());
    }
}
fn micros(d: Duration) -> u64 {
    d.as_micros().min(u128::from(u64::MAX)) as u64
}

pub(super) struct CommandProbe {
    state: Rc<RefCell<State>>,
    started: Instant,
    value: Value,
}
impl CommandProbe {
    pub(super) fn begin(
        command: &Command,
        scoped: bool,
        limit: usize,
        deadline: Duration,
        preparation: Option<Duration>,
    ) -> Option<Self> {
        let state = current()?;
        if state.borrow().commands.len() >= 4 {
            return None;
        }
        let environment = command
            .get_envs()
            .map(|(k, v)| {
                let key = k.to_string_lossy().into_owned();
                // Only hashes of the executor's already restricted environment.
                let digest = v.map(|v| format!("{:x}", Sha256::digest(v.as_encoded_bytes())));
                (key, json!({"present":v.is_some(),"sha256":digest}))
            })
            .collect::<serde_json::Map<String, Value>>();
        Some(Self {
            state,
            started: Instant::now(),
            value: json!({
                "program":command.get_program().to_str(),"arguments":command.get_args().map(|arg|arg.to_str()).collect::<Vec<_>>(),
                "cwd":command.get_current_dir(),"scoped":scoped,"creationFlags":if cfg!(windows){0x0800_0000u32}else{0},
                "stdin":"null","stdout":"pipe","stderr":"pipe","environment":environment,
                "deadlineMs":deadline.as_millis() as u64,"outputLimitBytes":limit,
                "executorPreparationMicros":preparation.map(micros),
                "childPid":null,"spawnReturnedMicros":null,"firstOutputMicros":null,
                "exitObservedMicros":null,"exitCode":null,"streamsClosedMicros":null,
                "cleanupStartedMicros":null,"cleanupReturnedMicros":null,"image":null
            }),
        })
    }
    pub(super) fn spawned(&mut self, pid: u32, elapsed: Duration) {
        self.value["childPid"] = json!(pid);
        self.value["spawnCallMicros"] = json!(micros(elapsed));
        self.value["spawnReturnedMicros"] = json!(micros(self.started.elapsed()));
    }
    pub(super) fn spawn_failed(&mut self, elapsed: Duration) {
        self.value["spawnCallMicros"] = json!(micros(elapsed));
        self.value["spawnError"] = json!("process-creation-failed");
        self.value["spawnReturnedMicros"] = json!(micros(self.started.elapsed()));
    }
    pub(super) fn output(&mut self) {
        if self.value["firstOutputMicros"].is_null() {
            self.value["firstOutputMicros"] = json!(micros(self.started.elapsed()));
        }
    }
    pub(super) fn exit(&mut self, status: &std::process::ExitStatus) {
        self.value["exitObservedMicros"] = json!(micros(self.started.elapsed()));
        self.value["exitCode"] = json!(status.code());
    }
    pub(super) fn streams_closed(&mut self) {
        self.value["streamsClosedMicros"] = json!(micros(self.started.elapsed()));
    }
    pub(super) fn cleanup_start(&mut self) {
        self.value["cleanupStartedMicros"] = json!(micros(self.started.elapsed()));
    }
    pub(super) fn cleanup_returned(&mut self) {
        self.value["cleanupReturnedMicros"] = json!(micros(self.started.elapsed()));
    }
    pub(super) fn observe_created(
        &mut self,
        child: &std::process::Child,
        elapsed: Duration,
        deadline: Duration,
    ) {
        self.observe_with(elapsed, deadline, || match orchestrix_process::process_observation::observe_child(child) {
            Ok(image) => json!({"pid":image.process_id,"path":image.image_path.to_str(),
                "creationTime100ns":image.creation_time_100ns.map(|v|v.to_string()),"creationTimeError":image.creation_time_error,
                "source":"borrowed-child-handle-after-spawn","descendantsObserved":false}),
            Err(error) => json!({"unavailable":format!("{error:?}"),"source":"borrowed-child-handle-after-spawn"}),
        });
    }
    fn observe_with(
        &mut self,
        elapsed: Duration,
        deadline: Duration,
        observe: impl FnOnce() -> Value,
    ) {
        // An exited process may no longer expose its image (ERROR_GEN_FAILURE).
        // Query only the borrowed just-created child, if time remains. The
        // existing loop checks the SAME deadline immediately after this call;
        // hashing and filesystem reads never run here. Like spawn/try_wait,
        // these fixed native calls are synchronous, not hard-interruptible.
        if elapsed >= deadline {
            self.value["image"] = json!({"unavailable":"deadline-before-image-observation"});
            return;
        }
        let start = Instant::now();
        self.value["imageObservationStartedMicros"] = json!(micros(self.started.elapsed()));
        self.value["image"] = observe();
        self.value["imageObservationMicros"] = json!(micros(start.elapsed()));
    }
    pub(super) fn finish(&mut self, _child: &std::process::Child, ok: bool) {
        self.value["commandReturnedMicros"] = json!(micros(self.started.elapsed()));
        self.value["ok"] = json!(ok);
    }
}
impl Drop for CommandProbe {
    fn drop(&mut self) {
        self.state.borrow_mut().commands.push(self.value.take());
    }
}

fn authorized_root(repository: &Path, selected: Option<OsString>) -> GitResult<PathBuf> {
    let selected = selected.ok_or(GitError::Scope)?;
    let root = canonical_directory(repository)?;
    let opted = canonical_directory(Path::new(&selected))?;
    let temp = canonical_directory(&env::temp_dir())?;
    if root != opted
        || !super::super::paths::strict_descendant(&temp, &root)
        || root.file_name().and_then(|s| s.to_str()) != Some("repository")
        || !root
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("orchestrix-"))
    {
        return Err(GitError::Scope);
    }
    Ok(root)
}

pub(super) fn execute(
    repository: &Path,
    cancel: &crate::runtime::CancellationToken,
) -> GitResult<Value> {
    let root = authorized_root(
        repository,
        env::var_os("ORCHESTRIX_GIT_EXECUTOR_PROBE_ROOT"),
    )?;
    if cancel.is_cancelled() {
        return Err(GitError::Pending);
    }
    reserve(&SAMPLES)?;
    execute_authorized(&root, cancel)
}
fn reserve(samples: &AtomicU64) -> GitResult<()> {
    if samples
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
            (n < MAX_SAMPLES).then(|| n + 1)
        })
        .is_err()
    {
        return Err(GitError::Limit);
    }
    Ok(())
}
fn execute_authorized(root: &Path, cancel: &crate::runtime::CancellationToken) -> GitResult<Value> {
    if cancel.is_cancelled() || current().is_some() {
        return Err(GitError::Pending);
    }
    let core = file_observation(&env::current_exe()?)?;
    if cancel.is_cancelled() {
        return Err(GitError::Pending);
    }
    let started = Instant::now();
    let state = Rc::new(RefCell::new(State {
        cancel: Some(cancel.clone()),
        started: Some(started),
        ..State::default()
    }));
    ACTIVE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
    let guard = Guard;
    let result = (|| {
        let executor =
            GitExecutor::new()?.with_read_budget(started, SEQUENCE_LIMIT, cancel.clone());
        executor.check_read_budget()?;
        executor.ensure_supported(root)?;
        executor.inspect_head(root)
    })();
    let elapsed = micros(started.elapsed());
    drop(guard);
    let state = state.borrow();
    let report = json!({"schemaVersion":1,"kind":"git-executor-probe","authority":false,"modelCalled":false,
        "compatibilityInspection":false,"sampleLimitPerProcess":MAX_SAMPLES,"corePid":std::process::id(),"core":core,
        "cwd":root,"selectedGit":state.selected,"validatedVersion":state.version,
        "versionProvenance":"existing-constructor-admission","commands":state.commands,
        "sequenceMicros":elapsed,"head":result.as_ref().ok(),"probeSucceeded":result.is_ok(),
        "error":result.err().map(|e|e.to_string()),"nativeCallsInterruptible":false,
        "identityObservationIsExecutionAuthority":false,"retry":false});
    if serde_json::to_vec(&report)
        .map_err(|_| GitError::Limit)?
        .len()
        > 65_536
    {
        return Err(GitError::OutputLimit);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_opt_in_rejects_before_discovery_or_spawn() {
        assert_eq!(
            authorized_root(Path::new("not-an-absolute-root"), None),
            Err(GitError::Scope)
        );
        assert!(current().is_none());
    }
    #[test]
    fn guard_clears_capture_without_installing_a_global_observer() {
        assert!(current().is_none());
        ACTIVE.with(|s| *s.borrow_mut() = Some(Rc::new(RefCell::new(State::default()))));
        {
            let _guard = Guard;
            assert!(current().is_some());
        }
        assert!(current().is_none());
    }

    #[test]
    fn reservation_has_two_samples_without_retry_or_counter_wraparound() {
        let samples = AtomicU64::new(0);
        reserve(&samples).unwrap();
        reserve(&samples).unwrap();
        assert_eq!(reserve(&samples), Err(GitError::Limit));
        assert_eq!(samples.load(Ordering::SeqCst), 2);
        let exhausted = AtomicU64::new(u64::MAX);
        assert_eq!(reserve(&exhausted), Err(GitError::Limit));
        assert_eq!(exhausted.load(Ordering::SeqCst), u64::MAX);
    }

    #[test]
    fn cancelled_probe_does_not_discover_or_spawn() {
        let cancel = crate::runtime::CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            execute_authorized(Path::new("absent"), &cancel),
            Err(GitError::Pending)
        );
        assert!(current().is_none());
    }

    #[test]
    fn cancellation_during_selected_file_identity_io_prevents_version_spawn() {
        let cancel = crate::runtime::CancellationToken::new();
        let state = Rc::new(RefCell::new(State {
            cancel: Some(cancel.clone()),
            ..State::default()
        }));
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
        let _guard = Guard;
        assert_eq!(
            selected_with(None, || {
                cancel.cancel();
                Ok(json!({"authority":false}))
            }),
            Err(GitError::Pending)
        );
        assert!(state.borrow().commands.is_empty());
        assert!(state.borrow().selected.is_none());
        assert_eq!(
            selected_with(None, || panic!("cancelled read must not start")),
            Err(GitError::Pending)
        );
    }

    #[test]
    fn expired_identity_read_cannot_start_version_or_renew_the_sequence() {
        let cancel = crate::runtime::CancellationToken::new();
        let started = Instant::now();
        let state = Rc::new(RefCell::new(State {
            cancel: Some(cancel.clone()),
            started: Some(started),
            ..State::default()
        }));
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
        let _guard = Guard;
        let (observed_start, limit, _) = read_budget().unwrap();
        assert_eq!(observed_start, started);
        assert_eq!(limit, SEQUENCE_LIMIT);
        assert_eq!(
            selected_with(None, || {
                state.borrow_mut().started = Some(Instant::now() - SEQUENCE_LIMIT);
                Ok(json!({"authority":false}))
            }),
            Err(GitError::Deadline)
        );
        assert!(state.borrow().commands.is_empty());
        assert!(state.borrow().selected.is_none());
        assert_eq!(
            selected_with(None, || panic!("expired identity read must not start")),
            Err(GitError::Deadline)
        );
        // The constructor receives this same budget before its first command;
        // even its normal command guard refuses before touching cwd or spawn.
        let executor = GitExecutor {
            binary: "absent".into(),
            private: PathBuf::new(),
            scope: None,
            read_budget: read_budget(),
        };
        assert!(matches!(
            executor.run(Path::new("absent"), &strings(&["--version"])),
            Err(GitError::Deadline)
        ));
        assert!(state.borrow().commands.is_empty());
    }

    #[test]
    fn command_capture_hashes_environment_and_distinguishes_spawn_failure() {
        let state = Rc::new(RefCell::new(State::default()));
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
        let _guard = Guard;
        let mut command = Command::new("fixed-git");
        command
            .env_clear()
            .env("GIT_CONFIG_GLOBAL", "fixture-private-value")
            .args(["rev-parse", "--verify", "HEAD^{commit}"]);
        let mut probe =
            CommandProbe::begin(&command, false, 256, Duration::from_secs(15), None).unwrap();
        probe.spawn_failed(Duration::from_millis(7));
        drop(probe);
        let state = state.borrow();
        let report = &state.commands[0];
        assert_eq!(report["childPid"], Value::Null);
        assert_eq!(report["spawnCallMicros"], 7000);
        assert_eq!(report["spawnError"], "process-creation-failed");
        assert_eq!(report["firstOutputMicros"], Value::Null);
        assert_eq!(report["image"], Value::Null);
        assert!(
            !serde_json::to_string(report)
                .unwrap()
                .contains("fixture-private-value")
        );
        assert_eq!(report["deadlineMs"], 15000);
        assert_eq!(report["outputLimitBytes"], 256);
    }

    #[test]
    fn elapsed_creation_budget_skips_native_query_before_cleanup() {
        let state = Rc::new(RefCell::new(State::default()));
        ACTIVE.with(|slot| *slot.borrow_mut() = Some(state.clone()));
        let _guard = Guard;
        let command = Command::new("fixed-git");
        let mut probe =
            CommandProbe::begin(&command, false, 256, Duration::from_secs(15), None).unwrap();
        probe.observe_with(Duration::from_secs(15), Duration::from_secs(15), || {
            panic!("expired command must not query image")
        });
        assert_eq!(
            probe.value["image"]["unavailable"],
            "deadline-before-image-observation"
        );
        assert!(probe.value["imageObservationMicros"].is_null());
    }

    #[test]
    fn file_identity_hash_and_root_scope_are_bounded_and_read_only() {
        let temp = canonical_directory(&env::temp_dir()).unwrap();
        let parent = temp.join(format!(
            "orchestrix-probe-test-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&parent).unwrap();
        let root = parent.join("repository");
        fs::create_dir(&root).unwrap();
        let file = root.join("opaque.bin");
        fs::write(&file, b"private\0fixture").unwrap();
        assert_eq!(
            authorized_root(&root, Some(root.as_os_str().into())).unwrap(),
            canonical_directory(&root).unwrap()
        );
        assert_eq!(
            authorized_root(&root, Some(temp.as_os_str().into())),
            Err(GitError::Scope)
        );
        let observed = file_observation(&file).unwrap();
        assert_eq!(observed["bytes"], 15);
        assert_eq!(
            observed["sha256"],
            format!("{:x}", Sha256::digest(b"private\0fixture"))
        );
        assert_eq!(observed["authority"], false);
        assert!(
            !serde_json::to_string(&observed)
                .unwrap()
                .contains("fixture")
        );
        fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_len(32 * 1_048_576 + 1)
            .unwrap();
        assert_eq!(file_observation(&file), Err(GitError::Limit));
        fs::remove_file(file).unwrap();
        fs::remove_dir(root).unwrap();
        fs::remove_dir(parent).unwrap();
    }
}
