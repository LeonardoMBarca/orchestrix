//! Actual SQLite files, no provider, supervisor, in-memory database or external execution.
use super::*;
use crate::domain::*;
use rusqlite::Connection;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct TempDb {
    directory: PathBuf,
    path: PathBuf,
}
impl TempDb {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "orchestrix-sqlite-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("state.sqlite");
        Self { directory, path }
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn project(id: &str) -> Project {
    Project::new(
        ProjectId::new(id).unwrap(),
        Repository::new(
            RepositoryId::new(format!("repo-{id}")).unwrap(),
            LocalRef::new(format!("directory-{id}")).unwrap(),
        ),
    )
}
fn request(key: &str, revision: u64, command: Command) -> Request {
    Request {
        scope: LocalRef::new("installation").unwrap(),
        key: LocalRef::new(key).unwrap(),
        expected_revision: revision,
        command,
    }
}

#[test]
fn captured_artifact_receipts_use_one_closed_validated_reference_tag() {
    let result = CommandResult {
        schema_version: 1,
        revision: 3,
        disposition: Disposition::Applied,
        error_code: None,
        output: json!({"gitArtifact":{"id":"artifact-1","revision":"a".repeat(64),"operationId":"worktree-operation"}}),
    };
    assert_eq!(
        decode_result(&serde_json::to_string(&result).unwrap()).unwrap(),
        result
    );
    for output in [
        json!({"gitArtifact":{"id":"artifact-1","revision":"a".repeat(64),"operationId":"worktree-operation","extra":true}}),
        json!({"gitArtifact":{"id":"artifact-1","revision":"a".repeat(64),"operationId":"worktree-operation"},"extra":true}),
        json!({"gitArtifact":{"id":"artifact-1","revision":"broken-hash","operationId":"worktree-operation"}}),
        json!({"otherTag":{"id":"artifact-1"}}),
    ] {
        let altered = CommandResult {
            output,
            ..result.clone()
        };
        assert_eq!(
            decode_result(&serde_json::to_string(&altered).unwrap()),
            Err(StoreError::InvalidState)
        );
    }
}
fn add(key: &str, revision: u64, id: &str) -> Request {
    request(key, revision, Command::AddProject(project(id)))
}

#[test]
fn compiled_sqlite_settings_and_real_file_migration_preserve_existing_data() {
    let temp = TempDb::new();
    schema::create_v1(&temp.path);
    let prior = Connection::open(&temp.path).unwrap();
    let p = project("preserved");
    prior
        .execute(
            "INSERT INTO repositories VALUES(?1,?2)",
            params![
                p.repository().id().as_str(),
                p.repository().location().as_str()
            ],
        )
        .unwrap();
    prior
        .execute(
            "INSERT INTO projects VALUES(?1,?2,?3)",
            params![
                p.id().as_str(),
                p.repository().id().as_str(),
                p.checkpoint().to_string()
            ],
        )
        .unwrap();
    let mut conversations = ConversationStore::new();
    conversations.add_project(p).unwrap();
    prior
        .execute(
            "INSERT INTO conversation_store VALUES(1,?1)",
            [conversations.checkpoint().to_string()],
        )
        .unwrap();
    drop(prior);
    let store = Store::open(&temp.path).unwrap();
    assert_eq!(Store::sqlite_version(), "3.53.2");
    println!(
        "rusqlite 0.40.2 bundled SQLite {}: migrated file v1 -> current",
        Store::sqlite_version()
    );
    assert!(
        store
            .load()
            .unwrap()
            .projects()
            .contains_key(&ProjectId::new("preserved").unwrap())
    );
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))
            .unwrap(),
        250
    );
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
}

