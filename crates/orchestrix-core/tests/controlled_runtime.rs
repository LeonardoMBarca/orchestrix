//! Explicit QA runtime only; native adapter/Job/streams are real. No inference.
#![cfg(windows)]
use orchestrix_core::{
    runtime::{AttemptExecution, CancellationToken, CodexAdapter, RuntimeLaunch},
    runtime_contract::{RuntimeObserver, normalize_request},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchestrix-controlled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let home = root.join("home");
        std::fs::create_dir(&home).unwrap();
        Self { root, home }
    }
    fn connect(&self, watchdog: u64) -> CodexAdapter {
        let mut launch = RuntimeLaunch::fixture(
            PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
            vec![
                "--scenario".into(),
                "qa-controlled".into(),
                "--control-watchdog-ms".into(),
                watchdog.to_string().into(),
            ],
            self.home.clone(),
            self.root.clone(),
        );
        launch.turn_timeout = Duration::from_secs(10);
        launch.request_timeout = Duration::from_secs(2);
        launch.interrupt_grace = Duration::from_secs(1);
        CodexAdapter::connect(launch, "connection", 1, 100, &CancellationToken::new()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn execution() -> AttemptExecution {
    AttemptExecution {
        request: normalize_request(&json!({"version":{"major":1,"minor":1},"connectionId":"connection","connectionRevision":1,"sessionId":"session","attemptId":"controlled-attempt","turnId":null,"billingMode":"subscription-only","requested":{"modelPolicy":"runtime-default","reasoning":"runtime-default","sandbox":"workspace-write","network":"disabled","approval":"never"}})).unwrap(),
        prompt:"Explicit bounded QA hold".into(),base_instructions:"No file edits".into(),developer_instructions:"No delegation".into(),resume:None,
    }
}
fn journal(home: &Path) -> Vec<Value> {
    std::fs::read_dir(home)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "jsonl"))
        .flat_map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .collect()
}
fn wait(home: &Path, kind: &str) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let rows = journal(home);
        if rows.iter().any(|r| r["kind"] == kind) {
            return rows;
        }
        assert!(Instant::now() < deadline, "missing QA event {kind}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn release(active: &Value) -> Value {
    json!({"releaseToken":active["detail"]["releaseToken"],"threadId":active["identity"]["threadId"],"turnId":active["identity"]["turnId"],"sessionId":active["identity"]["sessionId"],"attemptId":active["identity"]["attemptId"]})
}

#[test]
fn controlled_turn_holds_with_identity_and_rejects_wrong_release_before_exact_release() {
    let f = Fixture::new();
    let mut adapter = f.connect(8000);
    let home = f.home.clone();
    let driver = std::thread::spawn(move || {
        let rows = wait(&home, "turn-active");
        let active = &rows[0];
        assert_eq!(active["identity"]["connectionId"], "connection");
        assert_eq!(active["identity"]["sessionId"], "session");
        assert_eq!(active["identity"]["attemptId"], "controlled-attempt");
        let path = home.join(active["detail"]["releaseFile"].as_str().unwrap());
        std::fs::write(&path, b"{}").unwrap();
        wait(&home, "release-rejected");
        let rows = wait(&home, "active-heartbeat");
        assert!(!rows.iter().any(|r| r["kind"] == "terminal-sent"));
        // Separate test-only release: never writes any domain/SQLite approval.
        std::fs::write(path, serde_json::to_vec(&release(active)).unwrap()).unwrap();
    });
    let report = adapter
        .execute(execution(), &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    driver.join().unwrap();
    let s = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(s["terminal"]["outcome"], "completed");
    assert_eq!(report.tree.unwrap().active_processes, 0);
    assert!(report.streams.stdout_closed);
    let rows = journal(&f.home);
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "exclusive-release-accepted")
    );
    assert!(!rows.iter().any(|r| r["kind"] == "interrupt-received"));
    assert!(
        rows.windows(2)
            .all(|r| r[1]["sequence"].as_u64() > r[0]["sequence"].as_u64())
    );
}
#[test]
fn controlled_turn_cancel_is_protocol_ack_terminal_and_owned_tree_closure() {
    let f = Fixture::new();
    let mut adapter = f.connect(8000);
    let token = CancellationToken::new();
    let trigger = token.clone();
    let home = f.home.clone();
    let driver = std::thread::spawn(move || {
        wait(&home, "active-heartbeat");
        trigger.cancel();
    });
    let report = adapter.execute(execution(), &token, |_, _| Ok(())).unwrap();
    driver.join().unwrap();
    let s = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_eq!(s["terminal"]["outcome"], "interrupted");
    assert_eq!(s["interrupt"]["acknowledged"], true);
    assert_eq!(report.tree.unwrap().active_processes, 0);
    assert!(
        report.streams.stdin_closed && report.streams.stdout_closed && report.streams.stderr_closed
    );
    let rows = journal(&f.home);
    let kinds: Vec<_> = rows.iter().filter_map(|r| r["kind"].as_str()).collect();
    let interrupt = kinds
        .iter()
        .position(|k| *k == "interrupt-received")
        .unwrap();
    let ack = kinds
        .iter()
        .position(|k| *k == "interrupt-ack-sent")
        .unwrap();
    let terminal = kinds.iter().position(|k| *k == "terminal-sent").unwrap();
    assert!(interrupt < ack && ack < terminal);
    assert!(!kinds.contains(&"watchdog-expired"));
    assert!(!kinds.contains(&"exclusive-release-accepted"));
}
#[test]
fn controlled_watchdog_is_failure_not_fabricated_success_or_interruption() {
    let f = Fixture::new();
    let mut adapter = f.connect(1500);
    let report = adapter
        .execute(execution(), &CancellationToken::new(), |_, _| Ok(()))
        .unwrap();
    let s = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    assert_ne!(s["outcome"], "completed");
    assert_ne!(s["terminal"]["outcome"], "completed");
    assert_ne!(s["terminal"]["outcome"], "interrupted");
    assert_eq!(report.tree.unwrap().active_processes, 0);
    let rows = journal(&f.home);
    assert!(rows.iter().any(|r| r["kind"] == "watchdog-expired"));
    assert!(!rows.iter().any(|r| r["kind"] == "terminal-sent"));
}
