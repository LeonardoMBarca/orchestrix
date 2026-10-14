use orchestrix_core::domain::*;
use orchestrix_core::persistence::{Command, Disposition, Receipt, Request, Store, StoreError};
use orchestrix_core::persistence_records::*;
use orchestrix_core::runtime_contract::normalize_descriptor;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchestrix-persistence-boundary-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn database(&self) -> PathBuf {
        self.0.join("state.sqlite")
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn execute(store: &mut Store, key: &str, command: Command) -> Receipt {
    let request = Request {
        scope: LocalRef::new("boundary-tests").unwrap(),
        key: LocalRef::new(key).unwrap(),
        expected_revision: store.load().unwrap().revision(),
        command,
    };
    store.execute(&request).unwrap()
}
fn project(id: &str) -> Project {
    Project::new(
        ProjectId::new(id).unwrap(),
        Repository::new(
            RepositoryId::new(format!("{id}-repo")).unwrap(),
            LocalRef::new(format!("{id}-directory")).unwrap(),
        ),
    )
}
fn connection(revision: u64) -> Connection {
    Connection::new(normalize_descriptor(&json!({"version":{"major":1,"minor":1},"adapterId":"fixture-adapter","runtimeVersion":"fixture-1","connection":{"id":"connection-1","revision":revision,"authentication":"authenticated","billing":"subscription","availability":"available"},"models":[]})).unwrap()).unwrap()
}
fn scope() -> DiagnosticScope {
    DiagnosticScope {
        kind: DiagnosticScopeKind::Model,
        reference: Some("model-1".to_owned()),
    }
}
fn diagnostic() -> DiagnosticInput {
    DiagnosticInput {
        id: "diagnostic-success".to_owned(),
        connection_id: "connection-1".to_owned(),
        connection_revision: 1,
        version: 1,
        scope: scope(),
        source: DiagnosticSource::Catalog,
        outcome: DiagnosticOutcome::Success,
        checked_at: Some("2026-10-14T12:00:00.000Z".to_owned()),
        valid_until: Some("2026-10-14T13:00:00.000Z".to_owned()),
        complete: false,
        values: json!({"support":"supported","availability":"unknown","api_key":"synthetic-diagnostic-credential"}),
        error_code: None,
    }
}

#[test]
fn shared_capacity_metadata_reopens_with_members_evidence_and_idempotency() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let group_id = CapacityGroupId::new("shared-plan").unwrap();
    let first = ConnectionId::new("connection-1").unwrap();
    let second = ConnectionId::new("connection-2").unwrap();
    let mut store = Store::open(&database).unwrap();
    execute(
        &mut store,
        "capacity-account-1",
        Command::AddConnection(connection(1)),
    );
    let mut unknown = CapacityGroup::new(group_id.clone(), SharedCapacity::Unknown);
    unknown.add_member(first.clone()).unwrap();
    let request = Request {
        scope: LocalRef::new("capacity-tests").unwrap(),
        key: LocalRef::new("unknown-group").unwrap(),
        expected_revision: store.load().unwrap().revision(),
        command: Command::SaveCapacityGroup(unknown),
    };
    let original = store.execute(&request).unwrap();
    let events = store.event_count().unwrap();
    assert_eq!(original.result.disposition, Disposition::Applied);
    assert!(store.execute(&request).unwrap().replayed);
    drop(store);
    let mut store = Store::open(&database).unwrap();
    assert_eq!(store.event_count().unwrap(), events);
    assert!(store.execute(&request).unwrap().replayed);
    let restored = store.load().unwrap();
    assert_eq!(
        restored.capacity_groups()[&group_id].capacity(),
        &SharedCapacity::Unknown
    );
    assert_eq!(
        restored.capacity_groups()[&group_id]
            .members()
            .iter()
            .collect::<Vec<_>>(),
        vec![&first]
    );
    let mut invalid = CapacityGroup::new(
        CapacityGroupId::new("unknown-member").unwrap(),
        SharedCapacity::Unknown,
    );
    invalid.add_member(second.clone()).unwrap();
    let rejected = execute(
        &mut store,
        "unknown-capacity-member",
        Command::SaveCapacityGroup(invalid),
    );
    assert_eq!(rejected.result.disposition, Disposition::InvalidNoChange);
    assert_eq!(store.load().unwrap().capacity_groups().len(), 1);
    let mut descriptor = connection(1).descriptor().as_json().clone();
    descriptor["connection"]["id"] = json!(second.as_str());
    execute(
        &mut store,
        "capacity-account-2",
        Command::AddConnection(
            Connection::new(normalize_descriptor(&descriptor).unwrap()).unwrap(),
        ),
    );
    let mut group = store.load().unwrap().capacity_groups()[&group_id].clone();
    let evidence = EvidenceId::new("shared-capacity-observation").unwrap();
    group.record_capacity(SharedCapacity::Observed(
        CapacityObservation::new(100, 25, evidence.clone()).unwrap(),
    ));
    group.add_member(second.clone()).unwrap();
    let mut invalid = group.checkpoint();
    invalid["data"]["capacity"]["remaining"] = json!(101);
    assert!(CapacityGroup::restore_checkpoint(&invalid).is_err());
    let observed = Request {
        scope: LocalRef::new("capacity-tests").unwrap(),
        key: LocalRef::new("observed-group").unwrap(),
        expected_revision: store.load().unwrap().revision(),
        command: Command::SaveCapacityGroup(group),
    };
    let receipt = store.execute(&observed).unwrap();
    drop(store);
    let mut store = Store::open(&database).unwrap();
    assert_eq!(store.execute(&observed).unwrap().result, receipt.result);
    let mut group = store.load().unwrap().capacity_groups()[&group_id].clone();
    assert_eq!(group.members().len(), 2);
    match group.capacity() {
        SharedCapacity::Observed(observation) => {
            assert_eq!(observation.limit(), 100);
            assert_eq!(observation.remaining(), 25);
            assert_eq!(observation.evidence(), &evidence);
        }
        SharedCapacity::Unknown => panic!("historical observation was lost"),
    }
    group.remove_member(&first).unwrap();
    execute(
        &mut store,
        "capacity-remove-member",
        Command::SaveCapacityGroup(group),
    );
    // Replaying an earlier successful save returns its old result without undoing later edits.
    assert!(store.execute(&observed).unwrap().replayed);
    drop(store);
    let store = Store::open(&database).unwrap();
    let state = store.load().unwrap();
    assert_eq!(
        state.capacity_groups()[&group_id]
            .members()
            .iter()
            .collect::<Vec<_>>(),
        vec![&second]
    );
    drop(store);
    let sql = rusqlite::Connection::open(&database).unwrap();
    sql.execute(
        "UPDATE capacity_groups SET id=?1 WHERE id=?2",
        rusqlite::params!["incorrect-row-id", group_id.as_str()],
    )
    .unwrap();
    drop(sql);
    assert!(matches!(
        Store::open(&database),
        Err(StoreError::InvalidState)
    ));
}

