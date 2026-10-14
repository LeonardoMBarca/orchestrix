//! Serial manual worker. Effects occur between short durable commands, never in a SQL transaction.
use crate::{
    domain::*,
    evaluator::*,
    git::{self, CaptureLimits, CreateWorktree, GitExecutor, WorktreeService},
    git_records::{CapturedArtifact, content_hash, relative_file_path},
    persistence::{AttemptSnapshot, Command, Disposition, Request, RunAction, Store},
    persistence_records::{DefinitionInput, DefinitionKind, SnapshotInput},
    policy::{
        self, AuthorizedContextFile, ContextBudget, ContextInput, ContextSource,
        ContextSourceIdentity, ContextSourceKind, ObservationLease, ObservedRuntime,
        PolicyDocument, TaskClassification,
    },
    runtime::{
        AttemptExecution, CancellationToken, CodexAdapter, RuntimeError, RuntimeLaunch,
        RuntimeMetadata,
    },
    worker_records::*,
};
use orchestrix_process::watch::DirectoryWatch;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFileRequest {
    pub path: String,
    pub authorization_ref: String,
    pub required: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualTaskRequest {
    pub schema_version: u32,
    pub id: String,
    pub project_id: String,
    pub repository_id: String,
    pub repository_directory: PathBuf,
    pub authorized_worktree_directory: PathBuf,
    pub base_commit: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    pub policy: PolicyDocument,
    pub classification: TaskClassification,
    pub context_files: Vec<ContextFileRequest>,
    pub instructions: Vec<String>,
    pub checks: Vec<CheckSpec>,
}
#[derive(Debug)]
pub struct WorkerError(pub &'static str);
impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Manual worker: {}", self.0)
    }
}
impl std::error::Error for WorkerError {}
impl From<DomainError> for WorkerError {
    fn from(_: DomainError) -> Self {
        Self("domain")
    }
}
impl From<crate::persistence::StoreError> for WorkerError {
    fn from(_: crate::persistence::StoreError) -> Self {
        Self("persistence")
    }
}
impl From<git::GitError> for WorkerError {
    fn from(_: git::GitError) -> Self {
        Self("git-inspection-required")
    }
}
impl From<policy::PolicyError> for WorkerError {
    fn from(_: policy::PolicyError) -> Self {
        Self("policy-or-context")
    }
}
impl From<RuntimeError> for WorkerError {
    fn from(e: RuntimeError) -> Self {
        Self(e.code())
    }
}
type Result<T> = std::result::Result<T, WorkerError>;

