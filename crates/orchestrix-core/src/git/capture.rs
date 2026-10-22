//! Exact, bounded snapshots of the supported regular UTF-8 file profile.
//! No checkout conversion, clean filter, external diff or project command runs.
#[cfg(all(test, windows))]
use super::executor::test_timing::Call as TimingCall;
use super::{GitError, GitExecutor, GitResult};
use crate::git_records::{
    BasePreservation, CaptureExclusion, CapturedArtifact, CapturedFile, FileContent,
    MAX_CAPTURE_BYTES, MAX_CAPTURE_FILES, MAX_FILE_BYTES, MAX_OPAQUE_FILE_BYTES, WorktreeIntent,
    content_hash, relative_file_path,
};
use crate::persistence_privacy::sanitize_text;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, Metadata},
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Limits apply to inventory/read work as well as the smaller stored artifact.
/// Unsupported or oversized inventories fail; no entry or content is truncated.
#[derive(Clone, Copy, Debug)]
pub struct CaptureLimits {
    pub inventory_files: usize,
    pub read_bytes: usize,
    pub file_bytes: usize,
    pub captured_files: usize,
    pub artifact_bytes: usize,
    pub patch_bytes: usize,
}
impl Default for CaptureLimits {
    fn default() -> Self {
        Self {
            inventory_files: super::SOURCE_FILES,
            read_bytes: super::SOURCE_BYTES,
            file_bytes: MAX_FILE_BYTES,
            captured_files: MAX_CAPTURE_FILES,
            artifact_bytes: MAX_CAPTURE_BYTES,
            patch_bytes: MAX_FILE_BYTES,
        }
    }
}
impl CaptureLimits {
    fn validate(self) -> GitResult<()> {
        let maximum = Self::default();
        if self.inventory_files == 0
            || self.inventory_files > maximum.inventory_files
            || self.read_bytes == 0
            || self.read_bytes > maximum.read_bytes
            || self.file_bytes == 0
            || self.file_bytes > maximum.file_bytes
            || self.captured_files == 0
            || self.captured_files > maximum.captured_files
            || self.artifact_bytes == 0
            || self.artifact_bytes > maximum.artifact_bytes
            || self.patch_bytes == 0
            || self.patch_bytes > maximum.patch_bytes
        {
            return Err(GitError::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BlobEntry {
    oid: String,
    mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    length: u64,
    modified_nanos: u128,
    readonly: bool,
    platform_identity: u128,
    mode: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkingFile {
    bytes: Vec<u8>,
    preserved_sha256: Option<String>,
    stamp: FileStamp,
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    head: Vec<u8>,
    branch: Vec<u8>,
    base: BTreeMap<String, BlobEntry>,
    index: BTreeMap<String, BlobEntry>,
    index_path: PathBuf,
    index_bytes: Vec<u8>,
    index_flags: Vec<u8>,
    untracked: BTreeSet<String>,
    exclusions: BTreeMap<String, String>,
    attributes: Vec<u8>,
    files: BTreeMap<String, Option<WorkingFile>>,
}

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
fn git(executor: &GitExecutor, root: &Path, values: &[&str]) -> GitResult<Vec<u8>> {
    executor.run(root, &args(values))?.checked()
}
fn safe_text(text: &str) -> GitResult<()> {
    if sanitize_text(text).map_err(|_| GitError::Limit)? != text {
        return Err(GitError::SecretContent);
    }
    Ok(())
}
fn path_text(bytes: &[u8]) -> GitResult<String> {
    let path = std::str::from_utf8(bytes).map_err(|_| GitError::Unsupported)?;
    if !relative_file_path(path) || path.chars().any(char::is_control) {
        return Err(GitError::Path);
    }
    safe_text(path)?;
    Ok(path.to_owned())
}
fn nul_entries(bytes: &[u8]) -> GitResult<Vec<&[u8]>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.ends_with(&[0]) {
        return Err(GitError::Process);
    }
    Ok(bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect())
}
fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn parse_entries(bytes: &[u8], tree: bool) -> GitResult<BTreeMap<String, BlobEntry>> {
    let mut entries = BTreeMap::new();
    for record in nul_entries(bytes)? {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(GitError::Process)?;
        let header = std::str::from_utf8(&record[..tab]).map_err(|_| GitError::Process)?;
        let fields: Vec<_> = header.split_ascii_whitespace().collect();
        if fields.len() != 3 || (tree && fields[1] != "blob") || (!tree && fields[2] != "0") {
            return Err(GitError::Unsupported);
        }
        let oid = fields[if tree { 2 } else { 1 }];
        if !valid_oid(oid) {
            return Err(GitError::Process);
        }
        let path = path_text(&record[tab + 1..])?;
        if entries
            .insert(
                path,
                BlobEntry {
                    oid: oid.to_owned(),
                    mode: fields[0].to_owned(),
                },
            )
            .is_some()
        {
            return Err(GitError::Unsupported);
        }
    }
    Ok(entries)
}
/// --stage and -v have a documented combined record: tag, mode, OID, stage,
/// TAB, path. Keep the exact old stage/flag observations while reading the
/// index once. Unsupported flags/stages/duplicates still fail closed.
fn staged_index(bytes: &[u8]) -> GitResult<(BTreeMap<String, BlobEntry>, Vec<u8>)> {
    let mut stages = Vec::new();
    let mut flags = Vec::new();
    for record in nul_entries(bytes)? {
        let stage = record.strip_prefix(b"H ").ok_or(GitError::Unsupported)?;
        let tab = stage
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(GitError::Process)?;
        stages.extend_from_slice(stage);
        stages.push(0);
        flags.extend_from_slice(b"H ");
        flags.extend_from_slice(&stage[tab + 1..]);
        flags.push(0);
    }
    Ok((parse_entries(&stages, false)?, flags))
}
fn path_list(bytes: &[u8]) -> GitResult<BTreeSet<String>> {
    nul_entries(bytes)?.into_iter().map(path_text).collect()
}
fn stamp(metadata: &Metadata) -> GitResult<FileStamp> {
    #[cfg(all(test, windows))]
    let _timing = TimingCall::phase("phase-capture-stamp");
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(GitError::Unsupported);
    }
    #[cfg(windows)]
    let platform_identity = {
        use std::os::windows::fs::MetadataExt;
        // Reparse points include junctions as well as ordinary symlinks.
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(GitError::Link);
        }
        ((metadata.creation_time() as u128) << 32) | metadata.file_attributes() as u128
    };
    #[cfg(unix)]
    let platform_identity = {
        use std::os::unix::fs::MetadataExt;
        ((metadata.dev() as u128) << 64) | metadata.ino() as u128
    };
    #[cfg(not(any(windows, unix)))]
    let platform_identity = 0;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        }
    };
    #[cfg(not(unix))]
    let mode = "100644";
    Ok(FileStamp {
        length: metadata.len(),
        modified_nanos: metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GitError::Io)?
            .as_nanos(),
        readonly: metadata.permissions().readonly(),
        platform_identity,
        mode: mode.to_owned(),
    })
}
fn read_working(root: &Path, relative: &str, maximum: usize) -> GitResult<Option<WorkingFile>> {
    #[cfg(all(test, windows))]
    let mut timing = TimingCall::phase("phase-capture-editable-file");
    let absolute = root.join(relative);
    #[cfg(all(test, windows))]
    let path_timing = TimingCall::phase("phase-capture-path-guard");
    super::paths::reject_links(&absolute)?;
    #[cfg(all(test, windows))]
    drop(path_timing);
    #[cfg(all(test, windows))]
    let metadata_timing = TimingCall::phase("phase-capture-path-metadata");
    let before = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => stamp(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(GitError::Io),
    };
    #[cfg(all(test, windows))]
    drop(metadata_timing);
    if before.length > maximum as u64 {
        return Err(GitError::Limit);
    }
    #[cfg(all(test, windows))]
    let open_timing = TimingCall::phase("phase-capture-file-open");
    let mut file = File::open(&absolute)?;
    #[cfg(all(test, windows))]
    drop(open_timing);
    #[cfg(all(test, windows))]
    let metadata_timing = TimingCall::phase("phase-capture-handle-metadata");
    if stamp(&file.metadata()?)? != before {
        return Err(GitError::InconsistentCapture);
    }
    #[cfg(all(test, windows))]
    drop(metadata_timing);
    let mut bytes = Vec::with_capacity(before.length as usize);
    #[cfg(all(test, windows))]
    let mut read_timing = TimingCall::phase("phase-capture-editable-read");
    file.by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    #[cfg(all(test, windows))]
    {
        if let Some(timing) = &mut timing {
            timing.add_bytes(bytes.len() as u64);
        }
        if let Some(timing) = &mut read_timing {
            timing.add_bytes(bytes.len() as u64);
        }
        drop(read_timing);
    }
    if bytes.len() > maximum {
        return Err(GitError::Limit);
    }
    #[cfg(all(test, windows))]
    let path_timing = TimingCall::phase("phase-capture-path-guard");
    super::paths::reject_links(&absolute)?;
    #[cfg(all(test, windows))]
    drop(path_timing);
    #[cfg(all(test, windows))]
    let _metadata_timing = TimingCall::phase("phase-capture-final-file-fence");
    if stamp(&fs::symlink_metadata(&absolute)?)? != before || bytes.len() as u64 != before.length {
        return Err(GitError::InconsistentCapture);
    }
    Ok(Some(WorkingFile {
        bytes,
        preserved_sha256: None,
        stamp: before,
    }))
}
fn read_preserved(root: &Path, relative: &str) -> GitResult<Option<WorkingFile>> {
    #[cfg(all(test, windows))]
    let mut timing = TimingCall::phase("phase-capture-preserved-file");
    let path = root.join(relative);
    #[cfg(all(test, windows))]
    let path_timing = TimingCall::phase("phase-capture-path-guard");
    super::paths::reject_links(&path)?;
    #[cfg(all(test, windows))]
    drop(path_timing);
    #[cfg(all(test, windows))]
    let metadata_timing = TimingCall::phase("phase-capture-path-metadata");
    let before = match fs::symlink_metadata(&path) {
        Ok(value) => stamp(&value)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(GitError::Io),
    };
    #[cfg(all(test, windows))]
    drop(metadata_timing);
    if before.length > MAX_OPAQUE_FILE_BYTES {
        return Err(GitError::Limit);
    }
    #[cfg(all(test, windows))]
    let open_timing = TimingCall::phase("phase-capture-file-open");
    let mut file = File::open(&path)?;
    #[cfg(all(test, windows))]
    drop(open_timing);
    #[cfg(all(test, windows))]
    let metadata_timing = TimingCall::phase("phase-capture-handle-metadata");
    if stamp(&file.metadata()?)? != before {
        return Err(GitError::InconsistentCapture);
    }
    #[cfg(all(test, windows))]
    drop(metadata_timing);
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65_536];
    let mut size = 0u64;
    loop {
        #[cfg(all(test, windows))]
        let mut read_timing = TimingCall::phase("phase-capture-preserved-read");
        let n = file.read(&mut buffer)?;
        #[cfg(all(test, windows))]
        {
            if let Some(timing) = &mut read_timing {
                timing.add_bytes(n as u64);
            }
            drop(read_timing);
        }
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > MAX_OPAQUE_FILE_BYTES {
            return Err(GitError::Limit);
        }
        #[cfg(all(test, windows))]
        let mut hash_timing = TimingCall::phase("phase-capture-preserved-hash");
        digest.update(&buffer[..n]);
        #[cfg(all(test, windows))]
        if let Some(timing) = &mut hash_timing {
            timing.add_bytes(n as u64);
        }
    }
    #[cfg(all(test, windows))]
    if let Some(timing) = &mut timing {
        timing.add_bytes(size);
    }
    #[cfg(all(test, windows))]
    let path_timing = TimingCall::phase("phase-capture-path-guard");
    super::paths::reject_links(&path)?;
    #[cfg(all(test, windows))]
    drop(path_timing);
    #[cfg(all(test, windows))]
    let metadata_timing = TimingCall::phase("phase-capture-final-file-fence");
    if size != before.length
        || stamp(&file.metadata()?)? != before
        || stamp(&fs::symlink_metadata(&path)?)? != before
    {
        return Err(GitError::InconsistentCapture);
    }
    #[cfg(all(test, windows))]
    drop(metadata_timing);
    #[cfg(all(test, windows))]
    let _hash_timing = TimingCall::phase("phase-capture-preserved-finalize");
    Ok(Some(WorkingFile {
        bytes: Vec::new(),
        preserved_sha256: Some(format!("{:x}", digest.finalize())),
        stamp: before,
    }))
}
pub(super) fn exclusion(path: &str) -> Option<(String, &'static str)> {
    let parts: Vec<_> = path.split('/').collect();
    for (position, part) in parts.iter().enumerate() {
        let lower = part.to_ascii_lowercase();
        if matches!(lower.as_str(), "node_modules" | "vendor" | ".venv" | "venv") {
            return Some((parts[..=position].join("/"), "dependency-directory"));
        }
        if matches!(
            lower.as_str(),
            "target" | "build" | "dist" | "coverage" | ".cache" | ".next"
        ) {
            return Some((parts[..=position].join("/"), "build-or-cache-directory"));
        }
        if lower == "secrets" || lower == "credentials" {
            return Some((parts[..=position].join("/"), "sensitive-path"));
        }
    }
    let name = parts.last()?.to_ascii_lowercase();
    if name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || matches!(
            name.as_str(),
            "id_rsa" | "id_ed25519" | "credentials.json" | "secrets.json"
        )
    {
        return Some((path.to_owned(), "sensitive-path"));
    }
    None
}
fn exclude(exclusions: &mut BTreeMap<String, String>, path: String, reason: &str) {
    // A directory-level omission already explains its descendants.
    if exclusions
        .keys()
        .any(|existing| path == *existing || path.starts_with(&format!("{existing}/")))
    {
        return;
    }
    exclusions.retain(|existing, _| !existing.starts_with(&format!("{path}/")));
    exclusions.insert(path, reason.to_owned());
}
fn index_path(executor: &GitExecutor, root: &Path) -> GitResult<PathBuf> {
    let output = git(
        executor,
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )?;
    let text = std::str::from_utf8(&output)
        .map_err(|_| GitError::Path)?
        .trim_end_matches(['\r', '\n']);
    if text.is_empty() || text.contains(['\r', '\n', '\0']) {
        return Err(GitError::Path);
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(GitError::Path);
    }
    super::paths::reject_links(&path)?;
    Ok(path)
}
fn raw_index_at(path: &Path) -> GitResult<Vec<u8>> {
    #[cfg(all(test, windows))]
    let _timing = TimingCall::phase("phase-capture-index-read");
    super::paths::reject_links(path)?;
    if path.with_extension("lock").exists() {
        return Err(GitError::Conflict);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > 16 * MAX_FILE_BYTES as u64 {
        return Err(GitError::Limit);
    }
    let bytes = fs::read(path)?;
    if bytes.len() > 16 * MAX_FILE_BYTES {
        return Err(GitError::Limit);
    }
    Ok(bytes)
}
// Read-only admission inspection uses the capture validator itself, including
// index flags, attributes, links and coherent repeated observations.
pub(super) fn inspect_preparation(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
    base_hashes: &BTreeMap<String, String>,
) -> GitResult<(bool, bool)> {
    inspect_preparation_with_preservation(executor, root, base, base_hashes, None)
}
pub(super) fn inspect_preparation_with_preservation(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
    base_hashes: &BTreeMap<String, String>,
    preservation: Option<&BasePreservation>,
) -> GitResult<(bool, bool)> {
    #[cfg(all(test, windows))]
    let _timing = TimingCall::phase("phase-capture-preparation");
    let limits = CaptureLimits::default();
    let first = inspect_at_profile(executor, root, base, base, limits, preservation)?;
    for (path, file) in &first.files {
        if preservation.is_none_or(|m| m.editable_paths.binary_search(path).is_ok())
            && let Some(file) = file
        {
            text_content(file.bytes.clone(), "100644")?;
        }
    }
    if inspect_at_profile(executor, root, base, base, limits, preservation)? != first
        || inspect_at_profile(executor, root, base, base, limits, preservation)? != first
    {
        return Err(GitError::InconsistentCapture);
    }
    let clean = first.base == first.index
        && first.untracked.is_empty()
        && first.exclusions.is_empty()
        && first.files.len() == base_hashes.len()
        && first.files.iter().all(|(path, file)| {
            file.as_ref().is_some_and(|file| {
                base_hashes.get(path).is_some_and(|hash| {
                    *hash
                        == file
                            .preserved_sha256
                            .clone()
                            .unwrap_or_else(|| content_hash(&file.bytes))
                })
            })
        });
    Ok((first.exclusions.is_empty(), clean))
}
pub(super) fn profile_attributes(
    executor: &GitExecutor,
    root: &Path,
    included: &[String],
    source: Option<&str>,
) -> GitResult<Vec<u8>> {
    #[cfg(all(test, windows))]
    let _timing = TimingCall::phase("phase-capture-attributes");
    let mut attributes = Vec::new();
    let mut position = 0;
    while position < included.len() {
        let (end, input) = attribute_input(included, position)?;
        let batch = &included[position..end];
        let mut command = args(&["check-attr", "--stdin", "-z", "--all"]);
        if let Some(source) = source {
            if !super::worktrees::valid_oid(source) {
                return Err(GitError::Base);
            }
            command.push(format!("--source={source}").into());
        }
        let observed = executor.run_with_input(root, &command, &input)?.checked()?;
        let fields = nul_entries(&observed)?;
        if fields.len() % 3 != 0 {
            return Err(GitError::Process);
        }
        let mut seen = BTreeSet::new();
        for triple in fields.chunks(3) {
            if !batch.iter().any(|path| path.as_bytes() == triple[0])
                || triple[1].is_empty()
                || !seen.insert((triple[0], triple[1]))
            {
                return Err(GitError::Process);
            }
            if matches!(triple[1], b"filter" | b"working-tree-encoding" | b"ident") {
                return Err(GitError::Unsupported);
            }
        }
        if attributes.len().saturating_add(observed.len()) > 8 * 1_048_576 {
            return Err(GitError::Limit);
        }
        attributes.extend(observed);
        position = end;
    }
    Ok(attributes)
}

// Bound both bytes and records, independently of editable-file budgets. NUL
// framing avoids Windows argv limits; --stdin never interprets paths as flags.
fn attribute_input(included: &[String], start: usize) -> GitResult<(usize, Vec<u8>)> {
    const INPUT_BYTES: usize = 65_536;
    const INPUT_PATHS: usize = 256;
    let mut end = start;
    let mut input = Vec::new();
    while let Some(path) = included.get(end) {
        if !relative_file_path(path) || path.chars().any(char::is_control) {
            return Err(GitError::Path);
        }
        if end - start == INPUT_PATHS || input.len() + path.len() + 1 > INPUT_BYTES {
            break;
        }
        input.extend_from_slice(path.as_bytes());
        input.push(0);
        end += 1;
    }
    if end == start {
        return Err(GitError::Limit);
    }
    Ok((end, input))
}
fn inspect_at_profile(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
    expected_head: &str,
    limits: CaptureLimits,
    preservation: Option<&BasePreservation>,
) -> GitResult<Snapshot> {
    #[cfg(all(test, windows))]
    let _timing = TimingCall::phase("phase-capture-observation");
    let observation = executor.capture_head(root)?;
    let head = observation.head;
    if std::str::from_utf8(&head)
        .map_err(|_| GitError::Base)?
        .trim()
        != expected_head
    {
        return Err(GitError::Base);
    }
    let branch = git(executor, root, &["symbolic-ref", "--quiet", "HEAD"])?;
    let initial_index = raw_index_at(&observation.index)?;
    let base_entries = parse_entries(
        &git(
            executor,
            root,
            &["ls-tree", "-r", "-z", "--full-tree", base],
        )?,
        true,
    )?;
    let (index, index_flags) =
        staged_index(&git(executor, root, &["ls-files", "-s", "-v", "-z"])?)?;
    if let Some(manifest) = preservation
        && (!manifest.validate()
            || manifest.base_oid != base
            || manifest.entries.len() != base_entries.len()
            || manifest.entries.iter().any(|entry| {
                !base_entries
                    .get(&entry.path)
                    .is_some_and(|base| base.oid == entry.git_oid && base.mode == entry.mode)
            }))
    {
        return Err(GitError::Ownership);
    }
    let untracked = path_list(&git(
        executor,
        root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?)?;
    let ignored = git(
        executor,
        root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
    )?;
    if preservation.is_some() && !ignored.is_empty() {
        return Err(GitError::UnpreservedWork);
    }
    let inventory: BTreeSet<_> = base_entries
        .keys()
        .chain(index.keys())
        .chain(untracked.iter())
        .cloned()
        .collect();
    if inventory.len() > limits.inventory_files {
        return Err(GitError::Limit);
    }
    let mut exclusions = BTreeMap::new();
    for raw in nul_entries(&ignored)? {
        let directory = raw.ends_with(b"/");
        let path = path_text(if directory {
            &raw[..raw.len() - 1]
        } else {
            raw
        })?;
        exclude(
            &mut exclusions,
            path,
            if directory {
                "git-ignored-directory"
            } else {
                "git-ignored-file"
            },
        );
    }
    let mut included = Vec::new();
    for path in inventory {
        if let Some(manifest) = preservation {
            if exclusion(&path).is_some() {
                return Err(GitError::Unsupported);
            }
            if manifest.editable_paths.binary_search(&path).is_err() {
                let entry = manifest
                    .entries
                    .iter()
                    .find(|e| e.path == path)
                    .ok_or(GitError::Scope)?;
                if !index
                    .get(&path)
                    .is_some_and(|value| value.oid == entry.git_oid && value.mode == entry.mode)
                {
                    return Err(GitError::UnpreservedWork);
                }
            }
            included.push(path);
        } else if let Some((scope, reason)) = exclusion(&path) {
            exclude(&mut exclusions, scope, reason);
        } else {
            included.push(path);
        }
    }
    if exclusions.len() > MAX_CAPTURE_FILES {
        return Err(GitError::Limit);
    }
    let attributes = profile_attributes(executor, root, &included, None)?;
    let mut files = BTreeMap::new();
    let mut read_bytes = 0usize;
    let mut editable_bytes = 0usize;
    let editable_budget = if let Some(manifest) = preservation {
        limits
            .artifact_bytes
            .checked_sub(
                serde_json::to_vec(manifest)
                    .map_err(|_| GitError::Limit)?
                    .len(),
            )
            .ok_or(GitError::Limit)?
    } else {
        limits.read_bytes
    };
    for path in included {
        for entry in [base_entries.get(&path), index.get(&path)]
            .into_iter()
            .flatten()
        {
            if entry.mode != "100644" {
                return Err(GitError::Unsupported);
            }
        }
        let opaque = preservation.filter(|m| m.editable_paths.binary_search(&path).is_err());
        let observed = if opaque.is_some() {
            read_preserved(root, &path)?
        } else {
            read_working(
                root,
                &path,
                if preservation.is_some() {
                    limits
                        .file_bytes
                        .min(editable_budget.saturating_sub(editable_bytes))
                } else {
                    limits.file_bytes
                },
            )?
        };
        if opaque.is_none()
            && let Some(file) = &observed
        {
            editable_bytes = editable_bytes
                .checked_add(file.bytes.len())
                .ok_or(GitError::Limit)?;
        }
        if let Some(manifest) = opaque {
            let entry = manifest
                .entries
                .iter()
                .find(|e| e.path == path)
                .ok_or(GitError::Scope)?;
            if !observed.as_ref().is_some_and(|file| {
                file.stamp.length == entry.size
                    && file.preserved_sha256.as_ref() == Some(&entry.sha256)
            }) {
                return Err(GitError::UnpreservedWork);
            }
        }
        if let Some(file) = &observed {
            if file.stamp.mode != "100644" {
                return Err(GitError::Unsupported);
            }
            read_bytes = read_bytes
                .checked_add(file.stamp.length as usize)
                .ok_or(GitError::Limit)?;
            if read_bytes > limits.read_bytes {
                return Err(GitError::Limit);
            }
        }
        files.insert(path, observed);
    }
    // Resolve again at the final edge, not only reread a cached index path.
    // A .git retarget with identical index bytes is still a different source.
    let final_index = index_path(executor, root)?;
    let index_bytes = raw_index_at(&final_index)?;
    if observation.index != final_index || initial_index != index_bytes {
        return Err(GitError::InconsistentCapture);
    }
    Ok(Snapshot {
        head,
        branch,
        base: base_entries,
        index,
        index_path: observation.index,
        index_bytes,
        index_flags,
        untracked,
        exclusions,
        attributes,
        files,
    })
}

/// Conservative application profile: no staged, unstaged, untracked or omitted
/// data. A commit name alone never grants authority over the current checkout.
pub(crate) fn clean_target_with_preservation(
    executor: &GitExecutor,
    root: &Path,
    common_git_dir: &Path,
    tree_commit: &str,
    head: &str,
    symbolic_ref: &str,
    preservation: Option<&BasePreservation>,
) -> GitResult<crate::integration_records::TargetState> {
    let root = super::paths::canonical_directory(root)?;
    let common = super::paths::canonical_directory(common_git_dir)?;
    let actual = git(
        executor,
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let actual = std::str::from_utf8(&actual)
        .map_err(|_| GitError::Path)?
        .trim();
    if super::paths::canonical_directory(Path::new(actual))? != common {
        return Err(GitError::Ownership);
    }
    let limits = CaptureLimits::default();
    let rebased = preservation
        .map(|m| super::preservation::for_tree(executor, &root, m, tree_commit))
        .transpose()?;
    let first = inspect_at_profile(executor, &root, tree_commit, head, limits, rebased.as_ref())?;
    if std::str::from_utf8(&first.branch)
        .map_err(|_| GitError::Path)?
        .trim()
        != symbolic_ref
        || first.base != first.index
        || !first.untracked.is_empty()
        || !first.exclusions.is_empty()
    {
        return Err(GitError::UnpreservedWork);
    }
    let mut cache = BTreeMap::new();
    let mut bytes = 0;
    let mut files = Vec::new();
    for (path, entry) in &first.base {
        let working = first
            .files
            .get(path)
            .and_then(Option::as_ref)
            .ok_or(GitError::UnpreservedWork)?;
        let sha = if let Some(manifest) = &rebased {
            let expected = manifest
                .entries
                .iter()
                .find(|e| e.path == *path)
                .ok_or(GitError::Ownership)?;
            let observed = working
                .preserved_sha256
                .clone()
                .unwrap_or_else(|| content_hash(&working.bytes));
            if observed != expected.sha256 {
                return Err(GitError::UnpreservedWork);
            }
            observed
        } else {
            let base = read_blob(executor, &root, entry, limits, &mut cache, &mut bytes)?;
            if working.bytes != base {
                return Err(GitError::UnpreservedWork);
            }
            content_hash(&working.bytes)
        };
        if entry.mode != "100644" {
            return Err(GitError::UnpreservedWork);
        }
        files.push(serde_json::json!([path, sha, working.stamp.mode]));
    }
    if inspect_at_profile(executor, &root, tree_commit, head, limits, rebased.as_ref())? != first {
        return Err(GitError::InconsistentCapture);
    }
    Ok(crate::integration_records::TargetState {
        directory: root.to_str().ok_or(GitError::Path)?.into(),
        common_git_dir: common.to_str().ok_or(GitError::Path)?.into(),
        symbolic_ref: symbolic_ref.into(),
        head: head.into(),
        // Git may refresh its stat cache without changing staged content.
        // Pin logical entries/flags; raw index bytes remain part of each
        // coherent read comparison, not the persisted approval fingerprint.
        index_sha256: content_hash(
            serde_json::to_string(
                &first
                    .index
                    .iter()
                    .map(|(path, entry)| serde_json::json!([path, entry.oid, entry.mode]))
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| GitError::Limit)?
            .as_bytes(),
        ),
        working_sha256: content_hash(
            serde_json::to_string(&files)
                .map_err(|_| GitError::Limit)?
                .as_bytes(),
        ),
    })
}

fn read_blob(
    executor: &GitExecutor,
    root: &Path,
    entry: &BlobEntry,
    limits: CaptureLimits,
    cache: &mut BTreeMap<String, Vec<u8>>,
    read_bytes: &mut usize,
) -> GitResult<Vec<u8>> {
    if let Some(bytes) = cache.get(&entry.oid) {
        return Ok(bytes.clone());
    }
    let size = git(executor, root, &["cat-file", "-s", &entry.oid])?;
    let size = std::str::from_utf8(&size)
        .map_err(|_| GitError::Process)?
        .trim()
        .parse::<usize>()
        .map_err(|_| GitError::Process)?;
    if size > limits.file_bytes {
        return Err(GitError::Limit);
    }
    *read_bytes = read_bytes.checked_add(size).ok_or(GitError::Limit)?;
    if *read_bytes > limits.read_bytes {
        return Err(GitError::Limit);
    }
    let bytes = git(executor, root, &["cat-file", "blob", &entry.oid])?;
    if bytes.len() != size {
        return Err(GitError::InconsistentCapture);
    }
    cache.insert(entry.oid.clone(), bytes.clone());
    Ok(bytes)
}
fn text_content(bytes: Vec<u8>, mode: &str) -> GitResult<FileContent> {
    if bytes.contains(&0) {
        return Err(GitError::Unsupported);
    }
    let content = String::from_utf8(bytes).map_err(|_| GitError::Unsupported)?;
    safe_text(&content)?;
    FileContent::new(content, mode.to_owned()).map_err(|_| GitError::Unsupported)
}
fn quote_path(prefix: &str, path: &str) -> String {
    // JSON string escaping is compatible with Git C quoting for this path profile.
    serde_json::to_string(&format!("{prefix}/{path}")).expect("string serialization")
}
fn append_lines(patch: &mut String, prefix: char, content: &str) {
    for line in content.split_inclusive('\n') {
        patch.push(prefix);
        patch.push_str(line);
        if !line.ends_with('\n') {
            patch.push_str("\n\\ No newline at end of file\n");
        }
    }
}
/// Deterministic full-file unified patch. Renames are explicit deletion/addition;
/// index-only differences remain in the separately hashed file manifest.
fn append_patch(
    patch: &mut String,
    path: &str,
    base: Option<&FileContent>,
    work: Option<&FileContent>,
) {
    if base == work {
        return;
    }
    let a = quote_path("a", path);
    let b = quote_path("b", path);
    patch.push_str(&format!("diff --git {a} {b}\n"));
    if base.is_none() {
        patch.push_str("new file mode 100644\n");
    }
    if work.is_none() {
        patch.push_str("deleted file mode 100644\n");
    }
    patch.push_str(&format!(
        "--- {}\n+++ {}\n",
        if base.is_some() { &a } else { "/dev/null" },
        if work.is_some() { &b } else { "/dev/null" }
    ));
    let old = base.map_or("", |file| file.content.as_str());
    let new = work.map_or("", |file| file.content.as_str());
    let old_count = old.split_inclusive('\n').count();
    let new_count = new.split_inclusive('\n').count();
    if old_count == 0 && new_count == 0 {
        return;
    }
    patch.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        usize::from(old_count > 0),
        old_count,
        usize::from(new_count > 0),
        new_count
    ));
    append_lines(patch, '-', old);
    append_lines(patch, '+', new);
}

/// Capture two matching inventories, file bytes and metadata around the capture
/// interval. Ownership/path authorization must also be checked by the service.
pub fn capture(
    executor: &GitExecutor,
    root: &Path,
    intent: &WorktreeIntent,
    limits: CaptureLimits,
) -> GitResult<CapturedArtifact> {
    capture_with_observer(executor, root, intent, limits, || Ok(()))
}

/// Deterministic fault/race seam: called after the first complete filesystem
/// observation. It never executes repository-defined commands itself.
#[doc(hidden)]
pub fn capture_with_observer<F: FnOnce() -> GitResult<()>>(
    executor: &GitExecutor,
    root: &Path,
    intent: &WorktreeIntent,
    limits: CaptureLimits,
    between: F,
) -> GitResult<CapturedArtifact> {
    limits.validate()?;
    if !intent.validate() {
        return Err(GitError::Path);
    }
    let root = super::paths::canonical_directory(root)?;
    if root != super::paths::canonical_directory(Path::new(&intent.path))? {
        return Err(GitError::Ownership);
    }
    super::paths::authorized_child(Path::new(&intent.authorized_root), &root, true)?;
    if root == super::paths::canonical_directory(Path::new(&intent.main_repository))? {
        return Err(GitError::Scope);
    }
    if let Some(manifest) = &intent.preservation {
        super::preservation::verify_base(executor, &root, manifest)?;
    }
    let observe = || {
        inspect_at_profile(
            executor,
            &root,
            &intent.base_oid,
            &intent.base_oid,
            limits,
            intent.preservation.as_ref(),
        )
    };
    let first = observe()?;
    let branch = std::str::from_utf8(&first.branch)
        .map_err(|_| GitError::Ownership)?
        .trim();
    if branch != format!("refs/heads/{}", intent.branch) {
        return Err(GitError::Ownership);
    }
    between()?;
    let second = observe()?;
    if first != second {
        return Err(GitError::InconsistentCapture);
    }
    let mut cache = BTreeMap::new();
    let mut read_bytes = 0;
    let mut files = Vec::new();
    let mut patch = String::new();
    for (path, working) in &first.files {
        if intent
            .preservation
            .as_ref()
            .is_some_and(|m| m.editable_paths.binary_search(path).is_err())
        {
            continue;
        }
        let base_entry = first.base.get(path);
        let index_entry = first.index.get(path);
        if let Some(manifest) = &intent.preservation
            && base_entry == index_entry
            && let Some(entry) = manifest.entries.iter().find(|entry| entry.path == *path)
            && working
                .as_ref()
                .is_some_and(|file| content_hash(&file.bytes) == entry.sha256)
        {
            continue;
        }
        let base_bytes = base_entry
            .map(|entry| read_blob(executor, &root, entry, limits, &mut cache, &mut read_bytes))
            .transpose()?;
        let index_bytes = index_entry
            .map(|entry| read_blob(executor, &root, entry, limits, &mut cache, &mut read_bytes))
            .transpose()?;
        if intent.preservation.is_some() {
            for (entry, bytes) in [
                (base_entry, base_bytes.as_ref()),
                (index_entry, index_bytes.as_ref()),
            ] {
                if let (Some(entry), Some(bytes)) = (entry, bytes) {
                    super::object_identity::verify("blob", &entry.oid, bytes)?;
                }
            }
        }
        let work_bytes = working.as_ref().map(|file| &file.bytes);
        if base_bytes.as_ref() == index_bytes.as_ref() && index_bytes.as_ref() == work_bytes {
            continue;
        }
        if files.len() >= limits.captured_files {
            return Err(GitError::Limit);
        }
        let base = base_bytes
            .map(|bytes| text_content(bytes, "100644"))
            .transpose()?;
        let index = index_bytes
            .map(|bytes| text_content(bytes, "100644"))
            .transpose()?;
        let worktree = working
            .as_ref()
            .map(|file| text_content(file.bytes.clone(), "100644"))
            .transpose()?;
        append_patch(&mut patch, path, base.as_ref(), worktree.as_ref());
        if patch.len() > limits.patch_bytes {
            return Err(GitError::Limit);
        }
        files.push(CapturedFile {
            path: path.clone(),
            base,
            index,
            worktree,
            untracked: base_entry.is_none()
                && index_entry.is_none()
                && first.untracked.contains(path),
        });
    }
    // The immutable-object reads above must not widen the accepted observation
    // interval: a final read also catches writes during patch/manifest assembly.
    if observe()? != first {
        return Err(GitError::InconsistentCapture);
    }
    if let Some(manifest) = &intent.preservation {
        super::preservation::verify_base(executor, &root, manifest)?;
    }
    safe_text(&patch)?;
    let mut artifact = CapturedArtifact {
        id: format!(
            "git_{}",
            &content_hash(intent.operation_id.as_bytes())[..48]
        ),
        revision: String::new(),
        operation_id: intent.operation_id.clone(),
        repository_id: intent.repository_id.clone(),
        run_id: intent.run_id.clone(),
        task_id: intent.task_id.clone(),
        attempt_id: intent.attempt_id.clone(),
        base_oid: intent.base_oid.clone(),
        origin: intent.path.clone(),
        files,
        exclusions: first
            .exclusions
            .into_iter()
            .map(|(path, reason)| CaptureExclusion { path, reason })
            .collect(),
        patch_sha256: content_hash(patch.as_bytes()),
        patch,
        preservation: intent.preservation.clone(),
    };
    artifact.revision = artifact.content_revision();
    let artifact_bytes = artifact
        .files
        .iter()
        .flat_map(|file| [&file.base, &file.index, &file.worktree])
        .flatten()
        .try_fold(
            artifact.patch.len()
                + artifact
                    .preservation
                    .as_ref()
                    .map(|m| {
                        serde_json::to_vec(m)
                            .map(|v| v.len())
                            .map_err(|_| GitError::Limit)
                    })
                    .transpose()?
                    .unwrap_or(0),
            |bytes, content| {
                bytes
                    .checked_add(content.content.len())
                    .ok_or(GitError::Limit)
            },
        )?;
    if artifact_bytes > limits.artifact_bytes || !artifact.validate() {
        return Err(GitError::Limit);
    }
    Ok(artifact)
}

#[cfg(test)]
mod metadata_batch_tests {
    use super::*;

    #[test]
    fn attribute_input_is_nul_framed_and_bounded_by_bytes_and_records() {
        let paths = (0..584)
            .map(|n| format!("docs/file {n}.md"))
            .collect::<Vec<_>>();
        let (next, bytes) = attribute_input(&paths, 0).unwrap();
        assert_eq!(next, 256);
        assert_eq!(
            bytes.split(|b| *b == 0).filter(|p| !p.is_empty()).count(),
            256
        );
        assert!(bytes.ends_with(&[0]));
        assert!(bytes.len() <= 65_536);
        let long = (0..20)
            .map(|n| format!("{n}{}", "x".repeat(4093)))
            .collect::<Vec<_>>();
        let (next, bytes) = attribute_input(&long, 0).unwrap();
        assert_eq!(next, 16);
        assert!(bytes.len() <= 65_536);
        for path in ["../outside", "bad\0path", "bad\npath", ".git/config"] {
            assert_eq!(attribute_input(&[path.into()], 0), Err(GitError::Path));
        }
    }

    #[test]
    fn stdin_attributes_preserve_exact_observations_and_reject_protected_attributes() {
        let fixture = super::super::executor::MetadataFixture::new();
        let root = fixture.repository("attribute-batches");
        fs::write(root.join(".gitattributes"), b"*.md text eol=lf\n").unwrap();
        let paths = (0..584)
            .map(|n| format!("docs/file {n}.md"))
            .collect::<Vec<_>>();
        let actual = profile_attributes(&fixture.executor, &root, &paths, None).unwrap();
        let mut expected = Vec::new();
        for batch in paths.chunks(16) {
            let mut command = args(&["check-attr", "-z", "--all", "--"]);
            command.extend(batch.iter().map(OsString::from));
            expected.extend(
                fixture
                    .executor
                    .run(&root, &command)
                    .unwrap()
                    .checked()
                    .unwrap(),
            );
        }
        assert_eq!(actual, expected);
        fs::write(root.join(".gitattributes"), b"*.md filter=must-never-run\n").unwrap();
        assert_eq!(
            profile_attributes(&fixture.executor, &root, &paths, None),
            Err(GitError::Unsupported)
        );
    }

    #[test]
    fn staged_flags_preserve_separate_observations() {
        for oid in ["a".repeat(40), "b".repeat(64)] {
            let stages = format!("100644 {oid} 0\tdirectory/file space.md\0");
            let combined = format!("H {stages}");
            let (entries, flags) = staged_index(combined.as_bytes()).unwrap();
            assert_eq!(entries, parse_entries(stages.as_bytes(), false).unwrap());
            assert_eq!(flags, b"H directory/file space.md\0");
        }
        assert_eq!(staged_index(b"").unwrap(), (BTreeMap::new(), Vec::new()));
    }

    #[test]
    fn staged_flags_reject_hidden_unmerged_malformed_and_duplicate_entries() {
        let oid = "a".repeat(40);
        for flag in ["h", "S", "s", "M", "m", "?", "R", "C"] {
            assert_eq!(
                staged_index(format!("{flag} 100644 {oid} 0\tfile.txt\0").as_bytes()),
                Err(GitError::Unsupported)
            );
        }
        for bad in [
            format!("H 100644 {oid} 1\tfile.txt\0"),
            format!("H 100644 {oid} 2\tfile.txt\0"),
            format!("H 100644 {oid} 3\tfile.txt\0"),
            format!("H 100644 {oid} 0\tfile.txt\0H 100644 {oid} 0\tfile.txt\0"),
        ] {
            assert_eq!(staged_index(bad.as_bytes()), Err(GitError::Unsupported));
        }
        for bad in [
            format!("H 100644 {oid} 0 file.txt\0"),
            format!("H 100644 {oid} 0\tfile.txt"),
            "H 100644 not-an-oid 0\tfile.txt\0".into(),
        ] {
            assert_eq!(staged_index(bad.as_bytes()), Err(GitError::Process));
        }
    }

    #[test]
    fn retargeted_git_marker_with_identical_index_bytes_is_not_the_same_snapshot() {
        let fixture = super::super::executor::MetadataFixture::new();
        let first = fixture.repository("first");
        let second = fixture.repository("second");
        let branch = "orchestrix/capture-probe";
        let base = fixture.seed_empty(&first, branch);
        assert_eq!(fixture.seed_empty(&second, branch), base);
        assert_eq!(
            fs::read(first.join(".git/index")).unwrap(),
            fs::read(second.join(".git/index")).unwrap()
        );
        let authorized = fixture.root.join("authorized");
        fs::create_dir(&authorized).unwrap();
        let checkout = authorized.join("checkout");
        fs::create_dir(&checkout).unwrap();
        let marker = |repository: &Path| {
            format!(
                "gitdir: {}\n",
                super::super::paths::git_path(&repository.join(".git"))
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            )
        };
        fs::write(checkout.join(".git"), marker(&first)).unwrap();
        let text = |path: &Path| path.to_str().unwrap().to_owned();
        let intent = WorktreeIntent {
            preservation: None,
            operation_id: "capture-frame".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: base,
            domain_base: "base".into(),
            worktree_ref: "worktree".into(),
            authorized_root: text(&authorized),
            path: text(&checkout),
            main_repository: text(&first),
            branch: branch.into(),
            common_git_dir: text(&first.join(".git")),
            ownership_token: "a".repeat(64),
        };
        assert!(intent.validate());
        let result = capture_with_observer(
            &fixture.executor,
            &checkout,
            &intent,
            CaptureLimits::default(),
            || {
                fs::write(checkout.join(".git"), marker(&second)).unwrap();
                Ok(())
            },
        );
        assert!(matches!(result, Err(GitError::InconsistentCapture)));
    }
}