#[test]
fn conversations_drafts_association_and_first_send_are_durable_without_creating_home_session() {
    let directory = TestDirectory::new();
    let database = directory.database();
    {
        let mut store = Store::open(&database).unwrap();
        execute(
            &mut store,
            "empty-project",
            Command::AddProject(project("project-1")),
        );
        assert_eq!(
            store
                .load()
                .unwrap()
                .conversations()
                .conversations()
                .count(),
            0
        );
        let conversation = ConversationId::new("conversation-1").unwrap();
        let draft = execute(
            &mut store,
            "home-draft",
            Command::SaveDraft {
                conversation: conversation.clone(),
                content: "Unsent idea".to_owned(),
            },
        );
        assert_eq!(draft.result.disposition, Disposition::InvalidNoChange);
        let empty = execute(
            &mut store,
            "empty-message",
            Command::Send {
                conversation: conversation.clone(),
                project: None,
                content: " \n\t ".to_owned(),
            },
        );
        assert_eq!(empty.result.disposition, Disposition::InvalidNoChange);
        assert_eq!(
            store
                .load()
                .unwrap()
                .conversations()
                .conversations()
                .count(),
            0
        );
        execute(
            &mut store,
            "first-message",
            Command::Send {
                conversation: conversation.clone(),
                project: None,
                content: "Develop a useful improvement".to_owned(),
            },
        );
        execute(
            &mut store,
            "reply",
            Command::Reply {
                conversation: conversation.clone(),
                content: "Let's clarify the acceptance criteria".to_owned(),
            },
        );
        execute(
            &mut store,
            "draft",
            Command::SaveDraft {
                conversation: conversation.clone(),
                content: "Saved follow-up".to_owned(),
            },
        );
        execute(
            &mut store,
            "attach",
            Command::Attach {
                conversation: conversation.clone(),
                project: ProjectId::new("project-1").unwrap(),
            },
        );
        let saved = store.load().unwrap();
        let conversation = saved.conversations().conversation(&conversation).unwrap();
        assert_eq!(conversation.project_id().unwrap().as_str(), "project-1");
        assert!(
            conversation
                .messages()
                .iter()
                .all(|message| message.project_at_send().is_none())
        );
    }
    let mut store = Store::open(&database).unwrap();
    let conversation = ConversationId::new("conversation-1").unwrap();
    let saved = store.load().unwrap();
    assert_eq!(saved.projects().len(), 1);
    let persisted = saved.conversations().conversation(&conversation).unwrap();
    assert_eq!(persisted.messages().len(), 2);
    assert_eq!(
        saved.drafts().get(&conversation).unwrap(),
        "Saved follow-up"
    );
    assert_eq!(persisted.messages()[0].role(), MessageRole::User);
    assert_eq!(persisted.messages()[1].role(), MessageRole::Orchestrator);
    execute(
        &mut store,
        "send-followup",
        Command::Send {
            conversation: conversation.clone(),
            project: None,
            content: "Use these criteria".to_owned(),
        },
    );
    let saved = store.load().unwrap();
    assert!(!saved.drafts().contains_key(&conversation));
    let persisted = saved.conversations().conversation(&conversation).unwrap();
    assert_eq!(persisted.messages().len(), 3);
    assert_eq!(
        persisted.messages()[2].project_at_send().unwrap().as_str(),
        "project-1"
    );
}

