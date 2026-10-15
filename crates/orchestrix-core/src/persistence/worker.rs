use super::{Command, RunAction, SavedState, StoreError, StoreResult};
use crate::git_records::content_hash;
use crate::{domain::*, worker_records::*};
use rusqlite::{Connection as SqlConnection, params};

pub(super) fn validate(state: &SavedState) -> StoreResult<()> {
    for (id, record) in &state.workers {
        let i = &record.intent;
        if id != &i.id || !record.validate() {
            return Err(StoreError::InvalidState);
        }
        for observation in record
            .observations
            .iter()
            .filter(|o| o.details["recoveryVersion"] == 1)
        {
            if !crate::recovery::valid_local_proof(record, &observation.details) {
                return Err(StoreError::InvalidState);
            }
        }
        let run = state
            .runs
            .get(&RunId::new(&i.run_id)?)
            .ok_or(StoreError::InvalidState)?;
        let task = run.task(&TaskId::new(&i.task_id)?)?;
        let attempt = task
            .attempts()
            .iter()
            .find(|a| a.id().as_str() == i.attempt_id)
            .ok_or(StoreError::InvalidState)?;
        if run.project().map(ProjectId::as_str) != Some(&i.project_id)
            || run.repository().map(RepositoryId::as_str) != Some(&i.repository_id)
            || attempt.inputs().base.as_str() != i.base_oid
        {
            return Err(StoreError::InvalidState);
        }
        let snapshot = state
            .snapshots
            .get(attempt.id())
            .ok_or(StoreError::InvalidState)?;
        let configuration = state
            .records
            .definition(&snapshot.definition.configuration)
            .ok_or(StoreError::InvalidState)?;
        let context = state
            .records
            .definition(&snapshot.definition.context)
            .ok_or(StoreError::InvalidState)?;
        let policy = crate::policy::EffectivePolicySnapshot::from_json(&i.policy)
            .map_err(|_| StoreError::InvalidState)?;
        if configuration.body != i.policy
            || i.context["id"].as_str() != Some(snapshot.definition.context.id.as_str())
            || i.context["version"].as_u64() != Some(snapshot.definition.context.version)
            || context.body["manifest_ref"] != content_hash(i.context.to_string().as_bytes())
            || attempt.inputs().plan_revision.as_str() != policy.revision()
        {
            return Err(StoreError::InvalidState);
        }
        let runtimes = i.policy["runtimeObservations"]
            .as_array()
            .ok_or(StoreError::InvalidState)?;
        for runtime in runtimes {
            let leases = runtime["observations"]
                .as_array()
                .ok_or(StoreError::InvalidState)?;
            for lease in leases {
                let reference = lease["evidence_ref"]
                    .as_str()
                    .ok_or(StoreError::InvalidState)?;
                let diagnostic = state
                    .records
                    .diagnostic(reference)
                    .ok_or(StoreError::InvalidState)?;
                if !snapshot.diagnostics.iter().any(|id| id == reference)
                    || diagnostic.values["descriptor"] != runtime["descriptor"]
                    || diagnostic.values["observations"]
                        .as_array()
                        .is_none_or(|items| !items.contains(lease))
                {
                    return Err(StoreError::InvalidState);
                }
            }
        }
        for observation in record
            .observations
            .iter()
            .filter(|o| o.status == WorkerStatus::DispatchPlanned)
        {
            let reference = observation.details["admissionDiagnostic"]
                .as_str()
                .ok_or(StoreError::InvalidState)?;
            let diagnostic = state
                .records
                .diagnostic(reference)
                .ok_or(StoreError::InvalidState)?;
            if diagnostic.connection_id != attempt.session().connection_id().as_str()
                || diagnostic.connection_revision != attempt.session().connection_revision()
                || !runtimes
                    .iter()
                    .any(|r| r["descriptor"] == diagnostic.values["descriptor"])
            {
                return Err(StoreError::InvalidState);
            }
        }
        match record.status() {
            WorkerStatus::SetAside
                if !crate::recovery::set_aside_evidence(state, record)
                    || record.observations.last().is_none_or(|o| {
                        o.details["authorizationRef"]
                            .as_str()
                            .is_none_or(|s| LocalRef::new(s).is_err())
                            || o.details["resumeAutomatically"] != false
                            || o.details["remoteResultUnchanged"] != true
                    }) =>
            {
                return Err(StoreError::InvalidState);
            }
            WorkerStatus::Cancelled if attempt.state() != AttemptState::Cancelled => {
                return Err(StoreError::InvalidState);
            }
            WorkerStatus::Failed if attempt.state() != AttemptState::Failed => {
                return Err(StoreError::InvalidState);
            }
            WorkerStatus::NoChecks
                if attempt.state() != AttemptState::Succeeded
                    || attempt.artifacts().is_empty()
                    || !i.authorized_checks.as_array().is_some_and(Vec::is_empty) =>
            {
                return Err(StoreError::InvalidState);
            }
            WorkerStatus::NeedsWork
                if attempt.state() != AttemptState::Succeeded
                    || !task.verifications().iter().any(|v| {
                        attempt.artifacts().contains(&v.artifact) && v.result == CheckResult::Failed
                    }) =>
            {
                return Err(StoreError::InvalidState);
            }
            _ => {}
        }
        if record.status() == WorkerStatus::Verified {
            let observation = record.observations.last().ok_or(StoreError::InvalidState)?;
            let revision = observation.details["artifactRevision"]
                .as_str()
                .ok_or(StoreError::InvalidState)?;
            let artifact = attempt
                .artifacts()
                .iter()
                .find(|a| a.revision().as_str() == revision)
                .ok_or(StoreError::InvalidState)?;
            let captured = state
                .captured_artifacts
                .get(&(artifact.id().as_str().into(), revision.into()))
                .ok_or(StoreError::InvalidState)?;
            let checks = observation.details["checks"]
                .as_array()
                .ok_or(StoreError::InvalidState)?;
            let authorized = i
                .authorized_checks
                .as_array()
                .ok_or(StoreError::InvalidState)?;
            if attempt.state() != AttemptState::Succeeded
                || captured.attempt_id != i.attempt_id
                || checks.is_empty()
                || checks.len() != authorized.len()
                || !task
                    .verifications()
                    .iter()
                    .any(|v| &v.artifact == artifact && v.result == CheckResult::Passed)
                || task
                    .verifications()
                    .iter()
                    .any(|v| &v.artifact == artifact && v.result != CheckResult::Passed)
                || checks.iter().zip(authorized).any(|(report, spec)| {
                    report["result"] != "passed"
                        || report["artifact_revision"] != revision
                        || report["command_hash"] != content_hash(spec.to_string().as_bytes())
                        || report["tree_confirmed"] != true
                        || report["streams_closed"] != true
                        || report["exit_code"] != 0
                })
            {
                return Err(StoreError::InvalidState);
            }
        }
    }
    Ok(())
}
pub(super) fn apply(state: &mut SavedState, command: &WorkerCommand) -> StoreResult<()> {
    match command {
        WorkerCommand::Reserve(r) => {
            // Initial production profile is serial per installation/database, including unresolved effects.
            if state
                .workers
                .values()
                .any(|w| w.status().blocks_admission())
            {
                return Err(DomainError::InvalidTransition.into());
            }
            if !r.intent.validate()
                || !command.bounded()
                || state.workers.contains_key(&r.intent.id)
                || r.run.id().as_str() != r.intent.run_id
                || r.attempt.id().as_str() != r.intent.attempt_id
                || r.project.id().as_str() != r.intent.project_id
                || r.project.repository().id().as_str() != r.intent.repository_id
                || r.run.tasks().len() != 1
                || r.run.task(&TaskId::new(&r.intent.task_id)?)?.state() != TaskState::Ready
            {
                return Err(DomainError::Ownership.into());
            }
            let mut staged = state.sanitized()?;
            if let Some(existing) = staged.projects.get(r.project.id()) {
                if existing.checkpoint() != r.project.checkpoint() {
                    return Err(DomainError::Ownership.into());
                }
            } else {
                Command::AddProject(Project::restore_checkpoint(&r.project.checkpoint())?)
                    .apply(&mut staged)?;
            }
            if let Some(existing) = staged
                .connections
                .get(&(r.connection.id().clone(), r.connection.revision()))
            {
                if existing.checkpoint() != r.connection.checkpoint() {
                    return Err(DomainError::Ownership.into());
                }
            } else {
                Command::AddConnection(Connection::restore_checkpoint(&r.connection.checkpoint())?)
                    .apply(&mut staged)?;
            }
            for definition in &r.definitions {
                if let Some(existing) = staged.records.definition(&definition.reference()) {
                    if existing != definition {
                        return Err(DomainError::Ownership.into());
                    }
                } else {
                    Command::Records(crate::persistence_records::RecordsCommand::AddDefinition(
                        definition.clone(),
                    ))
                    .apply(&mut staged)?;
                }
            }
            for diagnostic in &r.diagnostics {
                Command::Records(crate::persistence_records::RecordsCommand::Diagnostic(
                    diagnostic.clone(),
                ))
                .apply(&mut staged)?;
            }
            Command::RegisterSession(RuntimeSessionRef::restore_checkpoint(
                &r.session.checkpoint(),
            )?)
            .apply(&mut staged)?;
            let ready_task = r.run.task(&TaskId::new(&r.intent.task_id)?)?;
            let empty = Run::new(
                r.run.id().clone(),
                Some(&r.project),
                RevisionId::new(&r.intent.base_oid)?,
                DeliveryRequirement::FinalApplication,
            )?;
            Command::create_run(empty).apply(&mut staged)?;
            let task = Task::new(
                ready_task.id().clone(),
                ready_task.objective(),
                ready_task.kind(),
                ready_task.acceptance_criteria().to_vec(),
                vec![],
            )?;
            Command::run(r.run.id().clone(), RunAction::AddTask(task)).apply(&mut staged)?;
            Command::run(
                r.run.id().clone(),
                RunAction::Ready(ready_task.id().clone()),
            )
            .apply(&mut staged)?;
            Command::run(
                r.run.id().clone(),
                RunAction::assign(
                    TaskId::new(&r.intent.task_id)?,
                    TaskAttempt::restore_checkpoint(&r.attempt.checkpoint())?,
                    r.snapshot.clone(),
                ),
            )
            .apply(&mut staged)?;
            staged.workers.insert(
                r.intent.id.clone(),
                WorkerRecord {
                    intent: r.intent.clone(),
                    observations: vec![],
                },
            );
            validate(&staged)?;
            *state = staged;
        }
        WorkerCommand::Observe {
            id,
            status,
            details,
        } => {
            if !exact_safe(details, 65_536) {
                return Err(DomainError::InvalidReference.into());
            }
            let record = state
                .workers
                .get_mut(id)
                .ok_or(DomainError::UnknownEntity)?;
            if record.observations.len() >= 64 || !transition(record.status(), *status) {
                return Err(DomainError::InvalidTransition.into());
            }
            record.observations.push(WorkerObservation {
                status: *status,
                details: details.clone(),
            });
        }
        WorkerCommand::RecordRecovery { id, details } => {
            let record = state
                .workers
                .get_mut(id)
                .ok_or(DomainError::UnknownEntity)?;
            if record.observations.len() >= 64
                || !exact_safe(details, 65_536)
                || !crate::recovery::valid_local_proof(record, details)
            {
                return Err(DomainError::InvalidReference.into());
            }
            record.observations.push(WorkerObservation {
                status: record.status(),
                details: details.clone(),
            });
        }
    }
    Ok(())
}
pub(super) fn load(connection: &SqlConnection, state: &mut SavedState) -> StoreResult<()> {
    let mut q = connection
        .prepare("SELECT id,run_id,task_id,attempt_id,intent FROM manual_workers ORDER BY id")?;
    for row in q.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })? {
        let (id, run, task, attempt, body) = row?;
        if body.len() > 1_048_576 {
            return Err(StoreError::InvalidState);
        }
        let intent: WorkerIntent =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        if intent.id != id
            || intent.run_id != run
            || intent.task_id != task
            || intent.attempt_id != attempt
        {
            return Err(StoreError::InvalidState);
        }
        state.workers.insert(
            id,
            WorkerRecord {
                intent,
                observations: vec![],
            },
        );
    }
    let mut q = connection.prepare(
        "SELECT worker_id,ordinal,body,digest FROM worker_observations ORDER BY worker_id,ordinal",
    )?;
    for row in q.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (id, ordinal, body, digest) = row?;
        if body.len() > 65_536 || content_hash(body.as_bytes()) != digest {
            return Err(StoreError::InvalidState);
        }
        let observation: WorkerObservation =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        let record = state.workers.get_mut(&id).ok_or(StoreError::InvalidState)?;
        if ordinal != record.observations.len() as i64 + 1 {
            return Err(StoreError::InvalidState);
        }
        record.observations.push(observation);
    }
    validate(state)
}
pub(super) fn save(connection: &SqlConnection, state: &SavedState) -> StoreResult<()> {
    for record in state.workers.values() {
        let i = &record.intent;
        let body = serde_json::to_string(i).map_err(|_| StoreError::InvalidState)?;
        connection.execute(
            "INSERT OR IGNORE INTO manual_workers VALUES(?1,?2,?3,?4,?5)",
            params![i.id, i.run_id, i.task_id, i.attempt_id, body],
        )?;
        let saved: String = connection.query_row(
            "SELECT intent FROM manual_workers WHERE id=?1",
            [&i.id],
            |r| r.get(0),
        )?;
        if saved != body {
            return Err(StoreError::InvalidState);
        }
        for (n, o) in record.observations.iter().enumerate() {
            let body = serde_json::to_string(o).map_err(|_| StoreError::InvalidState)?;
            connection.execute(
                "INSERT OR IGNORE INTO worker_observations VALUES(?1,?2,?3,?4)",
                params![i.id, (n + 1) as i64, body, content_hash(body.as_bytes())],
            )?;
            let saved: String = connection.query_row(
                "SELECT body FROM worker_observations WHERE worker_id=?1 AND ordinal=?2",
                params![i.id, (n + 1) as i64],
                |r| r.get(0),
            )?;
            if saved != body {
                return Err(StoreError::InvalidState);
            }
        }
    }
    Ok(())
}
