//! Short transactions for daemon controls, durable command deduplication and an event cursor.
use super::{Store, StoreError, StoreResult};
use crate::worker_records::{WorkerIntent, WorkerObservation, WorkerRecord};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const MIGRATION_V5: &str = r#"
CREATE TABLE daemon_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1), identity TEXT NOT NULL, suspended INTEGER NOT NULL CHECK(suspended IN(0,1)), generation INTEGER NOT NULL CHECK(generation>=0));
INSERT INTO daemon_meta VALUES(1,lower(hex(randomblob(16))),1,0);
CREATE TABLE daemon_commands(id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, kind TEXT NOT NULL, worker_id TEXT, response TEXT NOT NULL);
CREATE TABLE daemon_completions(id TEXT PRIMARY KEY REFERENCES daemon_commands(id), outcome TEXT NOT NULL);
CREATE TABLE daemon_events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, body TEXT NOT NULL);
CREATE TRIGGER daemon_commands_no_update BEFORE UPDATE ON daemon_commands BEGIN SELECT RAISE(ABORT,'immutable daemon command'); END;
CREATE TRIGGER daemon_commands_no_delete BEFORE DELETE ON daemon_commands BEGIN SELECT RAISE(ABORT,'retained daemon command'); END;
CREATE TRIGGER daemon_completions_no_update BEFORE UPDATE ON daemon_completions BEGIN SELECT RAISE(ABORT,'immutable daemon completion'); END;
CREATE TRIGGER daemon_completions_no_delete BEFORE DELETE ON daemon_completions BEGIN SELECT RAISE(ABORT,'retained daemon completion'); END;
CREATE TRIGGER daemon_events_no_update BEFORE UPDATE ON daemon_events BEGIN SELECT RAISE(ABORT,'append-only daemon event'); END;
CREATE TRIGGER daemon_events_no_delete BEFORE DELETE ON daemon_events BEGIN SELECT RAISE(ABORT,'retained daemon event'); END;
CREATE TRIGGER domain_event_daemon AFTER INSERT ON events BEGIN INSERT INTO daemon_events(kind,body) VALUES('domain-change',json_object('revision',new.revision,'domainSequence',new.sequence,'kind',new.kind,'runId',new.run_id,'taskId',new.task_id,'attemptId',new.attempt_id)); END;
"#;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DaemonCursor {
    pub database: String,
    pub sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonEvent {
    pub sequence: u64,
    pub kind: String,
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonCommandReceipt {
    pub replayed: bool,
    pub response: Value,
    pub outcome: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonState {
    pub suspended: bool,
    pub generation: u64,
    pub cursor: DaemonCursor,
}

fn reference(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
}
impl Store {
    /// Existence only, never admission eligibility or evidence of dispatch.
    pub fn daemon_worker_exists(&self, id: &str) -> StoreResult<bool> {
        crate::domain::LocalRef::new(id)?;
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM manual_workers WHERE id=?)",
            [id],
            |r| r.get(0),
        )?)
    }
    /// Historical worker facts with local integrity/ownership validation. This
    /// does not certify the aggregate domain, artifact, authentication or quota.
    /// Verified/SetAside are stored status names here, never admission authority
    /// or new verification evidence; their semantic gates still use Store::load.
    #[cfg(test)]
    fn daemon_worker_history(&self, id: &str) -> StoreResult<Option<WorkerRecord>> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = historical_worker(&self.connection, id);
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub fn daemon_state(&self) -> StoreResult<DaemonState> {
        let (identity, suspended, generation): (String, bool, i64) = self.connection.query_row(
            "SELECT identity,suspended,generation FROM daemon_meta WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let sequence = self.connection.query_row(
            "SELECT coalesce(max(sequence),0) FROM daemon_events",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64;
        Ok(DaemonState {
            suspended,
            generation: generation as u64,
            cursor: DaemonCursor {
                database: identity,
                sequence,
            },
        })
    }
    /// Restart always closes admission before reconciliation; no pending command is dispatched.
    pub fn daemon_start(&mut self) -> StoreResult<DaemonState> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE daemon_meta SET suspended=1,generation=generation+1 WHERE singleton=1",
            [],
        )?;
        tx.execute("INSERT INTO daemon_events(kind,body) VALUES('daemon-started','{\"admissionsSuspended\":true,\"resumeAutomatically\":false}')",[])?;
        tx.commit()?;
        self.daemon_state()
    }
    pub fn daemon_receipt(
        &self,
        id: &str,
        fingerprint: &str,
    ) -> StoreResult<Option<DaemonCommandReceipt>> {
        let old:Option<(String,String,Option<String>)>=self.connection.query_row("SELECT c.fingerprint,c.response,f.outcome FROM daemon_commands c LEFT JOIN daemon_completions f ON f.id=c.id WHERE c.id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        old.map(|(hash, response, outcome)| {
            if hash != fingerprint {
                return Err(StoreError::IdempotencyConflict);
            }
            Ok(DaemonCommandReceipt {
                replayed: true,
                response: serde_json::from_str(&response).map_err(|_| StoreError::InvalidState)?,
                outcome: outcome
                    .map(|b| serde_json::from_str(&b))
                    .transpose()
                    .map_err(|_| StoreError::InvalidState)?,
            })
        })
        .transpose()
    }
    /// Caller validates authorization/eligibility first. Commit intent before any effect.
    pub fn daemon_begin(
        &mut self,
        id: &str,
        fingerprint: &str,
        kind: &str,
        worker_id: Option<&str>,
        suspended: Option<bool>,
    ) -> StoreResult<DaemonCommandReceipt> {
        if !reference(id)
            || !reference(kind)
            || worker_id.is_some_and(|v| !reference(v))
            || fingerprint.len() != 64
            || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StoreError::InvalidState);
        }
        if let Some(old) = self.daemon_receipt(id, fingerprint)? {
            return Ok(old);
        }
        let response = json!({"accepted":true,"commandId":id,"kind":kind,"workerId":worker_id,"resumeAutomatically":false});
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO daemon_commands(id,fingerprint,kind,worker_id,response) VALUES(?,?,?,?,?)",
            params![id, fingerprint, kind, worker_id, response.to_string()],
        )?;
        if let Some(value) = suspended {
            tx.execute(
                "UPDATE daemon_meta SET suspended=? WHERE singleton=1",
                [value],
            )?;
        }
        tx.execute(
            "INSERT INTO daemon_events(kind,body) VALUES('command-accepted',?)",
            [response.to_string()],
        )?;
        tx.commit()?;
        Ok(DaemonCommandReceipt {
            replayed: false,
            response,
            outcome: None,
        })
    }
    pub fn daemon_finish(&mut self, id: &str, outcome: &Value) -> StoreResult<()> {
        if !reference(id)
            || serde_json::to_vec(outcome).is_err()
            || !crate::worker_records::exact_safe(outcome, 65_536)
            || ["state", "reason"].iter().any(|key| {
                !outcome[*key].is_null() && !outcome[*key].as_str().is_some_and(|s| s.len() <= 128)
            })
        {
            return Err(StoreError::InvalidState);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT outcome FROM daemon_completions WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if existing
            .as_ref()
            .is_some_and(|body| serde_json::from_str::<Value>(body).ok().as_ref() != Some(outcome))
        {
            return Err(StoreError::IdempotencyConflict);
        }
        if existing.is_none() {
            tx.execute(
                "INSERT INTO daemon_completions(id,outcome) VALUES(?,?)",
                params![id, outcome.to_string()],
            )?;
            tx.execute(
                "INSERT INTO daemon_events(kind,body) VALUES('command-finished',?)",
                [json!({"commandId":id,"state":outcome["state"],"reason":outcome["reason"],"workerId":outcome["worker"]["workerId"].as_str().or(outcome["workerId"].as_str())}).to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn daemon_pending(&self) -> StoreResult<Vec<(String, String, Option<String>)>> {
        let mut stmt=self.connection.prepare("SELECT c.id,c.kind,c.worker_id FROM daemon_commands c LEFT JOIN daemon_completions f ON c.id=f.id WHERE f.id IS NULL ORDER BY c.rowid LIMIT 1025")?;
        let items = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        if items.len() > 1024 {
            return Err(StoreError::InvalidState);
        }
        Ok(items)
    }
    /// No retention deletes in V1. Wrong database, future, or discontinuous cursor requires resync.
    pub fn daemon_events(
        &self,
        cursor: &DaemonCursor,
        limit: usize,
    ) -> StoreResult<Option<(Vec<DaemonEvent>, DaemonCursor, bool)>> {
        if limit == 0 || limit > 128 {
            return Err(StoreError::InvalidState);
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let state = self.daemon_state()?;
            if cursor.database != state.cursor.database || cursor.sequence > state.cursor.sequence {
                return Ok(None);
            }
            if cursor.sequence != 0
                && !self.connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM daemon_events WHERE sequence=?)",
                    [cursor.sequence as i64],
                    |r| r.get::<_, bool>(0),
                )?
            {
                return Ok(None);
            }
            let mut stmt=self.connection.prepare("SELECT sequence,kind,body FROM daemon_events WHERE sequence>? ORDER BY sequence LIMIT ?")?;
            let rows = stmt.query_map(params![cursor.sequence as i64, limit as i64], |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            let mut events = Vec::new();
            let mut last = cursor.sequence;
            for row in rows {
                let (sequence, kind, body) = row?;
                if sequence != last + 1 {
                    return Ok(None);
                }
                last = sequence;
                events.push(DaemonEvent {
                    sequence,
                    kind,
                    body: serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?,
                });
            }
            Ok(Some((
                events,
                DaemonCursor {
                    database: state.cursor.database,
                    sequence: last,
                },
                last < state.cursor.sequence,
            )))
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    /// Compact historical projection and matching high-water mark in one read
    /// snapshot. Full aggregate validation remains on open, writes and decisions.
    pub fn daemon_snapshot(&self) -> StoreResult<Value> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let meta = self.daemon_state()?;
            let (workers, next) = worker_page(&self.connection, None, 128)?;
            let count: u64 = u64::try_from(self.connection.query_row(
                "SELECT count(*) FROM manual_workers",
                [],
                |r| r.get::<_, i64>(0),
            )?)
            .map_err(|_| StoreError::InvalidState)?;
            Ok(
                json!({"state":meta,"workers":workers,"workerCount":count,"nextWorker":next,"workerFactsHistorical":true,"artifactRevalidation":"startup-reconciliation-or-standalone-status-after-daemon-stops","capacity":{"subscriptionQuota":"unknown","apiCredits":"not-used","source":"not-exposed-by-current-runtime","inferenceUsed":false},"resumeAutomatically":false}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub fn daemon_workers(
        &self,
        cursor: &DaemonCursor,
        after: Option<&str>,
        limit: usize,
    ) -> StoreResult<Value> {
        if limit == 0 || limit > 128 {
            return Err(StoreError::InvalidState);
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let state = self.daemon_state()?;
            if &state.cursor != cursor {
                return Ok(json!({"resyncRequired":true,"action":"request-snapshot"}));
            }
            if let Some(id) = after
                && !self.daemon_worker_exists(id)?
            {
                return Ok(json!({"resyncRequired":true,"action":"request-snapshot"}));
            }
            let (workers, next) = worker_page(&self.connection, after, limit)?;
            Ok(
                json!({"cursor":state.cursor,"workers":workers,"nextWorker":next,"resyncRequired":false,"historical":true}),
            )
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
}

fn historical_worker(
    connection: &rusqlite::Connection,
    id: &str,
) -> StoreResult<Option<WorkerRecord>> {
    crate::domain::LocalRef::new(id)?;
    let row: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT run_id,task_id,attempt_id,intent FROM manual_workers WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((run, task, attempt, body)) = row else {
        return Ok(None);
    };
    if body.len() > 1_048_576 {
        return Err(StoreError::InvalidState);
    }
    let intent: WorkerIntent = serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
    if intent.id != id
        || intent.run_id != run
        || intent.task_id != task
        || intent.attempt_id != attempt
    {
        return Err(StoreError::InvalidState);
    }
    let owned:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM runs r JOIN projects p ON p.id=r.project_id JOIN repositories repo ON repo.id=r.repository_id JOIN tasks t ON t.run_id=r.id AND t.id=?2 JOIN attempts a ON a.id=?3 AND a.run_id=r.id AND a.task_id=t.id JOIN sessions s ON s.id=a.session_id JOIN connections c ON c.id=s.connection_id AND c.revision=s.connection_revision WHERE r.id=?1 AND r.project_id=?4 AND r.repository_id=?5 AND p.repository_id=r.repository_id AND c.adapter=s.adapter)",params![run,task,attempt,intent.project_id,intent.repository_id],|r|r.get(0))?;
    if !owned {
        return Err(StoreError::InvalidState);
    }
    let mut record = WorkerRecord {
        intent,
        observations: vec![],
    };
    let mut statement = connection.prepare(
        "SELECT ordinal,body,digest FROM worker_observations WHERE worker_id=? ORDER BY ordinal",
    )?;
    for row in statement.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (ordinal, body, digest) = row?;
        if record.observations.len() >= 64
            || ordinal != record.observations.len() as i64 + 1
            || body.len() > 65_536
            || crate::git_records::content_hash(body.as_bytes()) != digest
        {
            return Err(StoreError::InvalidState);
        }
        let observation: WorkerObservation =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        record.observations.push(observation);
    }
    if !record.validate()
        || record.observations.iter().any(|o| {
            o.details["recoveryVersion"] == 1
                && !crate::recovery::valid_local_proof(&record, &o.details)
        })
    {
        return Err(StoreError::InvalidState);
    }
    if record.status() == crate::worker_records::WorkerStatus::SetAside
        && record.observations.last().is_none_or(|o| {
            o.details["authorizationRef"]
                .as_str()
                .is_none_or(|s| crate::domain::LocalRef::new(s).is_err())
                || o.details["resumeAutomatically"] != false
                || o.details["remoteResultUnchanged"] != true
        })
    {
        return Err(StoreError::InvalidState);
    }
    Ok(Some(record))
}
fn worker_page(
    connection: &rusqlite::Connection,
    after: Option<&str>,
    limit: usize,
) -> StoreResult<(Vec<Value>, Option<Value>)> {
    let mut statement = connection.prepare(
        "SELECT id FROM manual_workers WHERE (?1 IS NULL OR id>?1) ORDER BY id LIMIT ?2",
    )?;
    let ids = statement
        .query_map(params![after, (limit + 1) as i64], |r| {
            r.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more = ids.len() > limit;
    let workers = ids
        .into_iter()
        .take(limit)
        .map(|id| {
            let w = historical_worker(connection, &id)?.ok_or(StoreError::InvalidState)?;
            Ok(json!({"workerId":w.intent.id,"status":w.status(),"resumeAutomatically":false}))
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let next = if more {
        workers.last().map(|w| w["workerId"].clone())
    } else {
        None
    };
    Ok((workers, next))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Isolated query/validation fixture, not a substitute for the real worker
    // acceptance tests. Other aggregate tables are intentionally absent.
    fn projection_fixture(count: usize) -> Store {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE runs(id TEXT,project_id TEXT,repository_id TEXT);CREATE TABLE projects(id TEXT,repository_id TEXT);CREATE TABLE repositories(id TEXT);CREATE TABLE tasks(run_id TEXT,id TEXT);CREATE TABLE attempts(id TEXT,run_id TEXT,task_id TEXT,session_id TEXT);CREATE TABLE sessions(id TEXT,connection_id TEXT,connection_revision INTEGER,adapter TEXT);CREATE TABLE connections(id TEXT,revision INTEGER,adapter TEXT);CREATE TABLE manual_workers(id TEXT PRIMARY KEY,run_id TEXT,task_id TEXT,attempt_id TEXT,intent TEXT);CREATE TABLE worker_observations(worker_id TEXT,ordinal INTEGER,body TEXT,digest TEXT);CREATE TABLE events(revision INTEGER,sequence INTEGER,kind TEXT,run_id TEXT,task_id TEXT,attempt_id TEXT);INSERT INTO runs VALUES('run','project','repository');INSERT INTO projects VALUES('project','repository');INSERT INTO repositories VALUES('repository');INSERT INTO sessions VALUES('session','account',1,'fixture');INSERT INTO connections VALUES('account',1,'fixture');").unwrap();
        connection.execute_batch(MIGRATION_V5).unwrap();
        for n in 0..count {
            let id = format!("worker-{n:03}");
            let task = format!("task-{n:03}");
            let attempt = format!("attempt-{n:03}");
            let intent = WorkerIntent {
                id: id.clone(),
                request_hash: "a".repeat(64),
                run_id: "run".into(),
                task_id: task.clone(),
                attempt_id: attempt.clone(),
                project_id: "project".into(),
                repository_id: "repository".into(),
                base_oid: "b".repeat(40),
                policy: json!({}),
                context: json!({}),
                authorized_checks: json!([]),
                launch_protocol: None,
            };
            assert!(intent.validate());
            connection
                .execute("INSERT INTO tasks VALUES('run',?)", [&task])
                .unwrap();
            connection
                .execute(
                    "INSERT INTO attempts VALUES(?,'run',?,'session')",
                    params![attempt, task],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO manual_workers VALUES(?,'run',?,?,?)",
                    params![id, task, attempt, serde_json::to_string(&intent).unwrap()],
                )
                .unwrap();
        }
        Store {
            connection,
            path: std::path::PathBuf::from("isolated-query-fixture"),
        }
    }
    #[test]
    fn compact_projection_rejects_adulterated_identity_history_hash_and_ownership() {
        for fault in [
            "identity",
            "ordinal",
            "hash",
            "transition",
            "ownership",
            "set-aside",
        ] {
            let store = projection_fixture(1);
            let body = json!({"status":"preparing","details":{}}).to_string();
            store
                .connection
                .execute(
                    "INSERT INTO worker_observations VALUES('worker-000',1,?,?)",
                    params![body, crate::git_records::content_hash(body.as_bytes())],
                )
                .unwrap();
            assert_eq!(
                store.daemon_snapshot().unwrap()["workers"][0]["status"],
                "preparing"
            );
            match fault {
                "identity" => {
                    store.connection.execute("UPDATE manual_workers SET intent=json_set(intent,'$.id','another-worker')",[]).unwrap();
                }
                "ordinal" => {
                    store
                        .connection
                        .execute("UPDATE worker_observations SET ordinal=2", [])
                        .unwrap();
                }
                "hash" => {
                    store
                        .connection
                        .execute("UPDATE worker_observations SET digest='wrong'", [])
                        .unwrap();
                }
                "transition" => {
                    let body = json!({"status":"verified","details":{}}).to_string();
                    store
                        .connection
                        .execute(
                            "UPDATE worker_observations SET body=?,digest=?",
                            params![body, crate::git_records::content_hash(body.as_bytes())],
                        )
                        .unwrap();
                }
                "ownership" => {
                    store
                        .connection
                        .execute("UPDATE attempts SET run_id='another-run'", [])
                        .unwrap();
                }
                "set-aside" => {
                    for (ordinal, status) in [(2, "blocked"), (3, "set-aside")] {
                        let body = json!({"status":status,"details":{}}).to_string();
                        store
                            .connection
                            .execute(
                                "INSERT INTO worker_observations VALUES('worker-000',?,?,?)",
                                params![
                                    ordinal,
                                    body,
                                    crate::git_records::content_hash(body.as_bytes())
                                ],
                            )
                            .unwrap();
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(
                store.daemon_snapshot(),
                Err(StoreError::InvalidState),
                "{fault}"
            );
            assert_eq!(
                store.daemon_worker_history("worker-000"),
                Err(StoreError::InvalidState),
                "{fault}"
            );
        }
    }
    #[test]
    fn compact_projection_paginates_without_loading_unrelated_aggregate_and_invalidates_old_cursor()
    {
        let mut store = projection_fixture(130);
        let snapshot = store.daemon_snapshot().unwrap();
        let cursor: DaemonCursor =
            serde_json::from_value(snapshot["state"]["cursor"].clone()).unwrap();
        assert_eq!(snapshot["workerCount"], 130);
        assert_eq!(snapshot["workers"].as_array().unwrap().len(), 128);
        assert_eq!(snapshot["nextWorker"], "worker-127");
        let page = store
            .daemon_workers(&cursor, Some("worker-127"), 128)
            .unwrap();
        assert_eq!(page["workers"].as_array().unwrap().len(), 2);
        assert_eq!(page["workers"][1]["workerId"], "worker-129");
        assert!(page["nextWorker"].is_null());
        assert_eq!(
            store.daemon_workers(&cursor, Some("missing"), 1).unwrap()["resyncRequired"],
            true
        );
        store.daemon_start().unwrap();
        assert_eq!(
            store
                .daemon_workers(&cursor, Some("worker-127"), 128)
                .unwrap()["resyncRequired"],
            true
        );
    }
    #[test]
    fn command_completion_cursor_and_restart_are_durable_without_effect_replay() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-daemon-journal-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("state.sqlite");
        let mut store = Store::open(&path).unwrap();
        let initial = store.daemon_start().unwrap();
        assert!(initial.suspended);
        let hash = crate::git_records::content_hash(b"resume-command");
        let receipt = store
            .daemon_begin("resume-one", &hash, "resume", None, Some(false))
            .unwrap();
        assert!(!receipt.replayed);
        let outcome = json!({"admissionsSuspended":false,"resumeAutomatically":false});
        store.daemon_finish("resume-one", &outcome).unwrap();
        store.daemon_finish("resume-one", &outcome).unwrap();
        assert_eq!(
            store.daemon_finish("resume-one", &json!({"different":true})),
            Err(StoreError::IdempotencyConflict)
        );
        let replay = store
            .daemon_begin("resume-one", &hash, "resume", None, Some(true))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.outcome, Some(outcome));
        assert!(!store.daemon_state().unwrap().suspended);
        assert_eq!(
            store
                .daemon_receipt(
                    "resume-one",
                    &crate::git_records::content_hash(b"different")
                )
                .unwrap_err(),
            StoreError::IdempotencyConflict
        );
        let (events, cursor, more) = store.daemon_events(&initial.cursor, 128).unwrap().unwrap();
        assert_eq!(events.len(), 2);
        assert!(!more);
        assert_eq!(cursor.sequence, initial.cursor.sequence + 2);
        drop(store);
        let mut reopened = Store::open(&path).unwrap();
        let restarted = reopened.daemon_start().unwrap();
        assert!(restarted.suspended);
        assert_eq!(restarted.generation, 2);
        assert_eq!(restarted.cursor.database, initial.cursor.database);
        assert!(reopened.daemon_pending().unwrap().is_empty());
        assert_eq!(
            reopened
                .daemon_events(&cursor, 128)
                .unwrap()
                .unwrap()
                .0
                .len(),
            1
        );
        drop(reopened);
        assert_eq!(
            root.parent(),
            Some(
                std::fs::canonicalize(std::env::temp_dir())
                    .unwrap()
                    .as_path()
            )
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