#[test]
fn failed_diagnostics_preserve_original_success_across_sqlite_reopen_and_revision_change() {
    let directory = TestDirectory::new();
    let database = directory.database();
    {
        let mut store = Store::open(&database).unwrap();
        execute(
            &mut store,
            "connection",
            Command::AddConnection(connection(1)),
        );
        execute(
            &mut store,
            "successful-diagnostic",
            Command::Records(RecordsCommand::Diagnostic(diagnostic())),
        );
    }
    {
        let mut store = Store::open(&database).unwrap();
        let mut failed = diagnostic();
        failed.id = "diagnostic-failed".to_owned();
        failed.version = 2;
        failed.outcome = DiagnosticOutcome::Failed;
        failed.checked_at = Some("2026-10-14T12:30:00.000Z".to_owned());
        failed.valid_until = None;
        failed.values = serde_json::Value::Null;
        failed.complete = false;
        failed.error_code = Some("timeout".to_owned());
        execute(
            &mut store,
            "failed-diagnostic",
            Command::Records(RecordsCommand::Diagnostic(failed)),
        );
    }
    {
        let mut store = Store::open(&database).unwrap();
        let saved = store.load().unwrap();
        let head = saved
            .records()
            .diagnostic_head(&ConnectionId::new("connection-1").unwrap(), &scope())
            .unwrap();
        assert_eq!(head.latest().outcome, DiagnosticOutcome::Failed);
        assert_eq!(
            head.last_success().unwrap().checked_at.as_deref(),
            Some("2026-10-14T12:00:00.000Z")
        );
        assert_eq!(
            head.last_success().unwrap().valid_until.as_deref(),
            Some("2026-10-14T13:00:00.000Z")
        );
        assert_eq!(
            head.last_success().unwrap().values["availability"],
            "unknown"
        );
        assert_eq!(head.last_success().unwrap().values["api_key"], "[REDACTED]");
        assert!(
            !head
                .has_valid_success_at("2026-10-14T13:00:00.000Z")
                .unwrap()
        );
        execute(
            &mut store,
            "connection-revised",
            Command::AddConnection(connection(2)),
        );
        let mut late = diagnostic();
        late.id = "late-diagnostic".to_owned();
        late.version = 3;
        let receipt = execute(
            &mut store,
            "late-result",
            Command::Records(RecordsCommand::Diagnostic(late)),
        );
        assert_eq!(receipt.result.disposition, Disposition::InvalidNoChange);
    }
    let store = Store::open(&database).unwrap();
    let saved = store.load().unwrap();
    assert_eq!(saved.records().diagnostics().len(), 2);
    assert!(
        saved
            .records()
            .diagnostic_head(&ConnectionId::new("connection-1").unwrap(), &scope())
            .is_none()
    );
}

