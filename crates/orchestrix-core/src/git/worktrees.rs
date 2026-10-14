//! Recoverable Git effects around short SQLite transactions.
use super::{CaptureLimits, GitError, GitExecutor, GitResult, capture, paths};
use crate::domain::*;
use crate::git_records::{
    CapturedArtifact, GitCommand, MAX_FILE_BYTES, WorktreeIntent, WorktreeObservation,
    WorktreeRecord, WorktreeState, content_hash, relative_file_path,
};
use crate::persistence::{Command, Disposition, FaultPoint, Request, Store};
use crate::persistence_privacy::sanitize_text;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NONCE: AtomicU64 = AtomicU64::new(0);

/// The host supplies directory authority separately from opaque domain references.
pub struct CreateWorktree {
    pub scope: LocalRef,
    pub key: LocalRef,
    pub expected_revision: u64,
    pub project: ProjectId,
    pub repository: RepositoryId,
    pub run: RunId,
    pub task: TaskId,
    pub attempt: AttemptId,
    pub directory_reference: LocalRef,
    pub worktree_reference: LocalRef,
    pub repository_path: PathBuf,
    pub authorized_directory: PathBuf,
    /// Full commit object name, never HEAD, a tag, abbreviation or moving branch.
    pub base_commit: String,
}

/// Test seams leave owned resources for the same normal recovery path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitFaultPoint {
    None,
    AfterIntent,
    AfterRefs,
    AfterWorktree,
    CreateConfirmationRollback,
    AfterWorktreeRemoval,
    AfterRemoval,
    RemovalConfirmationRollback,
}

pub struct WorktreeService {
    executor: GitExecutor,
}
impl WorktreeService {
    pub fn new() -> GitResult<Self> {
        Ok(Self {
            executor: GitExecutor::new()?,
        })
    }

    pub fn create(&self, store: &mut Store, request: &CreateWorktree) -> GitResult<WorktreeRecord> {
        self.create_with_fault(store, request, GitFaultPoint::None)
    }
    pub fn create_with_fault(
        &self,
        store: &mut Store,
        request: &CreateWorktree,
        fault: GitFaultPoint,
    ) -> GitResult<WorktreeRecord> {
        let intent = self.intent(store, request)?;
        let plan = Request {
            scope: request.scope.clone(),
            key: request.key.clone(),
            expected_revision: request.expected_revision,
            command: Command::Git(GitCommand::Plan(intent.clone())),
        };
        applied(store.execute(&plan)?)?;
        if fault == GitFaultPoint::AfterIntent {
            return Err(GitError::Injected);
        }
        let current = record(store, &intent.operation_id)?;
        if matches!(
            current.state,
            WorktreeState::CleanupPlanned | WorktreeState::Removed
        ) {
            return Err(GitError::Conflict);
        }
        let repo = Path::new(&intent.main_repository);
        self.executor.ensure_supported(repo)?;
        self.validate_main(&intent)?;
        let material = read_base(&self.executor, repo, &intent.base_oid)?;
        self.ensure_refs(&intent)?;
        if fault == GitFaultPoint::AfterRefs {
            return Err(GitError::Injected);
        }
        let target = Path::new(&intent.path);
        if !path_exists(target)? {
            self.validate_target(&intent, false)?;
            let registrations = self.registrations(&intent)?;
            if registrations
                .iter()
                .any(|row| row.branch.as_deref() == Some(&format!("refs/heads/{}", intent.branch)))
            {
                return Err(GitError::Ownership);
            }
            let reason = lock_reason(&intent);
            let args = vec![
                "worktree".into(),
                "add".into(),
                "--quiet".into(),
                "--no-checkout".into(),
                "--lock".into(),
                "--reason".into(),
                reason.into(),
                paths::git_path(target)?,
                intent.branch.clone().into(),
            ];
            self.executor.run(repo, &args)?.checked()?;
            // No checkout, hooks, filters, reset, stash or clean: raw blob bytes
            // are created once inside the newly registered owned directory.
            self.inspect_registration(&intent, true)?;
            materialize(target, &material)?;
            self.executor
                .run(target, &args_of(&["read-tree", &intent.base_oid]))?
                .checked()?;
        }
        self.inspect_registration(&intent, true)?;
        // The retained completion ref distinguishes a finished materialization
        // from a partially created directory. Later edits are never overwritten.
        if !self.materialized(&intent)? {
            if current.state != WorktreeState::Planned {
                return Err(GitError::Ownership);
            }
            verify_base(&self.executor, target, &intent, &material)?;
            self.mark_materialized(&intent)?;
        }
        if fault == GitFaultPoint::AfterWorktree {
            return Err(GitError::Injected);
        }
        if current.state == WorktreeState::Planned {
            let observation = observation(
                "created",
                &intent,
                &json!({"head":intent.base_oid,"files":material.keys().collect::<Vec<_>>()}),
            );
            commit_phase(
                store,
                &request.scope,
                &intent.operation_id,
                "created",
                GitCommand::ConfirmCreated {
                    operation_id: intent.operation_id.clone(),
                    observation,
                },
                if fault == GitFaultPoint::CreateConfirmationRollback {
                    FaultPoint::AfterResult
                } else {
                    FaultPoint::None
                },
            )?;
        }
        record(store, &intent.operation_id)
    }

