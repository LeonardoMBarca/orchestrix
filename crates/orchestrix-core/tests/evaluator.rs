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

#[test]
fn oversized_executable_is_rejected_by_preflight_without_reinterpreting_legacy_spec() {
    let directory = Directory::new();
    let executable = directory.0.join("oversized-check.exe");
    let file = fs::File::create(&executable).unwrap();
    file.set_len(64 * 1_048_576 + 1).unwrap();
    drop(file);
    let mut input = spec("--check-secret");
    input.executable = executable;
    input.executable_sha256 = Some("a".repeat(64));
    // Historical structural validation never claimed a current executable
    // size/hash observation. Its serialized representation stays unchanged.
    input.validate().unwrap();
    let historical = serde_json::to_string(&input).unwrap();
    let restored: CheckSpec = serde_json::from_str(&historical).unwrap();
    restored.validate().unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), historical);
    assert_eq!(
        restored.preflight_executable().unwrap_err(),
        "check-executable-size"
    );
    assert_eq!(
        evaluate(
            &restored,
            &directory.0,
            &"a".repeat(64),
            &CancellationToken::new()
        )
        .unwrap_err(),
        "check-executable-size"
    );
    restored.validate().unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), historical);
    assert_eq!(
        fs::metadata(&restored.executable).unwrap().len(),
        64 * 1_048_576 + 1
    );
}

#[test]
fn same_size_change_after_preflight_is_refused_by_preflight_and_evaluation() {
    let directory = Directory::new();
    let executable = directory.0.join("bounded-byte-input.exe");
    fs::write(&executable, b"before\n").unwrap();
    let mut input = spec("--check-secret");
    input.executable = executable;
    let pin = input.preflight_executable().unwrap();
    assert_eq!(pin, orchestrix_core::git_records::content_hash(b"before\n"));
    input.executable_sha256 = Some(pin);
    fs::write(&input.executable, b"edited\n").unwrap();
    assert_eq!(fs::metadata(&input.executable).unwrap().len(), 7);
    input.validate().unwrap();
    assert_eq!(
        input.preflight_executable().unwrap_err(),
        "check-executable-changed"
    );
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
    assert_eq!(fs::read(&input.executable).unwrap(), b"edited\n");
}

#[test]
fn executable_preflight_pin_is_rechecked_and_retained_by_a_normal_check() {
    let directory = Directory::new();
    fs::write(directory.0.join("fixture.txt"), "after\n").unwrap();
    let mut input = spec("--check-fixture");
    input.arguments.push("after\n".into());
    let pin = input.preflight_executable().unwrap();
    input.executable_sha256 = Some(pin.clone());
    let report = evaluate(
        &input,
        &directory.0,
        &"a".repeat(64),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(report.result, EvaluationResult::Passed);
    assert_eq!(report.exit_code, Some(0));
    assert_eq!(report.executable_hash, pin);
    assert!(report.tree_confirmed && report.streams_closed);
    assert_eq!(
        fs::read(directory.0.join("fixture.txt")).unwrap(),
        b"after\n"
    );
}