#[test]
fn definitions_retain_prior_versions_in_real_database_and_conflicting_edit_is_invalid() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let first = DefinitionInput {
        kind: DefinitionKind::Mode,
        id: "development".to_owned(),
        version: 1,
        body: json!({"name":"Development","purpose":"Develop software"}),
        references: vec![],
    };
    {
        let mut store = Store::open(&database).unwrap();
        execute(
            &mut store,
            "mode-v1",
            Command::Records(RecordsCommand::AddDefinition(first.clone())),
        );
        let mut next = first.clone();
        next.version = 2;
        next.body["purpose"] = json!("Develop carefully");
        execute(
            &mut store,
            "mode-v2",
            Command::Records(RecordsCommand::AddDefinition(next)),
        );
        let conflict = execute(
            &mut store,
            "old-mode-version",
            Command::Records(RecordsCommand::AddDefinition(first.clone())),
        );
        assert_eq!(conflict.result.disposition, Disposition::InvalidNoChange);
    }
    let store = Store::open(&database).unwrap();
    let saved = store.load().unwrap();
    assert_eq!(saved.records().definitions().len(), 2);
    assert_eq!(saved.records().definition(&first.reference()), Some(&first));
    assert_eq!(store.event_count().unwrap(), 2);
    assert_eq!(store.command_count().unwrap(), 3);
}

#[test]
fn supported_secrets_are_removed_before_any_text_reaches_sqlite_or_wal() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let secrets = [
        "sk-syntheticOpenAISecret123456",
        "ghp_syntheticGithubSecret123456",
        "synthetic-bearer-credential",
        "synthetic-api-key-value",
    ];
    let content = format!(
        "Please inspect {} and {}\nAuthorization: Bearer {}\napi_key={}",
        secrets[0], secrets[1], secrets[2], secrets[3]
    );
    {
        let mut store = Store::open(&database).unwrap();
        execute(
            &mut store,
            "secret-message",
            Command::Send {
                conversation: ConversationId::new("private-conversation").unwrap(),
                project: None,
                content: content.clone(),
            },
        );
        execute(
            &mut store,
            "secret-draft",
            Command::SaveDraft {
                conversation: ConversationId::new("private-conversation").unwrap(),
                content: content.clone(),
            },
        );
        let saved = store.load().unwrap();
        let conversation = saved
            .conversations()
            .conversation(&ConversationId::new("private-conversation").unwrap())
            .unwrap();
        let persisted = conversation.messages()[0].content();
        for secret in secrets {
            assert!(!persisted.contains(secret));
        }
        let connection = rusqlite::Connection::open(&database).unwrap();
        let messages: String = connection
            .query_row(
                "SELECT group_concat(content,' ') FROM messages",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let drafts: String = connection
            .query_row(
                "SELECT group_concat(content,' ') FROM saved_drafts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        for secret in secrets {
            assert!(!messages.contains(secret));
            assert!(!drafts.contains(secret));
        }
    }
    for entry in std::fs::read_dir(&directory.0).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        for secret in secrets {
            assert!(
                !bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()),
                "plaintext secret persisted in {:?}",
                path.file_name()
            );
        }
    }
    let store = Store::open(&database).unwrap();
    assert_eq!(
        store
            .load()
            .unwrap()
            .conversations()
            .conversations()
            .count(),
        1
    );
}

