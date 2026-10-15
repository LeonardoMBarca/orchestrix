//! Opt-in four-turn provider proof; normal test runs never consume a subscription.
//! All fixture files and the bounded report are retained after success or failure.
#![cfg(windows)]
use orchestrix_core::{
    domain::{LocalRef, RunId, TaskId, TaskState},
    evaluator::{CheckSpec, EvaluationResult, evaluate},
    git_records::content_hash,
    persistence::Store,
    policy::{Fallback, PolicyDocument, TaskClassification, TaskKind},
    review::ReviewPipeline,
    review_records::{ReviewBudget, ReviewRecord, ReviewRequest, ReviewStatus},
    runtime::{CancellationToken, RuntimeLaunch},
    worker::{ContextFileRequest, ManualTaskRequest, ManualWorker},
    worker_records::{WorkerRecord, WorkerStatus},
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const VERSION: &str = "0.162.0-alpha.17.2";
const BROKEN: &str = "function add(a, b) { if (a === 0 && b === 0) { return 1; } return a - b; }\nmodule.exports = { add: add };\n";
const PARTIAL: &str = "function add(a, b) { if (a === 0 && b === 0) { return 1; } return a + b; }\nmodule.exports = { add: add };\n";
const CORRECT: &str = "function add(a, b) { return a + b; }\nmodule.exports = { add: add };\n";
// ES3 JScript, as in the existing R9 arithmetic control: no Node dependency,
// shell, discovered project script or provider call is needed by the host.
const POSITIVE_CHECK: &str = "var input = new ActiveXObject('Scripting.FileSystemObject').OpenTextFile('sum.cjs', 1, false);\nvar source = input.ReadAll(); input.Close();\nvar module = { exports: {} }; eval(source);\nvar add = module.exports.add;\nif (typeof add !== 'function' || add(2, 3) !== 5 || add(4, 7) !== 11) { WScript.StdOut.WriteLine('Positive arithmetic assertion failed'); WScript.Quit(1); }\nWScript.StdOut.WriteLine('Two positive arithmetic cases passed');\n";
const COMPLETE_CHECK: &str = "var input = new ActiveXObject('Scripting.FileSystemObject').OpenTextFile('sum.cjs', 1, false);\nvar source = input.ReadAll(); input.Close();\nvar module = { exports: {} }; eval(source);\nvar add = module.exports.add;\nif (typeof add !== 'function' || add(2, 3) !== 5 || add(4, 7) !== 11 || add(-3, 2) !== -1 || add(0, 0) !== 0) { WScript.StdOut.WriteLine('Complete arithmetic assertion failed'); WScript.Quit(1); }\nWScript.StdOut.WriteLine('Four arithmetic cases passed');\n";
const INITIAL_ONLY: &str = "This is a deliberately staged review-pipeline proof. Only for the initial implementation attempt, correct addition for nonzero inputs while retaining the existing incorrect add(0,0) result of 1 as a known limitation. This is not the final acceptance criterion. On an explicitly authorized correction attempt, address all reviewer findings and meet all final acceptance criteria, including add(0,0)=0. Do not add staging comments, extra files, tests or scripts. Edit only sum.cjs, preserve ES3-compatible syntax and CommonJS exports, and do not delegate or use the network. Do not run checks or scripts: the host runs only its explicitly authorized checks.";

struct Journal {
    file: fs::File,
    value: Value,
    reserved: u8,
}
impl Journal {
    fn new(path: &Path, authorization: &str) -> Self {
        assert!(path.is_absolute());
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("new report required; no overwrite or automatic retry");
        let mut journal = Self {
            file,
            value: json!({"schemaVersion":1,"synthetic":false,"proof":"independent-review-correction",
                "authorizationSha256":content_hash(authorization.as_bytes()),"runtimeVersion":VERSION,
                "platform":"windows","billing":"subscription-only","paidFallback":"forbidden",
                "limits":{"maxTurns":4,"turnSeconds":90,"jobSeconds":300,"reviewChainMs":1_800_000,"serial":true,"automaticRetries":0},
                "initialImplementation":"deliberately-partial-zero-case","workerChecks":"two-positive-cases-only",
                "selection":{"modelPolicy":"runtime-default","reasoning":"runtime-default","fallback":"compatible"},
                "reservedTurns":0,"accepted":false,"steps":[]}),
            reserved: 0,
        };
        journal.flush();
        journal
    }
    fn flush(&mut self) {
        let bytes = serde_json::to_vec_pretty(&self.value).unwrap();
        assert!(bytes.len() <= 65_536, "bounded proof report");
        self.file.seek(SeekFrom::Start(0)).unwrap();
        self.file.set_len(0).unwrap();
        self.file.write_all(&bytes).unwrap();
        self.file.sync_all().unwrap();
    }
    fn stage(&mut self, name: &str, facts: Value) {
        let steps = self.value["steps"].as_array_mut().unwrap();
        assert!(steps.len() < 32);
        steps.push(json!({"stage":name,"facts":facts}));
        self.flush();
    }
    fn reserve(&mut self, name: &str) {
        assert!(self.reserved < 4, "four-turn allowance exhausted; no retry");
        self.reserved += 1;
        self.value["reservedTurns"] = json!(self.reserved);
        self.stage(name, json!({"dispatch":"not-yet-confirmed"}));
    }
}
struct Fixture {
    root: PathBuf,
    repository: PathBuf,
    authorized: PathBuf,
    database: PathBuf,
    base: String,
}
impl Fixture {
    fn create(root: PathBuf) -> Self {
        assert!(root.is_absolute() && !root.exists());
        let temporary = fs::canonicalize(env::temp_dir()).unwrap();
        assert_eq!(
            fs::canonicalize(root.parent().unwrap()).unwrap(),
            temporary,
            "isolated fixture must be a direct child of system temporary directory"
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchestrix-live-review-")
        );
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let repository = root.join("source");
        let authorized = root.join("attempts");
        fs::create_dir(&repository).unwrap();
        fs::create_dir(&authorized).unwrap();
        git(&repository, &["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("sum.cjs"), BROKEN).unwrap();
        git(&repository, &["add", "sum.cjs"]);
        git(
            &repository,
            &[
                "commit",
                "--quiet",
                "-m",
                "Isolated review arithmetic fixture",
            ],
        );
        let base = String::from_utf8(git(&repository, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned();
        fs::write(root.join("positive-check.js"), POSITIVE_CHECK).unwrap();
        fs::write(root.join("complete-check.js"), COMPLETE_CHECK).unwrap();
        Self {
            database: root.join("state.sqlite"),
            root,
            repository,
            authorized,
            base,
        }
    }
    fn check(&self, complete: bool, authorization: &str) -> CheckSpec {
        let executable = evaluator_executable();
        CheckSpec {
            id: if complete {
                "complete-arithmetic"
            } else {
                "positive-arithmetic"
            }
            .into(),
            executable: executable.clone(),
            arguments: vec![
                "//nologo".into(),
                "//B".into(),
                "//E:JScript".into(),
                self.root
                    .join(if complete {
                        "complete-check.js"
                    } else {
                        "positive-check.js"
                    })
                    .to_str()
                    .unwrap()
                    .into(),
            ],
            authorization_ref: authorization.into(),
            authorized: true,
            timeout_ms: 10_000,
            executable_sha256: Some(content_hash(&fs::read(executable).unwrap())),
        }
    }
    fn scripts_unchanged(&self) {
        assert_eq!(
            fs::read_to_string(self.root.join("positive-check.js")).unwrap(),
            POSITIVE_CHECK
        );
        assert_eq!(
            fs::read_to_string(self.root.join("complete-check.js")).unwrap(),
            COMPLETE_CHECK
        );
    }
}
fn explicit_path(name: &str) -> PathBuf {
    let value = PathBuf::from(env::var_os(name).expect("explicit live path required"));
    assert!(value.is_absolute());
    value
}
fn evaluator_executable() -> PathBuf {
    PathBuf::from(env::var_os("SystemRoot").expect("Windows SystemRoot"))
        .join("System32")
        .join("cscript.exe")
}
fn git(cwd: &Path, arguments: &[&str]) -> Vec<u8> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("git");
    command.creation_flags(0x0800_0000).env_clear();
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
fn worker_facts(record: &WorkerRecord, store: &Store) -> Value {
    let observed = record
        .observations
        .iter()
        .find(|o| o.status == WorkerStatus::RuntimeObserved);
    let state = store.load().unwrap();
    let snapshot = state.runs()[&RunId::new(&record.intent.run_id).unwrap()]
        .task(&TaskId::new(&record.intent.task_id).unwrap())
        .unwrap()
        .attempts()
        .iter()
        .find(|attempt| attempt.id().as_str() == record.intent.attempt_id)
        .unwrap()
        .runtime_snapshot();
    json!({"status":record.status(),"artifactRevision":record.observations.last().map(|o|&o.details["artifactRevision"]),
        "fault":observed.map(|o|&o.details["fault"]),"dispatch":observed.map(|o|&o.details["dispatch"]),
        "adapterDiagnostic":observed.map(|o|&o.details["adapterDiagnostic"]),
        "requested":snapshot["requested"],"effective":snapshot["effective"],
        "synthetic":observed.map(|o|&o.details["synthetic"]),"delivered":false})
}
fn review_facts(record: &ReviewRecord) -> Value {
    let summary = record.summary();
    json!({"status":record.status(),"artifactRevision":record.intent.artifact_revision,
        "requested":summary["requested"],"effective":summary["effective"],"policyRevision":summary["policy_revision"],
        "synthetic":summary["synthetic"],"findingCount":record.output().map(|o|o.findings.len()),"delivered":false})
}
fn request(
    worker: &WorkerRecord,
    revision: &str,
    id: &str,
    authorization: &str,
    policy: &PolicyDocument,
) -> ReviewRequest {
    ReviewRequest {
        schema_version: 1,
        id: id.into(),
        worker_id: worker.intent.id.clone(),
        artifact_revision: revision.into(),
        authorization_ref: authorization.into(),
        policy: policy.clone(),
        classification: TaskClassification::routine(
            TaskKind::Implementation,
            "live-independent-review-authorized",
        ),
        budget: ReviewBudget {
            max_corrections: 1,
            max_reviews: 2,
            max_elapsed_ms: 1_800_000,
        },
    }
}

#[test]
fn live_review_arithmetic_controls_distinguish_partial_implementation_without_provider() {
    let root = fs::canonicalize(env::temp_dir()).unwrap().join(format!(
        "orchestrix-live-review-control-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let fixture = Fixture::create(root);
    let cancel = CancellationToken::new();
    for (name, source, positive, complete) in [
        (
            "broken",
            BROKEN,
            EvaluationResult::Failed,
            EvaluationResult::Failed,
        ),
        (
            "partial",
            PARTIAL,
            EvaluationResult::Passed,
            EvaluationResult::Failed,
        ),
        (
            "correct",
            CORRECT,
            EvaluationResult::Passed,
            EvaluationResult::Passed,
        ),
    ] {
        let cwd = fixture.root.join(name);
        fs::create_dir(&cwd).unwrap();
        fs::write(cwd.join("sum.cjs"), source).unwrap();
        for (full, expected) in [(false, positive), (true, complete)] {
            let evidence = evaluate(
                &fixture.check(full, "offline-review-arithmetic-control"),
                &cwd,
                &content_hash(source.as_bytes()),
                &cancel,
            )
            .unwrap();
            assert_eq!(evidence.result, expected);
            assert_eq!(
                evidence.exit_code,
                Some(if expected == EvaluationResult::Passed {
                    0
                } else {
                    1
                })
            );
            assert!(
                evidence.stdout.contains(if full {
                    if expected == EvaluationResult::Passed {
                        "Four arithmetic cases passed"
                    } else {
                        "Complete arithmetic assertion failed"
                    }
                } else if expected == EvaluationResult::Passed {
                    "Two positive arithmetic cases passed"
                } else {
                    "Positive arithmetic assertion failed"
                }),
                "arithmetic assertion, not WSH/syntax failure, is required"
            );
            assert!(evidence.tree_confirmed && evidence.streams_closed);
        }
        assert_eq!(fs::read_to_string(cwd.join("sum.cjs")).unwrap(), source);
    }
    fixture.scripts_unchanged();
    assert!(fixture.root.exists()); // Retained controls; never recursively remove a computed path.
}

#[test]
#[ignore = "Consumes up to four subscription turns; requires a fresh explicit review authorization and all live environment gates"]
fn codex_independent_review_and_same_task_correction_live_opt_in() {
    assert_eq!(
        env::var("ORCHESTRIX_ALLOW_LIVE_REVIEW").as_deref(),
        Ok("four-turns-subscription-only")
    );
    assert_eq!(env::var("ORCHESTRIX_LIVE_MAX_TURNS").as_deref(), Ok("4"));
    assert_eq!(
        env::var("ORCHESTRIX_LIVE_CODEX_VERSION").as_deref(),
        Ok(VERSION)
    );
    let authorization = env::var("ORCHESTRIX_LIVE_AUTHORIZATION_ID")
        .expect("fresh explicit authorization reference required");
    LocalRef::new(&authorization).unwrap();
    let mut journal = Journal::new(&explicit_path("ORCHESTRIX_LIVE_REPORT"), &authorization);
    let fixture = Fixture::create(explicit_path("ORCHESTRIX_LIVE_FIXTURE_ROOT"));
    let launch = {
        let mut launch = RuntimeLaunch::codex(
            explicit_path("ORCHESTRIX_LIVE_CODEX_EXE"),
            explicit_path("ORCHESTRIX_LIVE_CODEX_HOME"),
            fixture.repository.clone(),
            VERSION.into(),
        );
        launch.turn_timeout = Duration::from_secs(90);
        launch.process_limits.lifetime = Duration::from_secs(300);
        launch
    };
    let cancel = CancellationToken::new();
    let mut policy = PolicyDocument::default();
    policy.mode_defaults.fallback = Some(Fallback::Compatible);
    let input=ManualTaskRequest{schema_version:1,id:"live-review-implementation".into(),project_id:"live-review-fixture".into(),
        repository_id:"live-review-repository".into(),repository_directory:fixture.repository.clone(),
        authorized_worktree_directory:fixture.authorized.clone(),base_commit:fixture.base.clone(),
        connection_id:"live-codex-subscription".into(),connection_revision:1,
        objective:"Implement numeric addition in add(a,b) in sum.cjs, with a staged first attempt followed by an independent final-criteria review.".into(),
        acceptance_criteria:vec!["Final behavior: add(2,3)=5, add(4,7)=11, add(-3,2)=-1 and add(0,0)=0. A known first-attempt zero limitation does not satisfy final acceptance.".into(),
            "Only sum.cjs changes. Preserve ES3-compatible function syntax and CommonJS exports with add: add.".into()],
        policy:policy.clone(),classification:TaskClassification::routine(TaskKind::Implementation,"live-review-user-authorized"),
        context_files:vec![ContextFileRequest{path:"sum.cjs".into(),authorization_ref:authorization.clone(),required:true}],
        instructions:vec![INITIAL_ONLY.into()],checks:vec![fixture.check(false,&authorization)]};
    let before = evaluate(
        &input.checks[0],
        &fixture.repository,
        &content_hash(BROKEN.as_bytes()),
        &cancel,
    )
    .unwrap();
    journal.stage("initial-positive-check",json!({"result":before.result,"exitCode":before.exit_code,
        "arithmeticFailureObserved":before.stdout.contains("Positive arithmetic assertion failed"),
        "scriptSha256":content_hash(POSITIVE_CHECK.as_bytes()),"executableSha256":before.executable_hash,
        "completeScriptSha256":content_hash(COMPLETE_CHECK.as_bytes()),"originalSourceSha256":content_hash(BROKEN.as_bytes())}));
    assert_eq!(before.result, EvaluationResult::Failed);
    assert!(
        before
            .stdout
            .contains("Positive arithmetic assertion failed")
    );
    let mut store = Store::open(&fixture.database).unwrap();
    journal.reserve("implementation-turn-reserved");
    let implemented = ManualWorker::run(&mut store, &input, launch.clone(), &cancel)
        .expect("implementation operation; no retry");
    let facts = worker_facts(&implemented, &store);
    journal.stage("implementation-result", facts.clone());
    assert_eq!(facts["synthetic"], false);
    assert_eq!(
        implemented.status(),
        WorkerStatus::Verified,
        "implementation not verified; stop without retry"
    );
    fixture.scripts_unchanged();
    let state = store.load().unwrap();
    let source_task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    let original = source_task.artifact().unwrap().clone();
    let original_capture = &state.captured_artifacts()[&(
        original.id().as_str().to_owned(),
        original.revision().as_str().to_owned(),
    )];
    assert_eq!(original_capture.base_oid, fixture.base);
    assert_eq!(original_capture.files.len(), 1);
    assert_eq!(original_capture.files[0].path, "sum.cjs");
    let original_session = source_task.attempts()[0].session().id().as_str().to_owned();
    assert_eq!(source_task.state(), TaskState::NeedsReview);
    assert_eq!(source_task.verifications().len(), 1);
    drop(state);
    drop(store);
    let mut store = Store::open(&fixture.database).unwrap();
    let first_review = request(
        &implemented,
        original.revision().as_str(),
        "live-independent-review-one",
        &authorization,
        &policy,
    );
    journal.reserve("independent-review-turn-reserved");
    let rejected = ReviewPipeline::review(&mut store, &first_review, launch.clone(), &cancel)
        .expect("review operation; no retry");
    journal.stage("independent-review-result", review_facts(&rejected));
    assert_eq!(rejected.summary()["synthetic"], false);
    assert_eq!(
        rejected.status(),
        ReviewStatus::Rejected,
        "a factual rejection is required to exercise correction; accepted, invalid, unavailable or uncertain stops the proof without retry"
    );
    assert_ne!(rejected.intent.session_id, original_session);
    assert!(
        rejected
            .output()
            .unwrap()
            .findings
            .iter()
            .any(|f| f.path == "sum.cjs" && f.artifact_revision == original.revision().as_str())
    );
    drop(store);
    let mut store = Store::open(&fixture.database).unwrap();
    let correction = ReviewPipeline::prepare_correction(
        &store,
        &rejected.intent.id,
        "live-review-correction",
        &authorization,
        policy.clone(),
        input.classification.clone(),
    )
    .unwrap();
    assert_eq!(
        correction.task.instructions, input.instructions,
        "first-attempt-only instruction remains identical in provenance"
    );
    assert_eq!(json!(correction.task.checks), json!(input.checks));
    journal.reserve("correction-turn-reserved");
    let corrected = ReviewPipeline::correct(&mut store, &correction, launch.clone(), &cancel)
        .expect("correction operation; no retry");
    let facts = worker_facts(&corrected, &store);
    journal.stage("correction-result", facts.clone());
    assert_eq!(facts["synthetic"], false);
    assert_eq!(
        corrected.status(),
        WorkerStatus::Verified,
        "correction not verified; stop without retry"
    );
    assert_eq!(corrected.intent.run_id, implemented.intent.run_id);
    assert_eq!(corrected.intent.task_id, implemented.intent.task_id);
    fixture.scripts_unchanged();
    let state = store.load().unwrap();
    let task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(state.runs().len(), 1);
    assert_eq!(task.attempts().len(), 2);
    assert_eq!(task.verifications().len(), 2);
    assert_eq!(
        task.attempts()[1].inputs().correction_of.as_ref(),
        Some(&original)
    );
    let current = task.artifact().unwrap().clone();
    assert_ne!(current, original);
    let artifact = &state.captured_artifacts()[&(
        current.id().as_str().to_owned(),
        current.revision().as_str().to_owned(),
    )];
    assert_eq!(artifact.base_oid, fixture.base);
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.files[0].path, "sum.cjs");
    let cwd = PathBuf::from(
        &state
            .worktrees()
            .values()
            .find(|w| w.intent.attempt_id == corrected.intent.attempt_id)
            .unwrap()
            .intent
            .path,
    );
    drop(state);
    let complete = evaluate(
        &fixture.check(true, &authorization),
        &cwd,
        current.revision().as_str(),
        &cancel,
    )
    .unwrap();
    journal.stage("authorized-final-arithmetic-control",json!({"result":complete.result,"exitCode":complete.exit_code,
        "artifactRevision":current.revision().as_str(),"treeConfirmed":complete.tree_confirmed,"streamsClosed":complete.streams_closed,
        "arithmeticSuccessObserved":complete.stdout.contains("Four arithmetic cases passed"),"scriptSha256":content_hash(COMPLETE_CHECK.as_bytes())}));
    assert_eq!(
        complete.result,
        EvaluationResult::Passed,
        "correction must actually satisfy zero case before final review"
    );
    assert!(complete.stdout.contains("Four arithmetic cases passed"));
    assert_eq!(
        ManualWorker::inspect(&mut store, &corrected.intent.id)
            .unwrap()
            .status(),
        WorkerStatus::Verified
    );
    fixture.scripts_unchanged();
    journal.reserve("corrected-independent-review-turn-reserved");
    let accepted = ReviewPipeline::review(
        &mut store,
        &request(
            &corrected,
            current.revision().as_str(),
            "live-independent-review-two",
            &authorization,
            &policy,
        ),
        launch,
        &cancel,
    )
    .expect("corrected review operation; no retry");
    journal.stage(
        "corrected-independent-review-result",
        review_facts(&accepted),
    );
    assert_eq!(accepted.summary()["synthetic"], false);
    assert_eq!(
        accepted.status(),
        ReviewStatus::Accepted,
        "final review not accepted; stop without retry"
    );
    assert_ne!(accepted.intent.session_id, rejected.intent.session_id);
    drop(store);
    let store = Store::open(&fixture.database).unwrap();
    let state = store.load().unwrap();
    let task = state.runs()[&RunId::new(&implemented.intent.run_id).unwrap()]
        .task(&TaskId::new(&implemented.intent.task_id).unwrap())
        .unwrap();
    assert_eq!(task.state(), TaskState::Approved);
    assert_eq!(task.reviews().len(), 2);
    assert!(task.integrations().is_empty());
    assert_eq!(
        fs::read_to_string(fixture.repository.join("sum.cjs")).unwrap(),
        BROKEN
    );
    fixture.scripts_unchanged();
    journal.value["accepted"] = json!(true);
    journal.stage("durable-proof-accepted",json!({"sameRunTask":true,"attempts":2,"independentReviews":2,
        "positiveChecksPerAttempt":1,"finalArithmeticControl":"four-cases-passed","artifactRevision":current.revision().as_str(),
        "originalCheckoutUnchanged":true,"delivered":false,"application":"pending-OX-014"}));
}