    pub fn inspect(&self, store: &Store, operation_id: &str) -> GitResult<WorktreeRecord> {
        let record = record(store, operation_id)?;
        self.validate_main(&record.intent)?;
        self.verify_refs(&record.intent, record.state == WorktreeState::Removed)?;
        if record.state == WorktreeState::Removed {
            if path_exists(Path::new(&record.intent.path))?
                || self
                    .registrations(&record.intent)?
                    .iter()
                    .any(|row| same_path(&row.path, Path::new(&record.intent.path)))
            {
                return Err(GitError::Ownership);
            }
        } else {
            if record.state != WorktreeState::Planned && !self.materialized(&record.intent)? {
                return Err(GitError::Ownership);
            }
            self.inspect_registration(
                &record.intent,
                record.state != WorktreeState::CleanupPlanned,
            )?;
        }
        Ok(record)
    }

    /// Capture is an immutable Git fact; no Task approval, checks or integration.
    pub fn capture(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
    ) -> GitResult<CapturedArtifact> {
        self.capture_with_observer(
            store,
            scope,
            key,
            expected_revision,
            operation_id,
            || Ok(()),
        )
    }
    #[doc(hidden)]
    pub fn capture_with_observer<F: FnOnce() -> GitResult<()>>(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
        between: F,
    ) -> GitResult<CapturedArtifact> {
        // Replaying a committed capture returns its immutable evidence, even if
        // the directory has since changed or been removed. Re-submit the exact
        // command so the Store still enforces scope/key/body/revision identity.
        if let Some(result) = store.command_result(scope, key)? {
            let receipt = result.output.get("gitArtifact").ok_or(GitError::Conflict)?;
            if result.disposition != Disposition::Applied
                || receipt.get("operationId").and_then(|v| v.as_str()) != Some(operation_id)
            {
                return Err(GitError::Conflict);
            }
            let id = receipt
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or(GitError::Conflict)?;
            let revision = receipt
                .get("revision")
                .and_then(|v| v.as_str())
                .ok_or(GitError::Conflict)?;
            let artifact = store
                .load()?
                .captured_artifacts()
                .get(&(id.into(), revision.into()))
                .cloned()
                .ok_or(GitError::Conflict)?;
            applied(store.execute(&Request {
                scope: scope.clone(),
                key: key.clone(),
                expected_revision,
                command: Command::Git(GitCommand::RecordArtifact(artifact.clone())),
            })?)?;
            return Ok(artifact);
        }
        let current = record(store, operation_id)?;
        if current.state != WorktreeState::Created {
            return Err(GitError::Pending);
        }
        self.inspect(store, operation_id)?;
        let artifact = super::capture_with_observer(
            &self.executor,
            Path::new(&current.intent.path),
            &current.intent,
            CaptureLimits::default(),
            between,
        )?;
        let request = Request {
            scope: scope.clone(),
            key: key.clone(),
            expected_revision,
            command: Command::Git(GitCommand::RecordArtifact(artifact.clone())),
        };
        applied(store.execute(&request)?)?;
        Ok(artifact)
    }

