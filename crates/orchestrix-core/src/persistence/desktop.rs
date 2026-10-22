//! Narrow Desktop read model. Does not restore workers or grant execution authority.
use super::{Store, StoreError, StoreResult};
use crate::{
    desktop::Settings,
    domain::*,
    git_records::{CapturedArtifact, WorktreeIntent, content_hash, is_hash},
    persistence_records::{DefinitionKind, DocumentRecords},
    review_records::{
        ReviewIntent, ReviewObservation, ReviewOutput, ReviewRecord, ReviewStatus, Verdict,
    },
};
use rusqlite::{Connection as SqlConnection, OptionalExtension, params};
use serde_json::{Value, json};

// Narrow historical projections. Full aggregate restoration remains mandatory
// at startup and every mutation; these reads grant no artifact/admission authority.
fn safe(text: &str) -> StoreResult<()> {
    if crate::persistence_privacy::sanitize_text(text).map_err(StoreError::Privacy)? != text {
        return Err(StoreError::InvalidState);
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(body: &str, limit: usize) -> StoreResult<T> {
    if body.len() > limit {
        return Err(StoreError::InvalidState);
    }
    serde_json::from_str(body).map_err(|_| StoreError::InvalidState)
}

/// Validate only this immutable body and its relational owner. No filesystem
/// inspection, admission or current verification is implied by this projection.
fn historical_artifact(
    connection: &SqlConnection,
    id: &str,
    revision: &str,
) -> StoreResult<Option<CapturedArtifact>> {
    ArtifactId::new(id)?;
    if !is_hash(revision) {
        return Err(StoreError::InvalidState);
    }
    let row: Option<Vec<String>> = connection.query_row(
        "SELECT a.id,a.revision,a.operation_id,a.attempt_id,a.base_oid,a.patch_sha256,a.body,g.intent,g.operation_id,g.project_id,g.repository_id,g.run_id,g.task_id,g.attempt_id,g.path,g.branch,g.main_repository FROM captured_artifacts a JOIN git_worktrees g ON g.operation_id=a.operation_id WHERE a.id=?1 AND a.revision=?2",
        params![id,revision], |r| (0..17).map(|i|r.get(i)).collect()
    ).optional()?;
    let Some(row) = row else { return Ok(None) };
    let artifact: CapturedArtifact = decode(&row[6], 32 * 1_048_576)?;
    let intent: WorktreeIntent =
        decode(&row[7], 65_536 + crate::git_records::MAX_PRESERVATION_BYTES)?;
    let columns = [
        &artifact.id,
        &artifact.revision,
        &artifact.operation_id,
        &artifact.attempt_id,
        &artifact.base_oid,
        &artifact.patch_sha256,
    ];
    let git_columns = [
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
    if !artifact.validate()
        || !intent.validate()
        || row[..6].iter().zip(columns).any(|(a, b)| a != b)
        || row[8..].iter().zip(git_columns).any(|(a, b)| a != b)
        || artifact.repository_id != intent.repository_id
        || artifact.run_id != intent.run_id
        || artifact.task_id != intent.task_id
        || artifact.attempt_id != intent.attempt_id
        || artifact.origin != intent.path
        || artifact.base_oid != intent.base_oid
        || artifact.preservation != intent.preservation
    {
        return Err(StoreError::InvalidState);
    }
    let owned:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM manual_workers w JOIN runs r ON r.id=w.run_id JOIN projects p ON p.id=r.project_id JOIN repositories repo ON repo.id=r.repository_id JOIN tasks t ON t.run_id=r.id AND t.id=w.task_id JOIN attempts a ON a.id=w.attempt_id AND a.run_id=r.id AND a.task_id=t.id JOIN sessions s ON s.id=a.session_id JOIN connections c ON c.id=s.connection_id AND c.revision=s.connection_revision WHERE w.run_id=?1 AND w.task_id=?2 AND w.attempt_id=?3 AND r.repository_id=?4 AND r.project_id=?5 AND p.repository_id=r.repository_id AND c.adapter=s.adapter)",params![artifact.run_id,artifact.task_id,artifact.attempt_id,artifact.repository_id,intent.project_id],|r|r.get(0))?;
    if !owned {
        return Err(StoreError::InvalidState);
    }
    Ok(Some(artifact))
}

fn historical_review(
    connection: &SqlConnection,
    id: &str,
    run: &str,
    task: &str,
    body: &str,
) -> StoreResult<ReviewRecord> {
    let intent: ReviewIntent = decode(body, 1_048_576)?;
    if intent.id != id || intent.run_id != run || intent.task_id != task {
        return Err(StoreError::InvalidState);
    }
    let owned:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM manual_workers w JOIN attempts a ON a.id=w.attempt_id AND a.run_id=w.run_id AND a.task_id=w.task_id JOIN sessions source ON source.id=a.session_id JOIN sessions reviewer ON reviewer.id=?4 JOIN connections reviewer_connection ON reviewer_connection.id=reviewer.connection_id AND reviewer_connection.revision=reviewer.connection_revision AND reviewer_connection.adapter=reviewer.adapter JOIN captured_artifacts ar ON ar.id=?5 AND ar.revision=?6 AND ar.attempt_id=a.id WHERE w.id=?1 AND w.run_id=?2 AND w.task_id=?3 AND w.attempt_id=?7 AND reviewer.id!=source.id AND reviewer.connection_id=source.connection_id AND ar.base_oid=?8)",params![intent.worker_id,run,task,intent.session_id,intent.artifact_id,intent.artifact_revision,intent.source_attempt_id,intent.base_oid],|r|r.get(0))?;
    if !owned
        || intent.runtime_request["sessionId"] != intent.session_id
        || intent.runtime_request["attemptId"] != intent.attempt_id
        || intent.runtime_request["requested"]["sandbox"] != "read-only"
        || intent.runtime_request["billingMode"] != "subscription-only"
    {
        return Err(StoreError::InvalidState);
    }
    let mut record = ReviewRecord {
        intent,
        observations: vec![],
    };
    let mut query=connection.prepare("SELECT ordinal,body,digest FROM review_observations WHERE review_id=? ORDER BY ordinal LIMIT 129")?;
    for row in query.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (ordinal, body, digest) = row?;
        if record.observations.len() >= 128
            || ordinal != record.observations.len() as i64 + 1
            || body.len() > 131_072
            || content_hash(body.as_bytes()) != digest
        {
            return Err(StoreError::InvalidState);
        }
        record
            .observations
            .push(decode::<ReviewObservation>(&body, 131_072)?);
    }
    if !record.validate() {
        return Err(StoreError::InvalidState);
    }
    let artifact = historical_artifact(
        connection,
        &record.intent.artifact_id,
        &record.intent.artifact_revision,
    )?
    .ok_or(StoreError::InvalidState)?;
    let source = super::daemon::historical_worker(connection, &record.intent.worker_id)?
        .ok_or(StoreError::InvalidState)?;
    for observation in &record.observations {
        if matches!(
            observation.status,
            ReviewStatus::Accepted | ReviewStatus::Rejected
        ) {
            let output = ReviewOutput::parse(
                &observation.details["output"].to_string(),
                &record.intent.artifact_revision,
                &artifact,
            )
            .map_err(|_| StoreError::InvalidState)?;
            let observer = crate::runtime_contract::RuntimeObserver::restore_checkpoint(
                &observation.details["observer"],
            )
            .map_err(|_| StoreError::InvalidState)?;
            let snapshot = observer.snapshot();
            let binding =
                crate::runtime::SessionBinding::restore(&observation.details["adapterBinding"])
                    .map_err(|_| StoreError::InvalidState)?;
            let source_binding =
                crate::review::source_binding(&source).map_err(|_| StoreError::InvalidState)?;
            if !binding.independent_of(&source_binding)
                || binding.checkpoint()["sessionId"] != record.intent.session_id
                || observer.request().as_json() != &record.intent.runtime_request
                || snapshot["outcome"] != "completed"
                || snapshot["terminalConfirmed"] != true
                || snapshot["effective"]["sandbox"]["value"] != "read-only"
                || snapshot["effective"]["network"]["value"] != "disabled"
                || snapshot["process"]["tree"]["activeCount"] != 0
                || snapshot["process"]["tree"]["ownership"] != "confirmed"
                || snapshot["fatal"] != false
                || !snapshot["fault"].is_null()
                || !observation.details["fault"].is_null()
                || snapshot["process"]["runtime"]["stdioClosed"] != true
                || snapshot["process"]["runtime"]["exited"] != true
                || (output.verdict == Verdict::Accepted)
                    != (observation.status == ReviewStatus::Accepted)
                || observation.details["treeStopped"] != true
                || observation.details["streamsClosed"] != true
                || observation.details["readOnlyConfirmed"] != true
                || observation.details["terminalSucceeded"] != true
            {
                return Err(StoreError::InvalidState);
            }
        } else if observation.details.get("output").is_some() {
            return Err(StoreError::InvalidState);
        }
    }
    if record.status() == ReviewStatus::SetAside {
        let details = &record
            .observations
            .last()
            .ok_or(StoreError::InvalidState)?
            .details;
        let guardian = record
            .observations
            .iter()
            .find_map(|o| o.details.get("guardian"))
            .ok_or(StoreError::InvalidState)?;
        let guardian: crate::recovery_guardian::GuardianBinding =
            serde_json::from_value(guardian.clone()).map_err(|_| StoreError::InvalidState)?;
        if !crate::recovery_guardian::valid_proof(&guardian, &details["localClosureProof"])
            || LocalRef::new(details["authorizationRef"].as_str().unwrap_or("")).is_err()
            || details["resumeAutomatically"] != false
            || details["remoteOutcomeUnchanged"] != true
        {
            return Err(StoreError::InvalidState);
        }
    }
    Ok(record)
}

