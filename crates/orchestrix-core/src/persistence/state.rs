use super::{AttemptSnapshot, StoreError, StoreResult, digest};
use crate::domain::*;
use crate::persistence_privacy::sanitize_checkpoint;
use crate::persistence_records::{DefinitionKind, DocumentRecords};
use rusqlite::{Connection as SqlConnection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// A detached, read-only loaded view. Loading records no current process/auth fact.
#[derive(Default)]
pub struct SavedState {
    pub(super) revision: u64,
    pub(super) projects: BTreeMap<ProjectId, Project>,
    pub(super) connections: BTreeMap<(ConnectionId, u64), Connection>,
    pub(super) capacity_groups: BTreeMap<CapacityGroupId, CapacityGroup>,
    pub(super) sessions: BTreeMap<RuntimeSessionId, RuntimeSessionRef>,
    pub(super) runs: BTreeMap<RunId, Run>,
    pub(super) conversations: ConversationStore,
    pub(super) drafts: BTreeMap<ConversationId, String>,
    pub(super) records: DocumentRecords,
    pub(super) snapshots: BTreeMap<AttemptId, AttemptSnapshot>,
}
impl SavedState {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn projects(&self) -> &BTreeMap<ProjectId, Project> {
        &self.projects
    }
    pub fn connections(&self) -> &BTreeMap<(ConnectionId, u64), Connection> {
        &self.connections
    }
    pub fn capacity_groups(&self) -> &BTreeMap<CapacityGroupId, CapacityGroup> {
        &self.capacity_groups
    }
    pub fn sessions(&self) -> &BTreeMap<RuntimeSessionId, RuntimeSessionRef> {
        &self.sessions
    }
    pub fn runs(&self) -> &BTreeMap<RunId, Run> {
        &self.runs
    }
    pub fn conversations(&self) -> &ConversationStore {
        &self.conversations
    }
    pub fn drafts(&self) -> &BTreeMap<ConversationId, String> {
        &self.drafts
    }
    pub fn records(&self) -> &DocumentRecords {
        &self.records
    }
    pub fn snapshots(&self) -> &BTreeMap<AttemptId, AttemptSnapshot> {
        &self.snapshots
    }

    pub(super) fn checkpoint(&self) -> Value {
        json!({"schemaVersion":1,
            "projects":self.projects.values().map(Project::checkpoint).collect::<Vec<_>>(),
            "connections":self.connections.values().map(Connection::checkpoint).collect::<Vec<_>>(),
            "capacityGroups":self.capacity_groups.values().map(CapacityGroup::checkpoint).collect::<Vec<_>>(),
            "sessions":self.sessions.values().map(RuntimeSessionRef::checkpoint).collect::<Vec<_>>(),
            "runs":self.runs.values().map(Run::checkpoint).collect::<Vec<_>>(),
            "conversations":self.conversations.checkpoint(),"records":self.records.checkpoint(),
            "drafts":self.drafts.iter().map(|(id,text)|json!([id.as_str(),text])).collect::<Vec<_>>(),
            "snapshots":self.snapshots.values().collect::<Vec<_>>()})
    }
    pub(super) fn validate_snapshot(
        &self,
        attempt: &TaskAttempt,
        snapshot: &AttemptSnapshot,
    ) -> StoreResult<()> {
        let d = &snapshot.definition;
        let input = attempt.inputs();
        if d.attempt_id != attempt.id().as_str()
            || d.mode.id != input.mode.id().as_str()
            || d.mode.version != input.mode.version().get()
            || d.profile.id != input.profile.id().as_str()
            || d.profile.version != input.profile.version().get()
            || d.context.id != input.context.reference.as_str()
            || d.context.version != input.context.version.get()
            || d.context.kind != DefinitionKind::Context
            || d.configuration.kind != DefinitionKind::Configuration
            || d.mode.kind != DefinitionKind::Mode
            || d.profile.kind != DefinitionKind::Profile
        {
            return Err(DomainError::Ownership.into());
        }
        let mode = self
            .records
            .definition(&d.mode)
            .ok_or(DomainError::MissingEvidence)?;
        let profile = self
            .records
            .definition(&d.profile)
            .ok_or(DomainError::MissingEvidence)?;
        if mode.body != json!({"name":input.mode.name(),"purpose":input.mode.purpose()})
            || profile.body
                != json!({"name":input.profile.name(),"role":input.profile.role(),"instructions_ref":input.profile.instructions().as_str()})
            || self.records.definition(&d.context).is_none()
            || self.records.definition(&d.configuration).is_none()
        {
            return Err(DomainError::Ownership.into());
        }
        let mut unique = BTreeSet::new();
        for id in &snapshot.diagnostics {
            let observation = self
                .records
                .diagnostic(id)
                .ok_or(DomainError::MissingEvidence)?;
            if !unique.insert(id)
                || observation.connection_id != attempt.session().connection_id().as_str()
                || observation.connection_revision != attempt.session().connection_revision()
            {
                return Err(DomainError::Ownership.into());
            }
        }
        Ok(())
    }
    fn validate(&self) -> StoreResult<()> {
        for group in self.capacity_groups.values() {
            if group
                .members()
                .iter()
                .any(|member| !self.connections.keys().any(|(id, _)| id == member))
            {
                return Err(StoreError::InvalidState);
            }
        }
        let projects = self
            .conversations
            .projects()
            .map(|p| (p.id().clone(), p.checkpoint()))
            .collect::<BTreeMap<_, _>>();
        if projects
            != self
                .projects
                .iter()
                .map(|(id, p)| (id.clone(), p.checkpoint()))
                .collect()
        {
            return Err(StoreError::InvalidState);
        }
        let mut repositories = BTreeMap::new();
        for project in self.projects.values() {
            if let Some(location) =
                repositories.insert(project.repository().id(), project.repository().location())
                && location != project.repository().location()
            {
                return Err(StoreError::InvalidState);
            }
        }
        for session in self.sessions.values() {
            let connection = self
                .connections
                .get(&(
                    session.connection_id().clone(),
                    session.connection_revision(),
                ))
                .ok_or(StoreError::InvalidState)?;
            if session.adapter_id() != connection.adapter_id() {
                return Err(StoreError::InvalidState);
            }
        }
        let latest =
            self.connections
                .keys()
                .fold(BTreeMap::<String, u64>::new(), |mut map, (id, rev)| {
                    map.entry(id.as_str().to_owned())
                        .and_modify(|current| *current = (*current).max(*rev))
                        .or_insert(*rev);
                    map
                });
        if &latest != self.records.connection_revisions() {
            return Err(StoreError::InvalidState);
        }
        let mut attempts = BTreeMap::new();
        let mut active = BTreeSet::new();
        for run in self.runs.values() {
            if let Some(id) = run.project() {
                let project = self.projects.get(id).ok_or(StoreError::InvalidState)?;
                if Some(project.repository().id()) != run.repository() {
                    return Err(StoreError::InvalidState);
                }
            }
            for attempt in run.tasks().values().flat_map(Task::attempts) {
                if attempts.insert(attempt.id(), attempt).is_some()
                    || self.sessions.get(attempt.session().id()) != Some(attempt.session())
                {
                    return Err(StoreError::InvalidState);
                }
                if !super::commands::resolved(attempt.state())
                    && !active.insert(attempt.session().id())
                {
                    return Err(StoreError::InvalidState);
                }
                let snapshot = self
                    .snapshots
                    .get(attempt.id())
                    .ok_or(StoreError::InvalidState)?;
                self.validate_snapshot(attempt, snapshot)
                    .map_err(|_| StoreError::InvalidState)?;
            }
        }
        if self.snapshots.len() != attempts.len()
            || self.records.snapshots().len() != attempts.len()
        {
            return Err(StoreError::InvalidState);
        }
        for snapshot in self.snapshots.values() {
            if !self.records.snapshots().contains(&snapshot.definition) {
                return Err(StoreError::InvalidState);
            }
        }
        for conversation in self.conversations.conversations() {
            for work in conversation.work_references() {
                let run = self
                    .runs
                    .get(work.run_id())
                    .ok_or(StoreError::InvalidState)?;
                // Historical result/project snapshots need not match the current conversation.
                if run.project() != work.project_at_link() {
                    return Err(StoreError::InvalidState);
                }
                if let Some(task) = work.task_id() {
                    let task = run.task(task).map_err(|_| StoreError::InvalidState)?;
                    if let Some(id) = work.attempt_id() {
                        let attempt = task
                            .attempts()
                            .iter()
                            .find(|a| a.id() == id)
                            .ok_or(StoreError::InvalidState)?;
                        if work.plan_revision() != Some(&attempt.inputs().plan_revision) {
                            return Err(StoreError::InvalidState);
                        }
                    }
                }
            }
        }
        for (id, text) in &self.drafts {
            if self.conversations.conversation(id).is_none() || text.trim().is_empty() {
                return Err(StoreError::InvalidState);
            }
        }
        Ok(())
    }
    /// Redact only supported local content, then validate the exact representation to be saved.
    pub(super) fn sanitized(&self) -> StoreResult<Self> {
        let checkpoint = sanitize_checkpoint(&self.checkpoint()).map_err(StoreError::Privacy)?;
        let mut state = Self::default();
        for item in array(&checkpoint, "projects")? {
            let value = Project::restore_checkpoint(item)?;
            state.projects.insert(value.id().clone(), value);
        }
        for item in array(&checkpoint, "connections")? {
            let value = Connection::restore_checkpoint(item)?;
            state
                .connections
                .insert((value.id().clone(), value.revision()), value);
        }
        for item in array(&checkpoint, "capacityGroups")? {
            let value = CapacityGroup::restore_checkpoint(item)?;
            state.capacity_groups.insert(value.id().clone(), value);
        }
        for item in array(&checkpoint, "sessions")? {
            let value = RuntimeSessionRef::restore_checkpoint(item)?;
            state.sessions.insert(value.id().clone(), value);
        }
        for item in array(&checkpoint, "runs")? {
            let value = Run::restore_checkpoint(item)?;
            state.runs.insert(value.id().clone(), value);
        }
        state.conversations = ConversationStore::restore_checkpoint(&checkpoint["conversations"])?;
        state.records =
            DocumentRecords::restore(&checkpoint["records"]).map_err(StoreError::Records)?;
        for item in array(&checkpoint, "drafts")? {
            let id = ConversationId::new(item[0].as_str().ok_or(StoreError::InvalidState)?)?;
            let text = item[1].as_str().ok_or(StoreError::InvalidState)?.to_owned();
            state.drafts.insert(id, text);
        }
        for item in array(&checkpoint, "snapshots")? {
            let value: AttemptSnapshot =
                serde_json::from_value(item.clone()).map_err(|_| StoreError::InvalidState)?;
            state
                .snapshots
                .insert(AttemptId::new(&value.definition.attempt_id)?, value);
        }
        state.revision = self.revision;
        state.validate()?;
        Ok(state)
    }
}

fn array<'a>(value: &'a Value, key: &str) -> StoreResult<&'a Vec<Value>> {
    value[key].as_array().ok_or(StoreError::InvalidState)
}
fn decode(text: &str) -> StoreResult<Value> {
    serde_json::from_str(text).map_err(|_| StoreError::InvalidState)
}
fn encode(value: &Value) -> StoreResult<String> {
    serde_json::to_string(value).map_err(|_| StoreError::InvalidState)
}
fn checkpoint_rows(connection: &SqlConnection, sql: &str) -> StoreResult<Vec<(String, Value)>> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (id, text) = row?;
        Ok((id, decode(&text)?))
    })
    .collect()
}

