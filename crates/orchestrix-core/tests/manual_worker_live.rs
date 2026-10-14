//! Explicit, ignored three-turn provider proof. Never run as part of offline acceptance.
//! Leaves the isolated fixture/report for inspection, including after failure.
#![cfg(windows)]
use orchestrix_core::{
    domain::{LocalRef, RunId, TaskId, TaskState},
    evaluator::{CheckSpec, EvaluationResult, evaluate},
    git_records::content_hash,
    persistence::Store,
    policy::{
        AuthorizedContextFile, ContextBudget, ContextInput, ContextSource, ContextSourceIdentity,
        ContextSourceKind, Fallback, PolicyDocument, TaskClassification, TaskKind, compile_context,
    },
    runtime::{
        AttemptExecution, CancellationToken, CodexAdapter, RefreshClock, RuntimeError,
        RuntimeLaunch, RuntimeReport, SUPPORTED_CODEX_VERSIONS, SessionBinding, SystemRefreshClock,
    },
    runtime_contract::{RuntimeObserver, RuntimeRequest, normalize_request},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker},
    worker_records::WorkerStatus,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const BROKEN: &str = "function add(a, b) { return a - b; }\nmodule.exports = { add: add };\n";
const CORRECT: &str = "function add(a, b) { return a + b; }\nmodule.exports = { add: add };\n";
// Explicit host-owned evaluator, never discovered from or writable by the task.
// ES3-compatible arithmetic fixture needs no additional toolchain installation.
const CHECK: &str = "var input = new ActiveXObject('Scripting.FileSystemObject').OpenTextFile('sum.cjs', 1, false);\nvar source = input.ReadAll(); input.Close();\nvar module = { exports: {} }; eval(source);\nvar add = module.exports.add;\nif (typeof add !== 'function' || add(2, 3) !== 5 || add(-3, 2) !== -1 || add(0, 0) !== 0) { WScript.StdOut.WriteLine('Arithmetic assertion failed'); WScript.Quit(1); }\nWScript.StdOut.WriteLine('Three arithmetic cases passed');\n";
fn marker_instruction(marker: &str) -> String {
    format!(
        "For a later context-continuity check, remember the checkpoint marker is {marker}. Do not write this marker to any file or print it now."
    )
}

