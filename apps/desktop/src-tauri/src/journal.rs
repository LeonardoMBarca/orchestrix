//! Exact authorized host effects survive a lost reply or a window restart.
use orchestrix_core::{
    daemon::Operation, git_records::content_hash, persistence_privacy::sanitize_json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub schema_version: u32,
    pub request_hash: String,
    pub operation: Operation,
    pub binding: Option<serde_json::Value>,
    pub worker_id: Option<String>,
    pub runtime_sha256: Option<String>,
    pub correction_budget: Option<u32>,
}
fn path(directory: &Path, command: &str) -> PathBuf {
    directory.join(format!(
        "desktop-command-{}.json",
        content_hash(command.as_bytes())
    ))
}
pub fn read(directory: &Path, command: &str, request_hash: &str) -> Result<Option<Effect>, String> {
    let file = path(directory, command);
    let bytes = match fs::read(&file) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("desktop-command-read".into()),
    };
    if bytes.len() > 1_048_576 {
        return Err("desktop-command-budget".into());
    }
    let effect: Effect = serde_json::from_slice(&bytes).map_err(|_| "desktop-command-schema")?;
    if effect.schema_version != 1 || effect.request_hash != request_hash {
        return Err("desktop-command-id-conflict".into());
    }
    Ok(Some(effect))
}
pub fn reserve(directory: &Path, command: &str, effect: &Effect) -> Result<Effect, String> {
    if let Some(old) = read(directory, command, &effect.request_hash)? {
        return Ok(old);
    }
    let value = serde_json::to_value(effect).map_err(|_| "desktop-command-schema")?;
    if sanitize_json(&value).map_err(|_| "desktop-command-privacy")? != value {
        return Err("desktop-command-privacy".into());
    }
    let bytes = serde_json::to_vec(effect).map_err(|_| "desktop-command-schema")?;
    if bytes.len() > 1_048_576 {
        return Err("desktop-command-budget".into());
    }
    let final_path = path(directory, command);
    let temporary = directory.join(format!(
        "desktop-command-pending-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| "desktop-command-write")?;
    file.write_all(&bytes)
        .map_err(|_| "desktop-command-write")?;
    file.sync_all().map_err(|_| "desktop-command-write")?;
    drop(file);
    // Hard-link is an atomic no-replace publication of complete bytes. Racing
    // windows inspect the winner; no partially written record authorizes effects.
    let published = fs::hard_link(&temporary, &final_path);
    let _ = fs::remove_file(&temporary);
    if published.is_err() && !final_path.is_file() {
        return Err("desktop-command-publication".into());
    }
    read(directory, command, &effect.request_hash)?
        .ok_or_else(|| "desktop-command-publication".into())
}
pub fn correction_budget(directory: &Path, worker: &str) -> Result<Option<u32>, String> {
    let mut budget = None;
    let mut count = 0;
    for entry in fs::read_dir(directory).map_err(|_| "desktop-command-read")? {
        let entry = entry.map_err(|_| "desktop-command-read")?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("desktop-command-") || !name.ends_with(".json") {
            continue;
        }
        count += 1;
        if count > 4096 {
            return Err("desktop-command-index-budget".into());
        }
        let bytes = fs::read(entry.path()).map_err(|_| "desktop-command-read")?;
        if bytes.len() > 1_048_576 {
            return Err("desktop-command-budget".into());
        }
        let effect: Effect =
            serde_json::from_slice(&bytes).map_err(|_| "desktop-command-schema")?;
        if effect.worker_id.as_deref() == Some(worker) {
            if let Some(value) = effect.correction_budget {
                if budget.is_some_and(|old| old != value) {
                    return Err("correction-budget-conflict".into());
                }
                budget = Some(value);
            }
        }
    }
    Ok(budget)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lost_reply_reopen_preserves_exact_effect_and_refuses_changed_body() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchestrix-command-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let effect = Effect {
            schema_version: 1,
            request_hash: content_hash(b"original"),
            operation: Operation::Resume {
                command_id: "same".into(),
            },
            binding: None,
            worker_id: Some("worker-zero".into()),
            runtime_sha256: None,
            correction_budget: Some(0),
        };
        reserve(&root, "same", &effect).unwrap();
        assert_eq!(correction_budget(&root, "unknown").unwrap(), None);
        assert_eq!(correction_budget(&root, "worker-zero").unwrap(), Some(0));
        drop(super::read(&root, "same", &effect.request_hash).unwrap());
        assert_eq!(correction_budget(&root, "worker-zero").unwrap(), Some(0));
        let read = read(&root, "same", &effect.request_hash).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(read.operation).unwrap(),
            serde_json::to_value(effect.operation).unwrap()
        );
        assert!(super::read(&root, "same", &content_hash(b"changed")).is_err());
        let forbidden = Effect {
            schema_version: 1,
            request_hash: content_hash(b"other"),
            operation: Operation::Resume {
                command_id: "secret".into(),
            },
            binding: Some(serde_json::json!({"apiKey":"sk-not-permitted"})),
            worker_id: None,
            runtime_sha256: None,
            correction_budget: None,
        };
        assert!(reserve(&root, "secret", &forbidden).is_err());
        assert_eq!(
            root.parent().unwrap(),
            std::env::temp_dir().canonicalize().unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
