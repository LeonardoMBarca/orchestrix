use super::{StoreError, StoreResult};
use rusqlite::{Connection, TransactionBehavior};
use std::time::Duration;

pub const SCHEMA_VERSION: i64 = 6;
const APPLICATION_ID: i64 = 0x4f585354;

// Kept as separate, immutable migrations so an existing v1 database can advance.
const V1: &str = r#"
CREATE TABLE store_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1), revision INTEGER NOT NULL CHECK(revision>=0));
INSERT INTO store_meta VALUES(1,0);
CREATE TABLE repositories(id TEXT PRIMARY KEY, location TEXT NOT NULL);
CREATE TABLE projects(id TEXT PRIMARY KEY, repository_id TEXT NOT NULL REFERENCES repositories(id), checkpoint TEXT NOT NULL);
CREATE TABLE connections(id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision>0), adapter TEXT NOT NULL, checkpoint TEXT NOT NULL, PRIMARY KEY(id,revision));
CREATE TABLE sessions(id TEXT PRIMARY KEY, connection_id TEXT NOT NULL, connection_revision INTEGER NOT NULL, adapter TEXT NOT NULL, checkpoint TEXT NOT NULL, FOREIGN KEY(connection_id,connection_revision) REFERENCES connections(id,revision));
CREATE TABLE runs(id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id), repository_id TEXT REFERENCES repositories(id), result_revision TEXT NOT NULL, state TEXT NOT NULL, checkpoint TEXT NOT NULL);
CREATE TABLE tasks(run_id TEXT NOT NULL REFERENCES runs(id), id TEXT NOT NULL, kind TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY(run_id,id));
CREATE TABLE dependencies(run_id TEXT NOT NULL, task_id TEXT NOT NULL, prerequisite TEXT NOT NULL, result TEXT NOT NULL, PRIMARY KEY(run_id,task_id,prerequisite), FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id), FOREIGN KEY(run_id,prerequisite) REFERENCES tasks(run_id,id));
CREATE TABLE attempts(id TEXT PRIMARY KEY, run_id TEXT NOT NULL, task_id TEXT NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id), active INTEGER NOT NULL CHECK(active IN(0,1)), FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE UNIQUE INDEX one_unresolved_attempt_per_session ON attempts(session_id) WHERE active=1;
CREATE INDEX attempts_task ON attempts(run_id,task_id);
CREATE TABLE artifacts(run_id TEXT NOT NULL, task_id TEXT NOT NULL, id TEXT NOT NULL, revision TEXT NOT NULL, base TEXT NOT NULL, PRIMARY KEY(run_id,task_id,id,revision,base), FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE TABLE evidence(digest TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id), task_id TEXT, kind TEXT NOT NULL, body TEXT NOT NULL, FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE INDEX evidence_work ON evidence(run_id,task_id);
CREATE TABLE conversations(id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id), checkpoint TEXT NOT NULL);
CREATE TABLE conversation_store(singleton INTEGER PRIMARY KEY CHECK(singleton=1), checkpoint TEXT NOT NULL);
CREATE TABLE messages(conversation_id TEXT NOT NULL REFERENCES conversations(id), ordinal INTEGER NOT NULL CHECK(ordinal>0), role TEXT NOT NULL, content TEXT NOT NULL, project_at_send TEXT REFERENCES projects(id), PRIMARY KEY(conversation_id,ordinal));
CREATE TABLE conversation_work(conversation_id TEXT NOT NULL REFERENCES conversations(id), ordinal INTEGER NOT NULL, run_id TEXT NOT NULL REFERENCES runs(id), task_id TEXT, attempt_id TEXT REFERENCES attempts(id), body TEXT NOT NULL, PRIMARY KEY(conversation_id,ordinal), FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE TABLE commands(scope TEXT NOT NULL, key TEXT NOT NULL, fingerprint TEXT NOT NULL, result TEXT NOT NULL, PRIMARY KEY(scope,key));
CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, schema_version INTEGER NOT NULL CHECK(schema_version=1), revision INTEGER NOT NULL, scope TEXT NOT NULL, command_key TEXT NOT NULL, kind TEXT NOT NULL, run_id TEXT REFERENCES runs(id), task_id TEXT, attempt_id TEXT REFERENCES attempts(id), body TEXT NOT NULL, FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id), FOREIGN KEY(scope,command_key) REFERENCES commands(scope,key) DEFERRABLE INITIALLY DEFERRED);
CREATE INDEX events_revision ON events(revision,sequence);
CREATE INDEX events_work ON events(run_id,task_id,attempt_id,sequence);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'append-only events'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'append-only events'); END;
CREATE TRIGGER commands_no_update BEFORE UPDATE ON commands BEGIN SELECT RAISE(ABORT,'immutable command result'); END;
CREATE TRIGGER commands_no_delete BEFORE DELETE ON commands BEGIN SELECT RAISE(ABORT,'immutable command result'); END;
CREATE TRIGGER evidence_no_update BEFORE UPDATE ON evidence BEGIN SELECT RAISE(ABORT,'append-only evidence'); END;
CREATE TRIGGER evidence_no_delete BEFORE DELETE ON evidence BEGIN SELECT RAISE(ABORT,'append-only evidence'); END;
"#;

