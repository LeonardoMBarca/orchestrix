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
};

pub(crate) struct IntegrationGit {
    pub executor: GitExecutor,
    scope: Option<JobScope>,
}
impl IntegrationGit {
    pub fn new(scope: JobScope) -> GitResult<Self> {
        Ok(Self {
            executor: GitExecutor::scoped(scope.clone())?,
            scope: Some(scope),
        })
    }
    pub fn read_only() -> GitResult<Self> {
        Ok(Self {
            executor: GitExecutor::new()?,
            scope: None,
        })
    }
    fn require_scope(&self) -> GitResult<()> {
        if self.scope.is_none() {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
    fn service(&self) -> GitResult<WorktreeService> {
        Ok(WorktreeService::with_executor(match &self.scope {
            Some(scope) => GitExecutor::scoped(scope.clone())?,
            None => GitExecutor::new()?,
        }))
    }
    pub fn target(
        &self,
        intent: &IntegrationIntent,
        tree: &str,
        head: &str,
    ) -> GitResult<TargetState> {
        super::capture::clean_target(
            &self.executor,
            Path::new(&intent.target.directory),
            Path::new(&intent.target.common_git_dir),
            tree,
            head,
            &intent.target.symbolic_ref,
        )
    }
    pub fn inspect(&self, intent: &IntegrationIntent) -> GitResult<CapturedArtifact> {
        self.service()?
            .inspect_integration_owned(&intent.worktree)?;
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
    ) -> GitResult<CapturedArtifact> {
        self.require_scope()?;
        if source.base_oid != intent.input_base
            || !source.exclusions.is_empty()
            || source.files.iter().any(|file| file.base != file.index)
        {
            return Err(GitError::Unsupported);
        }
        self.service()?.create_integration_owned(&intent.worktree)?;
        let initial = self.inspect(intent)?;
        if !initial.files.is_empty() || !initial.exclusions.is_empty() {
            return Err(GitError::UnpreservedWork);
        }
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
        let snapshot = self.inspect(intent)?;
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
    ) -> Result<TargetState, E>
    where
        E: From<GitError>,
    {
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
        before_write()?;
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
pub(crate) fn initial_target(
    executor: &GitExecutor,
    root: &Path,
    common: &Path,
    base: &str,
) -> GitResult<TargetState> {
    let branch = text(
        executor
            .run(root, &args(&["symbolic-ref", "--quiet", "HEAD"]))?
            .checked()?,
    )?;
    if !branch.starts_with("refs/heads/") || branch.starts_with("refs/heads/orchestrix/") {
        return Err(GitError::Unsupported);
    }
    super::capture::clean_target(executor, root, common, base, base, &branch)
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
