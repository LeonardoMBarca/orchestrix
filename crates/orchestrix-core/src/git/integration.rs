//! Narrow Git effects for the single-CodeTask integration profile.
//! All subprocesses use the already persisted keeper scope; no shell or hooks.
use super::{CaptureLimits, GitError, GitExecutor, GitResult, WorktreeService, paths};
use crate::{
    git_records::{CapturedArtifact, content_hash},
    integration_records::{IntegrationIntent, TargetState, oid},
};
use orchestrix_process::named_job::JobScope;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

#[cfg(all(test, windows))]
mod diagnostic_tests {
    use super::*;
    use crate::{
        persistence::Store,
        recovery_guardian::{Guardian, GuardianBinding},
    };

    #[test]
    #[ignore = "Explicit owned failed-fixture database and helper; read-only Git target diagnosis"]
    fn measured_owned_preserved_target() {
        let database = std::path::PathBuf::from(
            std::env::var_os("ORCHESTRIX_OPAQUE_TARGET_DATABASE").expect("explicit own fixture"),
        );
        let helper = std::path::PathBuf::from(
            std::env::var_os("ORCHESTRIX_OPAQUE_TARGET_HELPER").expect("explicit built helper"),
        );
        let state = Store::open(&database).unwrap().load().unwrap();
        let intent = state.integrations()["opaque-integration"].intent.clone();
        assert!(intent.validate());
        let binding =
            GuardianBinding::prepare(&database, "owned-read-only-diagnostic", &helper).unwrap();
        let mut guardian = Guardian::start(binding, &helper).unwrap();
        super::super::executor::test_timing::begin();
        let started = std::time::Instant::now();
        let git = IntegrationGit::new(guardian.job().unwrap()).unwrap();
        let result = git.target(&intent, &intent.input_base, &intent.input_base);
        let samples = super::super::executor::test_timing::finish();
        println!(
            "OPAQUE_SCOPED_TARGET_TIMING={}",
            serde_json::json!({"elapsedMs":started.elapsed().as_millis(),"result":format!("{result:?}"),"samples":samples})
        );
        assert_eq!(result.unwrap(), intent.target);
    }
}

pub(crate) struct IntegrationGit {
    pub executor: Arc<GitExecutor>,
    service: WorktreeService,
    scope: Option<JobScope>,
}

/// Keep infrastructure failures typed until the service classifies their
/// authority/provenance; a caller's authorization error is a separate result.
pub(crate) enum ApplyFilesError<E> {
    Git(GitError),
    BeforeWrite(E),
}
impl<E> From<GitError> for ApplyFilesError<E> {
    fn from(error: GitError) -> Self {
        Self::Git(error)
    }
}

