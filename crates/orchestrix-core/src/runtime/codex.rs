use super::rpc::{DiagnosticReason as R, DiagnosticStage as S, Rpc, diagnosed, diagnostic};
use super::{CancellationToken, RuntimeError, RuntimeResult};
use crate::runtime_contract::{
    EventOrigin, Limits, RuntimeDescriptor, RuntimeObserver, RuntimeRequest, assert_admission,
    normalize_descriptor,
};
use orchestrix_process::{OwnedProcess, ProcessLimits, SpawnSpec, StreamState, TreeState};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Inspected protocol revisions; another version requires schema inspection and validation.
pub const SUPPORTED_CODEX_VERSIONS: [&str; 3] = [
    "0.162.0-alpha.2",
    "0.162.0-alpha.17.2",
    "0.164.0-alpha.11.1",
];
/// Inspected stable outputSchema/agentMessage profiles; not proof of account entitlement.
pub const SUPPORTED_STRUCTURED_CODEX_VERSIONS: &[&str] =
    &["0.162.0-alpha.17.2", "0.164.0-alpha.11.1"];
const SYNTHETIC_VERSION: &str = "synthetic-rust-1";

fn global_metadata_notification(version: &str, method: &str) -> bool {
    matches!(
        method,
        "account/updated"
            | "account/rateLimits/updated"
            | "configWarning"
            | "deprecationNotice"
            | "windows/worldWritableWarning"
            | "remoteControl/status/changed"
    ) || (method == "skills/changed"
        && matches!(
            version,
            "0.162.0-alpha.17.2" | "0.164.0-alpha.11.1" | SYNTHETIC_VERSION
        ))
}

fn read_state_profile(version: &str) -> bool {
    matches!(version, "0.164.0-alpha.11.1" | SYNTHETIC_VERSION)
}

fn valid_read_state(params: &Value, thread: &str) -> bool {
    if !params.is_object()
        || params["threadId"] != thread
        || ["thread", "turn", "turnId"]
            .iter()
            .any(|key| params.get(*key).is_some())
    {
        return false;
    }
    let state = &params["readState"];
    if !state.is_object() || !native(state["revision"].as_str()) {
        return false;
    }
    match state.get("firstUnread") {
        None | Some(Value::Null) => true,
        Some(position) if position.is_object() => match position["type"].as_str() {
            Some("threadStart") => position.get("turnId").is_none(),
            Some("turn") => native(position["turnId"].as_str()),
            _ => false,
        },
        _ => false,
    }
}