    /// Conservative policy: only the exact clean pinned base can be removed.
    /// Changed, untracked, ignored, excluded or uniquely committed work is kept.
    pub fn remove(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
    ) -> GitResult<WorktreeRecord> {
        self.remove_with_fault(
            store,
            scope,
            key,
            expected_revision,
            operation_id,
            GitFaultPoint::None,
        )
    }
    pub fn remove_with_fault(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
        fault: GitFaultPoint,
    ) -> GitResult<WorktreeRecord> {
        let current = record(store, operation_id)?;
        let intent = &current.intent;
        self.validate_main(intent)?;
        if current.state == WorktreeState::Planned {
            return Err(GitError::Pending);
        }
        if current.state != WorktreeState::Removed {
            require_inactive_attempt(store, intent)?;
        }
        if current.state != WorktreeState::Removed && path_exists(Path::new(&intent.path))? {
            self.require_removable(intent, current.state == WorktreeState::Created)?;
        } else if !matches!(
            current.state,
            WorktreeState::CleanupPlanned | WorktreeState::Removed
        ) {
            return Err(GitError::Ownership);
        }
        let cleanup_observation = observation(
            "cleanup-planned",
            intent,
            &json!({"policy":"clean-pinned-base-only","baseAnchor":base_ref(intent)}),
        );
        let request = Request {
            scope: scope.clone(),
            key: key.clone(),
            expected_revision,
            command: Command::Git(GitCommand::PlanCleanup {
                operation_id: operation_id.into(),
                observation: cleanup_observation,
            }),
        };
        if current.state == WorktreeState::Removed {
            if store.command_result(scope, key)?.is_none() {
                return Err(GitError::Conflict);
            }
            applied(store.execute(&request)?)?;
            self.inspect(store, operation_id)?;
            return Ok(current);
        }
        applied(store.execute(&request)?)?;
        if path_exists(Path::new(&intent.path))? {
            // Revalidate after committing intent and immediately before removal.
            require_inactive_attempt(store, intent)?;
            self.require_removable(intent, false)?;
            let (registered, _) = self.inspect_registration(intent, false)?;
            if registered.locked.as_deref() == Some(&lock_reason(intent)) {
                self.executor
                    .run(
                        Path::new(&intent.main_repository),
                        &[
                            "worktree".into(),
                            "unlock".into(),
                            paths::git_path(Path::new(&intent.path))?,
                        ],
                    )?
                    .checked()?;
            }
            require_inactive_attempt(store, intent)?;
            self.require_removable(intent, false)?;
            self.executor
                .run(
                    Path::new(&intent.main_repository),
                    &[
                        "worktree".into(),
                        "remove".into(),
                        paths::git_path(Path::new(&intent.path))?,
                    ],
                )?
                .checked()?;
        }
        if path_exists(Path::new(&intent.path))?
            || self
                .registrations(intent)?
                .iter()
                .any(|row| same_path(&row.path, Path::new(&intent.path)))
        {
            return Err(GitError::Pending);
        }
        if fault == GitFaultPoint::AfterWorktreeRemoval {
            return Err(GitError::Injected);
        }
        self.verify_refs(intent, true)?;
        let branch = format!("refs/heads/{}", intent.branch);
        if self.ref_value(intent, &branch)?.is_some() {
            // Recovery may reach this step after the worktree disappeared.
            // Its surviving branch log can still be the only commit reference.
            require_base_reflog(
                &Path::new(&intent.common_git_dir).join("logs").join(&branch),
                &intent.base_oid,
                true,
            )?;
            // CAS delete only our direct ref. Base and ownership anchors remain.
            self.executor
                .run(
                    Path::new(&intent.main_repository),
                    &args_of(&["update-ref", "--no-deref", "-d", &branch, &intent.base_oid]),
                )?
                .checked()?;
        }
        if fault == GitFaultPoint::AfterRemoval {
            return Err(GitError::Injected);
        }
        let observation = observation(
            "removed",
            intent,
            &json!({"baseAnchor":base_ref(intent),"ownerAnchor":owner_ref(intent)}),
        );
        commit_phase(
            store,
            scope,
            operation_id,
            "removed",
            GitCommand::ConfirmRemoved {
                operation_id: operation_id.into(),
                observation,
            },
            if fault == GitFaultPoint::RemovalConfirmationRollback {
                FaultPoint::AfterResult
            } else {
                FaultPoint::None
            },
        )?;
        record(store, operation_id)
    }

