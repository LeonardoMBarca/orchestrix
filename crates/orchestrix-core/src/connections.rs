//! Runtime-managed connection lifecycle. A link is not a login or an independent quota.
use crate::{
    daemon::RuntimeChoice,
    persistence::Store,
    runtime::{CancellationToken, CodexMetadataService, MetadataRead, RuntimeMetadata},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectionRequest {
    List,
    Link {
        command_id: String,
        id: String,
        #[serde(default)]
        expected_revision: Option<u64>,
        name: String,
        runtime: RuntimeChoice,
        metadata_directory: PathBuf,
        executable_sha256: String,
        #[serde(default)]
        allow_metadata_read: bool,
    },
    Update {
        command_id: String,
        id: String,
        expected_revision: u64,
        change: ConnectionChange,
    },
    Diagnose {
        command_id: String,
        id: String,
        expected_revision: u64,
        allow_metadata_read: bool,
    },
    DiagnoseScope {
        command_id: String,
        id: String,
        expected_revision: u64,
        metadata_directory: PathBuf,
        allow_metadata_read: bool,
    },
    CancelDiagnostic {
        command_id: String,
        id: String,
        expected_revision: u64,
        diagnostic_id: String,
    },
    Diagnostics {
        id: String,
        #[serde(default)]
        diagnostic_id: Option<String>,
    },
    /// Native-host-only response: never forward this operation to the renderer.
    Resolve {
        id: String,
        expected_revision: u64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectionChange {
    Rename {
        name: String,
    },
    Select,
    Disable,
    Enable,
    Unlink,
    /// Explicitly accept the latest changed label/scope observation; never rebind an old thread.
    Reconfirm {
        diagnostic_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionLink {
    pub schema_version: u32,
    pub id: String,
    pub revision: u64,
    pub binding_revision: u64,
    pub name: String,
    pub enabled: bool,
    pub linked: bool,
    pub runtime: RuntimeChoice,
    pub metadata_directory: PathBuf,
    pub executable_sha256: String,
    pub baseline_observation: Option<String>,
    pub identity_assurance: String,
}
impl ConnectionLink {
    pub(crate) fn validate(&self) -> Result<(), String> {
        crate::domain::ConnectionId::new(&self.id).map_err(|_| "connection-id")?;
        if self.schema_version != 1
            || self.revision == 0
            || self.binding_revision == 0
            || self.binding_revision > i64::MAX as u64
            || self.revision > i64::MAX as u64
            || !valid_name(&self.name)
            || !self.metadata_directory.is_absolute()
            || !hash(&self.executable_sha256)
            || self.baseline_observation.as_ref().is_some_and(|s| !hash(s))
            || !["unknown", "observed-label", "synthetic"]
                .contains(&self.identity_assurance.as_str())
        {
            return Err("connection-schema".into());
        }
        let (exe, home) = match &self.runtime {
            RuntimeChoice::Codex {
                executable, home, ..
            }
            | RuntimeChoice::Synthetic {
                executable, home, ..
            } => (executable, home),
        };
        if !exe.is_absolute() || !home.is_absolute() {
            return Err("connection-path".into());
        }
        Ok(())
    }
    pub(crate) fn view(&self, selected: bool, diagnostics: Value) -> Value {
        let version = match &self.runtime {
            RuntimeChoice::Codex { version, .. } => version.as_str(),
            RuntimeChoice::Synthetic { .. } => "synthetic-rust-1",
        };
        json!({"id":self.id,"revision":self.revision,"bindingRevision":self.binding_revision,"name":self.name,"enabled":self.enabled,"linked":self.linked,"selected":selected,"adapterId":if matches!(self.runtime,RuntimeChoice::Synthetic{..}){"synthetic-codex-app-server"}else{"codex-app-server"},"runtimeVersion":version,"identityAssurance":self.identity_assurance,"identityStatus":if !self.linked{"unlinked"}else if self.baseline_observation.is_none(){"unconfirmed"}else if diagnostics["identityChanged"]==true{"changed"}else{"observed"},"independentAccountConfirmed":false,"independentQuotaConfirmed":false,"authenticationOwner":"runtime","diagnostics":diagnostics})
    }
    pub(crate) fn guard(&self, revision: u64) -> Result<(), String> {
        if self.revision != revision {
            return Err("connection-revision-changed".into());
        }
        if !self.linked {
            return Err("connection-unlinked".into());
        }
        if !self.enabled {
            return Err("connection-disabled".into());
        }
        Ok(())
    }
    pub(crate) fn guard_binding(&self, revision: u64) -> Result<(), String> {
        if self.binding_revision != revision {
            return Err("connection-binding-revision-changed".into());
        }
        if !self.enabled {
            return Err("connection-disabled".into());
        }
        if !self.linked {
            return Err("connection-unlinked".into());
        }
        Ok(())
    }
    pub(crate) fn matches_runtime(&self, runtime: &RuntimeChoice) -> bool {
        // allow_live is per-inference authorization, not part of the stored link.
        match (&self.runtime, runtime) {
            (
                RuntimeChoice::Codex {
                    executable: a,
                    home: b,
                    version: c,
                    ..
                },
                RuntimeChoice::Codex {
                    executable: x,
                    home: y,
                    version: z,
                    ..
                },
            ) => a == x && b == y && c == z,
            (
                RuntimeChoice::Synthetic {
                    executable: a,
                    home: b,
                    ..
                },
                RuntimeChoice::Synthetic {
                    executable: x,
                    home: y,
                    ..
                },
            ) => a == x && b == y,
            _ => false,
        }
    }
    pub(crate) fn launch(&self) -> Result<crate::runtime::RuntimeLaunch, String> {
        self.runtime
            .clone()
            .metadata_launch_in(&self.metadata_directory)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionDiagnostic {
    pub schema_version: u32,
    pub id: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub registration_revision: u64,
    pub daemon_generation: u64,
    pub synthetic: bool,
    pub state: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub observed_at_ms: Option<u64>,
    pub valid_until_ms: Option<u64>,
    pub next_allowed_at_ms: Option<u64>,
    pub error_code: Option<String>,
    pub identity_assurance: String,
    pub observation_fingerprint: Option<String>,
    pub identity_changed: bool,
    pub descriptor: Option<Value>,
    pub catalog_labels: Vec<crate::runtime::RuntimeModelPresentation>,
    pub control_observations: Vec<String>,
    pub structured_result_support: Option<bool>,
    pub complete: bool,
    pub cached: bool,
    pub purpose: String,
    pub scope_key: Option<String>,
}
impl ConnectionDiagnostic {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.connection_revision == 0
            || self.registration_revision == 0
            || !["registration", "task-preparation"].contains(&self.purpose.as_str())
            || !["unknown", "observed-label", "synthetic"]
                .contains(&self.identity_assurance.as_str())
            || self.scope_key.as_ref().is_some_and(|s| !hash(s))
            || ![
                "running",
                "complete",
                "partial",
                "failed",
                "cancelled",
                "interrupted",
                "backoff",
                "coalesced",
            ]
            .contains(&self.state.as_str())
            || crate::domain::LocalRef::new(&self.id).is_err()
            || crate::domain::ConnectionId::new(&self.connection_id).is_err()
            || self
                .observation_fingerprint
                .as_ref()
                .is_some_and(|s| !hash(s))
            || self.error_code.as_ref().is_some_and(|s| {
                s.len() > 80 || !s.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
            })
        {
            return Err("diagnostic-schema".into());
        }
        if matches!(self.state.as_str(), "complete" | "partial")
            && ((self.state == "complete") != self.complete
                || self.descriptor.is_none()
                || self.observed_at_ms.is_none()
                || self.scope_key.is_none()
                || self.observation_fingerprint.is_none()
                || self
                    .valid_until_ms
                    .is_none_or(|v| v <= self.observed_at_ms.unwrap_or(u64::MAX)))
        {
            return Err("diagnostic-schema".into());
        }
        if self.control_observations.len() > 16
            || self.control_observations.iter().any(|s| {
                !s.starts_with("control:") || s.len() > 80 || s.chars().any(char::is_control)
            })
        {
            return Err("diagnostic-controls".into());
        }
        if self.catalog_labels.len() > 64
            || self.catalog_labels.iter().any(|r| {
                r.label.is_empty()
                    || r.label.len() > 512
                    || r.label.chars().any(char::is_control)
                    || r.native_efforts.len() > 8
                    || r.native_efforts
                        .iter()
                        .any(|e| e.len() > 40 || e.chars().any(char::is_control))
                    || self.descriptor.as_ref().is_none_or(|d| {
                        d["models"]
                            .as_array()
                            .is_none_or(|rows| !rows.iter().any(|m| m["id"] == r.id))
                    })
            })
        {
            return Err("diagnostic-labels".into());
        }
        if let Some(d) = &self.descriptor {
            if d["connection"]["id"] != self.connection_id
                || d["connection"]["revision"] != self.connection_revision
            {
                return Err("diagnostic-descriptor".into());
            }
            let _ = crate::runtime_contract::normalize_descriptor(d)
                .map_err(|_| "diagnostic-descriptor")?;
        }
        Ok(())
    }
    pub(crate) fn view(&self, now: u64, current_generation: u64) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        v.as_object_mut()
            .expect("struct")
            .remove("observationFingerprint");
        v["current"] = json!(
            matches!(self.state.as_str(), "complete" | "partial")
                && self.daemon_generation == current_generation
                && self
                    .valid_until_ms
                    .is_some_and(|t| now >= self.observed_at_ms.unwrap_or(u64::MAX) && now < t)
        );
        v["stages"] = json!([{"stage":"runtime-metadata","state":self.state}]);
        v["provenance"] = json!(if self.synthetic {
            "synthetic"
        } else {
            "official-read-only-metadata"
        });
        v["catalogEstablishesEntitlement"] = json!(false);
        v
    }
}
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}
pub(crate) fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn diagnostic_id(command: &str) -> String {
    format!(
        "diagnostic-{}",
        &crate::git_records::content_hash(command.as_bytes())[..32]
    )
}
fn valid_name(s: &str) -> bool {
    !s.trim().is_empty()
        && s.len() <= 120
        && !s.chars().any(char::is_control)
        && crate::persistence_privacy::sanitize_text(s).is_ok_and(|v| v == s)
}
struct Flight {
    directory: PathBuf,
    revision: u64,
    cancel: CancellationToken,
    thread: std::thread::JoinHandle<()>,
    result: mpsc::Receiver<Result<(MetadataRead, String), String>>,
    diagnostic: ConnectionDiagnostic,
}
/// One daemon owns this instance; every Desktop window observes the same flight/cache.
pub(crate) struct ConnectionService {
    metadata: CodexMetadataService,
    clock: Arc<dyn crate::runtime::RefreshClock>,
    flights: BTreeMap<String, Flight>,
}
impl ConnectionService {
    pub(crate) fn new() -> Self {
        Self::with_clock(Arc::new(crate::runtime::SystemRefreshClock))
    }
    pub(crate) fn with_clock(clock: Arc<dyn crate::runtime::RefreshClock>) -> Self {
        Self {
            metadata: CodexMetadataService::new(
                Arc::clone(&clock),
                crate::runtime::refresh::RefreshBounds::default(),
            )
            .expect("bounds"),
            clock,
            flights: BTreeMap::new(),
        }
    }
    pub(crate) fn cancel_all(&self) {
        for f in self.flights.values() {
            f.cancel.cancel();
        }
    }
    pub(crate) fn is_idle(&self) -> bool {
        self.flights.is_empty()
    }
    pub(crate) fn drain(&mut self, store: &mut Store) -> Result<(), String> {
        let ready = self
            .flights
            .iter()
            .filter(|(_, f)| f.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ready {
            let f = self.flights.remove(&id).expect("flight");
            let result = f
                .result
                .recv()
                .unwrap_or_else(|_| Err("metadata-thread-lost".into()));
            let _ = f.thread.join();
            let Some(link) = store.connection_link(&id).map_err(|_| "connection-store")? else {
                continue;
            };
            if link.revision != f.revision || !link.enabled || !link.linked {
                continue;
            }
            let mut d = f.diagnostic;
            d.finished_at_ms = Some(self.clock.now_ms());
            if f.cancel.is_cancelled() {
                d.state = "cancelled".into();
                d.error_code = Some("metadata-cancelled".into())
            } else {
                match result {
                    Ok((
                        MetadataRead::Ready {
                            metadata,
                            complete,
                            cached,
                        },
                        scope,
                    )) => {
                        d.scope_key = Some(scope);
                        d.state = if complete { "complete" } else { "partial" }.into();
                        d.complete = complete;
                        d.cached = cached;
                        d.observed_at_ms = Some(metadata.observed_at_ms);
                        d.valid_until_ms = Some(metadata.observed_at_ms.saturating_add(60_000));
                        d.identity_assurance = metadata.identity_assurance().into();
                        d.observation_fingerprint = Some(metadata.observation_fingerprint().into());
                        d.identity_changed = link
                            .baseline_observation
                            .as_deref()
                            .is_some_and(|old| old != metadata.observation_fingerprint());
                        d.catalog_labels = metadata.model_presentation.clone();
                        d.control_observations = metadata.controls.clone();
                        d.structured_result_support = Some(
                            metadata.synthetic
                                || metadata.descriptor.as_json()["runtimeVersion"]
                                    == "0.162.0-alpha.17.2",
                        );
                        d.descriptor = Some(metadata.descriptor.as_json().clone());
                    }
                    Ok((MetadataRead::Backoff { until_ms }, _)) => {
                        d.state = "backoff".into();
                        d.next_allowed_at_ms = Some(until_ms)
                    }
                    Ok((MetadataRead::Coalesced | MetadataRead::Busy, _)) => {
                        d.state = "coalesced".into()
                    }
                    Err(e) => {
                        d.state = "failed".into();
                        d.error_code = Some(e);
                        d.next_allowed_at_ms = Some(self.clock.now_ms().saturating_add(1000));
                    }
                }
            }
            store
                .connection_record_diagnostic(&d)
                .map_err(|_| "connection-diagnostic-store")?;
        }
        Ok(())
    }
    pub(crate) fn handle(
        &mut self,
        store: &mut Store,
        request: &ConnectionRequest,
        shutting_down: bool,
    ) -> Result<Value, String> {
        self.drain(store)?;
        match request {
            ConnectionRequest::List => {
                return store
                    .connection_list_at(self.clock.now_ms())
                    .map_err(|_| "connection-store".into());
            }
            ConnectionRequest::Diagnostics { id, diagnostic_id } => {
                return store
                    .connection_diagnostics_run_at(
                        id,
                        diagnostic_id.as_deref(),
                        self.clock.now_ms(),
                    )
                    .map_err(|_| "connection-store".into());
            }
            ConnectionRequest::Resolve {
                id,
                expected_revision,
            } => {
                let l = store
                    .connection_link(id)
                    .map_err(|_| "connection-store")?
                    .ok_or("connection-not-found")?;
                l.guard(*expected_revision)?;
                return Ok(
                    json!({"id":l.id,"revision":l.revision,"bindingRevision":l.binding_revision,"runtime":l.runtime,"metadataDirectory":l.metadata_directory,"executableSha256":l.executable_sha256}),
                );
            }
            _ => {}
        }
        if shutting_down {
            return Err("daemon-shutting-down".into());
        }
        let fingerprint = crate::git_records::content_hash(
            serde_json::to_vec(&crate::daemon::Operation::Connections {
                request: Box::new(request.clone()),
            })
            .map_err(|_| "connection-schema")?
            .as_slice(),
        );
        let command = match request {
            ConnectionRequest::Link { command_id, .. }
            | ConnectionRequest::Update { command_id, .. }
            | ConnectionRequest::Diagnose { command_id, .. }
            | ConnectionRequest::DiagnoseScope { command_id, .. }
            | ConnectionRequest::CancelDiagnostic { command_id, .. } => command_id,
            _ => unreachable!(),
        };
        if let Some(value) = store
            .connection_command(command, &fingerprint)
            .map_err(|_| "connection-command-conflict")?
        {
            return Ok(value);
        }
        match request {
            ConnectionRequest::Link {
                command_id,
                id,
                expected_revision,
                name,
                runtime,
                metadata_directory,
                executable_sha256,
                allow_metadata_read,
            } => {
                let old = store.connection_link(id).map_err(|_| "connection-store")?;
                if old.as_ref().map(|v| v.revision) != *expected_revision {
                    return Err("connection-revision-changed".into());
                }
                let mut runtime = runtime.clone();
                if let RuntimeChoice::Codex { allow_live, .. } = &mut runtime {
                    *allow_live = false;
                }
                let l = ConnectionLink {
                    schema_version: 1,
                    id: id.clone(),
                    revision: old.as_ref().map_or(1, |v| v.revision.saturating_add(1)),
                    binding_revision: old.as_ref().map_or(
                        store
                            .connection_next_binding(id)
                            .map_err(|_| "connection-store")?,
                        |v| v.binding_revision.saturating_add(1),
                    ),
                    name: name.clone(),
                    enabled: true,
                    linked: true,
                    runtime,
                    metadata_directory: metadata_directory.clone(),
                    executable_sha256: executable_sha256.clone(),
                    baseline_observation: None,
                    identity_assurance: "unknown".into(),
                };
                l.validate()?;
                if store
                    .connection_duplicate_auth_store(&l)
                    .map_err(|_| "connection-store")?
                    .is_some()
                {
                    return Err("connection-auth-store-already-linked".into());
                }
                if *allow_metadata_read && !self.flights.is_empty() {
                    return Err("metadata-service-busy".into());
                }
                let requested_diagnostic = allow_metadata_read.then(|| diagnostic_id(command_id));
                let result = store
                    .connection_register(
                        command_id,
                        &fingerprint,
                        &l,
                        *expected_revision,
                        requested_diagnostic.as_deref(),
                    )
                    .map_err(|_| "connection-registration")?;
                if *allow_metadata_read {
                    self.start(store, &l, &diagnostic_id(command_id), None, None)?;
                }
                Ok(result)
            }
            ConnectionRequest::Update {
                command_id,
                id,
                expected_revision,
                change,
            } => {
                let result = store
                    .connection_update_at(
                        command_id,
                        &fingerprint,
                        id,
                        *expected_revision,
                        change,
                        self.clock.now_ms(),
                    )
                    .map_err(|e| match e {
                        crate::persistence::StoreError::StaleRevision => {
                            "connection-revision-changed"
                        }
                        _ => "connection-update",
                    })?;
                if !matches!(change, ConnectionChange::Select) {
                    if let Some(f) = self.flights.get(id) {
                        f.cancel.cancel();
                    }
                    let updated = store
                        .connection_link(id)
                        .map_err(|_| "connection-store")?
                        .ok_or("connection-not-found")?;
                    self.metadata
                        .invalidate_connection(id, updated.binding_revision);
                    self.metadata
                        .invalidate_observations(id, updated.binding_revision);
                }
                Ok(result)
            }
            ConnectionRequest::Diagnose {
                command_id,
                id,
                expected_revision,
                allow_metadata_read,
            }
            | ConnectionRequest::DiagnoseScope {
                command_id,
                id,
                expected_revision,
                allow_metadata_read,
                ..
            } => {
                if !allow_metadata_read {
                    return Err("metadata-read-not-authorized".into());
                }
                let l = store
                    .connection_link(id)
                    .map_err(|_| "connection-store")?
                    .ok_or("connection-not-found")?;
                l.guard(*expected_revision)?;
                let directory = if let ConnectionRequest::DiagnoseScope {
                    metadata_directory, ..
                } = request
                {
                    Some(metadata_directory.clone())
                } else {
                    None
                };
                if directory.as_ref().is_some_and(|p| !p.is_absolute()) {
                    return Err("metadata-directory".into());
                }
                if directory.is_some()
                    && (l.baseline_observation.is_none()
                        || store
                            .connection_identity_changed(id, l.binding_revision)
                            .map_err(|_| "connection-store")?)
                {
                    return Err("connection-observation-unconfirmed".into());
                }
                if let Some(f) = self.flights.get(id) {
                    if f.revision != l.revision
                        || f.directory
                            != directory.as_ref().unwrap_or(&l.metadata_directory).clone()
                        || f.diagnostic.purpose
                            != if directory.is_some() {
                                "task-preparation"
                            } else {
                                "registration"
                            }
                    {
                        return Err("metadata-service-busy-different-scope".into());
                    }
                    let value = json!({"state":"coalesced","diagnosticId":f.diagnostic.id,"connectionId":id,"connectionRevision":l.binding_revision,"registrationRevision":l.revision,"sameScope":true});
                    store
                        .connection_save_command(command_id, &fingerprint, &value)
                        .map_err(|_| "connection-command")?;
                    return Ok(value);
                }
                let did = diagnostic_id(command_id);
                let result = json!({"state":"accepted","diagnosticId":did,"connectionId":id,"connectionRevision":l.binding_revision,"registrationRevision":l.revision});
                if !self.flights.is_empty() {
                    return Err("metadata-service-busy".into());
                }
                self.start(
                    store,
                    &l,
                    &did,
                    directory,
                    Some((command_id, &fingerprint, &result)),
                )?;
                Ok(result)
            }
            ConnectionRequest::CancelDiagnostic {
                command_id,
                id,
                expected_revision,
                diagnostic_id,
            } => {
                let l = store
                    .connection_link(id)
                    .map_err(|_| "connection-store")?
                    .ok_or("connection-not-found")?;
                if l.revision != *expected_revision {
                    return Err("connection-revision-changed".into());
                }
                let matches = self.flights.get(id).is_some_and(|f| {
                    &f.diagnostic.id == diagnostic_id && f.revision == *expected_revision
                });
                if matches {
                    if let Some(f) = self.flights.get(id) {
                        f.cancel.cancel();
                    }
                    self.metadata
                        .invalidate_observations(id, l.binding_revision);
                }
                let v = json!({"cancellationRequested":matches,"executionCancelled":false,"diagnosticId":diagnostic_id,"state":if matches{"cancellation-requested"}else{"not-running"}});
                store
                    .connection_save_command(command_id, &fingerprint, &v)
                    .map_err(|_| "connection-command")?;
                Ok(v)
            }
            _ => unreachable!(),
        }
    }
    fn start(
        &mut self,
        store: &mut Store,
        l: &ConnectionLink,
        id: &str,
        directory: Option<PathBuf>,
        command: Option<(&str, &str, &Value)>,
    ) -> Result<(), String> {
        if self.flights.contains_key(&l.id) {
            return Ok(());
        }
        if !self.flights.is_empty() {
            return Err("metadata-service-busy".into());
        }
        let d = ConnectionDiagnostic {
            schema_version: 1,
            id: id.into(),
            connection_id: l.id.clone(),
            connection_revision: l.binding_revision,
            registration_revision: l.revision,
            synthetic: matches!(l.runtime, RuntimeChoice::Synthetic { .. }),
            daemon_generation: store
                .daemon_state()
                .map_err(|_| "connection-store")?
                .generation,
            state: "running".into(),
            started_at_ms: self.clock.now_ms(),
            finished_at_ms: None,
            observed_at_ms: None,
            valid_until_ms: None,
            next_allowed_at_ms: None,
            error_code: None,
            identity_assurance: "unknown".into(),
            observation_fingerprint: None,
            identity_changed: false,
            descriptor: None,
            catalog_labels: Vec::new(),
            control_observations: Vec::new(),
            structured_result_support: None,
            complete: false,
            cached: false,
            purpose: if directory.is_some() {
                "task-preparation"
            } else {
                "registration"
            }
            .into(),
            scope_key: None,
        };
        d.validate()?;
        store
            .connection_begin_diagnostic(&d, command)
            .map_err(|_| "connection-diagnostic-store")?;
        let target_directory = directory.as_ref().unwrap_or(&l.metadata_directory).clone();
        let service = self.metadata.clone();
        let link = l.clone();
        let cancel = CancellationToken::new();
        let owned = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("connection-metadata".into())
            .spawn(move || {
                let result = (|| {
                    let executable = match &link.runtime {
                        RuntimeChoice::Codex { executable, .. }
                        | RuntimeChoice::Synthetic { executable, .. } => executable,
                    };
                    if runtime_executable_sha256(executable)? != link.executable_sha256 {
                        return Err("runtime-executable-changed".into());
                    }
                    let launch = link.launch()?;
                    let launch = directory.map_or(launch.clone(), |p| launch.with_cwd(p));
                    let scope = launch.metadata_scope().map_err(|e| e.code().to_string())?;
                    service
                        .read(launch, &link.id, link.binding_revision, true, &owned)
                        .map(|r| (r, scope))
                        .map_err(|e| e.code().to_string())
                })();
                let _ = tx.send(result);
            });
        let thread = match thread {
            Ok(t) => t,
            Err(_) => {
                let mut failed = d.clone();
                failed.state = "failed".into();
                failed.finished_at_ms = Some(self.clock.now_ms());
                failed.error_code = Some("metadata-thread-spawn".into());
                store
                    .connection_record_diagnostic(&failed)
                    .map_err(|_| "connection-diagnostic-store")?;
                return Err("metadata-thread-spawn".into());
            }
        };
        self.flights.insert(
            l.id.clone(),
            Flight {
                directory: target_directory,
                revision: l.revision,
                cancel,
                thread,
                result: rx,
                diagnostic: d,
            },
        );
        Ok(())
    }
}
impl Drop for ConnectionService {
    fn drop(&mut self) {
        for f in self.flights.values() {
            f.cancel.cancel();
        }
    }
}

/// Admission remains current: an immutable historical diagnostic is never authentication authority.
pub(crate) fn guard_metadata(
    store: &Store,
    id: &str,
    revision: u64,
    metadata: &RuntimeMetadata,
) -> Result<(), String> {
    if let Some(l) = store.connection_link(id).map_err(|_| "connection-store")? {
        l.guard_binding(revision)?;
        if store
            .connection_identity_changed(id, revision)
            .map_err(|_| "connection-store")?
        {
            return Err("connection-identity-reconfirmation-required".into());
        }
        let baseline = l
            .baseline_observation
            .as_deref()
            .ok_or("connection-observation-unconfirmed")?;
        if baseline != metadata.observation_fingerprint() {
            return Err("connection-identity-observation-changed".into());
        }
    } else if store
        .connection_registry_managed()
        .map_err(|_| "connection-store")?
    {
        return Err("connection-not-registered".into());
    }
    Ok(())
}

/// The approved Windows runtime is ~319 MiB; hash in 64 KiB chunks instead of allocating it.
pub const MAX_RUNTIME_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
pub fn runtime_executable_sha256(path: &std::path::Path) -> Result<String, String> {
    runtime_executable_sha256_observed(path, || {})
}
fn runtime_executable_sha256_observed(
    path: &std::path::Path,
    mut after_read: impl FnMut(),
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|_| "runtime-executable-unavailable")?;
    let before = file
        .metadata()
        .map_err(|_| "runtime-executable-unavailable")?;
    if !before.is_file() || before.len() > MAX_RUNTIME_EXECUTABLE_BYTES {
        return Err("runtime-executable-budget".into());
    }
    let modified = before
        .modified()
        .map_err(|_| "runtime-executable-unavailable")?;
    let began = std::time::Instant::now();
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if began.elapsed() > std::time::Duration::from_secs(15) {
            return Err("runtime-executable-hash-deadline".into());
        }
        let n = file
            .read(&mut chunk)
            .map_err(|_| "runtime-executable-unavailable")?;
        if n == 0 {
            break;
        }
        count = count.saturating_add(n as u64);
        if count > MAX_RUNTIME_EXECUTABLE_BYTES {
            return Err("runtime-executable-budget".into());
        }
        hash.update(&chunk[..n]);
    }
    after_read();
    let after = file
        .metadata()
        .map_err(|_| "runtime-executable-unavailable")?;
    let path_after = std::fs::metadata(path).map_err(|_| "runtime-executable-unavailable")?;
    if count != before.len()
        || after.len() != before.len()
        || path_after.len() != before.len()
        || after.modified().ok() != Some(modified)
        || path_after.modified().ok() != Some(modified)
    {
        return Err("runtime-executable-changed-during-hash".into());
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Link scope ignores the task cwd but binds executable bytes, runtime version and auth-store directory.
pub(crate) fn guard_launch(
    store: &Store,
    id: &str,
    revision: u64,
    launch: &crate::runtime::RuntimeLaunch,
) -> Result<(), String> {
    if let Some(l) = store.connection_link(id).map_err(|_| "connection-store")? {
        l.guard_binding(revision)?;
        if l.baseline_observation.is_none() {
            return Err("connection-observation-unconfirmed".into());
        }
        if store
            .connection_identity_changed(id, revision)
            .map_err(|_| "connection-store")?
        {
            return Err("connection-identity-reconfirmation-required".into());
        }
        let (exe, home, version) = match &l.runtime {
            RuntimeChoice::Codex {
                executable,
                home,
                version,
                ..
            } => (executable, home, version.as_str()),
            RuntimeChoice::Synthetic {
                executable, home, ..
            } => (executable, home, "synthetic-rust-1"),
        };
        if !launch.matches_registration(exe, home, version)
            || runtime_executable_sha256(exe)? != l.executable_sha256
        {
            return Err("connection-runtime-binding-changed".into());
        }
    } else if store
        .connection_registry_managed()
        .map_err(|_| "connection-store")?
    {
        return Err("connection-not-registered".into());
    }
    Ok(())
}

pub(crate) fn guard_choice(
    store: &Store,
    id: &str,
    revision: u64,
    runtime: &RuntimeChoice,
) -> Result<(), String> {
    if let Some(l) = store.connection_link(id).map_err(|_| "connection-store")? {
        l.guard_binding(revision)?;
        if l.baseline_observation.is_none() {
            return Err("connection-observation-unconfirmed".into());
        }
        if store
            .connection_identity_changed(id, revision)
            .map_err(|_| "connection-store")?
        {
            return Err("connection-identity-reconfirmation-required".into());
        }
        if !l.matches_runtime(runtime) {
            return Err("connection-runtime-binding-changed".into());
        }
    } else if store
        .connection_registry_managed()
        .map_err(|_| "connection-store")?
    {
        return Err("connection-not-registered".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "orchestrix-connection-hash-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root.join(name)
    }
    #[test]
    fn streaming_runtime_hash_is_exact_and_detects_changed_or_unavailable_files() {
        let p = path("runtime.bin");
        std::fs::write(&p, vec![42u8; 140_000]).unwrap();
        assert_eq!(
            runtime_executable_sha256(&p).unwrap(),
            crate::git_records::content_hash(&vec![42u8; 140_000])
        );
        assert_eq!(
            runtime_executable_sha256_observed(&p, || std::fs::write(&p, b"changed").unwrap())
                .unwrap_err(),
            "runtime-executable-changed-during-hash"
        );
        assert_eq!(
            runtime_executable_sha256(&p.with_file_name("missing")).unwrap_err(),
            "runtime-executable-unavailable"
        );
    }
    #[test]
    fn oversized_sparse_runtime_is_rejected_before_reading_or_allocating_it() {
        let p = path("oversized.bin");
        let file = std::fs::File::create(&p).unwrap();
        file.set_len(MAX_RUNTIME_EXECUTABLE_BYTES + 1).unwrap();
        drop(file);
        assert_eq!(
            runtime_executable_sha256(&p).unwrap_err(),
            "runtime-executable-budget"
        );
        std::fs::remove_file(p).unwrap();
    }
}