pub(super) fn load(connection: &SqlConnection) -> StoreResult<SavedState> {
    let mut state = SavedState {
        revision: u64::try_from(connection.query_row(
            "SELECT revision FROM store_meta WHERE singleton=1",
            [],
            |row| row.get::<_, i64>(0),
        )?)
        .map_err(|_| StoreError::InvalidState)?,
        ..SavedState::default()
    };
    let invalid: Option<String> = connection
        .prepare("PRAGMA foreign_key_check")?
        .query_map([], |row| row.get(0))?
        .next()
        .transpose()?;
    if invalid.is_some() {
        return Err(StoreError::InvalidState);
    }
    for (id, data) in checkpoint_rows(connection, "SELECT id,checkpoint FROM projects ORDER BY id")?
    {
        let value = Project::restore_checkpoint(&data)?;
        if value.id().as_str() != id {
            return Err(StoreError::InvalidState);
        }
        state.projects.insert(value.id().clone(), value);
    }
    for (id, data) in checkpoint_rows(
        connection,
        "SELECT id,checkpoint FROM connections ORDER BY id,revision",
    )? {
        let value = Connection::restore_checkpoint(&data)?;
        if value.id().as_str() != id {
            return Err(StoreError::InvalidState);
        }
        state
            .connections
            .insert((value.id().clone(), value.revision()), value);
    }
    for (id, data) in checkpoint_rows(connection, "SELECT id,checkpoint FROM sessions ORDER BY id")?
    {
        let value = RuntimeSessionRef::restore_checkpoint(&data)?;
        if value.id().as_str() != id {
            return Err(StoreError::InvalidState);
        }
        state.sessions.insert(value.id().clone(), value);
    }
    for (id, data) in checkpoint_rows(
        connection,
        "SELECT id,checkpoint FROM capacity_groups ORDER BY id",
    )? {
        let group = CapacityGroup::restore_checkpoint(&data)?;
        if group.id().as_str() != id {
            return Err(StoreError::InvalidState);
        }
        state.capacity_groups.insert(group.id().clone(), group);
    }
    for (id, data) in checkpoint_rows(connection, "SELECT id,checkpoint FROM runs ORDER BY id")? {
        let value = Run::restore_checkpoint(&data)?;
        if value.id().as_str() != id {
            return Err(StoreError::InvalidState);
        }
        state.runs.insert(value.id().clone(), value);
    }
    let conversation_store: Option<String> = connection
        .query_row(
            "SELECT checkpoint FROM conversation_store WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    state.conversations = match conversation_store {
        Some(text) => ConversationStore::restore_checkpoint(&decode(&text)?)?,
        None => ConversationStore::new(),
    };
    let records: Option<String> = connection
        .query_row(
            "SELECT checkpoint FROM document_records WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(text) = records {
        state.records = DocumentRecords::restore(&decode(&text)?).map_err(StoreError::Records)?;
    }
    let mut statement = connection
        .prepare("SELECT conversation_id,content FROM saved_drafts ORDER BY conversation_id")?;
    for row in statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, text) = row?;
        state.drafts.insert(ConversationId::new(id)?, text);
    }
    for (id, data) in checkpoint_rows(
        connection,
        "SELECT attempt_id,checkpoint FROM attempt_snapshots ORDER BY attempt_id",
    )? {
        let value: AttemptSnapshot =
            serde_json::from_value(data).map_err(|_| StoreError::InvalidState)?;
        if value.definition.attempt_id != id {
            return Err(StoreError::InvalidState);
        }
        state.snapshots.insert(AttemptId::new(id)?, value);
    }
    state.validate()?;
    let checkpoint = state.checkpoint();
    if sanitize_checkpoint(&checkpoint).map_err(StoreError::Privacy)? != checkpoint {
        return Err(StoreError::InvalidState);
    }
    validate_projections(connection, &state)?;
    Ok(state)
}