const V2: &str = r#"
CREATE TABLE capacity_groups(id TEXT PRIMARY KEY, checkpoint TEXT NOT NULL);
CREATE TABLE saved_drafts(conversation_id TEXT PRIMARY KEY REFERENCES conversations(id), content TEXT NOT NULL CHECK(length(trim(content))>0));
CREATE TABLE document_records(singleton INTEGER PRIMARY KEY CHECK(singleton=1), checkpoint TEXT NOT NULL);
CREATE TABLE attempt_snapshots(attempt_id TEXT PRIMARY KEY REFERENCES attempts(id), checkpoint TEXT NOT NULL);
CREATE TABLE definitions(kind TEXT NOT NULL, id TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0), body TEXT NOT NULL, PRIMARY KEY(kind,id,version));
CREATE TABLE definition_references(kind TEXT NOT NULL, id TEXT NOT NULL, version INTEGER NOT NULL, target_kind TEXT NOT NULL, target_id TEXT NOT NULL, target_version INTEGER NOT NULL, PRIMARY KEY(kind,id,version,target_kind,target_id,target_version), FOREIGN KEY(kind,id,version) REFERENCES definitions(kind,id,version), FOREIGN KEY(target_kind,target_id,target_version) REFERENCES definitions(kind,id,version));
CREATE TABLE diagnostic_observations(id TEXT PRIMARY KEY, connection_id TEXT NOT NULL, connection_revision INTEGER NOT NULL, scope TEXT NOT NULL, version INTEGER NOT NULL CHECK(version>0), body TEXT NOT NULL, FOREIGN KEY(connection_id,connection_revision) REFERENCES connections(id,revision), UNIQUE(connection_id,connection_revision,scope,version));
CREATE INDEX diagnostics_scope ON diagnostic_observations(connection_id,connection_revision,scope,version);
CREATE TRIGGER definitions_no_update BEFORE UPDATE ON definitions BEGIN SELECT RAISE(ABORT,'immutable definition'); END;
CREATE TRIGGER definitions_no_delete BEFORE DELETE ON definitions BEGIN SELECT RAISE(ABORT,'immutable definition'); END;
CREATE TRIGGER diagnostics_no_update BEFORE UPDATE ON diagnostic_observations BEGIN SELECT RAISE(ABORT,'append-only diagnostic'); END;
CREATE TRIGGER diagnostics_no_delete BEFORE DELETE ON diagnostic_observations BEGIN SELECT RAISE(ABORT,'append-only diagnostic'); END;
"#;

const V3: &str = r#"
CREATE TABLE git_worktrees(operation_id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), repository_id TEXT NOT NULL REFERENCES repositories(id), run_id TEXT NOT NULL, task_id TEXT NOT NULL, attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id), path TEXT NOT NULL UNIQUE, branch TEXT NOT NULL, main_repository TEXT NOT NULL, state TEXT NOT NULL, intent TEXT NOT NULL, FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id), UNIQUE(main_repository,branch));
CREATE TABLE git_observations(operation_id TEXT NOT NULL REFERENCES git_worktrees(operation_id), ordinal INTEGER NOT NULL CHECK(ordinal>0), kind TEXT NOT NULL, evidence_sha256 TEXT NOT NULL, PRIMARY KEY(operation_id,ordinal));
CREATE TABLE captured_artifacts(id TEXT NOT NULL, revision TEXT NOT NULL, operation_id TEXT NOT NULL REFERENCES git_worktrees(operation_id), attempt_id TEXT NOT NULL REFERENCES attempts(id), base_oid TEXT NOT NULL, patch_sha256 TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(id,revision));
CREATE INDEX captured_artifacts_attempt ON captured_artifacts(attempt_id,id,revision);
CREATE TRIGGER git_intent_immutable BEFORE UPDATE ON git_worktrees WHEN old.operation_id!=new.operation_id OR old.project_id!=new.project_id OR old.repository_id!=new.repository_id OR old.run_id!=new.run_id OR old.task_id!=new.task_id OR old.attempt_id!=new.attempt_id OR old.path!=new.path OR old.branch!=new.branch OR old.main_repository!=new.main_repository OR old.intent!=new.intent BEGIN SELECT RAISE(ABORT,'immutable Git intent'); END;
CREATE TRIGGER git_worktrees_no_delete BEFORE DELETE ON git_worktrees BEGIN SELECT RAISE(ABORT,'retained Git provenance'); END;
CREATE TRIGGER git_observations_no_update BEFORE UPDATE ON git_observations BEGIN SELECT RAISE(ABORT,'append-only Git observation'); END;
CREATE TRIGGER git_observations_no_delete BEFORE DELETE ON git_observations BEGIN SELECT RAISE(ABORT,'append-only Git observation'); END;
CREATE TRIGGER captured_artifacts_no_update BEFORE UPDATE ON captured_artifacts BEGIN SELECT RAISE(ABORT,'immutable exact artifact'); END;
CREATE TRIGGER captured_artifacts_no_delete BEFORE DELETE ON captured_artifacts BEGIN SELECT RAISE(ABORT,'retained exact artifact'); END;
"#;

