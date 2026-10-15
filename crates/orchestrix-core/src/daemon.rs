//! Versioned local application API. CLI and future Desktop are clients, never schedulers.
use crate::{
    git_records::content_hash,
    persistence::{DaemonCommandReceipt, DaemonCursor, Store},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ManualTaskRequest, ManualWorker},
};
use orchestrix_process::{ipc, lease::HostLease};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

pub const API_VERSION: u32 = 1;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    Snapshot,
    Workers {
        cursor: DaemonCursor,
        after_worker: Option<String>,
        limit: usize,
    },
    Query {
        query: crate::mechanical::Query,
    },
    Status {
        worker_id: String,
    },
    Events {
        cursor: DaemonCursor,
        limit: usize,
    },
    Submit {
        command_id: String,
        authorization_ref: String,
        task: Box<ManualTaskRequest>,
        runtime: RuntimeChoice,
    },
    Suspend {
        command_id: String,
    },
    Resume {
        command_id: String,
    },
    Cancel {
        command_id: String,
        worker_id: String,
    },
    Shutdown {
        command_id: String,
    },
    Recovery,
    Continuity {
        worker_id: String,
        budget: crate::policy::ContextBudget,
    },
    Intervene {
        command_id: String,
        worker_id: String,
        expected_revision: u64,
        authorization_ref: String,
        action: crate::recovery::RecoveryAction,
    },
    RecheckRecovery {
        command_id: String,
        worker_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RuntimeChoice {
    Codex {
        executable: PathBuf,
        home: PathBuf,
        version: String,
        allow_live: bool,
    },
    Synthetic {
        executable: PathBuf,
        home: PathBuf,
        scenario: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: u32,
    pub ok: bool,
    pub error: Option<String>,
    pub result: Value,
}
impl Response {
    fn value(result: Value) -> Self {
        Self {
            api_version: API_VERSION,
            ok: true,
            error: None,
            result,
        }
    }
    fn error(code: &str) -> Self {
        Self {
            api_version: API_VERSION,
            ok: false,
            error: Some(code.into()),
            result: Value::Null,
        }
    }
}
pub struct Client {
    scope: String,
}
impl Client {
    /// Derives the endpoint from an absolute canonical parent. Never opens SQLite.
    pub fn new(database: &Path) -> Result<Self, String> {
        Ok(Self {
            scope: endpoint_scope(database)?,
        })
    }
    pub fn request(&self, request: &Request) -> Result<Response, String> {
        ipc::clear_diagnostic();
        let body = serde_json::to_vec(request).map_err(|_| "request-schema")?;
        let mut stream =
            ipc::Stream::connect(&self.scope, IO_TIMEOUT).map_err(|_| "daemon-unavailable")?;
        stream
            .write(&body, IO_TIMEOUT)
            .map_err(|_| "request-not-confirmed")?;
        let bytes = stream
            .read(IO_TIMEOUT)
            .map_err(|_| "response-not-confirmed-query-before-retry")?;
        let response: Response = serde_json::from_slice(&bytes).map_err(|_| "response-schema")?;
        // Write completion alone does not establish that a pipe peer consumed a reply.
        // A bounded application ACK lets the server disconnect without discarding it.
        let _ = stream.write(b"received", IO_TIMEOUT);
        if response.api_version != API_VERSION {
            return Err("api-version".into());
        }
        Ok(response)
    }
}
/// A separate host lease excludes a second daemon and legacy mutating CLI host.
pub fn owner_lease(database: &Path) -> Result<HostLease, String> {
    let path = canonical_database_path(database)?;
    let mut marker = path.as_os_str().to_owned();
    marker.push(".daemon-host-lock");
    HostLease::acquire(Path::new(&marker))
        .map_err(|_| "daemon-ownership")?
        .ok_or_else(|| "daemon-already-running-or-legacy-host-active".into())
}
pub fn endpoint_scope(database: &Path) -> Result<String, String> {
    let path = canonical_database_path(database)?;
    let user = ipc::current_user_scope().map_err(|_| "local-transport-unsupported")?;
    Ok(content_hash(
        format!(
            "orchestrix-api-v1\0{user}\0{}",
            path.to_string_lossy().to_lowercase()
        )
        .as_bytes(),
    ))
}
fn canonical_database_path(database: &Path) -> Result<PathBuf, String> {
    if !database.is_absolute() || database.is_dir() {
        return Err("database-path".into());
    }
    if database.exists() {
        return database.canonicalize().map_err(|_| "database-path".into());
    }
    let parent = database
        .parent()
        .ok_or("database-parent")?
        .canonicalize()
        .map_err(|_| "database-parent")?;
    Ok(parent.join(database.file_name().ok_or("database-file")?))
}
struct Active {
    worker_id: String,
    command_id: String,
    cancel: CancellationToken,
    thread: std::thread::JoinHandle<()>,
    done: mpsc::Receiver<Result<Value, String>>,
}
/// Private bounded transport/latency facts. Never contains identities, paths,
/// requests, responses or provider output; application errors remain normalized.
#[derive(Default)]
struct ControlDiagnostics {
    recent: std::collections::VecDeque<Value>,
    errors_emitted: u8,
    slow_emitted: u8,
    stderr_enabled: bool,
}
impl ControlDiagnostics {
    fn record(
        &mut self,
        stage: &'static str,
        operation: &'static str,
        started: std::time::Instant,
        error: Option<ipc::Error>,
    ) {
        let elapsed = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let native=error.and_then(|_|ipc::last_diagnostic()).map(|d|json!({"stage":d.stage,"error":format!("{:?}",d.error),"nativeCode":d.native_code,"waitResult":d.wait_result,"elapsedMs":d.elapsed_ms,"transferredBytes":d.transferred_bytes}));
        if self.recent.len() == 16 {
            self.recent.pop_front();
        }
        self.recent.push_back(json!({"stage":stage,"operation":operation,"elapsedMs":elapsed,"error":error.map(|e|format!("{e:?}")),"native":native}));
        let emit = if error.is_some() && self.errors_emitted < 8 {
            self.errors_emitted += 1;
            true
        } else if error.is_none() && elapsed >= 250 && self.slow_emitted < 8 {
            self.slow_emitted += 1;
            true
        } else {
            false
        };
        if emit && self.stderr_enabled {
            eprintln!(
                "local-control-diagnostic {}",
                json!({"schemaVersion":1,"recent":self.recent})
            );
        }
    }
}
fn operation_tag(operation: &Operation) -> &'static str {
    match operation {
        Operation::Snapshot => "snapshot",
        Operation::Workers { .. } => "workers",
        Operation::Query { .. } => "query",
        Operation::Status { .. } => "status",
        Operation::Events { .. } => "events",
        Operation::Submit { .. } => "submit",
        Operation::Suspend { .. } => "suspend",
        Operation::Resume { .. } => "resume",
        Operation::Cancel { .. } => "cancel",
        Operation::Shutdown { .. } => "shutdown",
        Operation::Recovery => "recovery",
        Operation::Continuity { .. } => "continuity",
        Operation::Intervene { .. } => "intervene",
        Operation::RecheckRecovery { .. } => "recheck-recovery",
    }
}
struct Shutdown {
    command_id: String,
    stop: mpsc::Sender<()>,
    watchdog: Option<std::thread::JoinHandle<()>>,
}
impl Shutdown {
    fn new(command_id: String) -> Self {
        Self::with_spawn(command_id, |wait| {
            std::thread::Builder::new()
                .name("orchestrix-shutdown-deadline".into())
                .spawn(move || {
                    if matches!(
                        wait.recv_timeout(Duration::from_secs(20)),
                        Err(mpsc::RecvTimeoutError::Timeout)
                    ) {
                        std::process::exit(3);
                    }
                })
        })
    }
    fn with_spawn(
        command_id: String,
        spawn: impl FnOnce(mpsc::Receiver<()>) -> std::io::Result<std::thread::JoinHandle<()>>,
    ) -> Self {
        let (stop, wait) = mpsc::channel();
        // A shutdown cannot be acknowledged without its independent deadline.
        // Failure terminates this dedicated host before journaling acceptance.
        let watchdog = match spawn(wait) {
            Ok(thread) => thread,
            Err(_) => std::process::exit(3),
        };
        Self {
            command_id,
            stop,
            watchdog: Some(watchdog),
        }
    }
}
impl Drop for Shutdown {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.watchdog.take() {
            let _ = thread.join();
        }
    }
}
/// Dedicated-process host, never embed inside Desktop. The final shutdown deadline
/// terminates this host alone, releasing kill-on-close Jobs and preserving uncertainty.
pub fn serve(database: &Path) -> Result<(), String> {
    let _owner = owner_lease(database)?;
    let scope = endpoint_scope(database)?;
    let mut store = Store::open(database).map_err(|_| "database")?;
    store.daemon_start().map_err(|_| "daemon-start")?;
    // Recovery is deterministic. This startup never redispatches an accepted command.
    let _recovery = crate::recovery::reconcile(&mut store).map_err(|_| "reconciliation")?;
    for (id, kind, _) in store.daemon_pending().map_err(|_| "pending-commands")? {
        store.daemon_finish(&id,&json!({"state":"interrupted-before-response","kind":kind,"resumeAutomatically":false,"inspectionRequired":true})).map_err(|_|"pending-reconciliation")?;
    }
    let mut listener = ipc::Listener::bind(&scope).map_err(|_| "daemon-endpoint-in-use")?;
    let mut active: Option<Active> = None;
    let mut shutdown: Option<Shutdown> = None;
    let mut diagnostics = ControlDiagnostics {
        stderr_enabled: std::env::var_os("ORCHESTRIX_LOCAL_CONTROL_DIAGNOSTICS")
            .is_some_and(|v| v == "1"),
        ..ControlDiagnostics::default()
    };
    loop {
        if active.as_ref().is_some_and(|a| a.thread.is_finished()) {
            let completed = active.take().expect("checked active");
            let outcome = completed
                .done
                .recv()
                .unwrap_or_else(|_| Err("worker-thread-lost".into()));
            let joined = completed.thread.join().is_ok();
            let result = match outcome {
                Ok(summary) if joined => {
                    json!({"state":"finished","workerId":summary["workerId"],"status":summary["status"],"artifactRevision":summary["details"]["artifactRevision"],"resultQuery":"status","delivered":false})
                }
                Err(code) if joined => {
                    let reserved = store
                        .daemon_worker_exists(&completed.worker_id)
                        .map_err(|_| "completion-inspection")?;
                    json!({"state":if reserved{"inspection-required"}else{"rejected-before-reservation"},"reason":code,"workerId":completed.worker_id,"reservationPersisted":reserved,"resumeAutomatically":false,"nextAction":if reserved{"inspect-recovery"}else{"inspect-connection-and-submit-a-new-authorized-command"}})
                }
                _ => {
                    json!({"state":"inspection-required","reason":"worker-thread-lost","workerId":completed.worker_id,"resumeAutomatically":false})
                }
            };
            store
                .daemon_finish(&completed.command_id, &result)
                .map_err(|_| "worker-completion")?;
        }
        if let Some(shutdown) = &shutdown
            && active.is_none()
        {
            store
                .daemon_finish(
                    &shutdown.command_id,
                    &json!({"state":"stopped","resumeAutomatically":false}),
                )
                .map_err(|_| "shutdown-completion")?;
            return Ok(());
        }
        let Some(mut stream) = listener
            .accept(Duration::from_millis(100))
            .map_err(|_| "local-accept")?
        else {
            continue;
        };
        ipc::clear_diagnostic();
        let read_started = std::time::Instant::now();
        let read = stream.read(IO_TIMEOUT);
        diagnostics.record(
            "request-read",
            "unknown",
            read_started,
            read.as_ref().err().copied(),
        );
        let mut operation = "invalid";
        let handler_started = std::time::Instant::now();
        let response = match read {
            Ok(bytes) => match serde_json::from_slice::<Request>(&bytes) {
                Ok(request) if request.api_version == API_VERSION => {
                    operation = operation_tag(&request.operation);
                    handle(
                        &mut store,
                        database,
                        request.operation,
                        &mut active,
                        &mut shutdown,
                    )
                }
                Ok(_) => Response::error("api-version"),
                Err(_) => Response::error("request-schema"),
            },
            Err(_) => Response::error("request-deadline-or-budget"),
        };
        diagnostics.record("handler", operation, handler_started, None);
        let bytes = serde_json::to_vec(&response).unwrap_or_default();
        let write_started = std::time::Instant::now();
        let written = if bytes.len() <= ipc::MAX_FRAME {
            stream.write(&bytes, IO_TIMEOUT)
        } else {
            stream.write(
                br#"{"api_version":1,"ok":false,"error":"response-budget","result":null}"#,
                IO_TIMEOUT,
            )
        };
        diagnostics.record("response-write", operation, write_started, written.err());
        let ack_started = std::time::Instant::now();
        let ack = stream.read(IO_TIMEOUT);
        diagnostics.record("response-ack", operation, ack_started, ack.err());
    }
}
fn handle(
    store: &mut Store,
    database: &Path,
    operation: Operation,
    active: &mut Option<Active>,
    shutdown: &mut Option<Shutdown>,
) -> Response {
    match handle_inner(store, database, operation, active, shutdown) {
        Ok(value) => Response::value(value),
        Err(code) => Response::error(&code),
    }
}
fn handle_inner(
    store: &mut Store,
    database: &Path,
    operation: Operation,
    active: &mut Option<Active>,
    shutdown: &mut Option<Shutdown>,
) -> Result<Value, String> {
    match &operation {
        Operation::Snapshot => {
            let mut snapshot = store.daemon_snapshot().map_err(|_| "snapshot")?;
            snapshot["activeWorkerId"] = json!(active.as_ref().map(|a| &a.worker_id));
            snapshot["shuttingDown"] = json!(shutdown.is_some());
            return Ok(snapshot);
        }
        Operation::Workers {
            cursor,
            after_worker,
            limit,
        } => {
            return store
                .daemon_workers(cursor, after_worker.as_deref(), *limit)
                .map_err(|_| "worker-page".into());
        }
        Operation::Query { query } => {
            if shutdown.is_some() {
                return Err("daemon-shutting-down".into());
            }
            if !query.authorized || crate::domain::LocalRef::new(&query.authorization_ref).is_err()
            {
                return Err("query-not-authorized".into());
            }
            match &query.query {
                crate::mechanical::QueryKind::GitStatus{..}|crate::mechanical::QueryKind::GitDiff{..}|crate::mechanical::QueryKind::FileExists{..}=>return Err("filesystem-query-not-supported-by-control-loop".into()),
                crate::mechanical::QueryKind::CheckResults{worker_id}|crate::mechanical::QueryKind::ProcessStatus{worker_id}=>return store.load().map_err(|_|"database")?.workers().get(worker_id).map(|w|json!({"route":"deterministic","result":w.summary(),"historical":true,"currentVerification":"not-revalidated","modelCalled":false})).ok_or_else(||"worker-not-found".into()),
            }
        }
        Operation::Status { worker_id } => {
            return store
                .load()
                .map_err(|_| "database")?
                .workers()
                .get(worker_id)
                .map(
                    |w| json!({"historical":true,"currentVerification":"not-revalidated","worker":w.summary()}),
                )
                .ok_or_else(|| "worker-not-found".into());
        }
        Operation::Events { cursor, limit } => {
            return match store
                .daemon_events(cursor, *limit)
                .map_err(|_| "events-query")?
            {
                Some((events, cursor, more)) => {
                    Ok(json!({"events":events,"cursor":cursor,"more":more,"resyncRequired":false}))
                }
                None => Ok(json!({"events":[],"resyncRequired":true,"action":"request-snapshot"})),
            };
        }
        Operation::Recovery => {
            return serde_json::to_value(
                crate::recovery::snapshot(store).map_err(|_| "recovery-inspection")?,
            )
            .map_err(|_| "recovery-schema".into());
        }
        Operation::Continuity { worker_id, budget } => {
            let pack = crate::recovery::compile_continuity(store, worker_id, *budget)
                .map_err(|_| "continuity-inspection")?;
            return Ok(
                json!({"pack":pack.as_json(),"revision":pack.revision(),"rendered":pack.rendered(),"inferenceUsed":false}),
            );
        }
        _ => {}
    }
    let fingerprint = content_hash(
        serde_json::to_vec(&operation)
            .map_err(|_| "command-schema")?
            .as_slice(),
    );
    let command_id = match &operation {
        Operation::Submit { command_id, .. }
        | Operation::Suspend { command_id }
        | Operation::Resume { command_id }
        | Operation::Cancel { command_id, .. }
        | Operation::Shutdown { command_id }
        | Operation::Intervene { command_id, .. }
        | Operation::RecheckRecovery { command_id, .. } => command_id,
        _ => unreachable!(),
    };
    if let Some(receipt) = store
        .daemon_receipt(command_id, &fingerprint)
        .map_err(|_| "command-id-conflict")?
    {
        return receipt_json(receipt);
    }
    if shutdown.is_some() {
        return Err("daemon-shutting-down".into());
    }
    match operation {
        Operation::Submit {
            command_id,
            authorization_ref,
            task,
            runtime,
        } => {
            if authorization_ref.is_empty()
                || authorization_ref.len() > 128
                || crate::persistence_privacy::sanitize_text(&authorization_ref).is_err()
            {
                return Err("explicit-submission-authorization-required".into());
            }
            if store.daemon_state().map_err(|_| "database")?.suspended {
                return Err("admissions-suspended".into());
            }
            if active.is_some() {
                return Err("serial-worker-active".into());
            }
            if crate::recovery::snapshot(store)
                .map_err(|_| "recovery-inspection")?
                .has_blocking_cases
            {
                return Err("recovery-inspection-required".into());
            }
            if store
                .daemon_worker_exists(&task.id)
                .map_err(|_| "database")?
            {
                return Err("worker-id-already-recorded-inspect-only".into());
            }
            // Explicit synthetic fault seam only; Codex has no injected stall.
            let stall_after_runtime = matches!(&runtime,RuntimeChoice::Synthetic{scenario,..}if scenario=="shutdown-stall");
            let launch = runtime.launch(&task)?;
            let receipt = store
                .daemon_begin(&command_id, &fingerprint, "submit", Some(&task.id), None)
                .map_err(|_| "command-journal")?;
            let cancel = CancellationToken::new();
            let token = cancel.clone();
            let worker_id = task.id.clone();
            let path = database.to_owned();
            let (tx, rx) = mpsc::sync_channel(1);
            let spawned = std::thread::Builder::new()
                .name("orchestrix-serial-worker".into())
                .spawn(move || {
                    let result = validate_database_separation(&path, &task).and_then(|()| {
                        Store::open(&path)
                            .map_err(|_| "database".to_owned())
                            .and_then(|mut worker_store| {
                                let result = if stall_after_runtime {
                                    ManualWorker::run_with_checkpoint(
                                        &mut worker_store,
                                        &task,
                                        launch,
                                        &token,
                                        |checkpoint| {
                                            if checkpoint
                                                == crate::worker::WorkerCheckpoint::RuntimeObserved
                                            {
                                                std::thread::sleep(Duration::from_secs(25));
                                            }
                                        },
                                    )
                                } else {
                                    ManualWorker::run(&mut worker_store, &task, launch, &token)
                                };
                                result
                                    .map(|record| record.summary())
                                    .map_err(|e| e.0.to_owned())
                            })
                    });
                    let _ = tx.send(result);
                });
            let thread = worker_thread_result(store, &command_id, &worker_id, spawned)?;
            *active = Some(Active {
                worker_id,
                command_id,
                cancel,
                thread,
                done: rx,
            });
            receipt_json(receipt)
        }
        Operation::Suspend { command_id } => {
            let r = store
                .daemon_begin(&command_id, &fingerprint, "suspend", None, Some(true))
                .map_err(|_| "command-journal")?;
            store
                .daemon_finish(
                    &command_id,
                    &json!({"admissionsSuspended":true,"activeWorkCancelled":false}),
                )
                .map_err(|_| "command-completion")?;
            receipt_json(r)
        }
        Operation::Resume { command_id } => {
            if active.is_none() {
                let summary = crate::recovery::snapshot(store).map_err(|_| "reconciliation")?;
                if summary.has_blocking_cases {
                    return Err("recovery-inspection-required".into());
                }
            }
            let r = store
                .daemon_begin(&command_id, &fingerprint, "resume", None, Some(false))
                .map_err(|_| "command-journal")?;
            store
                .daemon_finish(
                    &command_id,
                    &json!({"admissionsSuspended":false,"resumedOldWork":false}),
                )
                .map_err(|_| "command-completion")?;
            receipt_json(r)
        }
        Operation::Cancel {
            command_id,
            worker_id,
        } => {
            let a = active
                .as_ref()
                .filter(|a| a.worker_id == worker_id)
                .ok_or("worker-not-active-owned-by-daemon")?;
            let r = store
                .daemon_begin(&command_id, &fingerprint, "cancel", Some(&worker_id), None)
                .map_err(|_| "command-journal")?;
            a.cancel.cancel();
            store.daemon_finish(&command_id,&json!({"cancellationRequested":true,"terminalConfirmed":false,"workerId":worker_id})).map_err(|_|"command-completion")?;
            receipt_json(r)
        }
        Operation::Shutdown { command_id } => {
            let bounded = Shutdown::new(command_id.clone());
            let r = store
                .daemon_begin(&command_id, &fingerprint, "shutdown", None, Some(true))
                .map_err(|_| "command-journal")?;
            if let Some(a) = active {
                a.cancel.cancel();
            }
            *shutdown = Some(bounded);
            receipt_json(r)
        }
        Operation::Intervene {
            command_id,
            worker_id,
            expected_revision,
            authorization_ref,
            action,
        } => {
            if active.is_some() {
                return Err("worker-active-recovery-not-permitted".into());
            }
            let r = store
                .daemon_begin(
                    &command_id,
                    &fingerprint,
                    "intervene",
                    Some(&worker_id),
                    None,
                )
                .map_err(|_| "command-journal")?;
            let result = crate::recovery::intervene(
                store,
                &worker_id,
                expected_revision,
                &authorization_ref,
                action,
            )
            .map_err(|_| "recovery-intervention-rejected");
            match result {
                Ok(case) => {
                    store
                        .daemon_finish(
                            &command_id,
                            &json!({"case":case,"resumeAutomatically":false}),
                        )
                        .map_err(|_| "command-completion")?;
                    receipt_json(r)
                }
                Err(code) => {
                    store
                        .daemon_finish(&command_id, &json!({"rejected":true,"reason":code}))
                        .map_err(|_| "command-completion")?;
                    Err(code.into())
                }
            }
        }
        Operation::RecheckRecovery {
            command_id,
            worker_id,
        } => {
            if active.is_some() {
                return Err("worker-active-recovery-not-permitted".into());
            }
            let receipt = store
                .daemon_begin(
                    &command_id,
                    &fingerprint,
                    "recheck-recovery",
                    Some(&worker_id),
                    None,
                )
                .map_err(|_| "command-journal")?;
            match crate::recovery::refresh_closure_evidence(store, &worker_id) {
                Ok(case) => {
                    store.daemon_finish(&command_id,&json!({"state":"recovery-rechecked","case":case,"resumeAutomatically":false})).map_err(|_|"command-completion")?;
                    receipt_json(receipt)
                }
                Err(error) => {
                    store.daemon_finish(&command_id,&json!({"state":"recovery-recheck-rejected","reason":error.0,"resumeAutomatically":false})).map_err(|_|"command-completion")?;
                    Err(error.0.into())
                }
            }
        }
        _ => unreachable!(),
    }
}
fn receipt_json(receipt: DaemonCommandReceipt) -> Result<Value, String> {
    serde_json::to_value(receipt).map_err(|_| "response-schema".into())
}
fn worker_thread_result(
    store: &mut Store,
    command_id: &str,
    worker_id: &str,
    result: std::io::Result<std::thread::JoinHandle<()>>,
) -> Result<std::thread::JoinHandle<()>, String> {
    match result {
        Ok(thread) => Ok(thread),
        Err(_) => {
            store.daemon_finish(command_id,&json!({"state":"rejected-before-reservation","reason":"worker-thread-spawn-failed","workerId":worker_id,"reservationPersisted":false,"dispatch":"not-sent","resumeAutomatically":false,"nextAction":"inspect-host-resources-before-a-new-authorized-command"})).map_err(|_|"worker-completion")?;
            Err("worker-thread-spawn-failed".into())
        }
    }
}
impl RuntimeChoice {
    fn launch(self, task: &ManualTaskRequest) -> Result<RuntimeLaunch, String> {
        match self {
            Self::Codex {
                executable,
                home,
                version,
                allow_live,
            } => {
                if !allow_live {
                    return Err("live-opt-in-required".into());
                }
                Ok(RuntimeLaunch::codex(
                    executable,
                    home,
                    task.repository_directory.clone(),
                    version,
                ))
            }
            Self::Synthetic {
                executable,
                home,
                scenario,
            } => {
                if ![
                    "success",
                    "hang",
                    "lost-reply",
                    "crash",
                    "check-failed",
                    "quota-failure",
                    "rate-limit",
                    "catalog-seven",
                    "shutdown-stall",
                ]
                .contains(&scenario.as_str())
                {
                    return Err("synthetic-scenario".into());
                }
                let mut launch = RuntimeLaunch::fixture(
                    executable,
                    vec![
                        "--scenario".into(),
                        if scenario == "shutdown-stall" {
                            "hang".into()
                        } else {
                            scenario.into()
                        },
                    ],
                    home,
                    task.repository_directory.clone(),
                );
                launch.turn_timeout = Duration::from_secs(6);
                launch.interrupt_grace = Duration::from_millis(300);
                Ok(launch)
            }
        }
    }
}
fn validate_database_separation(database: &Path, task: &ManualTaskRequest) -> Result<(), String> {
    let parent = database
        .parent()
        .ok_or("database-parent")?
        .canonicalize()
        .map_err(|_| "database-parent")?;
    for root in [
        &task.repository_directory,
        &task.authorized_worktree_directory,
    ] {
        let root = root.canonicalize().map_err(|_| "task-directory")?;
        if parent.starts_with(root) {
            return Err("database-must-be-outside-worker-directories".into());
        }
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn private_control_diagnostics_are_memory_only_by_default_and_bounded() {
        let mut diagnostics = ControlDiagnostics::default();
        assert!(!diagnostics.stderr_enabled);
        for _ in 0..100 {
            diagnostics.record(
                "request-read",
                "unknown",
                std::time::Instant::now(),
                Some(ipc::Error::Closed),
            );
        }
        assert_eq!(diagnostics.recent.len(), 16);
        assert_eq!(diagnostics.errors_emitted, 8);
        assert!(!diagnostics.stderr_enabled);
    }
    #[test]
    fn owner_scope_keeps_database_extensions_distinct_and_shutdown_guard_does_not_leak() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-daemon-owner-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&root).unwrap();
        let first = owner_lease(&root.join("state.sqlite")).unwrap();
        assert!(owner_lease(&root.join("state.sqlite")).is_err());
        let different = owner_lease(&root.join("state.db")).unwrap();
        let before = std::time::Instant::now();
        drop(Shutdown::new("test-deadline-cancel".into()));
        assert!(before.elapsed() < Duration::from_secs(1));
        drop(first);
        drop(different);
        assert_eq!(
            root.parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_worker_thread_creation_completes_receipt_without_reservation_or_retry() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-thread-failure-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&root).unwrap();
        let mut store = Store::open(root.join("state.sqlite")).unwrap();
        store.daemon_start().unwrap();
        let hash = content_hash(b"explicit-synthetic-command");
        store
            .daemon_begin(
                "submit-failed-thread",
                &hash,
                "submit",
                Some("never-reserved"),
                None,
            )
            .unwrap();
        let result = worker_thread_result(
            &mut store,
            "submit-failed-thread",
            "never-reserved",
            Err(std::io::Error::other("injected-thread-creation-failure")),
        );
        assert_eq!(result.unwrap_err(), "worker-thread-spawn-failed");
        let receipt = store
            .daemon_receipt("submit-failed-thread", &hash)
            .unwrap()
            .unwrap();
        assert!(receipt.replayed);
        let outcome = receipt.outcome.unwrap();
        assert_eq!(outcome["state"], "rejected-before-reservation");
        assert_eq!(outcome["reservationPersisted"], false);
        assert_eq!(outcome["dispatch"], "not-sent");
        assert!(store.daemon_pending().unwrap().is_empty());
        assert!(store.load().unwrap().workers().is_empty());
        let old = store
            .daemon_begin(
                "submit-failed-thread",
                &hash,
                "submit",
                Some("never-reserved"),
                None,
            )
            .unwrap();
        assert!(old.replayed);
        assert_eq!(old.outcome, Some(outcome));
        drop(store);
        assert_eq!(
            root.parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn watchdog_thread_creation_failure_terminates_own_host_before_acceptance() {
        const CHILD_FLAG: &str = "ORCHESTRIX_TEST_WATCHDOG_CREATION_FAILURE";
        if std::env::var_os(CHILD_FLAG).is_some() {
            let _guard = Shutdown::with_spawn("never-accepted".into(), |_| {
                Err(std::io::Error::other("injected-thread-creation-failure"))
            });
            std::process::exit(99);
        }
        let child=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","daemon::tests::watchdog_thread_creation_failure_terminates_own_host_before_acceptance","--nocapture"]).env(CHILD_FLAG,"yes").output().unwrap();
        assert_eq!(child.status.code(), Some(3));
    }
}
