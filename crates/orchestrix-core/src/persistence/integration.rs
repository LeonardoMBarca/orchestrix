//! Integration intent/observation storage. No filesystem, process or Git effects.
use super::{SavedState, StoreError, StoreResult};
use crate::{
    domain::*,
    evaluator::{CheckReport, EvaluationResult},
    git_records::content_hash,
    integration_records::*,
    recovery_guardian::{GuardianBinding, valid_proof},
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

pub(super) const MIGRATION_V7: &str = r#"
CREATE TABLE serial_integrations(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES runs(id),task_id TEXT NOT NULL,intent TEXT NOT NULL,FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE TABLE integration_observations(integration_id TEXT NOT NULL REFERENCES serial_integrations(id),ordinal INTEGER NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(integration_id,ordinal));
CREATE TRIGGER serial_integrations_no_update BEFORE UPDATE ON serial_integrations BEGIN SELECT RAISE(ABORT,'immutable integration intent'); END;
CREATE TRIGGER serial_integrations_no_delete BEFORE DELETE ON serial_integrations BEGIN SELECT RAISE(ABORT,'retained integration intent'); END;
CREATE TRIGGER integration_observations_no_update BEFORE UPDATE ON integration_observations BEGIN SELECT RAISE(ABORT,'append-only integration observations'); END;
CREATE TRIGGER integration_observations_no_delete BEFORE DELETE ON integration_observations BEGIN SELECT RAISE(ABORT,'retained integration observations'); END;
"#;
fn invalid() -> StoreError {
    StoreError::InvalidState
}
fn artifact(record: &IntegrationRecord) -> StoreResult<ArtifactVersion> {
    ArtifactVersion::restore_checkpoint(&record.intent.artifact).map_err(Into::into)
}
fn candidate(record: &IntegrationRecord) -> StoreResult<DeliveryCandidate> {
    DeliveryCandidate::restore_checkpoint(record.detail("candidate").ok_or_else(invalid)?)
        .map_err(Into::into)
}
fn closure(record: &IntegrationRecord, ordinal: usize, details: &Value) -> StoreResult<()> {
    let launch = record.observations[..=ordinal]
        .iter()
        .rev()
        .find(|o| o.details.get("guardian").is_some())
        .ok_or_else(invalid)?;
    if launch.details["effects"] != "git-and-checks-owned-v1" {
        return Err(invalid());
    }
    let binding = &launch.details["guardian"];
    let binding: GuardianBinding =
        serde_json::from_value(binding.clone()).map_err(|_| invalid())?;
    if !valid_proof(&binding, &details["localClosureProof"]) {
        return Err(invalid());
    }
    Ok(())
}
fn checked(record: &IntegrationRecord, details: &Value) -> StoreResult<()> {
    let snapshot = record.snapshot().ok_or_else(invalid)?;
    if !snapshot.validate() || details["snapshotRevision"] != snapshot.revision {
        return Err(invalid());
    }
    let reports: Vec<CheckReport> =
        serde_json::from_value(details["checks"].clone()).map_err(|_| invalid())?;
    if reports.len() != record.intent.checks.len() {
        return Err(invalid());
    }
    for (spec, report) in record.intent.checks.iter().zip(reports) {
        if report.id != spec.id
            || report.artifact_revision != snapshot.revision
            || report.cwd != record.intent.worktree.path
            || report.command_hash != content_hash(json!(spec).to_string().as_bytes())
            || report.authorization_ref != spec.authorization_ref
            || report.timeout_ms != spec.timeout_ms
            || spec
                .executable_sha256
                .as_ref()
                .is_some_and(|hash| hash != &report.executable_hash)
            || report.result != EvaluationResult::Passed
            || report.output_truncated
            || report.exit_code != Some(0)
            || !report.tree_confirmed
            || !report.streams_closed
        {
            return Err(invalid());
        }
    }
    Ok(())
}
pub(super) fn validate(state: &SavedState) -> StoreResult<()> {
    for (id, record) in &state.integrations {
        let i = &record.intent;
        if id != &i.id || !record.validate() {
            return Err(invalid());
        }
        let worker = state.workers.get(&i.worker_id).ok_or_else(invalid)?;
        let run = state
            .runs
            .get(&RunId::new(&i.run_id)?)
            .ok_or_else(invalid)?;
        let task = run.task(&TaskId::new(&i.task_id)?)?;
        let exact = artifact(record)?;
        let source = state
            .captured_artifacts
            .get(&(i.capture_id.clone(), i.capture_revision.clone()))
            .ok_or_else(invalid)?;
        let original_worktree = state
            .worktrees
            .get(&source.operation_id)
            .ok_or_else(invalid)?;
        if run.tasks().len() != 1
            || task.kind() != TaskKind::Code
            || worker.intent.run_id != i.run_id
            || worker.intent.task_id != i.task_id
            || exact.id().as_str() != i.capture_id
            || exact.revision().as_str() != i.capture_revision
            || source.base_oid != i.input_base
            || source.preservation != i.worktree.preservation
            || source.attempt_id != worker.intent.attempt_id
            || i.worktree.project_id != worker.intent.project_id
            || i.worktree.repository_id != worker.intent.repository_id
            || i.worktree.attempt_id != worker.intent.attempt_id
            || i.target.directory != original_worktree.intent.main_repository
            || i.target.common_git_dir != original_worktree.intent.common_git_dir
            || source.files.iter().any(|f| f.base != f.index)
            || !source.exclusions.is_empty()
            || !task
                .attempts()
                .iter()
                .any(|a| a.artifacts().contains(&exact))
            || !task
                .reviews()
                .iter()
                .any(|r| r.artifact == exact && r.result == ReviewResult::Accepted)
            || !task
                .verifications()
                .iter()
                .any(|v| v.artifact == exact && v.result == CheckResult::Passed)
        {
            return Err(invalid());
        }
        if let Some(snapshot) = record.snapshot()
            && (!snapshot.validate()
                || snapshot.operation_id != i.worktree.operation_id
                || snapshot.base_oid != i.input_base
                || snapshot.preservation != i.worktree.preservation
                || snapshot.run_id != i.run_id
                || snapshot.task_id != i.task_id
                || snapshot.attempt_id != i.worktree.attempt_id
                || snapshot.repository_id != i.worktree.repository_id
                || snapshot.origin != i.worktree.path
                || !snapshot.exclusions.is_empty()
                || snapshot
                    .files
                    .iter()
                    .map(|f| (&f.path, &f.base, &f.index, &f.worktree))
                    .collect::<Vec<_>>()
                    != source
                        .files
                        .iter()
                        .map(|f| (&f.path, &f.base, &f.index, &f.worktree))
                        .collect::<Vec<_>>())
        {
            return Err(invalid());
        }
        let count = |status| {
            record
                .observations
                .iter()
                .filter(|o| o.status == status)
                .count()
        };
        let domain_integrations = task
            .integrations()
            .iter()
            .filter(|e| e.reference.as_str() == id)
            .count();
        let domain_application = usize::from(
            run.application()
                .is_some_and(|e| e.reference.as_str() == id),
        );
        let domain_delivered =
            usize::from(domain_application == 1 && run.state() == RunState::Delivered);
        if count(IntegrationStatus::Integrated) != domain_integrations
            || count(IntegrationStatus::Applied) != domain_application
            || count(IntegrationStatus::Delivered) != domain_delivered
        {
            return Err(invalid());
        }
        for (n, o) in record.observations.iter().enumerate() {
            match o.status {
                IntegrationStatus::Checked => checked(record, &o.details)?,
                IntegrationStatus::Integrated => {
                    closure(record, n, &o.details)?;
                    let result = o.details["resultRevision"]
                        .as_str()
                        .filter(|v| oid(v))
                        .ok_or_else(invalid)?;
                    if !task.integrations().iter().any(|e| {
                        e.reference.as_str() == id
                            && e.artifact == exact
                            && e.input_base.as_str() == i.input_base
                            && e.result_revision.as_str() == result
                            && e.validation == CheckResult::Passed
                    }) {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Prepared => {
                    let c = DeliveryCandidate::restore_checkpoint(&o.details["candidate"])?;
                    if c.run().as_str() != i.run_id
                        || c.result().as_str()
                            != record
                                .detail("resultRevision")
                                .and_then(Value::as_str)
                                .ok_or_else(invalid)?
                        || c.target_base().as_str() != i.target.head
                        || c.artifacts() != [exact.clone()]
                    {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Approved => {
                    let a = &o.details["approval"];
                    let request: ApprovalRequest =
                        serde_json::from_value(a["request"].clone()).map_err(|_| invalid())?;
                    let issued = a["issuedMs"].as_u64().ok_or_else(invalid)?;
                    let expires = a["expiresMs"].as_u64().ok_or_else(invalid)?;
                    if request.schema_version != 1
                        || request.integration_id != i.id
                        || request.candidate_revision
                            != record.candidate_revision().ok_or_else(invalid)?
                        || !(1_000..=86_400_000).contains(&request.valid_for_ms)
                        || issued < i.created_ms
                        || issued.checked_add(request.valid_for_ms) != Some(expires)
                        || a["target"] != json!(i.target)
                        || a["requestHash"] != content_hash(json!(request).to_string().as_bytes())
                        || LocalRef::new(&request.id).is_err()
                        || LocalRef::new(&request.authorization_ref).is_err()
                    {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Applying => {
                    let request: ApplicationRequest =
                        serde_json::from_value(o.details["applicationRequest"].clone())
                            .map_err(|_| invalid())?;
                    let approval = record.observations[..n]
                        .iter()
                        .rev()
                        .find_map(|o| o.details.get("approval"))
                        .ok_or_else(invalid)?;
                    let started = o.details["startedMs"].as_u64().ok_or_else(invalid)?;
                    if request.schema_version != 1
                        || request.integration_id != i.id
                        || request.candidate_revision
                            != record.candidate_revision().ok_or_else(invalid)?
                        || o.details["applicationRequestHash"]
                            != content_hash(json!(request).to_string().as_bytes())
                        || started < approval["issuedMs"].as_u64().ok_or_else(invalid)?
                        || started >= approval["expiresMs"].as_u64().ok_or_else(invalid)?
                        || LocalRef::new(&request.id).is_err()
                        || LocalRef::new(&request.authorization_ref).is_err()
                    {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Applied => {
                    closure(record, n, &o.details)?;
                    let actual: TargetState =
                        serde_json::from_value(o.details["actualTarget"].clone())
                            .map_err(|_| invalid())?;
                    if !actual.validate()
                        || actual.directory != i.target.directory
                        || actual.common_git_dir != i.target.common_git_dir
                        || actual.symbolic_ref != i.target.symbolic_ref
                        || Some(actual.head.as_str())
                            != record.detail("resultRevision").and_then(Value::as_str)
                        || run.application().is_none_or(|e| {
                            e.reference.as_str() != id
                                || e.applied_revision.as_str() != actual.head
                                || e.validation != CheckResult::Passed
                        })
                    {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Delivered if run.state() != RunState::Delivered => {
                    return Err(invalid());
                }
                _ => {}
            }
        }
    }
    // A failed earlier preparation for the same Run remains historical. Bind
    // the current domain approval to exactly one current candidate instead of
    // requiring every historical preparation to own that approval.
    for run_id in state
        .integrations
        .values()
        .map(|r| r.intent.run_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let run = state.runs.get(&RunId::new(run_id)?).ok_or_else(invalid)?;
        if run.state() == RunState::ReadyToApply
            && let Some(approval) = run.approval()
        {
            let current = run.candidate().ok_or_else(invalid)?.checkpoint();
            let matches = state
                .integrations
                .values()
                .filter(|r| r.intent.run_id == run_id)
                .filter(|r| r.detail("candidate") == Some(&current))
                .flat_map(|r| &r.observations)
                .filter(|o| {
                    o.status == IntegrationStatus::Approved
                        && o.details["approval"]["request"]["id"] == approval.as_str()
                })
                .count();
            if matches != 1 {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
pub(super) fn apply(state: &mut SavedState, command: &IntegrationCommand) -> StoreResult<()> {
    match command {
        IntegrationCommand::Reserve(record) => {
            if !record.validate()
                || !record.observations.is_empty()
                || state.integrations.contains_key(&record.intent.id)
                || state
                    .integrations
                    .values()
                    .any(|r| r.status().blocks_admission())
                || state
                    .workers
                    .values()
                    .any(|w| w.status().blocks_admission())
                || state
                    .reviews
                    .values()
                    .any(|r| r.status().blocks_admission())
            {
                return Err(invalid());
            }
            let run = state
                .runs
                .get(&RunId::new(&record.intent.run_id)?)
                .ok_or_else(invalid)?;
            let task = run.task(&TaskId::new(&record.intent.task_id)?)?;
            if task.state() != TaskState::Approved
                || task.artifact() != Some(&artifact(record)?)
                || run.result_revision().as_str() != record.intent.input_base
            {
                return Err(invalid());
            }
            state
                .integrations
                .insert(record.intent.id.clone(), (**record).clone());
        }
        IntegrationCommand::Observe {
            id,
            status,
            details,
        } => {
            let mut record = state.integrations.get(id).ok_or_else(invalid)?.clone();
            if !transition(record.status(), *status) {
                return Err(invalid());
            }
            let run_id = RunId::new(&record.intent.run_id)?;
            let result = record
                .detail("resultRevision")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            match status {
                IntegrationStatus::Integrated => {
                    let result = details["resultRevision"]
                        .as_str()
                        .filter(|v| oid(v))
                        .ok_or_else(invalid)?;
                    let exact = artifact(&record)?;
                    state.runs.get_mut(&run_id).ok_or_else(invalid)?.integrate(
                        IntegrationEvidence {
                            run: run_id.clone(),
                            task: TaskId::new(&record.intent.task_id)?,
                            artifact: exact,
                            input_base: RevisionId::new(&record.intent.input_base)?,
                            result_revision: RevisionId::new(result)?,
                            reference: EvidenceId::new(id)?,
                            validation: CheckResult::Passed,
                        },
                    )?;
                }
                IntegrationStatus::Prepared => {
                    let actual = state
                        .runs
                        .get_mut(&run_id)
                        .ok_or_else(invalid)?
                        .prepare_delivery(RevisionId::new(&record.intent.target.head)?)?;
                    if actual.checkpoint() != details["candidate"] {
                        return Err(invalid());
                    }
                }
                IntegrationStatus::Approved => {
                    let candidate = candidate(&record)?;
                    state
                        .runs
                        .get_mut(&run_id)
                        .ok_or_else(invalid)?
                        .approve_delivery(
                            &candidate,
                            &RevisionId::new(&record.intent.target.head)?,
                            EvidenceId::new(
                                details["approval"]["request"]["id"]
                                    .as_str()
                                    .ok_or_else(invalid)?,
                            )?,
                        )?;
                }
                IntegrationStatus::Applied => {
                    let candidate = candidate(&record)?;
                    state
                        .runs
                        .get_mut(&run_id)
                        .ok_or_else(invalid)?
                        .confirm_application(
                            &candidate,
                            ApplicationEvidence {
                                run: run_id.clone(),
                                repository: candidate.repository().clone(),
                                result: candidate.result().clone(),
                                target_base: candidate.target_base().clone(),
                                applied_revision: RevisionId::new(result.ok_or_else(invalid)?)?,
                                reference: EvidenceId::new(id)?,
                                validation: CheckResult::Passed,
                            },
                        )?;
                }
                IntegrationStatus::Delivered => state
                    .runs
                    .get_mut(&run_id)
                    .ok_or_else(invalid)?
                    .mark_delivered()?,
                IntegrationStatus::Invalidated | IntegrationStatus::Blocked => {
                    let run = state.runs.get_mut(&run_id).ok_or_else(invalid)?;
                    if run.state() == RunState::ReadyToApply {
                        let candidate = run.candidate().ok_or_else(invalid)?.clone();
                        run.report_application_conflict(
                            &candidate,
                            candidate.target_base(),
                            EvidenceId::new(format!(
                                "integration-invalid-{}",
                                &content_hash(id.as_bytes())[..24]
                            ))?,
                        )?;
                    }
                }
                _ => {}
            }
            record.observations.push(IntegrationObservation {
                status: *status,
                details: details.clone(),
            });
            state.integrations.insert(id.clone(), record);
        }
    }
    validate(state)
}
pub(super) fn load(connection: &Connection, state: &mut SavedState) -> StoreResult<()> {
    let mut query = connection
        .prepare("SELECT id,run_id,task_id,intent FROM serial_integrations ORDER BY id")?;
    for row in query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (id, run, task, body) = row?;
        if body.len() > 1_048_576 {
            return Err(invalid());
        }
        let intent: IntegrationIntent = serde_json::from_str(&body).map_err(|_| invalid())?;
        if intent.id != id || intent.run_id != run || intent.task_id != task {
            return Err(invalid());
        }
        state.integrations.insert(
            id,
            IntegrationRecord {
                intent,
                observations: vec![],
            },
        );
    }
    let mut query=connection.prepare("SELECT integration_id,ordinal,body,digest FROM integration_observations ORDER BY integration_id,ordinal")?;
    for row in query.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (id, ordinal, body, hash) = row?;
        if body.len() > 4_194_304 || content_hash(body.as_bytes()) != hash {
            return Err(invalid());
        }
        let record = state.integrations.get_mut(&id).ok_or_else(invalid)?;
        if usize::try_from(ordinal).ok() != record.observations.len().checked_add(1) {
            return Err(invalid());
        }
        record
            .observations
            .push(serde_json::from_str(&body).map_err(|_| invalid())?);
    }
    Ok(())
}
pub(super) fn save(connection: &Connection, state: &SavedState) -> StoreResult<()> {
    for (id, record) in &state.integrations {
        let body = json!(record.intent).to_string();
        connection.execute(
            "INSERT OR IGNORE INTO serial_integrations VALUES(?1,?2,?3,?4)",
            params![id, record.intent.run_id, record.intent.task_id, body],
        )?;
        let stored: String = connection.query_row(
            "SELECT intent FROM serial_integrations WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        if stored != body {
            return Err(invalid());
        }
        for (n, observation) in record.observations.iter().enumerate() {
            let ordinal =
                i64::try_from(n.checked_add(1).ok_or_else(invalid)?).map_err(|_| invalid())?;
            let body = json!(observation).to_string();
            connection.execute(
                "INSERT OR IGNORE INTO integration_observations VALUES(?1,?2,?3,?4)",
                params![id, ordinal, body, content_hash(body.as_bytes())],
            )?;
            let stored: String = connection.query_row(
                "SELECT body FROM integration_observations WHERE integration_id=?1 AND ordinal=?2",
                params![id, ordinal],
                |r| r.get(0),
            )?;
            if stored != body {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