use rusqlite::OptionalExtension;

pub(super) fn save(connection: &SqlConnection, state: &SavedState) -> StoreResult<()> {
    state.validate()?;
    for project in state.projects.values() {
        connection.execute(
            "INSERT INTO repositories(id,location) VALUES(?1,?2) ON CONFLICT(id) DO NOTHING",
            params![
                project.repository().id().as_str(),
                project.repository().location().as_str()
            ],
        )?;
        connection.execute("INSERT INTO projects(id,repository_id,checkpoint) VALUES(?1,?2,?3) ON CONFLICT(id) DO NOTHING",params![project.id().as_str(),project.repository().id().as_str(),encode(&project.checkpoint())?])?;
    }
    for value in state.connections.values() {
        connection.execute("INSERT INTO connections(id,revision,adapter,checkpoint) VALUES(?1,?2,?3,?4) ON CONFLICT(id,revision) DO NOTHING",params![value.id().as_str(),super::sql_int(value.revision())?,value.adapter_id(),encode(&value.checkpoint())?])?;
    }
    for value in state.capacity_groups.values() {
        connection.execute("INSERT INTO capacity_groups(id,checkpoint) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET checkpoint=excluded.checkpoint",params![value.id().as_str(),encode(&value.checkpoint())?])?;
    }
    for value in state.sessions.values() {
        connection.execute("INSERT INTO sessions(id,connection_id,connection_revision,adapter,checkpoint) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO NOTHING",params![value.id().as_str(),value.connection_id().as_str(),super::sql_int(value.connection_revision())?,value.adapter_id(),encode(&value.checkpoint())?])?;
    }
    // Release terminal reservations before inserting another reservation on that session.
    for run in state.runs.values() {
        for attempt in run
            .tasks()
            .values()
            .flat_map(Task::attempts)
            .filter(|a| super::commands::resolved(a.state()))
        {
            connection.execute(
                "UPDATE attempts SET active=0 WHERE id=?1",
                [attempt.id().as_str()],
            )?;
        }
    }
    for run in state.runs.values() {
        connection.execute("INSERT INTO runs(id,project_id,repository_id,result_revision,state,checkpoint) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET result_revision=excluded.result_revision,state=excluded.state,checkpoint=excluded.checkpoint",params![run.id().as_str(),run.project().map(ProjectId::as_str),run.repository().map(RepositoryId::as_str),run.result_revision().as_str(),format!("{:?}",run.state()),encode(&run.checkpoint())?])?;
        for task in run.tasks().values() {
            connection.execute("INSERT INTO tasks(run_id,id,kind,state) VALUES(?1,?2,?3,?4) ON CONFLICT(run_id,id) DO UPDATE SET state=excluded.state",params![run.id().as_str(),task.id().as_str(),format!("{:?}",task.kind()),format!("{:?}",task.state())])?;
        }
        for task in run.tasks().values() {
            for dependency in task.dependencies() {
                connection.execute(
                    "INSERT OR IGNORE INTO dependencies VALUES(?1,?2,?3,?4)",
                    params![
                        run.id().as_str(),
                        task.id().as_str(),
                        dependency.task.as_str(),
                        format!("{:?}", dependency.result)
                    ],
                )?;
            }
            for attempt in task.attempts() {
                connection.execute("INSERT INTO attempts(id,run_id,task_id,session_id,active) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET active=excluded.active",params![attempt.id().as_str(),run.id().as_str(),task.id().as_str(),attempt.session().id().as_str(),!super::commands::resolved(attempt.state())])?;
                for artifact in attempt.artifacts() {
                    connection.execute(
                        "INSERT OR IGNORE INTO artifacts VALUES(?1,?2,?3,?4,?5)",
                        params![
                            run.id().as_str(),
                            task.id().as_str(),
                            artifact.id().as_str(),
                            artifact.revision().as_str(),
                            artifact.base().as_str()
                        ],
                    )?;
                }
            }
            let checkpoint = task.checkpoint();
            let data = &checkpoint["data"];
            for key in ["verifications", "reviews", "integrations"] {
                if let Some(items) = data[key].as_array() {
                    for item in items {
                        save_evidence(connection, run.id(), Some(task.id()), key, item)?;
                    }
                }
            }
        }
        // All supplied application failures/successes and conflict evidence stay historical.
        let checkpoint = run.checkpoint();
        let data = &checkpoint["data"];
        for key in ["applicationFailures", "conflicts"] {
            if let Some(items) = data[key].as_array() {
                for item in items {
                    save_evidence(connection, run.id(), None, key, item)?;
                }
            }
        }
        if let Some(item) = data.get("application").filter(|item| !item.is_null()) {
            save_evidence(connection, run.id(), None, "application", item)?;
        }
    }
    for conversation in state.conversations.conversations() {
        let body =
            json!({"id":conversation.id().as_str(),"messageCount":conversation.messages().len()});
        connection.execute("INSERT INTO conversations(id,project_id,checkpoint) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,checkpoint=excluded.checkpoint",params![conversation.id().as_str(),conversation.project_id().map(ProjectId::as_str),encode(&body)?])?;
        for message in conversation.messages() {
            connection.execute(
                "INSERT OR IGNORE INTO messages VALUES(?1,?2,?3,?4,?5)",
                params![
                    conversation.id().as_str(),
                    super::sql_int(message.order())?,
                    format!("{:?}", message.role()),
                    message.content(),
                    message.project_at_send().map(ProjectId::as_str)
                ],
            )?;
        }
        for (ordinal, reference) in conversation.work_references().iter().enumerate() {
            let body = json!({"resultRevision":reference.run_result_revision().as_str(),"projectAtLink":reference.project_at_link().map(ProjectId::as_str),"planRevision":reference.plan_revision().map(RevisionId::as_str)});
            connection.execute(
                "INSERT OR IGNORE INTO conversation_work VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    conversation.id().as_str(),
                    super::sql_int(ordinal as u64)?,
                    reference.run_id().as_str(),
                    reference.task_id().map(TaskId::as_str),
                    reference.attempt_id().map(AttemptId::as_str),
                    encode(&body)?
                ],
            )?;
        }
    }
    connection.execute("INSERT INTO conversation_store VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET checkpoint=excluded.checkpoint",[encode(&state.conversations.checkpoint())?])?;
    connection.execute("DELETE FROM saved_drafts", [])?;
    for (id, text) in &state.drafts {
        connection.execute(
            "INSERT INTO saved_drafts VALUES(?1,?2)",
            params![id.as_str(), text],
        )?;
    }
    connection.execute("INSERT INTO document_records VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET checkpoint=excluded.checkpoint",[encode(&state.records.checkpoint())?])?;
    for definition in state.records.definitions() {
        let kind = serde_json::to_string(&definition.kind).map_err(|_| StoreError::InvalidState)?;
        connection.execute(
            "INSERT OR IGNORE INTO definitions VALUES(?1,?2,?3,?4)",
            params![
                kind,
                definition.id,
                super::sql_int(definition.version)?,
                encode(&definition.body)?
            ],
        )?;
    }
    for definition in state.records.definitions() {
        let kind = serde_json::to_string(&definition.kind).map_err(|_| StoreError::InvalidState)?;
        for reference in &definition.references {
            let target_kind =
                serde_json::to_string(&reference.kind).map_err(|_| StoreError::InvalidState)?;
            connection.execute(
                "INSERT OR IGNORE INTO definition_references VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    kind,
                    definition.id,
                    super::sql_int(definition.version)?,
                    target_kind,
                    reference.id,
                    super::sql_int(reference.version)?
                ],
            )?;
        }
    }
    for observation in state.records.diagnostics() {
        connection.execute(
            "INSERT OR IGNORE INTO diagnostic_observations VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                observation.id,
                observation.connection_id,
                super::sql_int(observation.connection_revision)?,
                serde_json::to_string(&observation.scope).map_err(|_| StoreError::InvalidState)?,
                super::sql_int(observation.version)?,
                serde_json::to_string(observation).map_err(|_| StoreError::InvalidState)?
            ],
        )?;
    }
    for (attempt, snapshot) in &state.snapshots {
        connection.execute(
            "INSERT OR IGNORE INTO attempt_snapshots VALUES(?1,?2)",
            params![
                attempt.as_str(),
                serde_json::to_string(snapshot).map_err(|_| StoreError::InvalidState)?
            ],
        )?;
    }
    connection.execute(
        "UPDATE store_meta SET revision=?1 WHERE singleton=1",
        [super::sql_int(state.revision)?],
    )?;
    Ok(())
}
fn save_evidence(
    connection: &SqlConnection,
    run: &RunId,
    task: Option<&TaskId>,
    kind: &str,
    item: &Value,
) -> StoreResult<()> {
    let body = encode(item)?;
    let hash =
        digest(encode(&json!([run.as_str(), task.map(TaskId::as_str), kind, item]))?.as_bytes());
    connection.execute(
        "INSERT OR IGNORE INTO evidence VALUES(?1,?2,?3,?4,?5)",
        params![hash, run.as_str(), task.map(TaskId::as_str), kind, body],
    )?;
    Ok(())
}

