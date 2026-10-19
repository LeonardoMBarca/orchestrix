//! Native API client and filesystem authority. No SQLite or second worker engine.
#[path = "connection_discovery.rs"]
mod connection_discovery;
#[path = "policy_transfer.rs"]
mod policy_transfer;
use crate::daemon_startup::{StartupController, StartupLimits};
use crate::editor::{Editor, EditorRequest};
use orchestrix_core::{
    connections::{ConnectionChange, ConnectionRequest},
    daemon::{API_VERSION, Client, Operation, Request, RuntimeChoice},
    desktop::{DesktopRequest, Settings},
    evaluator::CheckSpec,
    git::GitExecutor,
    integration_records::{ApplicationRequest, ApprovalRequest, IntegrationRequest},
    policy::{TaskClassification, TaskKind},
    review_records::{CorrectionRequest, ReviewBudget, ReviewRequest},
    worker::{ContextFileRequest, ManualTaskRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(test)]
use std::sync::Arc;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Binding {
    id: String,
    label: String,
    executable: PathBuf,
    executable_hash: String,
    home: PathBuf,
    version: String,
    revision: u64,
}
#[derive(Clone)]
struct Prepared {
    task: ManualTaskRequest,
    connection_id: String,
    conversation_id: String,
    corrections: u32,
    settings_fingerprint: String,
}
#[cfg(test)]
type TestTransport = Arc<dyn Fn(Operation) -> Result<Value, String> + Send + Sync>;
pub struct Host {
    pub database: PathBuf,
    pub directory: PathBuf,
    pub editor: Mutex<Editor>,
    bindings: Mutex<HashMap<String, Binding>>,
    prepared: Mutex<HashMap<String, Prepared>>,
    startup: Mutex<StartupController>,
    discovery: Mutex<connection_discovery::Discovery>,
    policy_files: Mutex<policy_transfer::PolicyFiles>,
    close_behavior: Mutex<String>,
    #[cfg(test)]
    test_transport: Option<TestTransport>,
    #[cfg(feature = "test-fixtures")]
    fixture: Option<FixtureConfig>,
}
#[cfg(feature = "test-fixtures")]
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureConfig {
    root: PathBuf,
    executable: PathBuf,
    home: PathBuf,
    repository: PathBuf,
    worktrees: PathBuf,
    #[serde(default)]
    review_scenario: Option<String>,
    #[serde(default)]
    worker_scenario: Option<String>,
    #[serde(default)]
    connections_onboarding: bool,
}
#[cfg(feature = "test-fixtures")]
impl FixtureConfig {
    fn validate_scenarios(&self) -> Result<(), String> {
        if ![
            None,
            Some("review-reject"),
            Some("review-accept"),
            Some("review-invalid"),
            Some("quota-failure"),
            Some("hang"),
            Some("qa-controlled"),
        ]
        .contains(&self.review_scenario.as_deref())
            || ![None, Some("review-implementation"), Some("qa-controlled")]
                .contains(&self.worker_scenario.as_deref())
        {
            return Err("native-fixture-scenario".into());
        }
        Ok(())
    }
    fn scenario(&self, purpose: &str) -> &str {
        match purpose {
            "review" => self.review_scenario.as_deref().unwrap_or("review-reject"),
            "review-corrected" => "review-accept",
            "correction" => "review-correction",
            "implementation" => self
                .worker_scenario
                .as_deref()
                .unwrap_or("review-implementation"),
            _ => "review-implementation",
        }
    }
}
#[cfg(feature = "test-fixtures")]
fn fixture_config() -> Result<Option<FixtureConfig>, String> {
    let Some(path) = std::env::var_os("ORCHESTRIX_DESKTOP_FIXTURE_CONFIG") else {
        return Ok(None);
    };
    let bytes = fs::read(path).map_err(|_| "native-fixture-config")?;
    if bytes.len() > 16384 {
        return Err("native-fixture-config-budget".into());
    }
    let config: FixtureConfig =
        serde_json::from_slice(&bytes).map_err(|_| "native-fixture-config")?;
    let root = config
        .root
        .canonicalize()
        .map_err(|_| "native-fixture-root")?;
    for path in [&config.home, &config.repository, &config.worktrees] {
        if !path
            .canonicalize()
            .map_err(|_| "native-fixture-path")?
            .starts_with(&root)
        {
            return Err("native-fixture-path-scope".into());
        }
    }
    if config.executable.file_name().and_then(|n| n.to_str())
        != Some("orchestrix-runtime-fixture.exe")
    {
        return Err("native-fixture-executable".into());
    }
    config.validate_scenarios()?;
    Ok(Some(config))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskDraft {
    pub conversation_id: String,
    pub project_id: Option<String>,
    pub repository_grant: String,
    pub worktree_grant: String,
    pub connection_id: String,
    #[serde(default)]
    pub connection_revision: Option<u64>,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    pub context_files: Vec<ContextDraft>,
    pub checks: Vec<CheckDraft>,
    pub strategy: String,
    pub preset: String,
    pub model: String,
    pub effort: String,
    pub correction_budget: u32,
    pub fallback: String,
    #[serde(default)]
    pub allow_metadata_read: bool,
    #[serde(default)]
    pub mode: Option<orchestrix_core::policy::DefinitionBinding>,
    #[serde(default)]
    pub profile: Option<orchestrix_core::policy::DefinitionBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextDraft {
    grant: String,
    relative: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckDraft {
    id: String,
    executable: String,
    args: Vec<String>,
    timeout_seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum HostRequest {
    Configurations {
        request: Box<orchestrix_core::configuration::ConfigurationRequest>,
    },
    PreparePolicyExport,
    PreviewPolicyImport {
        import_ref: String,
        mappings: orchestrix_core::policy_portability::ImportMappings,
    },
    ApplyPolicyImport {
        command_id: String,
        expected_revision: u64,
        import_ref: String,
        mappings: orchestrix_core::policy_portability::ImportMappings,
        expected_preview_digest: String,
        confirm: bool,
    },
    ReleasePolicyImport {
        import_ref: String,
    },
    ConfigureConnectionRefresh {
        command_id: String,
        id: String,
        expected_revision: u64,
        expected_refresh_revision: u64,
        settings: orchestrix_core::connection_refresh::RefreshSettings,
    },
    ConnectionRefreshStatus {
        id: Option<String>,
    },
    #[cfg(feature = "test-fixtures")]
    TestFixture,
    Bootstrap,
    Conversation {
        conversation_id: String,
        #[serde(default)]
        after_ordinal: u64,
        #[serde(default = "message_limit")]
        limit: usize,
    },
    CreateProject {
        command_id: String,
        expected_revision: u64,
        id: String,
        name: String,
        directory_grant: String,
    },
    SendMessage {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        project_id: Option<String>,
        content: String,
    },
    SaveDraft {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        content: String,
    },
    AttachConversation {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        project_id: String,
    },
    SaveSettings {
        command_id: String,
        expected_revision: u64,
        settings: Box<Settings>,
    },
    InspectWorker {
        worker_id: String,
    },
    PrepareIntegration {
        command_id: String,
        expected_revision: u64,
        worker_id: String,
        artifact_revision: String,
        target_grant: String,
        integration_grant: String,
        authorize_checks: bool,
    },
    InspectIntegration {
        integration_id: String,
    },
    RevalidateIntegration {
        command_id: String,
        integration_id: String,
    },
    ApproveIntegration {
        command_id: String,
        expected_revision: u64,
        integration_id: String,
        candidate_revision: String,
        authorize: bool,
    },
    ApplyIntegration {
        command_id: String,
        expected_revision: u64,
        integration_id: String,
        candidate_revision: String,
        authorize: bool,
    },
    PrepareTask {
        draft: Box<TaskDraft>,
    },
    ArtifactText {
        artifact_id: String,
        revision: String,
        path: Option<String>,
        offset: usize,
        limit: usize,
    },
    SubmitTask {
        command_id: String,
        preparation_id: String,
        authorize_runtime: bool,
        authorize_checks: bool,
    },
    SaveConnection {
        command_id: String,
        label: String,
        version: String,
        executable_grant: String,
        home_grant: String,
    },
    DetectCodexCandidates,
    InspectRuntimeVersion {
        executable_ref: String,
    },
    LinkConnection {
        command_id: String,
        id: Option<String>,
        expected_revision: Option<u64>,
        name: String,
        version: String,
        executable_ref: String,
        home_ref: String,
        executable_sha256: String,
        allow_metadata_read: bool,
    },
    UpdateConnection {
        command_id: String,
        id: String,
        expected_revision: u64,
        change: String,
        name: Option<String>,
        diagnostic_id: Option<String>,
    },
    DiagnoseConnection {
        command_id: String,
        id: String,
        expected_revision: u64,
        allow_metadata_read: bool,
    },
    CancelConnectionDiagnostic {
        command_id: String,
        id: String,
        expected_revision: u64,
        diagnostic_id: String,
    },
    ConnectionDiagnostics {
        id: String,
    },
    Control {
        action: String,
        command_id: String,
        worker_id: Option<String>,
    },
    Recovery,
    Intervene {
        command_id: String,
        worker_id: String,
        expected_revision: u64,
        action: orchestrix_core::recovery::RecoveryAction,
        authorize: bool,
    },
    SetAsideReview {
        command_id: String,
        review_id: String,
        expected_revision: u64,
        authorize: bool,
    },
    RequestReview {
        command_id: String,
        worker_id: String,
        authorize_runtime: bool,
    },
    AuthorizeCorrection {
        command_id: String,
        worker_id: String,
        review_id: String,
        authorize_runtime: bool,
    },
}
fn id(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
fn message_limit() -> usize {
    16
}
#[cfg(feature = "test-fixtures")]
fn version_for_synthetic(runtime: &RuntimeChoice) -> bool {
    matches!(runtime,RuntimeChoice::Codex{version,..}if version=="synthetic-rust-1")
}
fn stable(prefix: &str, command: &str) -> String {
    format!("{prefix}-{}", &sha(command.as_bytes())[..32])
}
fn err(s: &str) -> String {
    s.into()
}
fn sha(bytes: &[u8]) -> String {
    orchestrix_core::git_records::content_hash(bytes)
}
impl Host {
    pub fn data_directory(default: PathBuf) -> Result<PathBuf, String> {
        #[cfg(feature = "test-fixtures")]
        if let Some(config) = fixture_config()? {
            return Ok(config.root.join("desktop-client"));
        }
        Ok(default)
    }
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&directory).map_err(|_| err("app-data-directory"))?;
        let directory = directory
            .canonicalize()
            .map_err(|_| err("app-data-directory"))?;
        let bindings = HashMap::new();
        Ok(Self {
            database: directory.join("orchestrix.sqlite"),
            directory: directory.clone(),
            editor: Mutex::new(Editor::default()),
            bindings: Mutex::new(bindings),
            prepared: Mutex::new(HashMap::new()),
            startup: Mutex::new(StartupController::new(StartupLimits::new(
                Duration::from_secs(15),
            )?)),
            discovery: Mutex::new(connection_discovery::Discovery::default()),
            policy_files: Mutex::new(policy_transfer::PolicyFiles::default()),
            close_behavior: Mutex::new(
                fs::read_to_string(directory.join("window-preferences.json"))
                    .ok()
                    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                    .and_then(|v| {
                        v["closeBehavior"]
                            .as_str()
                            .filter(|s| ["continue", "suspend"].contains(s))
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "continue".into()),
            ),
            #[cfg(test)]
            test_transport: None,
            #[cfg(feature = "test-fixtures")]
            fixture: fixture_config()?,
        })
    }
    pub fn call(&self, operation: Operation) -> Result<Value, String> {
        #[cfg(test)]
        if let Some(transport) = &self.test_transport {
            return transport(operation);
        }
        let response = Client::new(&self.database)?.request(&Request {
            api_version: API_VERSION,
            operation,
        })?;
        if !response.ok {
            return Err(response.error.unwrap_or_else(|| err("daemon-rejected")));
        }
        Ok(response.result)
    }
    fn desktop(&self, request: DesktopRequest) -> Result<Value, String> {
        self.call(Operation::Desktop {
            request: Box::new(request),
        })
    }
    pub fn connect(&self) -> Result<Value, String> {
        let mut startup = self.startup.lock().map_err(|_| "startup-lock")?;
        let executable = std::env::current_exe().map_err(|_| "desktop-executable")?;
        let parent = executable.parent().ok_or("desktop-directory")?;
        let core = parent.join("orchestrix.exe");
        let keeper = parent.join("orchestrix-supervisor.exe");
        startup
            .connect(&self.database, &core, &keeper, compatible)
            .map(|update| self.startup_value(update.into_value()))
    }
    pub fn observe_startup(&self) -> Result<Value, String> {
        self.startup
            .lock()
            .map_err(|_| "startup-lock")?
            .observe(&self.database, compatible)
            .map(|update| self.startup_value(update.into_value()))
    }
    fn startup_value(&self, value: Value) -> Value {
        // Private QA output only: preserve the bounded child phase trace after
        // the renderer bootstraps, without logging a Snapshot or user content.
        #[cfg(feature = "test-fixtures")]
        if self.fixture.is_some()
            && let Some(diagnostics) = value.get("startupDiagnostics")
        {
            eprintln!(
                "ORCHESTRIX_QA_STARTUP {}",
                json!({"hostPid":std::process::id(),"diagnostics":diagnostics})
            );
        }
        value
    }
    pub fn native_qa_enabled(&self) -> bool {
        #[cfg(feature = "test-fixtures")]
        {
            self.fixture.is_some()
        }
        #[cfg(not(feature = "test-fixtures"))]
        {
            false
        }
    }
    #[cfg(feature = "test-fixtures")]
    pub fn native_qa_root(&self) -> Option<&std::path::Path> {
        self.fixture.as_ref().map(|fixture| fixture.root.as_path())
    }
    pub fn poll(
        &self,
        cursor: orchestrix_core::persistence::DaemonCursor,
    ) -> Result<Value, String> {
        let events = self.call(Operation::Events { cursor, limit: 128 })?;
        let daemon = self.call(Operation::Snapshot)?;
        Ok(
            json!({"events":events["events"],"cursor":events["cursor"],"resyncRequired":events["resyncRequired"]==true,"daemon":daemon}),
        )
    }
    pub fn edit(&self, request: EditorRequest) -> Result<Value, String> {
        let mutating = matches!(
            &request,
            EditorRequest::Save { .. } | EditorRequest::Create { .. }
        );
        let (outcome, path) = {
            let mut editor = self.editor.lock().map_err(|_| "editor-lock")?;
            let path = match &request {
                EditorRequest::Save { document, .. } => Some(editor.document_path(document)?),
                EditorRequest::Create { grant, name } => Some(editor.creation_path(grant, name)?),
                _ => None,
            };
            let outcome = editor.execute(request);
            let opened = match &outcome {
                Ok(value) => value
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|document| editor.document_path(document))
                    .transpose()?,
                Err(_) => None,
            };
            (outcome, path.or(opened))
        };
        // A failed write/create may already have changed disk bytes. The target
        // was authorized before the effect, so conservatively revalidate even
        // when failure happened before writing. Never claim successful creation.
        let mut result = match outcome {
            Ok(value) => value,
            Err(reason) if path.is_some() => json!({"saveError":reason}),
            Err(reason) => return Err(reason),
        };
        if let Some(path) = path {
            result["authorizedPath"] = json!(path);
            result["origin"] = json!("authorized-file");
            result["originObservation"] = Value::Null;
            match self.desktop(DesktopRequest::AffectedWorkers {
                canonical_path: path.to_string_lossy().into(),
            }) {
                Ok(affected) => {
                    // These are historical relationships, never file grants. Do not
                    // canonicalize or open paths obtained from project metadata.
                    let projects = if affected["workerIds"].as_array().is_some_and(Vec::is_empty) {
                        self.desktop(DesktopRequest::Bootstrap).ok()
                    } else {
                        None
                    };
                    let origin = observed_file_origin(&path, &affected, projects.as_ref());
                    result["origin"] = origin["origin"].clone();
                    result["originObservation"] = origin["originObservation"].clone();
                    if !mutating {
                        return Ok(result);
                    }
                    let ids: Vec<String> = affected["workerIds"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|id| id.as_str().map(str::to_owned))
                        .collect();
                    let mut checks = Vec::new();
                    for worker_id in &ids {
                        let command_id = id("save-revalidation");
                        let observation = self.call(Operation::RevalidateArtifact {
                            command_id: command_id.clone(),
                            worker_id: worker_id.clone(),
                        });
                        checks.push(json!({"workerId":worker_id,"commandId":command_id,"accepted":observation.is_ok(),"reason":observation.err()}));
                    }
                    result["affectedWorkers"] = json!(ids);
                    result["revalidation"] = json!(checks);
                    let ids: Vec<String> = affected["integrationIds"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|id| id.as_str().map(str::to_owned))
                        .collect();
                    let mut checks = Vec::new();
                    for integration_id in &ids {
                        let command_id = id("save-integration-revalidation");
                        let observation = self.handle(HostRequest::RevalidateIntegration {
                            command_id: command_id.clone(),
                            integration_id: integration_id.clone(),
                        });
                        checks.push(json!({"integrationId":integration_id,"commandId":command_id,"accepted":observation.is_ok(),"reason":observation.err()}));
                    }
                    result["affectedIntegrations"] = json!(ids);
                    if checks.iter().any(|check| check["accepted"] != true) {
                        result["warning"] = json!(
                            "Candidate revalidation was not confirmed; inspect and revalidate the exact candidate before approval/application."
                        );
                    }
                    result["integrationRevalidation"] = json!(checks);
                }
                Err(_) => {
                    if mutating {
                        result["revalidationPending"] = json!(true);
                        result["integrationRevalidationPending"] = json!(true);
                        result["warning"] = json!(
                            "Artifact/candidate revalidation unavailable; prior evidence is historical."
                        );
                    }
                }
            }
        }
        Ok(result)
    }
    fn binding(&self, id: &str, allow_live: bool, purpose: &str) -> Result<RuntimeChoice, String> {
        let binding = self.linked_binding(id, None)?;
        if connection_discovery::binary_hash(&binding.executable)? != binding.executable_hash {
            return Err(err("runtime-executable-changed-reconfigure"));
        }
        #[cfg(feature = "test-fixtures")]
        if binding.version == "synthetic-rust-1" {
            let fixture = self
                .fixture
                .as_ref()
                .ok_or("native-fixture-config-required")?;
            if fixture
                .executable
                .canonicalize()
                .map_err(|_| "native-fixture-executable")?
                != binding.executable
            {
                return Err("native-fixture-executable".into());
            }
            return Ok(RuntimeChoice::Synthetic {
                executable: binding.executable,
                home: binding.home,
                scenario: fixture.scenario(purpose).into(),
            });
        }
        #[cfg(not(feature = "test-fixtures"))]
        let _ = purpose;
        Ok(RuntimeChoice::Codex {
            executable: binding.executable,
            home: binding.home,
            version: binding.version,
            allow_live,
        })
    }
    fn replay(&self, effect: crate::journal::Effect) -> Result<Value, String> {
        let mut known = None;
        if let Operation::Connections { request } = &effect.operation
            && let ConnectionRequest::Link {
                command_id,
                runtime,
                executable_sha256,
                ..
            } = &**request
        {
            let receipt = self.call(Operation::CommandReceipt {
                command_id: command_id.clone(),
                fingerprint: sha(
                    &serde_json::to_vec(&effect.operation).map_err(|_| "desktop-command-schema")?
                ),
            })?;
            if receipt["found"] == true {
                known = Some(receipt["receipt"].clone());
            } else {
                let executable = match runtime {
                    RuntimeChoice::Codex { executable, .. } => executable,
                    RuntimeChoice::Synthetic { executable, .. } => {
                        #[cfg(not(feature = "test-fixtures"))]
                        {
                            let _ = executable;
                            return Err("synthetic-runtime-not-allowed".into());
                        }
                        #[cfg(feature = "test-fixtures")]
                        {
                            if self.fixture.is_none() {
                                return Err("native-fixture-config-required".into());
                            }
                            executable
                        }
                    }
                };
                validate_runtime_pin(executable, Some(executable_sha256))?;
            }
        }
        let integration_id = match &effect.operation {
            Operation::PrepareIntegration { request, .. } => Some(request.id.clone()),
            Operation::ApproveIntegration { request, .. } => Some(request.integration_id.clone()),
            Operation::ApplyIntegration { request, .. } => Some(request.integration_id.clone()),
            Operation::RevalidateIntegration { integration_id, .. } => Some(integration_id.clone()),
            _ => None,
        };
        if let Operation::PrepareIntegration {
            command_id,
            request,
            ..
        } = &effect.operation
        {
            let receipt = self.call(Operation::CommandReceipt {
                command_id: command_id.clone(),
                fingerprint: sha(
                    &serde_json::to_vec(&effect.operation).map_err(|_| "desktop-command-schema")?
                ),
            })?;
            if receipt["found"] == true {
                known = Some(receipt["receipt"].clone());
            } else {
                validate_integration_checks(&request.checks)?;
            }
        }
        if let Some(runtime) = effect_runtime(&effect.operation) {
            let command = effect_command(&effect.operation).ok_or("desktop-command-schema")?;
            let fingerprint =
                sha(&serde_json::to_vec(&effect.operation).map_err(|_| "desktop-command-schema")?);
            let receipt = self.call(Operation::CommandReceipt {
                command_id: command.into(),
                fingerprint,
            })?;
            if receipt["found"] == true {
                known = Some(receipt["receipt"].clone());
            } else {
                let executable = match runtime {
                    RuntimeChoice::Codex { executable, .. } => executable,
                    RuntimeChoice::Synthetic { executable, .. } => {
                        #[cfg(not(feature = "test-fixtures"))]
                        {
                            let _ = executable;
                            return Err("synthetic-runtime-not-allowed".into());
                        }
                        #[cfg(feature = "test-fixtures")]
                        {
                            if self.fixture.is_none() {
                                return Err("native-fixture-config-required".into());
                            }
                            executable
                        }
                    }
                };
                validate_runtime_pin(executable, effect.runtime_sha256.as_deref())?;
            }
        }
        let receipt = if let Some(receipt) = known {
            receipt
        } else {
            self.call(effect.operation)?
        };
        if let Some(binding) = effect.binding {
            let binding: Binding =
                serde_json::from_value(binding).map_err(|_| "runtime-binding-schema")?;
            self.persist_binding(binding)?;
        }
        Ok(if let Some(integration_id) = integration_id {
            json!({"integrationId":integration_id,"receipt":receipt})
        } else if let Some(worker_id) = effect.worker_id {
            json!({"workerId":worker_id,"receipt":receipt})
        } else {
            receipt
        })
    }
    fn effect(
        &self,
        command: &str,
        fingerprint: &str,
        operation: Operation,
        binding: Option<Binding>,
        worker_id: Option<String>,
        runtime_connection: Option<&str>,
    ) -> Result<Value, String> {
        let runtime_sha256 = if let Some(runtime) = effect_runtime(&operation) {
            let key = runtime_connection.ok_or("runtime-connection-required")?;
            let selected = self.linked_binding(key, None)?;
            let matches = match runtime {
                RuntimeChoice::Codex {
                    executable,
                    home,
                    version,
                    ..
                } => {
                    selected.executable == *executable
                        && selected.home == *home
                        && selected.version == *version
                }
                RuntimeChoice::Synthetic {
                    executable, home, ..
                } => {
                    selected.executable == *executable
                        && selected.home == *home
                        && selected.version == "synthetic-rust-1"
                }
            };
            if !matches {
                return Err("runtime-connection-binding-mismatch".into());
            }
            validate_runtime_pin(&selected.executable, Some(&selected.executable_hash))?;
            Some(selected.executable_hash.clone())
        } else {
            None
        };
        let correction_budget = if let Operation::SubmitConversation { task, .. } = &operation {
            Some(
                self.prepared
                    .lock()
                    .map_err(|_| "preparation-lock")?
                    .values()
                    .find(|p| p.task.id == task.id)
                    .ok_or("preparation-required")?
                    .corrections,
            )
        } else {
            None
        };
        let binding = binding
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| "runtime-binding-schema")?;
        let reserved = crate::journal::reserve(
            &self.directory,
            command,
            &crate::journal::Effect {
                schema_version: 1,
                request_hash: fingerprint.into(),
                operation,
                binding,
                worker_id,
                runtime_sha256,
                correction_budget,
            },
        )?;
        self.replay(reserved)
    }
    fn save_bindings(&self, bindings: &HashMap<String, Binding>) -> Result<(), String> {
        let bytes = serde_json::to_vec(bindings).map_err(|_| "runtime-binding-schema")?;
        let temporary = self
            .directory
            .join(format!("bindings-pending-{}", id("write")));
        fs::write(&temporary, bytes).map_err(|_| "runtime-binding-save")?;
        fs::rename(temporary, self.directory.join("runtime-bindings.json"))
            .map_err(|_| "runtime-binding-save".to_owned())
    }
    fn load_bindings(&self) -> Result<HashMap<String, Binding>, String> {
        let bytes = match fs::read(self.directory.join("runtime-bindings.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(_) => return Err("runtime-binding-read".into()),
        };
        if bytes.len() > 65536 {
            return Err("runtime-binding-limit".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "runtime-binding-schema".into())
    }
    fn persist_binding(&self, binding: Binding) -> Result<(), String> {
        let _lease = orchestrix_process::lease::HostLease::acquire(
            &self.directory.join("desktop-bindings.lock"),
        )
        .map_err(|_| "runtime-binding-lock")?
        .ok_or("runtime-binding-busy-recheck-original-command")?;
        let mut latest = self.load_bindings()?;
        if let Some(old) = latest.get(&binding.id) {
            if old.revision > binding.revision {
                return Ok(());
            }
            if old.revision == binding.revision
                && serde_json::to_value(old).map_err(|_| "runtime-binding-schema")?
                    != serde_json::to_value(&binding).map_err(|_| "runtime-binding-schema")?
            {
                return Err("runtime-binding-revision-conflict".into());
            }
        }
        latest.insert(binding.id.clone(), binding);
        self.save_bindings(&latest)?;
        *self.bindings.lock().map_err(|_| "binding-lock")? = latest;
        Ok(())
    }
    fn cache_close_preference(&self, value: &Value) -> Result<(), String> {
        if let Some(behavior) = value["settings"]["closeBehavior"]
            .as_str()
            .filter(|s| ["continue", "suspend"].contains(s))
        {
            *self
                .close_behavior
                .lock()
                .map_err(|_| "window-preference-lock")? = behavior.into();
            fs::write(
                self.directory.join("window-preferences.json"),
                json!({"closeBehavior":behavior}).to_string(),
            )
            .map_err(|_| "window-preference-save")?;
        }
        Ok(())
    }
    pub fn handle(&self, request: HostRequest) -> Result<Value, String> {
        let body = serde_json::to_value(&request).map_err(|_| "desktop-request-schema")?;
        let fingerprint = sha(serde_json::to_vec(&body)
            .map_err(|_| "desktop-request-schema")?
            .as_slice());
        if let Some(command) = body["commandId"].as_str()
            && let Some(effect) = crate::journal::read(&self.directory, command, &fingerprint)?
        {
            return self.replay(effect);
        }
        self.handle_new(request, &fingerprint)
    }
    fn handle_new(&self, request: HostRequest, fingerprint: &str) -> Result<Value, String> {
        match request {
            HostRequest::Configurations { request } => {
                let changed = !matches!(
                    request.as_ref(),
                    orchestrix_core::configuration::ConfigurationRequest::Catalog {}
                        | orchestrix_core::configuration::ConfigurationRequest::Get { .. }
                        | orchestrix_core::configuration::ConfigurationRequest::Preview { .. }
                );
                let result = self.desktop(DesktopRequest::Configurations { request })?;
                if changed {
                    self.prepared
                        .lock()
                        .map_err(|_| "preparation-lock")?
                        .clear();
                }
                Ok(result)
            }
            #[cfg(feature = "test-fixtures")]
            HostRequest::TestFixture => self.test_fixture(),
            HostRequest::PreparePolicyExport => self.prepare_policy_export(),
            HostRequest::PreviewPolicyImport {
                import_ref,
                mappings,
            } => self.preview_policy_file(&import_ref, mappings),
            HostRequest::ApplyPolicyImport {
                command_id,
                expected_revision,
                import_ref,
                mappings,
                expected_preview_digest,
                confirm,
            } => self.apply_policy_file(
                command_id,
                policy_transfer::ImportExpectation {
                    revision: expected_revision,
                    digest: expected_preview_digest,
                },
                &import_ref,
                mappings,
                confirm,
                fingerprint,
            ),
            HostRequest::ReleasePolicyImport { import_ref } => {
                self.release_policy_file(&import_ref)
            }
            HostRequest::ConfigureConnectionRefresh {
                command_id,
                id,
                expected_revision,
                expected_refresh_revision,
                settings,
            } => self.effect(
                &command_id,
                fingerprint,
                Operation::Connections {
                    request: Box::new(ConnectionRequest::ConfigureRefresh {
                        command_id: command_id.clone(),
                        id,
                        expected_revision,
                        expected_refresh_revision,
                        settings,
                    }),
                },
                None,
                None,
                None,
            ),
            HostRequest::ConnectionRefreshStatus { id } => {
                self.connections(ConnectionRequest::RefreshStatus { id })
            }
            HostRequest::Bootstrap => {
                let mut snapshot = self.desktop(DesktopRequest::Bootstrap)?;
                self.cache_close_preference(&snapshot)?;
                let daemon = self.call(Operation::Snapshot)?;
                snapshot["daemon"] = daemon;
                let links = self.connections(ConnectionRequest::List)?;
                let mut normalized = Vec::new();
                for link in links["connections"].as_array().into_iter().flatten() {
                    let mut row = link.clone();
                    row["label"] = row["name"].clone();
                    row["provider"] = json!("Codex");
                    row["mode"] = json!("subscription");
                    row["status"] = if row["linked"] == false {
                        json!("unlinked")
                    } else if row["enabled"] == false {
                        json!("disabled")
                    } else {
                        row["identityStatus"].clone()
                    };
                    normalized.push(row);
                }
                for d in snapshot["connections"].as_array().into_iter().flatten() {
                    let id = d["connection"]["id"].as_str().unwrap_or_default();
                    if !normalized.iter().any(|r| r["id"] == id) {
                        normalized.push(json!({"id":id,"revision":d["connection"]["revision"],"label":"Codex (historical)","provider":"Codex","mode":"subscription","status":"relink-required","linked":false,"enabled":false,"models":[],"identityAssurance":"unknown"}));
                    }
                }
                snapshot["connections"] = json!(normalized);
                #[cfg(feature = "test-fixtures")]
                {
                    snapshot["nativeTestMode"] = json!(self.fixture.is_some());
                }
                Ok(snapshot)
            }
            HostRequest::Conversation {
                conversation_id,
                after_ordinal,
                limit,
            } => self.desktop(DesktopRequest::Conversation {
                conversation_id,
                after_ordinal,
                limit,
            }),
            HostRequest::CreateProject {
                command_id,
                expected_revision,
                id,
                name,
                directory_grant,
            } => {
                let path = self
                    .editor
                    .lock()
                    .map_err(|_| "editor-lock")?
                    .directory(&directory_grant)?;
                self.desktop(DesktopRequest::CreateProject {
                    command_id,
                    expected_revision,
                    id,
                    name,
                    directory: path.to_string_lossy().into(),
                })
            }
            HostRequest::SendMessage {
                command_id,
                expected_revision,
                conversation_id,
                project_id,
                content,
            } => self.desktop(DesktopRequest::SendMessage {
                command_id,
                expected_revision,
                conversation_id,
                project_id,
                content,
            }),
            HostRequest::SaveDraft {
                command_id,
                expected_revision,
                conversation_id,
                content,
            } => self.desktop(DesktopRequest::SaveDraft {
                command_id,
                expected_revision,
                conversation_id,
                content,
            }),
            HostRequest::AttachConversation {
                command_id,
                expected_revision,
                conversation_id,
                project_id,
            } => self.desktop(DesktopRequest::AttachConversation {
                command_id,
                expected_revision,
                conversation_id,
                project_id,
            }),
            HostRequest::SaveSettings {
                command_id,
                expected_revision,
                settings,
            } => self.desktop(DesktopRequest::SaveSettings {
                command_id,
                expected_revision,
                settings,
            }),
            HostRequest::InspectWorker { worker_id } => {
                self.desktop(DesktopRequest::InspectWorker { worker_id })
            }
            HostRequest::PrepareIntegration {
                command_id,
                expected_revision,
                worker_id,
                artifact_revision,
                target_grant,
                integration_grant,
                authorize_checks,
            } => {
                if !authorize_checks {
                    return Err(err("explicit-integration-check-authorization-required"));
                }
                let inspection = self.desktop(DesktopRequest::InspectWorker {
                    worker_id: worker_id.clone(),
                })?;
                let checks =
                    integration_source_checks(&inspection, &worker_id, &artifact_revision)?;
                let (target_directory, authorized_integration_directory) = {
                    let editor = self.editor.lock().map_err(|_| "editor-lock")?;
                    (
                        editor.directory(&target_grant)?,
                        editor.directory(&integration_grant)?,
                    )
                };
                if target_directory == authorized_integration_directory
                    || self.database.starts_with(&target_directory)
                    || self.database.starts_with(&authorized_integration_directory)
                {
                    return Err(err("integration-directory-separation"));
                }
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::PrepareIntegration {
                        command_id: command_id.clone(),
                        expected_revision,
                        request: Box::new(IntegrationRequest {
                            schema_version: 1,
                            id: stable("integration", &command_id),
                            worker_id,
                            artifact_revision,
                            authorization_ref: stable("integration-consent", &command_id),
                            authorized_integration_directory,
                            target_directory,
                            checks,
                        }),
                    },
                    None,
                    None,
                    None,
                )
            }
            HostRequest::InspectIntegration { integration_id } => {
                self.call(Operation::InspectIntegration { integration_id })
            }
            HostRequest::RevalidateIntegration {
                command_id,
                integration_id,
            } => self.effect(
                &command_id,
                fingerprint,
                Operation::RevalidateIntegration {
                    command_id: command_id.clone(),
                    integration_id,
                },
                None,
                None,
                None,
            ),
            HostRequest::ApproveIntegration {
                command_id,
                expected_revision,
                integration_id,
                candidate_revision,
                authorize,
            } => {
                if !authorize {
                    return Err(err("explicit-candidate-approval-required"));
                }
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::ApproveIntegration {
                        command_id: command_id.clone(),
                        expected_revision,
                        request: Box::new(ApprovalRequest {
                            schema_version: 1,
                            id: stable("approval", &command_id),
                            integration_id,
                            candidate_revision,
                            authorization_ref: stable("approval-consent", &command_id),
                            valid_for_ms: 900_000,
                        }),
                    },
                    None,
                    None,
                    None,
                )
            }
            HostRequest::ApplyIntegration {
                command_id,
                expected_revision,
                integration_id,
                candidate_revision,
                authorize,
            } => {
                if !authorize {
                    return Err(err("explicit-application-authorization-required"));
                }
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::ApplyIntegration {
                        command_id: command_id.clone(),
                        expected_revision,
                        request: Box::new(ApplicationRequest {
                            schema_version: 1,
                            id: stable("application", &command_id),
                            integration_id,
                            candidate_revision,
                            authorization_ref: stable("application-consent", &command_id),
                        }),
                    },
                    None,
                    None,
                    None,
                )
            }
            HostRequest::ArtifactText {
                artifact_id,
                revision,
                path,
                offset,
                limit,
            } => self.desktop(DesktopRequest::ArtifactText {
                artifact_id,
                revision,
                path,
                offset,
                limit,
            }),
            HostRequest::PrepareTask { draft } => self.prepare(*draft),
            HostRequest::DetectCodexCandidates => {
                #[cfg(feature = "test-fixtures")]
                if let Some(fixture) = &self.fixture
                    && fixture.connections_onboarding
                {
                    let candidates = self
                        .discovery
                        .lock()
                        .map_err(|_| "discovery-lock")?
                        .detect_fixture(fixture.executable.clone(), fixture.home.clone());
                    return Ok(
                        json!({"candidates":candidates,"syntheticQa":true,"authenticationRead":false,"executablesStarted":false}),
                    );
                }
                let linked = Vec::new();
                #[cfg(feature = "test-fixtures")]
                let linked = if let Some(fixture) = &self.fixture {
                    vec![(fixture.executable.clone(), fixture.home.clone())]
                } else {
                    linked
                };
                let candidates = self
                    .discovery
                    .lock()
                    .map_err(|_| "discovery-lock")?
                    .detect(&linked);
                Ok(
                    json!({"candidates":candidates,"authenticationRead":false,"executablesStarted":false}),
                )
            }
            HostRequest::InspectRuntimeVersion { executable_ref } => {
                let executable = self.connection_path(&executable_ref, "executable")?;
                #[cfg(feature = "test-fixtures")]
                if self.fixture.as_ref().is_some_and(|f| {
                    f.connections_onboarding
                        && f.executable.canonicalize().ok().as_ref() == Some(&executable)
                }) {
                    return connection_discovery::synthetic_version(
                        executable,
                        self.directory.clone(),
                    );
                }
                connection_discovery::version(executable, self.directory.clone())
            }
            request @ HostRequest::LinkConnection { .. } => {
                self.link_connection(request, fingerprint)
            }
            HostRequest::UpdateConnection {
                command_id,
                id,
                expected_revision,
                change,
                name,
                diagnostic_id,
            } => {
                let change = match change.as_str() {
                    "rename" => ConnectionChange::Rename {
                        name: name.ok_or("connection-name")?,
                    },
                    "select" => ConnectionChange::Select,
                    "disable" => ConnectionChange::Disable,
                    "enable" => ConnectionChange::Enable,
                    "unlink" => ConnectionChange::Unlink,
                    "reconfirm-identity" => ConnectionChange::Reconfirm {
                        diagnostic_id: diagnostic_id.ok_or("diagnostic-id")?,
                    },
                    _ => return Err("connection-change-not-allowed".into()),
                };
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::Connections {
                        request: Box::new(ConnectionRequest::Update {
                            command_id: command_id.clone(),
                            id,
                            expected_revision,
                            change,
                        }),
                    },
                    None,
                    None,
                    None,
                )
            }
            HostRequest::DiagnoseConnection {
                command_id,
                id,
                expected_revision,
                allow_metadata_read,
            } => self.effect(
                &command_id,
                fingerprint,
                Operation::Connections {
                    request: Box::new(ConnectionRequest::Diagnose {
                        command_id: command_id.clone(),
                        id,
                        expected_revision,
                        allow_metadata_read,
                    }),
                },
                None,
                None,
                None,
            ),
            HostRequest::CancelConnectionDiagnostic {
                command_id,
                id,
                expected_revision,
                diagnostic_id,
            } => self.effect(
                &command_id,
                fingerprint,
                Operation::Connections {
                    request: Box::new(ConnectionRequest::CancelDiagnostic {
                        command_id: command_id.clone(),
                        id,
                        expected_revision,
                        diagnostic_id,
                    }),
                },
                None,
                None,
                None,
            ),
            HostRequest::ConnectionDiagnostics { id } => {
                self.connections(ConnectionRequest::Diagnostics {
                    id,
                    diagnostic_id: None,
                })
            }
            HostRequest::SaveConnection { .. } => Err("connection-onboarding-required".into()),
            HostRequest::SubmitTask {
                command_id,
                preparation_id,
                authorize_runtime,
                authorize_checks,
            } => {
                if !authorize_runtime {
                    return Err(err("explicit-runtime-authorization-required"));
                }
                let prepared = self
                    .prepared
                    .lock()
                    .map_err(|_| "preparation-lock")?
                    .get(&preparation_id)
                    .cloned()
                    .ok_or("preparation-required")?;
                self.guard_prepared_settings(&prepared.settings_fingerprint)?;
                if !prepared.task.checks.is_empty() && !authorize_checks {
                    return Err(err("explicit-check-authorization-required"));
                }
                self.linked_binding(
                    &prepared.connection_id,
                    Some(prepared.task.connection_revision),
                )?;
                let runtime = self.binding(&prepared.connection_id, true, "implementation")?;
                let result = self.effect(
                    &command_id,
                    fingerprint,
                    Operation::SubmitConversation {
                        authorization_ref: stable("desktop-consent", &command_id),
                        conversation_id: prepared.conversation_id.clone(),
                        settings_fingerprint: prepared.settings_fingerprint.clone(),
                        command_id: command_id.clone(),
                        task: Box::new(prepared.task.clone()),
                        runtime,
                    },
                    None,
                    Some(prepared.task.id.clone()),
                    Some(&prepared.connection_id),
                )?;
                // Keep the exact original request available for inspection/correction in this
                // host. Restart does not grant a fresh execution or reconstruct missing input.
                Ok(result)
            }
            HostRequest::Control {
                action,
                command_id,
                worker_id,
            } => self.call(match action.as_str() {
                "suspend" => Operation::Suspend { command_id },
                "resume" => Operation::Resume { command_id },
                "cancel" => Operation::Cancel {
                    command_id,
                    worker_id: worker_id.ok_or("worker-id-required")?,
                },
                "recheck-recovery" => Operation::RecheckRecovery {
                    command_id,
                    worker_id: worker_id.ok_or("worker-id-required")?,
                },
                "revalidate-artifact" => Operation::RevalidateArtifact {
                    command_id,
                    worker_id: worker_id.ok_or("worker-id-required")?,
                },
                _ => return Err(err("control-not-allowed")),
            }),
            HostRequest::Recovery => self.call(Operation::Recovery),
            HostRequest::Intervene {
                command_id,
                worker_id,
                expected_revision,
                action,
                authorize,
            } => {
                if !authorize {
                    return Err("explicit-recovery-authorization-required".into());
                }
                self.call(Operation::Intervene {
                    authorization_ref: stable("recovery-consent", &command_id),
                    command_id,
                    worker_id,
                    expected_revision,
                    action,
                })
            }
            HostRequest::SetAsideReview {
                command_id,
                review_id,
                expected_revision,
                authorize,
            } => {
                if !authorize {
                    return Err("explicit-recovery-authorization-required".into());
                }
                self.call(Operation::SetAsideReview {
                    authorization_ref: stable("review-recovery-consent", &command_id),
                    command_id,
                    review_id,
                    expected_revision,
                })
            }
            HostRequest::RequestReview {
                command_id,
                worker_id,
                authorize_runtime,
            } => {
                if !authorize_runtime {
                    return Err(err("explicit-runtime-authorization-required"));
                }
                let inspection = self.desktop(DesktopRequest::InspectWorker {
                    worker_id: worker_id.clone(),
                })?;
                let revision = inspection["artifact"]["revision"]
                    .as_str()
                    .ok_or("verified-artifact-required")?
                    .into();
                let runtime = self.binding(
                    inspection["connectionId"].as_str().ok_or("connection-id")?,
                    true,
                    if inspection["worker"]["intent"]["correction"].is_object() {
                        "review-corrected"
                    } else {
                        "review"
                    },
                )?;
                let settings: Settings = serde_json::from_value(
                    self.desktop(DesktopRequest::Bootstrap)?["settings"].clone(),
                )
                .map_err(|_| "settings-schema")?;
                let budget = if let Some(first) = inspection["reviews"]
                    .as_array()
                    .and_then(|reviews| reviews.first())
                {
                    serde_json::from_value(first["budget"].clone()).map_err(|_| "review-budget")?
                } else {
                    let corrections =
                        crate::journal::correction_budget(&self.directory, &worker_id)?
                            .ok_or("recorded-correction-budget-required")?;
                    ReviewBudget {
                        max_corrections: corrections,
                        max_reviews: corrections + 1,
                        max_elapsed_ms: 900_000,
                    }
                };
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::RequestReview {
                        request: Box::new(ReviewRequest {
                            schema_version: 1,
                            id: stable("review", &command_id),
                            worker_id,
                            artifact_revision: revision,
                            authorization_ref: stable("review-consent", &command_id),
                            policy: settings.policy,
                            classification: TaskClassification::routine(
                                TaskKind::Implementation,
                                "desktop-review",
                            ),
                            budget,
                        }),
                        command_id: command_id.clone(),
                        runtime,
                    },
                    None,
                    None,
                    inspection["connectionId"].as_str(),
                )
            }
            HostRequest::AuthorizeCorrection {
                command_id,
                worker_id,
                review_id,
                authorize_runtime,
            } => {
                if !authorize_runtime {
                    return Err(err("explicit-runtime-authorization-required"));
                }
                let inspection = self.desktop(DesktopRequest::InspectWorker { worker_id })?;
                let runtime = self.binding(
                    inspection["connectionId"].as_str().ok_or("connection-id")?,
                    true,
                    "correction",
                )?;
                let settings: Settings = serde_json::from_value(
                    self.desktop(DesktopRequest::Bootstrap)?["settings"].clone(),
                )
                .map_err(|_| "settings-schema")?;
                let correction = self.desktop(DesktopRequest::PrepareCorrection {
                    review_id,
                    new_id: stable("correction", &command_id),
                    authorization_ref: stable("correction-consent", &command_id),
                    policy: Box::new(settings.policy),
                    classification: TaskClassification::routine(
                        TaskKind::Implementation,
                        "desktop-correction",
                    ),
                })?;
                let input: CorrectionRequest =
                    serde_json::from_value(correction).map_err(|_| "correction-schema")?;
                let next_id = input.id.clone();
                self.effect(
                    &command_id,
                    fingerprint,
                    Operation::Correct {
                        request: Box::new(input),
                        command_id: command_id.clone(),
                        runtime,
                    },
                    None,
                    Some(next_id),
                    inspection["connectionId"].as_str(),
                )
            }
        }
    }
    fn connections(&self, request: ConnectionRequest) -> Result<Value, String> {
        self.call(Operation::Connections {
            request: Box::new(request),
        })
    }
    fn connection_path(&self, reference: &str, kind: &str) -> Result<PathBuf, String> {
        if reference.starts_with("native-candidate-") {
            return self
                .discovery
                .lock()
                .map_err(|_| "discovery-lock")?
                .resolve(reference, kind);
        }
        let editor = self.editor.lock().map_err(|_| "editor-lock")?;
        if kind == "executable" {
            editor.file(reference)
        } else {
            editor.directory(reference)
        }
    }
    fn linked_binding(&self, id: &str, expected: Option<u64>) -> Result<Binding, String> {
        let list = self.connections(ConnectionRequest::List)?;
        let row = list["connections"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["id"] == id)
            .ok_or("connection-relink-required")?;
        let registration = row["revision"].as_u64().ok_or("connection-revision")?;
        let resolved = self.connections(ConnectionRequest::Resolve {
            id: id.into(),
            expected_revision: registration,
        })?;
        let revision = resolved["bindingRevision"]
            .as_u64()
            .ok_or("connection-binding-revision")?;
        if expected.is_some_and(|expected| expected != revision) {
            return Err("connection-binding-revision-changed".into());
        }
        let runtime: RuntimeChoice = serde_json::from_value(resolved["runtime"].clone())
            .map_err(|_| "connection-runtime-schema")?;
        let (executable, home, version) = match runtime {
            RuntimeChoice::Codex {
                executable,
                home,
                version,
                ..
            } => (executable, home, version),
            RuntimeChoice::Synthetic {
                executable, home, ..
            } => (executable, home, "synthetic-rust-1".into()),
        };
        Ok(Binding {
            id: id.into(),
            label: row["name"].as_str().unwrap_or("Codex").into(),
            revision,
            executable,
            home,
            version,
            executable_hash: resolved["executableSha256"]
                .as_str()
                .ok_or("connection-executable-pin")?
                .into(),
        })
    }
    fn link_connection(&self, request: HostRequest, fingerprint: &str) -> Result<Value, String> {
        let HostRequest::LinkConnection {
            command_id,
            id,
            expected_revision,
            name,
            version,
            executable_ref,
            home_ref,
            executable_sha256,
            allow_metadata_read,
        } = request
        else {
            return Err("connection-request".into());
        };
        let executable = self.connection_path(&executable_ref, "executable")?;
        let home = self.connection_path(&home_ref, "home")?;
        if !allow_metadata_read {
            return Err("metadata-read-not-authorized".into());
        }
        let supported = connection_discovery::SUPPORTED_VERSIONS.contains(&version.as_str());
        #[cfg(feature = "test-fixtures")]
        let supported = supported
            || self.fixture.as_ref().is_some_and(|f| {
                f.connections_onboarding
                    && version == "synthetic-rust-1"
                    && f.executable.canonicalize().ok().as_ref() == Some(&executable)
                    && f.home.canonicalize().ok().as_ref() == Some(&home)
            });
        if !supported {
            return Err("runtime-version-not-supported".into());
        }
        if connection_discovery::binary_hash(&executable)? != executable_sha256 {
            return Err("runtime-executable-changed".into());
        }
        let runtime = RuntimeChoice::Codex {
            executable,
            home,
            version,
            allow_live: false,
        };
        #[cfg(feature = "test-fixtures")]
        let runtime = if version_for_synthetic(&runtime) {
            let RuntimeChoice::Codex {
                executable, home, ..
            } = runtime
            else {
                return Err("fixture-runtime".into());
            };
            RuntimeChoice::Synthetic {
                executable,
                home,
                scenario: "connection-metadata".into(),
            }
        } else {
            runtime
        };
        self.effect(
            &command_id,
            fingerprint,
            Operation::Connections {
                request: Box::new(ConnectionRequest::Link {
                    command_id: command_id.clone(),
                    id: id.unwrap_or_else(|| stable("connection", &command_id)),
                    expected_revision,
                    name,
                    runtime,
                    metadata_directory: self.directory.clone(),
                    executable_sha256,
                    allow_metadata_read,
                }),
            },
            None,
            None,
            None,
        )
    }
    fn preparation_scope(
        &self,
        id: &str,
        binding_revision: u64,
        directory: PathBuf,
        authorized: bool,
    ) -> Result<Value, String> {
        if !authorized {
            return Err("project-metadata-read-not-authorized".into());
        }
        let list = self.connections(ConnectionRequest::List)?;
        let row = list["connections"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"] == id)
            .ok_or("connection-not-found")?;
        let registration = row["revision"].as_u64().ok_or("connection-revision")?;
        if row["bindingRevision"].as_u64() != Some(binding_revision) {
            return Err("connection-binding-revision-changed".into());
        }
        let began = std::time::Instant::now();
        let scope_request = ConnectionRequest::DiagnoseScope {
            command_id: stable(
                "prepare-metadata",
                &format!(
                    "{}-{}",
                    id,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ),
            ),
            id: id.into(),
            expected_revision: registration,
            metadata_directory: directory,
            allow_metadata_read: true,
        };
        let started = loop {
            match self.connections(scope_request.clone()) {
                Ok(value) => break value,
                Err(code)
                    if code == "metadata-foreground-wait"
                        && began.elapsed() < Duration::from_secs(20) =>
                {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(code) => return Err(code),
            }
        };
        let diagnostic = started["diagnosticId"]
            .as_str()
            .ok_or("preparation-diagnostic-required")?;
        loop {
            let report = self.connections(ConnectionRequest::Diagnostics {
                id: id.into(),
                diagnostic_id: Some(diagnostic.into()),
            })?;
            let current = &report["latest"];
            if current["id"] != diagnostic
                || current["registrationRevision"].as_u64() != Some(registration)
                || current["connectionRevision"].as_u64() != Some(binding_revision)
                || current["purpose"] != "task-preparation"
            {
                return Err("preparation-diagnostic-scope-changed".into());
            }
            match current["state"].as_str() {
                Some("complete" | "partial")
                    if current["current"] == true && current["identityChanged"] == false =>
                {
                    return Ok(
                        json!({"diagnosticId":diagnostic,"registrationRevision":registration,"connectionRevision":binding_revision,"scopeKey":current["scopeKey"],"observedAtMs":current["observedAtMs"],"validUntilMs":current["validUntilMs"],"effectiveValidUntilMs":current["effectiveValidUntilMs"],"effectiveRuntimeConfirmed":false}),
                    );
                }
                Some("running") => {}
                _ => {
                    return Err(current["errorCode"]
                        .as_str()
                        .unwrap_or("project-metadata-revalidation-required")
                        .into());
                }
            }
            if began.elapsed() >= Duration::from_secs(20) {
                return Err("project-metadata-pending-recheck".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn preparation_binding(
        &self,
        connection_id: &str,
        revision: Option<u64>,
    ) -> Result<Binding, String> {
        let revision = revision.ok_or("connection-binding-revision-required")?;
        self.linked_binding(connection_id, Some(revision))
    }
    fn prepare(&self, draft: TaskDraft) -> Result<Value, String> {
        if draft.correction_budget > 3 || draft.context_files.len() > 32 || draft.checks.len() > 16
        {
            return Err(err("task-budget"));
        }
        let binding = self.preparation_binding(&draft.connection_id, draft.connection_revision)?;
        let (snapshot, settings, settings_fingerprint) = self.preparation_settings()?;
        let (repository, worktrees, context_files, checks) = {
            let editor = self.editor.lock().map_err(|_| "editor-lock")?;
            let repository = editor.directory(&draft.repository_grant)?;
            let worktrees = editor.directory(&draft.worktree_grant)?;
            let mut context_files = Vec::new();
            for item in &draft.context_files {
                let file = editor.context_file(&item.grant, &item.relative)?;
                let child = file
                    .strip_prefix(&repository)
                    .map_err(|_| "context-must-be-inside-authorized-repository")?;
                context_files.push(ContextFileRequest {
                    path: child.to_string_lossy().replace('\\', "/"),
                    authorization_ref: id("context-consent"),
                    required: true,
                });
            }
            let mut checks = Vec::new();
            for check in &draft.checks {
                let executable = editor.file(&check.executable)?;
                if check.timeout_seconds == 0 || check.timeout_seconds > 90 {
                    return Err(err("check-timeout"));
                }
                checks.push(CheckSpec {
                    id: check.id.clone(),
                    executable: executable.clone(),
                    arguments: check.args.clone(),
                    authorization_ref: id("check-consent"),
                    authorized: true,
                    timeout_ms: check.timeout_seconds * 1000,
                    executable_sha256: Some(sha(
                        &fs::read(executable).map_err(|_| "check-executable-unavailable")?
                    )),
                });
            }
            (repository, worktrees, context_files, checks)
        };
        if repository == worktrees
            || worktrees.starts_with(&repository)
            || self.database.starts_with(&repository)
            || self.database.starts_with(&worktrees)
        {
            return Err(err("repository-worktree-database-separation"));
        }
        let base = GitExecutor::new()
            .and_then(|g| g.inspect_head(&repository))
            .map_err(|_| "git-head-inspection")?;
        let mut policy = settings.policy;
        if let Some(mode) = draft.mode {
            policy.mode = mode;
        }
        if let Some(profile) = draft.profile {
            policy.profile = profile;
        }
        policy.revision = policy
            .revision
            .checked_add(1)
            .ok_or("policy-revision-exhausted")?;
        policy.mode_requirements.subscription_only = true;
        policy.mode_requirements.max_concurrency = Some(1);
        policy.mode_requirements.require_review = true;
        let task_preferences = orchestrix_core::policy::Preferences {
            strategy: if draft.strategy.is_empty() {
                None
            } else {
                Some(serde_json::from_value(json!(draft.strategy)).map_err(|_| "strategy")?)
            },
            fallback: if draft.fallback.is_empty() {
                None
            } else {
                Some(serde_json::from_value(json!(draft.fallback)).map_err(|_| "fallback-policy")?)
            },
            connection_id: Some(draft.connection_id.clone()),
            model_id: (!draft.model.is_empty()).then_some(draft.model),
            reasoning: if draft.effort.is_empty() {
                None
            } else {
                Some(serde_json::from_value(json!(draft.effort)).map_err(|_| "reasoning")?)
            },
            preset: if draft.preset.is_empty() {
                None
            } else {
                Some(serde_json::from_value(json!(draft.preset)).map_err(|_| "preset")?)
            },
            ..Default::default()
        };
        policy
            .layers
            .retain(|layer| layer.scope != orchestrix_core::policy::Scope::Task);
        policy.layers.push(orchestrix_core::policy::PolicyLayer {
            scope: orchestrix_core::policy::Scope::Task,
            reference: "desktop-task-preferences".into(),
            revision: policy.revision,
            preferences: task_preferences,
            requirements: Default::default(),
        });
        policy.validate().map_err(|_| "policy-schema")?;
        let project_id = if let Some(project) = draft.project_id {
            project
        } else {
            return Err(err("task-requires-project-create-or-attach-conversation"));
        };
        let project = snapshot["projects"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["id"] == project_id)
            .ok_or("project-not-found")?;
        if project["directory"].as_str().map(std::path::Path::new) != Some(repository.as_path()) {
            return Err("selected-repository-does-not-match-project".into());
        }
        let repository_id = project["repositoryId"]
            .as_str()
            .ok_or("repository-id")?
            .to_owned();
        let configuration_preview = self.desktop(DesktopRequest::ResolveConfiguration {
            policy: Box::new(policy.clone()),
            project_id: Some(project_id.clone()),
        })?;
        if configuration_preview["fingerprint"].as_str() != Some(&settings_fingerprint) {
            return Err("prepared-policy-changed-prepare-again".into());
        }
        if configuration_preview["definitions"]["mode"]["executionSupport"]
            == "artifact-flow-unproven"
        {
            return Err("configuration-mode-artifact-flow-unproven".into());
        }
        if configuration_preview["definitions"]["profile"]["role"] == "review" {
            return Err("configuration-implementation-profile-required".into());
        }
        let scope_evidence = self.preparation_scope(
            &draft.connection_id,
            binding.revision,
            repository.clone(),
            draft.allow_metadata_read,
        )?;
        self.guard_prepared_settings(&settings_fingerprint)?;
        let task = ManualTaskRequest {
            schema_version: 1,
            id: id("worker"),
            project_id,
            repository_id,
            repository_directory: repository,
            authorized_worktree_directory: worktrees,
            base_commit: base,
            connection_id: draft.connection_id.clone(),
            connection_revision: binding.revision,
            objective: draft.objective,
            acceptance_criteria: draft
                .acceptance_criteria
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect(),
            policy,
            classification: TaskClassification::routine(
                TaskKind::Implementation,
                "desktop-manual-task",
            ),
            context_files,
            instructions: if settings.system_instructions.is_empty() {
                vec![]
            } else {
                vec![settings.system_instructions]
            },
            checks,
        };
        let key = id("preparation");
        let public = json!({"id":key,"task":task,"configuration":configuration_preview,"scopeEvidence":scope_evidence,"warning":"Preparation is not execution. Policy/capability/context admission is authoritatively revalidated by Core before any turn."});
        let mut prepared = self.prepared.lock().map_err(|_| "preparation-lock")?;
        if prepared.len() >= 32 {
            return Err(err("preparation-limit"));
        }
        prepared.insert(
            key,
            Prepared {
                task,
                connection_id: draft.connection_id,
                conversation_id: draft.conversation_id,
                corrections: draft.correction_budget,
                settings_fingerprint,
            },
        );
        Ok(public)
    }
    pub fn close_preference(&self) -> Result<(), String> {
        if *self
            .close_behavior
            .lock()
            .map_err(|_| "window-preference-lock")?
            == "suspend"
        {
            self.call(Operation::Suspend {
                command_id: id("window-close"),
            })
            .map_err(|_| "close-suspend-not-confirmed")?;
        }
        Ok(())
    }
    #[cfg(feature = "test-fixtures")]
    fn test_fixture(&self) -> Result<Value, String> {
        let fixture = self
            .fixture
            .as_ref()
            .ok_or("native-fixture-config-required")?;
        let executable = fixture
            .executable
            .canonicalize()
            .map_err(|_| "native-fixture-executable")?;
        let home = fixture
            .home
            .canonicalize()
            .map_err(|_| "native-fixture-home")?;
        let mut snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        if !fixture.connections_onboarding {
            let links = self.connections(ConnectionRequest::List)?;
            if !links["connections"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|r| r["id"] == "native-fixture-account")
            {
                self.connections(ConnectionRequest::Link {
                    command_id: id("fixture-link"),
                    id: "native-fixture-account".into(),
                    expected_revision: None,
                    name: "Synthetic native validation".into(),
                    runtime: RuntimeChoice::Synthetic {
                        executable: executable.clone(),
                        home: home.clone(),
                        scenario: "connection-metadata".into(),
                    },
                    metadata_directory: self.directory.clone(),
                    executable_sha256: connection_discovery::binary_hash(&executable)?,
                    allow_metadata_read: true,
                })?;
            }
            snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        }
        if !snapshot["projects"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|p| p["id"] == "native-fixture-project")
        {
            self.desktop(DesktopRequest::CreateProject {
                command_id: id("fixture-project"),
                expected_revision: snapshot["revision"].as_u64().ok_or("desktop-revision")?,
                id: "native-fixture-project".into(),
                name: "Native validation fixture".into(),
                directory: fixture
                    .repository
                    .canonicalize()
                    .map_err(|_| "native-fixture-repository")?
                    .to_string_lossy()
                    .into(),
            })?;
        }
        snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        let mut settings: Settings =
            serde_json::from_value(snapshot["settings"].clone()).map_err(|_| "settings-schema")?;
        settings.policy.mode_defaults.fallback =
            Some(orchestrix_core::policy::Fallback::Compatible);
        self.desktop(DesktopRequest::SaveSettings {
            command_id: id("fixture-settings"),
            expected_revision: snapshot["revision"].as_u64().ok_or("desktop-revision")?,
            settings: Box::new(settings),
        })?;
        let (repository, worktrees, check) = {
            let mut editor = self.editor.lock().map_err(|_| "editor-lock")?;
            (
                editor.grant_dialog_selection(fixture.repository.clone())?,
                editor.grant_dialog_selection(fixture.worktrees.clone())?,
                editor.grant_dialog_selection(executable)?,
            )
        };
        let connection_revision = if fixture.connections_onboarding {
            None
        } else {
            let links = self.connections(ConnectionRequest::List)?;
            Some(
                links["connections"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|link| link["id"] == "native-fixture-account")
                    .and_then(|link| link["bindingRevision"].as_u64())
                    .ok_or("native-fixture-binding-revision")?,
            )
        };
        let draft = json!({"conversationId":"","projectId":"native-fixture-project","repositoryGrant":repository.id,"worktreeGrant":worktrees.id,"connectionId":if fixture.connections_onboarding{""}else{"native-fixture-account"},"connectionRevision":connection_revision,"allowMetadataRead":!fixture.connections_onboarding,"objective":"fixture-edit: update fixture.txt while preserving the project","acceptanceCriteria":["fixture.txt contains after and the authorized check passes"],"contextFiles":[],"checks":[{"id":"fixture-check","executable":check.id,"args":["--check-fixture","after\n"],"timeoutSeconds":30}],"strategy":"balanced","preset":"","model":"","effort":"","correctionBudget":1,"fallback":"compatible"});
        Ok(
            json!({"nativeTestMode":true,"grants":[repository,worktrees,check],"projectId":"native-fixture-project","draft":draft,"prompt":"fixture-edit: update fixture.txt while preserving the project"}),
        )
    }
}
/// A role is an observed Core relationship, independent of native file access.
/// The caller must already possess a validated document/grant. This helper has
/// no filesystem effects and does not inspect arbitrary project metadata paths.
fn observed_file_origin(
    path: &std::path::Path,
    affected: &Value,
    projects: Option<&Value>,
) -> Value {
    let unknown = || json!({"origin":"authorized-file","originObservation":null});
    if !path.is_absolute() || affected["grantsAccess"] != false || affected["historical"] != true {
        return unknown();
    }
    let Some(workers) = affected["workerIds"].as_array() else {
        return unknown();
    };
    if workers.len() == 1 {
        let Some(worker) = workers[0]
            .as_str()
            .filter(|id| orchestrix_core::domain::LocalRef::new(*id).is_ok())
        else {
            return unknown();
        };
        return json!({"origin":"worktree-file","originObservation":{"source":"core-metadata","historical":true,"workerId":worker}});
    }
    if !workers.is_empty() {
        return unknown();
    }
    let Some(snapshot) = projects else {
        return unknown();
    };
    let Some(revision) = snapshot["revision"].as_u64() else {
        return unknown();
    };
    let Some(projects) = snapshot["projects"].as_array() else {
        return unknown();
    };
    let mut matches = projects.iter().filter(|project| {
        project["directory"].as_str().is_some_and(|directory| {
            let root = std::path::Path::new(directory);
            root.is_absolute() && path.starts_with(root)
        })
    });
    let Some(project) = matches.next() else {
        return unknown();
    };
    if matches.next().is_some() {
        return unknown();
    }
    let Some(id) = project["id"]
        .as_str()
        .filter(|id| orchestrix_core::domain::ProjectId::new(*id).is_ok())
    else {
        return unknown();
    };
    json!({"origin":"project-file","originObservation":{"source":"core-metadata","historical":true,"projectId":id,"projectName":project["name"].as_str(),"revision":revision}})
}

fn validate_integration_checks(checks: &[CheckSpec]) -> Result<(), String> {
    if checks.is_empty() || checks.len() > 16 {
        return Err(err("integration-checks-required"));
    }
    for check in checks {
        check.validate().map_err(err)?;
        let hash = check
            .executable_sha256
            .as_deref()
            .ok_or("integration-check-hash-required")?;
        validate_runtime_pin(&check.executable, Some(hash))?;
    }
    Ok(())
}
fn integration_source_checks(
    inspection: &Value,
    worker_id: &str,
    artifact_revision: &str,
) -> Result<Vec<CheckSpec>, String> {
    if inspection["summary"]["workerId"] != worker_id
        || inspection["summary"]["status"] != "verified"
        || inspection["artifact"]["revision"] != artifact_revision
        || !inspection["reviews"].as_array().is_some_and(|reviews| {
            reviews.iter().any(|review| {
                review["status"] == "accepted"
                    && review["worker_id"] == worker_id
                    && review["artifact_revision"] == artifact_revision
            })
        })
    {
        return Err(err("verified-reviewed-exact-artifact-required"));
    }
    let checks: Vec<CheckSpec> =
        serde_json::from_value(inspection["worker"]["intent"]["authorized_checks"].clone())
            .map_err(|_| "integration-source-check-schema")?;
    validate_integration_checks(&checks)?;
    Ok(checks)
}

fn effect_runtime(operation: &Operation) -> Option<&RuntimeChoice> {
    match operation {
        Operation::SubmitConversation { runtime, .. }
        | Operation::RequestReview { runtime, .. }
        | Operation::Correct { runtime, .. } => Some(runtime),
        _ => None,
    }
}
fn effect_command(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::SubmitConversation { command_id, .. }
        | Operation::RequestReview { command_id, .. }
        | Operation::Correct { command_id, .. } => Some(command_id),
        _ => None,
    }
}
fn compatible(snapshot: Value) -> Result<Value, String> {
    let features = snapshot["features"]
        .as_array()
        .ok_or("daemon-upgrade-required-close-explicitly-before-upgrade")?;
    if !features.iter().any(|f| f == "desktop-v1")
        || !features.iter().any(|f| f == "review-v1")
        || !features.iter().any(|f| f == "connections-v1")
        || !features.iter().any(|f| f == "control-settings-v1")
        || !features.iter().any(|f| f == "configuration-v1")
    {
        return Err(err("daemon-version-incompatible"));
    }
    Ok(snapshot)
}

fn validate_runtime_pin(
    executable: &std::path::Path,
    expected: Option<&str>,
) -> Result<(), String> {
    let current = connection_discovery::binary_hash(executable)?;
    if expected != Some(current.as_str()) {
        return Err("runtime-executable-changed-reconfigure".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "test-fixtures")]
    #[test]
    fn controlled_worker_scenario_is_explicit_and_scoped_to_implementation() {
        let mut fixture: FixtureConfig = serde_json::from_value(json!({
            "root":"fixture-root",
            "executable":"orchestrix-runtime-fixture.exe",
            "home":"runtime-home",
            "repository":"repository",
            "worktrees":"worktrees",
            "reviewScenario":"qa-controlled"
        }))
        .unwrap();
        assert!(fixture.validate_scenarios().is_ok());
        assert_eq!(fixture.scenario("implementation"), "review-implementation");
        assert_eq!(fixture.scenario("review"), "qa-controlled");
        fixture.worker_scenario = Some("qa-controlled".into());
        assert!(fixture.validate_scenarios().is_ok());
        assert_eq!(fixture.scenario("implementation"), "qa-controlled");
        assert_eq!(fixture.scenario("review"), "qa-controlled");
        assert_eq!(fixture.scenario("review-corrected"), "review-accept");
        assert_eq!(fixture.scenario("correction"), "review-correction");
        assert_eq!(fixture.scenario("unknown-purpose"), "review-implementation");
        fixture.worker_scenario = Some("review-implementation".into());
        assert_eq!(fixture.scenario("implementation"), "review-implementation");
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    fn fixture_worker_scenario_rejects_other_runtime_fault_modes() {
        let value = json!({
            "root":"fixture-root",
            "executable":"orchestrix-runtime-fixture.exe",
            "home":"runtime-home",
            "repository":"repository",
            "worktrees":"worktrees"
        });
        for worker in ["hang", "quota-failure", "review-accept", "", "success"] {
            let mut input = value.clone();
            input["workerScenario"] = json!(worker);
            let fixture: FixtureConfig = serde_json::from_value(input).unwrap();
            assert_eq!(
                fixture.validate_scenarios().unwrap_err(),
                "native-fixture-scenario"
            );
        }
        for worker in [json!(true), json!([]), json!({})] {
            let mut input = value.clone();
            input["workerScenario"] = worker;
            assert!(serde_json::from_value::<FixtureConfig>(input).is_err());
        }
    }

    #[test]
    fn editor_mutations_revalidate_candidates_even_on_conflict_without_losing_disk_bytes() {
        let root = directory();
        let file = root.join("candidate.txt");
        fs::write(&file, b"before").unwrap();
        let mut host = Host::new(root.clone()).unwrap();
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let calls = observed.clone();
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::Desktop { request } => match *request {
                DesktopRequest::AffectedWorkers { .. } => Ok(
                    json!({"workerIds":[],"integrationIds":["candidate-one"],"historical":true,"grantsAccess":false}),
                ),
                DesktopRequest::Bootstrap => Ok(json!({"revision":1,"projects":[]})),
                _ => Err("unexpected-test-desktop".into()),
            },
            Operation::RevalidateIntegration {
                command_id,
                integration_id,
            } => {
                assert_eq!(integration_id, "candidate-one");
                calls.lock().unwrap().push(command_id);
                Ok(json!({"response":{"state":"accepted"},"outcome":null}))
            }
            _ => Err("unexpected-test-operation".into()),
        }));
        let grant = host
            .editor
            .lock()
            .unwrap()
            .grant_dialog_selection(root.clone())
            .unwrap();
        let opened = host
            .edit(EditorRequest::Open {
                grant: grant.id.clone(),
                relative: "candidate.txt".into(),
            })
            .unwrap();
        assert!(observed.lock().unwrap().is_empty());
        let document = opened["id"].as_str().unwrap().to_owned();
        let saved = host
            .edit(EditorRequest::Save {
                document: document.clone(),
                base: opened["version"].as_str().unwrap().into(),
                text: "saved".into(),
            })
            .unwrap();
        assert_eq!(saved["affectedIntegrations"], json!(["candidate-one"]));
        assert_eq!(saved["integrationRevalidation"][0]["accepted"], true);
        assert_eq!(
            saved["integrationRevalidation"][0]["integrationId"],
            "candidate-one"
        );
        assert!(saved.get("approved").is_none());
        assert_eq!(fs::read(&file).unwrap(), b"saved");
        fs::write(&file, b"external").unwrap();
        let conflict = host
            .edit(EditorRequest::Save {
                document,
                base: saved["version"].as_str().unwrap().into(),
                text: "unsaved buffer".into(),
            })
            .unwrap();
        assert!(
            conflict["saveError"]
                .as_str()
                .unwrap()
                .contains("save-conflict")
        );
        assert!(conflict.get("version").is_none());
        assert_eq!(fs::read(&file).unwrap(), b"external");
        assert_eq!(conflict["affectedIntegrations"], json!(["candidate-one"]));
        assert_eq!(observed.lock().unwrap().len(), 2);
        let created = host
            .edit(EditorRequest::Create {
                grant: grant.id,
                name: "new-file.txt".into(),
            })
            .unwrap();
        assert_eq!(created["affectedIntegrations"], json!(["candidate-one"]));
        assert_eq!(observed.lock().unwrap().len(), 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn integration_uses_only_exact_reviewed_checks_and_requires_pinned_bytes() {
        let root = directory();
        let executable = root.join("check.exe");
        fs::write(&executable, b"authorized-check").unwrap();
        let check = CheckSpec {
            id: "check-one".into(),
            executable: executable.clone(),
            arguments: vec!["--verify".into()],
            authorization_ref: "original-check-consent".into(),
            authorized: true,
            timeout_ms: 30_000,
            executable_sha256: Some(sha(b"authorized-check")),
        };
        let mut inspection = json!({"summary":{"workerId":"worker-one","status":"verified"},"artifact":{"revision":"exact-artifact"},"worker":{"intent":{"authorized_checks":[check]}},"reviews":[{"worker_id":"worker-one","artifact_revision":"exact-artifact","status":"accepted"}]});
        let selected =
            integration_source_checks(&inspection, "worker-one", "exact-artifact").unwrap();
        assert_eq!(
            serde_json::to_value(&selected[0]).unwrap(),
            serde_json::to_value(&check).unwrap()
        );
        assert!(integration_source_checks(&inspection, "worker-one", "another-artifact").is_err());
        inspection["reviews"][0]["worker_id"] = json!("another-worker");
        assert!(integration_source_checks(&inspection, "worker-one", "exact-artifact").is_err());
        inspection["reviews"][0]["worker_id"] = json!("worker-one");
        inspection["worker"]["intent"]["authorized_checks"][0]["executable_sha256"] = Value::Null;
        assert_eq!(
            integration_source_checks(&inspection, "worker-one", "exact-artifact").unwrap_err(),
            "integration-check-hash-required"
        );
        fs::write(&executable, b"replaced-check").unwrap();
        assert!(validate_integration_checks(&[check]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn integration_grants_commands_and_replay_are_narrow_and_original() {
        let root = directory();
        let target = root.join("target-checkout");
        let integration = root.join("integration-worktrees");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&integration).unwrap();
        // Native grants store canonical absolute paths, including Windows'
        // extended-path prefix and long names when TEMP uses an 8.3 alias.
        let target = target.canonicalize().unwrap();
        let integration = integration.canonicalize().unwrap();
        let executable = root.join("check.exe");
        fs::write(&executable, b"original-check").unwrap();
        let inspection = json!({"summary":{"workerId":"worker-one","status":"verified"},"artifact":{"revision":"exact-artifact"},"worker":{"intent":{"authorized_checks":[{"id":"check-one","executable":executable,"arguments":["--verify"],"authorization_ref":"original-consent","authorized":true,"timeout_ms":30000,"executable_sha256":sha(b"original-check")}]}},"reviews":[{"worker_id":"worker-one","artifact_revision":"exact-artifact","status":"accepted"}]});
        let mut host = Host::new(root.clone()).unwrap();
        let operations = Arc::new(Mutex::new(Vec::<Value>::new()));
        let receipts = Arc::new(Mutex::new(false));
        let observed = operations.clone();
        let known = receipts.clone();
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::Desktop { request }
                if matches!(*request, DesktopRequest::InspectWorker { .. }) =>
            {
                Ok(inspection.clone())
            }
            Operation::CommandReceipt { .. } => {
                Ok(json!({"found":*known.lock().unwrap(),"receipt":{"state":"accepted"}}))
            }
            Operation::PrepareIntegration {
                request,
                expected_revision,
                ..
            } => {
                assert_eq!(expected_revision, 7);
                observed.lock().unwrap().push(json!(request));
                *known.lock().unwrap() = true;
                Ok(json!({"state":"accepted"}))
            }
            _ => Err("unexpected-test-operation".into()),
        }));
        let (target_grant, integration_grant) = {
            let mut editor = host.editor.lock().unwrap();
            (
                editor.grant_dialog_selection(target.clone()).unwrap(),
                editor.grant_dialog_selection(integration.clone()).unwrap(),
            )
        };
        let mut request = HostRequest::PrepareIntegration {
            command_id: "integration-command".into(),
            expected_revision: 7,
            worker_id: "worker-one".into(),
            artifact_revision: "exact-artifact".into(),
            target_grant: target_grant.id,
            integration_grant: integration_grant.id,
            authorize_checks: false,
        };
        assert!(host.handle(request.clone()).is_err());
        if let HostRequest::PrepareIntegration {
            authorize_checks, ..
        } = &mut request
        {
            *authorize_checks = true;
        }
        let mut changed = serde_json::to_value(&request).unwrap();
        changed["checks"] = json!([]);
        assert!(serde_json::from_value::<HostRequest>(changed).is_err());
        let result = host.handle(request.clone()).unwrap();
        assert!(
            result["integrationId"]
                .as_str()
                .unwrap()
                .starts_with("integration-")
        );
        assert_eq!(operations.lock().unwrap().len(), 1);
        assert_eq!(
            operations.lock().unwrap()[0]["target_directory"],
            json!(target)
        );
        assert_eq!(
            operations.lock().unwrap()[0]["authorized_integration_directory"],
            json!(integration)
        );
        fs::write(&executable, b"replaced-check").unwrap();
        // A known receipt can be recovered without executing changed bytes.
        host.handle(request.clone()).unwrap();
        assert_eq!(operations.lock().unwrap().len(), 1);
        // An unknown outcome cannot dispatch the original check after replacement.
        *receipts.lock().unwrap() = false;
        assert!(host.handle(request).is_err());
        assert_eq!(operations.lock().unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(id("orchestrix-native-boundary"));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn pending_runtime_pin_blocks_changed_bytes_before_dispatch() {
        let root = directory();
        let executable = root.join("owned-runtime-not-executed.exe");
        fs::write(&executable, b"authorized executable bytes").unwrap();
        let expected = sha(&fs::read(&executable).unwrap());
        assert!(validate_runtime_pin(&executable, Some(&expected)).is_ok());
        fs::write(
            &executable,
            b"changed before original command could dispatch",
        )
        .unwrap();
        assert_eq!(
            validate_runtime_pin(&executable, Some(&expected)).unwrap_err(),
            "runtime-executable-changed-reconfigure"
        );
        assert!(validate_runtime_pin(&executable, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn independent_clients_merge_native_bindings_and_restore_continue_without_daemon() {
        let root = directory();
        let first = Host::new(root.clone()).unwrap();
        let second = Host::new(root.clone()).unwrap();
        let binding = |id: &str| Binding {
            id: id.into(),
            label: id.into(),
            executable: root.join("same.exe"),
            home: root.join(id),
            executable_hash: sha(b"same"),
            version: "0.162.0-alpha.17.2".into(),
            revision: 1,
        };
        first.persist_binding(binding("account-a")).unwrap();
        second.persist_binding(binding("account-b")).unwrap();
        let restored = Host::new(root.clone()).unwrap();
        assert_eq!(restored.load_bindings().unwrap().len(), 2);
        assert!(restored.close_preference().is_ok());
        let mut changed = binding("account-a");
        changed.label = "changed at same revision".into();
        assert!(second.persist_binding(changed).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_origin_uses_unique_recorded_relationships_without_filesystem_access() {
        let root = std::env::temp_dir();
        let project = root.join("metadata-only-project");
        let path = project.join("notes.md");
        let empty = json!({"workerIds":[],"historical":true,"grantsAccess":false});
        let snapshot = json!({"revision":7,"projects":[{"id":"project-one","name":"Recorded project","directory":project}]});
        let role = observed_file_origin(&path, &empty, Some(&snapshot));
        assert_eq!(role["origin"], "project-file");
        assert_eq!(role["originObservation"]["projectId"], "project-one");
        assert_eq!(role["originObservation"]["revision"], 7);
        let worktree = json!({"workerIds":["worker-one"],"historical":true,"grantsAccess":false});
        let role = observed_file_origin(&path, &worktree, None);
        assert_eq!(role["origin"], "worktree-file");
        assert_eq!(role["originObservation"]["workerId"], "worker-one");
        assert_eq!(role["originObservation"]["historical"], true);
        let ambiguous = json!({"revision":7,"projects":[{"id":"one","directory":project},{"id":"two","directory":root}]});
        for (workers, projects) in [
            (empty.clone(), None),
            (empty.clone(), Some(ambiguous)),
            (
                json!({"workerIds":["one","two"],"historical":true,"grantsAccess":false}),
                Some(snapshot.clone()),
            ),
            (
                json!({"workerIds":["one"],"historical":true,"grantsAccess":true}),
                Some(snapshot.clone()),
            ),
            (
                json!({"workerIds":[null],"historical":true,"grantsAccess":false}),
                Some(snapshot.clone()),
            ),
        ] {
            assert_eq!(
                observed_file_origin(&path, &workers, projects.as_ref())["origin"],
                "authorized-file"
            );
        }
        let unrelated = root.join("metadata-only-project-other").join("notes.md");
        assert_eq!(
            observed_file_origin(&unrelated, &empty, Some(&snapshot))["origin"],
            "authorized-file"
        );
        // Neither a saved project directory nor a role label is a file grant.
        let mut editor = Editor::default();
        assert!(
            editor
                .execute(EditorRequest::Open {
                    grant: "project-one".into(),
                    relative: "notes.md".into()
                })
                .is_err()
        );
    }

    #[test]
    fn offline_open_preserves_the_authorized_buffer_and_unknown_role() {
        let root = directory();
        let path = root.join("notes.md");
        fs::write(&path, "authorized buffer survives offline\n").unwrap();
        let host = Host::new(root.clone()).unwrap();
        let grant = host
            .editor
            .lock()
            .unwrap()
            .grant_dialog_selection(path)
            .unwrap();
        let result = host
            .edit(EditorRequest::Open {
                grant: grant.id,
                relative: String::new(),
            })
            .unwrap();
        assert_eq!(result["text"], "authorized buffer survives offline\n");
        assert_eq!(result["origin"], "authorized-file");
        assert_eq!(result["originObservation"], Value::Null);
        assert!(result.get("saveError").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_schedules_worktree_revalidation_while_open_is_metadata_only() {
        let root = directory();
        let mut host = Host::new(root.clone()).unwrap();
        let operations = Arc::new(Mutex::new(Vec::new()));
        let recorded = operations.clone();
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::Desktop { request }
                if matches!(&*request, DesktopRequest::AffectedWorkers { .. }) =>
            {
                Ok(
                    json!({"workerIds":["previously-verified-worker"],"historical":true,"grantsAccess":false}),
                )
            }
            Operation::RevalidateArtifact {
                command_id,
                worker_id,
            } => {
                recorded
                    .lock()
                    .unwrap()
                    .push((command_id.clone(), worker_id.clone()));
                // Acceptance only schedules inspection. It is never a retained
                // verification or review approval of the changed inventory.
                Ok(json!({"accepted":true,"commandId":command_id,"workerId":worker_id}))
            }
            _ => Err("unexpected-test-operation".into()),
        }));
        let grant = host
            .editor
            .lock()
            .unwrap()
            .grant_dialog_selection(root.clone())
            .unwrap();
        let created = host
            .edit(EditorRequest::Create {
                grant: grant.id.clone(),
                name: "new-notes.md".into(),
            })
            .unwrap();
        assert_eq!(fs::read(root.join("new-notes.md")).unwrap(), b"");
        assert_eq!(
            created["affectedWorkers"],
            json!(["previously-verified-worker"])
        );
        assert_eq!(created["revalidation"][0]["accepted"], true);
        assert_eq!(
            created["revalidation"][0]["workerId"],
            "previously-verified-worker"
        );
        assert!(
            !created["revalidation"][0]["commandId"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert_eq!(operations.lock().unwrap().len(), 1);
        assert!(created.get("verified").is_none());
        assert!(created.get("approved").is_none());
        let opened = host
            .edit(EditorRequest::Open {
                grant: grant.id.clone(),
                relative: "new-notes.md".into(),
            })
            .unwrap();
        assert_eq!(opened["origin"], "worktree-file");
        assert!(opened.get("revalidation").is_none());
        assert_eq!(operations.lock().unwrap().len(), 1);
        // After target validation a failure may follow create_new, sync or
        // reopen. Existing-name failure also revalidates conservatively, without
        // claiming a new write or returning a usable document.
        let failed = host
            .edit(EditorRequest::Create {
                grant: grant.id.clone(),
                name: "new-notes.md".into(),
            })
            .unwrap();
        assert_eq!(
            failed["saveError"],
            "create-file-exists-or-permission-denied"
        );
        assert_eq!(
            failed["affectedWorkers"],
            json!(["previously-verified-worker"])
        );
        assert_eq!(failed["revalidation"][0]["accepted"], true);
        assert!(failed.get("id").is_none());
        assert!(failed.get("version").is_none());
        assert_eq!(fs::read(root.join("new-notes.md")).unwrap(), b"");
        assert_eq!(operations.lock().unwrap().len(), 2);
        // Invalid names or nonexistent grants fail before any authorized target
        // or filesystem effect exists, so no revalidation is requested.
        assert!(
            host.edit(EditorRequest::Create {
                grant: grant.id,
                name: "../escape.md".into()
            })
            .is_err()
        );
        assert!(
            host.edit(EditorRequest::Create {
                grant: "not-a-native-grant".into(),
                name: "notes.md".into()
            })
            .is_err()
        );
        assert_eq!(operations.lock().unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn renderer_connection_refs_do_not_grant_paths_and_legacy_binding_is_not_authority() {
        let root = directory();
        let exe = root.join("codex.exe");
        fs::write(&exe, b"owned fixture bytes").unwrap();
        let mut host = Host::new(root.clone()).unwrap();
        assert!(
            host.connection_path(exe.to_str().unwrap(), "executable")
                .is_err()
        );
        host.persist_binding(Binding {
            id: "legacy".into(),
            label: "Legacy".into(),
            executable: exe,
            executable_hash: sha(b"owned fixture bytes"),
            home: root.clone(),
            version: "0.162.0-alpha.17.2".into(),
            revision: 1,
        })
        .unwrap();
        host.test_transport = Some(Arc::new(|operation| match operation {
            Operation::Connections { request } if matches!(*request, ConnectionRequest::List) => {
                Ok(json!({"connections":[]}))
            }
            _ => panic!("unexpected native operation"),
        }));
        assert_eq!(
            host.linked_binding("legacy", None).unwrap_err(),
            "connection-relink-required"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn binding_revision_pin_survives_rename_and_refuses_configuration_change() {
        let root = directory();
        let exe = root.join("codex.exe");
        fs::write(&exe, b"owned fixture bytes").unwrap();
        let mut host = Host::new(root.clone()).unwrap();
        let runtime = RuntimeChoice::Codex {
            executable: exe.clone(),
            home: root.clone(),
            version: "0.162.0-alpha.17.2".into(),
            allow_live: false,
        };
        let hash = sha(b"owned fixture bytes");
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::Connections { request } => match *request {
                ConnectionRequest::List => Ok(
                    json!({"connections":[{"id":"account","revision":5,"bindingRevision":3,"name":"Renamed"}]}),
                ),
                ConnectionRequest::Resolve {
                    expected_revision, ..
                } => {
                    assert_eq!(expected_revision, 5);
                    Ok(json!({"runtime":runtime,"bindingRevision":3,"executableSha256":hash}))
                }
                _ => panic!("unexpected connection operation"),
            },
            _ => panic!("unexpected native operation"),
        }));
        assert_eq!(host.linked_binding("account", Some(3)).unwrap().revision, 3);
        assert_eq!(
            host.preparation_binding("account", Some(3))
                .unwrap()
                .revision,
            3
        );
        assert_eq!(
            host.preparation_binding("account", None).unwrap_err(),
            "connection-binding-revision-required"
        );
        assert_eq!(
            host.preparation_binding("account", Some(2)).unwrap_err(),
            "connection-binding-revision-changed"
        );
        assert_eq!(
            host.linked_binding("account", Some(2)).unwrap_err(),
            "connection-binding-revision-changed"
        );
        assert!(host.binding("account", false, "implementation").is_ok());
        fs::write(&exe, b"changed binary").unwrap();
        assert_eq!(
            host.binding("account", false, "implementation")
                .unwrap_err(),
            "runtime-executable-changed-reconfigure"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn lost_link_reply_replays_exact_original_operation_and_checks_binary_before_new_dispatch() {
        let root = directory();
        let exe = root.join("codex.exe");
        fs::write(&exe, b"owned fixture bytes").unwrap();
        let mut host = Host::new(root.clone()).unwrap();
        let executable_ref = host
            .editor
            .lock()
            .unwrap()
            .grant_dialog_selection(exe.clone())
            .unwrap()
            .id;
        let home_ref = host
            .editor
            .lock()
            .unwrap()
            .grant_dialog_selection(root.clone())
            .unwrap()
            .id;
        let request = HostRequest::LinkConnection {
            command_id: "original-link".into(),
            id: None,
            expected_revision: None,
            name: "Local label".into(),
            version: "0.162.0-alpha.17.2".into(),
            executable_ref,
            home_ref,
            executable_sha256: sha(b"owned fixture bytes"),
            allow_metadata_read: true,
        };
        let operations = Arc::new(Mutex::new(Vec::new()));
        let recorded = operations.clone();
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::CommandReceipt { .. } => Ok(json!({"found":false})),
            Operation::Connections { .. } => {
                recorded
                    .lock()
                    .unwrap()
                    .push(serde_json::to_value(operation).unwrap());
                Err("daemon-unavailable".into())
            }
            _ => panic!("unexpected operation"),
        }));
        assert_eq!(
            host.handle(request.clone()).unwrap_err(),
            "daemon-unavailable"
        );
        assert_eq!(operations.lock().unwrap().len(), 1);
        fs::write(&exe, b"changed after uncertain reply").unwrap();
        assert_eq!(
            host.handle(request.clone()).unwrap_err(),
            "runtime-executable-changed-reconfigure"
        );
        assert_eq!(
            operations.lock().unwrap().len(),
            1,
            "changed binary cannot receive another link dispatch"
        );
        fs::write(&exe, b"owned fixture bytes").unwrap();
        let original = operations.lock().unwrap()[0].clone();
        let replayed = Arc::new(Mutex::new(Vec::new()));
        let seen = replayed.clone();
        let mut reopened = Host::new(root.clone()).unwrap();
        reopened.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::CommandReceipt { .. } => Ok(json!({"found":false})),
            Operation::Connections { .. } => {
                let actual = serde_json::to_value(operation).unwrap();
                assert_eq!(actual, original);
                seen.lock().unwrap().push(actual);
                Ok(json!({"id":"original","linked":true}))
            }
            _ => panic!("unexpected operation"),
        }));
        assert_eq!(reopened.handle(request).unwrap()["linked"], true);
        assert_eq!(replayed.lock().unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "policy_transfer_tests.rs"]
mod policy_transfer_tests;