pub fn neutral_effort(version: &str, native: &str) -> Option<&'static str> {
    if !SUPPORTED_CODEX_VERSIONS.contains(&version) && version != SYNTHETIC_VERSION {
        return None;
    }
    match native {
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "xhigh" => Some("maximum"),
        _ => None,
    }
}
pub fn native_effort(version: &str, neutral: &str) -> Option<&'static str> {
    if !SUPPORTED_CODEX_VERSIONS.contains(&version) && version != SYNTHETIC_VERSION {
        return None;
    }
    match neutral {
        "minimal" => Some("minimal"),
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "maximum" => Some("xhigh"),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeLaunch {
    executable: PathBuf,
    codex_home: PathBuf,
    cwd: PathBuf,
    version: String,
    fixture_arguments: Option<Vec<OsString>>,
    job_scope: Option<orchestrix_process::named_job::JobScope>,
    pub process_limits: ProcessLimits,
    pub request_timeout: Duration,
    pub turn_timeout: Duration,
    pub interrupt_grace: Duration,
}
impl RuntimeLaunch {
    /// Existing login is read from this explicitly bound auth store; no login or config write occurs.
    pub fn codex(executable: PathBuf, codex_home: PathBuf, cwd: PathBuf, version: String) -> Self {
        Self {
            executable,
            codex_home,
            cwd,
            version,
            fixture_arguments: None,
            job_scope: None,
            process_limits: ProcessLimits::default(),
            request_timeout: Duration::from_secs(15),
            turn_timeout: Duration::from_secs(90),
            interrupt_grace: Duration::from_secs(5),
        }
    }
    /// Explicit synthetic provenance; fixtures can never be reported as provider validation.
    pub fn fixture(
        executable: PathBuf,
        arguments: Vec<OsString>,
        codex_home: PathBuf,
        cwd: PathBuf,
    ) -> Self {
        let mut launch = Self::codex(executable, codex_home, cwd, SYNTHETIC_VERSION.into());
        launch.fixture_arguments = Some(arguments);
        launch
    }
    pub(crate) fn matches_registration(
        &self,
        executable: &Path,
        home: &Path,
        version: &str,
    ) -> bool {
        self.version == version
            && self.executable.canonicalize().ok() == executable.canonicalize().ok()
            && self.codex_home.canonicalize().ok() == home.canonicalize().ok()
            && self.executable.canonicalize().is_ok()
            && self.codex_home.canonicalize().is_ok()
    }
    pub fn is_synthetic(&self) -> bool {
        self.fixture_arguments.is_some()
    }
    /// The structured-result schema was inspected on this pinned native profile.
    pub fn structured_results_supported(&self) -> bool {
        self.is_synthetic() || SUPPORTED_STRUCTURED_CODEX_VERSIONS.contains(&self.version.as_str())
    }
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
    /// A new process/metadata admission will validate the selected owned worktree.
    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = cwd;
        self
    }
    /// Host-owned scope persisted before the adapter may create any process.
    pub fn with_job_scope(mut self, scope: orchestrix_process::named_job::JobScope) -> Self {
        self.job_scope = Some(scope);
        self
    }
    fn owned_process(&self, spec: SpawnSpec) -> RuntimeResult<OwnedProcess> {
        Ok(if let Some(scope) = &self.job_scope {
            OwnedProcess::spawn_identified(spec, scope)?
        } else {
            OwnedProcess::spawn(spec)?
        })
    }
    fn validate(&self) -> RuntimeResult<()> {
        if (!self.is_synthetic() && !SUPPORTED_CODEX_VERSIONS.contains(&self.version.as_str()))
            || (self.is_synthetic() && self.version != SYNTHETIC_VERSION)
        {
            return Err(RuntimeError::new("version", "not-sent"));
        }
        if !self.executable.is_absolute()
            || !self.executable.is_file()
            || !self.codex_home.is_absolute()
            || !self.codex_home.is_dir()
            || !self.cwd.is_absolute()
            || !self.cwd.is_dir()
            || self.request_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(15)
            || self.turn_timeout.is_zero()
            || self.turn_timeout > Duration::from_secs(90)
            || self.interrupt_grace.is_zero()
            || self.interrupt_grace > Duration::from_secs(10)
        {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        for path in [&self.codex_home, &self.cwd, &self.executable] {
            reject_links(path)?;
        }
        Ok(())
    }
    pub(crate) fn metadata_scope(&self) -> RuntimeResult<String> {
        self.validate()?;
        let value = json!({"profile":"codex-local-v1","executable":std::fs::canonicalize(&self.executable).map_err(|_| RuntimeError::new("policy","not-sent"))?,"home":std::fs::canonicalize(&self.codex_home).map_err(|_| RuntimeError::new("policy","not-sent"))?,"cwd":std::fs::canonicalize(&self.cwd).map_err(|_| RuntimeError::new("policy","not-sent"))?,"version":self.version,"fixtureArguments":self.fixture_arguments});
        Ok(format!(
            "{:x}",
            Sha256::digest(value.to_string().as_bytes())
        ))
    }
    pub(super) fn version(&self) -> &str {
        &self.version
    }
    fn spawn(&self, mcp_names: &[String]) -> RuntimeResult<Rpc> {
        self.validate()?;
        let arguments = if let Some(arguments) = &self.fixture_arguments {
            arguments.clone()
        } else {
            let mut arguments: Vec<OsString> = [
                "--no-daemon",
                "-c",
                "model_provider=\"openai\"",
                "-c",
                "openai_base_url=\"\"",
                "-c",
                "chatgpt_base_url=\"https://chatgpt.com/backend-api/\"",
                "-c",
                "analytics.enabled=false",
                "-c",
                "web_search=\"disabled\"",
                "-c",
                "features.hooks=false",
                "-c",
                "features.plugins=false",
                "-c",
                "features.apps=false",
                "-c",
                "features.multi_agent=false",
                "-c",
                "agents.enabled=false",
                "-c",
                "notify=[]",
                "-c",
                "windows.sandbox=\"mxc\"",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            for name in mcp_names {
                arguments.extend([
                    "-c".into(),
                    format!("mcp_servers.{name}.enabled=false").into(),
                ]);
            }
            arguments.extend(["app-server".into(), "--listen".into(), "stdio://".into()]);
            arguments
        };
        let mut environment = runtime_environment();
        environment.retain(|(key, _)| !key.to_string_lossy().eq_ignore_ascii_case("CODEX_HOME"));
        environment.push(("CODEX_HOME".into(), self.codex_home.as_os_str().into()));
        Ok(Rpc::new(self.owned_process(SpawnSpec {
            executable: self.executable.clone(),
            arguments,
            cwd: self.cwd.clone(),
            environment,
            limits: self.process_limits,
        })?))
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeModelPresentation {
    pub id: String,
    pub label: String,
    pub native_efforts: Vec<String>,
}
#[derive(Clone)]
pub struct RuntimeMetadata {
    pub descriptor: RuntimeDescriptor,
    pub observed_at_ms: u64,
    /// Configuration-backed restrictions; adapter rechecks the effective per-thread response.
    pub controls: Vec<String>,
    pub synthetic: bool,
    /// Display-only rows from the same official catalog read; not admission authority.
    pub model_presentation: Vec<RuntimeModelPresentation>,
    pub capacity: Box<crate::capacity::CapacityObservation>,
    account_fingerprint: String,
}
impl std::fmt::Debug for RuntimeMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeMetadata")
            .field("descriptor", &self.descriptor)
            .field("observed_at_ms", &self.observed_at_ms)
            .field("controls", &self.controls)
            .field("synthetic", &self.synthetic)
            .finish_non_exhaustive()
    }
}
impl RuntimeMetadata {
    pub fn usage_available_at(&self, now_ms: u64, source_until: u64) -> bool {
        self.capacity.usage_allowed() == Some(true)
            && now_ms >= self.observed_at_ms
            && now_ms < self.capacity.valid_until(source_until)
            && self.descriptor.as_json()["connection"]["availability"] == "available"
    }
    /// Compares opaque adapter-owned identity; no email/account ID leaves the adapter.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.account_fingerprint == other.account_fingerprint
            && self.descriptor.as_json()["connection"]["id"]
                == other.descriptor.as_json()["connection"]["id"]
            && self.descriptor.as_json()["connection"]["revision"]
                == other.descriptor.as_json()["connection"]["revision"]
            && self.descriptor.as_json()["runtimeVersion"]
                == other.descriptor.as_json()["runtimeVersion"]
            && self.synthetic == other.synthetic
    }
    /// Opaque account/read observation fence, not proof of provider account identity.
    /// Routing fields in 0.164 are an additional observation fence, not proof of
    /// immutable identity, independent quota or backend billing guarantees.
    pub fn observation_fingerprint(&self) -> &str {
        &self.account_fingerprint
    }
    pub fn identity_assurance(&self) -> &'static str {
        if self.synthetic {
            "synthetic"
        } else {
            "observed-label"
        }
    }
    /// Historical connection observation is checked without exposing native account data.
    pub fn matches_binding(&self, binding: &SessionBinding) -> bool {
        self.account_fingerprint == binding.account_fingerprint
            && self.descriptor.as_json()["connection"]["id"] == binding.connection_id
            && self.descriptor.as_json()["connection"]["revision"] == binding.revision
            && self.descriptor.as_json()["runtimeVersion"] == binding.runtime_version
    }
}
/// Adapter-owned mapping. The domain only sees local connection/session/attempt IDs.
#[derive(Clone)]
pub struct SessionBinding {
    connection_id: String,
    revision: u64,
    session_id: String,
    native_thread: String,
    account_fingerprint: String,
    runtime_version: String,
}
impl std::fmt::Debug for SessionBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionBinding")
            .field("connection_id", &self.connection_id)
            .field("revision", &self.revision)
            .field("session_id", &self.session_id)
            .field("runtime_version", &self.runtime_version)
            .finish_non_exhaustive()
    }
}
impl SessionBinding {
    pub fn checkpoint(&self) -> Value {
        json!({"version":1,"connectionId":self.connection_id,"connectionRevision":self.revision,"sessionId":self.session_id,"nativeThread":self.native_thread,"accountFingerprint":self.account_fingerprint,"runtimeVersion":self.runtime_version})
    }
    /// Separate native and local sessions on the same explicitly bound connection.
    pub fn independent_of(&self, source: &Self) -> bool {
        self.connection_id == source.connection_id
            && self.revision == source.revision
            && self.account_fingerprint == source.account_fingerprint
            && self.runtime_version == source.runtime_version
            && self.session_id != source.session_id
            && self.native_thread != source.native_thread
    }
    pub fn restore(value: &Value) -> RuntimeResult<Self> {
        if value.as_object().is_none_or(|map| map.len() != 7)
            || value["version"] != 1
            || !local(value["connectionId"].as_str())
            || !local(value["sessionId"].as_str())
            || !native(value["nativeThread"].as_str())
            || value["connectionRevision"]
                .as_u64()
                .is_none_or(|revision| revision == 0)
            || value["accountFingerprint"]
                .as_str()
                .is_none_or(|text| text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()))
            || value["runtimeVersion"]
                .as_str()
                .is_none_or(|v| !SUPPORTED_CODEX_VERSIONS.contains(&v) && v != SYNTHETIC_VERSION)
        {
            return Err(RuntimeError::new("ownership", "not-sent"));
        }
        Ok(Self {
            connection_id: value["connectionId"].as_str().unwrap().into(),
            revision: value["connectionRevision"].as_u64().unwrap(),
            session_id: value["sessionId"].as_str().unwrap().into(),
            native_thread: value["nativeThread"].as_str().unwrap().into(),
            account_fingerprint: value["accountFingerprint"].as_str().unwrap().into(),
            runtime_version: value["runtimeVersion"].as_str().unwrap().into(),
        })
    }
}
pub struct AttemptExecution {
    pub request: RuntimeRequest,
    pub prompt: String,
    pub base_instructions: String,
    pub developer_instructions: String,
    pub resume: Option<SessionBinding>,
}
#[derive(Clone, Debug)]
pub struct RuntimeReport {
    pub observer_checkpoint: Value,
    pub binding: Option<SessionBinding>,
    pub fault: Option<RuntimeError>,
    /// Adapter-private route/shape evidence, without provider content or identity values.
    /// This is not a normalized domain event or evidence of a native terminal.
    pub adapter_diagnostic: Option<Value>,
    pub dispatch: &'static str,
    /// Bounded final assistant result requested through the official output schema channel.
    /// Never includes reasoning, tool output or a conversation transcript.
    pub structured_output: Option<String>,
    pub tree: Option<TreeState>,
    pub streams: StreamState,
    pub synthetic: bool,
}
pub struct CodexAdapter {
    launch: RuntimeLaunch,
    rpc: Rpc,
    metadata: RuntimeMetadata,
    models: BTreeMap<String, NativeModel>,
    account_fingerprint: String,
    used: bool,
    observation_started: Instant,
}
struct NativeModel {
    name: String,
    efforts: Vec<String>,
}
struct Discovery {
    metadata: RuntimeMetadata,
    models: BTreeMap<String, NativeModel>,
    account_fingerprint: String,
    mcp_names: Vec<String>,
    ready: bool,
}
impl CodexAdapter {
    fn assert_usage_current(&self) -> RuntimeResult<()> {
        let now = self.metadata.observed_at_ms.saturating_add(
            self.observation_started
                .elapsed()
                .as_millis()
                .min(u64::MAX as u128) as u64,
        );
        if !self
            .metadata
            .usage_available_at(now, self.metadata.observed_at_ms.saturating_add(60_000))
        {
            return Err(RuntimeError::new("availability-unknown", "not-sent"));
        }
        Ok(())
    }
    /// Authorized read-only metadata; this never creates a thread, sends a turn or refreshes login.
    pub fn connect(
        launch: RuntimeLaunch,
        connection_id: &str,
        revision: u64,
        now_ms: u64,
        cancel: &CancellationToken,
    ) -> RuntimeResult<Self> {
        let observation_started = Instant::now();
        if !local(Some(connection_id)) || revision == 0 {
            return Err(RuntimeError::new("ownership", "not-sent"));
        }
        launch.validate()?;
        if !launch.is_synthetic() {
            verify_binary_version(&launch, cancel)?;
        }
        let mut rpc = launch.spawn(&[])?;
        initialize(&mut rpc, &launch, cancel)?;
        let mut discovery = discover(&mut rpc, &launch, connection_id, revision, now_ms, cancel)?;
        if !discovery.ready && !discovery.mcp_names.is_empty() {
            // Disable inherited MCP resources in this child only, before any thread exists.
            rpc.process.terminate(Duration::from_secs(3))?;
            drop(rpc);
            rpc = launch.spawn(&discovery.mcp_names)?;
            initialize(&mut rpc, &launch, cancel)?;
            discovery = discover(&mut rpc, &launch, connection_id, revision, now_ms, cancel)?;
        }
        if discovery.metadata.capacity.usage_allowed() != Some(true) {
            return Err(RuntimeError::new(
                if discovery.metadata.capacity.usage_allowed() == Some(false) {
                    "unavailable"
                } else {
                    "availability-unknown"
                },
                "not-sent",
            ));
        }
        if !discovery.ready {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        Ok(Self {
            launch,
            rpc,
            metadata: discovery.metadata,
            models: discovery.models,
            account_fingerprint: discovery.account_fingerprint,
            used: false,
            observation_started,
        })
    }
    pub fn metadata(&self) -> &RuntimeMetadata {
        &self.metadata
    }
    /// Observation-only path. Negative permission never constructs an executable adapter.
    pub(crate) fn read_metadata(
        launch: RuntimeLaunch,
        connection_id: &str,
        revision: u64,
        now_ms: u64,
        cancel: &CancellationToken,
    ) -> RuntimeResult<RuntimeMetadata> {
        if !local(Some(connection_id)) || revision == 0 {
            return Err(RuntimeError::new("ownership", "not-sent"));
        }
        launch.validate()?;
        if !launch.is_synthetic() {
            verify_binary_version(&launch, cancel)?;
        }
        let mut rpc = launch.spawn(&[])?;
        initialize(&mut rpc, &launch, cancel)?;
        let mut d = discover(&mut rpc, &launch, connection_id, revision, now_ms, cancel)?;
        if d.metadata.capacity.usage_allowed() == Some(true) && !d.ready && !d.mcp_names.is_empty()
        {
            rpc.process.terminate(Duration::from_secs(3))?;
            drop(rpc);
            rpc = launch.spawn(&d.mcp_names)?;
            initialize(&mut rpc, &launch, cancel)?;
            d = discover(&mut rpc, &launch, connection_id, revision, now_ms, cancel)?;
        }
        if d.metadata.capacity.usage_allowed() == Some(true) && !d.ready {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        stop_process(&mut rpc)?;
        Ok(d.metadata)
    }
    pub fn shutdown(&mut self) -> RuntimeResult<TreeState> {
        stop_process(&mut self.rpc)
    }
    /// The callback must durably record observations before returning. No DB writer is held here.
    /// A failed/uncertain admission is never retried by this adapter.
    pub fn execute<F>(
        &mut self,
        execution: AttemptExecution,
        cancel: &CancellationToken,
        sink: F,
    ) -> RuntimeResult<RuntimeReport>
    where
        F: FnMut(&Value, EventOrigin) -> RuntimeResult<()>,
    {
        self.execute_result(execution, None, cancel, sink)
    }
    /// Structured results still require host-side schema and evidence validation.
    pub fn execute_structured<F>(
        &mut self,
        execution: AttemptExecution,
        output_schema: Value,
        cancel: &CancellationToken,
        sink: F,
    ) -> RuntimeResult<RuntimeReport>
    where
        F: FnMut(&Value, EventOrigin) -> RuntimeResult<()>,
    {
        if !self.launch.structured_results_supported() {
            return Err(RuntimeError::new("unsupported", "not-sent"));
        }
        if !output_schema.is_object() || output_schema.to_string().len() > 16_384 {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        self.execute_result(execution, Some(output_schema), cancel, sink)
    }
    fn execute_result<F>(
        &mut self,
        execution: AttemptExecution,
        output_schema: Option<Value>,
        cancel: &CancellationToken,
        mut sink: F,
    ) -> RuntimeResult<RuntimeReport>
    where
        F: FnMut(&Value, EventOrigin) -> RuntimeResult<()>,
    {
        if self.used {
            return Err(RuntimeError::new("busy", "not-sent"));
        }
        self.assert_usage_current()?;
        self.used = true;
        let mut observer = RuntimeObserver::new(
            self.metadata.descriptor.clone(),
            execution.request.clone(),
            Limits::default(),
        )?;
        let mut binding = None;
        let mut structured_output = None;
        let attempt_result = self.execute_inner(
            &execution,
            output_schema.as_ref(),
            &mut structured_output,
            cancel,
            &mut observer,
            &mut binding,
            &mut sink,
        );
        let mut fault = attempt_result.err();
        if let Some(error) = &fault
            && observer.snapshot()["fatal"] != true
        {
            let dispatch = if self.rpc.turn_submitted {
                error.dispatch
            } else {
                "not-sent"
            };
            let emitted = emit(
                &mut observer,
                "runtime.error",
                json!({"error":{"code":error.code,"phase":if self.rpc.turn_submitted {"running"} else {"admission"},"dispatch":dispatch,"retryable":false},"fatal":(["protocol","ownership","sequence","version","budget"].contains(&error.code))}),
                EventOrigin::Runtime,
                &mut sink,
            );
            if emitted.is_err() {
                fault = Some(RuntimeError::new("transport-loss", "unknown"));
            }
        }
        // Remote terminal, stream EOF, root exit and Job exhaustion are distinct evidence.
        let tree = match stop_process(&mut self.rpc) {
            Ok(tree) => Some(tree),
            Err(error) => {
                if fault.is_none() {
                    fault = Some(error);
                }
                None
            }
        };
        let streams = self.rpc.process.wait_streams(Duration::from_secs(1));
        if let Some(tree) = tree {
            if let Some(code) = tree.root_exit_code
                && emit(
                    &mut observer,
                    "process.exited",
                    json!({"role":"runtime","code":code,"signal":if code == 137 {"killed"} else {"none"}}),
                    EventOrigin::Supervisor,
                    &mut sink,
                ).is_err() {
                    fault = Some(RuntimeError::new("transport-loss", "unknown"));
            }
            if emit(
                &mut observer,
                "process.streams-observed",
                json!({"role":"runtime","streams":{"stdin":streams.stdin_closed,"stdout":streams.stdout_closed,"stderr":streams.stderr_closed,"faulted":self.rpc.process.health().is_err()}}),
                EventOrigin::Supervisor,
                &mut sink,
            ).is_err() { fault = Some(RuntimeError::new("transport-loss", "unknown")); }
            if emit(
                &mut observer,
                "process.tree-observed",
                json!({"ownership":if tree.owned {"confirmed"} else {"unknown"},"source":"owned-job","termination":if tree.root_exit_code.is_some() && tree.active_processes == 0 {"confirmed"} else {"unknown"},"activeCount":tree.active_processes,"evidenceRef":"rust-owned-job"}),
                EventOrigin::Supervisor,
                &mut sink,
            ).is_err() { fault = Some(RuntimeError::new("transport-loss", "unknown")); }
        }
        let dispatch = self.rpc.turn_dispatch;
        let adapter_diagnostic = fault.as_ref().map(|error| {
            error
                .adapter_diagnostic
                .clone()
                .unwrap_or_else(|| diagnostic(S::Attempt, R::OperationFailed, None))
        });
        Ok(RuntimeReport {
            observer_checkpoint: observer.checkpoint(),
            binding,
            fault,
            adapter_diagnostic,
            dispatch,
            structured_output,
            tree,
            streams,
            synthetic: self.launch.is_synthetic(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn execute_inner<F>(
        &mut self,
        execution: &AttemptExecution,
        output_schema: Option<&Value>,
        structured_output: &mut Option<String>,
        cancel: &CancellationToken,
        observer: &mut RuntimeObserver,
        binding: &mut Option<SessionBinding>,
        sink: &mut F,
    ) -> RuntimeResult<()>
    where
        F: FnMut(&Value, EventOrigin) -> RuntimeResult<()>,
    {
        let request = execution.request.as_json();
        let selected = &request["requested"];
        if request["connectionId"] != self.metadata.descriptor.as_json()["connection"]["id"]
            || request["connectionRevision"]
                != self.metadata.descriptor.as_json()["connection"]["revision"]
            || request["billingMode"] != "subscription-only"
            || selected["network"] != "disabled"
            || selected["approval"] != "never"
            || cancel.is_cancelled()
        {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        if execution.prompt.is_empty()
            || execution.prompt.len() > 32_768
            || execution.base_instructions.len() > 8192
            || execution.developer_instructions.len() > 8192
        {
            return Err(RuntimeError::new("budget", "not-sent"));
        }
        assert_admission(&self.metadata.descriptor, &execution.request, None)?;
        let model = if selected["modelPolicy"] == "specific" {
            Some(
                self.models
                    .get(selected["modelId"].as_str().unwrap())
                    .ok_or_else(|| RuntimeError::new("ownership", "not-sent"))?,
            )
        } else {
            None
        };
        let effort = if [json!("auto"), json!("runtime-default")].contains(&selected["reasoning"]) {
            None
        } else {
            Some(
                native_effort(
                    &self.launch.version,
                    selected["reasoning"].as_str().unwrap(),
                )
                .ok_or_else(|| RuntimeError::new("unsupported", "not-sent"))?,
            )
        };
        if let (Some(model), Some(effort)) = (model, effort)
            && !model.efforts.iter().any(|native| native == effort)
        {
            return Err(RuntimeError::new("unsupported", "not-sent"));
        }
        let writable = selected["sandbox"] == "workspace-write";
        let mut params = json!({"cwd":self.launch.cwd,"modelProvider":"openai","approvalPolicy":"never","approvalsReviewer":"user","sandbox":if writable {"workspace-write"} else {"read-only"},"baseInstructions":execution.base_instructions,"developerInstructions":execution.developer_instructions,"config":{"web_search":"disabled","mcp_servers":{},"sandbox_workspace_write":{"network_access":false,"writable_roots":[],"exclude_tmpdir_env_var":true,"exclude_slash_tmp":true}}});
        if let Some(model) = model {
            params["model"] = json!(model.name);
        }
        if let Some(effort) = effort {
            params["config"]["model_reasoning_effort"] = json!(effort);
        }
        let method = if let Some(previous) = &execution.resume {
            if previous.connection_id != request["connectionId"]
                || previous.revision != request["connectionRevision"]
                || previous.session_id != request["sessionId"]
                || previous.account_fingerprint != self.account_fingerprint
                || previous.runtime_version != self.launch.version
            {
                return Err(RuntimeError::new("ownership", "not-sent"));
            }
            params["threadId"] = json!(previous.native_thread);
            "thread/resume"
        } else {
            params["ephemeral"] = json!(false);
            "thread/start"
        };
        let configuration =
            self.rpc
                .request(method, params, self.launch.request_timeout, cancel)?;
        let native_thread = validate_configuration(
            &configuration,
            &self.launch.cwd,
            writable,
            execution.resume.as_ref(),
        )
        .map_err(|error| diagnosed(error, S::Configuration, R::ConfigurationRejected, None))?;
        if let Some(model) = model
            && configuration["model"] != model.name
        {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        if let Some(effort) = effort
            && configuration["reasoningEffort"] != effort
        {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        *binding = Some(SessionBinding {
            connection_id: request["connectionId"].as_str().unwrap().into(),
            revision: request["connectionRevision"].as_u64().unwrap(),
            session_id: request["sessionId"].as_str().unwrap().into(),
            native_thread: native_thread.clone(),
            account_fingerprint: self.account_fingerprint.clone(),
            runtime_version: self.launch.version.clone(),
        });
        let effective = effective(
            &configuration,
            &self.models,
            &self.launch.version,
            writable,
            self.launch.is_synthetic(),
        );
        emit(
            observer,
            "configuration.observation-recorded",
            json!({"effective":effective}),
            EventOrigin::Runtime,
            sink,
        )?;
        let mut turn_params = json!({"threadId":native_thread,"input":[{"type":"text","text":execution.prompt,"text_elements":[]}],"sandboxPolicy":configuration["sandbox"],"approvalPolicy":"never","approvalsReviewer":"user"});
        // Private QA correlation only on this explicit synthetic executable
        // profile. Native providers never receive these test-only fields.
        if self.launch.fixture_arguments.as_ref().is_some_and(|args| {
            args.windows(2)
                .any(|pair| pair[0] == "--scenario" && pair[1] == "qa-controlled")
        }) {
            turn_params["_orchestrixQa"] = json!({"connectionId":request["connectionId"],"sessionId":request["sessionId"],"attemptId":request["attemptId"]});
        }
        if let Some(schema) = output_schema {
            turn_params["outputSchema"] = schema.clone();
        }
        if let Some(effort) = effort {
            turn_params["effort"] = json!(effort);
        }
        self.assert_usage_current()?;
        let acknowledgement = self.rpc.request(
            "turn/start",
            turn_params,
            self.launch.request_timeout,
            cancel,
        )?;
        let native_turn = acknowledgement["turn"]["id"]
            .as_str()
            .filter(|value| native(Some(value)))
            .ok_or_else(|| {
                diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    S::Admission,
                    R::InvalidTurnId,
                    None,
                )
            })?
            .to_owned();
        emit(
            observer,
            "attempt.admission-observed",
            json!({}),
            EventOrigin::Runtime,
            sink,
        )?;
        let began = Instant::now();
        let mut interrupted_at = None;
        let mut interrupt_acked = false;
        let mut output_bytes = 0u64;
        let mut expired_drain_frames = 0usize;
        let mut forced_after_grace = false;
        loop {
            let cancelled = cancel.is_cancelled() || began.elapsed() >= self.launch.turn_timeout;
            if cancelled && interrupted_at.is_none() {
                if observer.snapshot()["phase"] == "running"
                    || observer.snapshot()["phase"] == "accepted"
                {
                    emit(
                        observer,
                        "attempt.interrupt-requested",
                        json!({}),
                        EventOrigin::Runtime,
                        sink,
                    )?;
                }
                // A cancellation request is a new bounded control operation, not inference or retry.
                let control_cancel = CancellationToken::new();
                let interrupt = self.rpc.request(
                    "turn/interrupt",
                    json!({"threadId":native_thread,"turnId":native_turn}),
                    self.launch.interrupt_grace,
                    &control_cancel,
                );
                interrupt_acked = interrupt.is_ok();
                emit(
                    observer,
                    "attempt.interrupt-result",
                    json!({"acknowledged":interrupt_acked}),
                    EventOrigin::Runtime,
                    sink,
                )?;
                interrupted_at = Some(Instant::now());
            }
            let interrupt_expired = interrupted_at
                .is_some_and(|instant| instant.elapsed() >= self.launch.interrupt_grace);
            if interrupt_expired {
                if !forced_after_grace {
                    // No producer can prolong the grace by continuously
                    // supplying frames. Stop only the owned Job, then retain
                    // already received facts within the fixed queue budget.
                    self.rpc.process.terminate(Duration::from_secs(3))?;
                    forced_after_grace = true;
                }
                if expired_drain_frames >= 128 {
                    return Err(RuntimeError::new("timeout", "unknown"));
                }
            }
            let message = if let Some(message) = self.rpc.notifications.pop_front() {
                Some(message)
            } else {
                // Durable observer callbacks may consume wall time after the
                // child already emitted its terminal. At grace expiry only
                // drain immediately available bounded frames; never wait more
                // for a provider response or discard a terminal already here.
                self.rpc.next(if interrupt_expired {
                    Duration::ZERO
                } else {
                    Duration::from_millis(5)
                })?
            };
            let Some(message) = message else {
                if interrupt_expired {
                    return Err(RuntimeError::new("timeout", "unknown"));
                }
                continue;
            };
            if interrupt_expired {
                expired_drain_frames += 1;
            }
            if message.get("id").is_some() {
                if message.get("method").is_some() {
                    self.rpc.deny_request(&message)?;
                    continue;
                }
                return Err(diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    S::Notification,
                    R::UnexpectedResponse,
                    Some(&message),
                ));
            }
            let method = message["method"].as_str().ok_or_else(|| {
                diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    S::Notification,
                    R::MissingMethod,
                    Some(&message),
                )
            })?;
            let params = &message["params"];
            if method == "thread/readState/changed" && read_state_profile(&self.launch.version) {
                if !valid_read_state(params, &native_thread) {
                    let code = if params.is_object()
                        && params["threadId"] == native_thread
                        && ["thread", "turn", "turnId"]
                            .iter()
                            .all(|key| params.get(*key).is_none())
                    {
                        "protocol"
                    } else {
                        "ownership"
                    };
                    return Err(diagnosed(
                        RuntimeError::new(code, "unknown"),
                        S::Notification,
                        R::ThreadMetadataScope,
                        Some(&message),
                    ));
                }
                // A read cursor may reference an older turn. It establishes no
                // active execution, terminal, configuration or permission fact.
                continue;
            }
            // Both pinned v2 metadata notifications have a nullable threadId.
            // Their contents grant no execution/capability/control evidence.
            // A warning without a target is also safely discarded as info;
            // MCP requires the schema's explicit target (possibly null).
            // Any targeted message must belong to this thread with no aliases.
            if matches!(method, "warning" | "mcpServer/startupStatus/updated") {
                if !params.is_object()
                    || (method == "mcpServer/startupStatus/updated"
                        && params.get("threadId").is_none())
                    || params
                        .get("threadId")
                        .is_some_and(|id| !id.is_null() && *id != native_thread)
                    || params.get("thread").is_some()
                    || params.get("turnId").is_some()
                    || params.get("turn").is_some()
                {
                    return Err(diagnosed(
                        RuntimeError::new("ownership", "unknown"),
                        S::Notification,
                        if method == "warning" {
                            R::WarningScope
                        } else {
                            R::McpMetadataScope
                        },
                        Some(&message),
                    ));
                }
                continue;
            }
            // Global metadata is not evidence of this attempt's execution. The
            // transport still bounds its frames/bytes; never adopt its contents
            // as an auth, quota, configuration or terminal observation here.
            if global_metadata_notification(&self.launch.version, method) {
                // The pinned remote-control status is global, with no thread
                // or turn identities. Its remote identifiers/content are never
                // adopted or retained, and no remote-control command is sent.
                // On alpha.17.2, skills/changed is only a local skill-cache
                // invalidation. This adapter has no skills/list cache to
                // refresh: discard it without another RPC or any new authority.
                if !params.is_object()
                    || params.get("threadId").is_some()
                    || params.get("thread").is_some()
                    || params.get("turnId").is_some()
                    || params.get("turn").is_some()
                {
                    return Err(diagnosed(
                        RuntimeError::new("ownership", "unknown"),
                        S::Notification,
                        R::GlobalScope,
                        Some(&message),
                    ));
                }
                if method == "account/updated" && params["authMode"] != "chatgpt" {
                    return Err(diagnosed(
                        RuntimeError::new("authentication", "unknown"),
                        S::Notification,
                        R::AuthenticationChanged,
                        Some(&message),
                    ));
                }
                continue;
            }
            // thread/started has a nested Thread, not a top-level threadId. A
            // queued start must match the thread/configuration RPC accepted
            // above, and does not establish that the turn itself started.
            if method == "thread/started" {
                if params["thread"]["id"] != native_thread
                    || params["thread"]["cwd"] != configuration["thread"]["cwd"]
                    || params["thread"]["modelProvider"] != configuration["thread"]["modelProvider"]
                    || params
                        .get("threadId")
                        .is_some_and(|id| *id != native_thread)
                    || params.get("turnId").is_some()
                    || params.get("turn").is_some()
                {
                    return Err(diagnosed(
                        RuntimeError::new("ownership", "unknown"),
                        S::Notification,
                        R::ThreadStartedScope,
                        Some(&message),
                    ));
                }
                continue;
            }
            let relevant = matches!(
                method,
                "turn/started" | "turn/completed" | "item/agentMessage/delta" | "item/completed"
            );
            if params["threadId"] != native_thread {
                // A contradictory execution frame is a protocol ownership
                // failure, even when a later valid terminal follows it.
                if relevant || params.get("threadId").is_some() {
                    return Err(diagnosed(
                        RuntimeError::new("ownership", "unknown"),
                        S::Notification,
                        R::ThreadScope,
                        Some(&message),
                    ));
                }
                return Err(diagnosed(
                    RuntimeError::new("protocol", "unknown"),
                    S::Notification,
                    R::UnsupportedGlobal,
                    Some(&message),
                ));
            }
            if relevant {
                let actual_turn = if method.starts_with("turn/") {
                    &params["turn"]["id"]
                } else {
                    &params["turnId"]
                };
                if *actual_turn != native_turn
                    || (method.starts_with("turn/")
                        && params.get("turnId").is_some_and(|id| *id != native_turn))
                    || (method == "item/agentMessage/delta"
                        && params["turn"]
                            .get("id")
                            .is_some_and(|id| *id != native_turn))
                {
                    return Err(diagnosed(
                        RuntimeError::new("ownership", "unknown"),
                        S::Notification,
                        R::TurnScope,
                        Some(&message),
                    ));
                }
            }
            match method {
                "turn/started" => {
                    if observer.snapshot()["startObserved"] != true {
                        emit(
                            observer,
                            "attempt.start-observed",
                            json!({}),
                            EventOrigin::Runtime,
                            sink,
                        )
                        .map_err(|error| {
                            diagnosed(error, S::Observer, R::ObservationRejected, Some(&message))
                        })?;
                    }
                }
                "item/agentMessage/delta" => {
                    let delta = params["delta"].as_str().ok_or_else(|| {
                        diagnosed(
                            RuntimeError::new("protocol", "unknown"),
                            S::Notification,
                            R::DeltaType,
                            Some(&message),
                        )
                    })?;
                    output_bytes = output_bytes.saturating_add(delta.len() as u64);
                    if output_bytes > 65_536 {
                        return Err(diagnosed(
                            RuntimeError::new("budget", "unknown"),
                            S::Notification,
                            R::OutputBudget,
                            Some(&message),
                        ));
                    }
                }
                "item/completed"
                    if output_schema.is_some() && params["item"]["type"] == "agentMessage" =>
                {
                    let item = &params["item"];
                    if item.get("phase").is_some_and(|phase| phase == "commentary") {
                        continue;
                    }
                    let text = item["text"]
                        .as_str()
                        .filter(|text| !text.is_empty() && text.len() <= 65_536)
                        .ok_or_else(|| RuntimeError::new("protocol", "sent"))?;
                    if structured_output.is_some()
                        || item
                            .get("phase")
                            .is_some_and(|phase| !phase.is_null() && phase != "final_answer")
                    {
                        return Err(RuntimeError::new("protocol", "sent"));
                    }
                    output_bytes = output_bytes.max(text.len() as u64);
                    *structured_output = Some(text.to_owned());
                }
                "turn/completed" => {
                    let outcome = params["turn"]["status"]
                        .as_str()
                        .filter(|status| ["completed", "failed", "interrupted"].contains(status))
                        .ok_or_else(|| {
                            diagnosed(
                                RuntimeError::new("protocol", "unknown"),
                                S::Notification,
                                R::TerminalStatus,
                                Some(&message),
                            )
                        })?;
                    if output_bytes > 0 {
                        emit(
                            observer,
                            "output.observed",
                            json!({"channel":"assistant","bytes":output_bytes}),
                            EventOrigin::Runtime,
                            sink,
                        )
                        .map_err(|error| {
                            diagnosed(error, S::Observer, R::ObservationRejected, Some(&message))
                        })?;
                    }
                    let error = if params["turn"]["error"].is_null() {
                        Value::Null
                    } else {
                        json!({"code":native_error(&params["turn"]["error"]),"phase":"running","dispatch":"sent","retryable":false})
                    };
                    emit(
                        observer,
                        "attempt.terminal-observed",
                        json!({"outcome":outcome,"error":error}),
                        EventOrigin::Runtime,
                        sink,
                    )
                    .map_err(|error| {
                        diagnosed(error, S::Observer, R::ObservationRejected, Some(&message))
                    })?;
                    if cancelled && outcome == "completed" && !interrupt_acked {
                        return Err(RuntimeError::new("timeout", "sent"));
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
    }
}
fn initialize(
    rpc: &mut Rpc,
    launch: &RuntimeLaunch,
    cancel: &CancellationToken,
) -> RuntimeResult<()> {
    let response = rpc.request("initialize", json!({"clientInfo":{"name":"orchestrix","title":"Orchestrix","version":"0.1.0"},"capabilities":{"experimentalApi":false}}), launch.request_timeout, cancel)?;
    if !response.is_object() || response.get("userAgent").and_then(Value::as_str).is_none() {
        return Err(RuntimeError::new("protocol", "not-sent"));
    }
    rpc.notify("initialized", json!({}))
}
fn stop_process(rpc: &mut Rpc) -> RuntimeResult<TreeState> {
    rpc.process.close_input();
    let began = Instant::now();
    loop {
        // A closed stdin gives a cooperative app-server a short, bounded exit window.
        let tree = rpc.process.tree()?;
        if tree.root_exit_code.is_some() && tree.active_processes == 0 {
            return Ok(tree);
        }
        if began.elapsed() >= Duration::from_millis(250) {
            break;
        }
        let _ = rpc.process.receive(Duration::from_millis(5));
    }
    rpc.process
        .terminate(Duration::from_secs(3))
        .map_err(Into::into)
}
fn discover(
    rpc: &mut Rpc,
    launch: &RuntimeLaunch,
    connection_id: &str,
    revision: u64,
    now_ms: u64,
    cancel: &CancellationToken,
) -> RuntimeResult<Discovery> {
    let account = rpc.request(
        "account/read",
        json!({"refreshToken":false}),
        launch.request_timeout,
        cancel,
    )?;
    if account["account"]["type"] != "chatgpt" || account["requiresOpenaiAuth"] != true {
        return Err(RuntimeError::new("authentication", "not-sent"));
    }
    let account_fingerprint =
        account_observation_fingerprint(&account, &launch.version, launch.is_synthetic())?;
    let quota = rpc.request(
        "account/rateLimits/read",
        json!({"supportsLunaReserve":false,"excludeResetCreditDetails":true}),
        launch.request_timeout,
        cancel,
    )?;
    let capacity = super::codex_capacity::normalize(
        &quota,
        connection_id,
        revision,
        &launch.version,
        now_ms,
        launch.is_synthetic(),
    );
    capacity
        .validate()
        .map_err(|_| RuntimeError::new("protocol", "not-sent"))?;
    if capacity.usage_allowed() != Some(true) {
        // Account/read succeeded, but configuration/catalog were not inspected.
        // Do not claim subscription routing, permissions or executable readiness.
        let descriptor = normalize_descriptor(
            &json!({"version":{"major":1,"minor":1},"adapterId":if launch.is_synthetic(){"synthetic-codex-app-server"}else{"codex-app-server"},"runtimeVersion":launch.version,"connection":{"id":connection_id,"revision":revision,"authentication":"authenticated","billing":"unknown","availability":if capacity.usage_allowed()==Some(false){"unavailable"}else{"unknown"}},"catalog":{"completeness":"partial"},"models":[],"capabilities":{}}),
        )?;
        return Ok(Discovery {
            metadata: RuntimeMetadata {
                descriptor,
                observed_at_ms: now_ms,
                controls: Vec::new(),
                synthetic: launch.is_synthetic(),
                model_presentation: Vec::new(),
                capacity: Box::new(capacity),
                account_fingerprint: account_fingerprint.clone(),
            },
            models: BTreeMap::new(),
            account_fingerprint,
            mcp_names: Vec::new(),
            ready: false,
        });
    }
    let configuration = rpc.request(
        "config/read",
        json!({"cwd":launch.cwd,"includeLayers":false}),
        launch.request_timeout,
        cancel,
    )?;
    let config = &configuration["config"];
    // A custom definition with the built-in provider's name can change routing
    // or credential selection. This first subscription-only profile rejects it.
    let providers = &config["model_providers"];
    if (!providers.is_null() && !providers.is_object())
        || providers
            .as_object()
            .and_then(|map| map.get("openai"))
            .is_some_and(|provider| {
                provider
                    .as_object()
                    .is_none_or(|definition| !definition.is_empty())
            })
    {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    if config["model_provider"] != "openai"
        || config["chatgpt_base_url"] != "https://chatgpt.com/backend-api/"
        || ![Value::Null, json!("")].contains(&config["openai_base_url"])
        || config["windows"]["sandbox"] != "mxc"
        || config["web_search"] != "disabled"
    {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    if config["features"]["hooks"] != false
        || config["features"]["plugins"] != false
        || config["features"]["apps"] != false
        || config["features"]["multi_agent"] != false
        || config["agents"]["enabled"] != false
        || config["notify"]
            .as_array()
            .is_none_or(|items| !items.is_empty())
    {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    let servers = config["mcp_servers"].as_object();
    if !config["mcp_servers"].is_null() && servers.is_none() {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    let mut mcp_names = Vec::new();
    let mut ready = true;
    if let Some(servers) = servers {
        if servers.len() > 32 {
            return Err(RuntimeError::new("budget", "not-sent"));
        }
        for (name, server) in servers {
            if !local(Some(name)) {
                return Err(RuntimeError::new("policy", "not-sent"));
            }
            if server["enabled"] != false {
                ready = false;
                mcp_names.push(name.clone());
            }
        }
    }
    let mut rows = Vec::new();
    let mut models = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut cursors = BTreeSet::new();
    let mut cursor = None;
    let mut complete = false;
    let began = Instant::now();
    for _ in 0..10 {
        let remaining = launch.request_timeout.saturating_sub(began.elapsed());
        if remaining.is_zero() {
            break;
        }
        let mut params = json!({"includeHidden":false,"limit":64});
        if let Some(cursor) = &cursor {
            params["cursor"] = json!(cursor);
        }
        let page = match rpc.request("model/list", params, remaining, cancel) {
            Ok(page) => page,
            Err(error) if rows.is_empty() => return Err(error),
            Err(_) => break,
        };
        let Some(values) = page["data"].as_array() else {
            return Err(RuntimeError::new("protocol", "not-sent"));
        };
        if values.len() > 64 {
            return Err(RuntimeError::new("budget", "not-sent"));
        }
        for row in values {
            let Some(name) = row["model"].as_str().filter(|name| native(Some(name))) else {
                return Err(RuntimeError::new("protocol", "not-sent"));
            };
            if !names.insert(name.to_owned()) || rows.len() >= 64 {
                return Err(RuntimeError::new("protocol", "not-sent"));
            }
            let id = format!("model-{:x}", Sha256::digest(name.as_bytes()))[..54].to_owned();
            let effort_values = row["supportedReasoningEfforts"].as_array();
            let mut efforts = Vec::new();
            let mut levels = Vec::new();
            let mut all_mapped = effort_values.is_some_and(|items| items.len() <= 8);
            if let Some(values) = effort_values {
                for value in values.iter().take(8) {
                    let value = value.as_str().or_else(|| value["reasoningEffort"].as_str());
                    if let Some(value) = value {
                        efforts.push(value.into());
                        if let Some(level) = neutral_effort(&launch.version, value) {
                            if !levels.contains(&level) {
                                levels.push(level);
                            }
                        } else {
                            all_mapped = false;
                        }
                    } else {
                        all_mapped = false;
                    }
                }
            }
            rows.push(json!({"id":id,"availability":"unknown","reasoningLevels":if all_mapped {json!(levels)} else {Value::Null},"capabilities":{"modelSelection":{"support":"supported","evidence":{"source":"catalog","scope":"model","reference":id}},"reasoning":{"support":if all_mapped {if levels.is_empty() {"unsupported"} else {"supported"}} else {"unknown"},"evidence":{"source":"catalog","scope":"model","reference":id}}}}));
            models.insert(
                id,
                NativeModel {
                    name: name.into(),
                    efforts,
                },
            );
        }
        if page["nextCursor"].is_null() && page.get("nextCursor").is_some() {
            complete = true;
            break;
        }
        let Some(next) = page["nextCursor"].as_str().filter(|cursor| {
            !cursor.is_empty() && cursor.len() <= 4096 && !cursor.chars().any(char::is_control)
        }) else {
            break;
        };
        if !cursors.insert(next.to_owned()) {
            break;
        }
        cursor = Some(next.to_owned());
    }
    let descriptor = normalize_descriptor(
        &json!({"version":{"major":1,"minor":1},"adapterId":if launch.is_synthetic() {"synthetic-codex-app-server"} else {"codex-app-server"},"runtimeVersion":launch.version,"connection":{"id":connection_id,"revision":revision,"authentication":"authenticated","billing":"subscription","availability":"available"},"catalog":{"completeness":if complete {"complete"} else {"partial"}},"models":rows,"capabilities":{"resume":{"support":"supported","evidence":{"source":"declared","reference":"pinned-protocol"}},"contextManagement":{"support":"runtime-managed","evidence":{"source":"declared","reference":"runtime-context"}}}}),
    )?;
    Ok(Discovery {
        metadata: RuntimeMetadata {
            descriptor,
            capacity: Box::new(capacity),
            observed_at_ms: now_ms,
            controls: [
                "control:read-only",
                "control:workspace-write",
                "control:network-disabled",
                "control:approval-never",
                "control:native-delegation-disabled",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            synthetic: launch.is_synthetic(),
            model_presentation: models
                .iter()
                .map(|(id, m)| RuntimeModelPresentation {
                    id: id.clone(),
                    label: m.name.clone(),
                    native_efforts: m.efforts.clone(),
                })
                .collect(),
            account_fingerprint: account_fingerprint.clone(),
        },
        models,
        account_fingerprint,
        mcp_names,
        ready,
    })
}
fn account_observation_fingerprint(
    account: &Value,
    version: &str,
    synthetic: bool,
) -> RuntimeResult<String> {
    let email = account["account"]["email"]
        .as_str()
        .filter(|text| !text.is_empty());
    let fixture_identity = synthetic
        .then(|| account["account"]["accountId"].as_str())
        .flatten();
    if email.is_none() && fixture_identity.is_none() {
        return Err(RuntimeError::new("ownership", "not-sent"));
    }
    // Keep older profiles' exact projection/hash semantics. Native fixture-only
    // accountId is never adopted from an actual provider response.
    let mut projection =
        json!({"email":email,"id":fixture_identity,"plan":account["account"]["planType"]});
    if version == "0.164.0-alpha.11.1" || (synthetic && account.get("workspaceRouting").is_some()) {
        let routing = match account.get("workspaceRouting") {
            None | Some(Value::Null) => Value::Null,
            Some(routing)
                if routing.is_object()
                    && native(routing["chatgptAccountId"].as_str())
                    && native(routing["backendOrigin"].as_str())
                    && matches!(
                        routing["accountRoutingOverride"].as_str(),
                        Some("NO_CONSTRAINT" | "us" | "us_cr")
                    ) =>
            {
                json!({"chatgptAccountId":routing["chatgptAccountId"],"backendOrigin":routing["backendOrigin"],"accountRoutingOverride":routing["accountRoutingOverride"]})
            }
            _ => return Err(RuntimeError::new("protocol", "not-sent")),
        };
        // Not an origin allowlist or a source of billing/entitlement authority.
        // Only this opaque hash leaves the adapter, never the routing/ID values.
        projection["workspaceRouting"] = routing;
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(projection.to_string().as_bytes())
    ))
}
fn validate_configuration(
    value: &Value,
    cwd: &Path,
    writable: bool,
    resume: Option<&SessionBinding>,
) -> RuntimeResult<String> {
    let thread = &value["thread"];
    let id = thread["id"]
        .as_str()
        .filter(|value| native(Some(value)))
        .ok_or_else(|| RuntimeError::new("protocol", "not-sent"))?;
    let sandbox = &value["sandbox"];
    if resume.is_some_and(|previous| previous.native_thread != id)
        || thread["ephemeral"] != false
        || thread["status"]["type"] != "idle"
        || thread
            .get("sessionId")
            .is_some_and(|session| !session.is_null() && *session != id)
        || !thread["forkedFromId"].is_null()
        || thread["turns"]
            .as_array()
            .is_some_and(|turns| turns.iter().any(|turn| turn["status"] == "inProgress"))
        || !same_path(value["cwd"].as_str(), cwd)
        || !same_path(thread["cwd"].as_str(), cwd)
        || value["modelProvider"] != "openai"
        || thread["modelProvider"] != "openai"
        || value["approvalPolicy"] != "never"
        || value["approvalsReviewer"] != "user"
        || !native(value["model"].as_str())
        || value["reasoningEffort"].as_str().is_none()
        || sandbox["type"]
            != if writable {
                "workspaceWrite"
            } else {
                "readOnly"
            }
        || sandbox["networkAccess"] != false
    {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    if writable
        && (sandbox["excludeTmpdirEnvVar"] != true
            || sandbox["excludeSlashTmp"] != true
            || sandbox["writableRoots"].as_array().is_none_or(|roots| {
                roots.len() > 32 || roots.iter().any(|root| !same_path(root.as_str(), cwd))
            }))
    {
        return Err(RuntimeError::new("policy", "not-sent"));
    }
    Ok(id.into())
}
fn effective(
    configuration: &Value,
    models: &BTreeMap<String, NativeModel>,
    version: &str,
    writable: bool,
    synthetic: bool,
) -> Value {
    let mut result = json!({});
    let source = if synthetic {
        "synthetic"
    } else {
        "configuration"
    };
    let mut put = |key: &str, value: Value| {
        result[key] = json!({"state":"known","value":value,"evidence":{"source":source,"scope":"attempt","reference":"resolved-configuration"}});
    };
    if let Some((id, _)) = models
        .iter()
        .find(|(_, model)| configuration["model"] == model.name)
    {
        put("model", json!(id));
    }
    if let Some(level) = configuration["reasoningEffort"]
        .as_str()
        .and_then(|native| neutral_effort(version, native))
    {
        put("reasoning", json!(level));
    }
    put(
        "sandbox",
        json!(if writable {
            "workspace-write"
        } else {
            "read-only"
        }),
    );
    put("network", json!("disabled"));
    put("approval", json!("never"));
    result
}
fn emit<F>(
    observer: &mut RuntimeObserver,
    kind: &str,
    fields: Value,
    origin: EventOrigin,
    sink: &mut F,
) -> RuntimeResult<()>
where
    F: FnMut(&Value, EventOrigin) -> RuntimeResult<()>,
{
    let snapshot = observer.snapshot();
    let mut frame = snapshot["scope"].clone();
    frame["version"] = json!({"major":1,"minor":1});
    frame["sequence"] = json!(snapshot["budget"]["frames"].as_u64().unwrap() + 1);
    frame["type"] = json!(kind);
    for (key, value) in fields.as_object().unwrap() {
        frame[key] = value.clone();
    }
    let event = observer
        .accept(&frame.to_string(), origin)
        .map_err(|error| diagnosed(error.into(), S::Observer, R::ObservationRejected, None))?;
    sink(&event, origin)
        .map_err(|error| diagnosed(error, S::Observer, R::ObservationRejected, None))
}
fn native_error(error: &Value) -> &'static str {
    match error["codexErrorInfo"]
        .as_str()
        .or_else(|| error["codexErrorInfo"]["type"].as_str())
    {
        Some("usageLimitExceeded" | "rateLimitExceeded" | "sessionBudgetExceeded") => "rate-limit",
        Some("unauthorized") => "authentication",
        _ => "unavailable",
    }
}
fn local(value: Option<&str>) -> bool {
    value.is_some_and(|text| {
        !text.is_empty()
            && text.len() <= 80
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}
fn native(value: Option<&str>) -> bool {
    value.is_some_and(|text| {
        !text.is_empty() && text.len() <= 512 && !text.chars().any(char::is_control)
    })
}
fn same_path(value: Option<&str>, expected: &Path) -> bool {
    value
        .and_then(|path| std::fs::canonicalize(path).ok())
        .zip(std::fs::canonicalize(expected).ok())
        .is_some_and(|(left, right)| left == right)
}
fn reject_links(path: &Path) -> RuntimeResult<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        ) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| RuntimeError::new("policy", "not-sent"))?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(RuntimeError::new("policy", "not-sent"));
            }
        }
        if metadata.file_type().is_symlink() {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
    }
    Ok(())
}
fn runtime_environment() -> Vec<(OsString, OsString)> {
    const ALLOWED: &[&str] = &[
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "SYSTEMDRIVE",
        "COMSPEC",
        "TEMP",
        "TMP",
        "TMPDIR",
        "USERPROFILE",
        "HOME",
        "HOMEDRIVE",
        "HOMEPATH",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "COMMONPROGRAMFILES",
        "PROCESSOR_ARCHITECTURE",
        "NUMBER_OF_PROCESSORS",
        "LANG",
        "LC_ALL",
    ];
    std::env::vars_os()
        .filter(|(key, _)| {
            ALLOWED
                .iter()
                .any(|allowed| key.to_string_lossy().eq_ignore_ascii_case(allowed))
        })
        .collect()
}
fn verify_binary_version(launch: &RuntimeLaunch, cancel: &CancellationToken) -> RuntimeResult<()> {
    let mut process = launch.owned_process(SpawnSpec {
        executable: launch.executable.clone(),
        arguments: vec!["--version".into()],
        cwd: launch.cwd.clone(),
        environment: runtime_environment(),
        limits: ProcessLimits {
            lifetime: Duration::from_secs(10),
            max_output_bytes: 8192,
            max_write_bytes: 1024,
        },
    })?;
    process.close_input();
    let begin = Instant::now();
    let mut output = Vec::new();
    loop {
        if cancel.is_cancelled() {
            return Err(RuntimeError::new("timeout", "not-sent"));
        }
        if let Some(orchestrix_process::ProcessEvent::Stdout(bytes)) =
            process.receive(Duration::from_millis(5))?
        {
            output.extend(bytes);
        }
        let tree = process.tree()?;
        let streams = process.streams();
        if tree.root_exit_code.is_some() && streams.stdout_closed && streams.stderr_closed {
            while let Some(event) = process.receive(Duration::ZERO)? {
                if let orchestrix_process::ProcessEvent::Stdout(bytes) = event {
                    output.extend(bytes);
                }
            }
            if tree.root_exit_code != Some(0) {
                return Err(RuntimeError::new("version", "not-sent"));
            }
            // Root exit/EOF are separate from tree termination. Windows can
            // retain owned console helpers briefly; close only this Job and
            // prove it empty instead of misreporting a correct binary version.
            let stopped = if tree.active_processes == 0 {
                tree
            } else {
                process.terminate(Duration::from_secs(3))?
            };
            if !stopped.owned || stopped.active_processes != 0 {
                return Err(RuntimeError::new("ownership", "not-sent"));
            }
            break;
        }
        if begin.elapsed() >= Duration::from_secs(10) {
            return Err(RuntimeError::new("timeout", "not-sent"));
        }
    }
    let output = String::from_utf8(output).map_err(|_| RuntimeError::new("version", "not-sent"))?;
    if output.trim() != format!("codex-cli {}", launch.version) {
        return Err(RuntimeError::new("version", "not-sent"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SYNTHETIC_VERSION, account_observation_fingerprint, global_metadata_notification,
        read_state_profile, valid_read_state,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};

    #[test]
    fn skills_invalidation_is_limited_to_the_inspected_profile() {
        for version in [
            "0.162.0-alpha.17.2",
            "0.164.0-alpha.11.1",
            SYNTHETIC_VERSION,
        ] {
            assert!(global_metadata_notification(version, "skills/changed"));
        }
        for version in ["0.162.0-alpha.2", "0.162.0-alpha.18", "unknown"] {
            assert!(!global_metadata_notification(version, "skills/changed"));
        }
        for version in ["0.162.0-alpha.2", "0.162.0-alpha.17.2", SYNTHETIC_VERSION] {
            for method in ["account/updated", "remoteControl/status/changed"] {
                assert!(global_metadata_notification(version, method));
            }
            assert!(!global_metadata_notification(version, "skills/unknown"));
        }
    }
    #[test]
    fn read_state_shape_and_version_cannot_establish_execution_or_alias_scope() {
        assert!(read_state_profile("0.164.0-alpha.11.1"));
        for v in [
            "0.162.0-alpha.2",
            "0.162.0-alpha.17.2",
            "0.164.0-alpha.12",
            "unknown",
        ] {
            assert!(!read_state_profile(v));
        }
        let valid = json!({"threadId":"own-thread","readState":{"revision":"opaque-revision","firstUnread":{"type":"turn","turnId":"older-turn"}}});
        assert!(valid_read_state(&valid, "own-thread"));
        for key in ["thread", "turn", "turnId"] {
            let mut bad = valid.clone();
            bad[key] = serde_json::Value::Null;
            assert!(!valid_read_state(&bad, "own-thread"));
        }
        assert!(!valid_read_state(&valid, "foreign"));
        for state in [
            json!({}),
            json!({"revision":42}),
            json!({"revision":""}),
            json!({"revision":"r","firstUnread":false}),
            json!({"revision":"r","firstUnread":{"type":"turn"}}),
            json!({"revision":"r","firstUnread":{"type":"threadStart","turnId":"old"}}),
        ] {
            let mut bad = valid.clone();
            bad["readState"] = state;
            assert!(!valid_read_state(&bad, "own-thread"));
        }
        for position in [serde_json::Value::Null, json!({"type":"threadStart"})] {
            let mut value = valid.clone();
            value["readState"]["firstUnread"] = position;
            assert!(valid_read_state(&value, "own-thread"));
        }
    }
    #[test]
    fn routing_fence_is_bounded_optional_versioned_and_not_quota_or_billing_authority() {
        let base = json!({"account":{"type":"chatgpt","email":"synthetic@example.invalid","planType":"pro"}});
        let old = account_observation_fingerprint(&base, "0.162.0-alpha.17.2", false).unwrap();
        let old_projection = json!({"email":"synthetic@example.invalid","id":null,"plan":"pro"});
        assert_eq!(
            old,
            format!(
                "{:x}",
                Sha256::digest(old_projection.to_string().as_bytes())
            )
        );
        let mut routed = base.clone();
        routed["workspaceRouting"] = json!({"chatgptAccountId":"workspace-one","backendOrigin":"https://chatgpt.com/backend-api/","accountRoutingOverride":"NO_CONSTRAINT"});
        assert_eq!(
            old,
            account_observation_fingerprint(&routed, "0.162.0-alpha.17.2", false).unwrap()
        );
        let current =
            account_observation_fingerprint(&routed, "0.164.0-alpha.11.1", false).unwrap();
        for (key, value) in [
            ("chatgptAccountId", "workspace-two"),
            ("backendOrigin", "https://example.invalid/changed"),
            ("accountRoutingOverride", "us"),
        ] {
            let mut different = routed.clone();
            different["workspaceRouting"][key] = json!(value);
            assert_ne!(
                current,
                account_observation_fingerprint(&different, "0.164.0-alpha.11.1", false).unwrap()
            );
        }
        let mut volatile = routed.clone();
        volatile["ordinaryUsageAllowed"] = json!(false);
        volatile["workspaceRouting"]["usedPercent"] = json!(99);
        assert_eq!(
            current,
            account_observation_fingerprint(&volatile, "0.164.0-alpha.11.1", false).unwrap()
        );
        assert_ne!(
            current,
            account_observation_fingerprint(&base, "0.164.0-alpha.11.1", false).unwrap()
        );
        let mut null = base.clone();
        null["workspaceRouting"] = serde_json::Value::Null;
        assert_eq!(
            account_observation_fingerprint(&base, "0.164.0-alpha.11.1", false).unwrap(),
            account_observation_fingerprint(&null, "0.164.0-alpha.11.1", false).unwrap()
        );
        for routing in [
            json!(false),
            json!({}),
            json!({"chatgptAccountId":"x","backendOrigin":"y","accountRoutingOverride":"unknown"}),
            json!({"chatgptAccountId":"x".repeat(513),"backendOrigin":"y","accountRoutingOverride":"us"}),
        ] {
            let mut bad = base.clone();
            bad["workspaceRouting"] = routing;
            assert!(account_observation_fingerprint(&bad, "0.164.0-alpha.11.1", false).is_err());
        }
    }
}