#[test]
fn v2_database_with_existing_project_migrates_to_git_schema_without_rewriting_history() {
    let temp = TempDb::new();
    schema::create_v2(&temp.path);
    let connection = Connection::open(&temp.path).unwrap();
    let project = project("v2-existing");
    connection
        .execute(
            "INSERT INTO repositories VALUES(?1,?2)",
            params![
                project.repository().id().as_str(),
                project.repository().location().as_str()
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO projects VALUES(?1,?2,?3)",
            params![
                project.id().as_str(),
                project.repository().id().as_str(),
                project.checkpoint().to_string()
            ],
        )
        .unwrap();
    let mut conversations = ConversationStore::new();
    conversations.add_project(project).unwrap();
    connection
        .execute(
            "INSERT INTO conversation_store VALUES(1,?1)",
            [conversations.checkpoint().to_string()],
        )
        .unwrap();
    let before: String = connection
        .query_row("SELECT checkpoint FROM projects", [], |row| row.get(0))
        .unwrap();
    drop(connection);
    let store = Store::open(&temp.path).unwrap();
    assert_eq!(store.load().unwrap().projects().len(), 1);
    assert!(store.load().unwrap().worktrees().is_empty());
    let after: String = store
        .connection
        .query_row("SELECT checkpoint FROM projects", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        schema::SCHEMA_VERSION
    );
}

#[test]
fn populated_v3_migrates_to_worker_schema_without_rewriting_existing_project() {
    let temp = TempDb::new();
    schema::create_v3(&temp.path);
    let db = Connection::open(&temp.path).unwrap();
    let p = project("v3-existing");
    let body = p.checkpoint().to_string();
    db.execute(
        "INSERT INTO repositories VALUES(?1,?2)",
        params![
            p.repository().id().as_str(),
            p.repository().location().as_str()
        ],
    )
    .unwrap();
    db.execute(
        "INSERT INTO projects VALUES(?1,?2,?3)",
        params![p.id().as_str(), p.repository().id().as_str(), body],
    )
    .unwrap();
    let mut conversations = ConversationStore::new();
    conversations.add_project(p).unwrap();
    db.execute(
        "INSERT INTO conversation_store VALUES(1,?1)",
        [conversations.checkpoint().to_string()],
    )
    .unwrap();
    drop(db);
    let store = Store::open(&temp.path).unwrap();
    assert_eq!(store.load().unwrap().projects().len(), 1);
    assert!(store.load().unwrap().workers().is_empty());
    let saved: String = store
        .connection
        .query_row("SELECT checkpoint FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(saved, body);
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        SCHEMA_VERSION
    );
}

#[test]
fn worker_migration_failure_is_atomic_and_preserves_version_three() {
    let temp = TempDb::new();
    schema::create_v3(&temp.path);
    let db = Connection::open(&temp.path).unwrap();
    db.execute_batch(
        "CREATE TABLE manual_workers(sentinel TEXT); INSERT INTO manual_workers VALUES('preserve')",
    )
    .unwrap();
    drop(db);
    assert!(Store::open(&temp.path).is_err());
    let db = Connection::open(&temp.path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM manual_workers", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='worker_observations'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn version_four_gains_stable_daemon_identity_and_atomic_domain_event_feed() {
    let temp = TempDb::new();
    schema::create_v4(&temp.path);
    let mut store = Store::open(&temp.path).unwrap();
    let initial = store.daemon_state().unwrap();
    assert!(initial.suspended);
    assert_eq!(initial.cursor.sequence, 0);
    store
        .execute(&add("after-v4-migration", 0, "preserved-project"))
        .unwrap();
    let (events, cursor, more) = store.daemon_events(&initial.cursor, 128).unwrap().unwrap();
    assert!(!events.is_empty());
    assert!(!more);
    assert_eq!(events[0].kind, "domain-change");
    assert_eq!(events[0].body["revision"], 1);
    assert_eq!(cursor.database, initial.cursor.database);
    drop(store);
    let reopened = Store::open(&temp.path).unwrap();
    assert_eq!(reopened.daemon_state().unwrap().cursor, cursor);
    assert_eq!(reopened.load().unwrap().projects().len(), 1);
}

#[test]
fn daemon_migration_failure_is_atomic_and_preserves_version_four() {
    let temp = TempDb::new();
    schema::create_v4(&temp.path);
    let db = Connection::open(&temp.path).unwrap();
    db.execute_batch(
        "CREATE TABLE daemon_meta(sentinel TEXT); INSERT INTO daemon_meta VALUES('preserve')",
    )
    .unwrap();
    drop(db);
    assert!(Store::open(&temp.path).is_err());
    let db = Connection::open(&temp.path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM daemon_meta", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='daemon_events'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn future_unknown_and_corrupt_databases_are_refused_without_recreation() {
    let temp = TempDb::new();
    let connection = Connection::open(&temp.path).unwrap();
    connection.execute_batch("CREATE TABLE sentinel(value TEXT);INSERT INTO sentinel VALUES('preserve');PRAGMA user_version=999;").unwrap();
    drop(connection);
    let before = std::fs::read(&temp.path).unwrap();
    assert!(matches!(
        Store::open(&temp.path),
        Err(StoreError::FutureSchema)
    ));
    assert_eq!(std::fs::read(&temp.path).unwrap(), before);
    let connection = Connection::open(&temp.path).unwrap();
    connection.pragma_update(None, "user_version", 0).unwrap();
    drop(connection);
    assert!(matches!(
        Store::open(&temp.path),
        Err(StoreError::UnknownDatabase)
    ));
    let other = TempDb::new();
    let mut store = Store::open(&other.path).unwrap();
    store.execute(&add("p", 0, "one")).unwrap();
    store
        .connection
        .execute("UPDATE projects SET checkpoint='{}'", [])
        .unwrap();
    drop(store);
    assert!(Store::open(&other.path).is_err());
    let check = Connection::open(&other.path).unwrap();
    assert_eq!(
        check
            .query_row("SELECT checkpoint FROM projects", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "{}"
    );
}

#[test]
fn integration_migration_preserves_v6_rows_and_rolls_back_collisions() {
    for collision in [false, true] {
        let temp = TempDb::new();
        let mut store = Store::open(&temp.path).unwrap();
        store
            .execute(&add("before-integration", 0, "preserved-v6-project"))
            .unwrap();
        let before: String = store
            .connection
            .query_row("SELECT checkpoint FROM projects", [], |r| r.get(0))
            .unwrap();
        // Migrations 1..6 are unchanged. Remove the empty later extensions
        // in this private fixture to exercise both migration outcomes.
        store.connection.execute_batch("DROP TABLE connection_diagnostic_validity;DROP TABLE connection_refresh_configurations;DROP TABLE connection_diagnostic_runs;DROP TABLE connection_links;DROP TABLE connection_selection;DROP TABLE connection_commands;DROP TABLE integration_observations;DROP TABLE serial_integrations;PRAGMA user_version=6;").unwrap();
        if collision {
            store.connection.execute_batch("CREATE TABLE serial_integrations(sentinel TEXT);INSERT INTO serial_integrations VALUES('preserve');").unwrap();
        }
        drop(store);
        let opened = Store::open(&temp.path);
        if collision {
            assert!(opened.is_err());
            let db = Connection::open(&temp.path).unwrap();
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                6
            );
            assert_eq!(
                db.query_row("SELECT sentinel FROM serial_integrations", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                "preserve"
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='integration_observations'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            assert_eq!(
                db.query_row("SELECT checkpoint FROM projects", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                before
            );
        } else {
            let store = opened.unwrap();
            assert_eq!(
                store
                    .connection
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                SCHEMA_VERSION
            );
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT checkpoint FROM projects", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                before
            );
            assert!(store.load().unwrap().integrations().is_empty());
            assert_eq!(store.load().unwrap().projects().len(), 1);
        }
    }
}

#[test]
fn connection_migration_preserves_v7_rows_and_rolls_back_collisions() {
    for collision in [false, true] {
        let temp = TempDb::new();
        let mut store = Store::open(&temp.path).unwrap();
        store
            .execute(&add("before-connections", 0, "preserved-v7-project"))
            .unwrap();
        let before: String = store
            .connection
            .query_row("SELECT checkpoint FROM projects", [], |r| r.get(0))
            .unwrap();
        store.connection.execute_batch("DROP TABLE connection_diagnostic_validity;DROP TABLE connection_refresh_configurations;DROP TABLE connection_diagnostic_runs;DROP TABLE connection_links;DROP TABLE connection_selection;DROP TABLE connection_commands;PRAGMA user_version=7;").unwrap();
        if collision {
            store.connection.execute_batch("CREATE TABLE connection_links(sentinel TEXT);INSERT INTO connection_links VALUES('preserve');").unwrap();
        }
        drop(store);
        let opened = Store::open(&temp.path);
        if collision {
            assert!(opened.is_err());
            let db = Connection::open(&temp.path).unwrap();
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                7
            );
            assert_eq!(
                db.query_row("SELECT sentinel FROM connection_links", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                "preserve"
            );
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='connection_commands'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            assert_eq!(
                db.query_row("SELECT checkpoint FROM projects", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                before
            );
        } else {
            let store = opened.unwrap();
            assert_eq!(
                store
                    .connection
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                SCHEMA_VERSION
            );
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT checkpoint FROM projects", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                before
            );
            assert_eq!(store.connection_list().unwrap()["connections"], json!([]));
            assert_eq!(store.load().unwrap().projects().len(), 1);
        }
    }
}

#[test]
fn migration_failure_rolls_back_schema_and_user_version() {
    let temp = TempDb::new();
    schema::create_v1(&temp.path);
    let connection = Connection::open(&temp.path).unwrap();
    // An incompatible table is not silently dropped to make migration succeed.
    connection.execute_batch("CREATE TABLE attempt_snapshots(sentinel TEXT);INSERT INTO attempt_snapshots VALUES('keep')").unwrap();
    drop(connection);
    assert!(Store::open(&temp.path).is_err());
    let connection = Connection::open(&temp.path).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='saved_drafts'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT sentinel FROM attempt_snapshots", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "keep"
    );
}

#[test]
fn duplicate_before_and_after_reopen_and_conflicting_content() {
    let temp = TempDb::new();
    let command = add("same", 0, "one");
    let mut store = Store::open(&temp.path).unwrap();
    let first = store.execute(&command).unwrap();
    assert!(!first.replayed);
    assert_eq!(
        store.execute(&command).unwrap(),
        Receipt {
            result: first.result.clone(),
            replayed: true
        }
    );
    assert_eq!(store.event_count().unwrap(), 1);
    assert_eq!(store.command_count().unwrap(), 1);
    drop(store);
    let mut store = Store::open(&temp.path).unwrap();
    assert_eq!(store.execute(&command).unwrap().result, first.result);
    assert_eq!(
        store.execute(&add("same", 0, "different")),
        Err(StoreError::IdempotencyConflict)
    );
    assert_eq!(
        store.execute(&add("same", 1, "one")),
        Err(StoreError::IdempotencyConflict)
    );
    assert_eq!(store.load().unwrap().projects().len(), 1);
    assert_eq!(store.event_count().unwrap(), 1);
}

#[test]
fn invalid_command_has_durable_result_without_business_transition_or_event() {
    let temp = TempDb::new();
    let mut store = Store::open(&temp.path).unwrap();
    store.execute(&add("first", 0, "one")).unwrap();
    let invalid = add("different-command", 1, "one");
    let receipt = store.execute(&invalid).unwrap();
    assert_eq!(receipt.result.disposition, Disposition::InvalidNoChange);
    assert_eq!(receipt.result.revision, 1);
    assert_eq!(store.event_count().unwrap(), 1);
    drop(store);
    let mut store = Store::open(&temp.path).unwrap();
    assert!(store.execute(&invalid).unwrap().replayed);
    assert_eq!(store.command_count().unwrap(), 2);
}

#[test]
fn injected_failures_between_writes_leave_no_partial_state_event_or_result() {
    for fault in [
        FaultPoint::AfterState,
        FaultPoint::AfterEvent,
        FaultPoint::AfterResult,
    ] {
        let temp = TempDb::new();
        let mut store = Store::open(&temp.path).unwrap();
        store.execute(&add("baseline", 0, "one")).unwrap();
        let old = store.load().unwrap();
        let command = add("rolled-back", 1, "two");
        assert_eq!(
            store.execute_with_fault(&command, fault),
            Err(StoreError::InjectedRollback)
        );
        assert_eq!(old.revision(), 1); // caller's detached view did not become a false success.
        assert_eq!(store.load().unwrap().projects().len(), 1);
        assert_eq!(store.event_count().unwrap(), 1);
        assert_eq!(store.command_count().unwrap(), 1);
        drop(store);
        let mut store = Store::open(&temp.path).unwrap();
        assert_eq!(store.load().unwrap().revision(), 1);
        assert_eq!(store.execute(&command).unwrap().result.revision, 2);
    }
}

#[test]
fn committed_reply_loss_replays_persisted_result_without_new_writes() {
    let temp = TempDb::new();
    let mut store = Store::open(&temp.path).unwrap();
    let command = add("reply-lost", 0, "one");
    assert_eq!(
        store.execute_with_fault(&command, FaultPoint::AfterCommit),
        Err(StoreError::ReplyLost)
    );
    drop(store);
    let mut store = Store::open(&temp.path).unwrap();
    let receipt = store.execute(&command).unwrap();
    assert!(receipt.replayed);
    assert_eq!(receipt.result.revision, 1);
    assert_eq!(store.event_count().unwrap(), 1);
    assert_eq!(store.command_count().unwrap(), 1);
}

#[test]
fn two_connections_race_on_same_command_and_only_one_commits() {
    let temp = TempDb::new();
    let _initialized = Store::open(&temp.path).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let writers = (0..2)
        .map(|_| {
            let path = temp.path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(path).unwrap();
                barrier.wait();
                store.execute(&add("same", 0, "one")).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let receipts = writers
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(receipts.iter().filter(|r| r.replayed).count(), 1);
    assert_eq!(receipts[0].result, receipts[1].result);
    let store = Store::open(&temp.path).unwrap();
    assert_eq!(store.event_count().unwrap(), 1);
    assert_eq!(store.command_count().unwrap(), 1);
}

#[test]
fn stale_writer_cannot_overwrite_and_busy_wait_is_finite() {
    let temp = TempDb::new();
    let mut first = Store::open(&temp.path).unwrap();
    let mut second = Store::open(&temp.path).unwrap();
    first.execute(&add("one", 0, "one")).unwrap();
    assert_eq!(
        second.execute(&add("two", 0, "two")),
        Err(StoreError::StaleRevision)
    );
    let lock = first
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let started = Instant::now();
    assert_eq!(second.execute(&add("two", 1, "two")), Err(StoreError::Busy));
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(150));
    assert!(elapsed < Duration::from_secs(3));
    drop(lock);
    second.execute(&add("two", 1, "two")).unwrap();
    assert_eq!(first.load().unwrap().revision(), 2);
}

#[test]
fn append_only_and_foreign_keys_are_enforced_by_database() {
    let temp = TempDb::new();
    let mut store = Store::open(&temp.path).unwrap();
    store.execute(&add("one", 0, "one")).unwrap();
    for statement in [
        "UPDATE events SET kind='changed'",
        "DELETE FROM events",
        "UPDATE commands SET result='{}'",
        "DELETE FROM commands",
    ] {
        assert!(store.connection.execute(statement, []).is_err());
    }
    assert!(
        store
            .connection
            .execute(
                "INSERT INTO sessions VALUES('orphan','missing',1,'fake','{}')",
                []
            )
            .is_err()
    );
}

#[test]
fn changed_sql_projection_is_refused_instead_of_silently_repaired() {
    let temp = TempDb::new();
    let mut store = Store::open(&temp.path).unwrap();
    store.execute(&add("one", 0, "one")).unwrap();
    let saved_revision = store.load().unwrap().revision();
    store
        .connection
        .execute("UPDATE repositories SET location='other-location'", [])
        .unwrap();
    assert!(matches!(store.load(), Err(StoreError::InvalidState)));
    assert_eq!(
        store.execute(&add("two", saved_revision, "two")),
        Err(StoreError::InvalidState)
    );
    drop(store);
    assert!(matches!(
        Store::open(&temp.path),
        Err(StoreError::InvalidState)
    ));
    let raw = Connection::open(&temp.path).unwrap();
    assert_eq!(
        raw.query_row("SELECT location FROM repositories", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "other-location"
    );
    assert_eq!(
        raw.query_row("SELECT count(*) FROM commands", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn close_and_reopen_in_another_process_continues_domain_work() {
    let temp = TempDb::new();
    let mut store = Store::open(&temp.path).unwrap();
    store.execute(&add("first", 0, "one")).unwrap();
    drop(store);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "persistence::tests::subprocess_reopens_and_creates_run",
            "--ignored",
            "--nocapture",
        ])
        .env("ORCHESTRIX_SQLITE_TEST_PATH", &temp.path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    let store = Store::open(&temp.path).unwrap();
    assert_eq!(store.load().unwrap().runs().len(), 1);
    assert_eq!(store.load().unwrap().revision(), 2);
}

#[test]
#[ignore = "invoked only by the parent file-backed subprocess test"]
fn subprocess_reopens_and_creates_run() {
    let path =
        PathBuf::from(std::env::var_os("ORCHESTRIX_SQLITE_TEST_PATH").expect("parent test path"));
    let mut store = Store::open(path).unwrap();
    let state = store.load().unwrap();
    let project = state
        .projects()
        .get(&ProjectId::new("one").unwrap())
        .unwrap();
    let run = Run::new(
        RunId::new("child-run").unwrap(),
        Some(project),
        RevisionId::new("base").unwrap(),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    store
        .execute(&request(
            "child-run",
            state.revision(),
            Command::create_run(run),
        ))
        .unwrap();
}
