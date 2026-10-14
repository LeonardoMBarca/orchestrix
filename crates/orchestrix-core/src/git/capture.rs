//! Exact, bounded snapshots of the supported regular UTF-8 file profile.
//! No checkout conversion, clean filter, external diff or project command runs.
use super::{GitError, GitExecutor, GitResult};
use crate::git_records::{
    CaptureExclusion, CapturedArtifact, CapturedFile, FileContent, MAX_CAPTURE_BYTES,
    MAX_CAPTURE_FILES, MAX_FILE_BYTES, WorktreeIntent, content_hash, relative_file_path,
};
use crate::persistence_privacy::sanitize_text;
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
            inventory_files: 4096,
            read_bytes: 64 * MAX_FILE_BYTES,
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
    stamp: FileStamp,
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    head: Vec<u8>,
    branch: Vec<u8>,
    base: BTreeMap<String, BlobEntry>,
    index: BTreeMap<String, BlobEntry>,
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
fn path_list(bytes: &[u8]) -> GitResult<BTreeSet<String>> {
    nul_entries(bytes)?.into_iter().map(path_text).collect()
}
fn stamp(metadata: &Metadata) -> GitResult<FileStamp> {
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
    let absolute = root.join(relative);
    super::paths::reject_links(&absolute)?;
    let before = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => stamp(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(GitError::Io),
    };
    if before.length > maximum as u64 {
        return Err(GitError::Limit);
    }
    let mut file = File::open(&absolute)?;
    if stamp(&file.metadata()?)? != before {
        return Err(GitError::InconsistentCapture);
    }
    let mut bytes = Vec::with_capacity(before.length as usize);
    file.by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(GitError::Limit);
    }
    super::paths::reject_links(&absolute)?;
    if stamp(&fs::symlink_metadata(&absolute)?)? != before || bytes.len() as u64 != before.length {
        return Err(GitError::InconsistentCapture);
    }
    Ok(Some(WorkingFile {
        bytes,
        stamp: before,
    }))
}
fn exclusion(path: &str) -> Option<(String, &'static str)> {
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
fn raw_index(executor: &GitExecutor, root: &Path) -> GitResult<Vec<u8>> {
    let path = index_path(executor, root)?;
    if path.with_extension("lock").exists() {
        return Err(GitError::Conflict);
    }
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > 16 * MAX_FILE_BYTES as u64 {
        return Err(GitError::Limit);
    }
    let bytes = fs::read(&path)?;
    if bytes.len() > 16 * MAX_FILE_BYTES {
        return Err(GitError::Limit);
    }
    Ok(bytes)
}
fn inspect(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
    limits: CaptureLimits,
) -> GitResult<Snapshot> {
    executor.ensure_supported(root)?;
    let head = git(executor, root, &["rev-parse", "--verify", "HEAD"])?;
    if std::str::from_utf8(&head)
        .map_err(|_| GitError::Base)?
        .trim()
        != base
    {
        return Err(GitError::Base);
    }
    let branch = git(executor, root, &["symbolic-ref", "--quiet", "HEAD"])?;
    let initial_index = raw_index(executor, root)?;
    let base_entries = parse_entries(
        &git(
            executor,
            root,
            &["ls-tree", "-r", "-z", "--full-tree", base],
        )?,
        true,
    )?;
    let index = parse_entries(&git(executor, root, &["ls-files", "-s", "-z"])?, false)?;
    let index_flags = git(executor, root, &["ls-files", "-v", "-z"])?;
    for flag in nul_entries(&index_flags)? {
        if !flag.starts_with(b"H ") {
            return Err(GitError::Unsupported);
        }
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
        if let Some((scope, reason)) = exclusion(&path) {
            exclude(&mut exclusions, scope, reason);
        } else {
            included.push(path);
        }
    }
    if exclusions.len() > MAX_CAPTURE_FILES {
        return Err(GitError::Limit);
    }
    let mut attributes = Vec::new();
    // Small batches keep Windows command lines bounded, including long paths.
    for batch in included.chunks(16) {
        let mut command = args(&["check-attr", "-z", "--all", "--"]);
        command.extend(batch.iter().map(OsString::from));
        let observed = executor.run(root, &command)?.checked()?;
        let fields = nul_entries(&observed)?;
        if fields.len() % 3 != 0 {
            return Err(GitError::Process);
        }
        for triple in fields.chunks(3) {
            if matches!(triple[1], b"filter" | b"working-tree-encoding" | b"ident") {
                return Err(GitError::Unsupported);
            }
        }
        attributes.extend(observed);
    }
    let mut files = BTreeMap::new();
    let mut read_bytes = 0usize;
    for path in included {
        for entry in [base_entries.get(&path), index.get(&path)]
            .into_iter()
            .flatten()
        {
            if entry.mode != "100644" {
                return Err(GitError::Unsupported);
            }
        }
        let observed = read_working(root, &path, limits.file_bytes)?;
        if let Some(file) = &observed {
            if file.stamp.mode != "100644" {
                return Err(GitError::Unsupported);
            }
            read_bytes = read_bytes
                .checked_add(file.bytes.len())
                .ok_or(GitError::Limit)?;
            if read_bytes > limits.read_bytes {
                return Err(GitError::Limit);
            }
        }
        files.insert(path, observed);
    }
    let index_bytes = raw_index(executor, root)?;
    if initial_index != index_bytes {
        return Err(GitError::InconsistentCapture);
    }
    Ok(Snapshot {
        head,
        branch,
        base: base_entries,
        index,
        index_bytes,
        index_flags,
        untracked,
        exclusions,
        attributes,
        files,
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
    let first = inspect(executor, &root, &intent.base_oid, limits)?;
    let branch = std::str::from_utf8(&first.branch)
        .map_err(|_| GitError::Ownership)?
        .trim();
    if branch != format!("refs/heads/{}", intent.branch) {
        return Err(GitError::Ownership);
    }
    between()?;
    let second = inspect(executor, &root, &intent.base_oid, limits)?;
    if first != second {
        return Err(GitError::InconsistentCapture);
    }
    let mut cache = BTreeMap::new();
    let mut read_bytes = 0;
    let mut files = Vec::new();
    let mut patch = String::new();
    for (path, working) in &first.files {
        let base_entry = first.base.get(path);
        let index_entry = first.index.get(path);
        let base_bytes = base_entry
            .map(|entry| read_blob(executor, &root, entry, limits, &mut cache, &mut read_bytes))
            .transpose()?;
        let index_bytes = index_entry
            .map(|entry| read_blob(executor, &root, entry, limits, &mut cache, &mut read_bytes))
            .transpose()?;
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
    if inspect(executor, &root, &intent.base_oid, limits)? != first {
        return Err(GitError::InconsistentCapture);
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
    };
    artifact.revision = artifact.content_revision();
    let artifact_bytes = artifact
        .files
        .iter()
        .flat_map(|file| [&file.base, &file.index, &file.worktree])
        .flatten()
        .try_fold(artifact.patch.len(), |bytes, content| {
            bytes
                .checked_add(content.content.len())
                .ok_or(GitError::Limit)
        })?;
    if artifact_bytes > limits.artifact_bytes || !artifact.validate() {
        return Err(GitError::Limit);
    }
    Ok(artifact)
}
