//! Native QA only: delay one real Approved inspection reply after Host returns.
//!
//! All files are fixed children of the validated fixture root. Write
//! `native-reply-arm.json` as {integrationId, nonce}; a create-new claim consumes
//! this fixture once, even after errors or another arm. The held marker records
//! the real candidate revision. Only {integrationId, nonce, candidateRevision}
//! in `native-reply-release.json` releases it early. Release is instrumentation,
//! never Core approval. After 40 seconds the unchanged response returns with a
//! `deadline-returned` marker; that outcome does not prove Save-before-release.
//! No response fields, permissions, budgets or daemon state are changed here.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_FILE_BYTES: usize = 2048;
const HOLD_LIMIT: Duration = Duration::from_secs(40);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const ARM: &str = "native-reply-arm.json";
const CLAIM: &str = "native-reply-claim.json";
const HELD: &str = "native-reply-held.json";
const RELEASE: &str = "native-reply-release.json";
const FINISHED: &str = "native-reply-finished.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Arm {
    integration_id: String,
    nonce: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Release {
    integration_id: String,
    nonce: String,
    candidate_revision: String,
}
#[derive(Debug, PartialEq, Eq)]
struct Candidate {
    integration_id: String,
    revision: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Marker<'a> {
    native_qa: bool,
    integration_id: &'a str,
    nonce: &'a str,
    candidate_revision: &'a str,
    status: &'static str,
    pid: u32,
    observed_at_ms: u64,
    outcome: &'static str,
}
#[derive(Debug, PartialEq, Eq)]
enum WaitState {
    Waiting,
    Released,
    DeadlineReturned,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
}
fn valid_nonce(value: &str) -> bool {
    (16..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}
fn valid_revision(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn candidate(requested_id: &str, response: &Value) -> Result<Option<Candidate>, String> {
    let summary = &response["summary"];
    if summary["status"].as_str() != Some("approved") {
        return Ok(None);
    }
    let integration_id = summary["id"]
        .as_str()
        .filter(|value| valid_id(value) && *value == requested_id)
        .ok_or("native-reply-fixture-response-identity")?;
    let revision = summary["candidate_revision"]
        .as_str()
        .filter(|value| valid_revision(value))
        .ok_or("native-reply-fixture-response-revision")?;
    Ok(Some(Candidate {
        integration_id: integration_id.into(),
        revision: revision.into(),
    }))
}
fn parse_arm(bytes: &[u8]) -> Result<Arm, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("native-reply-fixture-file-budget".into());
    }
    let arm: Arm = serde_json::from_slice(bytes).map_err(|_| "native-reply-fixture-arm-schema")?;
    if !valid_id(&arm.integration_id) || !valid_nonce(&arm.nonce) {
        return Err("native-reply-fixture-arm-identity".into());
    }
    Ok(arm)
}
fn state(
    candidate: &Candidate,
    arm: &Arm,
    release: Option<&Release>,
    elapsed: Duration,
) -> Result<WaitState, String> {
    if elapsed >= HOLD_LIMIT {
        return Ok(WaitState::DeadlineReturned);
    }
    let Some(release) = release else {
        return Ok(WaitState::Waiting);
    };
    if release.integration_id != candidate.integration_id
        || release.nonce != arm.nonce
        || release.candidate_revision != candidate.revision
        || !valid_id(&release.integration_id)
        || !valid_nonce(&release.nonce)
        || !valid_revision(&release.candidate_revision)
    {
        return Err("native-reply-fixture-release-mismatch".into());
    }
    Ok(WaitState::Released)
}
fn fixture_root(root: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(root).map_err(|_| "native-reply-fixture-root")?;
    if !root.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("native-reply-fixture-root".into());
    }
    root.canonicalize()
        .map_err(|_| "native-reply-fixture-root".into())
}
fn read_optional(root: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
    let path = root.join(name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("native-reply-fixture-read".into()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("native-reply-fixture-file-type".into());
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err("native-reply-fixture-file-budget".into());
    }
    let file = fs::File::open(path).map_err(|_| "native-reply-fixture-read")?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native-reply-fixture-read")?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("native-reply-fixture-file-budget".into());
    }
    Ok(Some(bytes))
}
fn marker<'a>(
    candidate: &'a Candidate,
    arm: &'a Arm,
    outcome: &'static str,
) -> Result<Marker<'a>, String> {
    let observed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "native-reply-fixture-clock")?
        .as_millis()
        .try_into()
        .map_err(|_| "native-reply-fixture-clock")?;
    Ok(Marker {
        native_qa: true,
        integration_id: &candidate.integration_id,
        nonce: &arm.nonce,
        candidate_revision: &candidate.revision,
        status: "approved",
        pid: std::process::id(),
        observed_at_ms,
        outcome,
    })
}
fn write_new(root: &Path, name: &str, marker: &Marker<'_>) -> Result<bool, String> {
    let bytes = serde_json::to_vec(marker).map_err(|_| "native-reply-fixture-marker-schema")?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("native-reply-fixture-file-budget".into());
    }
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(_) => return Err("native-reply-fixture-marker-create".into()),
    };
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "native-reply-fixture-marker-write")?;
    Ok(true)
}
fn require_new(root: &Path, name: &str, marker: &Marker<'_>) -> Result<(), String> {
    if write_new(root, name, marker)? {
        Ok(())
    } else {
        Err("native-reply-fixture-marker-exists".into())
    }
}

