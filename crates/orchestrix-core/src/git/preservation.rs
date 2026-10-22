//! Opt-in, exact mixed-content bases. Only explicitly editable UTF-8 files may
//! become artifact content; preserved blobs remain hashes and native Git OIDs.
use super::{GitError, GitExecutor, GitResult, capture, paths, preflight, worktrees::valid_oid};
use crate::{
    git_records::{
        BasePreservation, MAX_BASE_BYTES, MAX_BASE_FILES, MAX_CAPTURE_BYTES, MAX_EDITABLE_FILES,
        MAX_FILE_BYTES, MAX_OPAQUE_FILE_BYTES, MAX_PRESERVATION_BYTES, PreservedBaseEntry,
        content_hash,
    },
    runtime::CancellationToken,
};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

const BATCH: usize = 7 * 1_048_576;
fn args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}
pub fn plan_base_preservation(
    root: &Path,
    base: &str,
    editable_paths: &[String],
    cancel: &CancellationToken,
) -> GitResult<BasePreservation> {
    let started = Instant::now();
    let root = paths::canonical_directory(root)?;
    let executor =
        GitExecutor::new()?.with_read_budget(started, Duration::from_secs(90), cancel.clone());
    let manifest = plan(&executor, &root, base, editable_paths)?;
    if cancel.is_cancelled() {
        return Err(GitError::Pending);
    }
    Ok(manifest)
}

