//! Offline integrated definitions proof. The daemon, SQLite, Git and owned
//! processes are real; the runtime is explicitly synthetic and opt-in captures
//! only requests sent to that fixture. No provider/login/inference is involved.
#![cfg(windows)]
use orchestrix_core::{
    configuration::ConfigurationRequest,
    daemon::{API_VERSION, Client, Operation, Request, Response, RuntimeChoice},
    desktop::{DesktopRequest, Settings},
    domain::{AttemptState, Run},
    evaluator::CheckSpec,
    persistence::Store,
    policy::{PolicyDocument, TaskClassification, TaskKind},
    review_records::{ReviewBudget, ReviewRequest, ReviewStatus},
    worker::{ContextFileRequest, ManualTaskRequest},
};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    attempts: PathBuf,
    home: PathBuf,
    database: PathBuf,
    base: String,
}
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-local-configuration-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        let repository = root.join("repository");
        let attempts = root.join("attempts");
        let home = root.join("synthetic-home");
        for directory in [&repository, &attempts, &home] {
            fs::create_dir(directory).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), "before\n").unwrap();
        git(&repository, &["add", "--all"]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "Local configuration fixture"],
        );
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        fs::write(
            home.join("capture-configuration-requests"),
            b"owned synthetic request evidence",
        )
        .unwrap();
        Self {
            database: root.join("state.sqlite"),
            root,
            repository,
            attempts,
            home,
            base,
        }
    }
    fn start(&self) -> Daemon {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new(env!("CARGO_BIN_EXE_orchestrix"))
            .args(["daemon", "--database"])
            .arg(&self.database)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.root.join("daemon.stderr.log"))
                    .unwrap(),
            )
            .spawn()
            .unwrap();
        let client = Client::new(&self.database).unwrap();
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            if client
                .request(&Request {
                    api_version: API_VERSION,
                    operation: Operation::Snapshot,
                })
                .is_ok()
            {
                break;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "owned daemon exited before readiness"
            );
            assert!(Instant::now() < deadline, "owned daemon readiness deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
        Daemon { child, client }
    }
    fn runtime(&self, scenario: &str) -> RuntimeChoice {
        RuntimeChoice::Synthetic {
            executable: runtime_binary(),
            home: self.home.clone(),
            scenario: scenario.into(),
        }
    }
    fn task(&self, id: &str, policy: PolicyDocument) -> ManualTaskRequest {
        ManualTaskRequest {
            editable_paths: None,
            schema_version: 1,
            id: id.into(),
            project_id: "configuration-project".into(),
            repository_id: "repo-configuration-project".into(),
            repository_directory: self.repository.clone(),
            authorized_worktree_directory: self.attempts.clone(),
            base_commit: self.base.clone(),
            connection_id: "fixture-account".into(),
            connection_revision: 1,
            objective: "fixture-edit: change only fixture.txt in the owned worktree".into(),
            acceptance_criteria: vec!["fixture.txt contains after followed by newline".into()],
            policy,
            classification: TaskClassification::routine(
                TaskKind::Implementation,
                "configuration-user",
            ),
            context_files: vec![ContextFileRequest {
                path: "fixture.txt".into(),
                authorization_ref: "read-fixture".into(),
                required: true,
            }],
            instructions: vec!["Only the explicitly authorized fixture is task input.".into()],
            checks: vec![CheckSpec {
                id: "configuration-check".into(),
                executable: runtime_binary(),
                arguments: vec!["--check-fixture".into(), "after\n".into()],
                authorization_ref: "explicit-synthetic-check".into(),
                authorized: true,
                timeout_ms: 2000,
                executable_sha256: None,
            }],
        }
    }
    fn captures(&self) -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(&self.home)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("configuration-request-")
            })
            .map(|entry| {
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }
    fn intent_bytes(&self, id: &str) -> Vec<u8> {
        // Read the retained authoritative bytes; never manufacture success with
        // SQL writes or substitute reconstructed JSON for the stored evidence.
        let database = rusqlite::Connection::open_with_flags(
            &self.database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        database
            .query_row(
                "SELECT intent FROM manual_workers WHERE id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
            .into_bytes()
    }
    fn running(&self, id: &str) -> bool {
        // Store::open initializes/verifies schema and may need a writer. The
        // active-turn observer is strictly read-only and restores the domain
        // checkpoint rather than competing with the daemon's writer.
        let database = rusqlite::Connection::open_with_flags(
            &self.database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let checkpoint: Option<String> = database.query_row(
            "SELECT r.checkpoint FROM runs r JOIN manual_workers w ON w.run_id=r.id WHERE w.id=?1",
            [id], |row| row.get(0),
        ).optional().unwrap();
        checkpoint.is_some_and(|body| {
            let run = Run::restore_checkpoint(&serde_json::from_str(&body).unwrap()).unwrap();
            run.tasks().values().any(|task| {
                task.attempts()
                    .iter()
                    .any(|attempt| attempt.state() == AttemptState::Running)
            })
        })
    }
    fn adapter_binding(&self, id: &str) -> Value {
        let database = rusqlite::Connection::open_with_flags(
            &self.database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let session: String = database.query_row(
            "SELECT s.id FROM sessions s JOIN attempts a ON a.session_id=s.id JOIN manual_workers w ON w.attempt_id=a.id WHERE w.id=?1",
            [id], |row|row.get(0),
        ).unwrap();
        let mut query = database
            .prepare(
                "SELECT body FROM worker_observations WHERE worker_id=?1 ORDER BY ordinal DESC",
            )
            .unwrap();
        let bodies: Vec<String> = query
            .query_map([id], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let binding = bodies
            .iter()
            .map(|body| serde_json::from_str::<Value>(body).unwrap())
            .find_map(|row| {
                row["details"]
                    .get("adapterBinding")
                    .filter(|binding| !binding.is_null())
                    .cloned()
            })
            .expect("observed adapter binding");
        assert_eq!(binding["sessionId"], session);
        assert_eq!(binding["connectionId"], "fixture-account");
        assert_eq!(binding["connectionRevision"], 1);
        binding
    }
}
struct Daemon {
    child: Child,
    client: Client,
}
impl Daemon {
    fn send(&self, operation: Operation) -> Response {
        self.client
            .request(&Request {
                api_version: API_VERSION,
                operation,
            })
            .unwrap()
    }
    fn ok(&self, operation: Operation) -> Value {
        let response = self.send(operation);
        assert!(response.ok, "{:?}", response.error);
        response.result
    }
    fn desktop(&self, request: DesktopRequest) -> Value {
        self.ok(Operation::Desktop {
            request: Box::new(request),
        })
    }
    fn configurations(&self, request: Value) -> Value {
        self.desktop(DesktopRequest::Configurations {
            request: Box::new(serde_json::from_value::<ConfigurationRequest>(request).unwrap()),
        })
    }
    fn catalog(&self) -> Value {
        self.configurations(json!({"type":"catalog"}))
    }
    fn revision(&self) -> u64 {
        self.desktop(DesktopRequest::Bootstrap)["revision"]
            .as_u64()
            .unwrap()
    }
    fn save_definition(&self, kind: &str, command: &str, definition: Value) {
        self.configurations(json!({"type":format!("save-{kind}"),"commandId":command,"expectedRevision":self.revision(),"definition":definition}));
    }
    fn saved(&self, kind: &str, id: &str) -> Value {
        self.catalog()[format!("{kind}s")]
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["id"] == id)
            .unwrap()
            .clone()
    }
    fn inspect(&self, id: &str) -> Value {
        self.desktop(DesktopRequest::InspectWorker {
            worker_id: id.into(),
        })
    }
    fn prepare(&self, policy: &PolicyDocument) -> Value {
        self.desktop(DesktopRequest::ResolveConfiguration {
            policy: Box::new(policy.clone()),
            project_id: Some("configuration-project".into()),
        })
    }
    fn select(&self, mode: Value, profile: Value, command: &str) -> PolicyDocument {
        let bootstrap = self.desktop(DesktopRequest::Bootstrap);
        let mut settings: Settings = serde_json::from_value(bootstrap["settings"].clone()).unwrap();
        settings.policy.mode = serde_json::from_value(mode).unwrap();
        settings.policy.profile = serde_json::from_value(profile).unwrap();
        settings.policy.mode_defaults.fallback =
            Some(orchestrix_core::policy::Fallback::Compatible);
        settings.policy.revision += 1;
        let policy = settings.policy.clone();
        self.desktop(DesktopRequest::SaveSettings {
            command_id: command.into(),
            expected_revision: bootstrap["revision"].as_u64().unwrap(),
            settings: Box::new(settings),
        });
        policy
    }
    fn wait_worker(&self, id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let snapshot = self.ok(Operation::Snapshot);
            if snapshot["activeWorkerId"].is_null()
                && snapshot["workers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|worker| worker["workerId"] == id)
            {
                return self.inspect(id);
            }
            assert!(Instant::now() < deadline, "worker deadline {snapshot}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    fn wait_review(&self, worker: &str, id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let snapshot = self.ok(Operation::Snapshot);
            if snapshot["activeWorkerId"].is_null() {
                let detail = self.inspect(worker);
                if let Some(review) = detail["reviews"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|review| review["id"] == id)
                {
                    return review.clone();
                }
            }
            assert!(Instant::now() < deadline, "review deadline {snapshot}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    fn stop(mut self) {
        self.ok(Operation::Shutdown {
            command_id: format!("stop-{}", self.child.id()),
        });
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "owned shutdown deadline");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn runtime_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture"))
}
fn git(cwd: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Orchestrix fixture",
            "-c",
            "user.email=fixture@orchestrix.invalid",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn local(mut source: Value, id: &str, name: &str, marker: &str) -> Value {
    source["base"] = json!({"id":source["id"],"version":source["version"]});
    source["id"] = json!(id);
    source["name"] = json!(name);
    source["version"] = json!(0);
    source["editRevision"] = json!(0);
    source["origin"] = json!("local");
    source["instructions"] = json!(marker);
    source
}
fn selected_turn(captures: &BTreeMap<String, Vec<u8>>, marker: &str) -> Value {
    let matches: Vec<_> = captures
        .values()
        .filter_map(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        .filter(|request| {
            request["method"] == "turn/start"
                && request["params"]["input"][0]["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(marker))
        })
        .collect();
    assert_eq!(matches.len(), 1, "one exact synthetic request for {marker}");
    matches.into_iter().next().unwrap()
}

#[test]
fn local_versions_reach_owned_runtime_review_and_future_tasks_without_rewriting_history() {
    let fixture = Fixture::new();
    let daemon = fixture.start();
    daemon.desktop(DesktopRequest::CreateProject {
        command_id: "create-project".into(),
        expected_revision: daemon.revision(),
        id: "configuration-project".into(),
        name: "Configuration proof".into(),
        directory: fixture.repository.to_string_lossy().into_owned(),
    });
    daemon.desktop(DesktopRequest::SendMessage {
        command_id: "first-message".into(),
        expected_revision: daemon.revision(),
        conversation_id: "configuration-chat".into(),
        project_id: Some("configuration-project".into()),
        content: "Prepare the explicitly authorized fixture task".into(),
    });
    let catalog = daemon.catalog();
    let mut implementation = local(
        catalog["profiles"][0].clone(),
        "local-implementation",
        "Local implementation",
        "OX016_IMPLEMENTATION_VERSION_ONE",
    );
    assert_eq!(implementation["role"], "implementation");
    implementation["preferences"]["strategy"] = json!("balanced");
    implementation["preferences"]["context_bytes"] = json!(24576);
    implementation["outputLanguage"] = json!("pt-BR");
    implementation["conventions"] = json!(["Keep observable verification evidence"]);
    daemon.save_definition("profile", "save-implementation-v1", implementation);
    let reviewer_native = catalog["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|profile| profile["role"] == "review")
        .unwrap()
        .clone();
    let reviewer = local(
        reviewer_native,
        "local-independent-review",
        "Local independent reviewer",
        "OX016_REVIEWER_READ_ONLY",
    );
    daemon.save_definition("profile", "save-reviewer-v1", reviewer);
    let mode_native = catalog["modes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|mode| mode["id"] == "development")
        .unwrap()
        .clone();
    let mut mode = local(
        mode_native,
        "local-development",
        "Local development",
        "OX016_MODE_VERSION_ONE",
    );
    mode["roles"] = json!([{"role":"implementation","profile":{"id":"local-implementation","version":1}},{"role":"review","profile":{"id":"local-independent-review","version":1}}]);
    daemon.save_definition("mode", "save-mode-v1", mode);
    for (scope, project, strategy, command) in [
        ("user", None, "efficiency", "save-user"),
        (
            "project",
            Some("configuration-project"),
            "performance",
            "save-project",
        ),
    ] {
        daemon.configurations(json!({"type":"save-scope","commandId":command,"expectedRevision":daemon.revision(),"scope":{"scope":scope,"projectId":project,"version":0,"editRevision":0,"removed":false,"preferences":{"strategy":strategy},"requirements":{}}}));
    }
    let policy_one = daemon.select(
        json!({"id":"local-development","version":1}),
        json!({"id":"local-implementation","version":1}),
        "select-v1",
    );
    let prepared_one = daemon.prepare(&policy_one);
    assert_eq!(
        prepared_one["decision"]["preferences"]["strategy"],
        "balanced"
    );
    assert_eq!(
        prepared_one["decision"]["origins"]["strategy"],
        "profile-defaults@profile"
    );
    assert_eq!(prepared_one["definitions"]["mode"]["version"], 1);
    let old_fingerprint = prepared_one["fingerprint"].as_str().unwrap().to_owned();
    let task_one = fixture.task("configured-first", policy_one.clone());
    let submit_one = Operation::SubmitConversation {
        command_id: "submit-first".into(),
        authorization_ref: "explicit-owned-fixture".into(),
        conversation_id: "configuration-chat".into(),
        settings_fingerprint: old_fingerprint.clone(),
        task: Box::new(task_one.clone()),
        runtime: fixture.runtime("success"),
    };
    fs::write(
        fixture.home.join("hold-configuration-turn"),
        b"hold only this owned synthetic turn",
    )
    .unwrap();
    daemon.ok(Operation::Resume {
        command_id: "resume-owned-work".into(),
    });
    daemon.ok(submit_one.clone());
    // This is the existing bounded daemon/worker envelope, not the fixture's
    // ten-second active-turn hold watchdog. Git/metadata preparation precedes
    // turn/start and must not consume that separate hold interval.
    let active_deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if fixture.running("configured-first") {
            break;
        }
        let snapshot = daemon.ok(Operation::Snapshot);
        assert!(
            !snapshot["activeWorkerId"].is_null(),
            "worker finished before confirming runtime activity: {snapshot}"
        );
        assert!(
            Instant::now() < active_deadline,
            "runtime must confirm active turn before editing definitions"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let first_bytes = fixture.intent_bytes("configured-first");
    let first_intent: Value = serde_json::from_slice(&first_bytes).unwrap();
    let delivered_v1 = selected_turn(&fixture.captures(), "OX016_IMPLEMENTATION_VERSION_ONE");
    let delivered_context: Value =
        serde_json::from_str(delivered_v1["params"]["input"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(delivered_context, first_intent["context"]);
    assert_eq!(
        first_intent["policy"]["preferences"]["strategy"],
        "balanced"
    );
    assert_eq!(
        first_intent["policy"]["preferences"]["context_bytes"],
        24576
    );
    let captured_v1 = fixture.captures();
    let mut implementation_v2 = daemon.saved("profile", "local-implementation");
    implementation_v2["instructions"] = json!("OX016_IMPLEMENTATION_VERSION_TWO");
    daemon.save_definition("profile", "save-implementation-v2", implementation_v2);
    let mut mode_v2 = daemon.saved("mode", "local-development");
    mode_v2["instructions"] = json!("OX016_MODE_VERSION_TWO");
    mode_v2["roles"][0]["profile"]["version"] = json!(2);
    daemon.save_definition("mode", "save-mode-v2", mode_v2);
    let policy_two = daemon.select(
        json!({"id":"local-development","version":2}),
        json!({"id":"local-implementation","version":2}),
        "select-v2",
    );
    assert_eq!(
        fixture.intent_bytes("configured-first"),
        first_bytes,
        "active immutable intent must remain byte-identical"
    );
    fs::remove_file(fixture.home.join("hold-configuration-turn")).unwrap();
    let first = daemon.wait_worker("configured-first");
    assert_eq!(first["summary"]["status"], "verified", "{first}");
    assert_eq!(first["summary"]["details"]["checks"][0]["exit_code"], 0);
    assert_eq!(
        delivered_v1["params"]["threadId"],
        fixture.adapter_binding("configured-first")["nativeThread"]
    );
    assert_eq!(fixture.intent_bytes("configured-first"), first_bytes);
    let stale = daemon.send(Operation::SubmitConversation {
        command_id: "stale-prepared-submit".into(),
        authorization_ref: "explicit-owned-fixture".into(),
        conversation_id: "configuration-chat".into(),
        settings_fingerprint: old_fingerprint,
        task: Box::new(fixture.task("must-not-dispatch", policy_one.clone())),
        runtime: fixture.runtime("success"),
    });
    assert_eq!(
        stale.error.as_deref(),
        Some("prepared-policy-changed-prepare-again")
    );
    assert!(
        Store::open(&fixture.database)
            .unwrap()
            .load()
            .unwrap()
            .workers()
            .get("must-not-dispatch")
            .is_none()
    );
    daemon.ok(Operation::RequestReview {
        command_id: "review-first".into(),
        request: Box::new(ReviewRequest {
            schema_version: 1,
            id: "configured-review".into(),
            worker_id: "configured-first".into(),
            artifact_revision: first["artifact"]["revision"].as_str().unwrap().into(),
            authorization_ref: "explicit-independent-review".into(),
            policy: policy_one,
            classification: TaskClassification::routine(
                TaskKind::Implementation,
                "configuration-review",
            ),
            budget: ReviewBudget {
                max_corrections: 0,
                max_reviews: 1,
                max_elapsed_ms: 300_000,
            },
        }),
        runtime: fixture.runtime("review-accept"),
    });
    let reviewed = daemon.wait_review("configured-first", "configured-review");
    assert_eq!(reviewed["status"], "accepted", "{reviewed}");
    let review_state = Store::open(&fixture.database).unwrap().load().unwrap();
    let review = &review_state.reviews()["configured-review"];
    assert_eq!(review.status(), ReviewStatus::Accepted);
    assert_eq!(
        review.intent.policy["policy"]["profile"]["id"],
        "local-independent-review"
    );
    assert_eq!(review.intent.policy["requirements"]["read_only"], true);
    let delivered_review = selected_turn(&fixture.captures(), "OX016_REVIEWER_READ_ONLY");
    assert_eq!(
        delivered_review["params"]["sandboxPolicy"]["type"],
        "readOnly"
    );
    assert_ne!(
        delivered_review["params"]["threadId"],
        delivered_v1["params"]["threadId"]
    );
    let review_context: Value = serde_json::from_str(
        delivered_review["params"]["input"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(review_context, review.intent.context);
    let review_bytes = serde_json::to_vec(&review.intent).unwrap();
    drop(review_state);
    let prepared_two = daemon.prepare(&policy_two);
    assert_ne!(prepared_one["fingerprint"], prepared_two["fingerprint"]);
    daemon.ok(Operation::SubmitConversation {
        command_id: "submit-second".into(),
        authorization_ref: "explicit-owned-fixture".into(),
        conversation_id: "configuration-chat".into(),
        settings_fingerprint: prepared_two["fingerprint"].as_str().unwrap().into(),
        task: Box::new(fixture.task("configured-second", policy_two)),
        runtime: fixture.runtime("success"),
    });
    let second = daemon.wait_worker("configured-second");
    assert_eq!(second["summary"]["status"], "verified", "{second}");
    let delivered_v2 = selected_turn(&fixture.captures(), "OX016_IMPLEMENTATION_VERSION_TWO");
    assert!(
        !delivered_v2["params"]["input"][0]["text"]
            .as_str()
            .unwrap()
            .contains("OX016_IMPLEMENTATION_VERSION_ONE")
    );
    assert_eq!(
        second["worker"]["intent"]["policy"]["policy"]["mode"]["version"],
        2
    );
    assert_eq!(
        second["worker"]["intent"]["policy"]["policy"]["profile"]["version"],
        2
    );
    for (path, bytes) in &captured_v1 {
        assert_eq!(fixture.captures().get(path), Some(bytes));
    }
    let ledger_before_reopen = fixture.captures();
    daemon.stop();
    let reopened = fixture.start();
    assert_eq!(
        reopened.inspect("configured-first")["worker"]["intent"],
        first_intent
    );
    assert_eq!(fixture.intent_bytes("configured-first"), first_bytes);
    let restored = Store::open(&fixture.database).unwrap().load().unwrap();
    assert_eq!(
        serde_json::to_vec(&restored.reviews()["configured-review"].intent).unwrap(),
        review_bytes
    );
    assert_eq!(
        fixture.captures(),
        ledger_before_reopen,
        "reopening cannot redispatch"
    );
    assert_eq!(
        git(&fixture.repository, &["rev-parse", "HEAD"]),
        format!("{}\n", fixture.base).into_bytes()
    );
    assert_eq!(
        fs::read(fixture.repository.join("fixture.txt")).unwrap(),
        b"before\n"
    );
    fs::write(fixture.root.join("local-configuration-evidence.json"),serde_json::to_vec_pretty(&json!({"synthetic":true,"first":first,"review":reviewed,"second":second,"firstRequest":delivered_v1,"reviewRequest":delivered_review,"secondRequest":delivered_v2,"retainedIntentSha256":orchestrix_core::git_records::content_hash(&first_bytes),"captures":ledger_before_reopen.keys().collect::<Vec<_>>(),"noRedispatchOnReopen":true,"sourceRepositoryUnchanged":true})).unwrap()).unwrap();
    reopened.stop();
    println!(
        "local configuration integrated evidence: {}",
        fixture.root.display()
    );
}
