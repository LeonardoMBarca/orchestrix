//! External synthetic runtime. Real stdio/processes, no provider/login/inference.
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;
mod controlled_turn;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--check-output-limit") {
        print!("{}", "x".repeat(100 * 1024));
        std::io::stdout().flush().unwrap();
        return;
    }
    if args.first().map(String::as_str) == Some("--check-secret") {
        println!(concat!(
            "API_KEY=",
            "sk-",
            "syntheticfixturecredential123456"
        ));
        return;
    }
    const OPAQUE_DOC_APPEND: &str = "
## Workspace access and explicit context

A Context Pack selects the content supplied by Orchestrix. It is not a read sandbox: runtime tools may read other materialized workspace files. Preserving a file locally does not authorize adding its content to a prompt or editing it.
";
    if args.first().map(String::as_str) == Some("--check-opaque-document") {
        let text = std::fs::read("docs/CONTEXT_OWNERSHIP.md").unwrap();
        let valid = text
            .strip_suffix(OPAQUE_DOC_APPEND.as_bytes())
            .is_some_and(|base| {
                args.get(1).is_some_and(|expected| {
                    *expected == orchestrix_core::git_records::content_hash(base)
                })
            });
        std::process::exit(if valid { 0 } else { 1 });
    }
    if args.first().map(String::as_str) == Some("--check-fixture") {
        let expected = args.get(1).expect("explicit expected fixture bytes");
        std::process::exit(
            if std::fs::read("fixture.txt").is_ok_and(|bytes| bytes == expected.as_bytes()) {
                0
            } else {
                1
            },
        );
    }
    if args.first().map(String::as_str) == Some("--mutate-fixture-check") {
        std::fs::write("fixture.txt", "changed-by-check\n").unwrap();
        return;
    }
    if args.first().map(String::as_str) == Some("--mutate-restore-fixture-check") {
        let before = std::fs::read("fixture.txt").unwrap();
        std::fs::write("fixture.txt", "changed-by-check\n").unwrap();
        std::fs::write("fixture.txt", before).unwrap();
        return;
    }
    if args.first().map(String::as_str) == Some("--check-hang") {
        if let Some(marker) = args.get(1) {
            std::fs::write(marker, "check-started").unwrap();
        }
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    let scenario = args
        .windows(2)
        .find(|args| args[0] == "--scenario")
        .map(|args| args[1].as_str())
        .unwrap_or("success");
    if args.first().map(String::as_str) == Some("--version") {
        println!(
            "codex-cli {}",
            if scenario == "connection-metadata" {
                "synthetic-rust-1"
            } else {
                "0.162.0-alpha.2"
            }
        );
        return;
    }
    if scenario == "child-hold" {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    if scenario == "owner-crash" {
        use orchestrix_process::{OwnedProcess, ProcessLimits, SpawnSpec};
        let environment = std::env::vars_os()
            .filter(|(key, _)| {
                ["PATH", "SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "TEMP", "TMP"]
                    .iter()
                    .any(|allowed| key.to_string_lossy().eq_ignore_ascii_case(allowed))
            })
            .collect();
        let child = OwnedProcess::spawn(SpawnSpec {
            executable: std::env::current_exe().unwrap(),
            arguments: vec!["--scenario".into(), "child-hold".into()],
            cwd: std::env::current_dir().unwrap(),
            environment,
            limits: ProcessLimits::default(),
        })
        .unwrap();
        assert!(child.tree().unwrap().active_processes > 0);
        // Deliberately bypass Rust destructors: the OS must close the private Job.
        std::process::exit(42);
    }
    if scenario == "blocked-stdin" {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    if scenario == "parent-exit" || scenario == "owned-child" {
        let mut child_command = std::process::Command::new(std::env::current_exe().unwrap());
        child_command.args(["--scenario", "child-hold"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            child_command.creation_flags(0x0800_0000);
        }
        let mut child = child_command.spawn().unwrap();
        println!("{}", child.id());
        std::io::stdout().flush().unwrap();
        // Retain a normal wait path without joining it: parent-exit must end
        // its root immediately while the real descendant remains in the host's
        // Job. Process exit ends this waiter thread, not the owned descendant.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        if scenario == "parent-exit" {
            return;
        }
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    if scenario == "oversized" {
        println!("{}", "x".repeat(200_000));
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    if scenario == "env" {
        println!(
            "{}",
            json!({"openai":std::env::var("OPENAI_API_KEY").is_ok(),"codex":std::env::var("CODEX_HOME").ok(),"route":std::env::var("OPENAI_BASE_URL").is_ok()})
        );
        return;
    }
    let mut thread = String::new();
    let mut cwd = std::env::current_dir().unwrap();
    let mut sandbox = json!({"type":"readOnly","networkAccess":false});
    let mut model = "fixture-codex".to_owned();
    let mut effort = "medium".to_owned();
    let mut turn = 0u64;
    let auth_home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap_or_default());
    let mut controlled = None;
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        let Some(method) = request["method"].as_str() else {
            continue;
        };
        if request.get("id").is_none() {
            continue;
        }
        let id = request["id"].clone();
        let params = &request["params"];
        // Explicit opt-in in an owned synthetic home only. This fixture is not
        // a production adapter/logger and never handles provider credentials.
        // Preserve the exact received JSON bytes for Context Pack provenance.
        if auth_home.join("capture-configuration-requests").is_file()
            && matches!(method, "thread/start" | "thread/resume" | "turn/start")
        {
            assert!(line.len() <= 1_048_576, "bounded configuration capture");
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let mut capture = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(auth_home.join(format!(
                    "configuration-request-{}-{turn}-{nonce}.json",
                    std::process::id()
                )))
                .unwrap();
            capture.write_all(line.as_bytes()).unwrap();
            capture.sync_all().unwrap();
        }
        match method {
            "initialize" => {
                let counter = auth_home.join("metadata-reads");
                let count = std::fs::read_to_string(&counter)
                    .ok()
                    .and_then(|text| text.parse::<u64>().ok())
                    .unwrap_or(0)
                    + 1;
                std::fs::write(counter, count.to_string()).unwrap();
                if scenario == "metadata-pause" {
                    std::thread::sleep(Duration::from_millis(100));
                }
                if scenario == "connection-metadata"
                    && auth_home.join("connection-metadata-hang").exists()
                {
                    std::thread::sleep(Duration::from_secs(60));
                }
                if scenario == "metadata-hang" {
                    std::thread::sleep(Duration::from_secs(60));
                }
                send(json!({"id":id,"result":{"userAgent":"orchestrix-rust-fixture/1"}}))
            }
            "account/read" => send(
                json!({"id":id,"result":{"account":if scenario == "auth-failure" || (scenario=="connection-metadata"&&auth_home.join("connection-auth-invalid").exists()) {Value::Null} else {json!({"type":if scenario == "api" {"apiKey"} else {"chatgpt"},"accountId":if scenario == "connection-metadata" {std::fs::read_to_string(auth_home.join("connection-identity")).unwrap_or_else(|_|"synthetic-identity".into())} else if scenario == "wrong-account" {"different-fixture-identity".into()} else {"synthetic-identity".into()},"planType":"pro"})},"requiresOpenaiAuth":true}}),
            ),
            "account/rateLimits/read" => {
                if auth_home.join("synthetic-capacity-read-failure").exists() {
                    send(
                        json!({"id":id,"error":{"code":-32000,"message":"synthetic read unavailable"}}),
                    );
                } else {
                    // Explicit test-runtime input, never consulted by the production Codex adapter.
                    let response = std::fs::read(auth_home.join("synthetic-capacity-response.json"))
                        .ok().map(|bytes| { assert!(bytes.len() <= 64*1024); serde_json::from_slice::<Value>(&bytes).unwrap() })
                        .unwrap_or_else(||json!({"ordinaryUsageAllowed":scenario != "quota-failure","rateLimits":{}}));
                    send(json!({"id":id,"result":response}));
                }
            }
            "config/read" => send(
                json!({"id":id,"result":{"config":{"model_provider":"openai","chatgpt_base_url":"https://chatgpt.com/backend-api/","openai_base_url":"","web_search":"disabled","windows":{"sandbox":"mxc"},"features":{"hooks":false,"plugins":false,"apps":false,"multi_agent":scenario == "delegation-enabled"},"agents":{"enabled":scenario == "agents-enabled"},"notify":[],"mcp_servers":{}}}}),
            ),
            "model/list" => {
                let first = json!({"id":"fixture-codex","model":"fixture-codex","isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"medium"},{"reasoningEffort":"high"},{"reasoningEffort":"xhigh"}]});
                let mut data = vec![first.clone()];
                if scenario == "catalog-seven" {
                    for index in 2..=7 {
                        let mut row = first.clone();
                        row["id"] = json!(format!("fixture-codex-{index}"));
                        row["model"] = row["id"].clone();
                        row["isDefault"] = json!(false);
                        data.push(row);
                    }
                }
                if scenario == "connection-metadata"
                    && auth_home.join("connection-partial").exists()
                {
                    if params.get("cursor").is_some() {
                        data.clear();
                    }
                    send(json!({"id":id,"result":{"data":data,"nextCursor":"partial-fixture"}}));
                } else {
                    send(json!({"id":id,"result":{"data":data,"nextCursor":null}}));
                }
            }
            "thread/start" | "thread/resume" => {
                cwd = params["cwd"].as_str().map(PathBuf::from).unwrap_or(cwd);
                thread = if method == "thread/resume" {
                    params["threadId"].as_str().unwrap().into()
                } else {
                    format!("fixture-thread-{}", std::process::id())
                };
                if let Some(value) = params["model"].as_str() {
                    model = value.into();
                }
                if let Some(value) = params["config"]["model_reasoning_effort"].as_str() {
                    effort = value.into();
                }
                sandbox = if params["sandbox"] == "workspace-write" {
                    json!({"type":"workspaceWrite","networkAccess":false,"writableRoots":[],"excludeTmpdirEnvVar":true,"excludeSlashTmp":true})
                } else {
                    json!({"type":"readOnly","networkAccess":false})
                };
                if scenario == "wrong-sandbox" {
                    sandbox["networkAccess"] = json!(true);
                }
                let binding_path = auth_home.join(&thread);
                if method == "thread/resume" {
                    if !binding_path.is_file() {
                        send(
                            json!({"id":id,"error":{"code":-1,"message":"unknown fixture thread"}}),
                        );
                        continue;
                    }
                } else {
                    std::fs::write(binding_path, b"synthetic thread state").unwrap();
                }
                // Real app-server emits this before the RPC reply. It carries
                // a nested Thread rather than a top-level threadId.
                send(
                    json!({"method":"thread/started","params":{"thread":{"id":if scenario == "contradictory-thread-start" {"foreign-thread"} else {&thread},"cwd":cwd,"modelProvider":"openai"}}}),
                );
                send(
                    json!({"id":id,"result":{"thread":{"id":thread,"ephemeral":false,"status":{"type":"idle"},"cwd":cwd,"modelProvider":"openai","turns":[]},"model":model,"modelProvider":"openai","cwd":cwd,"reasoningEffort":effort,"approvalPolicy":"never","approvalsReviewer":"user","sandbox":sandbox}}),
                );
            }
            "turn/start" => {
                turn += 1;
                let turn_id = format!("fixture-turn-{turn}");
                // Per-process dispatch-count remains unchanged for existing
                // scenarios. This separate ledger records received requests
                // durably before any ACK, including lost-reply/crash paths.
                let pid = std::process::id();
                let epoch_nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let mut event = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(auth_home.join(format!("dispatch-event-{pid}-{turn}-{epoch_nanos}.json")))
                    .unwrap();
                serde_json::to_writer(
                    &mut event,
                    &json!({"pid":pid,"turn":turn,"threadId":thread,"turnId":turn_id}),
                )
                .unwrap();
                event.sync_all().unwrap();
                drop(event);
                let prompt = params["input"][0]["text"].as_str().unwrap_or("");
                std::fs::write(auth_home.join("dispatch-count"), turn.to_string()).unwrap();
                if scenario != "lost-reply" {
                    send(json!({"id":id,"result":{"turn":{"id":turn_id,"status":"inProgress"}}}));
                }
                if scenario == "unscoped-private" {
                    send(
                        json!({"method":"private.method.fixture-value","params":{"output":"do-not-persist-provider-message"}}),
                    );
                }
                if scenario.starts_with("remote-status-") {
                    let mut params = json!({"status":"disabled","serverName":"do-not-persist-server-name","installationId":"do-not-persist-installation-id","environmentId":"do-not-persist-environment-id"});
                    match scenario {
                        "remote-status-global" => {}
                        "remote-status-thread" => {
                            params["threadId"] = json!("private-foreign-thread")
                        }
                        "remote-status-thread-null" => params["threadId"] = Value::Null,
                        "remote-status-turn-null" => params["turnId"] = Value::Null,
                        "remote-status-nested-thread" => {
                            params["thread"] = json!({"id":"private-foreign-thread"})
                        }
                        "remote-status-nested-turn" => {
                            params["turn"] = json!({"id":"private-foreign-turn"})
                        }
                        "remote-status-nonobject" => {
                            params = json!(["do-not-persist-provider-message"])
                        }
                        _ => panic!("unknown remote status fixture"),
                    }
                    send(json!({"method":"remoteControl/status/changed","params":params}));
                }
                if scenario.starts_with("warning-") {
                    let mut params = json!({"message":"do-not-persist-provider-message"});
                    match scenario {
                        "warning-null" => params["threadId"] = Value::Null,
                        "warning-absent" => {}
                        "warning-thread" => params["threadId"] = json!(thread),
                        "warning-foreign" => params["threadId"] = json!("private-foreign-thread"),
                        "warning-turn" => {
                            params["threadId"] = Value::Null;
                            params["turnId"] = json!("private-foreign-turn");
                        }
                        "warning-nested-thread" => {
                            params["threadId"] = Value::Null;
                            params["thread"] = json!({"id":"private-foreign-thread"});
                        }
                        _ => panic!("unknown warning fixture"),
                    }
                    send(json!({"method":"warning","params":params}));
                }
                if scenario.starts_with("mcp-metadata-") {
                    let mut params = json!({"name":"do-not-persist-server-name","status":"ready","error":"do-not-persist-provider-message","failureReason":null});
                    match scenario {
                        "mcp-metadata-null" => params["threadId"] = Value::Null,
                        "mcp-metadata-thread" => params["threadId"] = json!(thread),
                        "mcp-metadata-foreign" => {
                            params["threadId"] = json!("private-foreign-thread")
                        }
                        "mcp-metadata-turn" => {
                            params["threadId"] = Value::Null;
                            params["turnId"] = Value::Null;
                        }
                        "mcp-metadata-absent" => {}
                        "mcp-metadata-nonobject" => {
                            params = json!(["do-not-persist-provider-message"])
                        }
                        _ => panic!("unknown MCP metadata fixture"),
                    }
                    send(json!({"method":"mcpServer/startupStatus/updated","params":params}));
                }
                send(
                    json!({"method":"turn/started","params":{"threadId":thread,"turn":{"id":turn_id,"status":"inProgress"}}}),
                );
                // Give the integrated configuration test an observable active
                // attempt while another client saves a future definition. The
                // finite watchdog cannot silently complete or fabricate a turn.
                if auth_home.join("capture-configuration-requests").is_file() {
                    let release = auth_home.join("hold-configuration-turn");
                    let began = std::time::Instant::now();
                    while release.is_file() {
                        assert!(
                            began.elapsed() < Duration::from_secs(10),
                            "configuration fixture hold watchdog"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                if scenario == "qa-controlled" {
                    let watchdog = args
                        .windows(2)
                        .find(|pair| pair[0] == "--control-watchdog-ms")
                        .map(|pair| pair[1].parse::<u64>().unwrap())
                        .unwrap_or(85_000);
                    controlled = Some(controlled_turn::ControlledTurn::start(
                        &auth_home,
                        &thread,
                        &turn_id,
                        &params["_orchestrixQa"],
                        watchdog,
                    ));
                    continue;
                }
                if scenario == "crash" {
                    std::process::exit(23);
                }
                if scenario == "malformed" {
                    println!("{{ malformed");
                    std::io::stdout().flush().unwrap();
                    continue;
                }
                if scenario == "review-hang" {
                    std::fs::write(auth_home.join("review-started"), b"started").unwrap();
                    continue;
                }
                if scenario == "hang" || scenario == "lost-reply" || scenario == "ignore-interrupt"
                {
                    continue;
                }
                if scenario == "global-notifications" {
                    send(
                        json!({"method":"account/updated","params":{"authMode":"chatgpt","planType":"pro"}}),
                    );
                    send(
                        json!({"method":"account/rateLimits/updated","params":{"rateLimits":{"primary":null}}}),
                    );
                    send(
                        json!({"method":"configWarning","params":{"summary":"Synthetic harmless metadata update"}}),
                    );
                }
                if scenario == "changed-global-auth" {
                    send(
                        json!({"method":"account/updated","params":{"authMode":"apikey","planType":null}}),
                    );
                }
                if scenario == "rate-limit" {
                    send(
                        json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":turn_id,"status":"failed","error":{"codexErrorInfo":"usageLimitExceeded","message":"Synthetic quota boundary"}}}}),
                    );
                    continue;
                }
                if scenario == "contradictory-delta" {
                    send(
                        json!({"method":"item/agentMessage/delta","params":{"threadId":thread,"turnId":turn_id,"turn":{"id":"foreign-turn"},"delta":"Contradictory execution identity"}}),
                    );
                }
                if scenario == "contradictory-thread" {
                    send(
                        json!({"method":"item/agentMessage/delta","params":{"threadId":"foreign-thread","turnId":turn_id,"delta":"Contradictory execution scope"}}),
                    );
                }
                if matches!(
                    scenario,
                    "success"
                        | "check-failed"
                        | "catalog-seven"
                        | "remote-status-global"
                        | "review-implementation"
                ) && prompt.contains("fixture-edit")
                {
                    let path = cwd.join("fixture.txt");
                    if path.is_file() {
                        std::fs::write(
                            path,
                            if scenario == "check-failed" {
                                "incorrect\n"
                            } else {
                                "after\n"
                            },
                        )
                        .unwrap();
                    }
                }
                if scenario == "opaque-document" && prompt.contains("opaque-document:") {
                    let path = cwd.join("docs/CONTEXT_OWNERSHIP.md");
                    let mut bytes = std::fs::read(&path).unwrap();
                    assert!(!bytes.ends_with(OPAQUE_DOC_APPEND.as_bytes()));
                    bytes.extend_from_slice(OPAQUE_DOC_APPEND.as_bytes());
                    std::fs::write(path, bytes).unwrap();
                }
                if scenario == "review-implementation" {
                    std::fs::write(
                        cwd.join("implementation.txt"),
                        "preserve prior implementation\n",
                    )
                    .unwrap();
                }
                if scenario == "review-correction" {
                    assert!(prompt.contains("finding-one"));
                    assert!(prompt.contains("Correct the exact rejected artifact"));
                    assert_eq!(
                        std::fs::read_to_string(cwd.join("fixture.txt")).unwrap(),
                        "after\n"
                    );
                    assert_eq!(
                        std::fs::read_to_string(cwd.join("implementation.txt")).unwrap(),
                        "preserve prior implementation\n"
                    );
                    std::fs::write(cwd.join("correction.txt"), "finding-one addressed\n").unwrap();
                }
                if params.get("outputSchema").is_some() && scenario.starts_with("review-") {
                    assert_eq!(sandbox["type"], "readOnly");
                    std::fs::write(auth_home.join("review-sandbox"), sandbox.to_string()).unwrap();
                    std::fs::write(
                        auth_home.join("review-schema"),
                        params["outputSchema"].to_string(),
                    )
                    .unwrap();
                    let marker = "REVIEW_ARTIFACT_REVISION=";
                    let revision = prompt
                        .split(marker)
                        .nth(1)
                        .unwrap()
                        .chars()
                        .take(64)
                        .collect::<String>();
                    let rejected = scenario == "review-reject";
                    let result = if scenario == "review-invalid" {
                        "unstructured approval is not evidence".to_owned()
                    } else {
                        json!({"schema_version":1,"artifact_revision":revision,"verdict":if rejected{"rejected"}else{"accepted"},"summary":"Synthetic independent review result","findings":if rejected{json!([{"id":"finding-one","severity":"medium","path":"fixture.txt","artifact_revision":revision,"line_start":1,"line_end":1,"explanation":"Exercise a correction of the exact artifact","evidence":"fixture.txt line 1 contains after"}])}else{json!([])}}).to_string()
                    };
                    if scenario == "review-mutate" {
                        std::fs::write(cwd.join("fixture.txt"), "unauthorized write\n").unwrap();
                    }
                    send(
                        json!({"method":"item/agentMessage/delta","params":{"threadId":thread,"turnId":turn_id,"itemId":"final-result","delta":result}}),
                    );
                    send(
                        json!({"method":"item/completed","params":{"threadId":thread,"turnId":turn_id,"item":{"type":"agentMessage","id":"final-result","phase":"final_answer","text":result}}}),
                    );
                } else {
                    send(
                        json!({"method":"item/agentMessage/delta","params":{"threadId":thread,"turnId":turn_id,"delta":"Synthetic runtime finished."}}),
                    );
                }
                send(
                    json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":turn_id,"status":"completed","error":null}}}),
                );
            }
            "turn/interrupt" => {
                if let Some(controlled) = &controlled {
                    controlled.interrupt(&id, params);
                    continue;
                }
                if scenario == "ignore-interrupt" {
                    continue;
                }
                send(json!({"id":id,"result":{}}));
                send(
                    json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":params["turnId"],"status":"interrupted","error":null}}}),
                );
            }
            _ => send(
                json!({"id":id,"error":{"code":-32601,"message":"unsupported fixture operation"}}),
            ),
        }
    }
    if let Some(controlled) = controlled {
        controlled.input_closed();
    }
}
fn send(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
