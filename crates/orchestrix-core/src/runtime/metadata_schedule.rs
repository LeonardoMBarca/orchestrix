//! Bounded host-driven periodic/event refresh. Registering never starts a process.
//! The host calls `tick`; this component owns no daemon, inference or persistence.
use super::refresh::MetadataResource;
use super::{
    CancellationToken, CodexMetadataService, MetadataRead, RefreshClock, RuntimeError,
    RuntimeLaunch, RuntimeResult,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub struct MetadataScheduleBounds {
    pub max_connections: usize,
    pub max_reads_per_tick: usize,
    pub max_in_flight: usize,
    pub minimum_interval_ms: u64,
}
impl Default for MetadataScheduleBounds {
    fn default() -> Self {
        Self {
            max_connections: 64,
            max_reads_per_tick: 4,
            max_in_flight: 4,
            minimum_interval_ms: 1_000,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MetadataRegistration {
    pub connection_id: String,
    pub revision: u64,
    pub launch: RuntimeLaunch,
    pub authorized: bool,
    pub enabled: bool,
    pub interval_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataRefreshTrigger {
    Periodic,
    RequirementNeeded,
    Reconnected,
    ConfigurationChanged,
    IdentityChanged,
    RuntimeVersionChanged,
    ResourceError(MetadataResource),
}
#[derive(Clone, Debug)]
pub enum ScheduledMetadataOutcome {
    Read(MetadataRead),
    Failed(RuntimeError),
    /// The host replaced/removed this connection while its read was pending.
    Discarded,
}
#[derive(Clone, Debug)]
pub struct ScheduledMetadataUpdate {
    pub connection_id: String,
    pub revision: u64,
    pub trigger: MetadataRefreshTrigger,
    pub outcome: ScheduledMetadataOutcome,
}
struct Registration {
    value: MetadataRegistration,
    scope: String,
    generation: u64,
    due_ms: u64,
    last_start_ms: Option<u64>,
    trigger: MetadataRefreshTrigger,
    paused: bool,
}
struct State {
    registrations: BTreeMap<String, Registration>,
    // Retained until the old call settles even when a registration is replaced.
    busy: BTreeSet<String>,
    generation: u64,
    last_tick_ms: u64,
}
/// Clones share registrations, coalescence, limits and cancellation of owned reads.
#[derive(Clone)]
pub struct MetadataRefreshCoordinator {
    service: CodexMetadataService,
    clock: Arc<dyn RefreshClock>,
    bounds: MetadataScheduleBounds,
    state: Arc<Mutex<State>>,
}
impl MetadataRefreshCoordinator {
    pub fn new(
        service: CodexMetadataService,
        clock: Arc<dyn RefreshClock>,
        bounds: MetadataScheduleBounds,
    ) -> Option<Self> {
        if bounds.max_connections == 0
            || bounds.max_connections > 64
            || bounds.max_reads_per_tick == 0
            || bounds.max_reads_per_tick > 16
            || bounds.max_in_flight == 0
            || bounds.max_in_flight > 16
            || bounds.minimum_interval_ms < 1_000
            || bounds.minimum_interval_ms > 900_000
        {
            return None;
        }
        Some(Self {
            service,
            clock,
            bounds,
            state: Arc::new(Mutex::new(State {
                registrations: BTreeMap::new(),
                busy: BTreeSet::new(),
                generation: 0,
                last_tick_ms: 0,
            })),
        })
    }
    /// No RPC/process is launched. A changed executable/auth-store/directory or
    /// runtime version requires a higher connection revision.
    pub fn register(&self, value: MetadataRegistration) -> RuntimeResult<()> {
        if value.connection_id.is_empty()
            || value.connection_id.len() > 80
            || !value
                .connection_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
            || value.revision == 0
            || value.interval_ms < self.bounds.minimum_interval_ms
            || value.interval_ms > 900_000
        {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        let scope = value.launch.metadata_scope()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| RuntimeError::new("transport-loss", "not-sent"))?;
        if !state.registrations.contains_key(&value.connection_id)
            && state.registrations.len() >= self.bounds.max_connections
        {
            return Err(RuntimeError::new("budget", "not-sent"));
        }
        let old = state.registrations.get(&value.connection_id);
        if old.is_some_and(|old| {
            old.value.revision > value.revision
                || (old.value.revision == value.revision && old.scope != scope)
        }) {
            return Err(RuntimeError::new("ownership", "not-sent"));
        }
        let last_start = old.and_then(|old| old.last_start_ms);
        let revised = old.is_none_or(|old| old.value.revision < value.revision);
        let revoked = !value.enabled || !value.authorized;
        let now = self.clock.now_ms();
        let due_ms = last_start.map_or(now, |last| {
            now.max(last.saturating_add(self.bounds.minimum_interval_ms))
        });
        state.generation = state.generation.saturating_add(1);
        let generation = state.generation;
        let connection = value.connection_id.clone();
        let revision = value.revision;
        state.registrations.insert(
            connection.clone(),
            Registration {
                value,
                scope,
                generation,
                due_ms,
                last_start_ms: last_start,
                trigger: MetadataRefreshTrigger::Reconnected,
                paused: false,
            },
        );
        if revised {
            self.service.invalidate_connection(&connection, revision);
        } else if revoked {
            self.service.invalidate_observations(&connection, revision);
        }
        Ok(())
    }
    /// Old-revision events are ignored. Metadata-only refresh never probes a
    /// model's inference or turns a partial catalog into an entitlement.
    pub fn trigger(
        &self,
        connection_id: &str,
        revision: u64,
        trigger: MetadataRefreshTrigger,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state
            .registrations
            .get(connection_id)
            .is_none_or(|entry| entry.value.revision != revision)
        {
            return false;
        }
        let invalidates = !matches!(
            trigger,
            MetadataRefreshTrigger::Periodic | MetadataRefreshTrigger::RequirementNeeded
        );
        if invalidates {
            self.service
                .invalidate_observations(connection_id, revision);
            state.generation = state.generation.saturating_add(1);
        }
        let generation = state.generation;
        let entry = state.registrations.get_mut(connection_id).unwrap();
        if invalidates {
            entry.generation = generation;
        }
        if matches!(
            trigger,
            MetadataRefreshTrigger::Reconnected
                | MetadataRefreshTrigger::ConfigurationChanged
                | MetadataRefreshTrigger::IdentityChanged
                | MetadataRefreshTrigger::RuntimeVersionChanged
        ) {
            entry.paused = false;
        }
        let now = self.clock.now_ms();
        entry.due_ms = entry.last_start_ms.map_or(now, |last| {
            now.max(last.saturating_add(self.bounds.minimum_interval_ms))
        });
        entry.trigger = trigger;
        true
    }
    /// Disconnect/revocation cancels only reads owned by this metadata service.
    pub fn remove(&self, connection_id: &str, revision: u64) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state
            .registrations
            .get(connection_id)
            .is_none_or(|entry| entry.value.revision != revision)
        {
            return false;
        }
        state.registrations.remove(connection_id);
        self.service
            .invalidate_observations(connection_id, revision);
        true
    }
    /// The host explicitly invokes this bounded tick. No lock/database lease
    /// spans a read, and concurrent cloned ticks cannot duplicate one connection.
    pub fn tick(&self, cancel: &CancellationToken) -> RuntimeResult<Vec<ScheduledMetadataUpdate>> {
        let mut updates = Vec::new();
        let mut considered = BTreeSet::new();
        for _ in 0..self.bounds.max_reads_per_tick {
            if cancel.is_cancelled() {
                break;
            }
            let now = self.clock.now_ms();
            let selected = {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| RuntimeError::new("transport-loss", "not-sent"))?;
                if now < state.last_tick_ms {
                    return Err(RuntimeError::new("timeout", "not-sent"));
                }
                state.last_tick_ms = now;
                if state.busy.len() >= self.bounds.max_in_flight {
                    break;
                }
                let selected = state
                    .registrations
                    .iter()
                    .filter(|(id, entry)| {
                        !considered.contains(*id)
                            && !state.busy.contains(*id)
                            && entry.value.authorized
                            && entry.value.enabled
                            && !entry.paused
                            && now >= entry.due_ms
                    })
                    .min_by_key(|(id, entry)| (entry.due_ms, *id))
                    .map(|(id, entry)| {
                        (
                            id.clone(),
                            entry.value.clone(),
                            entry.generation,
                            entry.trigger,
                        )
                    });
                if let Some((id, _, _, _)) = &selected {
                    state.busy.insert(id.clone());
                    state.registrations.get_mut(id).unwrap().last_start_ms = Some(now);
                }
                selected
            };
            let Some((id, registration, generation, trigger)) = selected else {
                break;
            };
            considered.insert(id.clone());
            let result = self.service.read(
                registration.launch,
                &id,
                registration.revision,
                true,
                cancel,
            );
            let now = self.clock.now_ms();
            let mut state = self
                .state
                .lock()
                .map_err(|_| RuntimeError::new("transport-loss", "not-sent"))?;
            state.busy.remove(&id);
            let outcome = if let Some(entry) = state
                .registrations
                .get_mut(&id)
                .filter(|entry| entry.generation == generation)
            {
                entry.due_ms = now.saturating_add(entry.value.interval_ms);
                match result {
                    Ok(read) => {
                        if let MetadataRead::Backoff { until_ms } = &read {
                            entry.due_ms = entry.due_ms.max(*until_ms);
                        }
                        ScheduledMetadataOutcome::Read(read)
                    }
                    Err(error) => {
                        // Permission/auth/config failures require a host event;
                        // periodic refresh must not spin on failed credentials.
                        entry.paused = matches!(
                            error.code(),
                            "authentication" | "policy" | "version" | "unsupported" | "ownership"
                        );
                        ScheduledMetadataOutcome::Failed(error)
                    }
                }
            } else {
                ScheduledMetadataOutcome::Discarded
            };
            updates.push(ScheduledMetadataUpdate {
                connection_id: id,
                revision: registration.revision,
                trigger,
                outcome,
            });
        }
        Ok(updates)
    }
}