pub struct ManualWorker;
impl ManualWorker {
    pub fn run(
        store: &mut Store,
        input: &ManualTaskRequest,
        launch: RuntimeLaunch,
        cancel: &CancellationToken,
    ) -> Result<WorkerRecord> {
        let body = serde_json::to_value(input).map_err(|_| WorkerError("manifest"))?;
        if !exact_safe(&body, 1_048_576)
            || input.schema_version != 1
            || LocalRef::new(&input.id).is_err()
        {
            return Err(WorkerError("manifest"));
        }
        let request_hash = content_hash(body.to_string().as_bytes());
        if let Some(old) = store.load()?.workers().get(&input.id) {
            if old.intent.request_hash != request_hash {
                return Err(WorkerError("request-id-conflict"));
            }
            return Self::inspect(store, &input.id);
        }
        let _lease = lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
        if store
            .load()?
            .workers()
            .values()
            .any(|w| !w.status().terminal() || w.status() == WorkerStatus::Uncertain)
        {
            return Err(WorkerError("serial-admission-blocked"));
        }
        if input.checks.len() > 16
            || input.context_files.len() > 32
            || input.instructions.len() > 32
        {
            return Err(WorkerError("manifest-limit"));
        }
        let mut check_ids = std::collections::BTreeSet::new();
        for check in &input.checks {
            check.validate().map_err(WorkerError)?;
            if !check_ids.insert(&check.id) {
                return Err(WorkerError("duplicate-check"));
            }
        }
        for required in input
            .policy
            .layers
            .iter()
            .flat_map(|l| l.requirements.required_checks.iter())
            .chain(input.policy.mode_requirements.required_checks.iter())
        {
            if !check_ids.contains(required) {
                return Err(WorkerError("required-check-missing"));
            }
        }
        let repository = git::paths::canonical_directory(&input.repository_directory)?;
        let authorized = git::paths::canonical_directory(&input.authorized_worktree_directory)?;
        if input.base_commit.len() != 40 && input.base_commit.len() != 64 {
            return Err(WorkerError("base-commit"));
        }
        if !input
            .base_commit
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(WorkerError("base-commit"));
        }
        if cancel.is_cancelled() {
            return Err(WorkerError("cancelled-before-reservation"));
        }
        // Mechanical classifications are handled by the explicit local query entry point, never inferred by a model.
        if input.classification.kind.is_mechanical() {
            return Err(WorkerError("use-authorized-deterministic-query"));
        }
        let hash = content_hash(input.id.as_bytes());
        let suffix = &hash[..24];
        let diagnostic_id = format!("diagnostic-{suffix}");
        static METADATA: std::sync::OnceLock<crate::runtime::CodexMetadataService> =
            std::sync::OnceLock::new();
        let discovery = METADATA.get_or_init(|| {
            crate::runtime::CodexMetadataService::new(
                std::sync::Arc::new(crate::runtime::SystemRefreshClock),
                crate::runtime::refresh::RefreshBounds::default(),
            )
            .expect("constant refresh bounds")
        });
        let metadata = match discovery.read(
            launch.clone().with_cwd(repository.clone()),
            &input.connection_id,
            input.connection_revision,
            true,
            cancel,
        )? {
            crate::runtime::MetadataRead::Ready { metadata, .. } => metadata,
            _ => return Err(WorkerError("metadata-refresh-pending")),
        };
        let runtime_observation = observed(&metadata, &diagnostic_id)?;
        let decision = policy::resolve(
            &input.policy,
            &input.classification,
            std::slice::from_ref(&runtime_observation),
            now()?,
        )?;
        let selection = decision.selection().ok_or(WorkerError("policy-blocked"))?;
        if selection.concurrency != 1 || selection.service_tier.is_some() {
            return Err(WorkerError("unsupported-initial-worker-control"));
        }
        let run_id = RunId::new(format!("run-{suffix}"))?;
        let task_id = TaskId::new(format!("task-{suffix}"))?;
        let attempt_id = AttemptId::new(format!("attempt-{suffix}"))?;
        let session_id = RuntimeSessionId::new(format!("session-{suffix}"))?;
        let context_id = format!("context-{suffix}");
        let context = context(
            input,
            &repository,
            &context_id,
            selection.context_bytes.min(32768),
        )?;
        let runtime_request =
            selection.runtime_request(session_id.as_str(), attempt_id.as_str(), &context_id)?;
        let connection = Connection::new(metadata.descriptor.clone())?;
        let session = RuntimeSessionRef::new(session_id, &connection);
        let project = Project::new(
            ProjectId::new(&input.project_id)?,
            Repository::new(
                RepositoryId::new(&input.repository_id)?,
                LocalRef::new(format!("directory-{}", input.project_id))?,
            ),
        );
        let mode = ActionMode::new(
            ModeId::new(&input.policy.mode.id)?,
            DefinitionVersion::new(input.policy.mode.version)?,
            "Manual work",
            "User-directed work under the captured policy",
        )?;
        let profile = AgentProfile::new(
            ProfileId::new(&input.policy.profile.id)?,
            DefinitionVersion::new(input.policy.profile.version)?,
            "Executor",
            "Execute the authorized Context Pack",
            LocalRef::new("manual-instructions")?,
        )?;
        let attempt = TaskAttempt::new(
            &connection,
            &session,
            runtime_request.clone(),
            AttemptInputs {
                plan_revision: RevisionId::new(decision.snapshot().revision())?,
                context: ContextVersion {
                    reference: ContextPackId::new(&context_id)?,
                    version: DefinitionVersion::new(1)?,
                },
                mode,
                profile,
                base: RevisionId::new(&input.base_commit)?,
                worktree: LocalRef::new(format!("worktree-{suffix}"))?,
                project: Some(project.id().clone()),
                correction_of: None,
            },
        )?;
        let mut run = Run::new(
            run_id.clone(),
            Some(&project),
            RevisionId::new(&input.base_commit)?,
            DeliveryRequirement::FinalApplication,
        )?;
        run.add_task(Task::new(
            task_id.clone(),
            &input.objective,
            TaskKind::Code,
            input.acceptance_criteria.clone(),
            vec![],
        )?)?;
        run.make_ready(&task_id)?;
        let definitions = vec![
            definition(
                DefinitionKind::Mode,
                &input.policy.mode.id,
                input.policy.mode.version,
                json!({"name":"Manual work","purpose":"User-directed work under the captured policy"}),
            ),
            definition(
                DefinitionKind::Profile,
                &input.policy.profile.id,
                input.policy.profile.version,
                json!({"name":"Executor","role":"Execute the authorized Context Pack","instructions_ref":"manual-instructions"}),
            ),
            definition(
                DefinitionKind::Context,
                &context_id,
                1,
                json!({"manifest_ref":context.revision()}),
            ),
            definition(
                DefinitionKind::Configuration,
                &format!("configuration-{suffix}"),
                1,
                decision.snapshot().as_json().clone(),
            ),
        ];
        let snapshot = AttemptSnapshot {
            definition: SnapshotInput {
                id: format!("snapshot-{suffix}"),
                attempt_id: attempt_id.as_str().into(),
                mode: definitions[0].reference(),
                profile: definitions[1].reference(),
                context: definitions[2].reference(),
                configuration: definitions[3].reference(),
            },
            diagnostics: vec![diagnostic_id.clone()],
        };
        let intent = WorkerIntent {
            id: input.id.clone(),
            request_hash,
            run_id: run_id.as_str().into(),
            task_id: task_id.as_str().into(),
            attempt_id: attempt_id.as_str().into(),
            project_id: input.project_id.clone(),
            repository_id: input.repository_id.clone(),
            base_oid: input.base_commit.clone(),
            policy: decision.snapshot().as_json().clone(),
            context: context.as_json().clone(),
            authorized_checks: json!(input.checks),
        };
        let diagnostics = vec![diagnostic_input(
            store,
            &metadata,
            &runtime_observation,
            &diagnostic_id,
        )?];
        commit(
            store,
            &input.id,
            "reserve",
            Command::Worker(WorkerCommand::Reserve(Box::new(WorkerReservation {
                intent,
                project,
                connection,
                session,
                run,
                attempt,
                snapshot,
                definitions,
                diagnostics,
            }))),
        )?;
        // Any interruption after this point leaves a durable attempt and will not automatically dispatch again.
        let outcome = Self::execute(
            store,
            input,
            launch,
            cancel,
            runtime_request,
            context.rendered().to_owned(),
            &repository,
            &authorized,
            suffix,
            &metadata,
        );
        if let Err(error) = outcome {
            let current = record(store, &input.id)?;
            if !current.status().terminal() {
                let dispatched = current
                    .observations
                    .iter()
                    .any(|o| o.status == WorkerStatus::DispatchPlanned);
                if dispatched {
                    transport_lost(store, &current, "worker-effect-not-confirmed")?;
                }
                observe(
                    store,
                    &input.id,
                    if dispatched {
                        WorkerStatus::Uncertain
                    } else {
                        WorkerStatus::Blocked
                    },
                    json!({"reason":error.0,"inspectionRequired":true,"synthetic":metadata.synthetic}),
                )?;
            }
        }
        record(store, &input.id)
    }
    #[allow(clippy::too_many_arguments)]
    fn execute(
        store: &mut Store,
        input: &ManualTaskRequest,
        launch: RuntimeLaunch,
        cancel: &CancellationToken,
        request: crate::runtime_contract::RuntimeRequest,
        prompt: String,
        repository: &Path,
        authorized: &Path,
        suffix: &str,
        expected_metadata: &RuntimeMetadata,
    ) -> Result<()> {
        let initial = record(store, &input.id)?;
        let service = WorktreeService::new()?;
        let worktree = service.create(
            store,
            &CreateWorktree {
                scope: LocalRef::new(&input.id)?,
                key: LocalRef::new(format!("create-{suffix}"))?,
                expected_revision: store.load()?.revision(),
                project: ProjectId::new(&input.project_id)?,
                repository: RepositoryId::new(&input.repository_id)?,
                run: RunId::new(&initial.intent.run_id)?,
                task: TaskId::new(&initial.intent.task_id)?,
                attempt: AttemptId::new(&initial.intent.attempt_id)?,
                directory_reference: LocalRef::new(format!("directory-{}", input.project_id))?,
                worktree_reference: LocalRef::new(format!("worktree-{suffix}"))?,
                repository_path: repository.into(),
                authorized_directory: authorized.into(),
                base_commit: input.base_commit.clone(),
            },
        )?;
        let path = PathBuf::from(&worktree.intent.path);
        let mut adapter = CodexAdapter::connect(
            launch.with_cwd(path.clone()),
            &input.connection_id,
            input.connection_revision,
            now()?,
            cancel,
        )?;
        if !expected_metadata.same_identity(adapter.metadata())
            || expected_metadata.descriptor.as_json() != adapter.metadata().descriptor.as_json()
            || expected_metadata.controls != adapter.metadata().controls
        {
            return Err(WorkerError("metadata-changed-before-dispatch"));
        }
        let admission_id = format!("admission-{suffix}");
        let fresh = observed(adapter.metadata(), &admission_id)?;
        let revalidated = policy::resolve(
            &input.policy,
            &input.classification,
            std::slice::from_ref(&fresh),
            now()?,
        )?;
        let selected = revalidated
            .selection()
            .ok_or(WorkerError("policy-revalidation-blocked"))?;
        if selected
            .runtime_request(
                request.as_json()["sessionId"]
                    .as_str()
                    .ok_or(WorkerError("session"))?,
                &initial.intent.attempt_id,
                request.as_json()["contextPackRef"]
                    .as_str()
                    .ok_or(WorkerError("context"))?,
            )?
            .as_json()
            != request.as_json()
        {
            return Err(WorkerError("policy-changed-before-dispatch"));
        }
        let admission_diagnostic =
            diagnostic_input(store, adapter.metadata(), &fresh, &admission_id)?;
        commit(
            store,
            &input.id,
            "admission-diagnostic",
            Command::Records(crate::persistence_records::RecordsCommand::Diagnostic(
                admission_diagnostic,
            )),
        )?;
        // Admission and selected configuration are immutable; execute revalidates the observed thread settings.
        observe(
            store,
            &input.id,
            WorkerStatus::DispatchPlanned,
            json!({"worktreeOperation":worktree.intent.operation_id,"synthetic":adapter.metadata().synthetic,"admissionDiagnostic":admission_id}),
        )?;
        let mut sequence = 0usize;
        let report=adapter.execute(AttemptExecution{request,prompt,base_instructions:"Follow only the authorized Context Pack. Do not delegate. Preserve the user's source repository.".into(),developer_instructions:"Use the owned worktree only. Do not execute tests or scripts: the host runs explicitly authorized checks after capture.".into(),resume:None},cancel,|frame,origin|{
            sequence+=1;
            commit(store,&input.id,&format!("event-{sequence}"),Command::run(RunId::new(&initial.intent.run_id).map_err(|_|RuntimeError::observation_failed())?,RunAction::Observe{task:TaskId::new(&initial.intent.task_id).map_err(|_|RuntimeError::observation_failed())?,frame:frame.to_string(),origin})).map_err(|_|RuntimeError::observation_failed())
        })?;
        let tree_stopped = report
            .tree
            .as_ref()
            .is_some_and(|t| t.owned && t.active_processes == 0);
        let drained = report.streams.stdin_closed
            && report.streams.stdout_closed
            && report.streams.stderr_closed;
        observe(
            store,
            &input.id,
            WorkerStatus::RuntimeObserved,
            json!({"dispatch":report.dispatch,"fault":report.fault.as_ref().map(RuntimeError::code),"treeStopped":tree_stopped,"streamsClosed":drained,"synthetic":report.synthetic,"adapterBinding":report.binding.as_ref().map(|b|b.checkpoint()),"adapterDiagnostic":report.adapter_diagnostic,"worktreeOperation":worktree.intent.operation_id}),
        )?;
        let current = record(store, &input.id)?;
        let state = store.load()?;
        let attempt_state = state.runs()[&RunId::new(&current.intent.run_id)?]
            .task(&TaskId::new(&current.intent.task_id)?)?
            .attempts()
            .last()
            .ok_or(WorkerError("attempt"))?
            .state();
        drop(state);
        if !tree_stopped
            || !drained
            || report.fault.is_some()
            || !matches!(
                attempt_state,
                AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
            )
        {
            transport_lost(store, &current, "runtime-outcome-not-confirmed")?;
            if tree_stopped {
                let _ = capture_store(
                    &service,
                    store,
                    &input.id,
                    &worktree.intent.operation_id,
                    "uncertain-artifact",
                );
            }
            return observe(
                store,
                &input.id,
                WorkerStatus::Uncertain,
                json!({"reason":"runtime-outcome-not-confirmed","synthetic":report.synthetic,"inspectionRequired":true,"worktreeOperation":worktree.intent.operation_id}),
            );
        }
        let artifact = capture_store(
            &service,
            store,
            &input.id,
            &worktree.intent.operation_id,
            "runtime-artifact",
        )?;
        if attempt_state != AttemptState::Succeeded {
            return observe(
                store,
                &input.id,
                if attempt_state == AttemptState::Cancelled {
                    WorkerStatus::Cancelled
                } else {
                    WorkerStatus::Failed
                },
                json!({"artifactRevision":artifact.revision,"synthetic":report.synthetic,"worktreeOperation":worktree.intent.operation_id}),
            );
        }
        let version = artifact_version(&artifact)?;
        commit(
            store,
            &input.id,
            "publish",
            Command::run(
                RunId::new(&current.intent.run_id)?,
                RunAction::Publish {
                    task: TaskId::new(&current.intent.task_id)?,
                    artifact: version.clone(),
                },
            ),
        )?;
        if input.checks.is_empty() {
            return observe(
                store,
                &input.id,
                WorkerStatus::NoChecks,
                json!({"artifactRevision":artifact.revision,"synthetic":report.synthetic,"worktreeOperation":worktree.intent.operation_id,"reason":"no-authorized-checks"}),
            );
        }
        // Observe both working files and Git admin/index/refs. Any write, including write+restore, invalidates this conservative profile.
        let files_watch =
            DirectoryWatch::start(&path).map_err(|_| WorkerError("verification-watch"))?;
        let git_watch = DirectoryWatch::start(Path::new(&worktree.intent.common_git_dir))
            .map_err(|_| WorkerError("verification-watch"))?;
        let executor = GitExecutor::new()?;
        let mut checks = vec![];
        let mut all_passed = true;
        let mut invalidated = false;
        for (n, spec) in input.checks.iter().enumerate() {
            let before =
                git::capture(&executor, &path, &worktree.intent, CaptureLimits::default())?;
            if before.revision != artifact.revision || changed(&files_watch, &git_watch)? {
                invalidated = true;
                break;
            }
            let mut check =
                evaluate(spec, &path, &artifact.revision, cancel).map_err(WorkerError)?;
            let after = git::capture(&executor, &path, &worktree.intent, CaptureLimits::default());
            let stable = after
                .as_ref()
                .is_ok_and(|a| a.revision == artifact.revision)
                && !changed(&files_watch, &git_watch)?;
            let passed = check.result == EvaluationResult::Passed && stable;
            if !stable {
                check.result = EvaluationResult::Invalidated;
            }
            invalidated |= !stable;
            all_passed &= passed;
            commit(
                store,
                &input.id,
                &format!("verify-{n}"),
                Command::run(
                    RunId::new(&current.intent.run_id)?,
                    RunAction::Verify {
                        task: TaskId::new(&current.intent.task_id)?,
                        evidence: VerificationEvidence {
                            artifact: version.clone(),
                            reference: EvidenceId::new(format!("check-{suffix}-{n}"))?,
                            result: if passed {
                                CheckResult::Passed
                            } else {
                                CheckResult::Failed
                            },
                        },
                    },
                ),
            )?;
            checks.push(check);
            // A failed verification moves the domain to NeedsWork. Remaining commands are not authorized as a retry.
            if !passed || cancel.is_cancelled() {
                break;
            }
        }
        invalidated |= changed(&files_watch, &git_watch)?;
        if invalidated {
            invalidate_evidence(store, &current, &version, "changed-during-checks")?;
        }
        let status = if invalidated {
            WorkerStatus::Invalidated
        } else if all_passed && checks.len() == input.checks.len() {
            WorkerStatus::Verified
        } else {
            WorkerStatus::NeedsWork
        };
        observe(
            store,
            &input.id,
            status,
            json!({"artifactRevision":artifact.revision,"checksOmitted":input.checks.iter().skip(checks.len()).map(|c|json!({"id":c.id,"reason":"prior-check-failed-or-artifact-invalidated"})).collect::<Vec<_>>(),"checks":checks,"synthetic":report.synthetic,"worktreeOperation":worktree.intent.operation_id,"review":"pending-independent-review","delivered":false}),
        )?;
        if status == WorkerStatus::Verified && changed(&files_watch, &git_watch)? {
            invalidate_evidence(store, &current, &version, "changed-after-checks")?;
            observe(
                store,
                &input.id,
                WorkerStatus::Invalidated,
                json!({"reason":"changed-after-checks","artifactRevision":artifact.revision,"worktreeOperation":worktree.intent.operation_id}),
            )?;
        }
        Ok(())
    }
    /// Reopening never starts a process. Historical checks remain facts, not a claim about new bytes.
    pub fn inspect(store: &mut Store, id: &str) -> Result<WorkerRecord> {
        let current = record(store, id)?;
        let Some(_lease) = lease(store)? else {
            return Ok(current);
        };
        if !current.status().terminal() {
            transport_lost(store, &current, "host-reopened-without-terminal")?;
            observe(
                store,
                id,
                WorkerStatus::Uncertain,
                json!({"reason":"host-reopened-without-terminal","inspectionRequired":true}),
            )?;
        } else if current.status() == WorkerStatus::Verified {
            let last = current
                .observations
                .last()
                .ok_or(WorkerError("observation"))?;
            let operation = last.details["worktreeOperation"]
                .as_str()
                .ok_or(WorkerError("worktree"))?;
            let artifact_revision = last.details["artifactRevision"]
                .as_str()
                .ok_or(WorkerError("artifact"))?;
            let service = WorktreeService::new()?;
            let checked = service.inspect(store, operation).and_then(|w| {
                git::capture(
                    &GitExecutor::new()?,
                    Path::new(&w.intent.path),
                    &w.intent,
                    CaptureLimits::default(),
                )
            });
            if !checked
                .as_ref()
                .is_ok_and(|a| a.revision == artifact_revision)
            {
                let state = store.load()?;
                let artifact = state.runs()[&RunId::new(&current.intent.run_id)?]
                    .task(&TaskId::new(&current.intent.task_id)?)?
                    .artifact()
                    .cloned()
                    .ok_or(WorkerError("artifact"))?;
                invalidate_evidence(store, &current, &artifact, "changed-after-verification")?;
                observe(
                    store,
                    id,
                    WorkerStatus::Invalidated,
                    json!({"reason":"changed-or-uninspectable-after-verification","artifactRevision":artifact_revision,"worktreeOperation":operation}),
                )?;
            }
        }
        record(store, id)
    }
}
fn lease(store: &Store) -> Result<Option<orchestrix_process::lease::HostLease>> {
    let path = store.path();
    let mut filename = path
        .file_name()
        .ok_or(WorkerError("database-path"))?
        .to_os_string();
    filename.push(".worker-lock");
    orchestrix_process::lease::HostLease::acquire(&path.with_file_name(filename))
        .map_err(|_| WorkerError("worker-host-ownership"))
}
fn changed(files: &DirectoryWatch, git: &DirectoryWatch) -> Result<bool> {
    Ok(files
        .changed()
        .map_err(|_| WorkerError("verification-watch"))?
        || git
            .changed()
            .map_err(|_| WorkerError("verification-watch"))?)
}
fn record(store: &Store, id: &str) -> Result<WorkerRecord> {
    store
        .load()?
        .workers()
        .get(id)
        .cloned()
        .ok_or(WorkerError("unknown-worker"))
}
fn commit(store: &mut Store, id: &str, key: &str, command: Command) -> Result<()> {
    let result = store.execute(&Request {
        scope: LocalRef::new(id)?,
        key: LocalRef::new(key)?,
        expected_revision: store.load()?.revision(),
        command,
    })?;
    if result.result.disposition != Disposition::Applied {
        return Err(WorkerError("command-rejected"));
    }
    Ok(())
}
fn observe(store: &mut Store, id: &str, status: WorkerStatus, details: Value) -> Result<()> {
    let record = record(store, id)?;
    if status == WorkerStatus::Invalidated && record.status() == WorkerStatus::Invalidated {
        return Ok(());
    }
    let n = record.observations.len();
    commit(
        store,
        id,
        &format!("worker-observation-{n}"),
        Command::Worker(WorkerCommand::Observe {
            id: id.into(),
            status,
            details,
        }),
    )
}
fn transport_lost(store: &mut Store, record: &WorkerRecord, reason: &str) -> Result<()> {
    let state = store.load()?;
    let task = state.runs()[&RunId::new(&record.intent.run_id)?]
        .task(&TaskId::new(&record.intent.task_id)?)?;
    if task.attempts().last().is_some_and(|a| {
        !matches!(
            a.state(),
            AttemptState::Succeeded
                | AttemptState::Failed
                | AttemptState::Cancelled
                | AttemptState::Uncertain
        )
    }) {
        commit(
            store,
            &record.intent.id,
            "transport-lost",
            Command::run(
                RunId::new(&record.intent.run_id)?,
                RunAction::TransportLost {
                    task: TaskId::new(&record.intent.task_id)?,
                    reason: reason.into(),
                },
            ),
        )?;
    }
    Ok(())
}
fn invalidate_evidence(
    store: &mut Store,
    record: &WorkerRecord,
    artifact: &ArtifactVersion,
    reason: &str,
) -> Result<()> {
    // Withdraw a positive current summary before appending negative evidence. A crash between commands stays non-green.
    let current = self::record(store, &record.intent.id)?;
    if current.status() == WorkerStatus::Verified {
        let mut details = current
            .observations
            .last()
            .ok_or(WorkerError("observation"))?
            .details
            .clone();
        details["reason"] = json!(reason);
        observe(store, &record.intent.id, WorkerStatus::Invalidated, details)?;
    }
    let state = store.load()?;
    let task = state.runs()[&RunId::new(&record.intent.run_id)?]
        .task(&TaskId::new(&record.intent.task_id)?)?;
    if task.artifact() != Some(artifact)
        || task
            .verifications()
            .iter()
            .any(|v| &v.artifact == artifact && v.result == CheckResult::Failed)
    {
        return Ok(());
    }
    let n = state.revision();
    commit(
        store,
        &record.intent.id,
        &format!("invalidate-{n}"),
        Command::run(
            RunId::new(&record.intent.run_id)?,
            RunAction::Verify {
                task: TaskId::new(&record.intent.task_id)?,
                evidence: VerificationEvidence {
                    artifact: artifact.clone(),
                    reference: EvidenceId::new(format!("{reason}-{n}"))?,
                    result: CheckResult::Failed,
                },
            },
        ),
    )
}
fn artifact_version(a: &CapturedArtifact) -> Result<ArtifactVersion> {
    Ok(ArtifactVersion::new(
        ArtifactId::new(&a.id)?,
        RevisionId::new(&a.revision)?,
        RevisionId::new(&a.base_oid)?,
    ))
}
fn capture_store(
    service: &WorktreeService,
    store: &mut Store,
    id: &str,
    operation: &str,
    key: &str,
) -> Result<CapturedArtifact> {
    Ok(service.capture(
        store,
        &LocalRef::new(id)?,
        &LocalRef::new(key)?,
        store.load()?.revision(),
        operation,
    )?)
}
fn definition(kind: DefinitionKind, id: &str, version: u64, body: Value) -> DefinitionInput {
    DefinitionInput {
        kind,
        id: id.into(),
        version,
        body,
        references: vec![],
    }
}
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WorkerError("clock"))?
        .as_millis()
        .try_into()
        .map_err(|_| WorkerError("clock"))
}
fn observed(m: &RuntimeMetadata, evidence_ref: &str) -> Result<ObservedRuntime> {
    let d = m.descriptor.as_json();
    let mut facts = vec![
        "identity".to_owned(),
        "billing".into(),
        "availability".into(),
        "catalog".into(),
    ];
    facts.extend(m.controls.iter().cloned());
    if let Some(caps) = d["capabilities"].as_object() {
        facts.extend(caps.keys().map(|s| format!("capability:{s}")));
    }
    let observations = facts
        .into_iter()
        .map(|fact| ObservationLease {
            fact,
            model_id: None,
            evidence_ref: evidence_ref.into(),
            connection_id: d["connection"]["id"].as_str().unwrap_or("").into(),
            connection_revision: d["connection"]["revision"].as_u64().unwrap_or(0),
            runtime_version: d["runtimeVersion"].as_str().unwrap_or("").into(),
            observed_at_ms: m.observed_at_ms,
            valid_until_ms: m.observed_at_ms.saturating_add(60_000),
        })
        .collect();
    Ok(ObservedRuntime::new(m.descriptor.clone(), observations)?)
}
fn context(
    input: &ManualTaskRequest,
    repository: &Path,
    id: &str,
    max_bytes: usize,
) -> Result<policy::ContextPack> {
    let mut files = vec![];
    for file in &input.context_files {
        if !relative_file_path(&file.path) || LocalRef::new(&file.authorization_ref).is_err() {
            return Err(WorkerError("context-file-authorization"));
        }
        let path = git::paths::authorized_child(repository, &repository.join(&file.path), true)?;
        if !path.is_file() {
            return Err(WorkerError("context-file-type"));
        }
        let mut bytes = vec![];
        std::fs::File::open(&path)
            .map_err(|_| WorkerError("context-file-read"))?
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|_| WorkerError("context-file-read"))?;
        if bytes.len() > 1_048_576 {
            return Err(WorkerError("context-file-size"));
        }
        files.push(AuthorizedContextFile {
            path: file.path.clone(),
            source: ContextSourceIdentity {
                reference: format!("source-{}", files.len()),
                revision: content_hash(&bytes),
            },
            authorization_ref: Some(file.authorization_ref.clone()),
            content: String::from_utf8(bytes).map_err(|_| WorkerError("context-file-encoding"))?,
            required: file.required,
            priority: 0,
        });
    }
    let sources = input
        .instructions
        .iter()
        .enumerate()
        .map(|(n, text)| ContextSource {
            identity: ContextSourceIdentity {
                reference: format!("user-instruction-{n}"),
                revision: content_hash(text.as_bytes()),
            },
            kind: ContextSourceKind::UserInstructions,
            authorization_ref: Some("manual-manifest".into()),
            text: text.clone(),
            required: true,
            priority: 0,
        })
        .collect();
    Ok(policy::compile_context(
        &ContextInput {
            schema_version: 1,
            id: id.into(),
            version: 1,
            objective: input.objective.clone(),
            acceptance_criteria: input.acceptance_criteria.clone(),
            constraints: vec![
                "Only the authorized worktree; no native delegation or paid fallback.".into(),
            ],
            sources,
            files,
        },
        ContextBudget {
            max_bytes,
            ..ContextBudget::default()
        },
    )?)
}

