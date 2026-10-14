//! Host-driven, bounded metadata reads. This cache never performs a provider probe.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RefreshScope {
    pub connection_id: String,
    pub connection_revision: u64,
    pub runtime_version: String,
    /// Hash of the executable/auth-store/directory/config scope, never a raw path or credential.
    pub context_key: String,
    pub resource: MetadataResource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataResource {
    Identity,
    Configuration,
    Models,
    Quota,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshTicket {
    scope: RefreshScope,
    generation: u64,
    started_ms: u64,
    deadline_ms: u64,
}
impl RefreshTicket {
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
    pub fn scope(&self) -> &RefreshScope {
        &self.scope
    }
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}
#[derive(Clone, Debug)]
pub enum RefreshAction<T> {
    Cached(T),
    Start(RefreshTicket),
    Coalesced(RefreshTicket),
    Backoff { until_ms: u64 },
    Busy,
    Unauthorized,
    Invalid,
}
#[derive(Clone, Debug)]
pub enum RefreshResult<T> {
    Complete(T),
    Partial(T),
    Failed,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefreshCompletion {
    Stored,
    Partial,
    Failed,
    Unsupported,
    Stale,
}
#[derive(Clone, Copy, Debug)]
pub struct RefreshBounds {
    pub max_entries: usize,
    pub max_in_flight: usize,
    pub ttl_ms: u64,
    pub deadline_ms: u64,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}
impl Default for RefreshBounds {
    fn default() -> Self {
        Self {
            max_entries: 64,
            max_in_flight: 4,
            ttl_ms: 60_000,
            deadline_ms: 15_000,
            initial_backoff_ms: 1000,
            max_backoff_ms: 60_000,
        }
    }
}
struct Entry<T> {
    cached: Option<(T, u64, u64)>,
    flight: Option<RefreshTicket>,
    failures: u32,
    backoff_until: u64,
}
struct State<T> {
    entries: BTreeMap<RefreshScope, Entry<T>>,
    revisions: BTreeMap<String, u64>,
    generation: u64,
}
/// Clone shares cache and in-flight tickets, so concurrent consumers coalesce.
pub struct RefreshCache<T> {
    state: Arc<Mutex<State<T>>>,
    bounds: RefreshBounds,
}
impl<T> Clone for RefreshCache<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            bounds: self.bounds,
        }
    }
}
impl<T: Clone> RefreshCache<T> {
    pub fn new(bounds: RefreshBounds) -> Option<Self> {
        if bounds.max_entries == 0
            || bounds.max_entries > 256
            || bounds.max_in_flight == 0
            || bounds.max_in_flight > 16
            || bounds.ttl_ms == 0
            || bounds.ttl_ms > 900_000
            || bounds.deadline_ms == 0
            || bounds.deadline_ms > 15_000
            || bounds.initial_backoff_ms == 0
            || bounds.max_backoff_ms > 300_000
            || bounds.max_backoff_ms < bounds.initial_backoff_ms
        {
            return None;
        }
        Some(Self {
            state: Arc::new(Mutex::new(State {
                entries: BTreeMap::new(),
                revisions: BTreeMap::new(),
                generation: 0,
            })),
            bounds,
        })
    }
    /// `authorized` means permission for this read-only metadata surface, never paid probes.
    /// Time comes from the caller's controlled clock; a backwards jump never renews a lease.
    pub fn begin(&self, scope: RefreshScope, now_ms: u64, authorized: bool) -> RefreshAction<T> {
        if !authorized {
            return RefreshAction::Unauthorized;
        }
        if !valid_scope(&scope) {
            return RefreshAction::Invalid;
        }
        let Ok(mut state) = self.state.lock() else {
            return RefreshAction::Invalid;
        };
        if !state.revisions.contains_key(&scope.connection_id)
            && state.revisions.len() >= self.bounds.max_entries
        {
            return RefreshAction::Busy;
        }
        if state
            .revisions
            .get(&scope.connection_id)
            .is_some_and(|revision| *revision > scope.connection_revision)
        {
            return RefreshAction::Invalid;
        }
        for entry in state.entries.values_mut() {
            if entry
                .flight
                .as_ref()
                .is_some_and(|ticket| now_ms >= ticket.deadline_ms)
            {
                entry.flight = None;
                entry.failures = entry.failures.saturating_add(1);
                entry.backoff_until = now_ms.saturating_add(backoff(self.bounds, entry.failures));
            }
        }
        let prior_revision = state
            .revisions
            .insert(scope.connection_id.clone(), scope.connection_revision);
        if prior_revision.is_some_and(|prior| prior < scope.connection_revision) {
            state
                .entries
                .retain(|key, _| key.connection_id != scope.connection_id);
        }
        if let Some(entry) = state.entries.get_mut(&scope) {
            if let Some((value, observed, valid_until)) = &entry.cached
                && now_ms >= *observed
                && now_ms < *valid_until
            {
                return RefreshAction::Cached(value.clone());
            }
            if let Some(ticket) = &entry.flight {
                if now_ms < ticket.deadline_ms {
                    return RefreshAction::Coalesced(ticket.clone());
                }
                entry.flight = None;
                entry.failures = entry.failures.saturating_add(1);
                entry.backoff_until = now_ms.saturating_add(backoff(self.bounds, entry.failures));
            }
            if now_ms < entry.backoff_until {
                return RefreshAction::Backoff {
                    until_ms: entry.backoff_until,
                };
            }
        }
        if state
            .entries
            .values()
            .filter(|entry| entry.flight.is_some())
            .count()
            >= self.bounds.max_in_flight
        {
            return RefreshAction::Busy;
        }
        if !state.entries.contains_key(&scope) && state.entries.len() >= self.bounds.max_entries {
            let removable = state
                .entries
                .iter()
                .find(|(_, entry)| {
                    entry.flight.is_none()
                        && entry
                            .cached
                            .as_ref()
                            .is_none_or(|(_, _, until)| *until <= now_ms)
                        && entry.backoff_until <= now_ms
                })
                .map(|(key, _)| key.clone());
            if let Some(key) = removable {
                state.entries.remove(&key);
            } else {
                return RefreshAction::Busy;
            }
        }
        state.generation = state.generation.saturating_add(1);
        let ticket = RefreshTicket {
            scope: scope.clone(),
            generation: state.generation,
            started_ms: now_ms,
            deadline_ms: now_ms.saturating_add(self.bounds.deadline_ms),
        };
        state
            .entries
            .entry(scope)
            .or_insert(Entry {
                cached: None,
                flight: None,
                failures: 0,
                backoff_until: 0,
            })
            .flight = Some(ticket.clone());
        RefreshAction::Start(ticket)
    }
    pub fn finish(
        &self,
        ticket: &RefreshTicket,
        now_ms: u64,
        result: RefreshResult<T>,
    ) -> RefreshCompletion {
        let Ok(mut state) = self.state.lock() else {
            return RefreshCompletion::Stale;
        };
        let Some(entry) = state.entries.get_mut(&ticket.scope) else {
            return RefreshCompletion::Stale;
        };
        if entry.flight.as_ref() != Some(ticket) {
            return RefreshCompletion::Stale;
        }
        entry.flight = None;
        if now_ms < ticket.started_ms || now_ms >= ticket.deadline_ms {
            entry.failures = entry.failures.saturating_add(1);
            entry.backoff_until = now_ms.saturating_add(backoff(self.bounds, entry.failures));
            return RefreshCompletion::Stale;
        }
        match result {
            RefreshResult::Complete(value) => {
                entry.cached = Some((value, now_ms, now_ms.saturating_add(self.bounds.ttl_ms)));
                entry.failures = 0;
                entry.backoff_until = 0;
                RefreshCompletion::Stored
            }
            RefreshResult::Partial(_) => {
                entry.failures = entry.failures.saturating_add(1);
                entry.backoff_until = now_ms.saturating_add(backoff(self.bounds, entry.failures));
                RefreshCompletion::Partial
            }
            RefreshResult::Failed => {
                entry.failures = entry.failures.saturating_add(1);
                entry.backoff_until = now_ms.saturating_add(backoff(self.bounds, entry.failures));
                RefreshCompletion::Failed
            }
            RefreshResult::Unsupported => {
                entry.backoff_until = now_ms.saturating_add(self.bounds.max_backoff_ms);
                RefreshCompletion::Unsupported
            }
        }
    }
    /// Invalidates both cached data and the owned logical read. An old response cannot reinsert it.
    pub fn cancel(&self, ticket: &RefreshTicket) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let Some(entry) = state.entries.get_mut(&ticket.scope) else {
            return false;
        };
        if entry.flight.as_ref() != Some(ticket) {
            return false;
        }
        entry.flight = None;
        true
    }
    pub fn invalidate_connection(&self, connection_id: &str, new_revision: u64) {
        if let Ok(mut state) = self.state.lock() {
            if !state.revisions.contains_key(connection_id)
                && state.revisions.len() >= self.bounds.max_entries
            {
                return;
            }
            let revision = state.revisions.entry(connection_id.into()).or_default();
            *revision = (*revision).max(new_revision);
            state
                .entries
                .retain(|scope, _| scope.connection_id != connection_id);
        }
    }
    /// A resource failure makes prior facts unusable but must not reset retry
    /// backoff. This also detaches a pending read, whose late result is stale.
    pub fn invalidate_observations(&self, connection_id: &str, revision: u64) {
        if let Ok(mut state) = self.state.lock() {
            for (scope, entry) in &mut state.entries {
                if scope.connection_id == connection_id && scope.connection_revision == revision {
                    entry.cached = None;
                    entry.flight = None;
                }
            }
        }
    }
}
fn backoff(bounds: RefreshBounds, failures: u32) -> u64 {
    bounds
        .initial_backoff_ms
        .saturating_mul(1u64 << failures.saturating_sub(1).min(20))
        .min(bounds.max_backoff_ms)
}
fn valid_scope(scope: &RefreshScope) -> bool {
    !scope.connection_id.is_empty()
        && scope.connection_id.len() <= 80
        && scope
            .connection_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && scope.connection_revision > 0
        && !scope.context_key.is_empty()
        && scope.context_key.len() <= 80
        && scope
            .context_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && !scope.runtime_version.is_empty()
        && scope.runtime_version.len() <= 100
        && scope
            .runtime_version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(revision: u64) -> RefreshScope {
        RefreshScope {
            connection_id: "connection".into(),
            connection_revision: revision,
            runtime_version: "test-1".into(),
            context_key: "config-scope".into(),
            resource: MetadataResource::Models,
        }
    }
    #[test]
    fn leases_coalesce_expire_and_reject_stale_revisions() {
        let cache = RefreshCache::new(RefreshBounds::default()).unwrap();
        assert!(matches!(
            cache.begin(scope(1), 100, false),
            RefreshAction::Unauthorized
        ));
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 100, true) else {
            panic!()
        };
        assert!(matches!(
            cache.clone().begin(scope(1), 101, true),
            RefreshAction::Coalesced(_)
        ));
        assert_eq!(
            cache.finish(&ticket, 200, RefreshResult::Complete("model-a")),
            RefreshCompletion::Stored
        );
        assert!(matches!(
            cache.begin(scope(1), 201, true),
            RefreshAction::Cached("model-a")
        ));
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 60_201, true) else {
            panic!()
        };
        cache.invalidate_connection("connection", 2);
        assert_eq!(
            cache.finish(&ticket, 60_202, RefreshResult::Complete("stale")),
            RefreshCompletion::Stale
        );
        assert!(matches!(
            cache.begin(scope(1), 60_203, true),
            RefreshAction::Invalid
        ));
        assert!(matches!(
            cache.begin(scope(2), 60_203, true),
            RefreshAction::Start(_)
        ));
    }
    #[test]
    fn partial_timeout_and_cancel_never_confirm_removal() {
        let cache: RefreshCache<Vec<&str>> = RefreshCache::new(RefreshBounds::default()).unwrap();
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 0, true) else {
            panic!()
        };
        assert_eq!(
            cache.finish(&ticket, 10, RefreshResult::Partial(vec![])),
            RefreshCompletion::Partial
        );
        assert!(matches!(
            cache.begin(scope(1), 11, true),
            RefreshAction::Backoff { until_ms: 1010 }
        ));
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 1010, true) else {
            panic!()
        };
        assert!(cache.cancel(&ticket));
        assert_eq!(
            cache.finish(&ticket, 1011, RefreshResult::Complete(vec![])),
            RefreshCompletion::Stale
        );
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 1012, true) else {
            panic!()
        };
        assert_eq!(
            cache.finish(
                &ticket,
                ticket.deadline_ms(),
                RefreshResult::Complete(vec![])
            ),
            RefreshCompletion::Stale
        );
        assert!(matches!(
            cache.begin(scope(1), ticket.deadline_ms(), true),
            RefreshAction::Backoff { .. }
        ));
    }
    #[test]
    fn bounded_flights_expire_across_scopes_and_backwards_time_is_not_fresh() {
        let cache: RefreshCache<&str> = RefreshCache::new(RefreshBounds {
            max_entries: 2,
            max_in_flight: 1,
            ..RefreshBounds::default()
        })
        .unwrap();
        let RefreshAction::Start(first) = cache.begin(scope(1), 100, true) else {
            panic!()
        };
        let mut second = scope(1);
        second.context_key = "another-directory".into();
        assert!(matches!(
            cache.begin(second.clone(), 101, true),
            RefreshAction::Busy
        ));
        assert!(matches!(
            cache.begin(second, 15_100, true),
            RefreshAction::Start(_)
        ));
        assert_eq!(
            cache.finish(&first, 15_101, RefreshResult::Complete("late")),
            RefreshCompletion::Stale
        );
        let cache = RefreshCache::new(RefreshBounds::default()).unwrap();
        let RefreshAction::Start(ticket) = cache.begin(scope(1), 100, true) else {
            panic!()
        };
        assert_eq!(
            cache.finish(&ticket, 99, RefreshResult::Complete("from-future")),
            RefreshCompletion::Stale
        );
        assert!(matches!(
            cache.begin(scope(1), 100, true),
            RefreshAction::Backoff { .. }
        ));
    }
}