struct Journal {
    file: fs::File,
    value: Value,
    reserved: u8,
}
impl Journal {
    fn new(path: &Path, authorization: &str, version: &str) -> Self {
        assert!(path.is_absolute(), "explicit absolute report path required");
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("new report required; never overwrite or retry an earlier proof");
        let mut journal = Self {
            file,
            value: json!({"schemaVersion":1,"synthetic":false,
            "authorizationSha256":content_hash(authorization.as_bytes()),"runtimeVersion":version,
            "platform":"windows","billing":"subscription-only","paidFallback":"forbidden",
            "limits":{"maxTurns":3,"turnSeconds":90,"jobSeconds":300,"serial":true,"automaticRetries":0},
            "selection":{"modelPolicy":"runtime-default","reasoning":"runtime-default","fallback":"compatible"},
            "reservedTurns":0,"accepted":false,"steps":[],"events":[]}),
            reserved: 0,
        };
        journal.flush().expect("write initial bounded report");
        journal
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.value)?;
        if bytes.len() > 65_536 {
            return Err(std::io::Error::other("bounded live report exceeded"));
        }
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file.write_all(&bytes)?;
        self.file.sync_all()
    }
    fn stage(&mut self, stage: &str, facts: Value) {
        let steps = self.value["steps"].as_array_mut().unwrap();
        assert!(steps.len() < 16);
        steps.push(json!({"stage":stage,"facts":facts}));
        self.flush().expect("write bounded live report");
    }
    fn reserve(&mut self, stage: &str) {
        assert!(self.reserved < 3, "live allowance exhausted; no retry");
        self.reserved += 1;
        self.value["reservedTurns"] = json!(self.reserved);
        self.stage(stage, json!({"dispatch":"not-yet-confirmed"}));
    }
    fn event(&mut self, stage: &str, event: &Value) -> Result<(), RuntimeError> {
        let events = self.value["events"].as_array_mut().unwrap();
        if events.len() >= 256 {
            return Err(RuntimeError::observation_failed());
        }
        // No output text, native thread/account IDs, paths, configuration or nonce.
        events.push(json!({"stage":stage,"type":event["type"],"sequence":event["sequence"]}));
        self.flush().map_err(|_| RuntimeError::observation_failed())
    }
}
struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    authorized: PathBuf,
    resume: PathBuf,
    database: PathBuf,
    base: String,
}
impl Fixture {
    fn create(root: PathBuf) -> Self {
        assert!(
            root.is_absolute() && !root.exists(),
            "new explicit fixture directory required"
        );
        let temporary = fs::canonicalize(env::temp_dir()).unwrap();
        assert_eq!(
            fs::canonicalize(root.parent().unwrap()).unwrap(),
            temporary,
            "fixture must be a direct child of the system temporary directory"
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-live-")
        );
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let repository = root.join("source");
        let authorized = root.join("attempts");
        let resume = root.join("resume");
        for directory in [&repository, &authorized, &resume] {
            fs::create_dir(directory).unwrap();
        }
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("sum.cjs"), BROKEN).unwrap();
        git(&repository, &["add", "sum.cjs"]);
        git(
            &repository,
            &[
                "commit",
                "--quiet",
                "-m",
                "Isolated live arithmetic fixture",
            ],
        );
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        Self {
            database: root.join("state.sqlite"),
            root,
            repository,
            authorized,
            resume,
            base,
        }
    }
}
fn explicit_path(name: &str) -> PathBuf {
    let path = PathBuf::from(env::var_os(name).expect("explicit live path required"));
    assert!(path.is_absolute(), "absolute live path required");
    path
}
fn git(cwd: &Path, arguments: &[&str]) -> Vec<u8> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("git");
    command.creation_flags(0x0800_0000);
    command.env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "SystemDrive",
        "WINDIR",
        "TEMP",
        "TMP",
        "COMSPEC",
        "PATHEXT",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "user.name=Orchestrix Fixture",
            "-c",
            "user.email=fixture@orchestrix.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(arguments)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated fixture Git operation failed"
    );
    output.stdout
}
fn resumed_request(binding: &SessionBinding, requested: &Value, attempt: &str) -> RuntimeRequest {
    let checkpoint = binding.checkpoint();
    normalize_request(&json!({"version":{"major":1,"minor":1},"connectionId":checkpoint["connectionId"],
        "connectionRevision":checkpoint["connectionRevision"],"sessionId":checkpoint["sessionId"],
        "attemptId":attempt,"turnId":null,"contextPackRef":attempt,"billingMode":"subscription-only",
        "paidFallback":"forbidden","requested":requested})).unwrap()
}
fn summary(report: &RuntimeReport) -> Value {
    let snapshot = RuntimeObserver::restore_checkpoint(&report.observer_checkpoint)
        .unwrap()
        .snapshot();
    json!({"synthetic":report.synthetic,"dispatch":report.dispatch,
        "fault":report.fault.as_ref().map(RuntimeError::code),"adapterDiagnostic":report.adapter_diagnostic,"phase":snapshot["phase"],
        "startObserved":snapshot["startObserved"],"terminalConfirmed":snapshot["terminalConfirmed"],
        "terminal":snapshot["terminal"],"interrupt":snapshot["interrupt"],
        "tree":report.tree.map(|tree|json!({"owned":tree.owned,"rootExitCode":tree.root_exit_code,"activeProcesses":tree.active_processes})),
        "streams":{"stdin":report.streams.stdin_closed,"stdout":report.streams.stdout_closed,"stderr":report.streams.stderr_closed}})
}
fn stopped(report: &RuntimeReport) -> bool {
    report
        .tree
        .is_some_and(|tree| tree.owned && tree.active_processes == 0)
        && report.streams.stdin_closed
        && report.streams.stdout_closed
        && report.streams.stderr_closed
}

