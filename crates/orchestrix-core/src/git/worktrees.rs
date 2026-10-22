//! Recoverable Git effects around short SQLite transactions.
use super::{CaptureLimits, GitError, GitExecutor, GitResult, capture, paths};
use crate::domain::*;
use crate::git_records::{
    CapturedArtifact, GitCommand, MAX_FILE_BYTES, WorktreeIntent, WorktreeObservation,
    WorktreeRecord, WorktreeState, content_hash,
};
use crate::persistence::{Command, Disposition, FaultPoint, Request, Store};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
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
    DuringMaterialization,
    AfterWorktree,
    CreateConfirmationRollback,
    AfterWorktreeRemoval,
    AfterRemoval,
    RemovalConfirmationRollback,
}

pub struct WorktreeService {
    executor: Arc<GitExecutor>,
}
impl WorktreeService {
    pub(super) fn with_executor(executor: GitExecutor) -> Self {
        Self::with_shared_executor(Arc::new(executor))
    }
    /// Share only the bounded transport and its scope, never repository facts.
    pub(super) fn with_shared_executor(executor: Arc<GitExecutor>) -> Self {
        Self { executor }
    }
    /// The caller owns a separate persisted integration intent. This does not
    /// reserve or fabricate an execution attempt in the domain/worker tables.
    /// Returns true only when this call verified the complete initial base and
    /// wrote its completion anchor. Existing completed work is never overwritten
    /// or represented as a fresh verification.
    pub(super) fn create_integration_owned(&self, intent: &WorktreeIntent) -> GitResult<bool> {
        if !intent.validate() {
            return Err(GitError::Ownership);
        }
        let root = paths::canonical_directory(Path::new(&intent.authorized_root))?;
        let repo = paths::canonical_directory(Path::new(&intent.main_repository))?;
        let common = paths::canonical_directory(Path::new(&intent.common_git_dir))?;
        let target = Path::new(&intent.path);
        if root == repo
            || paths::strict_descendant(&repo, &root)
            || root == common
            || paths::strict_descendant(&common, &root)
            || paths::strict_descendant(target, &repo)
            || paths::strict_descendant(target, &common)
        {
            return Err(GitError::Scope);
        }
        self.validate_main(intent)?;
        let material = read_intent_base(&self.executor, &repo, intent)?;
        self.ensure_refs(intent)?;
        if !path_exists(target)? {
            self.validate_target(intent, false)?;
            let rows = self.registrations(intent)?;
            for row in rows {
                if row.path == target
                    || paths::strict_descendant(&row.path, target)
                    || paths::strict_descendant(target, &row.path)
                    || row.branch.as_deref() == Some(&format!("refs/heads/{}", intent.branch))
                {
                    return Err(GitError::Ownership);
                }
            }
            self.executor
                .run(
                    &repo,
                    &[
                        "worktree".into(),
                        "add".into(),
                        "--quiet".into(),
                        "--no-checkout".into(),
                        "--lock".into(),
                        "--reason".into(),
                        lock_reason(intent).into(),
                        paths::git_path(target)?,
                        intent.branch.clone().into(),
                    ],
                )?
                .checked()?;
            self.inspect_registration(intent, true)?;
            materialize_profile(&self.executor, &repo, target, intent, &material, false)?;
            self.executor
                .run(target, &args_of(&["read-tree", &intent.base_oid]))?
                .checked()?;
        }
        self.inspect_registration(intent, true)?;
        if !self.materialized(intent)? {
            verify_base(&self.executor, target, intent, &material)?;
            self.mark_materialized(intent)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn inspect_integration_owned(&self, intent: &WorktreeIntent) -> GitResult<()> {
        self.validate_main(intent)?;
        if !self.materialized(intent)? {
            return Err(GitError::Ownership);
        }
        self.inspect_registration(intent, true)?;
        Ok(())
    }
    pub fn new() -> GitResult<Self> {
        Ok(Self::with_executor(GitExecutor::new()?))
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
        self.create_profile(store, request, None, fault)
    }
    pub fn create_with_preservation(
        &self,
        store: &mut Store,
        request: &CreateWorktree,
        editable_paths: &[String],
    ) -> GitResult<WorktreeRecord> {
        self.create_profile(store, request, Some(editable_paths), GitFaultPoint::None)
    }
    #[doc(hidden)]
    pub fn create_with_preservation_and_fault(
        &self,
        store: &mut Store,
        request: &CreateWorktree,
        editable_paths: &[String],
        fault: GitFaultPoint,
    ) -> GitResult<WorktreeRecord> {
        self.create_profile(store, request, Some(editable_paths), fault)
    }
    fn create_profile(
        &self,
        store: &mut Store,
        request: &CreateWorktree,
        editable_paths: Option<&[String]>,
        fault: GitFaultPoint,
    ) -> GitResult<WorktreeRecord> {
        let intent = self.intent(store, request, editable_paths)?;
        let plan = Request {
            scope: request.scope.clone(),
            key: request.key.clone(),
            expected_revision: request.expected_revision,
            command: Command::git(GitCommand::Plan(intent.clone())),
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
        let material = read_intent_base(&self.executor, repo, &intent)?;
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
            materialize_profile(
                &self.executor,
                repo,
                target,
                &intent,
                &material,
                fault == GitFaultPoint::DuringMaterialization,
            )?;
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
        self.capture_observed(
            store,
            scope,
            key,
            expected_revision,
            operation_id,
            between,
            |_| {},
        )
    }
    /// Reports only fixed operation stages; the caller receives the original
    /// error without repeating inspection, capture or the durable command.
    pub(crate) fn capture_with_diagnostics<F: FnMut(&'static str)>(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
        progress: F,
    ) -> GitResult<CapturedArtifact> {
        self.capture_observed(
            store,
            scope,
            key,
            expected_revision,
            operation_id,
            || Ok(()),
            progress,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn capture_observed<F: FnOnce() -> GitResult<()>, P: FnMut(&'static str)>(
        &self,
        store: &mut Store,
        scope: &LocalRef,
        key: &LocalRef,
        expected_revision: u64,
        operation_id: &str,
        between: F,
        mut progress: P,
    ) -> GitResult<CapturedArtifact> {
        progress("inspect");
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
            progress("persist");
            applied(store.execute(&Request {
                scope: scope.clone(),
                key: key.clone(),
                expected_revision,
                command: Command::git(GitCommand::RecordArtifact(artifact.clone())),
            })?)?;
            return Ok(artifact);
        }
        let current = record(store, operation_id)?;
        if current.state != WorktreeState::Created {
            return Err(GitError::Pending);
        }
        self.inspect(store, operation_id)?;
        progress("capture");
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
            command: Command::git(GitCommand::RecordArtifact(artifact.clone())),
        };
        progress("persist");
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
            command: Command::git(GitCommand::PlanCleanup {
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

    fn intent(
        &self,
        store: &Store,
        request: &CreateWorktree,
        editable_paths: Option<&[String]>,
    ) -> GitResult<WorktreeIntent> {
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
            || (attempt.inputs().base.as_str() != request.base_commit
                && !state.workers().values().any(|w| {
                    w.intent.attempt_id == attempt.id().as_str()
                        && w.intent.base_oid == request.base_commit
                        && w.intent.correction.as_ref().is_some_and(|origin| {
                            attempt.inputs().correction_of.as_ref().is_some_and(|a| {
                                a.id().as_str() == origin.artifact_id
                                    && a.revision().as_str() == origin.artifact_revision
                                    && attempt.inputs().base == *a.revision()
                            })
                        })
                }))
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
            preservation: editable_paths
                .map(|paths| super::preservation::plan(&self.executor, &repo, &resolved, paths))
                .transpose()?,
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
        let directories = self.executor.ensure_supported_directories(&repo)?;
        if directories.common != Path::new(&intent.common_git_dir) {
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
            // Git uses the same quiet status for a missing name and some
            // corrupt loose refs. A still-present file/directory is not proof
            // of absence and must never authorize creation/removal as missing.
            let loose = Path::new(&intent.common_git_dir).join(name);
            paths::reject_links(&loose)?;
            return match fs::symlink_metadata(&loose) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(GitError::Io),
                Ok(_) => Err(GitError::Ownership),
            };
        }
        Ok(Some(text(output.checked()?)?))
    }
    fn ref_values(
        &self,
        intent: &WorktreeIntent,
        names: &[String],
    ) -> GitResult<BTreeMap<String, String>> {
        if names.is_empty()
            || names.len() > 4
            || names.iter().collect::<BTreeSet<_>>().len() != names.len()
        {
            return Err(GitError::Ownership);
        }
        let mut args = args_of(&[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(symref)%00",
        ]);
        // Exact generated ref names contain no pattern metacharacters. Git can
        // also match descendants, so reject every unrequested result below.
        args.push(format!("--count={}", names.len() + 1).into());
        args.extend(names.iter().map(OsString::from));
        let raw = self
            .executor
            .run(Path::new(&intent.main_repository), &args)?
            .checked()?;
        owned_ref_values(&raw, names)
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
        let names = [
            owner_ref(intent),
            base_ref(intent),
            format!("refs/heads/{}", intent.branch),
        ];
        let values = self.ref_values(intent, &names)?;
        let owner = values.get(&names[0]).ok_or(GitError::Ownership)?;
        let body = self
            .executor
            .run(
                Path::new(&intent.main_repository),
                &args_of(&["cat-file", "blob", owner]),
            )?
            .checked()?;
        if body != serde_json::to_vec(intent).map_err(|_| GitError::Ownership)?
            || values.get(&names[1]) != Some(&intent.base_oid)
            || self.ref_values(intent, &names)? != values
        {
            return Err(GitError::Ownership);
        }
        match values.get(&names[2]) {
            Some(value) if *value == intent.base_oid => Ok(()),
            // for-each-ref omits broken/dangling refs. Absence is not authority
            // to treat a symbolic/broken branch as missing: preserve the native
            // symbolic-ref/rev-parse check at this optional-absence edge.
            None if allow_missing_branch && self.ref_value(intent, &names[2])?.is_none() => Ok(()),
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
        let directories = self.executor.ensure_supported_directories(path)?;
        if !paths::strict_descendant(
            &Path::new(&intent.common_git_dir).join("worktrees"),
            &directories.admin,
        ) {
            return Err(GitError::Ownership);
        }
        if directories.common != Path::new(&intent.common_git_dir) {
            return Err(GitError::Ownership);
        }
        Ok((row, directories.admin))
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
        let names = [materialized_ref(intent), owner_ref(intent)];
        let values = self.ref_values(intent, &names)?;
        match values.get(&names[0]) {
            None => {
                // Required rows fail closed; optional absence still uses the
                // old resolver, which rejects dangling symbolic refs omitted by
                // for-each-ref. Do not cache across materialization boundaries.
                if self.ref_value(intent, &names[0])?.is_some() {
                    return Err(GitError::Ownership);
                }
                Ok(false)
            }
            Some(value) => {
                if values.get(&names[1]) != Some(value) {
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

fn owned_ref_values(raw: &[u8], names: &[String]) -> GitResult<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    if raw.is_empty() {
        return Ok(values);
    }
    let body = raw.strip_suffix(b"\n").ok_or(GitError::Process)?;
    for row in body.split(|byte| *byte == b'\n') {
        let row = row.strip_suffix(b"\0").ok_or(GitError::Process)?;
        let fields: Vec<_> = row.split(|byte| *byte == 0).collect();
        if fields.len() != 3 {
            return Err(GitError::Process);
        }
        let name = std::str::from_utf8(fields[0]).map_err(|_| GitError::Process)?;
        let oid = std::str::from_utf8(fields[1]).map_err(|_| GitError::Process)?;
        if !names.iter().any(|expected| expected == name)
            || !valid_oid(oid)
            || !fields[2].is_empty()
            || values.insert(name.into(), oid.into()).is_some()
        {
            return Err(GitError::Ownership);
        }
    }
    Ok(values)
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
        command: Command::git(command),
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
pub(super) fn valid_oid(value: &str) -> bool {
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

pub(super) fn read_base(
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
            || !super::preflight::source_path(path, &mut aliases)
        {
            return Err(GitError::Unsupported);
        }
        let bytes = executor
            .run(root, &args_of(&["cat-file", "blob", metadata[2]]))?
            .checked()?;
        total = total.checked_add(bytes.len()).ok_or(GitError::Limit)?;
        if bytes.len() > MAX_FILE_BYTES
            || total > super::SOURCE_BYTES
            || files.len() >= super::SOURCE_FILES
        {
            return Err(GitError::Limit);
        }
        if let Some(issue) = super::preflight::content_issue(&bytes)? {
            return Err(if issue == "secret-pattern" {
                GitError::SecretContent
            } else {
                GitError::Unsupported
            });
        }
        if files.insert(path.into(), bytes).is_some() {
            return Err(GitError::Unsupported);
        }
    }
    Ok(files)
}
pub(super) fn read_intent_base(
    executor: &GitExecutor,
    root: &Path,
    intent: &WorktreeIntent,
) -> GitResult<BTreeMap<String, Vec<u8>>> {
    match &intent.preservation {
        Some(manifest) => {
            super::preservation::verify_base(executor, root, manifest)?;
            Ok(BTreeMap::new())
        }
        None => read_base(executor, root, &intent.base_oid),
    }
}
fn materialize_profile(
    executor: &GitExecutor,
    source: &Path,
    target: &Path,
    intent: &WorktreeIntent,
    files: &BTreeMap<String, Vec<u8>>,
    fault_after_first: bool,
) -> GitResult<()> {
    match &intent.preservation {
        Some(manifest) => {
            super::preservation::materialize(executor, source, target, manifest, fault_after_first)
        }
        None if fault_after_first => Err(GitError::Unsupported),
        None => materialize(target, files),
    }
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

#[cfg(test)]
mod metadata_batch_tests {
    use super::*;
    use std::{
        env,
        process::{Command as ProcessCommand, Stdio},
    };

    #[test]
    fn owned_refs_reject_symrefs_duplicates_extra_rows_and_invalid_oids() {
        let names = vec!["refs/orchestrix/ownership/one".to_owned()];
        let oid = "a".repeat(40);
        let row = format!("{}\0{oid}\0\0\n", names[0]);
        assert_eq!(
            owned_ref_values(row.as_bytes(), &names)
                .unwrap()
                .get(&names[0]),
            Some(&oid)
        );
        assert!(owned_ref_values(b"", &names).unwrap().is_empty());
        for bad in [
            format!("{}\0{oid}\0refs/heads/main\0\n", names[0]),
            format!("{row}{row}"),
            format!("{}/descendant\0{oid}\0\0\n", names[0]),
            format!("{}\0not-an-oid\0\0\n", names[0]),
        ] {
            assert_eq!(
                owned_ref_values(bad.as_bytes(), &names),
                Err(GitError::Ownership)
            );
        }
        for bad in [
            row.trim_end_matches('\n').to_owned(),
            format!("{}\0{oid}\n", names[0]),
        ] {
            assert_eq!(
                owned_ref_values(bad.as_bytes(), &names),
                Err(GitError::Process)
            );
        }
    }

    struct Fixture {
        root: PathBuf,
        temporary: PathBuf,
        repo: PathBuf,
        templates: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let temporary = paths::canonical_directory(&env::temp_dir()).unwrap();
            let root = temporary.join(format!(
                "orchestrix-metadata-batch-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let repo = root.join("repository");
            fs::create_dir(&repo).unwrap();
            let templates = root.join("empty-templates");
            fs::create_dir(&templates).unwrap();
            let fixture = Self {
                root,
                temporary,
                repo,
                templates,
            };
            fixture.git(&["init", "--quiet"], None);
            fixture
        }
        fn git(&self, args: &[&str], input: Option<&[u8]>) -> Vec<u8> {
            let mut command = ProcessCommand::new("git");
            command.env_clear();
            for key in ["PATH", "SystemRoot", "SystemDrive", "WINDIR", "TEMP", "TMP"] {
                if let Some(value) = env::var_os(key) {
                    command.env(key, value);
                }
            }
            let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
            command
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", null)
                .env("GIT_CONFIG_SYSTEM", null)
                .env("GIT_NO_REPLACE_OBJECTS", "1")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_OPTIONAL_LOCKS", "0")
                .arg("-c")
                .arg(format!(
                    "init.templateDir={}",
                    paths::git_path(&self.templates).unwrap().to_string_lossy()
                ))
                .arg("-c")
                .arg(format!(
                    "core.hooksPath={}",
                    paths::git_path(&self.templates).unwrap().to_string_lossy()
                ))
                .args(["-c", "core.fsmonitor=false", "-c", "core.autocrlf=false"])
                .args(args)
                .current_dir(&self.repo)
                .stdin(if input.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000);
            }
            let mut child = command.spawn().unwrap();
            if let Some(input) = input {
                child.stdin.take().unwrap().write_all(input).unwrap();
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "owned metadata fixture setup: {} {}: {}",
                args.first().unwrap_or(&"unknown"),
                args.get(1).unwrap_or(&"none"),
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let root = fs::canonicalize(&self.root).unwrap();
            assert!(
                root.parent() == Some(self.temporary.as_path())
                    && root
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("orchestrix-metadata-batch-")
            );
            paths::reject_links(&root).unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn shared_executor_reobserves_linked_directories_config_and_owned_refs() {
        let fixture = Fixture::new();
        let executor = Arc::new(GitExecutor::new().unwrap());
        let service = WorktreeService::with_shared_executor(Arc::clone(&executor));
        let second = WorktreeService::with_shared_executor(Arc::clone(&executor));
        assert!(Arc::ptr_eq(&service.executor, &second.executor));
        let tree =
            text(fixture.git(&["hash-object", "-t", "tree", "-w", "--stdin"], Some(b""))).unwrap();
        let commit = format!(
            "tree {tree}\nauthor Fixture <fixture@orchestrix.invalid> 1 +0000\ncommitter Fixture <fixture@orchestrix.invalid> 1 +0000\n\nFresh supported directories\n"
        );
        let base = text(fixture.git(
            &["hash-object", "-t", "commit", "-w", "--stdin"],
            Some(commit.as_bytes()),
        ))
        .unwrap();
        fixture.git(&["update-ref", "refs/heads/main", &base], None);
        fixture.git(&["symbolic-ref", "HEAD", "refs/heads/main"], None);
        fixture.git(&["read-tree", &base], None);

        // The source is genuinely linked: its admin and common directories
        // differ, and the shared executor must report both from fresh checks.
        let source = fixture.root.join("linked-source");
        let source_arg = paths::git_path(&source).unwrap();
        fixture.git(
            &[
                "worktree",
                "add",
                "--quiet",
                "--no-checkout",
                "-b",
                "source",
                source_arg.to_str().unwrap(),
                &base,
            ],
            None,
        );
        let source = paths::canonical_directory(&source).unwrap();
        let common = paths::canonical_directory(&fixture.repo.join(".git")).unwrap();
        let source_dirs = executor.ensure_supported_directories(&source).unwrap();
        assert_eq!(source_dirs.common, common);
        assert!(paths::strict_descendant(
            &common.join("worktrees"),
            &source_dirs.admin
        ));
        executor.ensure_supported(&source).unwrap();

        let authorized = fixture.root.join("attempts");
        fs::create_dir(&authorized).unwrap();
        let intent = WorktreeIntent {
            preservation: None,
            operation_id: "fresh-directories".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: base.clone(),
            domain_base: "base".into(),
            worktree_ref: "worktree".into(),
            authorized_root: path_string(&authorized).unwrap(),
            path: path_string(&authorized.join("checkout")).unwrap(),
            main_repository: path_string(&source).unwrap(),
            branch: "orchestrix/fresh-directories".into(),
            common_git_dir: path_string(&common).unwrap(),
            ownership_token: "b".repeat(64),
        };
        assert!(intent.validate());
        service.ensure_refs(&intent).unwrap();
        let target_arg = paths::git_path(Path::new(&intent.path)).unwrap();
        fixture.git(
            &[
                "worktree",
                "add",
                "--quiet",
                "--no-checkout",
                "--lock",
                "--reason",
                &lock_reason(&intent),
                target_arg.to_str().unwrap(),
                &intent.branch,
            ],
            None,
        );
        let (_, admin) = service.inspect_registration(&intent, true).unwrap();
        assert!(paths::strict_descendant(&common.join("worktrees"), &admin));

        // Redirecting the linked source's actual common directory is observed
        // on the next call, rather than accepting the previous support result.
        let other = fixture.root.join("other-repository");
        let other_arg = paths::git_path(&other).unwrap();
        fixture.git(&["init", "--quiet", other_arg.to_str().unwrap()], None);
        let other_common = paths::canonical_directory(&other.join(".git")).unwrap();
        let commondir = source_dirs.admin.join("commondir");
        let original_commondir = fs::read(&commondir).unwrap();
        fs::write(
            &commondir,
            format!(
                "{}\n",
                paths::git_path(&other_common).unwrap().to_str().unwrap()
            ),
        )
        .unwrap();
        let redirected = executor.ensure_supported_directories(&source).unwrap();
        assert_eq!(redirected.admin, source_dirs.admin);
        assert_eq!(redirected.common, other_common);
        let mismatched_common = second.validate_main(&intent);
        fs::write(&commondir, original_commondir).unwrap();
        assert_eq!(mismatched_common, Err(GitError::Ownership));
        second.validate_main(&intent).unwrap();

        // A checkout marker pointing at a supported main admin directory must
        // not pass the owned linked-admin namespace check.
        let marker = Path::new(&intent.path).join(".git");
        let original_marker = fs::read(&marker).unwrap();
        fs::write(
            &marker,
            format!(
                "gitdir: {}\n",
                paths::git_path(&common).unwrap().to_str().unwrap()
            ),
        )
        .unwrap();
        let mismatched_admin = second.inspect_registration(&intent, true).map(|_| ());
        fs::write(&marker, original_marker).unwrap();
        assert_eq!(mismatched_admin, Err(GitError::Ownership));
        second.inspect_registration(&intent, true).unwrap();

        let config_path = common.join("config");
        let original_config = fs::read(&config_path).unwrap();
        let mut changed_config = original_config.clone();
        changed_config.extend_from_slice(b"\n[filter \"blocked\"]\nclean = never-run\n");
        fs::write(&config_path, changed_config).unwrap();
        let unsupported = service.inspect_registration(&intent, true).map(|_| ());
        fs::write(&config_path, original_config).unwrap();
        assert_eq!(unsupported, Err(GitError::Unsupported));
        second.inspect_registration(&intent, true).unwrap();

        let branch = common.join(format!("refs/heads/{}", intent.branch));
        let original_branch = fs::read(&branch).unwrap();
        fs::write(&branch, b"ref: refs/heads/main\n").unwrap();
        let changed_ref = service.inspect_registration(&intent, true).map(|_| ());
        fs::write(&branch, original_branch).unwrap();
        assert_eq!(changed_ref, Err(GitError::Ownership));
        second.inspect_registration(&intent, true).unwrap();
    }

    #[test]
    fn installed_git_batches_preserve_flags_refs_replacements_and_optional_absence() {
        let fixture = Fixture::new();
        let service = WorktreeService::new().unwrap();
        let tree = text(
            service
                .executor
                .run_with_input(
                    &fixture.repo,
                    &args_of(&["hash-object", "-t", "tree", "-w", "--stdin"]),
                    b"",
                )
                .unwrap()
                .checked()
                .unwrap(),
        )
        .unwrap();
        let commit = format!(
            "tree {tree}\nauthor Fixture <fixture@orchestrix.invalid> 1 +0000\ncommitter Fixture <fixture@orchestrix.invalid> 1 +0000\n\nOwned metadata fixture\n"
        );
        let base = text(
            service
                .executor
                .run_with_input(
                    &fixture.repo,
                    &args_of(&["hash-object", "-t", "commit", "-w", "--stdin"]),
                    commit.as_bytes(),
                )
                .unwrap()
                .checked()
                .unwrap(),
        )
        .unwrap();
        let authorized = fixture.root.join("attempts");
        fs::create_dir(&authorized).unwrap();
        let intent = WorktreeIntent {
            preservation: None,
            operation_id: "metadata-batch".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: base.clone(),
            domain_base: "base".into(),
            worktree_ref: "worktree".into(),
            authorized_root: path_string(&authorized).unwrap(),
            path: path_string(&authorized.join("checkout")).unwrap(),
            main_repository: path_string(&fixture.repo).unwrap(),
            branch: "orchestrix/metadata-batch".into(),
            common_git_dir: path_string(&fixture.repo.join(".git")).unwrap(),
            ownership_token: "a".repeat(64),
        };
        assert!(intent.validate());
        let owner = text(
            service
                .executor
                .run_with_input(
                    &fixture.repo,
                    &args_of(&["hash-object", "-w", "--stdin"]),
                    &serde_json::to_vec(&intent).unwrap(),
                )
                .unwrap()
                .checked()
                .unwrap(),
        )
        .unwrap();
        let replacement = text(
            service
                .executor
                .run_with_input(
                    &fixture.repo,
                    &args_of(&["hash-object", "-w", "--stdin"]),
                    b"not ownership\n",
                )
                .unwrap()
                .checked()
                .unwrap(),
        )
        .unwrap();
        let branch = format!("refs/heads/{}", intent.branch);
        for (name, oid) in [
            (owner_ref(&intent), &owner),
            (base_ref(&intent), &base),
            (branch.clone(), &base),
            (materialized_ref(&intent), &owner),
            (format!("refs/replace/{owner}"), &replacement),
        ] {
            service
                .executor
                .run(&fixture.repo, &args_of(&["update-ref", &name, oid]))
                .unwrap()
                .checked()
                .unwrap();
        }
        // Replacement refs cannot rewrite the ownership blob addressed by OID.
        service.verify_refs(&intent, false).unwrap();
        assert!(service.materialized(&intent).unwrap());
        let marker = Path::new(&intent.common_git_dir).join(materialized_ref(&intent));
        fs::write(&marker, b"ref: refs/heads/missing-target\n").unwrap();
        assert_eq!(service.materialized(&intent), Err(GitError::Ownership));
        fs::write(&marker, format!("{owner}\n")).unwrap();
        let owner_path = Path::new(&intent.common_git_dir).join(owner_ref(&intent));
        fs::write(&owner_path, format!("ref: {}\n", base_ref(&intent))).unwrap();
        assert_eq!(
            service.verify_refs(&intent, false),
            Err(GitError::Ownership)
        );
        fs::write(&owner_path, format!("{owner}\n")).unwrap();
        let branch_path = Path::new(&intent.common_git_dir).join(&branch);
        fs::write(&branch_path, b"ref: refs/heads/missing-target\n").unwrap();
        assert_eq!(service.verify_refs(&intent, true), Err(GitError::Ownership));
        fs::remove_file(&branch_path).unwrap();
        assert_eq!(
            service.verify_refs(&intent, false),
            Err(GitError::Ownership)
        );
        service.verify_refs(&intent, true).unwrap();
        fs::remove_file(&marker).unwrap();
        assert!(!service.materialized(&intent).unwrap());
        fs::write(&marker, b"broken-ref-body\n").unwrap();
        assert!(service.materialized(&intent).is_err());

        // Exercise installed Git's combined -s/-v grammar, not just fixtures.
        fs::write(fixture.repo.join("file space.txt"), b"not ownership\n").unwrap();
        fixture.git(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{replacement},file space.txt"),
            ],
            None,
        );
        let stages = service
            .executor
            .run(&fixture.repo, &args_of(&["ls-files", "-s", "-z"]))
            .unwrap()
            .checked()
            .unwrap();
        let combined = service
            .executor
            .run(&fixture.repo, &args_of(&["ls-files", "-s", "-v", "-z"]))
            .unwrap()
            .checked()
            .unwrap();
        assert_eq!(combined, [b"H ".as_slice(), &stages].concat());
        let flags = service
            .executor
            .run(&fixture.repo, &args_of(&["ls-files", "-v", "-z"]))
            .unwrap()
            .checked()
            .unwrap();
        assert_eq!(flags, b"H file space.txt\0");
        fixture.git(
            &["update-index", "--assume-unchanged", "file space.txt"],
            None,
        );
        assert!(
            service
                .executor
                .run(&fixture.repo, &args_of(&["ls-files", "-s", "-v", "-z"]))
                .unwrap()
                .checked()
                .unwrap()
                .starts_with(b"h ")
        );
        fixture.git(
            &["update-index", "--no-assume-unchanged", "file space.txt"],
            None,
        );
        fixture.git(&["update-index", "--skip-worktree", "file space.txt"], None);
        assert!(
            service
                .executor
                .run(&fixture.repo, &args_of(&["ls-files", "-s", "-v", "-z"]))
                .unwrap()
                .checked()
                .unwrap()
                .starts_with(b"S ")
        );
    }
}
