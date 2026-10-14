//! Process facts only; exact Git/SQLite binding is covered by manual_worker.
#![cfg(windows)]
use orchestrix_core::{
    evaluator::{CheckSpec, EvaluationResult, evaluate},
    runtime::CancellationToken,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-evaluator-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn spec(argument: &str) -> CheckSpec {
    CheckSpec {
        id: "bounded-check".into(),
        executable: PathBuf::from(env!("CARGO_BIN_EXE_orchestrix-runtime-fixture")),
        arguments: vec![argument.into()],
        authorization_ref: "explicit-test-authority".into(),
        authorized: true,
        timeout_ms: 1500,
        executable_sha256: None,
    }
}
#[test]
fn output_budget_cannot_race_a_successful_process_exit() {
    let directory = Directory::new();
    let report = evaluate(
        &spec("--check-output-limit"),
        &directory.0,
        &"a".repeat(64),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(report.result, EvaluationResult::OutputLimit);
    assert!(report.tree_confirmed && report.streams_closed);
    assert!(report.stdout.len() <= 512 && report.stderr.len() <= 512);
}
#[test]
fn check_deadline_terminates_owned_job_and_is_not_green() {
    let directory = Directory::new();
    let mut input = spec("--check-hang");
    input.timeout_ms = 100;
    let report = evaluate(
        &input,
        &directory.0,
        &"a".repeat(64),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(report.result, EvaluationResult::Timeout);
    assert!(report.tree_confirmed && report.streams_closed);
}
#[test]
fn output_is_sanitized_and_exact_process_command_provenance_is_retained() {
    let directory = Directory::new();
    let input = spec("--check-secret");
    let report = evaluate(
        &input,
        &directory.0,
        &"a".repeat(64),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(report.result, EvaluationResult::Passed);
    assert!(!report.stdout.contains("sk-"));
    assert!(report.stdout.contains("[REDACTED]"));
    assert_eq!(report.authorization_ref, input.authorization_ref);
    assert_eq!(report.timeout_ms, input.timeout_ms);
    assert_eq!(report.cwd, directory.0.to_str().unwrap());
    assert_eq!(
        report.command_hash,
        orchestrix_core::git_records::content_hash(
            serde_json::to_value(&input).unwrap().to_string().as_bytes()
        )
    );
}
#[test]
fn unknown_authority_or_changed_pinned_executable_never_launches() {
    let directory = Directory::new();
    let mut input = spec("--check-secret");
    input.authorized = false;
    assert!(
        evaluate(
            &input,
            &directory.0,
            &"a".repeat(64),
            &CancellationToken::new()
        )
        .is_err()
    );
    input.authorized = true;
    input.executable_sha256 = Some("a".repeat(64));
    assert_eq!(
        evaluate(
            &input,
            &directory.0,
            &"a".repeat(64),
            &CancellationToken::new()
        )
        .unwrap_err(),
        "check-executable-changed"
    );
}