#[test]
fn live_arithmetic_check_fixture_is_deterministic_without_provider_or_inference() {
    let marker = "OX-context-offline-control-123456789";
    let instruction = marker_instruction(marker);
    let input = ContextInput {
        schema_version: 1,
        id: "live-context-control".into(),
        version: 1,
        objective: "Correct add(a,b) in sum.cjs so it adds numbers".into(),
        acceptance_criteria: vec!["The three arithmetic cases pass".into()],
        constraints: vec![
            "Edit only sum.cjs; preserve ES3-compatible function syntax and CommonJS exports"
                .into(),
        ],
        sources: vec![ContextSource {
            identity: ContextSourceIdentity {
                reference: "marker-instruction".into(),
                revision: content_hash(instruction.as_bytes()),
            },
            kind: ContextSourceKind::UserInstructions,
            authorization_ref: Some("explicit-live-context-control".into()),
            text: instruction,
            required: true,
            priority: 0,
        }],
        files: vec![AuthorizedContextFile {
            path: "sum.cjs".into(),
            source: ContextSourceIdentity {
                reference: "arithmetic-source".into(),
                revision: content_hash(BROKEN.as_bytes()),
            },
            authorization_ref: Some("explicit-read-fixture".into()),
            content: BROKEN.into(),
            required: true,
            priority: 0,
        }],
    };
    let restored: ContextInput =
        serde_json::from_value(serde_json::to_value(input).unwrap()).unwrap();
    let pack = compile_context(
        &restored,
        ContextBudget {
            max_bytes: 32_768,
            ..ContextBudget::default()
        },
    )
    .unwrap();
    assert!(pack.rendered().contains(marker));
    assert!(pack.as_json()["omissions"].as_array().unwrap().is_empty());
    let root = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
        "orchestrix-live-check-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let script = root.join("arithmetic-check.js");
    fs::write(&script, CHECK).unwrap();
    let executable = PathBuf::from(env::var_os("SystemRoot").expect("Windows SystemRoot"))
        .join("System32")
        .join("cscript.exe");
    let check = CheckSpec {
        id: "arithmetic-control".into(),
        executable: executable.clone(),
        arguments: vec![
            "//nologo".into(),
            "//B".into(),
            "//E:JScript".into(),
            script.to_str().unwrap().into(),
        ],
        authorization_ref: "explicit-offline-arithmetic-control".into(),
        authorized: true,
        timeout_ms: 2_000,
        executable_sha256: Some(content_hash(&fs::read(executable).unwrap())),
    };
    for (name, source, expected, code, marker) in [
        (
            "broken",
            BROKEN,
            EvaluationResult::Failed,
            1,
            "Arithmetic assertion failed",
        ),
        (
            "correct",
            CORRECT,
            EvaluationResult::Passed,
            0,
            "Three arithmetic cases passed",
        ),
    ] {
        let cwd = root.join(name);
        fs::create_dir(&cwd).unwrap();
        fs::write(cwd.join("sum.cjs"), source).unwrap();
        let result = evaluate(
            &check,
            &cwd,
            &content_hash(source.as_bytes()),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(result.result, expected);
        assert_eq!(result.exit_code, Some(code));
        assert!(
            result.stdout.contains(marker),
            "arithmetic result, not a WSH/syntax failure, is required"
        );
        assert!(result.tree_confirmed && result.streams_closed);
        assert_eq!(fs::read_to_string(cwd.join("sum.cjs")).unwrap(), source);
    }
    assert_eq!(fs::read_to_string(script).unwrap(), CHECK);
    // Two tiny controls are retained for inspection; no external source tree is changed.
}

#[test]
#[ignore = "Consumes up to three subscription turns; requires a fresh explicit authorization and all live environment gates"]
fn codex_worker_resume_context_and_cancellation_live_opt_in() {
    assert_eq!(
        env::var("ORCHESTRIX_ALLOW_LIVE_WORKER").as_deref(),
        Ok("three-turns-subscription-only")
    );
    assert_eq!(env::var("ORCHESTRIX_LIVE_MAX_TURNS").as_deref(), Ok("3"));
    let authorization = env::var("ORCHESTRIX_LIVE_AUTHORIZATION_ID")
        .expect("fresh authorization reference required");
    LocalRef::new(&authorization).expect("local authorization reference");
    let version =
        env::var("ORCHESTRIX_LIVE_CODEX_VERSION").expect("explicit pinned version required");
    assert!(SUPPORTED_CODEX_VERSIONS.contains(&version.as_str()));
    let mut journal = Journal::new(
        &explicit_path("ORCHESTRIX_LIVE_REPORT"),
        &authorization,
        &version,
    );
    let fixture = Fixture::create(explicit_path("ORCHESTRIX_LIVE_FIXTURE_ROOT"));
    let codex = explicit_path("ORCHESTRIX_LIVE_CODEX_EXE");
    let home = explicit_path("ORCHESTRIX_LIVE_CODEX_HOME");
    let check_executable = explicit_path("ORCHESTRIX_LIVE_CHECK_EXE");
    let check_script = fixture.root.join("arithmetic-check.js");
    fs::write(&check_script, CHECK).unwrap();
    let mut launch = RuntimeLaunch::codex(codex, home, fixture.repository.clone(), version);
    launch.turn_timeout = Duration::from_secs(90);
    launch.process_limits.lifetime = Duration::from_secs(300);
    let check = CheckSpec {
        id: "arithmetic-cases".into(),
        executable: check_executable.clone(),
        arguments: vec![
            "//nologo".into(),
            "//B".into(),
            "//E:JScript".into(),
            check_script.to_str().unwrap().into(),
        ],
        authorization_ref: authorization.clone(),
        authorized: true,
        timeout_ms: 10_000,
        executable_sha256: Some(content_hash(&fs::read(check_executable).unwrap())),
    };
    let before = evaluate(
        &check,
        &fixture.repository,
        &content_hash(BROKEN.as_bytes()),
        &CancellationToken::new(),
    )
    .unwrap();
    journal.stage("initial-check", json!({"result":before.result,"exitCode":before.exit_code,"treeConfirmed":before.tree_confirmed,
        "commandHash":before.command_hash,"executableHash":before.executable_hash,"checkScriptSha256":content_hash(CHECK.as_bytes()),
        "arithmeticFailureObserved":before.stdout.contains("Arithmetic assertion failed")}));
    assert_eq!(
        before.result,
        EvaluationResult::Failed,
        "fixture must fail before inference"
    );
    assert!(
        before.stdout.contains("Arithmetic assertion failed"),
        "initial failure must be arithmetic, not an unavailable evaluator"
    );
    let nonce = format!(
        "OX-context-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut policy = PolicyDocument::default();
    policy.mode_defaults.fallback = Some(Fallback::Compatible);
    let input = ManualTaskRequest { schema_version:1,id:"live-arithmetic-worker".into(),project_id:"live-fixture".into(),
        repository_id:"live-repository".into(),repository_directory:fixture.repository.clone(),authorized_worktree_directory:fixture.authorized.clone(),
        base_commit:fixture.base.clone(),connection_id:"live-codex-subscription".into(),connection_revision:1,
        objective:"Correct add(a,b) in sum.cjs so it adds numbers. Make the smallest change, then stop.".into(),
        acceptance_criteria:vec!["add(2,3)=5, add(-3,2)=-1 and add(0,0)=0".into(),"Only sum.cjs changes; preserve the existing ES3-compatible function syntax and CommonJS exports with add: add".into()],
        policy,classification:TaskClassification::routine(TaskKind::Implementation,"live-user-authorized"),
        context_files:vec![ContextFileRequest{path:"sum.cjs".into(),authorization_ref:"read-arithmetic-fixture".into(),required:true}],
        instructions:vec![marker_instruction(&nonce),
            "Edit only sum.cjs. Do not run checks, commands, scripts, network or delegation; the host runs the explicitly authorized deterministic check.".into()],checks:vec![check] };
    let mut store = Store::open(&fixture.database).unwrap();
    journal.reserve("worker-turn-reserved");
    let record = ManualWorker::run(
        &mut store,
        &input,
        launch.clone(),
        &CancellationToken::new(),
    )
    .expect("live worker operation");
    let observed = record
        .observations
        .iter()
        .find(|observation| observation.status == WorkerStatus::RuntimeObserved);
    journal.stage(
        "worker-result",
        json!({"status":record.status(),"delivered":false,"review":"pending-independent-review",
            "dispatch":observed.map(|value| &value.details["dispatch"]),
            "fault":observed.map(|value| &value.details["fault"]),
            "adapterDiagnostic":observed.map(|value| &value.details["adapterDiagnostic"])}),
    );
    assert_eq!(
        record.status(),
        WorkerStatus::Verified,
        "live worker not verified; no retry"
    );
    drop(store);
    let mut store = Store::open(&fixture.database).unwrap();
    let state = store.load().unwrap();
    let persisted = &state.workers()[&input.id];
    assert_eq!(persisted.status(), WorkerStatus::Verified);
    let task = state.runs()[&RunId::new(&record.intent.run_id).unwrap()]
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::NeedsReview);
    assert_eq!(task.verifications().len(), 1);
    let artifact = state
        .captured_artifacts()
        .values()
        .find(|artifact| artifact.attempt_id == record.intent.attempt_id)
        .unwrap();
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.files[0].path, "sum.cjs");
    let initial_artifact = artifact.revision.clone();
    let worker_snapshot = task.attempts().last().unwrap().runtime_snapshot();
    let requested = worker_snapshot["requested"].clone();
    let binding_value = persisted
        .observations
        .iter()
        .find(|observation| observation.status == WorkerStatus::RuntimeObserved)
        .unwrap()
        .details["adapterBinding"]
        .clone();
    let binding = SessionBinding::restore(&binding_value).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.repository.join("sum.cjs")).unwrap(),
        BROKEN
    );
    journal.stage(
        "durable-reopen",
        json!({"worker":"verified","task":"needs-review","artifactRevision":initial_artifact,
        "nonceSha256":content_hash(nonce.as_bytes()),"originalSourceUnchanged":true,"effective":worker_snapshot["effective"]}),
    );
    drop(state);
    // This separate cwd prevents the continuity probe from changing the exact
    // worker artifact. The adapter rejects a resume response that ignores cwd.
    let resumed_launch = launch.with_cwd(fixture.resume.clone());
    let mut adapter = CodexAdapter::connect(
        resumed_launch.clone(),
        &input.connection_id,
        1,
        SystemRefreshClock.now_ms(),
        &CancellationToken::new(),
    )
    .unwrap();
    let prompt = "Recover the checkpoint marker given in the first task from this thread's prior conversation. Write exactly that marker into proof.txt in your current working directory, then stop. Do not read files or databases to find it. Do not access or change any path outside your current working directory.";
    assert!(!prompt.contains(&nonce));
    let request = resumed_request(&binding, &requested, "live-context-resume");
    assert!(!request.as_json().to_string().contains(&nonce));
    journal.reserve("resume-turn-reserved");
    let report = adapter.execute(AttemptExecution {request,prompt:prompt.into(),base_instructions:"Work only in the current authorized directory. Do not delegate or use network.".into(),
        developer_instructions:"This is a continuity probe. Write only proof.txt; do not inspect other files, run scripts or run checks.".into(),resume:Some(binding)},
        &CancellationToken::new(),|event,_|journal.event("resume",event)).unwrap();
    let facts = summary(&report);
    journal.stage("resume-result", facts.clone());
    assert!(report.fault.is_none() && stopped(&report));
    assert_eq!(facts["terminalConfirmed"], true);
    assert_eq!(facts["terminal"]["outcome"], "completed");
    let next_binding = report.binding.unwrap();
    let same_thread = next_binding.checkpoint()["nativeThread"] == binding_value["nativeThread"];
    let proof_path = fixture.resume.join("proof.txt");
    let bounded = fs::metadata(&proof_path).is_ok_and(|metadata| metadata.len() <= 1_024);
    let remembered =
        bounded && fs::read_to_string(&proof_path).is_ok_and(|text| text.trim() == nonce);
    journal.stage("context-proof",json!({"sameNativeThread":same_thread,"nonceRecoveredWithoutResending":remembered,
        "newCwdConfirmedByAdapter":true,"proofSha256":if bounded {Some(content_hash(&fs::read(&proof_path).unwrap()))}else{None}}));
    assert!(
        same_thread && remembered,
        "continuity/cwd not proved; no retry"
    );
    drop(adapter);
    assert_eq!(
        ManualWorker::inspect(&mut store, &input.id)
            .unwrap()
            .status(),
        WorkerStatus::Verified,
        "continuity probe must preserve the verified worker artifact"
    );
    let mut adapter = CodexAdapter::connect(
        resumed_launch,
        &input.connection_id,
        1,
        SystemRefreshClock.now_ms(),
        &CancellationToken::new(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    journal.reserve("cancellation-turn-reserved");
    let report = adapter.execute(AttemptExecution {request:resumed_request(&next_binding,&requested,"live-cancel-probe"),
        prompt:"Do not edit files or run tools. Keep deriving arithmetic examples until the host interrupts this turn; do not conclude on your own.".into(),
        base_instructions:"Work only in the current authorized directory. Do not delegate or use network.".into(),
        developer_instructions:"This turn is intentionally interrupted by the host. Do not write or modify any files.".into(),resume:Some(next_binding)},
        &cancel,|event,_| { if event["type"] == "attempt.start-observed" { trigger.cancel(); } journal.event("cancellation",event) }).unwrap();
    let facts = summary(&report);
    let interrupted = facts["startObserved"] == true
        && facts["terminalConfirmed"] == true
        && facts["terminal"]["outcome"] == "interrupted"
        && facts["interrupt"]["acknowledged"] == true
        && report.fault.is_none()
        && stopped(&report);
    journal.stage("cancellation-result", facts);
    assert!(
        interrupted,
        "cooperative cancellation not proved; factual uncertainty/own Job evidence retained, no retry"
    );
    assert_eq!(
        ManualWorker::inspect(&mut store, &input.id)
            .unwrap()
            .status(),
        WorkerStatus::Verified
    );
    journal.value["accepted"] = json!(true);
    journal.stage("accepted",json!({"worker":"verified","task":"needs-review","contextRecovered":true,"cooperativeInterruption":true,
        "artifactRevision":initial_artifact,"delivered":false,"review":"pending-independent-review"}));
    assert!(fixture.root.exists()); // Intentionally retained, never recursively removed.
}
