//! Advisory, bounded inspection of the complete committed inventory. No refs,
//! worktrees, index, provider or database are modified. Effects revalidate.
use super::worktrees::valid_oid;
use super::{GitError, GitExecutor, GitResult, capture, paths};
use crate::{
    git_records::{MAX_FILE_BYTES, relative_file_path},
    persistence_privacy::sanitize_text,
    runtime::CancellationToken,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::Path,
    time::{Duration, Instant},
};

const FILES: usize = super::SOURCE_FILES;
const READ_BYTES: usize = super::SOURCE_BYTES;
const BATCH_BYTES: usize = 7 * MAX_FILE_BYTES;
const INSPECTION_SECONDS: u64 = 90;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    pub files: usize,
    pub bytes: u64,
    pub largest_file_bytes: u64,
    pub regular: usize,
    pub executables: usize,
    pub links: usize,
    pub submodules: usize,
    pub oversized: usize,
    pub binary: usize,
    pub non_utf8: usize,
    pub secret_pattern: usize,
    pub excluded: usize,
    pub content_inspected: usize,
    pub content_skipped: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkout {
    pub clean: bool,
    pub status: String,
    pub truncated: bool,
    pub ignored: usize,
    pub untracked: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Blocker {
    pub code: String,
    pub count: usize,
    pub examples: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub files: usize,
    pub file_bytes: usize,
    pub read_bytes: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryPreflight {
    pub schema_version: u32,
    pub profile: String,
    pub base_oid: String,
    pub head_oid: String,
    pub compatible: bool,
    pub application_eligible: bool,
    pub complete: bool,
    pub inventory: Inventory,
    pub checkout: Checkout,
    pub blockers: Vec<Blocker>,
    pub limits: Limits,
    pub model_called: bool,
    pub authority: String,
}
pub(super) fn source_path(path: &str, aliases: &mut BTreeSet<String>) -> bool {
    relative_file_path(path)
        && !path
            .chars()
            .any(|c| c.is_control() || "<>\"|?*".contains(c))
        && aliases.insert(path.to_lowercase())
}
pub(super) fn content_issue(bytes: &[u8]) -> GitResult<Option<&'static str>> {
    if bytes.contains(&0) {
        return Ok(Some("binary-content"));
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Ok(Some("non-utf8"));
    };
    if sanitize_text(text).map_err(|_| GitError::Limit)? != text {
        return Ok(Some("secret-pattern"));
    }
    Ok(None)
}
impl RepositoryPreflight {
    fn block(&mut self, code: &str, path: Option<&str>) {
        let index = self
            .blockers
            .iter()
            .position(|b| b.code == code)
            .unwrap_or_else(|| {
                self.blockers.push(Blocker {
                    code: code.into(),
                    count: 0,
                    examples: Vec::new(),
                });
                self.blockers.len() - 1
            });
        let item = &mut self.blockers[index];
        item.count += 1;
        if item.examples.len() < 3
            && let Some(path) = path
        {
            // Never return content; paths are sanitized and bounded too.
            let short = path.chars().take(180).collect::<String>();
            item.examples
                .push(sanitize_text(&short).unwrap_or_else(|_| "[redacted path]".into()));
        }
    }
}
fn args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}
fn checkpoint(cancel: &CancellationToken, start: Instant) -> GitResult<()> {
    if cancel.is_cancelled() {
        return Err(GitError::Pending);
    }
    if start.elapsed() >= Duration::from_secs(INSPECTION_SECONDS) {
        return Err(GitError::Deadline);
    }
    Ok(())
}
struct Entry {
    oid: String,
    path: String,
    size: usize,
}

