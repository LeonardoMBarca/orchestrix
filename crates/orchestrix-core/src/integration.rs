//! Explicit serial integration and final application. No runtime or model calls.
use crate::{
    domain::*,
    evaluator::{EvaluationResult, evaluate_in_scope},
    git::{self, GitExecutor, integration::IntegrationGit},
    git_records::{WorktreeIntent, content_hash},
    integration_records::*,
    persistence::{Command, Store},
    recovery_guardian::{Guardian, GuardianBinding, default_helper, read_proof},
    runtime::CancellationToken,
    worker::{ManualWorker, WorkerError},
    worker_records::WorkerStatus,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, WorkerError>;

pub trait IntegrationClock: Send + Sync {
    fn now_ms(&self) -> u64;
}
pub struct SystemIntegrationClock;
impl IntegrationClock for SystemIntegrationClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX))
    }
}
pub struct IntegrationService {
    clock: Arc<dyn IntegrationClock>,
}
impl Default for IntegrationService {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegrationCheckpoint {
    Reserved,
    Materialized,
    Checked,
    ResultRef,
    FilesApplied,
    TargetRef,
}
impl IntegrationService {
    pub fn new() -> Self {
        Self {
            clock: Arc::new(SystemIntegrationClock),
        }
    }
    pub fn with_clock(clock: Arc<dyn IntegrationClock>) -> Self {
        Self { clock }
    }
    pub fn prepare(
        &self,
        store: &mut Store,
        input: &IntegrationRequest,
        cancel: &CancellationToken,
    ) -> Result<IntegrationRecord> {
        self.prepare_with_checkpoint(store, input, cancel, |_| {})
    }
    #[doc(hidden)]
    pub fn prepare_with_checkpoint(
        &self,
        store: &mut Store,
        input: &IntegrationRequest,
        cancel: &CancellationToken,
        mut checkpoint: impl FnMut(IntegrationCheckpoint),
    ) -> Result<IntegrationRecord> {
        cancelled(cancel)?;
        if input.schema_version != 1
            || LocalRef::new(&input.id).is_err()
            || LocalRef::new(&input.authorization_ref).is_err()
            || input.checks.is_empty()
            || input.checks.len() > 16
            || !crate::worker_records::exact_safe(&json!(input), 1_048_576)
        {
            return Err(WorkerError("integration-request"));
        }
        let request_hash = content_hash(json!(input).to_string().as_bytes());
        if let Some(old) = store.load()?.integrations().get(&input.id) {
            if old.intent.request_hash != request_hash {
                return Err(WorkerError("integration-id-conflict"));
            }
            return Ok(old.clone());
        }
        let source_worker = ManualWorker::inspect(store, &input.worker_id)?;
        let _lease = crate::worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
        for check in &input.checks {
            check.validate().map_err(WorkerError)?;
        }
        let state = store.load()?;
        admission(&state, None)?;
        if source_worker.status() != WorkerStatus::Verified {
            return Err(WorkerError("verified-source-required"));
        }
        let run = state
            .runs()
            .get(&RunId::new(&source_worker.intent.run_id)?)
            .ok_or(WorkerError("run"))?;
        if run.tasks().len() != 1 {
            return Err(WorkerError("single-code-task-integration-profile"));
        }
        let task = run.task(&TaskId::new(&source_worker.intent.task_id)?)?;
        let exact = task.artifact().ok_or(WorkerError("artifact"))?;
        if task.kind() != TaskKind::Code
            || task.state() != TaskState::Approved
            || exact.revision().as_str() != input.artifact_revision
        {
            return Err(WorkerError("approved-exact-artifact-required"));
        }
        let source = state
            .captured_artifacts()
            .get(&(
                exact.id().as_str().to_owned(),
                input.artifact_revision.clone(),
            ))
            .ok_or(WorkerError("capture"))?
            .clone();
        if source.files.iter().any(|f| f.base != f.index) || !source.exclusions.is_empty() {
            return Err(WorkerError("integration-source-profile"));
        }
        if run.result_revision().as_str() != source.base_oid {
            return Err(WorkerError("integration-base-revalidation-required"));
        }
        let original = state
            .worktrees()
            .get(&source.operation_id)
            .ok_or(WorkerError("source-worktree"))?;
        let target = git::paths::canonical_directory(&input.target_directory)?;
        if target != git::paths::canonical_directory(Path::new(&original.intent.main_repository))? {
            return Err(WorkerError("integration-target-scope"));
        }
        let root = git::paths::canonical_directory(&input.authorized_integration_directory)?;
        let common = git::paths::canonical_directory(Path::new(&original.intent.common_git_dir))?;
        if root == target
            || root == common
            || git::paths::strict_descendant(&target, &root)
            || git::paths::strict_descendant(&common, &root)
            || git::paths::strict_descendant(&root, store.path())
        {
            return Err(WorkerError("integration-directory-scope"));
        }
        let target_state = git::integration::initial_target(
            &GitExecutor::new()?,
            &target,
            &common,
            &source.base_oid,
        )?;
        let hash = content_hash(input.id.as_bytes());
        let path = root.join(format!("run-integration-{}", &hash[..40]));
        git::paths::authorized_child(&root, &path, false)?;
        let now = self.clock.now_ms();
        if now == 0 {
            return Err(WorkerError("integration-clock"));
        }
        let intent = IntegrationIntent {
            launch_protocol: "guarded-git-and-checks-v1".into(),
            id: input.id.clone(),
            request_hash,
            worker_id: input.worker_id.clone(),
            run_id: source_worker.intent.run_id.clone(),
            task_id: source_worker.intent.task_id.clone(),
            artifact: exact.checkpoint(),
            capture_id: source.id.clone(),
            capture_revision: source.revision.clone(),
            input_base: source.base_oid.clone(),
            target: target_state,
            worktree: WorktreeIntent {
                operation_id: format!("run-integration-{hash}"),
                project_id: source_worker.intent.project_id.clone(),
                repository_id: source_worker.intent.repository_id.clone(),
                run_id: source_worker.intent.run_id.clone(),
                task_id: source_worker.intent.task_id.clone(),
                attempt_id: source_worker.intent.attempt_id.clone(),
                base_oid: source.base_oid.clone(),
                domain_base: source.base_oid.clone(),
                worktree_ref: format!("run-worktree-{}", &hash[..24]),
                authorized_root: path_text(&root)?,
                path: path_text(&path)?,
                main_repository: path_text(&target)?,
                branch: format!("orchestrix/integration-{hash}"),
                common_git_dir: path_text(&common)?,
                ownership_token: content_hash(
                    json!([
                        input.id,
                        now,
                        orchestrix_process::named_job::nonce()
                            .map_err(|_| WorkerError("integration-ownership"))?
                    ])
                    .to_string()
                    .as_bytes(),
                ),
            },
            checks: input.checks.clone(),
            authorization_ref: input.authorization_ref.clone(),
            created_ms: now,
        };
        drop(state);
        commit(
            store,
            &input.id,
            Command::Integration(IntegrationCommand::Reserve(Box::new(IntegrationRecord {
                intent,
                observations: vec![],
            }))),
        )?;
        checkpoint(IntegrationCheckpoint::Reserved);
        let outcome = (|| {
            cancelled(cancel)?;
            let record = load_record(store, &input.id)?;
            let helper = default_helper()?;
            let binding = GuardianBinding::prepare(
                store.path(),
                &format!("integration-{}", &hash[..24]),
                &helper,
            )?;
            observe(
                store,
                &input.id,
                IntegrationStatus::Preparing,
                json!({"guardian":binding,"effects":"git-and-checks-owned-v1"}),
            )?;
            let mut guardian = Guardian::start(binding.clone(), &helper)?;
            let git = IntegrationGit::new(guardian.job()?)?;
            if git.target(
                &record.intent,
                &record.intent.input_base,
                &record.intent.input_base,
            )? != record.intent.target
            {
                return Err(WorkerError("target-changed-before-integration"));
            }
            let snapshot = git.materialize(&record.intent, &source)?;
            observe(
                store,
                &input.id,
                IntegrationStatus::Materialized,
                json!({"snapshot":snapshot}),
            )?;
            checkpoint(IntegrationCheckpoint::Materialized);
            cancelled(cancel)?;
            observe(
                store,
                &input.id,
                IntegrationStatus::Checking,
                json!({"snapshotRevision":snapshot.revision}),
            )?;
            let watch = orchestrix_process::watch::DirectoryWatch::start(Path::new(
                &record.intent.worktree.path,
            ))
            .map_err(|_| WorkerError("integration-watch"))?;
            let mut reports = Vec::new();
            for check in &record.intent.checks {
                if git.inspect(&record.intent)?.revision != snapshot.revision
                    || watch
                        .changed()
                        .map_err(|_| WorkerError("integration-watch"))?
                {
                    return Err(WorkerError("combined-artifact-changed"));
                }
                let report = evaluate_in_scope(
                    check,
                    Path::new(&record.intent.worktree.path),
                    &snapshot.revision,
                    cancel,
                    &guardian.job()?,
                )
                .map_err(WorkerError)?;
                let pass = report.result == EvaluationResult::Passed && report.exit_code == Some(0);
                reports.push(report);
                if !pass {
                    drop(git);
                    drop(guardian);
                    let proof = read_proof(&binding)?;
                    observe(
                        store,
                        &input.id,
                        IntegrationStatus::Blocked,
                        json!({"checks":reports,"snapshotRevision":snapshot.revision,"reason":"combined-check-failed","localClosureProof":proof}),
                    )?;
                    return Ok(());
                }
            }
            if git.inspect(&record.intent)?.revision != snapshot.revision
                || watch
                    .changed()
                    .map_err(|_| WorkerError("integration-watch"))?
            {
                return Err(WorkerError("combined-artifact-changed"));
            }
            observe(
                store,
                &input.id,
                IntegrationStatus::Checked,
                json!({"checks":reports,"snapshotRevision":snapshot.revision}),
            )?;
            checkpoint(IntegrationCheckpoint::Checked);
            cancelled(cancel)?;
            let (tree, result) = git.plan_commit(&record.intent, &snapshot)?;
            observe(
                store,
                &input.id,
                IntegrationStatus::CommitPlanned,
                json!({"treeRevision":tree,"resultRevision":result}),
            )?;
            if git.inspect(&record.intent)?.revision != snapshot.revision
                || watch
                    .changed()
                    .map_err(|_| WorkerError("integration-watch"))?
            {
                return Err(WorkerError("combined-artifact-changed"));
            }
            git.commit(&record.intent, &tree, &result)?;
            checkpoint(IntegrationCheckpoint::ResultRef);
            drop(git);
            drop(guardian);
            let proof = read_proof(&binding)?;
            observe(
                store,
                &input.id,
                IntegrationStatus::Integrated,
                json!({"treeRevision":tree,"resultRevision":result,"localClosureProof":proof}),
            )?;
            self.prepare_candidate(store, &input.id)?;
            Ok(())
        })();
        if let Err(error) = outcome {
            let current = load_record(store, &input.id)?;
            if current.status().blocks_admission() {
                observe(
                    store,
                    &input.id,
                    if current.status() == IntegrationStatus::Reserved {
                        IntegrationStatus::Blocked
                    } else {
                        IntegrationStatus::Uncertain
                    },
                    json!({"reason":error.0,"automaticRetry":false,"inspectionRequired":current.status()!=IntegrationStatus::Reserved,"effectsNotEntered":current.status()==IntegrationStatus::Reserved}),
                )?;
            }
        }
        load_record(store, &input.id)
    }
    fn prepare_candidate(&self, store: &mut Store, id: &str) -> Result<()> {
        let record = load_record(store, id)?;
        let state = store.load()?;
        let run = state
            .runs()
            .get(&RunId::new(&record.intent.run_id)?)
            .ok_or(WorkerError("run"))?;
        let mut preview = Run::restore_checkpoint(&run.checkpoint())?;
        let candidate = preview.prepare_delivery(RevisionId::new(&record.intent.target.head)?)?;
        drop(state);
        observe(
            store,
            id,
            IntegrationStatus::Prepared,
            json!({"candidate":candidate.checkpoint(),"delivered":false}),
        )
    }
    pub fn approve(&self, store: &mut Store, input: &ApprovalRequest) -> Result<IntegrationRecord> {
        let _lease = crate::worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
        if input.schema_version != 1
            || LocalRef::new(&input.id).is_err()
            || LocalRef::new(&input.authorization_ref).is_err()
            || !(1_000..=86_400_000).contains(&input.valid_for_ms)
        {
            return Err(WorkerError("approval-request"));
        }
        let record = load_record(store, &input.integration_id)?;
        let hash = content_hash(json!(input).to_string().as_bytes());
        if let Some(old) = record.detail("approval")
            && old["request"]["id"] == input.id
        {
            if old["requestHash"] != hash {
                return Err(WorkerError("approval-id-conflict"));
            }
            return Ok(record);
        }
        if record.status() != IntegrationStatus::Prepared
            || record.candidate_revision().as_deref() != Some(&input.candidate_revision)
        {
            return Err(WorkerError("prepared-exact-candidate-required"));
        }
        self.validate_prepared(&record)?;
        let issued = self.clock.now_ms();
        if issued < record.intent.created_ms {
            return Err(WorkerError("approval-clock-rollback"));
        }
        let expires = issued
            .checked_add(input.valid_for_ms)
            .ok_or(WorkerError("approval-clock"))?;
        observe(
            store,
            &input.integration_id,
            IntegrationStatus::Approved,
            json!({"approval":{"request":input,"requestHash":hash,"issuedMs":issued,"expiresMs":expires,"target":record.intent.target}}),
        )?;
        load_record(store, &input.integration_id)
    }
    fn validate_prepared(&self, record: &IntegrationRecord) -> Result<()> {
        let mut diagnostics = PreparedValidationDiagnostics::new(
            std::env::var_os("ORCHESTRIX_INTEGRATION_DIAGNOSTICS")
                .is_some_and(|value| value == "1"),
        );
        let git =
            diagnostics.measure(PreparedValidationPhase::ReadOnly, IntegrationGit::read_only)?;
        let snapshot = record.snapshot().ok_or(WorkerError("combined-snapshot"))?;
        if diagnostics
            .measure(PreparedValidationPhase::Inspect, || {
                git.inspect(&record.intent)
            })?
            .revision
            != snapshot.revision
        {
            return Err(WorkerError("combined-artifact-changed"));
        }
        let tree = value_text(record, "treeRevision")?;
        let result = value_text(record, "resultRevision")?;
        diagnostics.measure(PreparedValidationPhase::VerifyCommit, || {
            git.verify_commit(&record.intent, tree, result)
        })?;
        let tree = value_text(record, "treeRevision")?;
        diagnostics.measure(PreparedValidationPhase::VerifySnapshotTree, || {
            git.verify_snapshot_tree(&record.intent, tree, &snapshot)
        })?;
        if diagnostics.measure(PreparedValidationPhase::Target, || {
            git.target(
                &record.intent,
                &record.intent.input_base,
                &record.intent.input_base,
            )
        })? != record.intent.target
        {
            return Err(WorkerError("target-changed-revalidation-required"));
        }
        Ok(())
    }
    pub fn apply(
        &self,
        store: &mut Store,
        input: &ApplicationRequest,
        cancel: &CancellationToken,
    ) -> Result<IntegrationRecord> {
        self.apply_with_checkpoint(store, input, cancel, |_| {})
    }
    #[doc(hidden)]
    pub fn apply_with_checkpoint(
        &self,
        store: &mut Store,
        input: &ApplicationRequest,
        cancel: &CancellationToken,
        mut checkpoint: impl FnMut(IntegrationCheckpoint),
    ) -> Result<IntegrationRecord> {
        if input.schema_version != 1
            || LocalRef::new(&input.id).is_err()
            || LocalRef::new(&input.authorization_ref).is_err()
        {
            return Err(WorkerError("application-request"));
        }
        let _lease = crate::worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
        let record = load_record(store, &input.integration_id)?;
        let hash = content_hash(json!(input).to_string().as_bytes());
        if let Some(old) = record.detail("applicationRequestHash") {
            if old != &json!(hash) {
                return Err(WorkerError("application-id-conflict"));
            }
            if matches!(
                record.status(),
                IntegrationStatus::Applied | IntegrationStatus::Delivered
            ) {
                return Ok(record);
            }
            return Err(WorkerError("application-uncertain-reconcile-required"));
        }
        if record.status() != IntegrationStatus::Approved
            || record.candidate_revision().as_deref() != Some(&input.candidate_revision)
        {
            return Err(WorkerError("approved-exact-candidate-required"));
        }
        let now = self.clock.now_ms();
        let approval = record.detail("approval").ok_or(WorkerError("approval"))?;
        if now
            < approval["issuedMs"]
                .as_u64()
                .ok_or(WorkerError("approval-clock"))?
            || now
                >= approval["expiresMs"]
                    .as_u64()
                    .ok_or(WorkerError("approval-clock"))?
        {
            observe(
                store,
                &input.integration_id,
                IntegrationStatus::Invalidated,
                json!({"reason":"approval-expired","applicationNotStarted":true}),
            )?;
            return load_record(store, &input.integration_id);
        }
        if let Err(error) = self.validate_prepared(&record) {
            observe(
                store,
                &input.integration_id,
                IntegrationStatus::Invalidated,
                json!({"reason":error.0,"applicationNotStarted":true}),
            )?;
            return load_record(store, &input.integration_id);
        }
        cancelled(cancel)?;
        let state = store.load()?;
        admission(&state, Some(&record.intent.id))?;
        drop(state);
        let helper = default_helper()?;
        let binding = GuardianBinding::prepare(
            store.path(),
            &format!("apply-{}", &content_hash(input.id.as_bytes())[..24]),
            &helper,
        )?;
        let now = self.clock.now_ms();
        if now
            < approval["issuedMs"]
                .as_u64()
                .ok_or(WorkerError("approval-clock"))?
            || now
                >= approval["expiresMs"]
                    .as_u64()
                    .ok_or(WorkerError("approval-clock"))?
        {
            observe(
                store,
                &input.integration_id,
                IntegrationStatus::Invalidated,
                json!({"reason":"approval-expired-or-clock-rollback","applicationNotStarted":true}),
            )?;
            return load_record(store, &input.integration_id);
        }
        observe(
            store,
            &input.integration_id,
            IntegrationStatus::Applying,
            json!({"guardian":binding,"applicationRequest":input,"applicationRequestHash":hash,"startedMs":now,"effects":"git-and-checks-owned-v1"}),
        )?;
        let mut effect_started = false;
        let outcome = (|| {
            let mut guardian = Guardian::start(binding.clone(), &helper)?;
            let git = IntegrationGit::new(guardian.job()?)?;
            let result = value_text(&record, "resultRevision")?;
            git.verify_commit(&record.intent, value_text(&record, "treeRevision")?, result)?;
            git.verify_snapshot_tree(
                &record.intent,
                value_text(&record, "treeRevision")?,
                &record.snapshot().ok_or(WorkerError("snapshot"))?,
            )?;
            if git.inspect(&record.intent)?.revision
                != record.snapshot().ok_or(WorkerError("snapshot"))?.revision
            {
                return Err(WorkerError("combined-artifact-changed"));
            }
            cancelled(cancel)?;
            let effect_time = self.clock.now_ms();
            if effect_time < now
                || effect_time
                    >= approval["expiresMs"]
                        .as_u64()
                        .ok_or(WorkerError("approval-clock"))?
            {
                return Err(WorkerError("approval-expired-before-effect"));
            }
            effect_started = true;
            git.apply_files(&record.intent, result, || {
                cancelled(cancel)?;
                let before_write = self.clock.now_ms();
                if before_write < effect_time
                    || before_write
                        >= approval["expiresMs"]
                            .as_u64()
                            .ok_or(WorkerError("approval-clock"))?
                {
                    return Err(WorkerError("approval-expired-before-file-update"));
                }
                Ok::<_, WorkerError>(())
            })?;
            checkpoint(IntegrationCheckpoint::FilesApplied);
            // Once files changed, cancellation cannot claim a rollback. Finish
            // only the already-authorized ref CAS, or preserve uncertain state.
            let actual = git.apply_ref(&record.intent, result)?;
            checkpoint(IntegrationCheckpoint::TargetRef);
            drop(git);
            drop(guardian);
            let proof = read_proof(&binding)?;
            observe(
                store,
                &input.integration_id,
                IntegrationStatus::Applied,
                json!({"actualTarget":actual,"resultRevision":result,"localClosureProof":proof}),
            )?;
            observe(
                store,
                &input.integration_id,
                IntegrationStatus::Delivered,
                json!({"delivered":true}),
            )?;
            Ok(())
        })();
        if let Err(error) = outcome {
            let current = load_record(store, &input.integration_id)?;
            if current.status() == IntegrationStatus::Applying {
                observe(
                    store,
                    &input.integration_id,
                    if effect_started {
                        IntegrationStatus::Uncertain
                    } else {
                        IntegrationStatus::Invalidated
                    },
                    json!({"reason":error.0,"automaticRetry":false,"inspectionRequired":effect_started,"applicationNotStarted":!effect_started}),
                )?;
            }
        }
        load_record(store, &input.integration_id)
    }
    /// Explicit expensive revalidation, not the renderer's compact polling path.
    pub fn inspect(&self, store: &mut Store, id: &str) -> Result<IntegrationRecord> {
        let current = load_record(store, id)?;
        if matches!(
            current.status(),
            IntegrationStatus::Prepared | IntegrationStatus::Approved
        ) {
            let _lease = crate::worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
            let expired = current.status() == IntegrationStatus::Approved
                && current.detail("approval").is_some_and(|a| {
                    self.clock.now_ms() >= a["expiresMs"].as_u64().unwrap_or(0)
                        || self.clock.now_ms() < a["issuedMs"].as_u64().unwrap_or(u64::MAX)
                });
            if expired {
                observe(
                    store,
                    id,
                    IntegrationStatus::Invalidated,
                    json!({"reason":"approval-expired"}),
                )?;
            } else if let Err(error) = self.validate_prepared(&current) {
                observe(
                    store,
                    id,
                    IntegrationStatus::Invalidated,
                    json!({"reason":error.0}),
                )?;
            }
        }
        load_record(store, id)
    }
    /// Reopening never repeats checks or writes user files. Only already fully
    /// observed Git effects with a fenced empty own Job can confirm domain facts.
    pub fn reconcile(&self, store: &mut Store) -> Result<Vec<IntegrationRecord>> {
        let Some(_lease) = crate::worker::lease(store)? else {
            return Ok(store.load()?.integrations().values().cloned().collect());
        };
        let ids = store
            .load()?
            .integrations()
            .values()
            .filter(|r| {
                r.status().blocks_admission()
                    || matches!(
                        r.status(),
                        IntegrationStatus::Integrated | IntegrationStatus::Applied
                    )
            })
            .map(|r| r.intent.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            let current = load_record(store, &id)?;
            if current.status() == IntegrationStatus::Reserved {
                observe(
                    store,
                    &id,
                    IntegrationStatus::Blocked,
                    json!({"reason":"recovered-before-effect-scope","effectsNotEntered":true,"automaticRetry":false}),
                )?;
                continue;
            }
            if current.status() == IntegrationStatus::Integrated {
                self.prepare_candidate(store, &id)?;
                continue;
            }
            if current.status() == IntegrationStatus::Applied {
                observe(
                    store,
                    &id,
                    IntegrationStatus::Delivered,
                    json!({"delivered":true,"reconciled":true}),
                )?;
                continue;
            }
            let binding = current
                .detail("guardian")
                .and_then(|v| serde_json::from_value::<GuardianBinding>(v.clone()).ok());
            let proof = binding.as_ref().and_then(|b| read_proof(b).ok());
            if let Some(proof) = proof {
                let git = IntegrationGit::read_only()?;
                if current.detail("applicationRequest").is_some() {
                    if let Ok(result) = value_text(&current, "resultRevision")
                        && let Ok(actual) = git.target(&current.intent, result, result)
                        && git
                            .verify_commit(
                                &current.intent,
                                value_text(&current, "treeRevision")?,
                                result,
                            )
                            .is_ok()
                        && current.snapshot().is_some_and(|snapshot| {
                            git.verify_snapshot_tree(
                                &current.intent,
                                value_text(&current, "treeRevision").unwrap_or(""),
                                &snapshot,
                            )
                            .is_ok()
                        })
                    {
                        observe(
                            store,
                            &id,
                            IntegrationStatus::Applied,
                            json!({"actualTarget":actual,"resultRevision":result,"localClosureProof":proof,"reconciled":true}),
                        )?;
                        observe(
                            store,
                            &id,
                            IntegrationStatus::Delivered,
                            json!({"delivered":true,"reconciled":true}),
                        )?;
                        continue;
                    }
                } else if let (Ok(tree), Ok(result), Some(snapshot)) = (
                    value_text(&current, "treeRevision"),
                    value_text(&current, "resultRevision"),
                    current.snapshot(),
                ) && git.verify_commit(&current.intent, tree, result).is_ok()
                    && git
                        .inspect(&current.intent)
                        .is_ok_and(|actual| actual.revision == snapshot.revision)
                    && git
                        .verify_snapshot_tree(&current.intent, tree, &snapshot)
                        .is_ok()
                {
                    observe(
                        store,
                        &id,
                        IntegrationStatus::Integrated,
                        json!({"treeRevision":tree,"resultRevision":result,"localClosureProof":proof,"reconciled":true}),
                    )?;
                    self.prepare_candidate(store, &id)?;
                    continue;
                }
            }
            if current.status() != IntegrationStatus::Uncertain {
                observe(
                    store,
                    &id,
                    IntegrationStatus::Uncertain,
                    json!({"reason":"integration-effect-not-fully-confirmed","automaticRetry":false,"inspectionRequired":true}),
                )?;
            }
        }
        Ok(store.load()?.integrations().values().cloned().collect())
    }
}
fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(WorkerError("integration-path"))
}
fn cancelled(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(WorkerError("integration-cancelled"))
    } else {
        Ok(())
    }
}
fn value_text<'a>(record: &'a IntegrationRecord, key: &str) -> Result<&'a str> {
    record
        .detail(key)
        .and_then(Value::as_str)
        .ok_or(WorkerError("integration-evidence"))
}
fn load_record(store: &Store, id: &str) -> Result<IntegrationRecord> {
    store
        .load()?
        .integrations()
        .get(id)
        .cloned()
        .ok_or(WorkerError("integration-not-found"))
}
fn admission(state: &crate::persistence::SavedState, except: Option<&str>) -> Result<()> {
    if state
        .workers()
        .values()
        .any(|w| w.status().blocks_admission())
        || state
            .reviews()
            .values()
            .any(|r| r.status().blocks_admission())
        || state
            .integrations()
            .values()
            .any(|r| Some(r.intent.id.as_str()) != except && r.status().blocks_admission())
    {
        return Err(WorkerError("serial-admission-blocked"));
    }
    Ok(())
}
fn commit(store: &mut Store, id: &str, command: Command) -> Result<()> {
    let ordinal = store
        .load()?
        .integrations()
        .get(id)
        .map_or(0, |r| r.observations.len() + 1);
    crate::worker::commit(store, id, &format!("integration-{ordinal}"), command)
}
fn observe(store: &mut Store, id: &str, status: IntegrationStatus, details: Value) -> Result<()> {
    commit(
        store,
        id,
        Command::Integration(IntegrationCommand::Observe {
            id: id.into(),
            status,
            details,
        }),
    )
}