// Projections have SQL constraints/indexes, but are not a second source of truth.
// Reject disagreement with the validated checkpoints; never silently repair it.
fn check_rows(connection: &SqlConnection, sql: &str, expected: Vec<Value>) -> StoreResult<()> {
    use rusqlite::types::ValueRef;
    let mut statement = connection.prepare(sql)?;
    let columns = statement.column_count();
    let mut query = statement.query([])?;
    let mut actual = Vec::new();
    while let Some(row) = query.next()? {
        let mut values = Vec::new();
        for column in 0..columns {
            values.push(match row.get_ref(column)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => json!(value),
                ValueRef::Text(value) => {
                    json!(std::str::from_utf8(value).map_err(|_| StoreError::InvalidState)?)
                }
                _ => return Err(StoreError::InvalidState),
            });
        }
        actual.push(Value::Array(values).to_string());
    }
    actual.sort();
    let mut expected = expected
        .into_iter()
        .map(|row| row.to_string())
        .collect::<Vec<_>>();
    expected.sort();
    if actual != expected {
        return Err(StoreError::InvalidState);
    }
    Ok(())
}
fn validate_projections(connection: &SqlConnection, state: &SavedState) -> StoreResult<()> {
    let repositories = state
        .projects
        .values()
        .map(|p| {
            json!([
                p.repository().id().as_str(),
                p.repository().location().as_str()
            ])
            .to_string()
        })
        .collect::<BTreeSet<_>>();
    check_rows(
        connection,
        "SELECT id,location FROM repositories",
        repositories
            .into_iter()
            .map(|v| decode(&v))
            .collect::<StoreResult<_>>()?,
    )?;
    check_rows(
        connection,
        "SELECT id,repository_id FROM projects",
        state
            .projects
            .values()
            .map(|p| json!([p.id().as_str(), p.repository().id().as_str()]))
            .collect(),
    )?;
    check_rows(
        connection,
        "SELECT id,revision,adapter FROM connections",
        state
            .connections
            .values()
            .map(|c| json!([c.id().as_str(), c.revision(), c.adapter_id()]))
            .collect(),
    )?;
    check_rows(
        connection,
        "SELECT id,connection_id,connection_revision,adapter FROM sessions",
        state
            .sessions
            .values()
            .map(|s| {
                json!([
                    s.id().as_str(),
                    s.connection_id().as_str(),
                    s.connection_revision(),
                    s.adapter_id()
                ])
            })
            .collect(),
    )?;
    check_rows(
        connection,
        "SELECT id,project_id,repository_id,result_revision,state FROM runs",
        state
            .runs
            .values()
            .map(|r| {
                json!([
                    r.id().as_str(),
                    r.project().map(ProjectId::as_str),
                    r.repository().map(RepositoryId::as_str),
                    r.result_revision().as_str(),
                    format!("{:?}", r.state())
                ])
            })
            .collect(),
    )?;
    let (mut tasks, mut dependencies, mut attempts, mut artifacts, mut evidence) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        BTreeMap::new(),
    );
    for run in state.runs.values() {
        for task in run.tasks().values() {
            tasks.push(json!([
                run.id().as_str(),
                task.id().as_str(),
                format!("{:?}", task.kind()),
                format!("{:?}", task.state())
            ]));
            for dependency in task.dependencies() {
                dependencies.push(json!([
                    run.id().as_str(),
                    task.id().as_str(),
                    dependency.task.as_str(),
                    format!("{:?}", dependency.result)
                ]));
            }
            for attempt in task.attempts() {
                attempts.push(json!([
                    attempt.id().as_str(),
                    run.id().as_str(),
                    task.id().as_str(),
                    attempt.session().id().as_str(),
                    if super::commands::resolved(attempt.state()) {
                        0
                    } else {
                        1
                    }
                ]));
                for artifact in attempt.artifacts() {
                    artifacts.push(json!([
                        run.id().as_str(),
                        task.id().as_str(),
                        artifact.id().as_str(),
                        artifact.revision().as_str(),
                        artifact.base().as_str()
                    ]));
                }
            }
            let checkpoint = task.checkpoint();
            for kind in ["verifications", "reviews", "integrations"] {
                for item in checkpoint["data"][kind]
                    .as_array()
                    .ok_or(StoreError::InvalidState)?
                {
                    projection_evidence(&mut evidence, run.id(), Some(task.id()), kind, item)?;
                }
            }
        }
        let checkpoint = run.checkpoint();
        for kind in ["applicationFailures", "conflicts"] {
            for item in checkpoint["data"][kind]
                .as_array()
                .ok_or(StoreError::InvalidState)?
            {
                projection_evidence(&mut evidence, run.id(), None, kind, item)?;
            }
        }
        if let Some(item) = checkpoint["data"]
            .get("application")
            .filter(|v| !v.is_null())
        {
            projection_evidence(&mut evidence, run.id(), None, "application", item)?;
        }
    }
    check_rows(connection, "SELECT run_id,id,kind,state FROM tasks", tasks)?;
    check_rows(
        connection,
        "SELECT run_id,task_id,prerequisite,result FROM dependencies",
        dependencies,
    )?;
    check_rows(
        connection,
        "SELECT id,run_id,task_id,session_id,active FROM attempts",
        attempts,
    )?;
    check_rows(
        connection,
        "SELECT run_id,task_id,id,revision,base FROM artifacts",
        artifacts,
    )?;
    check_rows(
        connection,
        "SELECT digest,run_id,task_id,kind,body FROM evidence",
        evidence.into_values().collect(),
    )?;
    let (mut conversations, mut messages, mut work) = (Vec::new(), Vec::new(), Vec::new());
    for conversation in state.conversations.conversations() {
        conversations.push(json!([
            conversation.id().as_str(),
            conversation.project_id().map(ProjectId::as_str),
            json!({"id":conversation.id().as_str(),"messageCount":conversation.messages().len()})
                .to_string()
        ]));
        for message in conversation.messages() {
            messages.push(json!([
                conversation.id().as_str(),
                message.order(),
                format!("{:?}", message.role()),
                message.content(),
                message.project_at_send().map(ProjectId::as_str)
            ]));
        }
        for (ordinal, reference) in conversation.work_references().iter().enumerate() {
            let body = json!({"resultRevision":reference.run_result_revision().as_str(),"projectAtLink":reference.project_at_link().map(ProjectId::as_str),"planRevision":reference.plan_revision().map(RevisionId::as_str)});
            work.push(json!([
                conversation.id().as_str(),
                ordinal,
                reference.run_id().as_str(),
                reference.task_id().map(TaskId::as_str),
                reference.attempt_id().map(AttemptId::as_str),
                body.to_string()
            ]));
        }
    }
    check_rows(
        connection,
        "SELECT id,project_id,checkpoint FROM conversations",
        conversations,
    )?;
    check_rows(
        connection,
        "SELECT conversation_id,ordinal,role,content,project_at_send FROM messages",
        messages,
    )?;
    check_rows(
        connection,
        "SELECT conversation_id,ordinal,run_id,task_id,attempt_id,body FROM conversation_work",
        work,
    )?;
    check_rows(
        connection,
        "SELECT attempt_id,checkpoint FROM attempt_snapshots",
        state
            .snapshots
            .iter()
            .map(|(id, snapshot)| {
                Ok(json!([
                    id.as_str(),
                    serde_json::to_string(snapshot).map_err(|_| StoreError::InvalidState)?
                ]))
            })
            .collect::<StoreResult<_>>()?,
    )?;
    let (mut definitions, mut references) = (Vec::new(), Vec::new());
    for definition in state.records.definitions() {
        let kind = serde_json::to_string(&definition.kind).map_err(|_| StoreError::InvalidState)?;
        definitions.push(json!([
            kind,
            definition.id,
            definition.version,
            definition.body.to_string()
        ]));
        for reference in &definition.references {
            references.push(json!([
                kind,
                definition.id,
                definition.version,
                serde_json::to_string(&reference.kind).map_err(|_| StoreError::InvalidState)?,
                reference.id,
                reference.version
            ]));
        }
    }
    check_rows(
        connection,
        "SELECT kind,id,version,body FROM definitions",
        definitions,
    )?;
    check_rows(
        connection,
        "SELECT kind,id,version,target_kind,target_id,target_version FROM definition_references",
        references,
    )?;
    check_rows(
        connection,
        "SELECT id,connection_id,connection_revision,scope,version,body FROM diagnostic_observations",
        state
            .records
            .diagnostics()
            .iter()
            .map(|observation| {
                Ok(json!([
                    observation.id,
                    observation.connection_id,
                    observation.connection_revision,
                    serde_json::to_string(&observation.scope)
                        .map_err(|_| StoreError::InvalidState)?,
                    observation.version,
                    serde_json::to_string(observation).map_err(|_| StoreError::InvalidState)?
                ]))
            })
            .collect::<StoreResult<_>>()?,
    )?;
    Ok(())
}
fn projection_evidence(
    evidence: &mut BTreeMap<String, Value>,
    run: &RunId,
    task: Option<&TaskId>,
    kind: &str,
    item: &Value,
) -> StoreResult<()> {
    let hash =
        digest(encode(&json!([run.as_str(), task.map(TaskId::as_str), kind, item]))?.as_bytes());
    evidence.insert(
        hash.clone(),
        json!([
            hash,
            run.as_str(),
            task.map(TaskId::as_str),
            kind,
            encode(item)?
        ]),
    );
    Ok(())
}