impl Store {
    pub fn desktop_worker(&self, id: &str) -> StoreResult<Value> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let worker = super::daemon::historical_worker(&self.connection, id)?
                .ok_or(StoreError::InvalidState)?;
            let (connection,revision):(String,i64)=self.connection.query_row("SELECT s.connection_id,s.connection_revision FROM attempts a JOIN sessions s ON s.id=a.session_id WHERE a.id=?1 AND a.run_id=?2 AND a.task_id=?3",params![worker.intent.attempt_id,worker.intent.run_id,worker.intent.task_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            ConnectionId::new(&connection)?;
            if revision <= 0 {
                return Err(StoreError::InvalidState);
            }
            // A later observation may identify the published exact artifact among
            // retained captures. Never guess between multiple unreferenced bodies.
            let referenced = worker
                .observations
                .iter()
                .rev()
                .find_map(|o| o.details.get("artifactRevision").filter(|v| !v.is_null()));
            let referenced = referenced
                .map(|v| {
                    v.as_str()
                        .filter(|r| is_hash(r))
                        .ok_or(StoreError::InvalidState)
                })
                .transpose()?;
            let mut query=self.connection.prepare("SELECT id,revision FROM captured_artifacts WHERE attempt_id=?1 AND (?2 IS NULL OR revision=?2) ORDER BY id,revision LIMIT 2")?;
            let artifacts = query
                .query_map(params![worker.intent.attempt_id, referenced], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if artifacts.len() > 1 {
                return Err(StoreError::Configuration);
            }
            if referenced.is_some() && artifacts.is_empty() {
                return Err(StoreError::InvalidState);
            }
            let artifact = artifacts
                .first()
                .map(|(id, revision)| historical_artifact(&self.connection, id, revision))
                .transpose()?
                .flatten();
            if artifact.as_ref().is_some_and(|a| {
                a.run_id != worker.intent.run_id
                    || a.task_id != worker.intent.task_id
                    || a.base_oid != worker.intent.base_oid
            }) {
                return Err(StoreError::InvalidState);
            }
            let mut query=self.connection.prepare("SELECT id,run_id,task_id,intent FROM independent_reviews WHERE run_id=?1 AND task_id=?2 ORDER BY id LIMIT 17")?;
            let mut reviews = vec![];
            for row in
                query.query_map(params![worker.intent.run_id, worker.intent.task_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
            {
                let (id, run, task, body) = row?;
                if reviews.len() >= 16 {
                    return Err(StoreError::Configuration);
                }
                reviews
                    .push(historical_review(&self.connection, &id, &run, &task, &body)?.summary());
            }
            let artifact=artifact.map(|a|json!({"id":a.id,"revision":a.revision,"baseOid":a.base_oid,"patchSha256":a.patch_sha256,"preservation":a.preservation.as_ref().map(|p|json!({"profile":"opaque-base-v2","baseFiles":p.entries.len(),"baseBytes":p.entries.iter().map(|e|e.size).sum::<u64>(),"editablePaths":p.editable_paths})),"files":a.files.iter().map(|f|json!({"path":f.path,"untracked":f.untracked,"baseHash":f.base.as_ref().map(|c|&c.sha256),"worktreeHash":f.worktree.as_ref().map(|c|&c.sha256)})).collect::<Vec<_>>(),"exclusions":a.exclusions,"immutable":true}));
            let integration_history = self.integration_worker_history(id)?;
            Ok(
                json!({"worker":{"intent":worker.intent,"status":worker.status()},"connectionId":connection,"connectionRevision":revision,"summary":worker.summary(),"artifact":artifact,"reviews":reviews,"integrations":integration_history.0,"integrationCount":integration_history.1,"integrationsMore":integration_history.1>16,"historical":true,"currentVerification":"not-revalidated","delivered":false}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }

    pub fn desktop_artifact_text(
        &self,
        id: &str,
        revision: &str,
        path: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> StoreResult<Value> {
        if !(1..=16_384).contains(&limit) {
            return Err(StoreError::Configuration);
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let artifact = historical_artifact(&self.connection, id, revision)?
                .ok_or(StoreError::InvalidState)?;
            let text = if let Some(path) = path {
                artifact
                    .files
                    .iter()
                    .find(|f| f.path == path)
                    .ok_or(StoreError::InvalidState)?
                    .worktree
                    .as_ref()
                    .map_or("", |c| c.content.as_str())
            } else {
                artifact.patch.as_str()
            };
            if offset > text.len() || !text.is_char_boundary(offset) {
                return Err(StoreError::InvalidState);
            }
            let mut end = offset.saturating_add(limit).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            Ok(
                json!({"artifactId":id,"revision":revision,"path":path,"text":&text[offset..end],"nextOffset":end,"more":end<text.len(),"immutable":true}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }

    pub fn desktop_affected_workers(&self, canonical_path: &str) -> StoreResult<Value> {
        let path = std::path::Path::new(canonical_path);
        if !path.is_absolute() || canonical_path.len() > 4096 {
            return Err(StoreError::InvalidPath);
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let mut query=self.connection.prepare("SELECT g.operation_id,g.project_id,g.repository_id,g.run_id,g.task_id,g.attempt_id,g.path,g.branch,g.main_repository,g.intent,w.id FROM git_worktrees g JOIN manual_workers w ON w.attempt_id=g.attempt_id AND w.run_id=g.run_id AND w.task_id=g.task_id WHERE substr(?1,1,length(g.path))=g.path ORDER BY g.operation_id LIMIT 129")?;
            let mut workers = vec![];
            let mut count = 0;
            for row in query.query_map([canonical_path], |r| {
                (0..11)
                    .map(|i| r.get::<_, String>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })? {
                let row = row?;
                count += 1;
                if count > 128 {
                    return Err(StoreError::Configuration);
                }
                let intent: WorktreeIntent =
                    decode(&row[9], 65_536 + crate::git_records::MAX_PRESERVATION_BYTES)?;
                let columns = [
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
                if !intent.validate() || row[..9].iter().zip(columns).any(|(a, b)| a != b) {
                    return Err(StoreError::InvalidState);
                }
                if !path.starts_with(&intent.path) {
                    continue;
                }
                let worker = super::daemon::historical_worker(&self.connection, &row[10])?
                    .ok_or(StoreError::InvalidState)?;
                if worker.intent.attempt_id != intent.attempt_id
                    || worker.intent.project_id != intent.project_id
                    || worker.intent.repository_id != intent.repository_id
                {
                    return Err(StoreError::InvalidState);
                }
                workers.push(worker.intent.id);
            }
            Ok(
                json!({"workerIds":workers,"integrationIds":self.integration_affected_by_file(canonical_path)?,"grantsAccess":false,"historical":true}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub fn daemon_review_exists(&self, id: &str) -> StoreResult<bool> {
        LocalRef::new(id)?;
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM independent_reviews WHERE id=?)",
            [id],
            |r| r.get(0),
        )?)
    }
    pub fn desktop_snapshot(&self) -> StoreResult<Value> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let revision: i64 = self.connection.query_row(
                "SELECT revision FROM store_meta WHERE singleton=1",
                [],
                |r| r.get(0),
            )?;
            let control = self.daemon_state()?;

            let records: Option<String> = self
                .connection
                .query_row(
                    "SELECT checkpoint FROM document_records WHERE singleton=1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            let records = match records {
                None => DocumentRecords::default(),
                Some(body) => DocumentRecords::restore(
                    &serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?,
                )
                .map_err(StoreError::Records)?,
            };
            let latest = |key: &str| {
                records
                    .definitions()
                    .iter()
                    .filter(|d| d.kind == DefinitionKind::Configuration && d.id == key)
                    .max_by_key(|d| d.version)
                    .map(|d| &d.body)
            };
            let settings: Settings = latest("desktop-settings")
                .map(|s| serde_json::from_value(s.clone()))
                .transpose()
                .map_err(|_| StoreError::InvalidState)?
                .unwrap_or_default();
            if !settings.validate() {
                return Err(StoreError::InvalidState);
            }
            let mut projects = Vec::new();
            let mut query=self.connection.prepare("SELECT p.id,p.repository_id,p.checkpoint,r.location FROM projects p JOIN repositories r ON r.id=p.repository_id ORDER BY p.id LIMIT 129")?;
            for row in query.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (id, repo, body, location) = row?;
                let p = Project::restore_checkpoint(
                    &serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?,
                )?;
                if p.id().as_str() != id
                    || p.repository().id().as_str() != repo
                    || p.repository().location().as_str() != location
                {
                    return Err(StoreError::InvalidState);
                }
                let key = format!("desktop-project-{id}");
                let meta = latest(&key);
                projects.push(json!({"id":id,"repositoryId":repo,"name":meta.and_then(|m|m["name"].as_str()).unwrap_or(&id),"directory":meta.and_then(|m|m["directory"].as_str())}));
            }
            let mut conversations = Vec::new();
            let mut query=self.connection.prepare("SELECT c.id,c.project_id,c.checkpoint,substr(m.content,1,80) FROM conversations c JOIN messages m ON m.conversation_id=c.id AND m.ordinal=1 ORDER BY c.id LIMIT 129")?;
            for row in query.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (id, project, body, title) = row?;
                ConversationId::new(&id)?;
                if let Some(p) = &project {
                    ProjectId::new(p)?;
                }
                safe(&title)?;
                let body: Value =
                    serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
                if body["id"] != id || body["messageCount"].as_u64().is_none_or(|n| n == 0) {
                    return Err(StoreError::InvalidState);
                }
                conversations.push(json!({"id":id,"projectId":project,"title":title,"messageCount":body["messageCount"]}));
            }
            if projects.len() > 128 || conversations.len() > 128 {
                return Err(StoreError::Configuration);
            }
            let mut query = self
                .connection
                .prepare("SELECT checkpoint FROM connections ORDER BY id,revision")?;
            let mut connections = std::collections::BTreeMap::new();
            for row in query.query_map([], |r| r.get::<_, String>(0))? {
                let body = row?;
                let data: Value =
                    serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
                let connection = Connection::restore_checkpoint(&data)?;
                connections.insert(
                    connection.id().as_str().to_owned(),
                    connection.descriptor().as_json().clone(),
                );
            }
            Ok(
                json!({"revision":revision,"projects":projects,"conversations":conversations,"settings":settings,"connections":connections.values().collect::<Vec<_>>(),"eventCursor":control.cursor,"admissionsSuspended":control.suspended}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub fn desktop_conversation(&self, id: &str, after: u64, limit: usize) -> StoreResult<Value> {
        ConversationId::new(id)?;
        if !(1..=16).contains(&limit) || after > i64::MAX as u64 {
            return Err(StoreError::InvalidState);
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let (project, checkpoint): (Option<String>, String) = self.connection.query_row(
                "SELECT project_id,checkpoint FROM conversations WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if let Some(p) = &project {
                ProjectId::new(p)?;
            }
            let checkpoint: Value =
                serde_json::from_str(&checkpoint).map_err(|_| StoreError::InvalidState)?;
            let count = checkpoint["messageCount"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(StoreError::InvalidState)?;
            if checkpoint["id"] != id || after > count {
                return Err(StoreError::InvalidState);
            }
            let mut query=self.connection.prepare("SELECT ordinal,role,content,project_at_send FROM messages WHERE conversation_id=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?3")?;
            let mut messages = Vec::new();
            for row in query.query_map(rusqlite::params![id, after as i64, limit as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })? {
                let (ordinal, role, text, project) = row?;
                if ordinal as u64 != after + messages.len() as u64 + 1
                    || !["User", "Orchestrator"].contains(&role.as_str())
                    || text.len() > 32_768
                    || text.trim().is_empty()
                {
                    return Err(StoreError::InvalidState);
                }
                safe(&text)?;
                if let Some(p) = &project {
                    ProjectId::new(p)?;
                }
                messages.push(json!({"id":format!("{id}-{ordinal}"),"ordinal":ordinal,"role":if role=="User"{"user"}else{"orchestrator"},"text":text,"projectAtSend":project}));
            }
            let next = after + messages.len() as u64;
            if next < count && messages.len() != limit {
                return Err(StoreError::InvalidState);
            }
            let draft: Option<String> = self
                .connection
                .query_row(
                    "SELECT content FROM saved_drafts WHERE conversation_id=?",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(text) = &draft {
                safe(text)?;
                if text.len() > 32_768 {
                    return Err(StoreError::InvalidState);
                }
            }
            let mut work = Vec::new();
            let mut query=self.connection.prepare("SELECT w.run_id,w.task_id,w.attempt_id,w.body,m.id FROM conversation_work w LEFT JOIN manual_workers m ON m.attempt_id=w.attempt_id AND m.run_id=w.run_id AND m.task_id=w.task_id WHERE w.conversation_id=? ORDER BY w.ordinal LIMIT 129")?;
            for row in query.query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })? {
                let (run, task, attempt, body, worker) = row?;
                RunId::new(&run)?;
                if let Some(t) = &task {
                    TaskId::new(t)?;
                }
                if let Some(a) = &attempt {
                    AttemptId::new(a)?;
                }
                let body: Value =
                    serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
                RevisionId::new(
                    body["resultRevision"]
                        .as_str()
                        .ok_or(StoreError::InvalidState)?,
                )?;
                work.push(json!({"runId":run,"taskId":task,"attemptId":attempt,"workerId":worker,"revision":body["resultRevision"]}));
            }
            if work.len() > 128 {
                return Err(StoreError::Configuration);
            }
            Ok(
                json!({"id":id,"projectId":project,"messages":messages,"draft":draft,"nextOrdinal":next,"more":next<count,"eventCursor":self.daemon_state()?.cursor,"work":work}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::{
        git_records::{CapturedFile, FileContent},
        worker_records::{WorkerIntent, WorkerObservation, WorkerStatus},
    };

    // Isolated historical-query fixtures deliberately omit the full aggregate
    // checkpoint tables. They cannot serve as mutation/admission evidence.
    fn fixture() -> (Store, CapturedArtifact, WorktreeIntent) {
        let connection = SqlConnection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE runs(id TEXT,project_id TEXT,repository_id TEXT);CREATE TABLE projects(id TEXT,repository_id TEXT);CREATE TABLE repositories(id TEXT);CREATE TABLE tasks(run_id TEXT,id TEXT);CREATE TABLE attempts(id TEXT,run_id TEXT,task_id TEXT,session_id TEXT);CREATE TABLE sessions(id TEXT,connection_id TEXT,connection_revision INTEGER,adapter TEXT);CREATE TABLE connections(id TEXT,revision INTEGER,adapter TEXT);CREATE TABLE manual_workers(id TEXT,run_id TEXT,task_id TEXT,attempt_id TEXT,intent TEXT);CREATE TABLE worker_observations(worker_id TEXT,ordinal INTEGER,body TEXT,digest TEXT);CREATE TABLE git_worktrees(operation_id TEXT,project_id TEXT,repository_id TEXT,run_id TEXT,task_id TEXT,attempt_id TEXT,path TEXT,branch TEXT,main_repository TEXT,intent TEXT);CREATE TABLE captured_artifacts(id TEXT,revision TEXT,operation_id TEXT,attempt_id TEXT,base_oid TEXT,patch_sha256 TEXT,body TEXT);CREATE TABLE independent_reviews(id TEXT,run_id TEXT,task_id TEXT,intent TEXT);CREATE TABLE review_observations(review_id TEXT,ordinal INTEGER,body TEXT,digest TEXT);CREATE TABLE serial_integrations(id TEXT,run_id TEXT,task_id TEXT,intent TEXT);CREATE TABLE integration_observations(integration_id TEXT,ordinal INTEGER,body TEXT,digest TEXT);INSERT INTO runs VALUES('run','project','repo');INSERT INTO projects VALUES('project','repo');INSERT INTO repositories VALUES('repo');INSERT INTO tasks VALUES('run','task');INSERT INTO attempts VALUES('attempt','run','task','source-session');INSERT INTO sessions VALUES('source-session','account',1,'fixture'),('review-session','account',1,'fixture');INSERT INTO connections VALUES('account',1,'fixture');").unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap();
        let intent = WorktreeIntent {
            preservation: None,
            operation_id: "operation".into(),
            project_id: "project".into(),
            repository_id: "repo".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: "b".repeat(40),
            domain_base: "b".repeat(40),
            worktree_ref: "owned-ref".into(),
            authorized_root: root.join("worktrees").to_string_lossy().into(),
            path: root
                .join("worktrees")
                .join("owned")
                .to_string_lossy()
                .into(),
            main_repository: root.join("repository").to_string_lossy().into(),
            branch: "orchestrix/owned".into(),
            common_git_dir: root
                .join("repository")
                .join(".git")
                .to_string_lossy()
                .into(),
            ownership_token: "c".repeat(64),
        };
        assert!(intent.validate());
        let worker = WorkerIntent {
            editable_paths: None,
            id: "worker".into(),
            request_hash: "a".repeat(64),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            project_id: "project".into(),
            repository_id: "repo".into(),
            base_oid: intent.base_oid.clone(),
            policy: json!({}),
            context: json!({}),
            authorized_checks: json!([]),
            launch_protocol: None,
            correction: None,
        };
        assert!(worker.validate());
        connection
            .execute(
                "INSERT INTO manual_workers VALUES('worker','run','task','attempt',?)",
                [serde_json::to_string(&worker).unwrap()],
            )
            .unwrap();
        let observation = serde_json::to_string(&WorkerObservation {
            status: WorkerStatus::Preparing,
            details: json!({"checks":[{"id":"historical-check","result":"passed","exit_code":0}]}),
        })
        .unwrap();
        connection
            .execute(
                "INSERT INTO worker_observations VALUES('worker',1,?,?)",
                params![observation, content_hash(observation.as_bytes())],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO git_worktrees VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    intent.operation_id,
                    intent.project_id,
                    intent.repository_id,
                    intent.run_id,
                    intent.task_id,
                    intent.attempt_id,
                    intent.path,
                    intent.branch,
                    intent.main_repository,
                    serde_json::to_string(&intent).unwrap()
                ],
            )
            .unwrap();
        let mut artifact = CapturedArtifact {
            preservation: None,
            id: "artifact".into(),
            revision: String::new(),
            operation_id: intent.operation_id.clone(),
            repository_id: intent.repository_id.clone(),
            run_id: intent.run_id.clone(),
            task_id: intent.task_id.clone(),
            attempt_id: intent.attempt_id.clone(),
            base_oid: intent.base_oid.clone(),
            origin: intent.path.clone(),
            files: vec![CapturedFile {
                path: "notes.md".into(),
                base: None,
                index: None,
                worktree: Some(FileContent::new("café\n".into(), "100644".into()).unwrap()),
                untracked: true,
            }],
            exclusions: vec![],
            patch: "bounded patch\n".into(),
            patch_sha256: content_hash(b"bounded patch\n"),
        };
        artifact.revision = artifact.content_revision();
        assert!(artifact.validate());
        connection
            .execute(
                "INSERT INTO captured_artifacts VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    artifact.id,
                    artifact.revision,
                    artifact.operation_id,
                    artifact.attempt_id,
                    artifact.base_oid,
                    artifact.patch_sha256,
                    serde_json::to_string(&artifact).unwrap()
                ],
            )
            .unwrap();
        let review = ReviewIntent {
            id: "review".into(),
            request_hash: "d".repeat(64),
            worker_id: "worker".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            source_attempt_id: "attempt".into(),
            artifact_id: artifact.id.clone(),
            artifact_revision: artifact.revision.clone(),
            base_oid: artifact.base_oid.clone(),
            session_id: "review-session".into(),
            attempt_id: "review-attempt".into(),
            authorization_ref: "review-consent".into(),
            budget: crate::review_records::ReviewBudget {
                max_corrections: 1,
                max_reviews: 2,
                max_elapsed_ms: 3000,
            },
            started_ms: 1,
            deadline_ms: 3001,
            policy: json!({}),
            context: json!({}),
            runtime_request: json!({"sessionId":"review-session","attemptId":"review-attempt","requested":{"sandbox":"read-only"},"billingMode":"subscription-only"}),
            checks: json!([]),
        };
        assert!(review.validate());
        connection
            .execute(
                "INSERT INTO independent_reviews VALUES('review','run','task',?)",
                [serde_json::to_string(&review).unwrap()],
            )
            .unwrap();
        let observation = serde_json::to_string(&ReviewObservation {
            status: ReviewStatus::Unavailable,
            details: json!({"reason":"quota unavailable"}),
        })
        .unwrap();
        connection
            .execute(
                "INSERT INTO review_observations VALUES('review',1,?,?)",
                params![observation, content_hash(observation.as_bytes())],
            )
            .unwrap();
        (
            Store {
                connection,
                path: root.join("isolated-desktop-projection.sqlite"),
            },
            artifact,
            intent,
        )
    }

    #[test]
    fn selected_historical_rows_preserve_dto_and_utf8_pages_without_full_restore() {
        let (store, artifact, intent) = fixture();
        assert!(store.load().is_err());
        let view = store.desktop_worker("worker").unwrap();
        assert_eq!(view["summary"]["details"]["checks"][0]["exit_code"], 0);
        assert_eq!(view["connectionId"], "account");
        assert_eq!(view["artifact"]["revision"], artifact.revision);
        assert!(view["artifact"]["files"][0].get("content").is_none());
        assert_eq!(view["reviews"][0]["status"], "unavailable");
        assert_eq!(view["reviews"][0]["result"], Value::Null);
        assert_eq!(view["currentVerification"], "not-revalidated");
        assert_eq!(view["delivered"], false);
        let page = store
            .desktop_artifact_text(&artifact.id, &artifact.revision, Some("notes.md"), 0, 4)
            .unwrap();
        assert_eq!(page["text"], "caf");
        assert_eq!(page["nextOffset"], 3);
        assert_eq!(
            store
                .desktop_artifact_text(&artifact.id, &artifact.revision, Some("notes.md"), 3, 2)
                .unwrap()["text"],
            "é"
        );
        assert!(
            store
                .desktop_artifact_text(&artifact.id, &artifact.revision, Some("notes.md"), 4, 2)
                .is_err()
        );
        let child = std::path::Path::new(&intent.path).join("notes.md");
        assert_eq!(
            store
                .desktop_affected_workers(&child.to_string_lossy())
                .unwrap()["workerIds"],
            json!(["worker"])
        );
        assert_eq!(
            store
                .desktop_affected_workers(&format!("{}-other", intent.path))
                .unwrap()["workerIds"],
            json!([])
        );
        assert_eq!(
            store
                .desktop_affected_workers(&child.to_string_lossy())
                .unwrap()["grantsAccess"],
            false
        );
        // Irrelevant corrupt rows do not force replay of another aggregate.
        store
            .connection
            .execute(
                "INSERT INTO manual_workers VALUES('other','other','other','other','bad')",
                [],
            )
            .unwrap();
        assert!(store.desktop_worker("worker").is_ok());
    }

    #[test]
    fn multiple_captures_require_the_exact_observed_revision() {
        let (store, first, _) = fixture();
        let mut second = first.clone();
        second.id = "second-artifact".into();
        second.files[0].worktree =
            Some(FileContent::new("second exact version\n".into(), "100644".into()).unwrap());
        second.revision = second.content_revision();
        assert!(second.validate());
        store
            .connection
            .execute(
                "INSERT INTO captured_artifacts VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    second.id,
                    second.revision,
                    second.operation_id,
                    second.attempt_id,
                    second.base_oid,
                    second.patch_sha256,
                    serde_json::to_string(&second).unwrap()
                ],
            )
            .unwrap();
        assert_eq!(
            store.desktop_worker("worker").unwrap_err(),
            StoreError::Configuration
        );
        let body = serde_json::to_string(&WorkerObservation {
            status: WorkerStatus::Preparing,
            details: json!({"artifactRevision":second.revision}),
        })
        .unwrap();
        store
            .connection
            .execute(
                "UPDATE worker_observations SET body=?1,digest=?2",
                params![body, content_hash(body.as_bytes())],
            )
            .unwrap();
        let view = store.desktop_worker("worker").unwrap();
        assert_eq!(view["artifact"]["id"], second.id);
        assert_eq!(view["artifact"]["revision"], second.revision);
        assert_eq!(
            store
                .desktop_artifact_text(&first.id, &first.revision, Some("notes.md"), 0, 16)
                .unwrap()["text"],
            "café\n"
        );
    }

    #[test]
    fn historical_projections_reject_tampered_digests_columns_contents_and_ownership() {
        for fault in [
            "worker-digest",
            "review-digest",
            "review-ordinal",
            "review-ownership",
            "review-revision",
            "review-adapter",
            "artifact-column",
            "artifact-content",
            "git-owner",
            "connection-owner",
        ] {
            let (store, artifact, intent) = fixture();
            match fault {
                "worker-digest" => {
                    store
                        .connection
                        .execute("UPDATE worker_observations SET digest='wrong'", [])
                        .unwrap();
                }
                "review-digest" => {
                    store
                        .connection
                        .execute("UPDATE review_observations SET digest='wrong'", [])
                        .unwrap();
                }
                "review-ordinal" => {
                    store
                        .connection
                        .execute("UPDATE review_observations SET ordinal=2", [])
                        .unwrap();
                }
                "review-ownership" => {
                    store
                        .connection
                        .execute(
                            "UPDATE sessions SET connection_id='other' WHERE id='review-session'",
                            [],
                        )
                        .unwrap();
                }
                "review-revision" => {
                    store
                        .connection
                        .execute(
                            "UPDATE sessions SET connection_revision=2 WHERE id='review-session'",
                            [],
                        )
                        .unwrap();
                }
                "review-adapter" => {
                    store
                        .connection
                        .execute(
                            "UPDATE sessions SET adapter='different' WHERE id='review-session'",
                            [],
                        )
                        .unwrap();
                }
                "artifact-column" => {
                    store
                        .connection
                        .execute(
                            "UPDATE captured_artifacts SET patch_sha256=?",
                            ["e".repeat(64)],
                        )
                        .unwrap();
                }
                "artifact-content" => {
                    store.connection.execute("UPDATE captured_artifacts SET body=json_set(body,'$.files[0].worktree.content','tampered')",[]).unwrap();
                }
                "git-owner" => {
                    store
                        .connection
                        .execute(
                            "UPDATE git_worktrees SET intent=json_set(intent,'$.path','different')",
                            [],
                        )
                        .unwrap();
                }
                _ => {
                    store
                        .connection
                        .execute("DELETE FROM connections", [])
                        .unwrap();
                }
            }
            assert!(store.desktop_worker("worker").is_err(), "fault {fault}");
            if [
                "artifact-column",
                "artifact-content",
                "git-owner",
                "connection-owner",
            ]
            .contains(&fault)
            {
                assert!(
                    store
                        .desktop_artifact_text(&artifact.id, &artifact.revision, None, 0, 16)
                        .is_err()
                );
            }
            if ["git-owner", "connection-owner", "worker-digest"].contains(&fault) {
                assert!(
                    store
                        .desktop_affected_workers(
                            &std::path::Path::new(&intent.path)
                                .join("notes.md")
                                .to_string_lossy()
                        )
                        .is_err()
                );
            }
        }
    }
}
