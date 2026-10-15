//! Deterministic reconciliation of durable facts. No runtime is launched, no PID
//! is adopted/killed, and no provider observation becomes fresh by being loaded.
use crate::{
    domain::{LocalRef, RunId, TaskId},
    git::WorktreeService,
    git_records::content_hash,
    persistence::{Command, Disposition, Request, SavedState, Store},
    policy::{
        self, AuthorizedContextFile, ContextBudget, ContextInput, ContextPack, ContextSource,
        ContextSourceIdentity, ContextSourceKind,
    },
    worker::{self, ManualWorker, WorkerError},
    worker_records::{WorkerCommand, WorkerRecord, WorkerStatus},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, WorkerError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchKnowledge {
    NeverSent,
    IntentOnly,
    Confirmed,
    OutcomeKnown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCase {
    pub worker_id: String,
    pub status: WorkerStatus,
    pub dispatch: DispatchKnowledge,
    pub remote_outcome: String,
    /// Only original supervisor observations establish local termination.
    pub local_tree_stopped: bool,
    pub streams_closed: bool,
    pub local_scope_exhausted: bool,
    pub worktrees: Vec<Value>,
    pub artifacts: Vec<Value>,
    pub inspection_required: bool,
    pub set_aside_eligible: bool,
    pub next_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecoverySummary {
    pub schema_version: u32,
    pub revision: u64,
    pub ready_for_admission: bool,
    pub has_blocking_cases: bool,
    pub cases: Vec<RecoveryCase>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryAction {
    /// Record that a human inspected the incident, without releasing admission.
    RecordInspection,
    /// Release only this local admission barrier. Never retry its old task,
    /// rewrite its native result or authorize a new provider session implicitly.
    SetAside,
}

/// Run before new admissions, under the same short host lease used by workers.
/// Daemon instance ownership must use a separate lease. An unavailable worker
/// lease is a live owner, not evidence of a crash.
pub fn reconcile(store: &mut Store) -> Result<RecoverySummary> {
    let _lease = worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
    let ids = store.load()?.workers().keys().cloned().collect::<Vec<_>>();
    for id in &ids {
        reconcile_one(store, id)?;
    }
    // Keep the lease across artifact validation; otherwise another worker host
    // could enter between reconciliation and the final admission decision.
    for id in &ids {
        if store.load()?.workers()[id].status() == WorkerStatus::Verified {
            ManualWorker::inspect_owned(store, id)?;
        }
    }
    let mut summary = snapshot(store)?;
    let service = summary
        .cases
        .iter()
        .any(|c| !c.worktrees.is_empty())
        .then(WorktreeService::new)
        .transpose()?;
    for entry in &mut summary.cases {
        for worktree in &mut entry.worktrees {
            let operation = worktree["operationId"]
                .as_str()
                .ok_or(WorkerError("worktree"))?;
            worktree["inspection"] = json!(if service
                .as_ref()
                .is_some_and(|s| s.inspect(store, operation).is_ok())
            {
                "confirmed-owned"
            } else {
                "inspection-required"
            });
        }
    }
    Ok(summary)
}

/// Read-only recovery diagnostics. It does not establish process liveness or
/// current authentication/capacity, and is safe while a worker is active.
pub fn snapshot(store: &Store) -> Result<RecoverySummary> {
    let state = store.load()?;
    let cases = state
        .workers()
        .values()
        .map(|w| case(&state, w))
        .collect::<Result<Vec<_>>>()?;
    Ok(RecoverySummary {
        schema_version: 1,
        revision: state.revision(),
        ready_for_admission: !state
            .workers()
            .values()
            .any(|w| w.status().blocks_admission()),
        has_blocking_cases: state
            .workers()
            .values()
            .any(|w| w.status().blocks_admission()),
        cases,
    })
}

pub(crate) fn reconcile_one(store: &mut Store, id: &str) -> Result<()> {
    refresh_owned(store, id)?;
    let state = store.load()?;
    let record = state
        .workers()
        .get(id)
        .cloned()
        .ok_or(WorkerError("unknown-worker"))?;
    if record.status().terminal() {
        return Ok(());
    }
    let before = case(&state, &record)?;
    // Only durable dispatch intent precedes any turn. A runtime process can
    // have been created during Preparing; that fact is not a sent turn.
    let status = if before.dispatch == DispatchKnowledge::NeverSent {
        WorkerStatus::Blocked
    } else {
        worker::transport_lost(store, &record, "host-reopened-without-terminal")?;
        WorkerStatus::Uncertain
    };
    worker::observe(
        store,
        id,
        status,
        json!({
            "reason":"host-reopened-without-terminal", "inspectionRequired":true,
            "recovery": {"dispatch":before.dispatch,"remoteOutcome":before.remote_outcome,
                "localTreeStopped":before.local_tree_stopped,"streamsClosed":before.streams_closed},
            "nextAction":if before.set_aside_eligible {"inspect-and-explicitly-set-aside"} else {"preserve-block-owned-process-reconciliation-unavailable-in-this-profile"},
            "resumeAutomatically":false,
        }),
    )
}

/// A stale intervention is rejected; persisted command identity is derived from
/// the exact revision/action/authorization. No user-provided PID or termination
/// boolean is accepted as evidence.
pub fn intervene(
    store: &mut Store,
    id: &str,
    expected_revision: u64,
    authorization_ref: &str,
    action: RecoveryAction,
) -> Result<RecoveryCase> {
    LocalRef::new(authorization_ref).map_err(|_| WorkerError("intervention-authorization"))?;
    let _lease = worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
    let state = store.load()?;
    if state.revision() != expected_revision {
        return Err(WorkerError("intervention-stale-revision"));
    }
    let record = state
        .workers()
        .get(id)
        .ok_or(WorkerError("unknown-worker"))?;
    if !matches!(
        record.status(),
        WorkerStatus::Blocked | WorkerStatus::Uncertain | WorkerStatus::InspectionRequired
    ) {
        return Err(WorkerError("intervention-not-required"));
    }
    if action == RecoveryAction::SetAside && !set_aside_evidence(&state, record) {
        return Err(WorkerError("owned-stop-evidence-or-never-sent-required"));
    }
    if action == RecoveryAction::RecordInspection
        && record.status() == WorkerStatus::InspectionRequired
    {
        return case(&state, record);
    }
    let status = if action == RecoveryAction::SetAside {
        WorkerStatus::SetAside
    } else {
        WorkerStatus::InspectionRequired
    };
    let receipt = store.execute(&Request {
        scope: LocalRef::new(id)?,
        key: LocalRef::new(format!("intervention-{expected_revision}"))?,
        expected_revision,
        command: Command::Worker(WorkerCommand::Observe { id:id.into(), status, details:json!({
            "reason":if action == RecoveryAction::SetAside {"explicitly-set-aside-without-retry"} else {"inspection-recorded-admission-remains-blocked"},
            "authorizationRef":authorization_ref,"inspectionRequired":action != RecoveryAction::SetAside,
            "resumeAutomatically":false,"remoteResultUnchanged":true,
            "nextAction":if action == RecoveryAction::SetAside {"submit-only-a-new-explicitly-authorized-task"} else {"preserve-block-owned-process-reconciliation-unavailable-in-this-profile"},
        })}),
    })?;
    if receipt.result.disposition != Disposition::Applied {
        return Err(WorkerError("intervention-rejected"));
    }
    let state = store.load()?;
    case(&state, &state.workers()[id])
}

/// Reused by persistence validation, so a caller cannot bypass the recovery API
/// by appending SetAside directly. No absence-of-PID inference is involved.
pub(crate) fn set_aside_evidence(state: &SavedState, record: &WorkerRecord) -> bool {
    knowledge(state, record).is_ok_and(|(dispatch, _, tree, streams)| {
        // Preparing is written before connect can spawn anything. Even though
        // no task turn was sent, a lost process handle is not stopped evidence.
        // Only the earlier reservation/worktree-only window is launch-free.
        (dispatch == DispatchKnowledge::NeverSent
            && !record.observations.iter().any(|o| {
                matches!(
                    o.status,
                    WorkerStatus::SupervisorReady
                        | WorkerStatus::DispatchPlanned
                        | WorkerStatus::RuntimeObserved
                        | WorkerStatus::Checking
                )
            })
            && record
                .observations
                .iter()
                .filter(|o| o.status == WorkerStatus::Preparing)
                .all(|o| {
                    o.details["runtimeSpawnRequiresSupervisorReady"] == true
                        && serde_json::from_value::<crate::recovery_guardian::GuardianBinding>(
                            o.details["guardian"].clone(),
                        )
                        .is_ok_and(|b| b.validate())
                }))
            || (tree
                && streams
                && !record
                    .observations
                    .iter()
                    .any(|o| o.status == WorkerStatus::Checking))
            || record
                .observations
                .iter()
                .any(|o| valid_local_proof(record, &o.details))
    })
}

fn knowledge(
    state: &SavedState,
    record: &WorkerRecord,
) -> Result<(DispatchKnowledge, String, bool, bool)> {
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id)?)
        .ok_or(WorkerError("run"))?;
    let task = run.task(&TaskId::new(&record.intent.task_id)?)?;
    let attempt = task
        .attempts()
        .iter()
        .find(|a| a.id().as_str() == record.intent.attempt_id)
        .ok_or(WorkerError("attempt"))?;
    let runtime = attempt.runtime_snapshot();
    let outcome = runtime["outcome"].as_str().unwrap_or("unknown").to_owned();
    let dispatch = if runtime["terminalConfirmed"] == true && outcome != "unknown" {
        DispatchKnowledge::OutcomeKnown
    } else if runtime["admissionObserved"] == true || runtime["startObserved"] == true {
        DispatchKnowledge::Confirmed
    } else if record
        .observations
        .iter()
        .any(|o| o.status == WorkerStatus::DispatchPlanned)
    {
        DispatchKnowledge::IntentOnly
    } else {
        DispatchKnowledge::NeverSent
    };
    let process = &runtime["process"];
    let tree = process["tree"]["ownership"] == "confirmed"
        && process["tree"]["source"] == "owned-job"
        && process["tree"]["termination"] == "confirmed"
        && process["tree"]["activeCount"] == 0
        && process["runtime"]["exited"] == true;
    let streams = process["runtime"]["stdioClosed"] == true;
    Ok((dispatch, outcome, tree, streams))
}

fn case(state: &SavedState, record: &WorkerRecord) -> Result<RecoveryCase> {
    let (dispatch, remote_outcome, local_tree_stopped, streams_closed) = knowledge(state, record)?;
    let worktrees = state
        .worktrees()
        .values()
        .filter(|w| w.intent.attempt_id == record.intent.attempt_id)
        .map(|w| {
            json!({"operationId":w.intent.operation_id,"state":w.state,
            "inspection":"historical-not-revalidated"})
        })
        .collect();
    let artifacts = state
        .captured_artifacts()
        .values()
        .filter(|a| a.attempt_id == record.intent.attempt_id)
        .map(|a| json!({"id":a.id,"revision":a.revision,"base":a.base_oid}))
        .collect();
    let inspection_required = record.status().blocks_admission();
    let local_scope_exhausted = record
        .observations
        .iter()
        .any(|o| valid_local_proof(record, &o.details));
    let set_aside_eligible = inspection_required && set_aside_evidence(state, record);
    Ok(RecoveryCase {
        worker_id: record.intent.id.clone(),
        status: record.status(),
        dispatch,
        remote_outcome,
        local_tree_stopped,
        streams_closed,
        local_scope_exhausted,
        worktrees,
        artifacts,
        inspection_required,
        set_aside_eligible,
        next_actions: if !inspection_required {
            vec![]
        } else if set_aside_eligible {
            vec!["inspect-preserved-work-and-authorize-set-aside".into()]
        } else {
            vec![
                "record-inspection-without-unblocking".into(),
                "retain-uncertainty-no-automatic-retry".into(),
                if guardian_binding(record).is_some() {
                    "recheck-local-keeper-receipt".into()
                } else {
                    "owned-process-reconciliation-unavailable-for-legacy-attempt".into()
                },
            ]
        },
    })
}

fn guardian_binding(record: &WorkerRecord) -> Option<crate::recovery_guardian::GuardianBinding> {
    record
        .observations
        .iter()
        .find(|o| o.status == WorkerStatus::Preparing)
        .and_then(|o| serde_json::from_value(o.details["guardian"].clone()).ok())
}
pub(crate) fn valid_local_proof(record: &WorkerRecord, details: &Value) -> bool {
    details["recoveryVersion"] == 1
        && guardian_binding(record)
            .is_some_and(|b| crate::recovery_guardian::valid_proof(&b, &details["localScopeProof"]))
}
fn refresh_owned(store: &mut Store, id: &str) -> Result<()> {
    let state = store.load()?;
    let record = state
        .workers()
        .get(id)
        .ok_or(WorkerError("unknown-worker"))?;
    if record
        .observations
        .iter()
        .any(|o| valid_local_proof(record, &o.details))
    {
        return Ok(());
    }
    let Some(binding) = guardian_binding(record) else {
        return Ok(());
    };
    if binding.directory.parent() != store.path().parent() {
        return Err(WorkerError("guardian-scope-directory"));
    }
    let Ok(proof) = crate::recovery_guardian::read_proof(&binding) else {
        return Ok(());
    };
    let mut details = record
        .observations
        .last()
        .map_or_else(|| json!({}), |o| o.details.clone());
    details["recoveryVersion"] = json!(1);
    details["localScopeProof"] = proof.clone();
    let key = format!(
        "scope-proof-{}",
        &content_hash(proof.to_string().as_bytes())[..24]
    );
    let receipt = store.execute(&Request {
        scope: LocalRef::new(id)?,
        key: LocalRef::new(key)?,
        expected_revision: state.revision(),
        command: Command::Worker(WorkerCommand::RecordRecovery {
            id: id.into(),
            details,
        }),
    })?;
    if receipt.result.disposition != Disposition::Applied {
        return Err(WorkerError("recovery-proof-rejected"));
    }
    Ok(())
}
/// Explicit local recheck only: bounded private-file validation, short commit,
/// no Git, provider query, process adoption, cancellation or new dispatch.
pub fn refresh_closure_evidence(store: &mut Store, id: &str) -> Result<RecoveryCase> {
    let _lease = worker::lease(store)?.ok_or(WorkerError("worker-host-busy"))?;
    refresh_owned(store, id)?;
    let state = store.load()?;
    case(
        &state,
        state
            .workers()
            .get(id)
            .ok_or(WorkerError("unknown-worker"))?,
    )
}

/// Deterministic continuity from durable task/policy/context/evidence only.
/// Original file bytes retain their old revisions; they are never described as
/// a fresh filesystem scan or as the runtime's conversation/hidden reasoning.
pub fn compile_continuity(store: &Store, id: &str, budget: ContextBudget) -> Result<ContextPack> {
    let state = store.load()?;
    let record = state
        .workers()
        .get(id)
        .ok_or(WorkerError("unknown-worker"))?;
    let run = state
        .runs()
        .get(&RunId::new(&record.intent.run_id)?)
        .ok_or(WorkerError("run"))?;
    let task = run.task(&TaskId::new(&record.intent.task_id)?)?;
    let context = &record.intent.context;
    let attempt = task
        .attempts()
        .iter()
        .find(|a| a.id().as_str() == record.intent.attempt_id)
        .ok_or(WorkerError("attempt"))?;
    let mut sources = Vec::new();
    for source in context["sources"]
        .as_array()
        .ok_or(WorkerError("context"))?
    {
        sources.push(ContextSource {
            identity: serde_json::from_value(source["identity"].clone())
                .map_err(|_| WorkerError("context-source"))?,
            kind: serde_json::from_value(source["kind"].clone())
                .map_err(|_| WorkerError("context-source"))?,
            authorization_ref: source["authorizationRef"].as_str().map(str::to_owned),
            text: source["content"]
                .as_str()
                .ok_or(WorkerError("context-source"))?
                .into(),
            // This package supports inspection, not dispatch. Basic state must
            // fit even if optional historical material consumes the budget;
            // every omission is explicit in the compiler's manifest.
            required: false,
            priority: 1,
        });
    }
    let facts = json!({"storeRevision":state.revision(),
        "worker":{"workerId":record.intent.id,"runId":record.intent.run_id,"taskId":record.intent.task_id,"attemptId":record.intent.attempt_id,"status":record.status(),"resumeAutomatically":false,
            "reason":record.observations.last().map(|o|o.details["reason"].clone())},
        "planRevision":attempt.inputs().plan_revision.as_str(),"policyRevision":content_hash(record.intent.policy.to_string().as_bytes()),
        "dependencies":task.dependencies().iter().map(|d|json!({"task":d.task.as_str(),"result":format!("{:?}",d.result)})).collect::<Vec<_>>(),
        "artifacts":state.captured_artifacts().values().filter(|a| a.attempt_id == record.intent.attempt_id).map(|a|json!({"id":a.id,"revision":a.revision,"base":a.base_oid})).collect::<Vec<_>>(),
        "checks":task.verifications().iter().map(|v|json!({"artifact":v.artifact.revision().as_str(),"evidence":v.reference.as_str(),"result":format!("{:?}",v.result)})).collect::<Vec<_>>(),
        "reviews":task.reviews().iter().map(|v|json!({"artifact":v.artifact.revision().as_str(),"evidence":v.reference.as_str(),"result":format!("{:?}",v.result)})).collect::<Vec<_>>(),
        "approval":run.approval().map(|a|a.as_str()),
        "deliveryCandidate":run.candidate().map(|c|json!({"run":c.run().as_str(),"repository":c.repository().as_str(),"result":c.result().as_str(),"targetBase":c.target_base().as_str(),"artifacts":c.artifacts().iter().map(|a|json!({"id":a.id().as_str(),"revision":a.revision().as_str(),"base":a.base().as_str()})).collect::<Vec<_>>()})),
        "originalContextRevision":content_hash(context.to_string().as_bytes()),
        "knownOmissions":context["omissions"],"unobserved":"runtime transcript and hidden reasoning are unavailable; provider authentication/capacity require fresh authorized validation"});
    sources.push(ContextSource {
        identity: ContextSourceIdentity {
            reference: "durable-continuity-checkpoint".into(),
            revision: content_hash(facts.to_string().as_bytes()),
        },
        kind: ContextSourceKind::DurableCheckpoint,
        authorization_ref: Some("durable-local-state".into()),
        text: facts.to_string(),
        required: true,
        priority: 0,
    });
    let files = context["files"]
        .as_array()
        .ok_or(WorkerError("context-files"))?
        .iter()
        .map(|file| {
            Ok(AuthorizedContextFile {
                path: file["path"]
                    .as_str()
                    .ok_or(WorkerError("context-file"))?
                    .into(),
                source: serde_json::from_value(file["source"].clone())
                    .map_err(|_| WorkerError("context-file"))?,
                authorization_ref: file["authorizationRef"].as_str().map(str::to_owned),
                content: file["content"]
                    .as_str()
                    .ok_or(WorkerError("context-file"))?
                    .into(),
                required: false,
                priority: 0,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut constraints = context["constraints"]
        .as_array()
        .ok_or(WorkerError("context"))?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or(WorkerError("context")))
        .collect::<Result<Vec<_>>>()?;
    constraints.push("Continuity package is historical local state; do not infer a resumed native session, current authentication/capacity, hidden reasoning or authorization to retry.".into());
    constraints.push("Omitted historical material is unavailable; inspection of this package does not authorize dispatch or replacement of original permissions.".into());
    Ok(policy::compile_context(
        &ContextInput {
            schema_version: 1,
            id: format!(
                "continuity-{}",
                &content_hash(record.intent.id.as_bytes())[..24]
            ),
            version: state.revision().max(1),
            objective: task.objective().into(),
            acceptance_criteria: task.acceptance_criteria().to_vec(),
            constraints,
            sources,
            files,
        },
        budget,
    )?)
}