/// Called only after `Host::handle` has returned and released all its guards.
/// Borrowing the response prevents this instrumentation from rewriting it.
pub(super) fn after_inspect(
    qa_root: Option<&Path>,
    requested_id: &str,
    response: &Value,
) -> Result<(), String> {
    let Some(root) = qa_root else {
        return Ok(());
    };
    let Some(candidate) = candidate(requested_id, response)? else {
        return Ok(());
    };
    let root = fixture_root(root)?;
    let Some(bytes) = read_optional(&root, ARM)? else {
        return Ok(());
    };
    let arm = parse_arm(&bytes)?;
    if arm.integration_id != candidate.integration_id {
        return Ok(());
    }
    // create_new is the atomic one-shot arbitration across threads/windows.
    // A partial write also leaves the claim consumed; never retry this fixture.
    if !write_new(&root, CLAIM, &marker(&candidate, &arm, "claimed")?)? {
        return Ok(());
    }
    let began = Instant::now();
    require_new(&root, HELD, &marker(&candidate, &arm, "held")?)?;
    let outcome = loop {
        let elapsed = began.elapsed();
        let release = if elapsed < HOLD_LIMIT {
            read_optional(&root, RELEASE)?
                .map(|bytes| {
                    serde_json::from_slice::<Release>(&bytes)
                        .map_err(|_| "native-reply-fixture-release-schema".to_owned())
                })
                .transpose()?
        } else {
            None
        };
        match state(&candidate, &arm, release.as_ref(), began.elapsed())? {
            WaitState::Released => break "released",
            WaitState::DeadlineReturned => break "deadline-returned",
            WaitState::Waiting => {
                std::thread::sleep(POLL_INTERVAL.min(HOLD_LIMIT.saturating_sub(began.elapsed())));
            }
        }
    };
    require_new(&root, FINISHED, &marker(&candidate, &arm, outcome)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn candidate_fixture() -> Candidate {
        Candidate {
            integration_id: "integration-native-qa".into(),
            revision: "a".repeat(64),
        }
    }
    fn arm_fixture() -> Arm {
        Arm {
            integration_id: "integration-native-qa".into(),
            nonce: "0123456789abcdef".into(),
        }
    }
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "orchestrix-native-reply-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            if self.0.parent() == Some(parent.as_path())
                && self
                    .0
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("orchestrix-native-reply-"))
            {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn selector_requires_real_approved_exact_identity_and_revision() {
        let response = json!({"summary":{"id":"integration-native-qa","status":"approved","candidate_revision":"a".repeat(64)},"historical":true});
        assert_eq!(
            candidate("integration-native-qa", &response).unwrap(),
            Some(candidate_fixture())
        );
        assert!(candidate("other-integration", &response).is_err());
        let mut malformed = response;
        malformed["summary"]["candidate_revision"] = json!("git-commit-is-not-candidate-revision");
        assert!(candidate("integration-native-qa", &malformed).is_err());
        malformed["summary"]["status"] = json!("invalidated");
        assert_eq!(
            candidate("integration-native-qa", &malformed).unwrap(),
            None
        );
    }

    #[test]
    fn arm_schema_and_ascii_identity_are_bounded() {
        assert!(
            parse_arm(br#"{"integrationId":"integration-native-qa","nonce":"0123456789abcdef"}"#)
                .is_ok()
        );
        assert!(parse_arm(br#"{"integrationId":"../other","nonce":"0123456789abcdef"}"#).is_err());
        assert!(
            parse_arm(br#"{"integrationId":"integration-native-qa","nonce":"short"}"#).is_err()
        );
        assert!(parse_arm(br#"{"integrationId":"integration-native-qa","nonce":"0123456789abcdef","path":"other"}"#).is_err());
        assert!(parse_arm(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
        assert!(!valid_nonce("0123456789abcde\u{e9}"));
        assert!(!valid_nonce(&"a".repeat(129)));
    }

    #[test]
    fn release_is_exact_and_deadline_is_not_release() {
        let candidate = candidate_fixture();
        let arm = arm_fixture();
        let mut release = Release {
            integration_id: candidate.integration_id.clone(),
            nonce: arm.nonce.clone(),
            candidate_revision: candidate.revision.clone(),
        };
        assert_eq!(
            state(&candidate, &arm, None, Duration::ZERO).unwrap(),
            WaitState::Waiting
        );
        assert_eq!(
            state(&candidate, &arm, Some(&release), Duration::ZERO).unwrap(),
            WaitState::Released
        );
        release.nonce.push('x');
        assert!(state(&candidate, &arm, Some(&release), Duration::ZERO).is_err());
        release.nonce = arm.nonce.clone();
        release.candidate_revision = "b".repeat(64);
        assert!(state(&candidate, &arm, Some(&release), Duration::ZERO).is_err());
        release.candidate_revision = candidate.revision.clone();
        release.integration_id = "other-integration".into();
        assert!(state(&candidate, &arm, Some(&release), Duration::ZERO).is_err());
        assert_eq!(
            state(&candidate, &arm, Some(&release), HOLD_LIMIT).unwrap(),
            WaitState::DeadlineReturned
        );
    }

    #[test]
    fn atomic_claim_is_one_shot_even_for_a_different_nonce() {
        let scratch = Scratch::new();
        let candidate = candidate_fixture();
        let mut arm = arm_fixture();
        assert!(
            write_new(
                &scratch.0,
                CLAIM,
                &marker(&candidate, &arm, "claimed").unwrap()
            )
            .unwrap()
        );
        arm.nonce = "fedcba9876543210".into();
        assert!(
            !write_new(
                &scratch.0,
                CLAIM,
                &marker(&candidate, &arm, "claimed").unwrap()
            )
            .unwrap()
        );
        let stored: Value =
            serde_json::from_slice(&read_optional(&scratch.0, CLAIM).unwrap().unwrap()).unwrap();
        assert_eq!(stored["nonce"], "0123456789abcdef");
    }

    #[test]
    fn bounded_reader_and_unarmed_inspection_do_not_claim() {
        let scratch = Scratch::new();
        assert_eq!(read_optional(&scratch.0, ARM).unwrap(), None);
        let response = json!({"summary":{"id":"integration-native-qa","status":"approved","candidate_revision":"a".repeat(64)}});
        after_inspect(Some(&scratch.0), "integration-native-qa", &response).unwrap();
        assert!(!scratch.0.join(CLAIM).exists());
        fs::write(scratch.0.join(ARM), vec![b' '; MAX_FILE_BYTES + 1]).unwrap();
        assert_eq!(
            read_optional(&scratch.0, ARM).unwrap_err(),
            "native-reply-fixture-file-budget"
        );
    }

    #[test]
    fn exact_release_returns_unchanged_real_response_without_long_wait() {
        let scratch = Scratch::new();
        fs::write(
            scratch.0.join(ARM),
            br#"{"integrationId":"integration-native-qa","nonce":"0123456789abcdef"}"#,
        )
        .unwrap();
        fs::write(scratch.0.join(RELEASE), serde_json::to_vec(&json!({"integrationId":"integration-native-qa","nonce":"0123456789abcdef","candidateRevision":"a".repeat(64)})).unwrap()).unwrap();
        let response = json!({"summary":{"id":"integration-native-qa","status":"approved","candidate_revision":"a".repeat(64)},"historical":true,"currentVerification":false});
        let original = response.clone();
        after_inspect(Some(&scratch.0), "integration-native-qa", &response).unwrap();
        assert_eq!(response, original);
        let finished: Value =
            serde_json::from_slice(&read_optional(&scratch.0, FINISHED).unwrap().unwrap()).unwrap();
        assert_eq!(finished["outcome"], "released");
        assert_eq!(finished["candidateRevision"], "a".repeat(64));
        assert_eq!(finished["status"], "approved");
        assert_eq!(finished["pid"], std::process::id());
        after_inspect(Some(&scratch.0), "integration-native-qa", &response).unwrap();
    }
}
