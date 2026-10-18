//! Bounded native runtime selection. No authentication file is opened and no
//! executable runs during discovery. Candidate IDs authorize only this flow.
use orchestrix_process::{OwnedProcess, ProcessEvent, ProcessLimits, SpawnSpec};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_CANDIDATES: usize = 16;
const MAX_PATH_ENTRIES: usize = 64;
// Parser compatibility in Core is broader than the production profile accepted
// for this Desktop increment. Older harness compatibility is not a live gate.
pub const SUPPORTED_VERSIONS: &[&str] = &["0.162.0-alpha.17.2"];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub kind: String,
    pub location: String,
    pub source: String,
}
#[derive(Default)]
pub struct Discovery {
    choices: BTreeMap<String, (String, PathBuf)>,
}
impl Discovery {
    pub fn detect(&mut self, linked: &[(PathBuf, PathBuf)]) -> Vec<Candidate> {
        let mut paths = Vec::new();
        for (executable, home) in linked.iter().take(MAX_CANDIDATES) {
            paths.push(("executable", executable.clone(), "linked-runtime"));
            paths.push(("home", home.clone(), "linked-home"));
        }
        if let Some(value) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&value).take(MAX_PATH_ENTRIES) {
                if directory.is_absolute() {
                    paths.push(("executable", directory.join("codex.exe"), "path"));
                }
            }
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let base = PathBuf::from(appdata);
            if base.is_absolute() {
                for suffix in [
                    "npm/node_modules/@openai/codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
                    "npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
                ] {
                    paths.push(("executable", base.join(suffix), "standard-install"));
                }
            }
        }
        if let Some(home) = std::env::var_os("CODEX_HOME") {
            paths.push(("home", PathBuf::from(home), "configured-home"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            paths.push((
                "home",
                PathBuf::from(profile).join(".codex"),
                "standard-home",
            ));
        }
        self.retain_candidates(paths)
    }
    #[cfg(feature = "test-fixtures")]
    pub fn detect_fixture(&mut self, executable: PathBuf, home: PathBuf) -> Vec<Candidate> {
        self.retain_candidates(vec![
            ("executable", executable, "synthetic-fixture"),
            ("home", home, "synthetic-fixture"),
        ])
    }
    fn retain_candidates(&mut self, paths: Vec<(&str, PathBuf, &str)>) -> Vec<Candidate> {
        self.choices.clear();
        let mut seen = BTreeSet::new();
        let mut result = Vec::new();
        let generation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (kind, path, source) in paths {
            if result.len() >= MAX_CANDIDATES || !path.is_absolute() {
                continue;
            }
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink()
                || (kind == "home" && !metadata.is_dir())
                || (kind == "executable"
                    && (!metadata.is_file()
                        || path
                            .extension()
                            .is_none_or(|e| !e.eq_ignore_ascii_case("exe"))))
            {
                continue;
            }
            let Ok(path) = path.canonicalize() else {
                continue;
            };
            if !seen.insert((kind.to_owned(), path.to_string_lossy().to_lowercase())) {
                continue;
            }
            let digest = orchestrix_core::git_records::content_hash(
                format!("{generation}|{kind}|{}", path.display()).as_bytes(),
            );
            let id = format!("native-candidate-{}", &digest[..32]);
            self.choices.insert(id.clone(), (kind.into(), path.clone()));
            result.push(Candidate {
                id,
                kind: kind.into(),
                location: path.to_string_lossy().into(),
                source: source.into(),
            });
        }
        result
    }
    pub fn resolve(&self, id: &str, kind: &str) -> Result<PathBuf, String> {
        let (actual, path) = self.choices.get(id).ok_or("native-candidate-expired")?;
        if actual != kind {
            return Err("native-candidate-kind".into());
        }
        let current = path
            .canonicalize()
            .map_err(|_| "native-candidate-unavailable")?;
        if &current != path {
            return Err("native-candidate-changed".into());
        }
        Ok(current)
    }
}

pub fn binary_hash(path: &Path) -> Result<String, String> {
    orchestrix_core::connections::runtime_executable_sha256(path)
}

