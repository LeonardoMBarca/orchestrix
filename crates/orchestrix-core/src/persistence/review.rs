//! Short durable review commands. No runtime, Git or filesystem effects occur here.
use super::{Command, RunAction, SavedState, StoreError, StoreResult};
use crate::{domain::*, git_records::content_hash, review_records::*};
use rusqlite::{Connection, params};

pub(super) const MIGRATION_V6: &str = r#"
CREATE TABLE independent_reviews(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES runs(id),task_id TEXT NOT NULL,intent TEXT NOT NULL,FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE TABLE review_observations(review_id TEXT NOT NULL REFERENCES independent_reviews(id),ordinal INTEGER NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(review_id,ordinal));
CREATE TRIGGER independent_reviews_no_update BEFORE UPDATE ON independent_reviews BEGIN SELECT RAISE(ABORT,'immutable review intent'); END;
CREATE TRIGGER independent_reviews_no_delete BEFORE DELETE ON independent_reviews BEGIN SELECT RAISE(ABORT,'retained review intent'); END;
CREATE TRIGGER review_observations_no_update BEFORE UPDATE ON review_observations BEGIN SELECT RAISE(ABORT,'append-only review observations'); END;
CREATE TRIGGER review_observations_no_delete BEFORE DELETE ON review_observations BEGIN SELECT RAISE(ABORT,'retained review observations'); END;

"#;

pub(super) fn validate(state: &SavedState) -> StoreResult<()> {
    for (id, record) in &state.reviews {
        let i = &record.intent;
        if id != &i.id || !record.validate() {
            return Err(StoreError::InvalidState);
        }
        let source = state
            .workers
            .get(&i.worker_id)
            .ok_or(StoreError::InvalidState)?;
        let task = state
            .runs
            .get(&RunId::new(&i.run_id)?)
            .ok_or(StoreError::InvalidState)?
            .task(&TaskId::new(&i.task_id)?)?;
        let implementation = task
            .attempts()
            .iter()
            .find(|a| a.id().as_str() == i.source_attempt_id)
            .ok_or(StoreError::InvalidState)?;
        let reviewer = state
            .sessions
            .get(&RuntimeSessionId::new(&i.session_id)?)
            .ok_or(StoreError::InvalidState)?;
        let artifact = state
            .captured_artifacts
            .get(&(i.artifact_id.clone(), i.artifact_revision.clone()))
            .ok_or(StoreError::InvalidState)?;
        if source.intent.run_id != i.run_id
            || source.intent.task_id != i.task_id
            || source.intent.attempt_id != i.source_attempt_id
            || reviewer.id() == implementation.session().id()
            || reviewer.connection_id() != implementation.session().connection_id()
            || artifact.attempt_id != i.source_attempt_id
            || artifact.base_oid != i.base_oid
            || i.runtime_request["sessionId"] != i.session_id
            || i.runtime_request["attemptId"] != i.attempt_id
            || i.runtime_request["requested"]["sandbox"] != "read-only"
            || i.runtime_request["billingMode"] != "subscription-only"
        {
            return Err(StoreError::InvalidState);
        }
        if record.status() == ReviewStatus::SetAside {
            let details = &record
                .observations
                .last()
                .ok_or(StoreError::InvalidState)?
                .details;
            let binding = record
                .observations
                .iter()
                .find_map(|o| o.details.get("guardian"))
                .ok_or(StoreError::InvalidState)?;
            let binding: crate::recovery_guardian::GuardianBinding =
                serde_json::from_value(binding.clone()).map_err(|_| StoreError::InvalidState)?;
            if !crate::recovery_guardian::valid_proof(&binding, &details["localClosureProof"])
                || LocalRef::new(details["authorizationRef"].as_str().unwrap_or("")).is_err()
                || details["resumeAutomatically"] != false
                || details["remoteOutcomeUnchanged"] != true
            {
                return Err(StoreError::InvalidState);
            }
        }
        let technical_count = record
            .observations
            .iter()
            .filter(|o| matches!(o.status, ReviewStatus::Accepted | ReviewStatus::Rejected))
            .count();
        let domain_count = task
            .reviews()
            .iter()
            .filter(|r| r.reference.as_str() == i.id)
            .count();
        if technical_count != domain_count {
            return Err(StoreError::InvalidState);
        }
        for observation in &record.observations {
            if matches!(
                observation.status,
                ReviewStatus::Accepted | ReviewStatus::Rejected
            ) {
                let output =
                    validate_final_evidence(state, i, observation.status, &observation.details)?;
                if !task.reviews().iter().any(|r| {
                    r.reference.as_str() == i.id
                        && r.artifact.id().as_str() == i.artifact_id
                        && r.artifact.revision().as_str() == i.artifact_revision
                        && (r.result == ReviewResult::Accepted)
                            == (output.verdict == Verdict::Accepted)
                }) {
                    return Err(StoreError::InvalidState);
                }
            } else if observation.details.get("output").is_some() {
                return Err(StoreError::InvalidState);
            }
        }
    }
    Ok(())
}

fn validate_final_evidence(
    state: &SavedState,
    intent: &ReviewIntent,
    status: ReviewStatus,
    details: &serde_json::Value,
) -> StoreResult<ReviewOutput> {
    let output: ReviewOutput =
        serde_json::from_value(details["output"].clone()).map_err(|_| StoreError::InvalidState)?;
    let artifact = state
        .captured_artifacts
        .get(&(intent.artifact_id.clone(), intent.artifact_revision.clone()))
        .ok_or(StoreError::InvalidState)?;
    ReviewOutput::parse(
        &serde_json::to_string(&output).map_err(|_| StoreError::InvalidState)?,
        &intent.artifact_revision,
        artifact,
    )
    .map_err(|_| StoreError::InvalidState)?;
    let observer =
        crate::runtime_contract::RuntimeObserver::restore_checkpoint(&details["observer"])
            .map_err(|_| StoreError::InvalidState)?;
    let snapshot = observer.snapshot();
    let binding = crate::runtime::SessionBinding::restore(&details["adapterBinding"])
        .map_err(|_| StoreError::InvalidState)?;
    let source = state
        .workers
        .get(&intent.worker_id)
        .ok_or(StoreError::InvalidState)?;
    let source_binding =
        crate::review::source_binding(source).map_err(|_| StoreError::InvalidState)?;
    if !binding.independent_of(&source_binding)
        || binding.checkpoint()["sessionId"] != intent.session_id
        || observer.request().as_json() != &intent.runtime_request
        || snapshot["outcome"] != "completed"
        || snapshot["terminalConfirmed"] != true
        || snapshot["effective"]["sandbox"]["value"] != "read-only"
        || snapshot["effective"]["network"]["value"] != "disabled"
        || snapshot["process"]["tree"]["activeCount"] != 0
        || snapshot["process"]["tree"]["ownership"] != "confirmed"
        || snapshot["fatal"] != false
        || !snapshot["fault"].is_null()
        || !details["fault"].is_null()
        || snapshot["process"]["runtime"]["stdioClosed"] != true
        || snapshot["process"]["runtime"]["exited"] != true
        || (output.verdict == Verdict::Accepted) != (status == ReviewStatus::Accepted)
        || details["treeStopped"] != true
        || details["streamsClosed"] != true
        || details["readOnlyConfirmed"] != true
        || details["terminalSucceeded"] != true
    {
        return Err(StoreError::InvalidState);
    }
    Ok(output)
}

pub(super) fn apply(state: &mut SavedState, command: &ReviewCommand) -> StoreResult<()> {
    match command {
        ReviewCommand::Reserve { record, session } => {
            crate::configuration::guard_context_snapshot(state, &record.intent.context)
                .map_err(|_| StoreError::Configuration)?;
            if !record.intent.validate()
                || !record.observations.is_empty()
                || state.reviews.contains_key(&record.intent.id)
                || state
                    .reviews
                    .values()
                    .any(|r| r.status().blocks_admission())
                || state
                    .workers
                    .values()
                    .any(|w| w.status().blocks_admission())
                || state
                    .integrations
                    .values()
                    .any(|r| r.status().blocks_admission())
            {
                return Err(DomainError::InvalidTransition.into());
            }
            let source = state
                .workers
                .get(&record.intent.worker_id)
                .ok_or(DomainError::UnknownEntity)?;
            let task = state
                .runs
                .get(&RunId::new(&record.intent.run_id)?)
                .ok_or(DomainError::UnknownEntity)?
                .task(&TaskId::new(&record.intent.task_id)?)?;
            if source.status() != crate::worker_records::WorkerStatus::Verified
                || task
                    .artifact()
                    .is_none_or(|a| a.revision().as_str() != record.intent.artifact_revision)
                || !matches!(task.state(), TaskState::NeedsReview | TaskState::Approved)
            {
                return Err(DomainError::StaleEvidence.into());
            }
            Command::RegisterSession(session.clone()).apply(state)?;
            state
                .reviews
                .insert(record.intent.id.clone(), *record.clone());
        }
        ReviewCommand::Observe {
            id,
            status,
            details,
        } => {
            let old = state.reviews.get(id).ok_or(DomainError::UnknownEntity)?;
            if !transition(old.status(), *status)
                || old.observations.len() >= 128
                || !crate::worker_records::exact_safe(details, 131_072)
            {
                return Err(DomainError::InvalidTransition.into());
            }
            if *status == ReviewStatus::SetAside {
                let binding = old
                    .observations
                    .iter()
                    .find_map(|o| o.details.get("guardian"))
                    .ok_or(StoreError::InvalidState)?;
                let binding: crate::recovery_guardian::GuardianBinding =
                    serde_json::from_value(binding.clone())
                        .map_err(|_| StoreError::InvalidState)?;
                if !crate::recovery_guardian::valid_proof(&binding, &details["localClosureProof"])
                    || LocalRef::new(details["authorizationRef"].as_str().unwrap_or("")).is_err()
                    || details["resumeAutomatically"] != false
                    || details["remoteOutcomeUnchanged"] != true
                {
                    return Err(StoreError::InvalidState);
                }
            }
            if matches!(status, ReviewStatus::Accepted | ReviewStatus::Rejected) {
                let output = validate_final_evidence(state, &old.intent, *status, details)?;
                let task = state
                    .runs
                    .get(&RunId::new(&old.intent.run_id)?)
                    .ok_or(DomainError::UnknownEntity)?
                    .task(&TaskId::new(&old.intent.task_id)?)?;
                let exact = task.artifact().ok_or(DomainError::MissingEvidence)?.clone();
                if exact.revision().as_str() != old.intent.artifact_revision {
                    return Err(DomainError::StaleEvidence.into());
                }
                Command::run(
                    RunId::new(&old.intent.run_id)?,
                    RunAction::Review {
                        task: TaskId::new(&old.intent.task_id)?,
                        evidence: ReviewEvidence {
                            artifact: exact,
                            reference: EvidenceId::new(id)?,
                            result: if output.verdict == Verdict::Accepted {
                                ReviewResult::Accepted
                            } else {
                                ReviewResult::Rejected
                            },
                        },
                    },
                )
                .apply(state)?;
            }
            state
                .reviews
                .get_mut(id)
                .ok_or(DomainError::UnknownEntity)?
                .observations
                .push(ReviewObservation {
                    status: *status,
                    details: details.clone(),
                });
        }
    }
    validate(state)
}
pub(super) fn load(connection: &Connection, state: &mut SavedState) -> StoreResult<()> {
    let mut q = connection
        .prepare("SELECT id,run_id,task_id,intent FROM independent_reviews ORDER BY id")?;
    for row in q.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (id, run, task, body) = row?;
        if body.len() > 1_048_576 {
            return Err(StoreError::InvalidState);
        }
        let intent: ReviewIntent =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        if intent.id != id || intent.run_id != run || intent.task_id != task {
            return Err(StoreError::InvalidState);
        }
        state.reviews.insert(
            id,
            ReviewRecord {
                intent,
                observations: vec![],
            },
        );
    }
    let mut q = connection.prepare(
        "SELECT review_id,ordinal,body,digest FROM review_observations ORDER BY review_id,ordinal",
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
        if body.len() > 131_072 || content_hash(body.as_bytes()) != digest {
            return Err(StoreError::InvalidState);
        }
        let observation = serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        let record = state.reviews.get_mut(&id).ok_or(StoreError::InvalidState)?;
        if ordinal != record.observations.len() as i64 + 1 {
            return Err(StoreError::InvalidState);
        }
        record.observations.push(observation);
    }
    // Cross-record validation belongs to the caller after all collections load.
    // In particular correction workers and reviews refer to one another.
    Ok(())
}
pub(super) fn save(connection: &Connection, state: &SavedState) -> StoreResult<()> {
    for record in state.reviews.values() {
        let i = &record.intent;
        let body = serde_json::to_string(i).map_err(|_| StoreError::InvalidState)?;
        connection.execute(
            "INSERT OR IGNORE INTO independent_reviews VALUES(?1,?2,?3,?4)",
            params![i.id, i.run_id, i.task_id, body],
        )?;
        let saved: String = connection.query_row(
            "SELECT intent FROM independent_reviews WHERE id=?1",
            [&i.id],
            |r| r.get(0),
        )?;
        if saved != body {
            return Err(StoreError::InvalidState);
        }
        for (n, o) in record.observations.iter().enumerate() {
            let body = serde_json::to_string(o).map_err(|_| StoreError::InvalidState)?;
            connection.execute(
                "INSERT OR IGNORE INTO review_observations VALUES(?1,?2,?3,?4)",
                params![i.id, (n + 1) as i64, body, content_hash(body.as_bytes())],
            )?;
            let saved: String = connection.query_row(
                "SELECT body FROM review_observations WHERE review_id=?1 AND ordinal=?2",
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

pub(super) fn validate_correction(
    state: &SavedState,
    record: &crate::worker_records::WorkerRecord,
    origin: &crate::worker_records::CorrectionOrigin,
) -> StoreResult<()> {
    let review = state
        .reviews
        .get(&origin.review_id)
        .ok_or(StoreError::InvalidState)?;
    let source = state
        .workers
        .get(&origin.source_worker_id)
        .ok_or(StoreError::InvalidState)?;
    let artifact = state
        .captured_artifacts
        .get(&(origin.artifact_id.clone(), origin.artifact_revision.clone()))
        .ok_or(StoreError::InvalidState)?;
    let i = &record.intent;
    let task = state
        .runs
        .get(&RunId::new(&i.run_id)?)
        .ok_or(StoreError::InvalidState)?
        .task(&TaskId::new(&i.task_id)?)?;
    let attempt = task
        .attempts()
        .iter()
        .find(|a| a.id().as_str() == i.attempt_id)
        .ok_or(StoreError::InvalidState)?;
    let previous = task
        .attempts()
        .iter()
        .position(|a| a.id().as_str() == i.attempt_id)
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| task.attempts().get(n))
        .ok_or(StoreError::InvalidState)?;
    if LocalRef::new(&origin.authorization_ref).is_err()
        || source.intent.id != review.intent.worker_id
        || review.intent.artifact_revision != origin.artifact_revision
        || review.intent.artifact_id != origin.artifact_id
        || review
            .output()
            .is_none_or(|o| o.verdict != Verdict::Rejected)
        || artifact.attempt_id != source.intent.attempt_id
        || artifact.base_oid != i.base_oid
        || source.intent.run_id != i.run_id
        || source.intent.task_id != i.task_id
        || source.intent.project_id != i.project_id
        || source.intent.repository_id != i.repository_id
        || source.intent.attempt_id != previous.id().as_str()
        || attempt.inputs().correction_of.as_ref().is_none_or(|a| {
            a.id().as_str() != origin.artifact_id
                || a.revision().as_str() != origin.artifact_revision
        })
        || attempt.inputs().base.as_str() != origin.artifact_revision
        || i.authorized_checks != source.intent.authorized_checks
        || !crate::review::supported_correction_source(artifact)
    {
        return Err(StoreError::InvalidState);
    }
    Ok(())
}
pub(super) fn admit_correction(
    state: &SavedState,
    intent: &crate::worker_records::WorkerIntent,
    origin: &crate::worker_records::CorrectionOrigin,
) -> StoreResult<()> {
    let review = state
        .reviews
        .get(&origin.review_id)
        .ok_or(DomainError::MissingEvidence)?;
    let task = state
        .runs
        .get(&RunId::new(&intent.run_id)?)
        .ok_or(DomainError::UnknownEntity)?
        .task(&TaskId::new(&intent.task_id)?)?;
    let artifact = state
        .captured_artifacts
        .get(&(origin.artifact_id.clone(), origin.artifact_revision.clone()))
        .ok_or(DomainError::MissingEvidence)?;
    if !crate::review::supported_correction_source(artifact) {
        return Err(DomainError::Policy.into());
    }
    let count = state
        .workers
        .values()
        .filter(|w| {
            w.intent.run_id == intent.run_id
                && w.intent.task_id == intent.task_id
                && w.intent.correction.is_some()
        })
        .count();
    if review.status() != ReviewStatus::Rejected
        || crate::worker::now().map_err(|_| StoreError::InvalidState)? >= review.intent.deadline_ms
        || count >= review.intent.budget.max_corrections as usize
        || state.workers.values().any(|w| {
            w.intent
                .correction
                .as_ref()
                .is_some_and(|c| c.review_id == origin.review_id)
        })
        || task.artifact().is_none_or(|a| {
            a.id().as_str() != origin.artifact_id
                || a.revision().as_str() != origin.artifact_revision
        })
        || task
            .current_attempt()
            .is_none_or(|a| a.id().as_str() != review.intent.source_attempt_id)
    {
        return Err(DomainError::Policy.into());
    }
    Ok(())
}
