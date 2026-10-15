//! Trusted keeper of the already-owned Job. It observes the Core's exact process
//! handle and writes only local containment facts, never provider/domain events.
use crate::{
    git_records::{content_hash, is_hash},
    worker::WorkerError,
};
use orchestrix_process::{
    lease::HostLease,
    named_job::{self, HostWatch, JobScope},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, WorkerError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuardianBinding {
    pub schema_version: u32,
    pub scope: String,
    pub directory: PathBuf,
    pub authority: String,
    pub host_pid: u32,
    pub host_created: u64,
    pub issued_at_ms: u64,
    pub lifetime_ms: u64,
    pub helper_sha256: String,
}
impl GuardianBinding {
    pub fn prepare(database: &Path, attempt: &str, helper: &Path) -> Result<Self> {
        let nonce = named_job::nonce().map_err(|_| WorkerError("guardian-ownership"))?;
        let authority = named_job::nonce().map_err(|_| WorkerError("guardian-ownership"))?;
        let scope = content_hash(json!([database, attempt, nonce]).to_string().as_bytes());
        let (host_pid, host_created) =
            named_job::host_identity().map_err(|_| WorkerError("guardian-owner"))?;
        let helper_sha256 = file_hash(helper)?;
        Ok(Self {
            schema_version: 1,
            directory: database
                .parent()
                .ok_or(WorkerError("guardian-directory"))?
                .join(format!("orchestrix-supervisor-{scope}")),
            scope,
            authority,
            host_pid,
            host_created,
            issued_at_ms: now()?,
            lifetime_ms: 300_000,
            helper_sha256,
        })
    }
    pub(crate) fn validate(&self) -> bool {
        self.schema_version == 1
            && is_hash(&self.scope)
            && is_hash(&self.authority)
            && is_hash(&self.helper_sha256)
            && self.directory.is_absolute()
            && self
                .directory
                .file_name()
                .is_some_and(|s| s == format!("orchestrix-supervisor-{}", self.scope).as_str())
            && self.host_pid > 0
            && self.host_created > 0
            && self.lifetime_ms > 0
            && self.lifetime_ms <= 300_000
    }
    fn digest(&self) -> String {
        content_hash(
            serde_json::to_vec(self)
                .expect("serializable binding")
                .as_slice(),
        )
    }
}
pub struct Guardian {
    binding: GuardianBinding,
    child: Child,
    job: JobScope,
    started: Instant,
}
impl Guardian {
    pub fn start(binding: GuardianBinding, helper: &Path) -> Result<Self> {
        if !binding.validate() || file_hash(helper)? != binding.helper_sha256 {
            return Err(WorkerError("guardian-helper-identity"));
        }
        crate::git::paths::reject_links(
            binding
                .directory
                .parent()
                .ok_or(WorkerError("guardian-directory"))?,
        )
        .map_err(|_| WorkerError("guardian-directory"))?;
        named_job::private_directory(&binding.directory)
            .map_err(|_| WorkerError("guardian-directory"))?;
        write_new(
            &binding.directory.join("spec.json"),
            &serde_json::to_vec(&binding).map_err(|_| WorkerError("guardian-spec"))?,
        )?;
        let job =
            JobScope::create(&binding.scope).map_err(|_| WorkerError("guardian-job-collision"))?;
        let mut command = Command::new(helper);
        command.env_clear();
        for key in ["PATH", "SystemRoot", "SystemDrive", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        command
            .args([
                OsString::from("--scope-directory"),
                binding.directory.as_os_str().into(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let child = command.spawn().map_err(|_| WorkerError("guardian-spawn"))?;
        let mut guard = Self {
            binding,
            child,
            job,
            started: Instant::now(),
        };
        let begin = Instant::now();
        loop {
            if let Ok(bytes) = read_bounded(&guard.binding.directory.join("ready.json")) {
                let ready: Value =
                    serde_json::from_slice(&bytes).map_err(|_| WorkerError("guardian-ready"))?;
                if ready
                    == json!({"bindingHash":guard.binding.digest(),"scope":guard.binding.scope,"ownerBound":true,"jobHandleVerified":true,"keeperPid":guard.child.id()})
                {
                    return Ok(guard);
                }
                return Err(WorkerError("guardian-ready-identity"));
            }
            if guard
                .child
                .try_wait()
                .map_err(|_| WorkerError("guardian-process"))?
                .is_some()
                || begin.elapsed() >= Duration::from_secs(5)
            {
                return Err(WorkerError("guardian-not-ready"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub fn job(&mut self) -> Result<JobScope> {
        if self.started.elapsed() >= Duration::from_millis(self.binding.lifetime_ms)
            || self
                .child
                .try_wait()
                .map_err(|_| WorkerError("guardian-process"))?
                .is_some()
        {
            return Err(WorkerError("guardian-unavailable"));
        }
        Ok(self.job.clone())
    }
    pub(crate) fn stop_for_fault(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(|_| WorkerError("guardian-fixture-stop"))?;
        let begin = Instant::now();
        while begin.elapsed() < Duration::from_secs(3) {
            if self
                .child
                .try_wait()
                .map_err(|_| WorkerError("guardian-fixture-stop"))?
                .is_some()
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(WorkerError("guardian-fixture-stop-unconfirmed"))
    }
}
impl Drop for Guardian {
    fn drop(&mut self) {
        let _ = write_new(&self.binding.directory.join("finish"), b"finish");
        let begin = Instant::now();
        while begin.elapsed() < Duration::from_secs(5) {
            if self.child.try_wait().is_ok_and(|s| s.is_some()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = self.job.terminate();
        let _ = self.child.kill();
        let ending = Instant::now();
        while ending.elapsed() < Duration::from_secs(3) {
            if self.child.try_wait().is_ok_and(|s| s.is_some()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
pub fn default_helper() -> Result<PathBuf> {
    let current = std::env::current_exe().map_err(|_| WorkerError("guardian-helper"))?;
    let mut directory = current.parent().ok_or(WorkerError("guardian-helper"))?;
    if directory.file_name().is_some_and(|s| s == "deps") {
        directory = directory.parent().ok_or(WorkerError("guardian-helper"))?;
    }
    Ok(directory.join(if cfg!(windows) {
        "orchestrix-supervisor.exe"
    } else {
        "orchestrix-supervisor"
    }))
}
pub fn entry(arguments: Vec<OsString>) -> Result<()> {
    if arguments.len() != 2 || arguments[0] != "--scope-directory" {
        return Err(WorkerError("guardian-arguments"));
    }
    let directory = PathBuf::from(&arguments[1]);
    let bytes = read_bounded(&directory.join("spec.json"))?;
    let binding: GuardianBinding =
        serde_json::from_slice(&bytes).map_err(|_| WorkerError("guardian-spec"))?;
    if !binding.validate()
        || binding.directory != directory
        || file_hash(&std::env::current_exe().map_err(|_| WorkerError("guardian-helper"))?)?
            != binding.helper_sha256
    {
        return Err(WorkerError("guardian-spec"));
    }
    let _lease = HostLease::acquire(&directory.join("keeper-lock"))
        .map_err(|_| WorkerError("guardian-lease"))?
        .ok_or(WorkerError("guardian-lease"))?;
    let job = JobScope::keeper_open(&binding.scope).map_err(|_| WorkerError("guardian-job"))?;
    // A reused PID cannot become this owner. On failure we close only the
    // pre-bound owned Job and never signal an arbitrary process.
    let owner = HostWatch::open(binding.host_pid, binding.host_created)
        .map_err(|_| WorkerError("guardian-owner"))?;
    write_atomic(&directory.join("ready.json"),&serde_json::to_vec(&json!({"bindingHash":binding.digest(),"scope":binding.scope,"ownerBound":true,"jobHandleVerified":true,"keeperPid":std::process::id()})).map_err(|_|WorkerError("guardian-ready"))?)?;
    let begin = Instant::now();
    let trigger = loop {
        if owner.exited().map_err(|_| WorkerError("guardian-owner"))? {
            break "owner-exited";
        }
        if directory.join("finish").is_file() {
            break "finish";
        }
        if begin.elapsed() >= Duration::from_millis(binding.lifetime_ms) {
            break "deadline";
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // An irreversible admission fence remains on this exact kernel Job. The
    // host cannot resume later and put another process into a proven-empty
    // scope; identified spawning never resets these limits.
    job.seal()
        .map_err(|_| WorkerError("guardian-admission-fence"))?;
    job.terminate()
        .map_err(|_| WorkerError("guardian-terminate"))?;
    let stopping = Instant::now();
    while job.active().map_err(|_| WorkerError("guardian-query"))? != 0 {
        if stopping.elapsed() >= Duration::from_secs(3) {
            return Err(WorkerError("guardian-termination-unconfirmed"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let body = json!({"schemaVersion":1,"scope":binding.scope,"bindingHash":binding.digest(),"localJobEmpty":true,"observedAtMs":now()?,"source":"trusted-rust-job-keeper","admissionFence":"active-process-limit-zero","closureTrigger":trigger,"rootExit":"unknown","streams":"not-observed","remoteOutcome":"not-observed"});
    let proof = json!({"body":body,"authorityHash":content_hash(json!([binding.authority,body]).to_string().as_bytes())});
    let temporary = directory.join("receipt.pending");
    write_new(
        &temporary,
        &serde_json::to_vec(&proof).map_err(|_| WorkerError("guardian-receipt"))?,
    )?;
    if directory.join("receipt.json").exists() {
        return Err(WorkerError("guardian-receipt-collision"));
    }
    fs::rename(temporary, directory.join("receipt.json"))
        .map_err(|_| WorkerError("guardian-receipt"))?;
    Ok(())
}
pub fn read_proof(binding: &GuardianBinding) -> Result<Value> {
    let proof: Value =
        serde_json::from_slice(&read_bounded(&binding.directory.join("receipt.json"))?)
            .map_err(|_| WorkerError("guardian-receipt"))?;
    if !valid_proof(binding, &proof) {
        return Err(WorkerError("guardian-proof-identity"));
    }
    Ok(proof)
}
/// Pure validation used during SQLite restoration; no filesystem/process API.
pub fn valid_proof(binding: &GuardianBinding, proof: &Value) -> bool {
    let body = &proof["body"];
    binding.validate()
        && proof.as_object().is_some_and(|o| o.len() == 2)
        && body.as_object().is_some_and(|o| o.len() == 11)
        && body["schemaVersion"] == 1
        && body["scope"] == binding.scope
        && body["bindingHash"] == binding.digest()
        && body["localJobEmpty"] == true
        && body["source"] == "trusted-rust-job-keeper"
        && matches!(
            body["closureTrigger"].as_str(),
            Some("owner-exited" | "finish" | "deadline")
        )
        && body["admissionFence"] == "active-process-limit-zero"
        && body["rootExit"] == "unknown"
        && body["streams"] == "not-observed"
        && body["remoteOutcome"] == "not-observed"
        && body["observedAtMs"]
            .as_u64()
            .is_some_and(|t| t >= binding.issued_at_ms)
        && proof["authorityHash"]
            == content_hash(json!([binding.authority, body]).to_string().as_bytes())
}
fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    crate::git::paths::reject_links(path).map_err(|_| WorkerError("guardian-path"))?;
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| WorkerError("guardian-file"))?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerError("guardian-file"))?;
    if bytes.len() > 8192 {
        return Err(WorkerError("guardian-budget"));
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| WorkerError("guardian-file"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| WorkerError("guardian-file"))
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("pending");
    write_new(&temporary, bytes)?;
    if path.exists() {
        return Err(WorkerError("guardian-publication-collision"));
    }
    fs::rename(temporary, path).map_err(|_| WorkerError("guardian-publication"))
}
fn file_hash(path: &Path) -> Result<String> {
    crate::git::paths::reject_links(path).map_err(|_| WorkerError("guardian-helper"))?;
    let metadata = fs::metadata(path).map_err(|_| WorkerError("guardian-helper"))?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
        return Err(WorkerError("guardian-helper"));
    }
    Ok(content_hash(
        &fs::read(path).map_err(|_| WorkerError("guardian-helper"))?,
    ))
}
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WorkerError("clock"))?
        .as_millis()
        .try_into()
        .map_err(|_| WorkerError("clock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(root: &Path) -> GuardianBinding {
        let scope = content_hash(b"test-scope");
        GuardianBinding {
            schema_version: 1,
            directory: root.join(format!("orchestrix-supervisor-{scope}")),
            scope,
            authority: content_hash(b"test-authority"),
            host_pid: 1,
            host_created: 123,
            issued_at_ms: 1000,
            lifetime_ms: 300_000,
            helper_sha256: content_hash(b"test-helper"),
        }
    }

    fn proof(binding: &GuardianBinding) -> Value {
        tag(
            binding,
            json!({"schemaVersion":1,"scope":binding.scope,"bindingHash":binding.digest(),"localJobEmpty":true,"observedAtMs":1001,"source":"trusted-rust-job-keeper","admissionFence":"active-process-limit-zero","closureTrigger":"owner-exited","rootExit":"unknown","streams":"not-observed","remoteOutcome":"not-observed"}),
        )
    }

    fn tag(binding: &GuardianBinding, body: Value) -> Value {
        json!({"authorityHash":content_hash(json!([binding.authority,body]).to_string().as_bytes()),"body":body})
    }

    fn temporary(label: &str) -> PathBuf {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-guardian-{label}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn proof_is_bound_to_the_exact_generation_and_closed_local_facts() {
        let original = binding(&std::env::temp_dir());
        let valid = proof(&original);
        assert!(valid_proof(&original, &valid));
        for field in [
            "scope",
            "authority",
            "helper_sha256",
            "host_pid",
            "host_created",
            "directory",
            "issued_at_ms",
        ] {
            let mut different = original.clone();
            match field {
                "scope" => {
                    different.scope = content_hash(b"different-scope");
                    different.directory = original
                        .directory
                        .parent()
                        .unwrap()
                        .join(format!("orchestrix-supervisor-{}", different.scope));
                }
                "authority" => different.authority = content_hash(b"different-authority"),
                "helper_sha256" => different.helper_sha256 = content_hash(b"different-helper"),
                "host_pid" => different.host_pid += 1,
                "host_created" => different.host_created += 1,
                "directory" => {
                    different.directory = original
                        .directory
                        .parent()
                        .unwrap()
                        .join("different-parent")
                        .join(original.directory.file_name().unwrap());
                }
                "issued_at_ms" => different.issued_at_ms += 1,
                _ => unreachable!(),
            }
            assert!(different.validate(), "{field}");
            assert!(!valid_proof(&different, &valid), "{field}");
        }

        // Even a correctly tagged document cannot invent a terminal response,
        // closed streams, an absent-name proof or a scope without its fence.
        for (field, value) in [
            ("schemaVersion", json!(2)),
            ("scope", json!(content_hash(b"other"))),
            ("bindingHash", json!(content_hash(b"other"))),
            ("localJobEmpty", json!(false)),
            ("observedAtMs", json!(999)),
            ("source", json!("provider-message")),
            ("admissionFence", json!("none")),
            ("closureTrigger", json!("job-name-not-found")),
            ("rootExit", json!(0)),
            ("streams", json!("closed")),
            ("remoteOutcome", json!("succeeded")),
        ] {
            let mut body = valid["body"].clone();
            body[field] = value;
            assert!(!valid_proof(&original, &tag(&original, body)), "{field}");
        }
        for field in valid["body"].as_object().unwrap().keys() {
            let mut body = valid["body"].clone();
            body.as_object_mut().unwrap().remove(field);
            assert!(!valid_proof(&original, &tag(&original, body)), "{field}");
        }
        let mut body = valid["body"].clone();
        body["providerFrame"] = json!({"type":"turn/completed"});
        assert!(!valid_proof(&original, &tag(&original, body)));
        let mut outer = valid.clone();
        outer["extra"] = json!(true);
        assert!(!valid_proof(&original, &outer));
        let mut outer = valid.clone();
        outer.as_object_mut().unwrap().remove("authorityHash");
        assert!(!valid_proof(&original, &outer));
        let mut descriptor = serde_json::to_value(&original).unwrap();
        descriptor["extra"] = json!(true);
        assert!(serde_json::from_value::<GuardianBinding>(descriptor).is_err());
    }

    #[test]
    fn receipt_reading_rejects_missing_invalid_oversized_and_mismatched_bytes() {
        let root = temporary("bounded");
        let binding = binding(&root);
        #[cfg(windows)]
        named_job::private_directory(&binding.directory).unwrap();
        #[cfg(not(windows))]
        fs::create_dir(&binding.directory).unwrap();
        let receipt = binding.directory.join("receipt.json");
        assert_eq!(read_proof(&binding).unwrap_err().0, "guardian-file");
        fs::write(&receipt, b"{").unwrap();
        assert_eq!(read_proof(&binding).unwrap_err().0, "guardian-receipt");
        fs::write(&receipt, vec![b' '; 8193]).unwrap();
        assert_eq!(read_proof(&binding).unwrap_err().0, "guardian-budget");
        let valid = proof(&binding);
        fs::write(&receipt, serde_json::to_vec(&valid).unwrap()).unwrap();
        assert_eq!(read_proof(&binding).unwrap(), valid);
        let mut mismatched = valid;
        mismatched["authorityHash"] = json!(content_hash(b"wrong-authority"));
        fs::write(&receipt, serde_json::to_vec(&mismatched).unwrap()).unwrap();
        assert_eq!(
            read_proof(&binding).unwrap_err().0,
            "guardian-proof-identity"
        );
        fs::remove_file(receipt).unwrap();
        fs::remove_dir(&binding.directory).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn receipt_reading_rejects_a_junction_before_reading_otherwise_valid_proof() {
        use std::os::windows::process::CommandExt;
        let root = temporary("junction");
        let binding = binding(&root);
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let valid = proof(&binding);
        fs::write(
            target.join("receipt.json"),
            serde_json::to_vec(&valid).unwrap(),
        )
        .unwrap();
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        // Only this owned test fixture uses a shell; production receipt access
        // never executes one. Arguments are passed through environment values.
        let output = Command::new(shell)
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:ORCHESTRIX_TEST_JUNCTION -Target $env:ORCHESTRIX_TEST_TARGET | Out-Null"])
            .env("ORCHESTRIX_TEST_JUNCTION", crate::git::paths::git_path(&binding.directory).unwrap())
            .env("ORCHESTRIX_TEST_TARGET", crate::git::paths::git_path(&target).unwrap())
            .current_dir(&root)
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(read_proof(&binding).unwrap_err().0, "guardian-path");
        // Remove only the junction itself, then direct owned children; never
        // recursively follow the reparse point or an unchecked cleanup path.
        fs::remove_dir(&binding.directory).unwrap();
        fs::remove_file(target.join("receipt.json")).unwrap();
        fs::remove_dir(target).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
