//! Test runtime only: an observable, responsive turn with a finite watchdog.
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct ControlledTurn(Arc<Mutex<State>>);
struct State {
    active: bool,
    sequence: u64,
    began: Instant,
    identity: Value,
    log: File,
    release: PathBuf,
    release_token: String,
    invalid_release_recorded: bool,
}
impl State {
    fn record(&mut self, kind: &str, detail: Value) {
        self.sequence += 1;
        // All elapsed times belong to this one runtime process. This is a QA
        // journal, never a Core state transition or provider evidence.
        let row = json!({"schemaVersion":1,"synthetic":true,"sequence":self.sequence,"kind":kind,"elapsedMs":self.began.elapsed().as_millis() as u64,"atUnixMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,"identity":self.identity,"detail":detail});
        serde_json::to_writer(&mut self.log, &row).unwrap();
        self.log.write_all(b"\n").unwrap();
        self.log.sync_data().unwrap();
    }
    fn terminal(&mut self, cause: &str, status: &str) {
        self.record("terminal-sending", json!({"cause":cause,"status":status}));
        super::send(
            json!({"method":"turn/completed","params":{"threadId":self.identity["threadId"],"turn":{"id":self.identity["turnId"],"status":status,"error":null}}}),
        );
        self.record("terminal-sent", json!({"cause":cause,"status":status}));
        self.active = false;
    }
}
impl ControlledTurn {
    pub fn start(home: &Path, thread: &str, turn: &str, qa: &Value, watchdog_ms: u64) -> Self {
        assert!((250..=85_000).contains(&watchdog_ms));
        for field in ["connectionId", "sessionId", "attemptId"] {
            let value = qa[field].as_str().expect("explicit synthetic identity");
            assert!(
                !value.is_empty()
                    && value.len() <= 256
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
            );
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let token = format!("{}-{nonce}", std::process::id());
        let stem = format!("controlled-{}-{nonce}", std::process::id());
        let log = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(home.join(format!("{stem}.jsonl")))
            .unwrap();
        let release = home.join(format!("{stem}.release.json"));
        let identity = json!({"pid":std::process::id(),"threadId":thread,"turnId":turn,"connectionId":qa["connectionId"],"sessionId":qa["sessionId"],"attemptId":qa["attemptId"]});
        let mut state = State {
            active: true,
            sequence: 0,
            began: Instant::now(),
            identity,
            log,
            release,
            release_token: token,
            invalid_release_recorded: false,
        };
        state.record("turn-active", json!({"watchdogMs":watchdog_ms,"releaseFile":format!("{stem}.release.json"),"releaseToken":state.release_token.clone()}));
        let shared = Arc::new(Mutex::new(state));
        let observer = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut next_heartbeat = Duration::from_secs(1);
            loop {
                std::thread::sleep(Duration::from_millis(25));
                let mut state = observer.lock().unwrap();
                if !state.active {
                    break;
                }
                if state.began.elapsed() >= Duration::from_millis(watchdog_ms) {
                    state.record("watchdog-expired", json!({"exitCode":24}));
                    // A safety exit is not a successful remote terminal.
                    std::process::exit(24);
                }
                if let Ok(metadata) = std::fs::symlink_metadata(&state.release) {
                    let release = if metadata.is_file()
                        && !metadata.file_type().is_symlink()
                        && metadata.len() <= 2048
                    {
                        std::fs::read(&state.release)
                            .ok()
                            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    } else {
                        None
                    };
                    let expected = json!({"releaseToken":state.release_token,"threadId":state.identity["threadId"],"turnId":state.identity["turnId"],"sessionId":state.identity["sessionId"],"attemptId":state.identity["attemptId"]});
                    if release.as_ref() == Some(&expected) {
                        state.record("exclusive-release-accepted", json!({}));
                        state.terminal("exclusive-test-release", "completed");
                        break;
                    } else if !state.invalid_release_recorded {
                        state.record(
                            "release-rejected",
                            json!({"reason":"identity-or-token-mismatch"}),
                        );
                        state.invalid_release_recorded = true;
                    }
                }
                if state.began.elapsed() >= next_heartbeat {
                    state.record("active-heartbeat", json!({}));
                    next_heartbeat += Duration::from_secs(1);
                }
            }
        });
        Self(shared)
    }
    pub fn interrupt(&self, request_id: &Value, params: &Value) {
        let mut state = self.0.lock().unwrap();
        if !state.active
            || params["threadId"] != state.identity["threadId"]
            || params["turnId"] != state.identity["turnId"]
        {
            super::send(
                json!({"id":request_id,"error":{"code":-1,"message":"controlled turn identity or state mismatch"}}),
            );
            return;
        }
        state.record("interrupt-received", json!({"requestId":request_id}));
        super::send(json!({"id":request_id,"result":{}}));
        state.record("interrupt-ack-sent", json!({"requestId":request_id}));
        state.terminal("protocol-interrupt", "interrupted");
    }
    pub fn input_closed(&self) {
        let mut state = self.0.lock().unwrap();
        if state.active {
            state.record("input-closed-without-terminal", json!({}));
            state.active = false;
        }
    }
}