#[derive(Clone, Copy)]
enum PreparedValidationPhase {
    ReadOnly,
    Inspect,
    VerifyCommit,
    VerifySnapshotTree,
    Target,
}

/// Private, opt-in diagnostics: no record contents, paths or provider data.
struct PreparedValidationDiagnostics {
    remaining: u8,
}
impl PreparedValidationDiagnostics {
    fn new(enabled: bool) -> Self {
        Self {
            remaining: if enabled { 5 } else { 0 },
        }
    }
    fn measure<T>(
        &mut self,
        phase: PreparedValidationPhase,
        action: impl FnOnce() -> std::result::Result<T, git::GitError>,
    ) -> std::result::Result<T, git::GitError> {
        if self.remaining == 0 {
            return action();
        }
        let started = Instant::now();
        let result = action();
        let elapsed_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        if let Some(line) = self.line(phase, result.as_ref().err(), elapsed_ms) {
            // A diagnostic sink failure must not change the operation's result.
            let _ = std::io::stderr().lock().write_all(line.as_bytes());
        }
        result
    }
    fn line(
        &mut self,
        phase: PreparedValidationPhase,
        error: Option<&git::GitError>,
        elapsed_ms: u64,
    ) -> Option<String> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let stage = match phase {
            PreparedValidationPhase::ReadOnly => "read-only",
            PreparedValidationPhase::Inspect => "inspect",
            PreparedValidationPhase::VerifyCommit => "verify-commit",
            PreparedValidationPhase::VerifySnapshotTree => "verify-snapshot-tree",
            PreparedValidationPhase::Target => "target",
        };
        let code = match error {
            None => "ok",
            Some(git::GitError::Io) => "io",
            Some(git::GitError::Path) => "path",
            Some(git::GitError::Scope) => "scope",
            Some(git::GitError::Link) => "link",
            Some(git::GitError::Collision) => "collision",
            Some(git::GitError::Unsupported) => "unsupported",
            Some(git::GitError::NotRepository) => "not-repository",
            Some(git::GitError::Base) => "base",
            Some(git::GitError::Ownership) => "ownership",
            Some(git::GitError::MissingAttempt) => "missing-attempt",
            Some(git::GitError::Conflict) => "conflict",
            Some(git::GitError::Process) => "process",
            Some(git::GitError::Deadline) => "deadline",
            Some(git::GitError::OutputLimit) => "output-limit",
            Some(git::GitError::Limit) => "limit",
            Some(git::GitError::InconsistentCapture) => "inconsistent-capture",
            Some(git::GitError::SecretContent) => "secret-content",
            Some(git::GitError::Pending) => "pending",
            Some(git::GitError::UnpreservedWork) => "unpreserved-work",
            Some(git::GitError::Injected) => "injected",
            Some(git::GitError::Storage(_)) => "storage",
        };
        Some(format!(
            "orchestrix-integration-validation stage={stage} code={code} elapsedMs={elapsed_ms}\n"
        ))
    }
}