const V4: &str = r#"
CREATE TABLE manual_workers(id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runs(id), task_id TEXT NOT NULL, attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id), intent TEXT NOT NULL, FOREIGN KEY(run_id,task_id) REFERENCES tasks(run_id,id));
CREATE TABLE worker_observations(worker_id TEXT NOT NULL REFERENCES manual_workers(id), ordinal INTEGER NOT NULL, body TEXT NOT NULL, digest TEXT NOT NULL, PRIMARY KEY(worker_id,ordinal));
CREATE TRIGGER manual_workers_no_update BEFORE UPDATE ON manual_workers BEGIN SELECT RAISE(ABORT,'immutable worker intent'); END;
CREATE TRIGGER manual_workers_no_delete BEFORE DELETE ON manual_workers BEGIN SELECT RAISE(ABORT,'retained worker intent'); END;
CREATE TRIGGER worker_observations_no_update BEFORE UPDATE ON worker_observations BEGIN SELECT RAISE(ABORT,'append-only worker observations'); END;
CREATE TRIGGER worker_observations_no_delete BEFORE DELETE ON worker_observations BEGIN SELECT RAISE(ABORT,'append-only worker observations'); END;
"#;

pub(super) fn initialize(connection: &mut Connection) -> StoreResult<()> {
    connection.busy_timeout(Duration::from_millis(250))?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let app: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if version < 0 {
        return Err(StoreError::UnknownDatabase);
    }
    if version > SCHEMA_VERSION {
        return Err(StoreError::FutureSchema);
    }
    if (version > 0 && app != APPLICATION_ID) || (version == 0 && app != 0) {
        return Err(StoreError::UnknownDatabase);
    }
    if version == 0 {
        let tables: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if tables != 0 {
            return Err(StoreError::UnknownDatabase);
        }
    }
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let foreign_keys: bool =
        connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if !foreign_keys {
        return Err(StoreError::Configuration);
    }
    let mode: String = connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if mode != "wal" {
        let selected: String =
            connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if selected != "wal" {
            return Err(StoreError::Configuration);
        }
    }
    connection.pragma_update(None, "synchronous", "FULL")?;
    let synchronous: i64 = connection.pragma_query_value(None, "synchronous", |row| row.get(0))?;
    if synchronous != 2 {
        return Err(StoreError::Configuration);
    }
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Recheck under the writer lock: another opener may have migrated meanwhile.
    let mut current: i64 =
        transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current_app: i64 =
        transaction.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if current < 0
        || (current > 0 && current_app != APPLICATION_ID)
        || (current == 0 && current_app != 0)
    {
        return Err(StoreError::UnknownDatabase);
    }
    if current > SCHEMA_VERSION {
        return Err(StoreError::FutureSchema);
    }
    if current == 0 {
        let tables: i64 = transaction.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if tables != 0 {
            return Err(StoreError::UnknownDatabase);
        }
        transaction.execute_batch(V1)?;
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
        transaction.pragma_update(None, "user_version", 1)?;
        current = 1;
    }
    if current == 1 {
        transaction.execute_batch(V2)?;
        transaction.pragma_update(None, "user_version", 2)?;
        current = 2;
    }
    if current == 2 {
        transaction.execute_batch(V3)?;
        transaction.pragma_update(None, "user_version", 3)?;
        current = 3;
    }
    if current == 3 {
        transaction.execute_batch(V4)?;
        transaction.pragma_update(None, "user_version", 4)?;
        current = 4;
    }
    if current == 4 {
        transaction.execute_batch(super::daemon::MIGRATION_V5)?;
        transaction.pragma_update(None, "user_version", 5)?;
        current = 5;
    }
    if current == 5 {
        transaction.execute_batch(super::review::MIGRATION_V6)?;
        transaction.pragma_update(None, "user_version", 6)?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_v1(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(V1).unwrap();
    connection
        .pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
}

#[cfg(test)]
pub(super) fn create_v2(path: &std::path::Path) {
    create_v1(path);
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(V2).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
}

#[cfg(test)]
pub(super) fn create_v3(path: &std::path::Path) {
    create_v2(path);
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(V3).unwrap();
    connection.pragma_update(None, "user_version", 3).unwrap();
}

#[cfg(test)]
pub(super) fn create_v4(path: &std::path::Path) {
    create_v3(path);
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(V4).unwrap();
    connection.pragma_update(None, "user_version", 4).unwrap();
}
