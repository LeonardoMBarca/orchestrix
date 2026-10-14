//! External synthetic runtime. Real stdio/processes, no provider/login/inference.
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--check-output-limit") {
        print!("{}", "x".repeat(100 * 1024));
        std::io::stdout().flush().unwrap();
        return;
    }
    if args.first().map(String::as_str) == Some("--check-secret") {
        println!("API_KEY={}{}", "sk-", "syntheticfixturecredential123456");
        return;
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
        println!("codex-cli 0.162.0-alpha.2");
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
        let child = child_command.spawn().unwrap();
        println!("{}", child.id());
        std::io::stdout().flush().unwrap();
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
                if scenario == "metadata-hang" {
                    std::thread::sleep(Duration::from_secs(60));
                }
                send(json!({"id":id,"result":{"userAgent":"orchestrix-rust-fixture/1"}}))
            }
            "account/read" => send(
                json!({"id":id,"result":{"account":if scenario == "auth-failure" {Value::Null} else {json!({"type":if scenario == "api" {"apiKey"} else {"chatgpt"},"accountId":if scenario == "wrong-account" {"different-fixture-identity"} else {"synthetic-identity"},"planType":"pro"})},"requiresOpenaiAuth":true}}),
            ),
            "account/rateLimits/read" => {
                send(json!({"id":id,"result":{"ordinaryUsageAllowed":scenario != "quota-failure"}}))
            }
            "config/read" => send(
                json!({"id":id,"result":{"config":{"model_provider":"openai","chatgpt_base_url":"https://chatgpt.com/backend-api/","openai_base_url":"","web_search":"disabled","windows":{"sandbox":"mxc"},"features":{"hooks":false,"plugins":false,"apps":false,"multi_agent":scenario == "delegation-enabled"},"agents":{"enabled":scenario == "agents-enabled"},"notify":[],"mcp_servers":{}}}}),
            ),
            "model/list" => send(
                json!({"id":id,"result":{"data":[{"id":"fixture-codex","model":"fixture-codex","isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"medium"},{"reasoningEffort":"high"},{"reasoningEffort":"xhigh"}]}],"nextCursor":null}}),
            ),
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
                send(
                    json!({"id":id,"result":{"thread":{"id":thread,"ephemeral":false,"status":{"type":"idle"},"cwd":cwd,"modelProvider":"openai","turns":[]},"model":model,"modelProvider":"openai","cwd":cwd,"reasoningEffort":effort,"approvalPolicy":"never","approvalsReviewer":"user","sandbox":sandbox}}),
                );
            }
            "turn/start" => {
                turn += 1;
                let turn_id = format!("fixture-turn-{turn}");
                let prompt = params["input"][0]["text"].as_str().unwrap_or("");
                std::fs::write(auth_home.join("dispatch-count"), turn.to_string()).unwrap();
                if scenario != "lost-reply" {
                    send(json!({"id":id,"result":{"turn":{"id":turn_id,"status":"inProgress"}}}));
                }
                send(
                    json!({"method":"turn/started","params":{"threadId":thread,"turn":{"id":turn_id,"status":"inProgress"}}}),
                );
                if scenario == "crash" {
                    std::process::exit(23);
                }
                if scenario == "malformed" {
                    println!("{{ malformed");
                    std::io::stdout().flush().unwrap();
                    continue;
                }
                if scenario == "hang" || scenario == "lost-reply" || scenario == "ignore-interrupt"
                {
                    continue;
                }
                if scenario == "success" || scenario == "check-failed" {
                    if prompt.contains("fixture-edit") {
                        let path = cwd.join("fixture.txt");
                        if path.is_file() {
                            std::fs::write(
                                path,
                                if scenario == "success" {
                                    "after\n"
                                } else {
                                    "incorrect\n"
                                },
                            )
                            .unwrap();
                        }
                    }
                }
                send(
                    json!({"method":"item/agentMessage/delta","params":{"threadId":thread,"turnId":turn_id,"delta":"Synthetic runtime finished."}}),
                );
                send(
                    json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":turn_id,"status":"completed","error":null}}}),
                );
            }
            "turn/interrupt" => {
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
}
fn send(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