impl IntegrationGit {
    pub fn new(scope: JobScope) -> GitResult<Self> {
        let executor = Arc::new(GitExecutor::scoped(scope.clone())?);
        Ok(Self {
            service: WorktreeService::with_shared_executor(Arc::clone(&executor)),
            executor,
            scope: Some(scope),
        })
    }
    pub fn read_only() -> GitResult<Self> {
        let executor = Arc::new(GitExecutor::new()?);
        Ok(Self {
            service: WorktreeService::with_shared_executor(Arc::clone(&executor)),
            executor,
            scope: None,
        })
    }
    fn require_scope(&self) -> GitResult<()> {
        if self.scope.is_none() {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
    fn service(&self) -> &WorktreeService {
        &self.service
    }
    pub fn target(
        &self,
        intent: &IntegrationIntent,
        tree: &str,
        head: &str,
    ) -> GitResult<TargetState> {
        super::capture::clean_target_with_preservation(
            &self.executor,
            Path::new(&intent.target.directory),
            Path::new(&intent.target.common_git_dir),
            tree,
            head,
            &intent.target.symbolic_ref,
            intent.worktree.preservation.as_ref(),
        )
    }
    pub fn inspect(&self, intent: &IntegrationIntent) -> GitResult<CapturedArtifact> {
        self.service().inspect_integration_owned(&intent.worktree)?;
        super::capture(
            &self.executor,
            Path::new(&intent.worktree.path),
            &intent.worktree,
            CaptureLimits::default(),
        )
    }
    pub fn materialize(
        &self,
        intent: &IntegrationIntent,
        source: &CapturedArtifact,
        after_base_verified: impl FnOnce() -> GitResult<()>,
    ) -> GitResult<CapturedArtifact> {
        self.require_scope()?;
        if source.base_oid != intent.input_base
            || source.preservation != intent.worktree.preservation
            || !source.exclusions.is_empty()
            || source.files.iter().any(|file| file.base != file.index)
        {
            return Err(GitError::Unsupported);
        }
        let just_verified_base = crate::integration_diagnostics::measure(
            crate::integration_diagnostics::Phase::MaterializeBase,
            || self.service().create_integration_owned(&intent.worktree),
        )?;
        // A fresh v2 completion already follows the full coherent base check.
        // Replayed/existing work still requires the original empty-artifact
        // inspection. Every edited file is checked against its base below, and
        // the final full capture must match the exact source, including all
        // protected files; a concurrent change is preserved and refused.
        if intent.worktree.preservation.is_none() || !just_verified_base {
            let initial = self.inspect(intent)?;
            if !initial.files.is_empty() || !initial.exclusions.is_empty() {
                return Err(GitError::UnpreservedWork);
            }
        }
        after_base_verified()?;
        let root = Path::new(&intent.worktree.path);
        for file in &source.files {
            let path = root.join(&file.path);
            paths::reject_links(&path)?;
            match &file.base {
                Some(base) => {
                    if read_file(&path)? != base.content.as_bytes() {
                        return Err(GitError::Conflict);
                    }
                }
                None => {
                    if path.exists() {
                        return Err(GitError::Collision);
                    }
                }
            }
            if let Some(content) = &file.worktree {
                let parent = path.parent().ok_or(GitError::Path)?;
                let relative = parent.strip_prefix(root).map_err(|_| GitError::Scope)?;
                let mut current = root.to_path_buf();
                for component in relative.components() {
                    current.push(component);
                    paths::reject_links(&current)?;
                    if !current.exists() {
                        fs::create_dir(&current)?;
                    } else if !current.is_dir() {
                        return Err(GitError::Collision);
                    }
                }
                let mut options = fs::OpenOptions::new();
                options.write(true);
                if file.base.is_some() {
                    options.truncate(true);
                } else {
                    options.create_new(true);
                }
                let mut output = options.open(&path)?;
                output.write_all(content.content.as_bytes())?;
                output.sync_all()?;
            } else {
                fs::remove_file(&path)?;
            }
        }
        let snapshot = crate::integration_diagnostics::measure(
            crate::integration_diagnostics::Phase::MaterializeCapture,
            || self.inspect(intent),
        )?;
        if snapshot.exclusions != source.exclusions
            || snapshot
                .files
                .iter()
                .map(|f| (&f.path, &f.base, &f.index, &f.worktree))
                .collect::<Vec<_>>()
                != source
                    .files
                    .iter()
                    .map(|f| (&f.path, &f.base, &f.index, &f.worktree))
                    .collect::<Vec<_>>()
        {
            return Err(GitError::InconsistentCapture);
        }
        Ok(snapshot)
    }
    pub fn plan_commit(
        &self,
        intent: &IntegrationIntent,
        snapshot: &CapturedArtifact,
    ) -> GitResult<(String, String)> {
        self.require_scope()?;
        let root = Path::new(&intent.worktree.path);
        if intent.worktree.preservation.is_some() {
            let tree = self.preserved_tree(intent, snapshot, true)?;
            let result = object(
                &self.executor,
                root,
                "commit",
                &commit_body(intent, &tree),
                false,
            )?;
            return Ok((tree, result));
        }
        let mut files = super::worktrees::read_base(&self.executor, root, &intent.input_base)?;
        for file in &snapshot.files {
            match &file.worktree {
                Some(content) => {
                    files.insert(file.path.clone(), content.content.as_bytes().to_vec());
                }
                None => {
                    files.remove(&file.path);
                }
            }
        }
        let tree = write_tree(&self.executor, root, &files, 0, true)?;
        let body = commit_body(intent, &tree);
        let result = object(&self.executor, root, "commit", &body, false)?;
        Ok((tree, result))
    }
    pub fn verify_snapshot_tree(
        &self,
        intent: &IntegrationIntent,
        tree: &str,
        snapshot: &CapturedArtifact,
    ) -> GitResult<()> {
        if intent.worktree.preservation.is_some() {
            return if self.preserved_tree(intent, snapshot, false)? == tree {
                Ok(())
            } else {
                Err(GitError::Ownership)
            };
        }
        let root = Path::new(&intent.worktree.path);
        let mut files = super::worktrees::read_base(&self.executor, root, &intent.input_base)?;
        for file in &snapshot.files {
            match &file.worktree {
                Some(content) => {
                    files.insert(file.path.clone(), content.content.as_bytes().to_vec());
                }
                None => {
                    files.remove(&file.path);
                }
            }
        }
        if write_tree(&self.executor, root, &files, 0, false)? != tree {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
    fn preserved_tree(
        &self,
        intent: &IntegrationIntent,
        snapshot: &CapturedArtifact,
        write: bool,
    ) -> GitResult<String> {
        let manifest = intent
            .worktree
            .preservation
            .as_ref()
            .ok_or(GitError::Ownership)?;
        if snapshot.preservation.as_ref() != Some(manifest)
            || !snapshot.validate()
            || snapshot.base_oid != intent.input_base
        {
            return Err(GitError::Ownership);
        }
        // Verify the immutable base once; unchanged blobs use their native OID
        // directly and never become UTF-8 content in the integration pipeline.
        super::preservation::verify_base(
            &self.executor,
            Path::new(&intent.worktree.path),
            manifest,
        )?;
        let mut files = manifest
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.git_oid.clone()))
            .collect::<BTreeMap<_, _>>();
        for file in &snapshot.files {
            if manifest.editable_paths.binary_search(&file.path).is_err() {
                return Err(GitError::Scope);
            }
            match &file.worktree {
                Some(content) => {
                    files.insert(
                        file.path.clone(),
                        object(
                            &self.executor,
                            Path::new(&intent.worktree.path),
                            "blob",
                            content.content.as_bytes(),
                            write,
                        )?,
                    );
                }
                None => {
                    files.remove(&file.path);
                }
            }
        }
        write_oid_tree(
            &self.executor,
            Path::new(&intent.worktree.path),
            &files,
            0,
            write,
        )
    }
    pub fn commit(&self, intent: &IntegrationIntent, tree: &str, result: &str) -> GitResult<()> {
        self.require_scope()?;
        let root = Path::new(&intent.worktree.path);
        if object(
            &self.executor,
            root,
            "commit",
            &commit_body(intent, tree),
            true,
        )? != result
        {
            return Err(GitError::Ownership);
        }
        let name = result_ref(intent);
        if self
            .executor
            .run(root, &args(&["symbolic-ref", "--quiet", &name]))?
            .status
            == 0
        {
            return Err(GitError::Ownership);
        }
        let old = self
            .executor
            .run(root, &args(&["rev-parse", "--verify", "--quiet", &name]))?;
        if old.status == 0 {
            if text(old.stdout)? == result {
                return self.verify_commit(intent, tree, result);
            }
            return Err(GitError::Collision);
        }
        if old.status != 1 {
            return Err(GitError::Process);
        }
        self.executor
            .run(
                root,
                &args(&[
                    "update-ref",
                    "--no-deref",
                    "--create-reflog",
                    &name,
                    result,
                    &"0".repeat(result.len()),
                ]),
            )?
            .checked()?;
        self.verify_commit(intent, tree, result)
    }
    pub fn verify_commit(
        &self,
        intent: &IntegrationIntent,
        tree: &str,
        result: &str,
    ) -> GitResult<()> {
        let root = Path::new(&intent.worktree.path);
        let name = result_ref(intent);
        if self
            .executor
            .run(root, &args(&["symbolic-ref", "--quiet", &name]))?
            .status
            == 0
        {
            return Err(GitError::Ownership);
        }
        let observed = self
            .executor
            .run(root, &args(&["rev-parse", "--verify", "--quiet", &name]))?
            .checked()?;
        if text(observed)? != result
            || self
                .executor
                .run(root, &args(&["cat-file", "commit", result]))?
                .checked()?
                != commit_body(intent, tree)
        {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
    pub fn apply_files<E>(
        &self,
        intent: &IntegrationIntent,
        result: &str,
        before_write: impl FnOnce() -> Result<(), E>,
    ) -> Result<TargetState, ApplyFilesError<E>> {
        self.require_scope()?;
        if self.target(intent, &intent.input_base, &intent.input_base)? != intent.target {
            return Err(GitError::Conflict.into());
        }
        let root = Path::new(&intent.target.directory);
        // The exact approved bytes/index were checked above. A file edited and
        // restored to the same bytes can retain stale index stat information;
        // read-tree deliberately refuses that state. Refresh only stat data,
        // never stage content/mode changes, and recheck the logical fingerprint.
        self.executor
            .run(root, &args(&["update-index", "--refresh"]))?
            .checked()?;
        if self.target(intent, &intent.input_base, &intent.input_base)? != intent.target {
            return Err(GitError::Conflict.into());
        }
        // Refresh and coherent fingerprint reads can consume time. The service
        // must recheck authorization/cancellation at the actual byte-write edge.
        before_write().map_err(ApplyFilesError::BeforeWrite)?;
        // Two-tree fast-forward update checks for local changes. No reset,
        // clean, stash, cherry-pick, hooks, filters or project scripts.
        self.executor
            .run(
                root,
                &args(&["read-tree", "-m", "-u", &intent.input_base, result]),
            )?
            .checked()?;
        let partial = self.target(intent, result, &intent.input_base)?;
        if partial.symbolic_ref != intent.target.symbolic_ref {
            return Err(GitError::Conflict.into());
        }
        Ok(partial)
    }
    pub fn apply_ref(&self, intent: &IntegrationIntent, result: &str) -> GitResult<TargetState> {
        self.require_scope()?;
        self.target(intent, result, &intent.input_base)?;
        self.executor
            .run(
                Path::new(&intent.target.directory),
                &args(&[
                    "update-ref",
                    "--no-deref",
                    &intent.target.symbolic_ref,
                    result,
                    &intent.input_base,
                ]),
            )?
            .checked()?;
        self.target(intent, result, result)
    }
}
pub(crate) fn result_ref(intent: &IntegrationIntent) -> String {
    format!(
        "refs/heads/orchestrix/result-{}",
        content_hash(intent.id.as_bytes())
    )
}
pub(crate) fn commit_body(intent: &IntegrationIntent, tree: &str) -> Vec<u8> {
    format!("tree {tree}\nparent {}\nauthor Orchestrix <orchestrix@local.invalid> {} +0000\ncommitter Orchestrix <orchestrix@local.invalid> {} +0000\n\nOrchestrix serial integration {}\n",intent.input_base,intent.created_ms/1000,intent.created_ms/1000,intent.id).into_bytes()
}
pub(crate) fn initial_target_with_preservation(
    executor: &GitExecutor,
    root: &Path,
    common: &Path,
    base: &str,
    preservation: Option<&crate::git_records::BasePreservation>,
) -> GitResult<TargetState> {
    let branch = text(
        executor
            .run(root, &args(&["symbolic-ref", "--quiet", "HEAD"]))?
            .checked()?,
    )?;
    if !branch.starts_with("refs/heads/") || branch.starts_with("refs/heads/orchestrix/") {
        return Err(GitError::Unsupported);
    }
    super::capture::clean_target_with_preservation(
        executor,
        root,
        common,
        base,
        base,
        &branch,
        preservation,
    )
}
fn read_file(path: &Path) -> GitResult<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > 1_048_576 {
        return Err(GitError::Limit);
    }
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err(GitError::Limit);
    }
    Ok(bytes)
}
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
fn text(bytes: Vec<u8>) -> GitResult<String> {
    let value = String::from_utf8(bytes).map_err(|_| GitError::Process)?;
    let value = value.trim();
    if value.is_empty() {
        return Err(GitError::Process);
    }
    Ok(value.into())
}
fn object(
    executor: &GitExecutor,
    root: &Path,
    kind: &str,
    bytes: &[u8],
    write: bool,
) -> GitResult<String> {
    let mut arguments = args(&["hash-object", "-t", kind]);
    if write {
        arguments.push("-w".into());
    }
    arguments.push("--stdin".into());
    let value = text(
        executor
            .run_with_input(root, &arguments, bytes)?
            .checked()?,
    )?;
    if !oid(&value) {
        return Err(GitError::Process);
    }
    Ok(value)
}
fn write_tree(
    executor: &GitExecutor,
    root: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    depth: usize,
    write: bool,
) -> GitResult<String> {
    if depth > 32 {
        return Err(GitError::Limit);
    }
    let mut direct = BTreeMap::new();
    let mut directories: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    for (path, bytes) in files {
        if let Some((parent, tail)) = path.split_once('/') {
            directories
                .entry(parent.into())
                .or_default()
                .insert(tail.into(), bytes.clone());
        } else {
            direct.insert(path.clone(), object(executor, root, "blob", bytes, write)?);
        }
    }
    let mut entries = Vec::new();
    for (name, id) in direct {
        if directories.contains_key(&name) {
            return Err(GitError::Conflict);
        }
        entries.push((name, false, id));
    }
    for (name, files) in directories {
        entries.push((
            name,
            true,
            write_tree(executor, root, &files, depth + 1, write)?,
        ));
    }
    entries.sort_by(|a, b| {
        let ak = format!("{}{}", a.0, if a.1 { "/" } else { "\0" });
        let bk = format!("{}{}", b.0, if b.1 { "/" } else { "\0" });
        ak.as_bytes().cmp(bk.as_bytes())
    });
    let mut body = Vec::new();
    for (name, directory, id) in entries {
        body.extend_from_slice(
            format!("{} {name}\0", if directory { "40000" } else { "100644" }).as_bytes(),
        );
        for byte in id.as_bytes().as_chunks::<2>().0 {
            let pair = std::str::from_utf8(byte).map_err(|_| GitError::Process)?;
            body.push(u8::from_str_radix(pair, 16).map_err(|_| GitError::Process)?);
        }
    }
    object(executor, root, "tree", &body, write)
}

fn write_oid_tree(
    executor: &GitExecutor,
    root: &Path,
    files: &BTreeMap<String, String>,
    depth: usize,
    write: bool,
) -> GitResult<String> {
    if depth > 32 {
        return Err(GitError::Limit);
    }
    let mut direct = BTreeMap::new();
    let mut directories: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (path, id) in files {
        if !oid(id) {
            return Err(GitError::Ownership);
        }
        if let Some((name, tail)) = path.split_once('/') {
            directories
                .entry(name.into())
                .or_default()
                .insert(tail.into(), id.clone());
        } else {
            direct.insert(path.clone(), id.clone());
        }
    }
    let mut entries = Vec::new();
    for (name, id) in direct {
        if directories.contains_key(&name) {
            return Err(GitError::Conflict);
        }
        entries.push((name, false, id));
    }
    for (name, files) in directories {
        entries.push((
            name,
            true,
            write_oid_tree(executor, root, &files, depth + 1, write)?,
        ));
    }
    entries.sort_by(|a, b| {
        format!("{}{}", a.0, if a.1 { "/" } else { "\0" })
            .as_bytes()
            .cmp(format!("{}{}", b.0, if b.1 { "/" } else { "\0" }).as_bytes())
    });
    let mut bytes = Vec::new();
    for (name, directory, id) in entries {
        bytes.extend_from_slice(
            format!("{} {name}\0", if directory { "40000" } else { "100644" }).as_bytes(),
        );
        for pair in id.as_bytes().as_chunks::<2>().0 {
            bytes.push(
                u8::from_str_radix(
                    std::str::from_utf8(pair).map_err(|_| GitError::Process)?,
                    16,
                )
                .map_err(|_| GitError::Process)?,
            );
        }
    }
    object(executor, root, "tree", &bytes, write)
}

#[cfg(all(test, windows))]
mod diagnostics {
    use super::*;
    use crate::recovery_guardian::{Guardian, GuardianBinding, default_helper, read_proof};
    use serde_json::json;
    use std::{
        env,
        path::PathBuf,
        sync::{Arc, Condvar, Mutex},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[derive(serde::Serialize, PartialEq, Eq)]
    struct Stamp {
        directory: bool,
        bytes: u64,
        modified_nanos: u128,
        readonly: bool,
        sha256: Option<String>,
    }
    // A complete 54 MiB target, its independent packed Git store and a second
    // full integration worktree measured 138,787,246 bytes (1,401 entries).
    // This read-only diagnostic fingerprints their union, not one source base.
    // Keep its independent ceiling explicit; production capture/base budgets
    // and the 120-second diagnostic Job ceiling remain unchanged.
    const DIAGNOSTIC_INVENTORY_BYTES: usize = 160 * 1024 * 1024;
    fn fingerprint(roots: &[PathBuf]) -> BTreeMap<PathBuf, Stamp> {
        fn visit(path: &Path, files: &mut BTreeMap<PathBuf, Stamp>, bytes: &mut usize) {
            if files.contains_key(path) {
                return;
            }
            assert!(files.len() < 16_384, "diagnostic-inventory-limit");
            paths::reject_links(path).expect("diagnostic-link-check");
            let metadata = fs::symlink_metadata(path).expect("diagnostic-metadata");
            assert!(
                metadata.is_file() || metadata.is_dir(),
                "diagnostic-special-file"
            );
            let sha256 = if metadata.is_file() {
                *bytes = bytes
                    .checked_add(usize::try_from(metadata.len()).expect("diagnostic-size"))
                    .expect("diagnostic-size");
                assert!(
                    *bytes <= DIAGNOSTIC_INVENTORY_BYTES,
                    "diagnostic-inventory-bytes"
                );
                Some(content_hash(&fs::read(path).expect("diagnostic-read")))
            } else {
                None
            };
            files.insert(
                path.to_owned(),
                Stamp {
                    directory: metadata.is_dir(),
                    bytes: metadata.len(),
                    modified_nanos: metadata
                        .modified()
                        .expect("diagnostic-modified")
                        .duration_since(UNIX_EPOCH)
                        .expect("diagnostic-clock")
                        .as_nanos(),
                    readonly: metadata.permissions().readonly(),
                    sha256,
                },
            );
            if metadata.is_dir() {
                for entry in fs::read_dir(path).expect("diagnostic-directory") {
                    visit(&entry.expect("diagnostic-entry").path(), files, bytes);
                }
            }
        }
        let mut files = BTreeMap::new();
        let mut bytes = 0;
        for root in roots {
            visit(root, &mut files, &mut bytes);
        }
        files
    }
    fn code(error: &GitError) -> &'static str {
        match error {
            GitError::Io => "io",
            GitError::Path => "path",
            GitError::Scope => "scope",
            GitError::Link => "link",
            GitError::Collision => "collision",
            GitError::Unsupported => "unsupported",
            GitError::NotRepository => "not-repository",
            GitError::Base => "base",
            GitError::Ownership => "ownership",
            GitError::MissingAttempt => "missing-attempt",
            GitError::Conflict => "conflict",
            GitError::Process => "process",
            GitError::Deadline => "deadline",
            GitError::OutputLimit => "output-limit",
            GitError::Limit => "limit",
            GitError::InconsistentCapture => "inconsistent-capture",
            GitError::SecretContent => "secret-content",
            GitError::Pending => "pending",
            GitError::UnpreservedWork => "unpreserved-work",
            GitError::Injected => "injected",
            GitError::Storage(_) => "storage",
        }
    }

    /// Runs the read-only validation phases, never approve or any persisted
    /// operation. Historical keeper bindings in the export grant no authority.
    #[test]
    #[ignore = "requires an authorized exact Prepared record export and its SHA-256"]
    fn prepared_validation_from_readonly_export() {
        use crate::integration_records::{IntegrationRecord, IntegrationStatus};

        let input = PathBuf::from(
            env::var_os("ORCHESTRIX_DIAGNOSTIC_RECORD").expect("diagnostic-record-opt-in"),
        );
        paths::reject_links(&input).expect("diagnostic-record-link");
        let input = fs::canonicalize(input).expect("diagnostic-record-path");
        let expected =
            env::var("ORCHESTRIX_DIAGNOSTIC_RECORD_SHA256").expect("diagnostic-record-sha-opt-in");
        assert!(
            expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "diagnostic-record-sha"
        );
        let bytes = fs::read(&input).expect("diagnostic-record-read");
        assert!(bytes.len() <= 1_048_576, "diagnostic-record-limit");
        let export_hash = content_hash(&bytes);
        assert!(
            export_hash == expected.to_ascii_lowercase(),
            "diagnostic-record-hash"
        );
        let record: IntegrationRecord =
            serde_json::from_slice(&bytes).expect("diagnostic-record-schema");
        assert!(
            record.validate() && record.status() == IntegrationStatus::Prepared,
            "diagnostic-prepared-record"
        );
        let snapshot = record.snapshot().expect("diagnostic-snapshot");
        assert!(snapshot.validate(), "diagnostic-snapshot-validation");
        let tree = record
            .detail("treeRevision")
            .and_then(serde_json::Value::as_str)
            .filter(|value| oid(value))
            .expect("diagnostic-tree");
        let result = record
            .detail("resultRevision")
            .and_then(serde_json::Value::as_str)
            .filter(|value| oid(value))
            .expect("diagnostic-result");
        let fixture = input.parent().expect("diagnostic-fixture-root");
        let roots = [
            &record.intent.target.directory,
            &record.intent.target.common_git_dir,
            &record.intent.worktree.path,
        ]
        .map(|path| {
            let root = paths::canonical_directory(Path::new(path)).expect("diagnostic-root");
            assert!(
                root.starts_with(fixture) && root != fixture,
                "diagnostic-root-outside-fixture"
            );
            root
        });
        let before = fingerprint(&roots);
        let started = Instant::now();
        let mut phases = Vec::new();
        let mut failure = None;
        let mut git = None;
        macro_rules! phase {
            ($name:literal, $operation:expr) => {
                if failure.is_none() {
                    super::super::executor::test_timing::begin();
                    let began = Instant::now();
                    let outcome: GitResult<()> = $operation;
                    let elapsed_micros = began.elapsed().as_micros();
                    let samples = super::super::executor::test_timing::finish();
                    phases.push(json!({
                        "phase":$name,"elapsedMicros":elapsed_micros,
                        "succeeded":outcome.is_ok(),"gitError":outcome.as_ref().err().map(code),
                        "gitCallCount":samples.len(),"timings":samples
                    }));
                    if outcome.is_err() {
                        failure = Some($name);
                    }
                }
            };
        }
        phase!(
            "read-only-transport",
            match IntegrationGit::read_only() {
                Ok(value) => {
                    git = Some(value);
                    Ok(())
                }
                Err(error) => Err(error),
            }
        );
        phase!(
            "inspect-snapshot",
            git.as_ref()
                .expect("diagnostic-transport")
                .inspect(&record.intent)
                .and_then(|actual| {
                    if actual.revision == snapshot.revision {
                        Ok(())
                    } else {
                        Err(GitError::InconsistentCapture)
                    }
                })
        );
        phase!(
            "verify-commit",
            git.as_ref()
                .expect("diagnostic-transport")
                .verify_commit(&record.intent, tree, result)
        );
        phase!(
            "verify-snapshot-tree",
            git.as_ref()
                .expect("diagnostic-transport")
                .verify_snapshot_tree(&record.intent, tree, &snapshot)
        );
        phase!(
            "target-snapshot",
            git.as_ref()
                .expect("diagnostic-transport")
                .target(
                    &record.intent,
                    &record.intent.input_base,
                    &record.intent.input_base
                )
                .and_then(|actual| {
                    if actual == record.intent.target {
                        Ok(())
                    } else {
                        Err(GitError::Conflict)
                    }
                })
        );
        drop(git);
        let elapsed = started.elapsed().as_micros();
        let after = fingerprint(&roots);
        let stable = before == after;
        let directory = fixture.join(format!(
            "readonly-validation-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("diagnostic-clock")
                .as_nanos(),
            std::process::id()
        ));
        fs::create_dir(&directory).expect("diagnostic-new-output");
        let report = json!({
            "schemaVersion":1,"diagnosticOnly":true,"exportSha256":export_hash,
            "storeOpened":false,"runtimeCalls":0,"approveCalled":false,
            "newKeeperCreated":false,"historicalKeeperReused":false,
            "commandDeadlineMs":15000,"commandOutputLimitBytes":8388608,
            "elapsedMicros":elapsed,"phases":phases,"failedPhase":failure,
            "physicalStateUnchanged":stable,"fingerprintedEntries":before.len(),
            "beforeSha256":content_hash(&serde_json::to_vec(&before).expect("diagnostic-hash")),
            "afterSha256":content_hash(&serde_json::to_vec(&after).expect("diagnostic-hash")),
            "sourceHashes":{
                "gitExecutor":content_hash(include_bytes!("executor.rs")),
                "gitIntegration":content_hash(include_bytes!("integration.rs")),
                "gitCapture":content_hash(include_bytes!("capture.rs")),
                "gitWorktrees":content_hash(include_bytes!("worktrees.rs"))
            }
        });
        fs::write(
            directory.join("result.json"),
            serde_json::to_vec_pretty(&report).expect("diagnostic-report-json"),
        )
        .expect("diagnostic-report-write");
        println!(
            "Read-only Prepared validation diagnostic: {}",
            directory.display()
        );
        assert!(stable, "diagnostic-physical-state-changed");
        assert!(
            failure.is_none(),
            "diagnostic-validation-failed; see static phase report"
        );
    }

    /// Explicit, read-only timing of one existing owned integration checkout.
    /// No Store, reservation, materialize, result ref, check or reconciliation.
    #[test]
    #[ignore = "requires an explicitly authorized readonly intent export and its SHA-256"]
    fn scoped_inspect_from_readonly_export() {
        let global_started = Instant::now();
        let input = fs::canonicalize(
            env::var_os("ORCHESTRIX_DIAGNOSTIC_INTENT").expect("diagnostic-intent-opt-in"),
        )
        .expect("diagnostic-intent-path");
        let expected =
            env::var("ORCHESTRIX_DIAGNOSTIC_INTENT_SHA256").expect("diagnostic-intent-sha-opt-in");
        assert!(
            expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "diagnostic-intent-sha"
        );
        let body = fs::read(&input).expect("diagnostic-intent-read");
        assert!(body.len() <= 1_048_576, "diagnostic-intent-limit");
        let intent_hash = content_hash(&body);
        assert!(
            intent_hash == expected.to_ascii_lowercase(),
            "diagnostic-intent-hash"
        );
        let parsed = serde_json::from_slice::<IntegrationIntent>(&body);
        assert!(parsed.is_ok(), "diagnostic-intent-schema");
        let intent = parsed.expect("validated intent");
        assert!(intent.validate(), "diagnostic-intent-validation");
        let fixture = input.parent().expect("diagnostic-fixture-root");
        let roots = [
            &intent.target.directory,
            &intent.target.common_git_dir,
            &intent.worktree.path,
        ]
        .map(|root| {
            let root = fs::canonicalize(root).expect("diagnostic-root");
            assert!(
                root.starts_with(fixture) && root != fixture,
                "diagnostic-root-outside-fixture"
            );
            root
        });
        let before = fingerprint(&roots);
        let repository = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("diagnostic-repository-root");
        let base = repository.join("artifacts/diagnostics");
        fs::create_dir_all(&base).expect("diagnostic-output-parent");
        let out = base.join(format!(
            "scoped-integration-inspect-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("diagnostic-clock")
                .as_nanos(),
            std::process::id()
        ));
        fs::create_dir(&out).expect("diagnostic-output-new");
        let helper = default_helper().expect("diagnostic-helper");
        let binding = GuardianBinding::prepare(
            &out.join("not-created.sqlite"),
            "readonly-integration-diagnostic",
            &helper,
        )
        .expect("diagnostic-binding");
        let setup = Instant::now();
        let mut guardian =
            Guardian::start(binding.clone(), &helper).expect("diagnostic-owned-guardian");
        let scope = guardian.job().expect("diagnostic-own-job");
        let git = IntegrationGit::new(scope.clone()).expect("diagnostic-scoped-git");
        let setup_micros = setup.elapsed().as_micros();
        // The diagnostic's own global ceiling is independent of the unchanged
        // production keeper/Git deadlines. Only this newly created Job can be
        // interrupted; no existing integration scope is opened or reused.
        let completed = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = completed.clone();
        let remaining = Duration::from_secs(120).saturating_sub(global_started.elapsed());
        let watchdog = thread::spawn(move || {
            let (lock, wake) = &*signal;
            let (finished, timeout) = wake
                .wait_timeout_while(
                    lock.lock().expect("diagnostic-watchdog-lock"),
                    remaining,
                    |finished| !*finished,
                )
                .expect("diagnostic-watchdog-wait");
            if timeout.timed_out() && !*finished {
                let _ = scope.terminate();
                true
            } else {
                false
            }
        });
        super::super::executor::test_timing::begin();
        let trace = crate::integration_diagnostics::PreparationTrace::new(&intent.id);
        let started = Instant::now();
        let result = trace.measure(
            crate::integration_diagnostics::Phase::ValidateInspect,
            None,
            || git.inspect(&intent),
        );
        let inspect_micros = started.elapsed().as_micros();
        let samples = super::super::executor::test_timing::finish();
        let (lock, wake) = &*completed;
        *lock.lock().expect("diagnostic-watchdog-lock") = true;
        wake.notify_one();
        let diagnostic_interrupted = watchdog.join().expect("diagnostic-watchdog-join");
        drop(git);
        drop(guardian);
        let proof = read_proof(&binding).expect("diagnostic-local-closure");
        let after = fingerprint(&roots);
        let stable = before == after;
        let mut totals: BTreeMap<&str, (usize, u128)> = BTreeMap::new();
        for sample in &samples {
            let aggregate = totals.entry(sample.builtin).or_default();
            aggregate.0 += 1;
            aggregate.1 += sample.elapsed_micros;
        }
        let before_hash =
            content_hash(&serde_json::to_vec(&before).expect("diagnostic-inventory-hash"));
        let after_hash =
            content_hash(&serde_json::to_vec(&after).expect("diagnostic-inventory-hash"));
        let report = json!({"schemaVersion":1,"diagnosticOnly":true,"integrationStateChanged":false,"providerCalls":0,"storeOpened":false,"inspectCalls":1,"intentSha256":intent_hash,"setupMicros":setup_micros,"inspectMicros":inspect_micros,"diagnosticCeilingMs":120000,"diagnosticInterrupted":diagnostic_interrupted,"gitCallCount":samples.len(),"timings":samples,"byBuiltin":totals,"inspectionSucceeded":result.is_ok(),"gitError":result.as_ref().err().map(code),"returnedRevision":result.as_ref().ok().map(|artifact|&artifact.revision),"physicalStateUnchanged":stable,"fingerprintedEntries":before.len(),"beforeSha256":before_hash,"afterSha256":after_hash,"keeper":{"closureTrigger":proof["body"]["closureTrigger"],"localJobEmpty":proof["body"]["localJobEmpty"],"observedAfterIssuedMs":proof["body"]["observedAtMs"].as_u64().and_then(|now|now.checked_sub(binding.issued_at_ms)),"lifetimeMs":binding.lifetime_ms},"sourceHashes":{"gitExecutor":content_hash(include_bytes!("executor.rs")),"gitIntegration":content_hash(include_bytes!("integration.rs")),"gitCapture":content_hash(include_bytes!("capture.rs")),"gitWorktrees":content_hash(include_bytes!("worktrees.rs"))}});
        fs::write(
            out.join("result.json"),
            serde_json::to_vec_pretty(&report).expect("diagnostic-report-json"),
        )
        .expect("diagnostic-report-write");
        println!("Read-only scoped inspection diagnostic: {}", out.display());
        assert!(stable, "diagnostic-physical-state-changed");
        assert!(
            proof["body"]["localJobEmpty"] == true,
            "diagnostic-owned-job-not-empty"
        );
        assert!(
            result.is_ok(),
            "diagnostic-inspection-failed; see static result code"
        );
    }
}