/// `base` is a full object ID, never an arbitrary Git expression. `None` pins
/// current HEAD. Cancellation is observed between bounded Git commands (15 s).
pub fn inspect_repository(
    root: &Path,
    base: Option<&str>,
    cancel: &CancellationToken,
) -> GitResult<RepositoryPreflight> {
    let start = Instant::now();
    let root = paths::canonical_directory(root)?;
    let executor = GitExecutor::new()?.with_read_budget(
        start,
        Duration::from_secs(INSPECTION_SECONDS),
        cancel.clone(),
    );
    executor.ensure_supported(&root)?;
    let head = executor.inspect_head(&root)?;
    let base = base.unwrap_or(&head);
    if !valid_oid(base) {
        return Err(GitError::Base);
    }
    let mut report = RepositoryPreflight {
        schema_version: 1,
        profile: "regular-utf8-v1".into(),
        base_oid: base.into(),
        head_oid: head.clone(),
        compatible: false,
        application_eligible: false,
        complete: true,
        inventory: Inventory::default(),
        checkout: Checkout::default(),
        blockers: Vec::new(),
        limits: Limits {
            files: FILES,
            file_bytes: MAX_FILE_BYTES,
            read_bytes: READ_BYTES,
        },
        model_called: false,
        authority: "inspection-only-revalidate-before-effects".into(),
    };
    if base != head {
        report.block("base-not-head", None);
    }
    checkpoint(cancel, start)?;
    let tree = executor
        .run(
            &root,
            &args(&["ls-tree", "-r", "-l", "-z", "--full-tree", base]),
        )?
        .checked()?;
    let mut aliases = BTreeSet::new();
    let mut entries = Vec::new();
    let mut reserved = 0usize;
    let mut paths = Vec::new();
    for raw in tree.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        checkpoint(cancel, start)?;
        let row = std::str::from_utf8(raw).map_err(|_| GitError::Unsupported)?;
        let (meta, path) = row.split_once('\t').ok_or(GitError::Unsupported)?;
        let fields = meta.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 4 || !valid_oid(fields[2]) {
            return Err(GitError::Unsupported);
        }
        report.inventory.files += 1;
        if !source_path(path, &mut aliases) {
            report.block("unsupported-path", Some(path));
        } else if paths.len() < FILES {
            paths.push(path.to_owned());
        }
        match fields[0] {
            "100644" => report.inventory.regular += 1,
            "100755" => {
                report.inventory.executables += 1;
                report.block("executable-mode", Some(path));
            }
            "120000" => {
                report.inventory.links += 1;
                report.block("symbolic-link", Some(path));
            }
            "160000" => {
                report.inventory.submodules += 1;
                report.block("submodule", Some(path));
            }
            _ => report.block("unsupported-mode", Some(path)),
        }
        if capture::exclusion(path).is_some() {
            report.inventory.excluded += 1;
            report.block("excluded-content", Some(path));
        }
        if fields[1] != "blob" {
            report.inventory.content_skipped += 1;
            continue;
        }
        let size: usize = fields[3].parse().map_err(|_| GitError::Unsupported)?;
        report.inventory.bytes = report
            .inventory
            .bytes
            .checked_add(size as u64)
            .ok_or(GitError::Limit)?;
        report.inventory.largest_file_bytes = report.inventory.largest_file_bytes.max(size as u64);
        if size > MAX_FILE_BYTES {
            report.inventory.oversized += 1;
            report.inventory.content_skipped += 1;
            report.block("file-size-limit", Some(path));
            continue;
        }
        if report.inventory.files > FILES || reserved.saturating_add(size) > READ_BYTES {
            report.inventory.content_skipped += 1;
            report.complete = false;
            continue;
        }
        reserved += size;
        entries.push(Entry {
            oid: fields[2].into(),
            path: path.into(),
            size,
        });
    }
    if report.inventory.files > FILES {
        report.block("inventory-limit", None);
        report.complete = false;
    }
    if report.inventory.bytes > READ_BYTES as u64 {
        report.block("read-bytes-limit", None);
    }
    match capture::profile_attributes(&executor, &root, &paths, Some(base)) {
        Ok(_) => {}
        Err(GitError::Unsupported) => report.block("unsupported-base-attributes", None),
        Err(GitError::Process) => {
            report.block("base-attribute-inspection-unavailable", None);
            report.complete = false;
        }
        Err(error) => return Err(error),
    }
    match capture::profile_attributes(&executor, &root, &paths, None) {
        Ok(_) => {}
        Err(GitError::Unsupported) => report.block("unsupported-attributes", None),
        Err(error) => return Err(error),
    }
    // Batch raw object reads, not checkout/filter output. Bound each response
    // below GitExecutor's 8 MiB output limit, including framing overhead.
    let mut position = 0;
    let mut base_hashes = BTreeMap::new();
    while position < entries.len() {
        checkpoint(cancel, start)?;
        let mut end = position;
        let mut bytes = 0usize;
        while end < entries.len() && bytes + entries[end].size + 128 <= BATCH_BYTES {
            bytes += entries[end].size + 128;
            end += 1;
        }
        let input = entries[position..end]
            .iter()
            .map(|e| format!("{}\n", e.oid))
            .collect::<String>();
        let response = executor
            .run_with_input(&root, &args(&["cat-file", "--batch"]), input.as_bytes())?
            .checked()?;
        let mut rest = response.as_slice();
        for entry in &entries[position..end] {
            let newline = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or(GitError::InconsistentCapture)?;
            if &rest[..newline] != format!("{} blob {}", entry.oid, entry.size).as_bytes() {
                return Err(GitError::InconsistentCapture);
            }
            rest = &rest[newline + 1..];
            if rest.len() < entry.size + 1 || rest[entry.size] != b'\n' {
                return Err(GitError::InconsistentCapture);
            }
            if let Some(code) = content_issue(&rest[..entry.size])? {
                match code {
                    "binary-content" => report.inventory.binary += 1,
                    "non-utf8" => report.inventory.non_utf8 += 1,
                    _ => report.inventory.secret_pattern += 1,
                };
                report.block(code, Some(&entry.path));
            }
            report.inventory.content_inspected += 1;
            base_hashes.insert(
                entry.path.clone(),
                crate::git_records::content_hash(&rest[..entry.size]),
            );
            rest = &rest[entry.size + 1..];
        }
        if !rest.is_empty() {
            return Err(GitError::InconsistentCapture);
        }
        position = end;
    }
    checkpoint(cancel, start)?;
    let status = executor
        .run(
            &root,
            &args(&["status", "--porcelain=v1", "-z", "--untracked-files=normal"]),
        )?
        .checked()?;
    let status_text = String::from_utf8_lossy(&status).replace('\0', "\n");
    let bounded = status_text.chars().take(8192).collect::<String>();
    let ignored = executor
        .run(
            &root,
            &args(&[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
                "-z",
            ]),
        )?
        .checked()?;
    let untracked = executor
        .run(
            &root,
            &args(&["ls-files", "--others", "--exclude-standard", "-z"]),
        )?
        .checked()?;
    report.checkout = Checkout {
        clean: status.is_empty(),
        truncated: bounded.len() != status_text.len(),
        status: sanitize_text(&bounded).map_err(|_| GitError::Limit)?,
        ignored: ignored.split(|b| *b == 0).filter(|s| !s.is_empty()).count(),
        untracked: untracked
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .count(),
    };
    checkpoint(cancel, start)?;
    let mut no_omissions = false;
    if report.blockers.is_empty() && report.complete {
        match capture::inspect_preparation(&executor, &root, base, &base_hashes) {
            Ok((value, clean)) => {
                report.compatible = true;
                no_omissions = value;
                report.checkout.clean &= clean;
            }
            Err(error) => {
                report.block(
                    &format!(
                        "capture-{}",
                        error.to_string().rsplit(": ").next().unwrap_or("rejected")
                    ),
                    None,
                );
                report.complete = false;
            }
        }
    }
    checkpoint(cancel, start)?;
    if executor
        .run(
            &root,
            &args(&["status", "--porcelain=v1", "-z", "--untracked-files=normal"]),
        )?
        .checked()?
        != status
        || executor
            .run(
                &root,
                &args(&[
                    "ls-files",
                    "--others",
                    "--ignored",
                    "--exclude-standard",
                    "--directory",
                    "-z",
                ]),
            )?
            .checked()?
            != ignored
        || executor
            .run(
                &root,
                &args(&["ls-files", "--others", "--exclude-standard", "-z"]),
            )?
            .checked()?
            != untracked
    {
        report.complete = false;
        report.compatible = false;
        report.block("checkout-changed", None);
    }
    if executor.inspect_head(&root)? != head {
        report.complete = false;
        report.compatible = false;
        report.block("head-changed", None);
    }
    report.application_eligible =
        report.compatible && no_omissions && report.checkout.clean && report.checkout.ignored == 0;
    // Fixed blocker kinds + bounded examples and status keep the wire response
    // small independently of repository content and file count.
    if serde_json::to_vec(&report)
        .map_err(|_| GitError::Limit)?
        .len()
        > 65_536
    {
        return Err(GitError::OutputLimit);
    }
    Ok(report)
}
