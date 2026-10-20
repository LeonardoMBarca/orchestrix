//! Runtime-managed connection lifecycle. A link is not a login or an independent quota.
#[cfg(test)]
#[path = "diagnostic_retention_tests.rs"]
mod diagnostic_retention_tests;
use crate::{
    connection_refresh::{RefreshMode, RefreshSettings},
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

pub(crate) const MAX_DIAGNOSTIC_BODY_BYTES: usize = 262_144;

/// A bounded read can still contain a catalog larger than one retained record.
/// Persist an explicit failed terminal using its reservation, never lose the
/// admitted round or silently treat a truncated catalog as successful evidence.
fn bound_terminal_evidence(d: &mut ConnectionDiagnostic) {
    if serde_json::to_vec(d).is_ok_and(|bytes| bytes.len() <= MAX_DIAGNOSTIC_BODY_BYTES) {
        return;
    }
    d.state = "failed".into();
    d.error_code = Some("metadata-evidence-size-limit".into());
    d.descriptor = None;
    d.catalog_labels.clear();
    d.control_observations.clear();
    d.structured_result_support = None;
    d.complete = false;
    d.cached = false;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConnectionRequest {
    List,
    RefreshStatus {
        #[serde(default)]
        id: Option<String>,
    },
    ConfigureRefresh {
        command_id: String,
        id: String,
        expected_revision: u64,
        expected_refresh_revision: u64,
        settings: RefreshSettings,
    },
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
    /// Local evidence only; never schedules a runtime read.
    DiagnosticHistory {
        id: String,
        #[serde(default)]
        cursor: Option<String>,
        #[serde(default = "history_page_size")]
        limit: u32,
    },
    DiagnosticObservation {
        id: String,
        diagnostic_id: String,
        #[serde(default)]
        ordinal: Option<u32>,
    },
    /// Native-host-only response: never forward this operation to the renderer.
    Resolve {
        id: String,
        expected_revision: u64,
    },
}
fn history_page_size() -> u32 {
    20
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
    periodic: Option<crate::runtime::MetadataClaim>,
    refresh_revision: Option<u64>,
}
/// One daemon owns this instance; every Desktop window observes the same flight/cache.
pub(crate) struct ConnectionService {
    metadata: CodexMetadataService,
    clock: Arc<dyn crate::runtime::RefreshClock>,
    flights: BTreeMap<String, Flight>,
    coordinator: crate::runtime::MetadataRefreshCoordinator,
    refresh_revisions: BTreeMap<String, u64>,
    periodic_ordinal: u64,
    next_reconcile_at_ms: u64,
    refresh_blocks: BTreeMap<String, String>,
    #[cfg(test)]
    fail_next_metadata_spawn: bool,
}
impl ConnectionService {
    pub(crate) fn new() -> Self {
        Self::with_metadata(
            Arc::new(crate::runtime::SystemRefreshClock),
            CodexMetadataService::shared().clone(),
        )
    }
    #[cfg(test)]
    pub(crate) fn with_clock(clock: Arc<dyn crate::runtime::RefreshClock>) -> Self {
        let metadata = CodexMetadataService::new(
            Arc::clone(&clock),
            crate::runtime::refresh::RefreshBounds {
                max_in_flight: 1,
                ..crate::runtime::refresh::RefreshBounds::default()
            },
        )
        .expect("bounds");
        Self::with_metadata(clock, metadata)
    }
    fn with_metadata(
        clock: Arc<dyn crate::runtime::RefreshClock>,
        metadata: CodexMetadataService,
    ) -> Self {
        let coordinator = crate::runtime::MetadataRefreshCoordinator::new(
            metadata.clone(),
            Arc::clone(&clock),
            crate::runtime::MetadataScheduleBounds {
                max_connections: 64,
                max_reads_per_tick: 1,
                max_in_flight: 1,
                minimum_interval_ms: 60_000,
            },
        )
        .expect("bounds");
        Self {
            metadata,
            clock,
            flights: BTreeMap::new(),
            coordinator,
            refresh_revisions: BTreeMap::new(),
            periodic_ordinal: 0,
            next_reconcile_at_ms: 0,
            refresh_blocks: BTreeMap::new(),
            #[cfg(test)]
            fail_next_metadata_spawn: false,
        }
    }
    /// Settings policy TTL mutations affect future reads/admissions, not active
    /// turns. Called only after the corresponding Settings transaction commits.
    pub(crate) fn invalidate_policy_observations(&self, store: &Store) -> Result<(), String> {
        for link in store
            .connection_links_current()
            .map_err(|_| "connection-store")?
        {
            self.metadata
                .invalidate_observations(&link.id, link.binding_revision);
            if let Some(f) = self.flights.get(&link.id) {
                f.cancel.cancel();
            }
            self.coordinator.defer(&link.id);
        }
        Ok(())
    }
    fn sync_refresh(&mut self, store: &Store) -> Result<(), String> {
        for link in store
            .connection_links_current()
            .map_err(|_| "connection-store")?
        {
            let config = store
                .connection_refresh_configuration(&link.id)
                .map_err(|_| "connection-store")?;
            let authorized = config.as_ref().is_some_and(|c| c.authorized_for(&link))
                && link.baseline_observation.is_some()
                && (store
                    .connection_diagnostic_remaining(&link.id)
                    .map_err(|_| "connection-store")?
                    >= 2
                    || self.flights.contains_key(&link.id))
                && !store
                    .connection_identity_changed(&link.id, link.binding_revision)
                    .map_err(|_| "connection-store")?;
            if !authorized {
                self.refresh_blocks.remove(&link.id);
                self.coordinator.unregister(&link.id);
                self.refresh_revisions.remove(&link.id);
                if let Some(f) = self.flights.get(&link.id).filter(|f| f.periodic.is_some()) {
                    f.cancel.cancel();
                }
                continue;
            }
            let config = config.expect("authorized configuration");
            if self.refresh_revisions.get(&link.id) != Some(&config.revision) {
                if let Some(f) = self.flights.get(&link.id).filter(|f| f.periodic.is_some()) {
                    f.cancel.cancel();
                }
                let registered =
                    self.coordinator
                        .register_deferred(crate::runtime::MetadataRegistration {
                            connection_id: link.id.clone(),
                            revision: link.binding_revision,
                            launch: link.launch()?,
                            authorized: true,
                            enabled: true,
                            interval_ms: config.settings.effective_interval_ms(),
                        });
                if let Err(error) = registered {
                    self.refresh_blocks
                        .insert(link.id.clone(), error.code().into());
                    self.coordinator.unregister(&link.id);
                    continue;
                }
                self.refresh_blocks.remove(&link.id);
                self.coordinator.defer(&link.id);
                self.refresh_revisions.insert(link.id, config.revision);
            }
        }
        Ok(())
    }
    /// Called only by the daemon main loop, outside SQLite transactions. At most
    /// one metadata flight is started; worker concurrency/admissions are unchanged.
    pub(crate) fn tick(&mut self, store: &mut Store) -> Result<(), String> {
        self.drain(store)?;
        let now = self.clock.now_ms();
        if now >= self.next_reconcile_at_ms || self.next_reconcile_at_ms.saturating_sub(now) > 1_000
        {
            self.sync_refresh(store)?;
            self.next_reconcile_at_ms = now.saturating_add(1_000);
        }
        if !self.flights.is_empty() {
            return Ok(());
        }
        let Some(claim) = self
            .coordinator
            .claim_due()
            .map_err(|e| e.code().to_string())?
        else {
            return Ok(());
        };
        let Some(link) = store
            .connection_link(&claim.connection_id)
            .map_err(|_| "connection-store")?
        else {
            self.coordinator.complete_claim(&claim, false, None, true);
            return Ok(());
        };
        self.periodic_ordinal = self.periodic_ordinal.saturating_add(1);
        let generation = store
            .daemon_state()
            .map_err(|_| "connection-store")?
            .generation;
        let id = format!(
            "periodic-{generation}-{}-{}",
            self.clock.now_ms(),
            self.periodic_ordinal
        );
        if let Err(error) = self.start(store, &link, &id, None, None) {
            self.coordinator.complete_claim(&claim, false, None, false);
            // A thread allocation failure is confirmed before any provider
            // process exists and its failed diagnostic is already durable.
            // Keep controls/other work alive; the coordinator's bounded retry
            // opportunity applies. Storage/integrity errors remain fatal.
            if error == "metadata-thread-spawn" {
                return Ok(());
            }
            return Err(error);
        }
        if let Some(f) = self.flights.get_mut(&link.id) {
            f.periodic = Some(claim);
            f.refresh_revision = self.refresh_revisions.get(&link.id).copied();
        }
        Ok(())
    }
    fn refresh_status(&mut self, store: &Store, id: Option<&str>) -> Result<Value, String> {
        self.sync_refresh(store)?;
        let now = self.clock.now_ms();
        let policy = store
            .connection_policy_validity()
            .map_err(|_| "connection-store")?;
        let mut values = Vec::new();
        for link in store
            .connection_links_current()
            .map_err(|_| "connection-store")?
            .into_iter()
            .filter(|l| id.is_none_or(|id| id == l.id))
        {
            let config = store
                .connection_refresh_configuration(&link.id)
                .map_err(|_| "connection-store")?;
            let requested = config
                .as_ref()
                .map_or_else(RefreshSettings::default, |c| c.settings.clone());
            let remaining = store
                .connection_diagnostic_remaining(&link.id)
                .map_err(|_| "connection-store")?;
            let storage = store
                .connection_diagnostic_storage(&link.id)
                .map_err(|_| "connection-store")?;
            let eligible = config.as_ref().is_some_and(|c| c.authorized_for(&link))
                && self.refresh_revisions.contains_key(&link.id);
            let schedule = self.coordinator.registration_status(&link.id);
            let active = self.flights.get(&link.id);
            let reason = if requested.mode == RefreshMode::Manual {
                "manual"
            } else if let Some(block) = self.refresh_blocks.get(&link.id) {
                block.as_str()
            } else if !link.enabled || !link.linked {
                "connection-disabled"
            } else if remaining < 2 {
                "diagnostic-history-storage-budget-exhausted"
            } else if !eligible {
                "reconfirmation-required"
            } else if schedule.is_some_and(|s| s.1) {
                "authorization-required"
            } else if active.is_some_and(|f| f.periodic.is_some()) {
                "running"
            } else if active.is_some() {
                "foreground-read"
            } else if schedule.is_some_and(|s| s.3 > 0) {
                "backoff"
            } else {
                "interval"
            };
            let mut effective_settings = requested.clone();
            effective_settings.interval_ms = requested.effective_interval_ms();
            effective_settings.validity_ms = requested.validity_ms.effective(&policy);
            if !eligible {
                effective_settings.mode = RefreshMode::Manual;
                effective_settings.recurring_consent = false;
            }
            values.push(json!({"connectionId":link.id,"connectionRevision":link.revision,"bindingRevision":link.binding_revision,
                "refreshRevision":config.as_ref().map_or(0,|c|c.revision),"schemaVersion":1,
                "requested":requested,"effective":effective_settings,"recurringAuthorized":eligible,
                "reason":reason,"nextOpportunityAtMs":schedule.map(|s|s.0),"observedAtMs":now,
                "sourceTtlMs":self.metadata.source_ttl_ms(),"policyValidityMs":policy,
                "activeDiagnosticId":active.map(|f|&f.diagnostic.id),"activeSource":active.map(|f|if f.periodic.is_some(){"periodic"}else{"foreground"}),
                "limits":{"globalConcurrentReads":1,"perConnectionConcurrentReads":1,"minimumIntervalMs":60_000,"maximumIntervalMs":86_400_000,"metadataDeadlineMs":15_000,"maximumBackoffMs":300_000,"retainedDiagnosticRows":512,"remainingDiagnosticRows":remaining},
                "storage":storage,
                "workerAdmissionsSuspendedByRefresh":false,"quotaPredictionAvailable":false,"consentScope":"registration-directory-binding-runtime","consentPortable":false}));
        }
        if id.is_some() && values.is_empty() {
            return Err("connection-not-found".into());
        }
        Ok(
            json!({"refresh":values,"daemonGeneration":store.daemon_state().map_err(|_|"connection-store")?.generation}),
        )
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
                if let Some(claim) = &f.periodic {
                    self.coordinator.complete_claim(claim, false, None, true);
                }
                continue;
            };
            if link.revision != f.revision || !link.enabled || !link.linked {
                if let Some(claim) = &f.periodic {
                    self.coordinator.complete_claim(
                        claim,
                        false,
                        None,
                        link.binding_revision != f.diagnostic.connection_revision
                            || !link.enabled
                            || !link.linked,
                    );
                }
                continue;
            }
            if f.refresh_revision.is_some_and(|r| {
                store
                    .connection_refresh_configuration(&id)
                    .ok()
                    .flatten()
                    .is_none_or(|c| c.revision != r || !c.authorized_for(&link))
            }) {
                f.cancel.cancel();
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
                        d.valid_until_ms = Some(
                            metadata
                                .observed_at_ms
                                .saturating_add(self.metadata.source_ttl_ms()),
                        );
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
            bound_terminal_evidence(&mut d);
            store
                .connection_record_diagnostic(&d)
                .map_err(|_| "connection-diagnostic-store")?;
            if let Some(claim) = &f.periodic {
                let pause = d.identity_changed
                    || matches!(
                        d.error_code.as_deref(),
                        Some(
                            "authentication"
                                | "policy"
                                | "version"
                                | "unsupported"
                                | "ownership"
                                | "runtime-executable-changed"
                        )
                    );
                self.coordinator.complete_claim(
                    claim,
                    matches!(d.state.as_str(), "complete" | "partial"),
                    d.next_allowed_at_ms,
                    pause,
                );
            } else if d.purpose == "registration"
                && matches!(d.state.as_str(), "complete" | "partial")
                && !d.identity_changed
            {
                self.coordinator.revalidated(&id);
            }
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
            ConnectionRequest::RefreshStatus { id } => {
                return self.refresh_status(store, id.as_deref());
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
            ConnectionRequest::DiagnosticHistory { id, cursor, limit } => {
                return store
                    .connection_diagnostic_history(id, cursor.as_deref(), *limit)
                    .map_err(|e| format!("diagnostic-history-{e}"));
            }
            ConnectionRequest::DiagnosticObservation {
                id,
                diagnostic_id,
                ordinal,
            } => {
                return store
                    .connection_diagnostic_observation(id, diagnostic_id, *ordinal)
                    .map_err(|e| format!("diagnostic-history-{e}"));
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
            ConnectionRequest::ConfigureRefresh { command_id, .. } => command_id,
            _ => unreachable!(),
        };
        if let Some(value) = store
            .connection_command(command, &fingerprint)
            .map_err(|_| "connection-command-conflict")?
        {
            return Ok(value);
        }
        match request {
            ConnectionRequest::ConfigureRefresh {
                command_id,
                id,
                expected_revision,
                expected_refresh_revision,
                settings,
            } => {
                settings.validate()?;
                let previous_validity = store
                    .connection_refresh_configuration(id)
                    .map_err(|_| "connection-store")?
                    .map_or_else(RefreshSettings::default, |c| c.settings)
                    .validity_ms;
                let value = store
                    .connection_configure_refresh(
                        command_id,
                        &fingerprint,
                        id,
                        *expected_revision,
                        *expected_refresh_revision,
                        settings,
                        self.clock.now_ms(),
                    )
                    .map_err(|e| {
                        if e == crate::persistence::StoreError::StaleRevision {
                            "refresh-revision-changed"
                        } else {
                            "refresh-configuration"
                        }
                    })?;
                if previous_validity != settings.validity_ms {
                    let binding = value["bindingRevision"]
                        .as_u64()
                        .ok_or("connection-binding-revision")?;
                    // The same pool serves future worker discovery. Detach old
                    // tickets as well as cached values before acknowledging the
                    // new validity; pending old observations cannot publish.
                    self.metadata.invalidate_observations(id, binding);
                }
                self.sync_refresh(store)?;
                Ok(value)
            }
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
                self.sync_refresh(store)?;
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
                if matches!(request, ConnectionRequest::DiagnoseScope { .. }) {
                    // A required foreground scope gets priority without racing
                    // a still-closing owned periodic process. Original command
                    // remains unrecorded and may be retried with its same ID.
                    for (connection, f) in &self.flights {
                        if f.periodic.is_some() {
                            f.cancel.cancel();
                            self.coordinator.defer(connection);
                        }
                    }
                    if self.flights.values().any(|f| f.periodic.is_some()) {
                        return Err("metadata-foreground-wait".into());
                    }
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
        if store
            .connection_diagnostic_storage(&l.id)
            .map_err(|_| "connection-store")?["storageBlocked"]
            == true
        {
            return Err("diagnostic-history-storage-budget-exhausted".into());
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
        let maximum_age = store
            .connection_read_maximum_age(&l.id)
            .map_err(|_| "connection-store")?;
        let link = l.clone();
        let cancel = CancellationToken::new();
        let owned = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let spawn = move || {
            std::thread::Builder::new()
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
                            .read_with_maximum_age(
                                launch,
                                &link.id,
                                link.binding_revision,
                                true,
                                &owned,
                                maximum_age,
                            )
                            .map(|r| (r, scope))
                            .map_err(|e| e.code().to_string())
                    })();
                    let _ = tx.send(result);
                })
        };
        #[cfg(test)]
        let thread = if std::mem::take(&mut self.fail_next_metadata_spawn) {
            Err(std::io::Error::other(
                "owned synthetic metadata spawn failure",
            ))
        } else {
            spawn()
        };
        #[cfg(not(test))]
        let thread = spawn();
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
                periodic: None,
                refresh_revision: None,
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
        let now = now_ms();
        let maximum_age = store
            .connection_effective_validity(id)
            .map_err(|_| "connection-store")?
            .minimum();
        if now < metadata.observed_at_ms
            || now >= metadata.observed_at_ms.saturating_add(maximum_age)
        {
            return Err("connection-observation-expired".into());
        }
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
    #[cfg(windows)]
    #[test]
    fn periodic_thread_spawn_failure_is_durable_bounded_and_keeps_controls_available() {
        use crate::runtime::ManualRefreshClock;
        let database = path("periodic-spawn.sqlite");
        let directory = database.parent().unwrap().canonicalize().unwrap();
        let mut store = Store::open(database).unwrap();
        // Declared synthetic unit setup, not an authentication or provider proof.
        let link = ConnectionLink {
            schema_version: 1,
            id: "spawn-fixture".into(),
            revision: 1,
            binding_revision: 1,
            name: "Synthetic spawn fixture".into(),
            enabled: true,
            linked: true,
            runtime: RuntimeChoice::Synthetic {
                executable: std::env::current_exe().unwrap(),
                home: directory.clone(),
                scenario: "connection-metadata".into(),
            },
            metadata_directory: directory,
            executable_sha256: "a".repeat(64),
            baseline_observation: Some("b".repeat(64)),
            identity_assurance: "synthetic".into(),
        };
        store
            .connection_register("spawn-link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let clock = Arc::new(ManualRefreshClock::new(100));
        let mut service = ConnectionService::with_clock(clock.clone());
        service
            .handle(
                &mut store,
                &ConnectionRequest::ConfigureRefresh {
                    command_id: "spawn-consent".into(),
                    id: link.id.clone(),
                    expected_revision: 1,
                    expected_refresh_revision: 0,
                    settings: RefreshSettings {
                        mode: RefreshMode::Automatic,
                        recurring_consent: true,
                        interval_ms: 60_000,
                        ..RefreshSettings::default()
                    },
                },
                false,
            )
            .unwrap();
        service.tick(&mut store).unwrap();
        assert!(service.is_idle());
        service.fail_next_metadata_spawn = true;
        clock.set(60_100);
        service.tick(&mut store).unwrap();
        assert!(service.is_idle());
        let report = store
            .connection_diagnostics_run_at(&link.id, None, 60_100)
            .unwrap();
        assert_eq!(report["latest"]["state"], "failed");
        assert_eq!(report["latest"]["errorCode"], "metadata-thread-spawn");
        assert_eq!(report["latest"]["finishedAtMs"], 60_100);
        let diagnostic_id = report["latest"]["id"].as_str().unwrap().to_owned();
        let status = service
            .handle(
                &mut store,
                &ConnectionRequest::RefreshStatus {
                    id: Some(link.id.clone()),
                },
                false,
            )
            .unwrap();
        assert_eq!(status["refresh"][0]["reason"], "backoff");
        assert_eq!(status["refresh"][0]["nextOpportunityAtMs"], 120_100);
        assert_eq!(status["refresh"][0]["activeDiagnosticId"], Value::Null);
        let cancelled = service
            .handle(
                &mut store,
                &ConnectionRequest::CancelDiagnostic {
                    command_id: "spawn-cancel".into(),
                    id: link.id.clone(),
                    expected_revision: 1,
                    diagnostic_id: diagnostic_id.clone(),
                },
                false,
            )
            .unwrap();
        assert_eq!(cancelled["state"], "not-running");
        clock.set(60_101);
        service.tick(&mut store).unwrap();
        assert_eq!(
            store
                .connection_diagnostics_run_at(&link.id, None, 60_101)
                .unwrap()["latest"]["id"],
            diagnostic_id
        );
        assert!(store.load().unwrap().workers().is_empty());
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires the owned orchestrix-runtime-fixture binary; synthetic metadata RPC, no account"]
    fn worker_metadata_cache_does_not_rejuvenate_after_validity_relaxation() {
        use crate::connection_refresh::{ObservationValidity, RefreshMode, RefreshSettings};
        use crate::runtime::{ManualRefreshClock, MetadataRead};
        use std::time::{Duration, Instant};

        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("orchestrix-runtime-fixture.exe");
        assert!(binary.is_file(), "build the existing fixture binary first");
        let root = path("worker-cache.sqlite").parent().unwrap().to_owned();
        let home = root.join("synthetic-home");
        let directory = root.join("registration");
        let project = root.join("project");
        for p in [&home, &directory, &project] {
            std::fs::create_dir_all(p).unwrap();
        }
        let home = std::fs::canonicalize(home).unwrap();
        let project = std::fs::canonicalize(project).unwrap();
        let mut store = Store::open(root.join("worker-cache.sqlite")).unwrap();
        let clock = Arc::new(ManualRefreshClock::new(100));
        let mut service = ConnectionService::with_clock(clock.clone());
        let reads = || {
            std::fs::read_to_string(home.join("metadata-reads"))
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };
        service
            .handle(
                &mut store,
                &ConnectionRequest::Link {
                    command_id: "cache-link".into(),
                    id: "cache-fixture".into(),
                    expected_revision: None,
                    name: "Synthetic cache".into(),
                    runtime: RuntimeChoice::Synthetic {
                        executable: binary.clone(),
                        home: home.clone(),
                        scenario: "connection-metadata".into(),
                    },
                    metadata_directory: std::fs::canonicalize(directory).unwrap(),
                    executable_sha256: runtime_executable_sha256(&binary).unwrap(),
                    allow_metadata_read: true,
                },
                false,
            )
            .unwrap();
        let until = Instant::now() + Duration::from_secs(20);
        while !service.is_idle() {
            service.drain(&mut store).unwrap();
            assert!(Instant::now() < until, "owned registration read completes");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(reads(), 1);
        let launch = store
            .connection_link("cache-fixture")
            .unwrap()
            .unwrap()
            .launch()
            .unwrap()
            .with_cwd(project);
        // Production workers and ConnectionService use this same read pool.
        let discovery = service.metadata.clone();
        let read = || {
            discovery
                .read(
                    launch.clone(),
                    "cache-fixture",
                    1,
                    true,
                    &CancellationToken::new(),
                )
                .unwrap()
        };
        clock.set(1_000);
        let MetadataRead::Ready {
            metadata: original,
            cached: false,
            ..
        } = read()
        else {
            panic!("initial worker-scope observation")
        };
        assert_eq!(original.observed_at_ms, 1_000);
        assert_eq!(reads(), 2);
        let configure = |service: &mut ConnectionService,
                         store: &mut Store,
                         revision: u64,
                         settings: RefreshSettings| {
            service
                .handle(
                    store,
                    &ConnectionRequest::ConfigureRefresh {
                        command_id: format!("cache-config-{revision}"),
                        id: "cache-fixture".into(),
                        expected_revision: 1,
                        expected_refresh_revision: revision,
                        settings,
                    },
                    false,
                )
                .unwrap();
        };
        let strict = RefreshSettings {
            validity_ms: ObservationValidity {
                identity: 1_000,
                ..ObservationValidity::default()
            },
            ..RefreshSettings::default()
        };
        configure(&mut service, &mut store, 0, strict.clone());
        let MetadataRead::Ready {
            metadata: strict_observation,
            cached: false,
            ..
        } = read()
        else {
            panic!("TTL mutation invalidates worker cache")
        };
        assert_eq!(reads(), 3);
        let automatic = RefreshSettings {
            mode: RefreshMode::Automatic,
            recurring_consent: true,
            interval_ms: 60_000,
            ..strict.clone()
        };
        configure(&mut service, &mut store, 1, automatic.clone());
        assert!(matches!(read(), MetadataRead::Ready { cached: true, .. }));
        assert_eq!(
            reads(),
            3,
            "cadence-only change retains valid source observation"
        );
        clock.set(2_001);
        assert_eq!(strict_observation.observed_at_ms, 1_000);
        let relaxed = RefreshSettings {
            validity_ms: ObservationValidity::default(),
            ..automatic.clone()
        };
        configure(&mut service, &mut store, 2, relaxed.clone());
        let MetadataRead::Ready {
            metadata: fresh,
            cached: false,
            ..
        } = read()
        else {
            panic!("expired strict observation must not regain authority after relaxation")
        };
        assert_eq!(fresh.observed_at_ms, 2_001);
        assert_eq!(
            strict_observation.observed_at_ms, 1_000,
            "old snapshot retains source time"
        );
        assert_eq!(reads(), 4);

        // Change validity while a different scope is physically reading. Its
        // detached ticket must neither publish later nor open another slot
        // until its own process shutdown has completed.
        std::fs::write(home.join("connection-metadata-hang"), b"owned barrier").unwrap();
        configure(&mut service, &mut store, 3, automatic);
        let reader = discovery.clone();
        let pending_launch = launch.clone();
        let pending = std::thread::spawn(move || {
            reader.read(
                pending_launch,
                "cache-fixture",
                1,
                true,
                &CancellationToken::new(),
            )
        });
        let until = Instant::now() + Duration::from_secs(5);
        while reads() != 5 {
            assert!(Instant::now() < until, "owned worker metadata RPC starts");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(matches!(read(), MetadataRead::Coalesced));
        assert_eq!(reads(), 5);
        configure(&mut service, &mut store, 4, relaxed);
        std::fs::remove_file(home.join("connection-metadata-hang")).unwrap();
        let after_invalidation = read();
        assert!(
            pending.join().unwrap().is_err(),
            "invalidated old response cannot publish"
        );
        let after_shutdown = match after_invalidation {
            MetadataRead::Busy => read(),
            ready => ready,
        };
        assert!(matches!(
            after_shutdown,
            MetadataRead::Ready { cached: false, .. }
        ));
        assert_eq!(
            reads(),
            6,
            "one replacement read, no old-response cache reuse"
        );

        let save_policy = |store: &mut Store, command: &str, ttl: u64| {
            let revision = store.desktop_snapshot().unwrap()["revision"]
                .as_u64()
                .unwrap();
            let mut settings = crate::desktop::Settings::default();
            settings.policy.observation_ttls.identity_billing_ms = ttl;
            crate::desktop::handle(
                store,
                &crate::desktop::DesktopRequest::SaveSettings {
                    command_id: command.into(),
                    expected_revision: revision,
                    settings: Box::new(settings),
                },
            )
            .unwrap();
        };
        save_policy(&mut store, "strict-policy-cache", 1_000);
        service.invalidate_policy_observations(&store).unwrap();
        assert!(matches!(read(), MetadataRead::Ready { cached: false, .. }));
        assert_eq!(reads(), 7);
        clock.set(3_002);
        save_policy(&mut store, "relax-policy-cache", 300_000);
        service.invalidate_policy_observations(&store).unwrap();
        let MetadataRead::Ready {
            metadata,
            cached: false,
            ..
        } = read()
        else {
            panic!("policy relaxation also invalidates prior discovery")
        };
        assert_eq!(metadata.observed_at_ms, 3_002);
        assert_eq!(reads(), 8);
        assert!(!home.join("dispatch-count").exists());
        assert!(!std::fs::read_dir(home).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("fixture-thread-")
        }));
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires the owned orchestrix-runtime-fixture binary built by integration tests; metadata only"]
    fn periodic_service_uses_real_fixture_process_cache_cancel_and_revocation() {
        use crate::runtime::ManualRefreshClock;
        use std::time::{Duration, Instant};
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("orchestrix-runtime-fixture.exe");
        assert!(binary.is_file(), "build the existing fixture binary first");
        let root = path("service.sqlite").parent().unwrap().to_owned();
        let home = root.join("managed-fixture-home");
        let directory = root.join("metadata");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let home = std::fs::canonicalize(home).unwrap();
        let directory = std::fs::canonicalize(directory).unwrap();
        let mut store = Store::open(root.join("service.sqlite")).unwrap();
        let clock = Arc::new(ManualRefreshClock::new(100));
        let mut service = ConnectionService::with_clock(clock.clone());
        let reads = || {
            std::fs::read_to_string(home.join("metadata-reads"))
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let settle = |service: &mut ConnectionService, store: &mut Store| {
            let until = Instant::now() + Duration::from_secs(20);
            while !service.is_idle() {
                service.drain(store).unwrap();
                assert!(Instant::now() < until, "owned metadata completion");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        service
            .handle(
                &mut store,
                &ConnectionRequest::Link {
                    command_id: "fixture-link".into(),
                    id: "fixture-cadence".into(),
                    expected_revision: None,
                    name: "Synthetic cadence".into(),
                    runtime: RuntimeChoice::Synthetic {
                        executable: binary.clone(),
                        home: home.clone(),
                        scenario: "connection-metadata".into(),
                    },
                    metadata_directory: directory,
                    executable_sha256: runtime_executable_sha256(&binary).unwrap(),
                    allow_metadata_read: true,
                },
                false,
            )
            .unwrap();
        settle(&mut service, &mut store);
        assert_eq!(reads(), 1);
        let settings = RefreshSettings {
            mode: RefreshMode::Automatic,
            recurring_consent: true,
            interval_ms: 60_000,
            ..RefreshSettings::default()
        };
        service
            .handle(
                &mut store,
                &ConnectionRequest::ConfigureRefresh {
                    command_id: "fixture-recurring".into(),
                    id: "fixture-cadence".into(),
                    expected_revision: 1,
                    expected_refresh_revision: 0,
                    settings,
                },
                false,
            )
            .unwrap();
        assert_eq!(reads(), 1);
        clock.set(200);
        service
            .handle(
                &mut store,
                &ConnectionRequest::Diagnose {
                    command_id: "fixture-manual-cache".into(),
                    id: "fixture-cadence".into(),
                    expected_revision: 1,
                    allow_metadata_read: true,
                },
                false,
            )
            .unwrap();
        settle(&mut service, &mut store);
        assert_eq!(reads(), 1);
        let cached = store
            .connection_diagnostics_run_at("fixture-cadence", None, 200)
            .unwrap();
        assert_eq!(cached["latest"]["cached"], true);
        assert_eq!(cached["latest"]["observedAtMs"], 100);
        service.tick(&mut store).unwrap();
        clock.set(60_200);
        service.tick(&mut store).unwrap();
        settle(&mut service, &mut store);
        assert_eq!(reads(), 2);
        let fresh = store
            .connection_diagnostics_run_at("fixture-cadence", None, 60_200)
            .unwrap();
        assert_eq!(fresh["latest"]["observedAtMs"], 60_200);
        assert_eq!(fresh["latest"]["cached"], false);
        assert!(
            fresh["latest"]["id"]
                .as_str()
                .unwrap()
                .starts_with("periodic-")
        );
        clock.set(9_000_000);
        service.tick(&mut store).unwrap();
        assert!(service.is_idle());
        assert_eq!(reads(), 2);
        std::fs::write(home.join("connection-metadata-hang"), b"owned test barrier").unwrap();
        clock.set(9_060_000);
        service.tick(&mut store).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while reads() != 3 {
            assert!(Instant::now() < until, "owned fixture start");
            std::thread::sleep(Duration::from_millis(5));
        }
        let status = service
            .refresh_status(&store, Some("fixture-cadence"))
            .unwrap();
        let diagnostic_id = status["refresh"][0]["activeDiagnosticId"]
            .as_str()
            .unwrap()
            .to_owned();
        service
            .handle(
                &mut store,
                &ConnectionRequest::CancelDiagnostic {
                    command_id: "fixture-cancel".into(),
                    id: "fixture-cadence".into(),
                    expected_revision: 1,
                    diagnostic_id,
                },
                false,
            )
            .unwrap();
        settle(&mut service, &mut store);
        let cancelled = store
            .connection_diagnostics_run_at("fixture-cadence", None, 9_060_000)
            .unwrap();
        assert_eq!(cancelled["latest"]["state"], "cancelled");
        assert_eq!(cancelled["lastSuccessfulAtMs"], 60_200);
        service
            .handle(
                &mut store,
                &ConnectionRequest::ConfigureRefresh {
                    command_id: "fixture-revoke".into(),
                    id: "fixture-cadence".into(),
                    expected_revision: 1,
                    expected_refresh_revision: 1,
                    settings: RefreshSettings::default(),
                },
                false,
            )
            .unwrap();
        clock.set(18_000_000);
        service.tick(&mut store).unwrap();
        assert!(service.is_idle());
        assert_eq!(reads(), 3);
        assert!(!home.join("dispatch-count").exists());
        assert!(!std::fs::read_dir(home).unwrap().any(|r| {
            r.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("fixture-thread-")
        }));
    }
}