#[cfg(test)]
mod prepared_validation_diagnostics_tests {
    use super::*;

    #[test]
    fn diagnostics_are_opt_in_bounded_and_static() {
        let mut disabled = PreparedValidationDiagnostics::new(false);
        assert_eq!(
            disabled.line(
                PreparedValidationPhase::Inspect,
                Some(&git::GitError::Io),
                1
            ),
            None
        );
        let mut enabled = PreparedValidationDiagnostics::new(true);
        for (phase, expected) in [
            (PreparedValidationPhase::ReadOnly, "read-only"),
            (PreparedValidationPhase::Inspect, "inspect"),
            (PreparedValidationPhase::VerifyCommit, "verify-commit"),
            (
                PreparedValidationPhase::VerifySnapshotTree,
                "verify-snapshot-tree",
            ),
            (PreparedValidationPhase::Target, "target"),
        ] {
            let line = enabled.line(phase, None, u64::MAX).unwrap();
            assert_eq!(
                line,
                format!(
                    "orchestrix-integration-validation stage={expected} code=ok elapsedMs={}\n",
                    u64::MAX
                )
            );
            assert!(line.len() < 128);
        }
        assert_eq!(enabled.line(PreparedValidationPhase::Target, None, 0), None);

        // Nested persistence details must never reach the private diagnostic line.
        let mut storage = PreparedValidationDiagnostics::new(true);
        assert_eq!(
            storage.line(
                PreparedValidationPhase::Inspect,
                Some(&git::GitError::Storage(
                    crate::persistence::StoreError::InvalidPath
                )),
                7,
            ),
            Some(
                "orchestrix-integration-validation stage=inspect code=storage elapsedMs=7\n".into()
            )
        );
        let mut process = PreparedValidationDiagnostics::new(true);
        assert_eq!(
            process.line(
                PreparedValidationPhase::VerifyCommit,
                Some(&git::GitError::Process),
                3
            ),
            Some(
                "orchestrix-integration-validation stage=verify-commit code=process elapsedMs=3\n"
                    .into()
            )
        );
    }
}