pub fn version(executable: PathBuf, cwd: PathBuf) -> Result<serde_json::Value, String> {
    read_version(executable, cwd, None)
}
#[cfg(feature = "test-fixtures")]
pub fn synthetic_version(executable: PathBuf, cwd: PathBuf) -> Result<serde_json::Value, String> {
    let mut result = read_version(executable, cwd, Some("connection-metadata"))?;
    if result["version"] != "synthetic-rust-1" {
        return Err("fixture-runtime-version".into());
    }
    result["supported"] = serde_json::json!(true);
    result["syntheticQa"] = serde_json::json!(true);
    Ok(result)
}
fn read_version(
    executable: PathBuf,
    cwd: PathBuf,
    scenario: Option<&str>,
) -> Result<serde_json::Value, String> {
    let before = binary_hash(&executable)?;
    let environment = std::env::vars_os()
        .filter(|(key, _)| {
            ["SYSTEMROOT", "WINDIR", "SYSTEMDRIVE", "TEMP", "TMP"]
                .iter()
                .any(|name| key.to_string_lossy().eq_ignore_ascii_case(name))
        })
        .collect();
    let mut process = OwnedProcess::spawn(SpawnSpec {
        executable: executable.clone(),
        arguments: if let Some(scenario) = scenario {
            vec!["--version".into(), "--scenario".into(), scenario.into()]
        } else {
            vec!["--version".into()]
        },
        cwd,
        environment,
        limits: ProcessLimits {
            lifetime: Duration::from_secs(10),
            max_output_bytes: 8192,
            max_write_bytes: 1024,
        },
    })
    .map_err(|_| "runtime-version-process")?;
    process.close_input();
    let start = Instant::now();
    let mut output = Vec::new();
    loop {
        if let Some(ProcessEvent::Stdout(bytes)) = process
            .receive(Duration::from_millis(5))
            .map_err(|_| "runtime-version-process")?
        {
            output.extend(bytes);
        }
        let tree = process.tree().map_err(|_| "runtime-version-ownership")?;
        let streams = process.streams();
        if tree.root_exit_code.is_some() && streams.stdout_closed && streams.stderr_closed {
            while let Some(event) = process
                .receive(Duration::ZERO)
                .map_err(|_| "runtime-version-process")?
            {
                if let ProcessEvent::Stdout(bytes) = event {
                    output.extend(bytes);
                }
            }
            if tree.root_exit_code != Some(0) {
                return Err("runtime-version-exit".into());
            }
            let stopped = if tree.active_processes == 0 {
                tree
            } else {
                process
                    .terminate(Duration::from_secs(3))
                    .map_err(|_| "runtime-version-ownership")?
            };
            if !stopped.owned || stopped.active_processes != 0 {
                return Err("runtime-version-ownership".into());
            }
            break;
        }
        if start.elapsed() >= Duration::from_secs(10) {
            return Err("runtime-version-timeout".into());
        }
    }
    let output = String::from_utf8(output).map_err(|_| "runtime-version-format")?;
    let version = parse_version(&output).ok_or("runtime-version-format")?;
    if binary_hash(&executable)? != before {
        return Err("runtime-executable-changed".into());
    }
    Ok(
        serde_json::json!({"version":version,"executableSha256":before,"supported":SUPPORTED_VERSIONS.contains(&version),"inferenceUsed":false,"authenticationRead":false}),
    )
}
fn parse_version(output: &str) -> Option<&str> {
    let value = output.trim().strip_prefix("codex-cli ")?;
    (!value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b)))
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_strict_and_never_returns_unexpected_output() {
        assert_eq!(
            parse_version("codex-cli 0.162.0-alpha.17.2\r\n"),
            Some("0.162.0-alpha.17.2")
        );
        assert_eq!(parse_version("codex-cli 1\ncredential-data"), None);
        assert_eq!(parse_version("different-tool 1"), None);
    }
    #[test]
    fn retained_candidates_expire_and_kind_is_enforced() {
        let home = std::env::temp_dir().canonicalize().unwrap();
        let mut discovery = Discovery::default();
        let found = discovery.retain_candidates(vec![
            ("home", home.clone(), "test"),
            ("home", home.clone(), "duplicate"),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(discovery.resolve(&found[0].id, "home").unwrap(), home);
        assert!(discovery.resolve(&found[0].id, "executable").is_err());
        discovery.retain_candidates(Vec::new());
        assert!(discovery.resolve(&found[0].id, "home").is_err());
    }
}