pub(super) fn plan(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
    editable_paths: &[String],
) -> GitResult<BasePreservation> {
    executor.ensure_supported(root)?;
    if !valid_oid(base) || editable_paths.is_empty() || editable_paths.len() > MAX_EDITABLE_FILES {
        return Err(GitError::Base);
    }
    let mut editable_paths = editable_paths.to_vec();
    editable_paths.sort();
    let mut aliases = BTreeSet::new();
    for path in &editable_paths {
        if !preflight::source_path(path, &mut aliases) || capture::exclusion(path).is_some() {
            return Err(GitError::Unsupported);
        }
    }
    let tree_oid = super::object_identity::base_tree(executor, root, base)?;
    if !valid_oid(&tree_oid) {
        return Err(GitError::Base);
    }
    let tree = executor
        .run(
            root,
            &args(&["ls-tree", "-r", "-l", "-z", "--full-tree", base]),
        )?
        .checked()?;
    let mut entries = Vec::new();
    let mut aliases = BTreeSet::new();
    let mut total = 0usize;
    let mut editable_bytes = 0usize;
    for raw in tree.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let row = std::str::from_utf8(raw).map_err(|_| GitError::Unsupported)?;
        let (meta, path) = row.split_once('\t').ok_or(GitError::Unsupported)?;
        let fields = meta.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 4
            || fields[0] != "100644"
            || fields[1] != "blob"
            || !valid_oid(fields[2])
            || !preflight::source_path(path, &mut aliases)
            || capture::exclusion(path).is_some()
        {
            return Err(GitError::Unsupported);
        }
        let size: usize = fields[3].parse().map_err(|_| GitError::Unsupported)?;
        if editable_paths
            .binary_search_by(|p| p.as_str().cmp(path))
            .is_ok()
        {
            editable_bytes = editable_bytes.checked_add(size).ok_or(GitError::Limit)?;
            if editable_bytes > MAX_CAPTURE_BYTES {
                return Err(GitError::Limit);
            }
        }
        total = total.checked_add(size).ok_or(GitError::Limit)?;
        if entries.len() >= MAX_BASE_FILES
            || total as u64 > MAX_BASE_BYTES
            || size
                > if editable_paths
                    .binary_search_by(|p| p.as_str().cmp(path))
                    .is_ok()
                {
                    MAX_FILE_BYTES
                } else {
                    MAX_OPAQUE_FILE_BYTES as usize
                }
        {
            return Err(GitError::Limit);
        }
        entries.push(PreservedBaseEntry {
            path: path.into(),
            mode: "100644".into(),
            size: size as u64,
            git_oid: fields[2].into(),
            sha256: String::new(),
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    super::object_identity::inventory_tree(&tree_oid, &entries)?;
    let paths = entries
        .iter()
        .map(|e| e.path.clone())
        .chain(editable_paths.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    capture::profile_attributes(executor, root, &paths, Some(base))?;
    capture::profile_attributes(executor, root, &paths, None)?;
    let hashes = visit(executor, root, &entries, |entry, bytes| {
        if editable_paths.binary_search(&entry.path).is_ok()
            && preflight::content_issue(bytes)?.is_some()
        {
            return Err(GitError::SecretContent);
        }
        Ok(())
    })?;
    for (entry, hash) in entries.iter_mut().zip(hashes) {
        entry.sha256 = hash;
    }
    let manifest = BasePreservation {
        schema_version: 2,
        base_oid: base.into(),
        tree_oid,
        entries,
        editable_paths,
    };
    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|_| GitError::Limit)?
        .len();
    if manifest_bytes > MAX_PRESERVATION_BYTES
        || manifest_bytes.saturating_add(editable_bytes) > MAX_CAPTURE_BYTES
        || !manifest.validate()
    {
        return Err(GitError::Limit);
    }
    Ok(manifest)
}

fn visit(
    executor: &GitExecutor,
    root: &Path,
    entries: &[PreservedBaseEntry],
    mut consume: impl FnMut(&PreservedBaseEntry, &[u8]) -> GitResult<()>,
) -> GitResult<Vec<String>> {
    let mut hashes = Vec::with_capacity(entries.len());
    let mut position = 0;
    while position < entries.len() {
        let mut end = position;
        let mut bytes = 0usize;
        while end < entries.len() && bytes + entries[end].size as usize + 128 <= BATCH {
            bytes += entries[end].size as usize + 128;
            end += 1;
        }
        if end == position {
            return Err(GitError::Limit);
        }
        let input = entries[position..end]
            .iter()
            .map(|e| format!("{}\n", e.git_oid))
            .collect::<String>();
        let response = executor
            .run_with_input(root, &args(&["cat-file", "--batch"]), input.as_bytes())?
            .checked()?;
        let mut rest = response.as_slice();
        for entry in &entries[position..end] {
            let newline = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or(GitError::InconsistentCapture)?;
            if &rest[..newline] != format!("{} blob {}", entry.git_oid, entry.size).as_bytes() {
                return Err(GitError::InconsistentCapture);
            }
            rest = &rest[newline + 1..];
            let size = entry.size as usize;
            if rest.len() < size + 1 || rest[size] != b'\n' {
                return Err(GitError::InconsistentCapture);
            }
            let bytes = &rest[..size];
            super::object_identity::verify("blob", &entry.git_oid, bytes)?;
            let hash = content_hash(bytes);
            if !entry.sha256.is_empty() && hash != entry.sha256 {
                return Err(GitError::Ownership);
            }
            consume(entry, bytes)?;
            hashes.push(hash);
            rest = &rest[size + 1..];
        }
        if !rest.is_empty() {
            return Err(GitError::InconsistentCapture);
        }
        position = end;
    }
    Ok(hashes)
}

pub(super) fn verify_base(
    executor: &GitExecutor,
    root: &Path,
    manifest: &BasePreservation,
) -> GitResult<()> {
    if !manifest.validate() {
        return Err(GitError::Ownership);
    }
    let observed = plan(executor, root, &manifest.base_oid, &manifest.editable_paths)?;
    if observed != *manifest {
        return Err(GitError::Ownership);
    }
    Ok(())
}

pub(super) fn for_tree(
    executor: &GitExecutor,
    root: &Path,
    manifest: &BasePreservation,
    base: &str,
) -> GitResult<BasePreservation> {
    let observed = plan(executor, root, base, &manifest.editable_paths)?;
    let unchanged = |m: &BasePreservation| {
        m.entries
            .iter()
            .filter(|e| m.editable_paths.binary_search(&e.path).is_err())
            .cloned()
            .collect::<Vec<_>>()
    };
    if unchanged(&observed) != unchanged(manifest) {
        return Err(GitError::Ownership);
    }
    Ok(observed)
}

pub(super) fn materialize(
    executor: &GitExecutor,
    root: &Path,
    target: &Path,
    manifest: &BasePreservation,
    fault_after_first: bool,
) -> GitResult<()> {
    if !manifest.validate() {
        return Err(GitError::Ownership);
    }
    visit(executor, root, &manifest.entries, |entry, bytes| {
        let path = target.join(&entry.path);
        paths::reject_links(&path)?;
        let relative = path
            .parent()
            .ok_or(GitError::Path)?
            .strip_prefix(target)
            .map_err(|_| GitError::Scope)?;
        let mut directory = target.to_path_buf();
        for component in relative.components() {
            directory.push(component);
            paths::reject_links(&directory)?;
            match fs::create_dir(&directory) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && directory.is_dir() => {}
                Err(_) => return Err(GitError::Collision),
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    GitError::Collision
                } else {
                    GitError::Io
                }
            })?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if fault_after_first {
            return Err(GitError::Injected);
        }
        Ok(())
    })?;
    Ok(())
}
