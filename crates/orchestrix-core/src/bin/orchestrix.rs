//! Minimal manual-worker entry point. There is no autonomous planner or implicit project script execution.
use orchestrix_core::{
    persistence::Store,
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ManualTaskRequest, ManualWorker},
};
use std::{collections::BTreeMap, env, io::Read, path::PathBuf};

fn main() {
    if let Err(code) = run() {
        eprintln!("Orchestrix: {code}");
        std::process::exit(2);
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let action = args.next().unwrap_or_else(|| "help".into());
    if ["help", "--help", "-h"].contains(&action.as_str()) {
        println!(
            "Local daemon API v1 (Windows, user-restricted):\ndaemon --database ABSOLUTE_PATH\napi --database ABSOLUTE_PATH --request VERSIONED_REQUEST.json\nsnapshot --database ABSOLUTE_PATH\nsuspend|resume|shutdown --database ABSOLUTE_PATH --command-id UNIQUE_ID\ncancel --database ABSOLUTE_PATH --command-id UNIQUE_ID --id WORKER_ID\n\nDaemon starts with admissions suspended; resume is explicit. API clients never open SQLite. Disconnect does not cancel work. Reuse the same command ID to inspect a lost response; never change its content."
        );
        println!(
            "Orchestrix manual worker\n\nrun --database ABSOLUTE_PATH --task MANIFEST.json --codex ABSOLUTE_EXE --codex-home ABSOLUTE_DIR --runtime-version PINNED --allow-live yes\nrun --database ABSOLUTE_PATH --task MANIFEST.json --synthetic-runtime FIXTURE_EXE --codex-home FIXTURE_HOME [--scenario success]\nstatus --database ABSOLUTE_PATH --id WORKER_ID\nquery --query QUERY.json [--database ABSOLUTE_PATH]\ndiscover --codex ABSOLUTE_EXE --codex-home ABSOLUTE_DIR --runtime-version PINNED --directory ABSOLUTE_DIR --authorized-read yes\n\nOptional run: --cancel-after-ms 1..90000. Existing IDs are inspected, never automatically executed again.\nCommands must be individually authorized in the task manifest; no scripts are discovered or run implicitly."
        );
        return Ok(());
    }
    let mut flags = BTreeMap::new();
    while let Some(flag) = args.next() {
        if !flag.starts_with("--") || flags.contains_key(&flag) {
            return Err("invalid-or-duplicate-option".into());
        }
        let value = args.next().ok_or("option-value-missing")?;
        flags.insert(flag, value);
    }
    let allowed = &[
        "--database",
        "--task",
        "--codex",
        "--codex-home",
        "--runtime-version",
        "--allow-live",
        "--synthetic-runtime",
        "--scenario",
        "--id",
        "--cancel-after-ms",
        "--query",
        "--authorized-read",
        "--directory",
        "--request",
        "--command-id",
        "--wait-git-probe",
    ];
    if flags.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("unknown-option".into());
    }
    if flags.contains_key("--wait-git-probe")
        && (action != "api"
            || flags.get("--wait-git-probe").map(String::as_str) != Some("yes")
            || env::var_os("ORCHESTRIX_GIT_EXECUTOR_PROBE_ROOT").is_none())
    {
        return Err("private-git-probe-wait-opt-in".into());
    }
    let required = |key: &str| {
        flags
            .get(key)
            .cloned()
            .ok_or_else(|| format!("missing-{key}"))
    };
    if action == "daemon" {
        return orchestrix_core::daemon::serve(&PathBuf::from(required("--database")?));
    }
    if ["api", "snapshot", "suspend", "resume", "cancel", "shutdown"].contains(&action.as_str()) {
        use orchestrix_core::daemon::{API_VERSION, Client, Operation, Request};
        let request = if action == "api" {
            read_json::<Request>(&PathBuf::from(required("--request")?))?
        } else {
            let operation = match action.as_str() {
                "snapshot" => Operation::Snapshot,
                "suspend" => Operation::Suspend {
                    command_id: required("--command-id")?,
                },
                "resume" => Operation::Resume {
                    command_id: required("--command-id")?,
                },
                "cancel" => Operation::Cancel {
                    command_id: required("--command-id")?,
                    worker_id: required("--id")?,
                },
                "shutdown" => Operation::Shutdown {
                    command_id: required("--command-id")?,
                },
                _ => unreachable!(),
            };
            Request {
                api_version: API_VERSION,
                operation,
            }
        };
        let wait_id = flags
            .contains_key("--wait-git-probe")
            .then(|| probe_wait_id(&request))
            .transpose()?;
        let client = Client::new(&PathBuf::from(required("--database")?))?;
        let mut response = client.request(&request)?;
        if let Some(id) = wait_id {
            let started = std::time::Instant::now();
            while response.ok && response.result["state"] == "pending" {
                if started.elapsed() >= std::time::Duration::from_secs(110) {
                    return Err("git-probe-wait-elapsed-inspect-original-id".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
                // Same request identity; never repeat Start after a lost reply.
                response = client.request(&Request {
                    api_version: API_VERSION,
                    operation: Operation::RepositoryInspection {
                        request:
                            orchestrix_core::repository_inspection::InspectionRequest::Inspect {
                                id: id.clone(),
                            },
                    },
                })?;
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&response).map_err(|_| "serialization")?
        );
        return if response.ok {
            Ok(())
        } else {
            Err("daemon-rejected-command".into())
        };
    }
    if action == "discover" {
        if required("--authorized-read")? != "yes" || flags.contains_key("--allow-live") {
            return Err("read-only-opt-in-required".into());
        }
        let launch = RuntimeLaunch::codex(
            PathBuf::from(required("--codex")?),
            PathBuf::from(required("--codex-home")?),
            PathBuf::from(required("--directory")?),
            required("--runtime-version")?,
        );
        let service = orchestrix_core::runtime::CodexMetadataService::new(
            std::sync::Arc::new(orchestrix_core::runtime::SystemRefreshClock),
            orchestrix_core::runtime::refresh::RefreshBounds::default(),
        )
        .ok_or("refresh-bounds")?;
        let result = service
            .read(launch, "manual-read", 1, true, &CancellationToken::new())
            .map_err(|e| e.to_string())?;
        let orchestrix_core::runtime::MetadataRead::Ready {
            metadata,
            cached,
            complete,
        } = result
        else {
            return Err("metadata-pending".into());
        };
        println!(
            "{}",
            serde_json::json!({"descriptor":metadata.descriptor.as_json(),"controls":metadata.controls,"observedAtMs":metadata.observed_at_ms,"cached":cached,"complete":complete,"synthetic":metadata.synthetic,"inference":false})
        );
        return Ok(());
    }
    if action == "query" {
        let query =
            read_json::<orchestrix_core::mechanical::Query>(&PathBuf::from(required("--query")?))?;
        let _owner = if let Some(path) = flags.get("--database") {
            match orchestrix_core::daemon::owner_lease(&PathBuf::from(path)) {
                Ok(lease) => Some(lease),
                Err(_) => {
                    let response = orchestrix_core::daemon::Client::new(&PathBuf::from(path))?
                        .request(&orchestrix_core::daemon::Request {
                            api_version: orchestrix_core::daemon::API_VERSION,
                            operation: orchestrix_core::daemon::Operation::Query { query },
                        })?;
                    if !response.ok {
                        return Err("daemon-query-rejected".into());
                    }
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&response.result)
                            .map_err(|_| "serialization")?
                    );
                    return Ok(());
                }
            }
        } else {
            None
        };
        let mut store = flags
            .get("--database")
            .map(|p| Store::open(PathBuf::from(p)))
            .transpose()
            .map_err(|_| "database")?;
        let result = orchestrix_core::mechanical::execute(store.as_mut(), &query)
            .map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|_| "serialization")?
        );
        return Ok(());
    }
    let database = PathBuf::from(required("--database")?);
    if action == "status" {
        let _owner = match orchestrix_core::daemon::owner_lease(&database) {
            Ok(lease) => lease,
            Err(_) => {
                let response = orchestrix_core::daemon::Client::new(&database)?.request(
                    &orchestrix_core::daemon::Request {
                        api_version: orchestrix_core::daemon::API_VERSION,
                        operation: orchestrix_core::daemon::Operation::Status {
                            worker_id: required("--id")?,
                        },
                    },
                )?;
                if !response.ok {
                    return Err("daemon-status-rejected".into());
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&response.result).map_err(|_| "serialization")?
                );
                return Ok(());
            }
        };
        if !database.is_file() {
            return Err("database-not-found".into());
        }
        let mut store = Store::open(&database).map_err(|_| "database")?;
        let record =
            ManualWorker::inspect(&mut store, &required("--id")?).map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&record.summary()).map_err(|_| "serialization")?
        );
        return Ok(());
    }
    if action != "run" {
        return Err("unknown-command".into());
    }
    let task = read_json::<ManualTaskRequest>(&PathBuf::from(required("--task")?))?;
    // Legacy standalone execution is explicitly exclusive with an operational daemon.
    // The daemon and this old entry point cannot each own a scheduler for the database.
    let _owner = orchestrix_core::daemon::owner_lease(&database)?;
    // Keep SQLite and its WAL/SHM outside directories exposed to the worker/checks.
    let parent = database
        .parent()
        .ok_or("database-path")?
        .canonicalize()
        .map_err(|_| "database-parent")?;
    for directory in [
        &task.repository_directory,
        &task.authorized_worktree_directory,
    ] {
        let root = directory.canonicalize().map_err(|_| "task-directory")?;
        if parent.starts_with(root) {
            return Err("database-must-be-outside-worker-directories".into());
        }
    }
    let mut store = Store::open(&database).map_err(|_| "database")?;
    // A restart only inspects history, so no launch information or new allowance is needed.
    if store
        .load()
        .map_err(|_| "database")?
        .workers()
        .contains_key(&task.id)
    {
        let invalid =
            RuntimeLaunch::fixture(PathBuf::new(), vec![], PathBuf::new(), PathBuf::new());
        let record = ManualWorker::run(&mut store, &task, invalid, &CancellationToken::new())
            .map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&record.summary()).map_err(|_| "serialization")?
        );
        return Ok(());
    }
    let launch = if let Some(executable) = flags.get("--synthetic-runtime") {
        if flags.contains_key("--codex") || flags.contains_key("--allow-live") {
            return Err("ambiguous-runtime".into());
        }
        let home = PathBuf::from(required("--codex-home")?);
        RuntimeLaunch::fixture(
            PathBuf::from(executable),
            vec![
                "--scenario".into(),
                flags
                    .get("--scenario")
                    .cloned()
                    .unwrap_or_else(|| "success".into())
                    .into(),
            ],
            home,
            task.repository_directory.clone(),
        )
    } else {
        if required("--allow-live")? != "yes" {
            return Err("live-opt-in-required".into());
        }
        RuntimeLaunch::codex(
            PathBuf::from(required("--codex")?),
            PathBuf::from(required("--codex-home")?),
            task.repository_directory.clone(),
            required("--runtime-version")?,
        )
    };
    let cancel = CancellationToken::new();
    if let Some(value) = flags.get("--cancel-after-ms") {
        let milliseconds: u64 = value.parse().map_err(|_| "cancel-deadline")?;
        if milliseconds == 0 || milliseconds > 90_000 {
            return Err("cancel-deadline".into());
        }
        let token = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(milliseconds));
            token.cancel();
        });
    }
    let record =
        ManualWorker::run(&mut store, &task, launch, &cancel).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&record.summary()).map_err(|_| "serialization")?
    );
    Ok(())
}
fn probe_wait_id(request: &orchestrix_core::daemon::Request) -> Result<String, String> {
    use orchestrix_core::{
        daemon::{API_VERSION, Operation},
        mechanical::QueryKind,
        repository_inspection::InspectionRequest,
    };
    if request.api_version != API_VERSION {
        return Err("private-git-probe-wait-request".into());
    }
    match &request.operation {
        Operation::RepositoryInspection {
            request: InspectionRequest::Start { id, query },
        } if query.authorized && matches!(query.query, QueryKind::GitExecutorProbe { .. }) => {
            Ok(id.clone())
        }
        _ => Err("private-git-probe-wait-request".into()),
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|_| "manifest-read")?
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| "manifest-read")?;
    if bytes.len() > 1_048_576 {
        return Err("manifest-size".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "manifest-schema".into())
}

#[cfg(test)]
mod probe_wait_tests {
    use super::*;
    use orchestrix_core::{
        daemon::{API_VERSION, Operation, Request},
        mechanical::{Query, QueryKind},
        repository_inspection::InspectionRequest,
    };
    #[test]
    fn wait_accepts_only_one_fixed_probe_start_and_retains_its_id() {
        let request = Request {
            api_version: API_VERSION,
            operation: Operation::RepositoryInspection {
                request: InspectionRequest::Start {
                    id: "fixed-b1".into(),
                    query: Query {
                        authorization_ref: "read".into(),
                        authorized: true,
                        query: QueryKind::GitExecutorProbe {
                            repository: PathBuf::from("owned"),
                        },
                    },
                },
            },
        };
        assert_eq!(probe_wait_id(&request).unwrap(), "fixed-b1");
        assert!(
            probe_wait_id(&Request {
                api_version: API_VERSION,
                operation: Operation::Snapshot
            })
            .is_err()
        );
        assert!(
            probe_wait_id(&Request {
                api_version: API_VERSION,
                operation: Operation::RepositoryInspection {
                    request: InspectionRequest::Inspect {
                        id: "fixed-b1".into()
                    }
                }
            })
            .is_err()
        );
    }
}