#[test]
fn file_open_and_privacy_limits_reject_explicitly_without_business_effects() {
    assert!(matches!(
        Store::open(Path::new("relative.sqlite")),
        Err(StoreError::InvalidPath)
    ));
    let directory = TestDirectory::new();
    let database = directory.database();
    let mut store = Store::open(&database).unwrap();
    let request = Request {
        scope: LocalRef::new("tests").unwrap(),
        key: LocalRef::new("oversized").unwrap(),
        expected_revision: 0,
        command: Command::Send {
            conversation: ConversationId::new("conversation").unwrap(),
            project: None,
            content: "x".repeat(1_048_577),
        },
    };
    assert!(matches!(
        store.execute(&request),
        Err(StoreError::Privacy(_))
    ));
    assert_eq!(store.load().unwrap().revision(), 0);
    assert_eq!(store.command_count().unwrap(), 0);
    assert_eq!(store.event_count().unwrap(), 0);
    assert_eq!(
        store
            .load()
            .unwrap()
            .conversations()
            .conversations()
            .count(),
        0
    );
}

#[test]
fn credential_shaped_scope_or_idempotency_key_is_rejected_before_sqlite_write() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let mut store = Store::open(&database).unwrap();
    for secret_in_scope in [false, true] {
        let secret = "sk-syntheticCommandIdentitySecret123456";
        let request = Request {
            scope: LocalRef::new(if secret_in_scope {
                secret
            } else {
                "safe-scope"
            })
            .unwrap(),
            key: LocalRef::new(if secret_in_scope { "safe-key" } else { secret }).unwrap(),
            expected_revision: 0,
            command: Command::Send {
                conversation: ConversationId::new("conversation").unwrap(),
                project: None,
                content: "An ordinary message".to_owned(),
            },
        };
        assert!(store.execute(&request).is_err());
        assert_eq!(store.load().unwrap().revision(), 0);
        assert_eq!(store.command_count().unwrap(), 0);
        assert_eq!(store.event_count().unwrap(), 0);
    }
}

#[test]
fn message_idempotency_replays_original_receipt_after_reopen_without_duplicate_history() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let make_request = |content: &str| Request {
        scope: LocalRef::new("conversation-commands").unwrap(),
        key: LocalRef::new("first-message").unwrap(),
        expected_revision: 0,
        command: Command::Send {
            conversation: ConversationId::new("conversation-1").unwrap(),
            project: None,
            content: content.to_owned(),
        },
    };
    let original;
    {
        let mut store = Store::open(&database).unwrap();
        original = store
            .execute(&make_request("An initial instruction"))
            .unwrap();
        let replay = store
            .execute(&make_request("An initial instruction"))
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.result, original.result);
    }
    let mut store = Store::open(&database).unwrap();
    let replay = store
        .execute(&make_request("An initial instruction"))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.result, original.result);
    assert_eq!(
        store
            .execute(&make_request("Different instruction"))
            .unwrap_err(),
        StoreError::IdempotencyConflict
    );
    let saved = store.load().unwrap();
    let conversation = saved
        .conversations()
        .conversation(&ConversationId::new("conversation-1").unwrap())
        .unwrap();
    assert_eq!(conversation.messages().len(), 1);
    assert_eq!(store.command_count().unwrap(), 1);
    assert_eq!(store.event_count().unwrap(), 1);
}

#[test]
fn inconsistent_message_projection_is_rejected_on_reopen_without_silent_repair() {
    let directory = TestDirectory::new();
    let database = directory.database();
    {
        let mut store = Store::open(&database).unwrap();
        execute(
            &mut store,
            "message",
            Command::Send {
                conversation: ConversationId::new("conversation-1").unwrap(),
                project: None,
                content: "Original canonical message".to_owned(),
            },
        );
    }
    {
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "UPDATE messages SET content=?1",
                ["Inconsistent projection"],
            )
            .unwrap();
    }
    assert!(matches!(
        Store::open(&database),
        Err(StoreError::InvalidState)
    ));
    let connection = rusqlite::Connection::open(&database).unwrap();
    let content: String = connection
        .query_row("SELECT content FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        content, "Inconsistent projection",
        "a rejected database is not silently repaired"
    );
}