    fn intent(&self, store: &Store, request: &CreateWorktree) -> GitResult<WorktreeIntent> {
        let state = store.load()?;
        let project = state
            .projects()
            .get(&request.project)
            .ok_or(GitError::MissingAttempt)?;
        let attempt = state
            .runs()
            .get(&request.run)
            .and_then(|r| r.task(&request.task).ok())
            .and_then(|t| t.attempts().iter().find(|a| a.id() == &request.attempt))
            .ok_or(GitError::MissingAttempt)?;
        if project.repository().id() != &request.repository
            || project.repository().location() != &request.directory_reference
            || attempt.inputs().project.as_ref() != Some(&request.project)
            || attempt.inputs().worktree != request.worktree_reference
            || attempt.inputs().base.as_str() != request.base_commit
        {
            return Err(GitError::Ownership);
        }
        let repo = paths::canonical_directory(&request.repository_path)?;
        let root = paths::canonical_directory(&request.authorized_directory)?;
        // Attempts live outside the main checkout, so its untracked files stay untouched.
        if repo == root || paths::strict_descendant(&repo, &root) {
            return Err(GitError::Scope);
        }
        self.executor.ensure_supported(&repo)?;
        let top = path_output(
            self.executor
                .run(&repo, &args_of(&["rev-parse", "--show-toplevel"]))?
                .checked()?,
        )?;
        if paths::canonical_directory(&top)? != repo {
            return Err(GitError::NotRepository);
        }
        if self
            .executor
            .run(&repo, &args_of(&["rev-parse", "--is-bare-repository"]))?
            .checked()?
            != b"false\n"
        {
            return Err(GitError::Unsupported);
        }
        if !valid_oid(&request.base_commit) {
            return Err(GitError::Base);
        }
        let resolved = text(
            self.executor
                .run(
                    &repo,
                    &args_of(&[
                        "rev-parse",
                        "--verify",
                        "--end-of-options",
                        &format!("{}^{{commit}}", request.base_commit),
                    ]),
                )?
                .checked()?,
        )?;
        if resolved != request.base_commit {
            return Err(GitError::Base);
        }
        let common = path_output(
            self.executor
                .run(
                    &repo,
                    &args_of(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
                )?
                .checked()?,
        )?;
        let common = paths::canonical_directory(&common)?;
        let hash = content_hash(
            json!([
                request.project.as_str(),
                request.repository.as_str(),
                request.run.as_str(),
                request.task.as_str(),
                request.attempt.as_str()
            ])
            .to_string()
            .as_bytes(),
        );
        let operation_id = format!("wt-{hash}");
        let target = root.join(format!("attempt-{}", &hash[..40]));
        paths::reject_links(&target)?;
        if repo == target
            || paths::strict_descendant(&target, &repo)
            || paths::strict_descendant(&repo, &target)
        {
            return Err(GitError::Scope);
        }
        // A linked checkout can have its common metadata outside `repo`.
        // Explicit directory authority never grants permission to nest an
        // attempt in that metadata or in another checkout of this repository.
        if root == common
            || paths::strict_descendant(&common, &root)
            || target == common
            || paths::strict_descendant(&common, &target)
            || paths::strict_descendant(&target, &common)
        {
            return Err(GitError::Scope);
        }
        let registrations = parse_registrations(
            &self
                .executor
                .run(&repo, &args_of(&["worktree", "list", "--porcelain", "-z"]))?
                .checked()?,
        )?;
        for registration in registrations {
            if !path_exists(&registration.path)? {
                continue;
            }
            let checkout = paths::canonical_directory(&registration.path)?;
            // The matching target is checked against the durable ownership
            // token below; other registered checkouts cannot contain it.
            if checkout != target
                && (paths::strict_descendant(&checkout, &target)
                    || paths::strict_descendant(&target, &checkout))
            {
                return Err(GitError::Scope);
            }
        }
        let token = state
            .worktrees()
            .get(&operation_id)
            .map(|r| r.intent.ownership_token.clone())
            .unwrap_or_else(|| {
                content_hash(
                    format!(
                        "{}:{}:{}:{hash}",
                        std::process::id(),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0),
                        NONCE.fetch_add(1, Ordering::Relaxed)
                    )
                    .as_bytes(),
                )
            });
        let intent = WorktreeIntent {
            operation_id: operation_id.clone(),
            project_id: request.project.as_str().into(),
            repository_id: request.repository.as_str().into(),
            run_id: request.run.as_str().into(),
            task_id: request.task.as_str().into(),
            attempt_id: request.attempt.as_str().into(),
            base_oid: resolved,
            domain_base: attempt.inputs().base.as_str().into(),
            worktree_ref: request.worktree_reference.as_str().into(),
            authorized_root: path_string(&root)?,
            path: path_string(&target)?,
            main_repository: path_string(&repo)?,
            branch: format!("orchestrix/attempt-{hash}"),
            common_git_dir: path_string(&common)?,
            ownership_token: token,
        };
        if !intent.validate() {
            return Err(GitError::Path);
        }
        if let Some(previous) = state.worktrees().get(&operation_id)
            && previous.intent != intent
        {
            return Err(GitError::Ownership);
        }
        Ok(intent)
    }
    fn validate_target(&self, intent: &WorktreeIntent, existing: bool) -> GitResult<()> {
        let resolved = paths::authorized_child(
            Path::new(&intent.authorized_root),
            Path::new(&intent.path),
            existing,
        )?;
        if path_string(&resolved)? != intent.path {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
    fn validate_main(&self, intent: &WorktreeIntent) -> GitResult<()> {
        let repo = paths::canonical_directory(Path::new(&intent.main_repository))?;
        if path_string(&repo)? != intent.main_repository {
            return Err(GitError::Ownership);
        }
        self.executor.ensure_supported(&repo)?;
        let common = path_output(
            self.executor
                .run(
                    &repo,
                    &args_of(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
                )?
                .checked()?,
        )?;
        if paths::canonical_directory(&common)? != Path::new(&intent.common_git_dir) {
            return Err(GitError::Ownership);
        }
        for relative in [
            format!("refs/heads/{}", intent.branch),
            owner_ref(intent),
            base_ref(intent),
            materialized_ref(intent),
        ] {
            paths::reject_links(&Path::new(&intent.common_git_dir).join(&relative))?;
            paths::reject_links(
                &Path::new(&intent.common_git_dir)
                    .join("logs")
                    .join(relative),
            )?;
        }
        Ok(())
    }
    fn ref_value(&self, intent: &WorktreeIntent, name: &str) -> GitResult<Option<String>> {
        let repo = Path::new(&intent.main_repository);
        if self
            .executor
            .run(repo, &args_of(&["symbolic-ref", "--quiet", name]))?
            .status
            == 0
        {
            return Err(GitError::Ownership);
        }
        let output = self
            .executor
            .run(repo, &args_of(&["rev-parse", "--verify", "--quiet", name]))?;
        if output.status == 1 {
            return Ok(None);
        }
        Ok(Some(text(output.checked()?)?))
    }
    fn ensure_refs(&self, intent: &WorktreeIntent) -> GitResult<()> {
        self.validate_main(intent)?;
        if self.ref_value(intent, &owner_ref(intent))?.is_some() {
            return self.verify_refs(intent, false);
        }
        if self
            .ref_value(intent, &format!("refs/heads/{}", intent.branch))?
            .is_some()
            || self.ref_value(intent, &base_ref(intent))?.is_some()
            || path_exists(Path::new(&intent.path))?
        {
            return Err(GitError::Collision);
        }
        let body = serde_json::to_vec(intent).map_err(|_| GitError::Ownership)?;
        let blob = text(
            self.executor
                .run_with_input(
                    Path::new(&intent.main_repository),
                    &args_of(&["hash-object", "-w", "--stdin"]),
                    &body,
                )?
                .checked()?,
        )?;
        let transaction = format!(
            "start\noption no-deref\ncreate refs/heads/{} {}\ncreate {} {}\ncreate {} {}\nprepare\ncommit\n",
            intent.branch,
            intent.base_oid,
            owner_ref(intent),
            blob,
            base_ref(intent),
            intent.base_oid
        );
        self.executor
            .run_with_input(
                Path::new(&intent.main_repository),
                &args_of(&["update-ref", "--create-reflog", "--stdin"]),
                transaction.as_bytes(),
            )?
            .checked()?;
        self.verify_refs(intent, false)
    }
    fn verify_refs(&self, intent: &WorktreeIntent, allow_missing_branch: bool) -> GitResult<()> {
        self.validate_main(intent)?;
        let owner = self
            .ref_value(intent, &owner_ref(intent))?
            .ok_or(GitError::Ownership)?;
        let body = self
            .executor
            .run(
                Path::new(&intent.main_repository),
                &args_of(&["cat-file", "blob", &owner]),
            )?
            .checked()?;
        if body != serde_json::to_vec(intent).map_err(|_| GitError::Ownership)?
            || self.ref_value(intent, &base_ref(intent))?.as_deref() != Some(&intent.base_oid)
        {
            return Err(GitError::Ownership);
        }
        match self.ref_value(intent, &format!("refs/heads/{}", intent.branch))? {
            Some(value) if value == intent.base_oid => Ok(()),
            None if allow_missing_branch => Ok(()),
            _ => Err(GitError::Ownership),
        }
    }
    fn registrations(&self, intent: &WorktreeIntent) -> GitResult<Vec<Registration>> {
        let raw = self
            .executor
            .run(
                Path::new(&intent.main_repository),
                &args_of(&["worktree", "list", "--porcelain", "-z"]),
            )?
            .checked()?;
        parse_registrations(&raw)
    }
    fn inspect_registration(
        &self,
        intent: &WorktreeIntent,
        locked: bool,
    ) -> GitResult<(Registration, PathBuf)> {
        self.validate_target(intent, true)?;
        self.verify_refs(intent, false)?;
        let path = Path::new(&intent.path);
        let mut rows = self
            .registrations(intent)?
            .into_iter()
            .filter(|row| same_path(&row.path, path))
            .collect::<Vec<_>>();
        if rows.len() != 1 {
            return Err(GitError::Ownership);
        }
        let row = rows.pop().ok_or(GitError::Ownership)?;
        if row.head != intent.base_oid
            || row.branch.as_deref() != Some(&format!("refs/heads/{}", intent.branch))
            || row
                .locked
                .as_ref()
                .is_some_and(|value| value != &lock_reason(intent))
            || (locked && row.locked.is_none())
        {
            return Err(GitError::Ownership);
        }
        paths::reject_links(&path.join(".git"))?;
        let admin = path_output(
            self.executor
                .run(path, &args_of(&["rev-parse", "--absolute-git-dir"]))?
                .checked()?,
        )?;
        let admin = paths::canonical_directory(&admin)?;
        if !paths::strict_descendant(&Path::new(&intent.common_git_dir).join("worktrees"), &admin) {
            return Err(GitError::Ownership);
        }
        self.executor.ensure_supported(path)?;
        let common = path_output(
            self.executor
                .run(
                    path,
                    &args_of(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
                )?
                .checked()?,
        )?;
        if paths::canonical_directory(&common)? != Path::new(&intent.common_git_dir) {
            return Err(GitError::Ownership);
        }
        Ok((row, admin))
    }
    fn require_removable(&self, intent: &WorktreeIntent, locked: bool) -> GitResult<()> {
        if !self.materialized(intent)? {
            return Err(GitError::Ownership);
        }
        let (_, admin) = self.inspect_registration(intent, locked).map_err(|error| {
            if error == GitError::Ownership {
                GitError::UnpreservedWork
            } else {
                error
            }
        })?;
        require_base_reflog(
            &Path::new(&intent.common_git_dir)
                .join("logs/refs/heads")
                .join(&intent.branch),
            &intent.base_oid,
            true,
        )?;
        require_base_reflog(&admin.join("logs/HEAD"), &intent.base_oid, false)?;
        let snapshot = capture(
            &self.executor,
            Path::new(&intent.path),
            intent,
            CaptureLimits::default(),
        )?;
        if !snapshot.files.is_empty() || !snapshot.exclusions.is_empty() {
            return Err(GitError::UnpreservedWork);
        }
        Ok(())
    }
    fn materialized(&self, intent: &WorktreeIntent) -> GitResult<bool> {
        match self.ref_value(intent, &materialized_ref(intent))? {
            None => Ok(false),
            Some(value) => {
                if self.ref_value(intent, &owner_ref(intent))?.as_deref() != Some(&value) {
                    return Err(GitError::Ownership);
                }
                Ok(true)
            }
        }
    }
    fn mark_materialized(&self, intent: &WorktreeIntent) -> GitResult<()> {
        self.verify_refs(intent, false)?;
        let owner = self
            .ref_value(intent, &owner_ref(intent))?
            .ok_or(GitError::Ownership)?;
        let tx = format!(
            "start\noption no-deref\ncreate {} {}\nprepare\ncommit\n",
            materialized_ref(intent),
            owner
        );
        self.executor
            .run_with_input(
                Path::new(&intent.main_repository),
                &args_of(&["update-ref", "--stdin"]),
                tx.as_bytes(),
            )?
            .checked()?;
        if !self.materialized(intent)? {
            return Err(GitError::Ownership);
        }
        Ok(())
    }
}

fn record(store: &Store, operation_id: &str) -> GitResult<WorktreeRecord> {
    store
        .load()?
        .worktrees()
        .get(operation_id)
        .cloned()
        .ok_or(GitError::MissingAttempt)
}
fn require_inactive_attempt(store: &Store, intent: &WorktreeIntent) -> GitResult<()> {
    let state = store.load()?;
    let attempt = state
        .runs()
        .get(&RunId::new(&intent.run_id).map_err(|_| GitError::Ownership)?)
        .and_then(|run| run.task(&TaskId::new(&intent.task_id).ok()?).ok())
        .and_then(|task| {
            task.attempts()
                .iter()
                .find(|attempt| attempt.id().as_str() == intent.attempt_id)
        })
        .ok_or(GitError::MissingAttempt)?;
    if matches!(
        attempt.state(),
        AttemptState::Admitted | AttemptState::Running | AttemptState::Uncertain
    ) {
        return Err(GitError::Pending);
    }
    Ok(())
}
/// Branch deletion and worktree removal also delete their reflogs. A clean
/// HEAD at base must not hide a unique commit retained only in those logs.
fn require_base_reflog(path: &Path, base: &str, required: bool) -> GitResult<()> {
    paths::reject_links(path)?;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => return Ok(()),
        Err(_) => return Err(GitError::UnpreservedWork),
    };
    let before = file.metadata()?;
    if !before.is_file() || before.len() > MAX_FILE_BYTES as u64 {
        return Err(GitError::Limit);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = fs::metadata(path)?;
    paths::reject_links(path)?;
    if (required && bytes.is_empty())
        || bytes.len() > MAX_FILE_BYTES
        || before.len() != after.len()
        || before.modified()? != after.modified()?
    {
        return Err(GitError::UnpreservedWork);
    }
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let mut fields = line.split(|b| *b == b' ');
        for _ in 0..2 {
            let oid = std::str::from_utf8(fields.next().ok_or(GitError::Ownership)?)
                .map_err(|_| GitError::Ownership)?;
            if oid.len() != base.len() || !valid_oid(oid) {
                return Err(GitError::Ownership);
            }
            if oid != base && !oid.bytes().all(|b| b == b'0') {
                return Err(GitError::UnpreservedWork);
            }
        }
    }
    Ok(())
}
fn applied(receipt: crate::persistence::Receipt) -> GitResult<()> {
    if receipt.result.disposition == Disposition::Applied {
        Ok(())
    } else {
        Err(GitError::Conflict)
    }
}
fn commit_phase(
    store: &mut Store,
    scope: &LocalRef,
    operation: &str,
    phase: &str,
    command: GitCommand,
    fault: FaultPoint,
) -> GitResult<()> {
    let key = LocalRef::new(format!(
        "git-{phase}-{}",
        &content_hash(operation.as_bytes())[..40]
    ))
    .map_err(|_| GitError::Ownership)?;
    let revision = store.load()?.revision();
    let request = Request {
        scope: scope.clone(),
        key,
        expected_revision: revision,
        command: Command::Git(command),
    };
    applied(store.execute_with_fault(&request, fault)?)
}
fn observation(
    kind: &str,
    intent: &WorktreeIntent,
    value: &serde_json::Value,
) -> WorktreeObservation {
    WorktreeObservation {
        kind: kind.into(),
        evidence_sha256: content_hash(
            json!([intent.operation_id, kind, value])
                .to_string()
                .as_bytes(),
        ),
    }
}
fn owner_ref(intent: &WorktreeIntent) -> String {
    format!("refs/orchestrix/ownership/{}", intent.operation_id)
}
fn base_ref(intent: &WorktreeIntent) -> String {
    format!("refs/orchestrix/bases/{}", intent.operation_id)
}
fn materialized_ref(intent: &WorktreeIntent) -> String {
    format!("refs/orchestrix/materialized/{}", intent.operation_id)
}
fn lock_reason(intent: &WorktreeIntent) -> String {
    format!(
        "orchestrix:{}:{}",
        intent.operation_id, intent.ownership_token
    )
}
fn args_of(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
fn text(bytes: Vec<u8>) -> GitResult<String> {
    String::from_utf8(bytes)
        .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
        .map_err(|_| GitError::Unsupported)
}
fn path_output(bytes: Vec<u8>) -> GitResult<PathBuf> {
    Ok(PathBuf::from(text(bytes)?))
}
fn path_string(path: &Path) -> GitResult<String> {
    path.to_str().map(str::to_owned).ok_or(GitError::Path)
}
fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn same_path(left: &Path, right: &Path) -> bool {
    fn resolve(path: &Path) -> Option<PathBuf> {
        fs::canonicalize(path).ok().or_else(|| {
            Some(
                fs::canonicalize(path.parent()?)
                    .ok()?
                    .join(path.file_name()?),
            )
        })
    }
    resolve(left)
        .zip(resolve(right))
        .is_some_and(|(a, b)| a == b)
}
fn path_exists(path: &Path) -> GitResult<bool> {
    paths::reject_links(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(GitError::Io),
    }
}

#[derive(Clone)]
struct Registration {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    locked: Option<String>,
}
fn parse_registrations(bytes: &[u8]) -> GitResult<Vec<Registration>> {
    let mut result = Vec::new();
    let mut current: Option<Registration> = None;
    for entry in bytes.split(|byte| *byte == 0) {
        let entry = std::str::from_utf8(entry).map_err(|_| GitError::Unsupported)?;
        if entry.is_empty() {
            if let Some(row) = current.take() {
                result.push(row)
            };
            continue;
        }
        if let Some(value) = entry.strip_prefix("worktree ") {
            if current.is_some() {
                return Err(GitError::Ownership);
            }
            current = Some(Registration {
                path: PathBuf::from(value),
                head: String::new(),
                branch: None,
                locked: None,
            });
        } else if let Some(row) = current.as_mut() {
            if let Some(value) = entry.strip_prefix("HEAD ") {
                row.head = value.into();
            } else if let Some(value) = entry.strip_prefix("branch ") {
                row.branch = Some(value.into());
            } else if let Some(value) = entry.strip_prefix("locked ") {
                row.locked = Some(value.into());
            } else if entry == "locked" {
                row.locked = Some(String::new());
            } else if entry == "bare" || entry == "detached" || entry.starts_with("prunable ") {
            } else {
                return Err(GitError::Unsupported);
            }
        } else {
            return Err(GitError::Ownership);
        }
    }
    if let Some(row) = current {
        result.push(row);
    }
    Ok(result)
}

fn read_base(
    executor: &GitExecutor,
    root: &Path,
    base: &str,
) -> GitResult<BTreeMap<String, Vec<u8>>> {
    let tree = executor
        .run(
            root,
            &args_of(&["ls-tree", "-r", "-z", "--full-tree", base]),
        )?
        .checked()?;
    let mut files = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    let mut total = 0usize;
    for entry in tree.split(|byte| *byte == 0).filter(|v| !v.is_empty()) {
        let entry = std::str::from_utf8(entry).map_err(|_| GitError::Unsupported)?;
        let (metadata, path) = entry.split_once('\t').ok_or(GitError::Unsupported)?;
        let metadata = metadata.split(' ').collect::<Vec<_>>();
        if metadata.len() != 3
            || metadata[0] != "100644"
            || metadata[1] != "blob"
            || !valid_oid(metadata[2])
            || !relative_file_path(path)
            || path
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c))
            || !aliases.insert(path.to_lowercase())
        {
            return Err(GitError::Unsupported);
        }
        let bytes = executor
            .run(root, &args_of(&["cat-file", "blob", metadata[2]]))?
            .checked()?;
        total = total.checked_add(bytes.len()).ok_or(GitError::Limit)?;
        if bytes.len() > MAX_FILE_BYTES || total > 64 * MAX_FILE_BYTES || files.len() >= 4096 {
            return Err(GitError::Limit);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| GitError::Unsupported)?;
        if text.contains('\0') {
            return Err(GitError::Unsupported);
        }
        if sanitize_text(text).map_err(|_| GitError::Limit)? != text {
            return Err(GitError::SecretContent);
        }
        if files.insert(path.into(), bytes).is_some() {
            return Err(GitError::Unsupported);
        }
    }
    Ok(files)
}
fn materialize(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> GitResult<()> {
    for (relative, bytes) in files {
        let path = root.join(relative);
        paths::reject_links(&path)?;
        if let Some(parent) = path.parent() {
            let relative = parent.strip_prefix(root).map_err(|_| GitError::Scope)?;
            let mut directory = root.to_path_buf();
            for component in relative.components() {
                directory.push(component.as_os_str());
                match fs::create_dir(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        if !directory.is_dir() {
                            return Err(GitError::Collision);
                        }
                    }
                    Err(_) => return Err(GitError::Io),
                }
                paths::reject_links(&directory)?;
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    GitError::Collision
                } else {
                    GitError::Io
                }
            })?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    Ok(())
}
fn verify_base(
    executor: &GitExecutor,
    root: &Path,
    intent: &WorktreeIntent,
    files: &BTreeMap<String, Vec<u8>>,
) -> GitResult<()> {
    let captured = capture(executor, root, intent, CaptureLimits::default())?;
    if !captured.files.is_empty()
        || captured
            .exclusions
            .iter()
            .any(|excluded| !files.contains_key(&excluded.path))
    {
        return Err(GitError::Pending);
    }
    for (relative, expected) in files {
        let path = root.join(relative);
        paths::reject_links(&path)?;
        if fs::read(path)? != *expected {
            return Err(GitError::Pending);
        }
    }
    Ok(())
}
