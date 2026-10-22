//! Advisory, bounded inspection of the complete committed inventory. No refs,
//! worktrees, index, provider or database are modified. Effects revalidate.
use super::worktrees::valid_oid;
use super::{GitError, GitExecutor, GitResult, capture, paths};
use crate::{
    git_records::{
        BasePreservation, MAX_EDITABLE_FILES, MAX_FILE_BYTES, MAX_OPAQUE_FILE_BYTES,
        PreservedBaseEntry, relative_file_path,
    },
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
const BATCH_BYTES: usize = 7 * 1_048_576;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preservation: Option<PreservationSummary>,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreservationSummary {
    pub base_files: usize,
    pub base_bytes: u64,
    pub preserved_files: usize,
    pub preserved_bytes: u64,
    pub editable_paths: Vec<String>,
    pub selection_required: bool,
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
    inspect_repository_profile(root, base, cancel, None)
}
pub fn inspect_repository_with_editable(
    root: &Path,
    base: Option<&str>,
    editable_paths: &[String],
    cancel: &CancellationToken,
) -> GitResult<RepositoryPreflight> {
    inspect_repository_profile(root, base, cancel, Some(editable_paths))
}
fn inspect_repository_profile(
    root: &Path,
    base: Option<&str>,
    cancel: &CancellationToken,
    editable_paths: Option<&[String]>,
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
    let mut selected = editable_paths.unwrap_or_default().to_vec();
    selected.sort();
    let mut selected_aliases = BTreeSet::new();
    let selected_valid = selected.len() <= MAX_EDITABLE_FILES
        && selected
            .iter()
            .all(|p| source_path(p, &mut selected_aliases) && capture::exclusion(p).is_none());
    let mut report = RepositoryPreflight {
        preservation: editable_paths.map(|_| PreservationSummary {
            base_files: 0,
            base_bytes: 0,
            preserved_files: 0,
            preserved_bytes: 0,
            editable_paths: selected.clone(),
            selection_required: selected.is_empty(),
        }),
        schema_version: if editable_paths.is_some() { 2 } else { 1 },
        profile: if editable_paths.is_some() {
            "opaque-base-v2"
        } else {
            "regular-utf8-v1"
        }
        .into(),
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
    if editable_paths.is_some() {
        if selected.is_empty() {
            report.block("editable-selection-required", None);
        } else if !selected_valid {
            report.block("editable-scope-invalid", None);
        }
    }
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
        let editable = selected.binary_search_by(|p| p.as_str().cmp(path)).is_ok();
        if let Some(summary) = &mut report.preservation {
            summary.base_files += 1;
            summary.base_bytes = summary.base_bytes.saturating_add(size as u64);
            if !editable {
                summary.preserved_files += 1;
                summary.preserved_bytes = summary.preserved_bytes.saturating_add(size as u64);
            }
        }
        if size
            > if editable_paths.is_some() && !editable {
                MAX_OPAQUE_FILE_BYTES as usize
            } else {
                MAX_FILE_BYTES
            }
        {
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
    let mut preservation_entries = Vec::new();
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
            if editable_paths.is_some() {
                super::object_identity::verify("blob", &entry.oid, &rest[..entry.size])?;
            }
            let opaque_large = editable_paths.is_some()
                && selected.binary_search(&entry.path).is_err()
                && entry.size > MAX_FILE_BYTES;
            let issue = if opaque_large {
                if rest[..entry.size].contains(&0) {
                    Some("binary-content")
                } else if std::str::from_utf8(&rest[..entry.size]).is_err() {
                    Some("non-utf8")
                } else {
                    None
                }
            } else {
                content_issue(&rest[..entry.size])?
            };
            if let Some(code) = issue {
                match code {
                    "binary-content" => report.inventory.binary += 1,
                    "non-utf8" => report.inventory.non_utf8 += 1,
                    _ => report.inventory.secret_pattern += 1,
                };
                if editable_paths.is_none() || selected.binary_search(&entry.path).is_ok() {
                    report.block(code, Some(&entry.path));
                }
            }
            report.inventory.content_inspected += 1;
            base_hashes.insert(
                entry.path.clone(),
                crate::git_records::content_hash(&rest[..entry.size]),
            );
            if editable_paths.is_some() {
                preservation_entries.push(PreservedBaseEntry {
                    path: entry.path.clone(),
                    mode: "100644".into(),
                    size: entry.size as u64,
                    git_oid: entry.oid.clone(),
                    sha256: crate::git_records::content_hash(&rest[..entry.size]),
                });
            }
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
        let preservation = if editable_paths.is_some() {
            preservation_entries.sort_by(|a, b| a.path.cmp(&b.path));
            let tree_oid = super::object_identity::base_tree(&executor, &root, base)?;
            super::object_identity::inventory_tree(&tree_oid, &preservation_entries)?;
            Some(BasePreservation {
                schema_version: 2,
                base_oid: base.into(),
                tree_oid,
                entries: preservation_entries,
                editable_paths: selected,
            })
        } else {
            None
        };
        let inspected = if preservation.as_ref().is_some_and(|p| !p.validate()) {
            Err(GitError::Limit)
        } else if preservation.is_some() {
            capture::inspect_preparation_with_preservation(
                &executor,
                &root,
                base,
                &base_hashes,
                preservation.as_ref(),
            )
        } else {
            capture::inspect_preparation(&executor, &root, base, &base_hashes)
        };
        match inspected {
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Explicit, read-only timing of an already isolated complete snapshot.
    /// Never falls back to the developer checkout or accesses a provider.
    #[test]
    #[ignore = "Requires an explicit owned snapshot path for bounded timing"]
    fn measured_owned_snapshot_preflight() {
        let root = std::path::PathBuf::from(
            std::env::var_os("ORCHESTRIX_OPAQUE_TIMING_REPOSITORY")
                .expect("explicit isolated repository required"),
        );
        let base = std::env::var("ORCHESTRIX_OPAQUE_TIMING_COMMIT")
            .expect("explicit pinned full commit required");
        super::super::executor::test_timing::begin();
        let started = Instant::now();
        let result = inspect_repository_with_editable(
            &root,
            Some(&base),
            &["docs/CONTEXT_OWNERSHIP.md".into()],
            &CancellationToken::new(),
        );
        let samples = super::super::executor::test_timing::finish();
        let mut grouped = BTreeMap::<&str, (usize, u128)>::new();
        for sample in &samples {
            let entry = grouped.entry(sample.builtin).or_default();
            entry.0 += 1;
            entry.1 += sample.elapsed_micros;
        }
        println!(
            "OPAQUE_PREFLIGHT_TIMING={}",
            serde_json::json!({"elapsedMs":started.elapsed().as_millis(),"result":format!("{result:?}"),"grouped":grouped,"samples":samples})
        );
        assert!(result.is_ok_and(|report| report.compatible && report.application_eligible));
    }
}
