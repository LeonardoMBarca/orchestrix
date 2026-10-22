//! Atomic local records around (never during) external Git effects.
use super::{SavedState, StoreError, StoreResult};
use crate::{
    domain::{DomainError, ProjectId, RunId, TaskId},
    git_records::*,
};
use rusqlite::{Connection, params};
use std::collections::BTreeSet;

fn bindings(state: &SavedState, intent: &WorktreeIntent) -> StoreResult<()> {
    if !intent.validate() {
        return Err(DomainError::InvalidReference.into());
    }
    let project = state
        .projects
        .get(&ProjectId::new(&intent.project_id)?)
        .ok_or(DomainError::UnknownEntity)?;
    let run = state
        .runs
        .get(&RunId::new(&intent.run_id)?)
        .ok_or(DomainError::UnknownEntity)?;
    let task = run.task(&TaskId::new(&intent.task_id)?)?;
    let attempt = task
        .attempts()
        .iter()
        .find(|a| a.id().as_str() == intent.attempt_id)
        .ok_or(DomainError::UnknownEntity)?;
    if state
        .workers
        .values()
        .find(|w| w.intent.attempt_id == intent.attempt_id)
        .is_some_and(|w| {
            w.intent.editable_paths.as_ref()
                != intent.preservation.as_ref().map(|p| &p.editable_paths)
        })
        || project.repository().id().as_str() != intent.repository_id
        || run.project() != Some(project.id())
        || run.repository() != Some(project.repository().id())
        || attempt.inputs().project.as_ref() != Some(project.id())
        || attempt.inputs().base.as_str() != intent.domain_base
        || attempt.inputs().worktree.as_str() != intent.worktree_ref
    {
        return Err(DomainError::Ownership.into());
    }
    Ok(())
}
fn artifact_bindings(state: &SavedState, artifact: &CapturedArtifact) -> StoreResult<()> {
    if !artifact.validate() {
        return Err(DomainError::InvalidReference.into());
    }
    let record = state
        .worktrees
        .get(&artifact.operation_id)
        .ok_or(DomainError::UnknownEntity)?;
    let i = &record.intent;
    if artifact.repository_id != i.repository_id
        || artifact.run_id != i.run_id
        || artifact.task_id != i.task_id
        || artifact.attempt_id != i.attempt_id
        || artifact.base_oid != i.base_oid
        || artifact.origin != i.path
        || artifact.preservation != i.preservation
    {
        return Err(DomainError::Ownership.into());
    }
    Ok(())
}
pub(super) fn validate(state: &SavedState) -> StoreResult<()> {
    let (mut attempts, mut paths, mut branches) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for (id, record) in &state.worktrees {
        if id != &record.intent.operation_id
            || !record.validate()
            || !attempts.insert(&record.intent.attempt_id)
            || !paths.insert(&record.intent.path)
            || !branches.insert((&record.intent.main_repository, &record.intent.branch))
        {
            return Err(StoreError::InvalidState);
        }
        bindings(state, &record.intent).map_err(|_| StoreError::InvalidState)?;
    }
    for ((id, revision), artifact) in &state.captured_artifacts {
        if id != &artifact.id || revision != &artifact.revision {
            return Err(StoreError::InvalidState);
        }
        artifact_bindings(state, artifact).map_err(|_| StoreError::InvalidState)?;
    }
    Ok(())
}
pub(super) fn apply(state: &mut SavedState, command: &GitCommand) -> StoreResult<()> {
    match command {
        GitCommand::Plan(intent) => {
            bindings(state, intent)?;
            if let Some(previous) = state.worktrees.get(&intent.operation_id) {
                return if previous.intent == *intent {
                    Ok(())
                } else {
                    Err(DomainError::Ownership.into())
                };
            }
            if state.worktrees.values().any(|p| {
                p.intent.attempt_id == intent.attempt_id
                    || p.intent.path == intent.path
                    || (p.intent.main_repository == intent.main_repository
                        && p.intent.branch == intent.branch)
            }) {
                return Err(DomainError::DuplicateIdentity.into());
            }
            state.worktrees.insert(
                intent.operation_id.clone(),
                WorktreeRecord {
                    intent: intent.clone(),
                    state: WorktreeState::Planned,
                    observations: Vec::new(),
                },
            );
        }
        GitCommand::RecordArtifact(artifact) => {
            artifact_bindings(state, artifact)?;
            let key = (artifact.id.clone(), artifact.revision.clone());
            if let Some(previous) = state.captured_artifacts.get(&key) {
                return if previous == artifact {
                    Ok(())
                } else {
                    Err(DomainError::Ownership.into())
                };
            }
            if state.worktrees[&artifact.operation_id].state != WorktreeState::Created {
                return Err(DomainError::InvalidTransition.into());
            }
            state.captured_artifacts.insert(key, artifact.clone());
        }
        GitCommand::Observe {
            operation_id,
            observation,
        } => {
            if !matches!(
                observation.kind.as_str(),
                "inspection-required" | "create-failed" | "cleanup-failed" | "recovery-observed"
            ) || !is_hash(&observation.evidence_sha256)
            {
                return Err(DomainError::InvalidReference.into());
            }
            let record = state
                .worktrees
                .get_mut(operation_id)
                .ok_or(DomainError::UnknownEntity)?;
            if record.state == WorktreeState::Removed || record.observations.len() >= 125 {
                return Err(DomainError::InvalidTransition.into());
            }
            record.observations.push(observation.clone());
        }
        GitCommand::ConfirmCreated {
            operation_id,
            observation,
        }
        | GitCommand::PlanCleanup {
            operation_id,
            observation,
        }
        | GitCommand::ConfirmRemoved {
            operation_id,
            observation,
        } => {
            let (from, to, kind) = match command {
                GitCommand::ConfirmCreated { .. } => {
                    (WorktreeState::Planned, WorktreeState::Created, "created")
                }
                GitCommand::PlanCleanup { .. } => (
                    WorktreeState::Created,
                    WorktreeState::CleanupPlanned,
                    "cleanup-planned",
                ),
                _ => (
                    WorktreeState::CleanupPlanned,
                    WorktreeState::Removed,
                    "removed",
                ),
            };
            if observation.kind != kind || !is_hash(&observation.evidence_sha256) {
                return Err(DomainError::InvalidReference.into());
            }
            let record = state
                .worktrees
                .get_mut(operation_id)
                .ok_or(DomainError::UnknownEntity)?;
            if record.state == to && record.observations.last() == Some(observation) {
                return Ok(());
            }
            if record.state != from {
                return Err(DomainError::InvalidTransition.into());
            }
            record.state = to;
            record.observations.push(observation.clone());
        }
    }
    Ok(())
}
fn encode<T: serde::Serialize>(value: &T) -> StoreResult<String> {
    serde_json::to_string(value).map_err(|_| StoreError::InvalidState)
}
fn decode<T: serde::de::DeserializeOwned>(value: &str, max: usize) -> StoreResult<T> {
    if value.len() > max {
        return Err(StoreError::InvalidState);
    }
    serde_json::from_str(value).map_err(|_| StoreError::InvalidState)
}
pub(super) fn load(connection: &Connection, state: &mut SavedState) -> StoreResult<()> {
    let mut statement = connection.prepare("SELECT operation_id,project_id,repository_id,run_id,task_id,attempt_id,path,branch,main_repository,state,intent FROM git_worktrees ORDER BY operation_id")?;
    let rows = statement.query_map([], |row| {
        (0..11)
            .map(|column| row.get::<_, String>(column))
            .collect::<Result<Vec<_>, _>>()
    })?;
    for row in rows {
        let row = row?;
        let intent: WorktreeIntent = decode(
            &row[10],
            65_536 + crate::git_records::MAX_PRESERVATION_BYTES,
        )?;
        let expected = [
            &intent.operation_id,
            &intent.project_id,
            &intent.repository_id,
            &intent.run_id,
            &intent.task_id,
            &intent.attempt_id,
            &intent.path,
            &intent.branch,
            &intent.main_repository,
        ];
        if row[..9]
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != expected)
        {
            return Err(StoreError::InvalidState);
        }
        let kind: WorktreeState = decode(&row[9], 128)?;
        let mut observations = Vec::new();
        let mut query = connection.prepare("SELECT ordinal,kind,evidence_sha256 FROM git_observations WHERE operation_id=?1 ORDER BY ordinal")?;
        for item in query.query_map([&intent.operation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (ordinal, kind, evidence_sha256) = item?;
            if ordinal != (observations.len() as i64) + 1 {
                return Err(StoreError::InvalidState);
            }
            observations.push(WorktreeObservation {
                kind,
                evidence_sha256,
            });
        }
        state.worktrees.insert(
            intent.operation_id.clone(),
            WorktreeRecord {
                intent,
                state: kind,
                observations,
            },
        );
    }
    let mut query = connection.prepare("SELECT id,revision,operation_id,attempt_id,base_oid,patch_sha256,body FROM captured_artifacts ORDER BY id,revision")?;
    for row in query.query_map([], |row| {
        (0..7)
            .map(|column| row.get::<_, String>(column))
            .collect::<Result<Vec<_>, _>>()
    })? {
        let row = row?;
        let artifact: CapturedArtifact = decode(&row[6], 32 * 1_048_576)?;
        let expected = [
            &artifact.id,
            &artifact.revision,
            &artifact.operation_id,
            &artifact.attempt_id,
            &artifact.base_oid,
            &artifact.patch_sha256,
        ];
        if row[..6]
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != expected)
        {
            return Err(StoreError::InvalidState);
        }
        state
            .captured_artifacts
            .insert((artifact.id.clone(), artifact.revision.clone()), artifact);
    }
    Ok(())
}
pub(super) fn save(connection: &Connection, state: &SavedState) -> StoreResult<()> {
    for record in state.worktrees.values() {
        let i = &record.intent;
        connection.execute("INSERT INTO git_worktrees(operation_id,project_id,repository_id,run_id,task_id,attempt_id,path,branch,main_repository,state,intent) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(operation_id) DO UPDATE SET state=excluded.state",params![i.operation_id,i.project_id,i.repository_id,i.run_id,i.task_id,i.attempt_id,i.path,i.branch,i.main_repository,encode(&record.state)?,encode(i)?])?;
        for (index, o) in record.observations.iter().enumerate() {
            connection.execute(
                "INSERT OR IGNORE INTO git_observations VALUES(?1,?2,?3,?4)",
                params![
                    i.operation_id,
                    super::sql_int(index as u64 + 1)?,
                    o.kind,
                    o.evidence_sha256
                ],
            )?;
        }
    }
    for artifact in state.captured_artifacts.values() {
        // Insertion only: hashes bind bytes, and SQL triggers prohibit later mutation.
        connection.execute(
            "INSERT OR IGNORE INTO captured_artifacts VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                artifact.id,
                artifact.revision,
                artifact.operation_id,
                artifact.attempt_id,
                artifact.base_oid,
                artifact.patch_sha256,
                encode(artifact)?
            ],
        )?;
    }
    Ok(())
}