fn diagnostic_input(
    store: &Store,
    metadata: &RuntimeMetadata,
    observation: &ObservedRuntime,
    id: &str,
) -> Result<crate::persistence_records::DiagnosticInput> {
    use crate::persistence_records::*;
    let descriptor = metadata.descriptor.as_json();
    let connection_id = descriptor["connection"]["id"]
        .as_str()
        .ok_or(WorkerError("connection"))?;
    let connection_revision = descriptor["connection"]["revision"]
        .as_u64()
        .ok_or(WorkerError("connection"))?;
    let version = store
        .load()?
        .records()
        .diagnostics()
        .iter()
        .filter(|d| {
            d.connection_id == connection_id && d.scope.kind == DiagnosticScopeKind::Connection
        })
        .map(|d| d.version)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(WorkerError("diagnostic-version"))?;
    let stamp = |milliseconds: u64| -> Result<String> {
        let milliseconds: i64 = milliseconds.try_into().map_err(|_| WorkerError("clock"))?;
        Ok(chrono::DateTime::from_timestamp_millis(milliseconds)
            .ok_or(WorkerError("clock"))?
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
    };
    Ok(DiagnosticInput {
        id: id.into(),
        connection_id: connection_id.into(),
        connection_revision,
        version,
        scope: DiagnosticScope {
            kind: DiagnosticScopeKind::Connection,
            reference: None,
        },
        source: if metadata.synthetic {
            DiagnosticSource::Synthetic
        } else {
            DiagnosticSource::RuntimeMetadata
        },
        outcome: DiagnosticOutcome::Success,
        checked_at: Some(stamp(metadata.observed_at_ms)?),
        valid_until: Some(stamp(metadata.observed_at_ms.saturating_add(60_000))?),
        complete: descriptor["catalog"]["completeness"] == "complete",
        values: json!({"descriptor":descriptor,"observations":observation.observations,"controls":metadata.controls}),
        error_code: None,
    })
}
