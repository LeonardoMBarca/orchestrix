//! Read-only Codex discovery wired to the scope/revision cache. No inference, login or paid probes.
use super::refresh::{
    MetadataResource, RefreshAction, RefreshBounds, RefreshCache, RefreshCompletion, RefreshResult,
};
use super::{
    CancellationToken, CodexAdapter, RuntimeError, RuntimeLaunch, RuntimeMetadata, RuntimeResult,
};
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub trait RefreshClock: Send + Sync {
    fn now_ms(&self) -> u64;
}
#[derive(Default)]
pub struct SystemRefreshClock;
impl RefreshClock for SystemRefreshClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0)
    }
}
#[derive(Default)]
pub struct ManualRefreshClock(AtomicU64);
impl ManualRefreshClock {
    pub fn new(now_ms: u64) -> Self {
        Self(AtomicU64::new(now_ms))
    }
    pub fn set(&self, now_ms: u64) {
        self.0.store(now_ms, Ordering::Release);
    }
}
impl RefreshClock for ManualRefreshClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}
#[derive(Clone, Debug)]
pub enum MetadataRead {
    Ready {
        metadata: RuntimeMetadata,
        cached: bool,
        complete: bool,
    },
    Coalesced,
    Backoff {
        until_ms: u64,
    },
    Busy,
}
struct Active {
    connection_id: String,
    revision: u64,
    token: CancellationToken,
}
#[derive(Clone)]
pub struct CodexMetadataService {
    cache: RefreshCache<RuntimeMetadata>,
    clock: Arc<dyn RefreshClock>,
    active: Arc<Mutex<BTreeMap<u64, Active>>>,
    deadline: Duration,
}
impl CodexMetadataService {
    pub fn new(clock: Arc<dyn RefreshClock>, bounds: RefreshBounds) -> Option<Self> {
        Some(Self {
            cache: RefreshCache::new(bounds)?,
            clock,
            active: Arc::new(Mutex::new(BTreeMap::new())),
            deadline: Duration::from_millis(bounds.deadline_ms),
        })
    }
    /// Cached answers retain their original observed time. Partial/failed reads do not renew a lease.
    /// Coalesced consumers receive the existing flight's status and may read the cache after it settles.
    pub fn read(
        &self,
        mut launch: RuntimeLaunch,
        connection_id: &str,
        revision: u64,
        authorized: bool,
        cancel: &CancellationToken,
    ) -> RuntimeResult<MetadataRead> {
        if !authorized {
            return Err(RuntimeError::new("policy", "not-sent"));
        }
        if cancel.is_cancelled() {
            return Err(RuntimeError::new("timeout", "not-sent"));
        }
        let scope = super::refresh::RefreshScope {
            connection_id: connection_id.into(),
            connection_revision: revision,
            runtime_version: launch.version().into(),
            context_key: launch.metadata_scope()?,
            resource: MetadataResource::Configuration,
        };
        let ticket = match self.cache.begin(scope, self.clock.now_ms(), true) {
            RefreshAction::Cached(metadata) => {
                return Ok(MetadataRead::Ready {
                    metadata,
                    cached: true,
                    complete: true,
                });
            }
            RefreshAction::Start(ticket) => ticket,
            RefreshAction::Coalesced(_) => return Ok(MetadataRead::Coalesced),
            RefreshAction::Backoff { until_ms } => return Ok(MetadataRead::Backoff { until_ms }),
            RefreshAction::Busy => return Ok(MetadataRead::Busy),
            RefreshAction::Unauthorized => return Err(RuntimeError::new("policy", "not-sent")),
            RefreshAction::Invalid => return Err(RuntimeError::new("ownership", "not-sent")),
        };
        let owned_cancel = CancellationToken::new();
        self.active
            .lock()
            .map_err(|_| RuntimeError::new("transport-loss", "not-sent"))?
            .insert(
                ticket.generation(),
                Active {
                    connection_id: connection_id.into(),
                    revision,
                    token: owned_cancel.clone(),
                },
            );
        let completed = Arc::new(AtomicBool::new(false));
        let watch_done = Arc::clone(&completed);
        let watch_cancel = owned_cancel.clone();
        let user_cancel = cancel.clone();
        let clock = Arc::clone(&self.clock);
        let deadline_ms = ticket.deadline_ms();
        let deadline = self.deadline;
        let watchdog = std::thread::spawn(move || {
            let began = Instant::now();
            while !watch_done.load(Ordering::Acquire) {
                if user_cancel.is_cancelled()
                    || clock.now_ms() >= deadline_ms
                    || began.elapsed() >= deadline
                {
                    watch_cancel.cancel();
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        // The owned Job deadline is a second bound if a protocol read or write stops responding.
        launch.process_limits.lifetime = launch.process_limits.lifetime.min(self.deadline);
        launch.request_timeout = launch.request_timeout.min(self.deadline);
        let result = (|| {
            let mut adapter = CodexAdapter::connect(
                launch,
                connection_id,
                revision,
                self.clock.now_ms(),
                &owned_cancel,
            )?;
            let metadata = adapter.metadata().clone();
            adapter.shutdown()?;
            Ok(metadata)
        })();
        completed.store(true, Ordering::Release);
        let _ = watchdog.join();
        self.active
            .lock()
            .map_err(|_| RuntimeError::new("transport-loss", "not-sent"))?
            .remove(&ticket.generation());
        if owned_cancel.is_cancelled() || cancel.is_cancelled() {
            if cancel.is_cancelled() {
                self.cache.cancel(&ticket);
            } else {
                self.cache
                    .finish(&ticket, self.clock.now_ms(), RefreshResult::Failed);
            }
            return Err(RuntimeError::new("timeout", "not-sent"));
        }
        match result {
            Ok(metadata) => {
                let complete =
                    metadata.descriptor.as_json()["catalog"]["completeness"] == "complete";
                let outcome = if complete {
                    RefreshResult::Complete(metadata.clone())
                } else {
                    RefreshResult::Partial(metadata.clone())
                };
                let completion = self.cache.finish(&ticket, self.clock.now_ms(), outcome);
                if completion == RefreshCompletion::Stale {
                    return Err(RuntimeError::new("ownership", "not-sent"));
                }
                Ok(MetadataRead::Ready {
                    metadata,
                    cached: false,
                    complete,
                })
            }
            Err(error) => {
                self.cache
                    .finish(&ticket, self.clock.now_ms(), RefreshResult::Failed);
                Err(error)
            }
        }
    }
    /// Cancels owned reads and invalidates cached/late responses when the host changes a connection.
    pub fn invalidate_connection(&self, connection_id: &str, new_revision: u64) {
        self.cache
            .invalidate_connection(connection_id, new_revision);
        if let Ok(active) = self.active.lock() {
            for read in active
                .values()
                .filter(|read| read.connection_id == connection_id)
            {
                read.token.cancel();
            }
        }
    }
    /// Host-observed resource errors invalidate facts without removing backoff.
    pub fn invalidate_observations(&self, connection_id: &str, revision: u64) {
        self.cache.invalidate_observations(connection_id, revision);
        if let Ok(active) = self.active.lock() {
            for read in active
                .values()
                .filter(|read| read.connection_id == connection_id && read.revision == revision)
            {
                read.token.cancel();
            }
        }
    }
}
