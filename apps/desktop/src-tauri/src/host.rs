//! Native API client and filesystem authority. No SQLite or second worker engine.
use crate::editor::{Editor, EditorRequest};
use orchestrix_core::{
    daemon::{API_VERSION, Client, Operation, Request, RuntimeChoice},
    desktop::{DesktopRequest, Settings},
    evaluator::CheckSpec,
    git::GitExecutor,
    policy::{TaskClassification, TaskKind},
    review_records::{CorrectionRequest, ReviewBudget, ReviewRequest},
    runtime::{
        CancellationToken, CodexMetadataService, MetadataRead, RuntimeLaunch, SystemRefreshClock,
    },
    worker::{ContextFileRequest, ManualTaskRequest},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
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
}
#[cfg(test)]
type TestTransport = Arc<dyn Fn(Operation) -> Result<Value, String> + Send + Sync>;
pub struct Host {
    pub database: PathBuf,
    pub directory: PathBuf,
    pub editor: Mutex<Editor>,
    bindings: Mutex<HashMap<String, Binding>>,
    prepared: Mutex<HashMap<String, Prepared>>,
    startup: Mutex<()>,
    metadata: CodexMetadataService,
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
    if ![
        None,
        Some("review-reject"),
        Some("review-accept"),
        Some("review-invalid"),
        Some("quota-failure"),
        Some("hang"),
    ]
    .contains(&config.review_scenario.as_deref())
    {
        return Err("native-fixture-scenario".into());
    }
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
        let bindings = match fs::read(directory.join("runtime-bindings.json")) {
            Ok(bytes) => {
                if bytes.len() > 65_536 {
                    return Err(err("runtime-binding-limit"));
                }
                serde_json::from_slice(&bytes).map_err(|_| err("runtime-binding-schema"))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(_) => return Err(err("runtime-binding-read")),
        };
        Ok(Self {
            database: directory.join("orchestrix.sqlite"),
            directory: directory.clone(),
            editor: Mutex::new(Editor::default()),
            bindings: Mutex::new(bindings),
            prepared: Mutex::new(HashMap::new()),
            startup: Mutex::new(()),
            metadata: CodexMetadataService::new(Arc::new(SystemRefreshClock), Default::default())
                .ok_or("metadata-bounds")?,
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
        let _lock = self.startup.lock().map_err(|_| "startup-lock")?;
        if let Ok(snapshot) = self.call(Operation::Snapshot) {
            return compatible(snapshot);
        }
        let executable = std::env::current_exe().map_err(|_| "desktop-executable")?;
        let parent = executable.parent().ok_or("desktop-directory")?;
        let core = parent.join("orchestrix.exe");
        let keeper = parent.join("orchestrix-supervisor.exe");
        if !core.is_file() || !keeper.is_file() {
            return Err(err(
                "core-and-supervisor-executables-required-beside-desktop",
            ));
        }
        let mut command = Command::new(core);
        command
            .arg("daemon")
            .arg("--database")
            .arg(&self.database)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000 | 0x00000200);
        }
        let mut child = command.spawn().map_err(|_| err("daemon-start-failed"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if let Ok(snapshot) = self.call(Operation::Snapshot) {
                return compatible(snapshot);
            }
            if child
                .try_wait()
                .map_err(|_| "daemon-start-observation")?
                .is_some()
            {
                // Another window may own startup; it is never killed or replaced.
                if let Ok(snapshot) = self.call(Operation::Snapshot) {
                    return compatible(snapshot);
                }
                return Err(err("daemon-start-failed-or-other-owner"));
            }
            if std::time::Instant::now() >= deadline {
                return Err(err("daemon-start-not-confirmed"));
            }
            std::thread::sleep(Duration::from_millis(80));
        }
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
                }
                Err(_) => {
                    if mutating {
                        result["revalidationPending"] = json!(true);
                        result["warning"] = json!(
                            "Artifact revalidation unavailable; prior evidence is historical."
                        );
                    }
                }
            }
        }
        Ok(result)
    }
    fn binding(&self, id: &str, allow_live: bool, purpose: &str) -> Result<RuntimeChoice, String> {
        let binding = self
            .load_bindings()?
            .get(id)
            .cloned()
            .ok_or("native-runtime-binding-required")?;
        let bytes = fs::read(&binding.executable).map_err(|_| "runtime-executable-unavailable")?;
        if sha(&bytes) != binding.executable_hash {
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
                scenario: if purpose == "review" {
                    fixture
                        .review_scenario
                        .clone()
                        .unwrap_or_else(|| "review-reject".into())
                } else if purpose == "review-corrected" {
                    "review-accept".into()
                } else if purpose == "correction" {
                    "review-correction".into()
                } else {
                    "review-implementation".into()
                },
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
        Ok(if let Some(worker_id) = effect.worker_id {
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
            let bindings = self.load_bindings()?;
            let selected = bindings.get(key).ok_or("native-runtime-binding-required")?;
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
            #[cfg(feature = "test-fixtures")]
            HostRequest::TestFixture => self.test_fixture(),
            HostRequest::Bootstrap => {
                let mut snapshot = self.desktop(DesktopRequest::Bootstrap)?;
                self.cache_close_preference(&snapshot)?;
                let daemon = self.call(Operation::Snapshot)?;
                snapshot["daemon"] = daemon;
                let labels = self.load_bindings()?;
                let normalized:Vec<Value>=snapshot["connections"].as_array().into_iter().flatten().map(|d|{let id=d["connection"]["id"].as_str().unwrap_or_default();let models=d["models"].as_array().into_iter().flatten().map(|m|json!({"id":m["id"],"name":m["name"].as_str().or(m["id"].as_str()),"efforts":m["reasoningLevels"]})).collect::<Vec<_>>();json!({"id":id,"label":labels.get(id).map(|b|b.label.as_str()).unwrap_or("Codex"),"provider":"Codex","mode":"subscription","status":"historical; revalidation required","models":models,"descriptor":d})}).collect();
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
            HostRequest::SaveConnection {
                command_id,
                label,
                version,
                executable_grant,
                home_grant,
            } => self.configure(
                command_id,
                label,
                version,
                executable_grant,
                home_grant,
                fingerprint,
            ),
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
                if !prepared.task.checks.is_empty() && !authorize_checks {
                    return Err(err("explicit-check-authorization-required"));
                }
                let runtime = self.binding(&prepared.connection_id, true, "implementation")?;
                let result = self.effect(
                    &command_id,
                    fingerprint,
                    Operation::SubmitConversation {
                        authorization_ref: stable("desktop-consent", &command_id),
                        conversation_id: prepared.conversation_id.clone(),
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
    fn configure(
        &self,
        command_id: String,
        label: String,
        version: String,
        executable_grant: String,
        home_grant: String,
        fingerprint: &str,
    ) -> Result<Value, String> {
        let (executable, home) = {
            let editor = self.editor.lock().map_err(|_| "editor-lock")?;
            (
                editor.file(&executable_grant)?,
                editor.directory(&home_grant)?,
            )
        };
        if label.trim().is_empty() || label.len() > 120 {
            return Err(err("connection-label"));
        }
        let key = format!("codex-{}", &sha(home.to_string_lossy().as_bytes())[..24]);
        let snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        let revision = snapshot["connections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["connection"]["id"] == key)
            .filter_map(|d| d["connection"]["revision"].as_u64())
            .max()
            .unwrap_or(0)
            .max(self.load_bindings()?.get(&key).map_or(0, |b| b.revision))
            + 1;
        let executable_hash =
            sha(&fs::read(&executable).map_err(|_| "runtime-executable-unavailable")?);
        let launch = RuntimeLaunch::codex(
            executable.clone(),
            home.clone(),
            self.directory.clone(),
            version.clone(),
        );
        let MetadataRead::Ready {
            metadata, complete, ..
        } = self
            .metadata
            .read(launch, &key, revision, true, &CancellationToken::new())
            .map_err(|e| e.code().to_owned())?
        else {
            return Err(err("metadata-not-ready"));
        };
        let snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        validate_runtime_pin(&executable, Some(&executable_hash))?;
        let binding = Binding {
            id: key.clone(),
            label,
            executable_hash,
            executable,
            home,
            version,
            revision,
        };
        // Nonsecret paths/labels only. Authentication files remain in the provider's
        // selected home and are never copied, serialized or returned to the renderer.
        let receipt = self.effect(
            &command_id,
            fingerprint,
            Operation::Desktop {
                request: Box::new(DesktopRequest::RegisterConnection {
                    command_id: command_id.clone(),
                    expected_revision: snapshot["revision"].as_u64().ok_or("desktop-revision")?,
                    descriptor: metadata.descriptor.as_json().clone(),
                }),
            },
            Some(binding),
            None,
            None,
        )?;
        Ok(
            json!({"receipt":receipt,"complete":complete,"inferenceUsed":false,"quota":"unknown","descriptor":metadata.descriptor.as_json()}),
        )
    }
    fn prepare(&self, draft: TaskDraft) -> Result<Value, String> {
        if draft.correction_budget > 3 || draft.context_files.len() > 32 || draft.checks.len() > 16
        {
            return Err(err("task-budget"));
        }
        let snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        let settings: Settings =
            serde_json::from_value(snapshot["settings"].clone()).map_err(|_| "settings-schema")?;
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
        let binding = self
            .load_bindings()?
            .get(&draft.connection_id)
            .cloned()
            .ok_or("native-runtime-binding-required")?;
        let mut policy = settings.policy;
        policy.revision = policy
            .revision
            .checked_add(1)
            .ok_or("policy-revision-exhausted")?;
        policy.mode_requirements.subscription_only = true;
        policy.mode_requirements.max_concurrency = Some(1);
        policy.mode_requirements.require_review = true;
        policy.mode_defaults.strategy =
            Some(serde_json::from_value(json!(draft.strategy)).map_err(|_| "strategy")?);
        policy.mode_defaults.fallback =
            Some(serde_json::from_value(json!(draft.fallback)).map_err(|_| "fallback-policy")?);
        policy.mode_defaults.connection_id = Some(draft.connection_id.clone());
        policy.mode_defaults.model_id = (!draft.model.is_empty()).then_some(draft.model);
        policy.mode_defaults.reasoning = if draft.effort.is_empty() {
            None
        } else {
            Some(serde_json::from_value(json!(draft.effort)).map_err(|_| "reasoning")?)
        };
        policy.mode_defaults.preset = if draft.preset.is_empty() {
            None
        } else {
            Some(serde_json::from_value(json!(draft.preset)).map_err(|_| "preset")?)
        };
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
        let public = json!({"id":key,"task":task,"warning":"Preparation is not execution. Policy/capability/context admission is authoritatively revalidated by Core before any turn."});
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
        let exists = snapshot["connections"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|d| d["connection"]["id"] == "native-fixture-account");
        if !exists {
            let launch = RuntimeLaunch::fixture(
                executable.clone(),
                vec!["--scenario".into(), "review-implementation".into()],
                home.clone(),
                fixture.repository.clone(),
            );
            let MetadataRead::Ready { metadata, .. } = self
                .metadata
                .read(
                    launch,
                    "native-fixture-account",
                    1,
                    true,
                    &CancellationToken::new(),
                )
                .map_err(|e| e.code().to_owned())?
            else {
                return Err("fixture-metadata-unavailable".into());
            };
            self.desktop(DesktopRequest::RegisterConnection {
                command_id: id("fixture-register"),
                expected_revision: snapshot["revision"].as_u64().ok_or("desktop-revision")?,
                descriptor: metadata.descriptor.as_json().clone(),
            })?;
            snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        }
        let binding = Binding {
            id: "native-fixture-account".into(),
            label: "Synthetic native validation".into(),
            executable_hash: sha(&fs::read(&executable).map_err(|_| "native-fixture-executable")?),
            executable: executable.clone(),
            home,
            version: "synthetic-rust-1".into(),
            revision: 1,
        };
        self.persist_binding(binding)?;
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
        let draft = json!({"conversationId":"","projectId":"native-fixture-project","repositoryGrant":repository.id,"worktreeGrant":worktrees.id,"connectionId":"native-fixture-account","objective":"fixture-edit: update fixture.txt while preserving the project","acceptanceCriteria":["fixture.txt contains after and the authorized check passes"],"contextFiles":[],"checks":[{"id":"fixture-check","executable":check.id,"args":["--check-fixture","after\n"],"timeoutSeconds":30}],"strategy":"balanced","preset":"","model":"","effort":"","correctionBudget":1,"fallback":"compatible"});
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
    if !features.iter().any(|f| f == "desktop-v1") || !features.iter().any(|f| f == "review-v1") {
        return Err(err("daemon-version-incompatible"));
    }
    Ok(snapshot)
}

fn validate_runtime_pin(
    executable: &std::path::Path,
    expected: Option<&str>,
) -> Result<(), String> {
    let current = sha(&fs::read(executable).map_err(|_| "runtime-executable-unavailable")?);
    if expected != Some(current.as_str()) {
        return Err("runtime-executable-changed-reconfigure".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
