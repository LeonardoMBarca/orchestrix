//! Explicitly authorized commands, bound to an immutable artifact by the worker.
use crate::{
    git_records::content_hash, persistence_privacy::sanitize_text, runtime::CancellationToken,
};
use orchestrix_process::{OwnedProcess, ProcessEvent, ProcessLimits, SpawnSpec};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSpec {
    pub id: String,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub authorization_ref: String,
    pub authorized: bool,
    pub timeout_ms: u64,
    #[serde(default)]
    pub executable_sha256: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvaluationResult {
    Passed,
    Failed,
    Cancelled,
    Timeout,
    OutputLimit,
    Invalidated,
    ProcessFailure,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckReport {
    pub id: String,
    pub artifact_revision: String,
    pub command_hash: String,
    pub executable_hash: String,
    pub cwd: String,
    pub authorization_ref: String,
    pub timeout_ms: u64,
    pub result: EvaluationResult,
    pub exit_code: Option<u32>,
    pub elapsed_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub output_truncated: bool,
    pub output_sha256: String,
    pub tree_confirmed: bool,
    pub streams_closed: bool,
}
impl CheckSpec {
    pub fn validate(&self) -> Result<(), &'static str> {
        if crate::domain::LocalRef::new(&self.id).is_err()
            || crate::domain::LocalRef::new(&self.authorization_ref).is_err()
            || !self.authorized
            || !self.executable.is_absolute()
            || !self.executable.is_file()
            || self.timeout_ms == 0
            || self.timeout_ms > 90_000
            || self.arguments.len() > 64
            || self
                .arguments
                .iter()
                .any(|s| s.len() > 4096 || s.contains('\0'))
        {
            return Err("check-not-authorized");
        }
        crate::git::paths::reject_links(&self.executable).map_err(|_| "check-path")?;
        if self
            .executable_sha256
            .as_ref()
            .is_some_and(|s| !crate::git_records::is_hash(s))
        {
            return Err("check-hash");
        }
        Ok(())
    }
}
/// Observe an authorized process only. The caller must independently bind bytes/checks to an artifact.
pub fn evaluate(
    spec: &CheckSpec,
    cwd: &Path,
    artifact: &str,
    cancel: &CancellationToken,
) -> Result<CheckReport, &'static str> {
    spec.validate()?;
    let file = std::fs::File::open(&spec.executable).map_err(|_| "check-path")?;
    let mut binary = Vec::new();
    file.take(64 * 1_048_576 + 1)
        .read_to_end(&mut binary)
        .map_err(|_| "check-path")?;
    if binary.len() > 64 * 1_048_576 {
        return Err("check-executable-size");
    }
    let executable_hash = content_hash(&binary);
    if spec
        .executable_sha256
        .as_ref()
        .is_some_and(|s| s != &executable_hash)
    {
        return Err("check-executable-changed");
    }
    let command_hash = content_hash(
        serde_json::to_value(spec)
            .map_err(|_| "check-spec")?
            .to_string()
            .as_bytes(),
    );
    let mut process = OwnedProcess::spawn(SpawnSpec {
        executable: spec.executable.clone(),
        arguments: spec.arguments.iter().map(OsString::from).collect(),
        cwd: cwd.into(),
        environment: environment(),
        limits: ProcessLimits {
            lifetime: Duration::from_millis(spec.timeout_ms),
            max_output_bytes: 65_536,
            max_write_bytes: 1024,
        },
    })
    .map_err(|_| "check-launch")?;
    process.close_input();
    let start = Instant::now();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut result = EvaluationResult::Passed;
    loop {
        if cancel.is_cancelled() {
            result = EvaluationResult::Cancelled;
            break;
        }
        if start.elapsed() >= Duration::from_millis(spec.timeout_ms) {
            result = EvaluationResult::Timeout;
            break;
        }
        match process.receive(Duration::from_millis(10)) {
            Ok(Some(ProcessEvent::Stdout(b))) => out.extend(b),
            Ok(Some(ProcessEvent::Stderr(b))) => err.extend(b),
            Ok(None) => {}
            Err(orchestrix_process::ProcessError::Budget) => {
                result = EvaluationResult::OutputLimit;
                break;
            }
            Err(orchestrix_process::ProcessError::Deadline) => {
                result = EvaluationResult::Timeout;
                break;
            }
            Err(_) => {
                result = EvaluationResult::ProcessFailure;
                break;
            }
        }
        let tree = process.tree().map_err(|_| "check-ownership")?;
        let streams = process.streams();
        if tree.root_exit_code.is_some()
            && tree.active_processes == 0
            && streams.stdout_closed
            && streams.stderr_closed
        {
            while let Ok(Some(event)) = process.receive(Duration::from_millis(1)) {
                match event {
                    ProcessEvent::Stdout(b) => out.extend(b),
                    ProcessEvent::Stderr(b) => err.extend(b),
                }
            }
            break;
        }
    }
    let tree = process
        .terminate(Duration::from_secs(3))
        .map_err(|_| "check-termination")?;
    let streams = process.wait_streams(Duration::from_secs(2));
    let tree_confirmed = tree.owned && tree.root_exit_code.is_some() && tree.active_processes == 0;
    let streams_closed = streams.stdin_closed && streams.stdout_closed && streams.stderr_closed;
    if let Err(error) = process.health() {
        result = match error {
            orchestrix_process::ProcessError::Budget => EvaluationResult::OutputLimit,
            orchestrix_process::ProcessError::Deadline => EvaluationResult::Timeout,
            _ => EvaluationResult::ProcessFailure,
        };
    }
    if result == EvaluationResult::Passed
        && (tree.root_exit_code != Some(0) || !tree_confirmed || !streams_closed)
    {
        result = EvaluationResult::Failed;
    }
    let mut combined = out.clone();
    combined.extend(&err);
    let text = |bytes: &[u8]| -> String {
        let value = String::from_utf8_lossy(bytes);
        sanitize_text(&value)
            .unwrap_or_else(|_| "[output rejected]".into())
            .chars()
            .take(512)
            .collect()
    };
    Ok(CheckReport {
        id: spec.id.clone(),
        artifact_revision: artifact.into(),
        command_hash,
        executable_hash,
        cwd: cwd.to_str().ok_or("check-cwd")?.into(),
        authorization_ref: spec.authorization_ref.clone(),
        timeout_ms: spec.timeout_ms,
        result,
        exit_code: tree.root_exit_code,
        elapsed_ms: start.elapsed().as_millis() as u64,
        stdout: text(&out),
        stderr: text(&err),
        output_truncated: out.len() > 512 || err.len() > 512,
        output_sha256: content_hash(&combined),
        tree_confirmed,
        streams_closed,
    })
}
pub(crate) fn environment() -> Vec<(OsString, OsString)> {
    const KEYS: &[&str] = &[
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "COMSPEC",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "PROGRAMFILES",
    ];
    KEYS.iter()
        .filter_map(|key| std::env::var_os(key).map(|v| ((*key).into(), v)))
        .collect()
}
