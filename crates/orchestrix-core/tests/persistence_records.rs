use orchestrix_core::domain::ConnectionId;
use orchestrix_core::persistence_privacy::sanitize_checkpoint;
use orchestrix_core::persistence_privacy::{OMITTED, REDACTED, sanitize_json, sanitize_text};
use orchestrix_core::persistence_records::*;
use orchestrix_core::runtime_contract::{
    EventOrigin, Limits, RuntimeObserver, normalize_descriptor, normalize_request,
};
use serde_json::{Value, json};

fn definition(kind: DefinitionKind, id: &str, body: Value) -> DefinitionInput {
    DefinitionInput {
        kind,
        id: id.to_owned(),
        version: 1,
        body,
        references: vec![],
    }
}
fn seeded() -> DocumentRecords {
    let mut records = DocumentRecords::default();
    for input in [
        definition(
            DefinitionKind::Mode,
            "development",
            json!({"name":"Development","purpose":"Develop software"}),
        ),
        definition(
            DefinitionKind::Profile,
            "coder",
            json!({"name":"Coder","role":"Implementation","instructions_ref":"instructions-1"}),
        ),
        definition(
            DefinitionKind::Context,
            "context-1",
            json!({"manifest_ref":"manifest-1"}),
        ),
        definition(
            DefinitionKind::Configuration,
            "policy-1",
            json!({"strategy":"balanced","billing":"subscription-only"}),
        ),
    ] {
        records.add_definition(input).unwrap();
    }
    records
}
fn reference(kind: DefinitionKind, id: &str) -> DefinitionRef {
    DefinitionRef {
        kind,
        id: id.to_owned(),
        version: 1,
    }
}
fn snapshot() -> SnapshotInput {
    SnapshotInput {
        id: "snapshot-1".to_owned(),
        attempt_id: "attempt-1".to_owned(),
        mode: reference(DefinitionKind::Mode, "development"),
        profile: reference(DefinitionKind::Profile, "coder"),
        context: reference(DefinitionKind::Context, "context-1"),
        configuration: reference(DefinitionKind::Configuration, "policy-1"),
    }
}
fn connection() -> ConnectionId {
    ConnectionId::new("connection-1").unwrap()
}
fn scope() -> DiagnosticScope {
    DiagnosticScope {
        kind: DiagnosticScopeKind::Model,
        reference: Some("model-1".to_owned()),
    }
}
fn diagnostic() -> DiagnosticInput {
    DiagnosticInput {
        id: "diagnostic-1".to_owned(),
        connection_id: "connection-1".to_owned(),
        connection_revision: 1,
        version: 1,
        scope: scope(),
        source: DiagnosticSource::Catalog,
        outcome: DiagnosticOutcome::Success,
        checked_at: Some("2026-10-14T12:00:00.000Z".to_owned()),
        valid_until: Some("2026-10-14T13:00:00.000Z".to_owned()),
        complete: false,
        values: json!({"support":"supported","availability":"unknown","catalog_complete":false}),
        error_code: None,
    }
}

#[test]
fn definitions_and_snapshots_restore_exact_versions_after_edits() {
    let mut records = seeded();
    records.add_snapshot(snapshot()).unwrap();
    let prior = records
        .definition(&reference(DefinitionKind::Mode, "development"))
        .unwrap()
        .clone();
    let mut revised = prior.clone();
    revised.version = 2;
    revised.body["purpose"] = json!("A different objective");
    records
        .apply(&RecordsCommand::AddDefinition(revised))
        .unwrap();
    let restored = DocumentRecords::restore(&records.checkpoint()).unwrap();
    assert_eq!(restored.definition(&snapshot().mode), Some(&prior));
    assert_eq!(restored.snapshots(), &[snapshot()]);
    assert_eq!(restored.definitions().len(), 5);
}

#[test]
fn definitions_reject_duplicate_versions_invalid_ids_unknown_links_without_mutation() {
    let mut records = seeded();
    let before = records.checkpoint();
    let duplicate = records.definitions()[0].clone();
    assert_eq!(
        records.add_definition(duplicate),
        Err(RecordError::Conflict)
    );
    let mut bad = definition(DefinitionKind::Configuration, "bad/path", json!({}));
    assert_eq!(
        records.add_definition(bad.clone()),
        Err(RecordError::Invalid)
    );
    bad.id = "good".to_owned();
    bad.references
        .push(reference(DefinitionKind::Mode, "absent"));
    assert_eq!(
        records.add_definition(bad),
        Err(RecordError::UnknownReference)
    );
    assert_eq!(records.checkpoint(), before);
}

#[test]
fn mode_profile_validation_reuses_domain_and_snapshot_kinds_are_guarded() {
    let mut records = seeded();
    let blank = definition(
        DefinitionKind::Mode,
        "blank",
        json!({"name":" ","purpose":"Plan"}),
    );
    assert_eq!(records.add_definition(blank), Err(RecordError::Invalid));
    let extra = definition(
        DefinitionKind::Profile,
        "extra",
        json!({"name":"A","role":"B","instructions_ref":"ref","private_state":true}),
    );
    assert_eq!(records.add_definition(extra), Err(RecordError::Invalid));
    let mut wrong = snapshot();
    wrong.mode = wrong.configuration.clone();
    assert_eq!(records.add_snapshot(wrong), Err(RecordError::Invalid));
    let mut absent = snapshot();
    absent.context.version = 2;
    assert_eq!(
        records.add_snapshot(absent),
        Err(RecordError::UnknownReference)
    );
    records.add_snapshot(snapshot()).unwrap();
    assert_eq!(records.add_snapshot(snapshot()), Err(RecordError::Conflict));
}

#[test]
fn failed_refresh_preserves_original_success_date_validity_and_partiality_after_restore() {
    let mut records = seeded();
    records.register_connection(&connection(), 1).unwrap();
    let successful = diagnostic();
    records.record_diagnostic(successful.clone()).unwrap();
    let mut failed = successful.clone();
    failed.id = "diagnostic-2".to_owned();
    failed.version = 2;
    failed.outcome = DiagnosticOutcome::Failed;
    failed.checked_at = Some("2026-10-14T12:30:00.000Z".to_owned());
    failed.valid_until = None;
    failed.values = Value::Null;
    failed.error_code = Some("timeout".to_owned());
    records.record_diagnostic(failed.clone()).unwrap();
    let restored = DocumentRecords::restore(&records.checkpoint()).unwrap();
    let head = restored.diagnostic_head(&connection(), &scope()).unwrap();
    assert_eq!(head.latest(), &failed);
    assert_eq!(head.last_success(), Some(&successful));
    assert!(
        head.has_valid_success_at("2026-10-14T12:40:00.000Z")
            .unwrap()
    );
    assert!(
        !head
            .has_valid_success_at("2026-10-14T13:00:00.000Z")
            .unwrap()
    );
    assert!(
        !head
            .has_valid_success_at("2026-10-14T11:59:59.000Z")
            .unwrap()
    );
    assert_eq!(restored.diagnostics().len(), 2);
}

#[test]
fn revision_change_invalidates_head_and_rejects_old_responses_preserving_history() {
    let mut records = DocumentRecords::default();
    records.register_connection(&connection(), 1).unwrap();
    records.record_diagnostic(diagnostic()).unwrap();
    records.register_connection(&connection(), 2).unwrap();
    let before = records.checkpoint();
    let mut late = diagnostic();
    late.id = "late".to_owned();
    late.version = 2;
    assert_eq!(
        records.record_diagnostic(late),
        Err(RecordError::StaleRevision)
    );
    assert_eq!(records.checkpoint(), before);
    assert!(records.diagnostic_head(&connection(), &scope()).is_none());
    let mut current = diagnostic();
    current.connection_revision = 2;
    current.version = 2;
    current.id = "new-revision".to_owned();
    records.record_diagnostic(current.clone()).unwrap();
    let restored = DocumentRecords::restore(&records.checkpoint()).unwrap();
    assert_eq!(
        restored
            .diagnostic_head(&connection(), &scope())
            .unwrap()
            .last_success(),
        Some(&current)
    );
    assert!(restored.diagnostic_exists("diagnostic-1"));
    assert_eq!(restored.diagnostics().len(), 2);
    assert_eq!(
        records.register_connection(&connection(), 1),
        Err(RecordError::StaleRevision)
    );
}

#[test]
fn undated_success_is_not_assigned_an_invented_timestamp_or_validity() {
    let mut records = DocumentRecords::default();
    records.register_connection(&connection(), 1).unwrap();
    let mut input = diagnostic();
    input.checked_at = None;
    input.valid_until = None;
    records.record_diagnostic(input).unwrap();
    let head = records.diagnostic_head(&connection(), &scope()).unwrap();
    assert_eq!(head.last_success().unwrap().checked_at, None);
    assert!(
        !head
            .has_valid_success_at("2026-10-14T12:00:00.000Z")
            .unwrap()
    );
}

#[test]
fn diagnostic_rejects_bad_dates_scope_versions_and_failure_capability_mutation() {
    let mut records = DocumentRecords::default();
    records.register_connection(&connection(), 1).unwrap();
    for edit in [0, 1, 2, 3, 4] {
        let mut input = diagnostic();
        match edit {
            0 => input.checked_at = Some("2026-10-14T12:00:00".to_owned()),
            1 => input.valid_until = input.checked_at.clone(),
            2 => input.scope.reference = None,
            3 => input.version = 0,
            _ => input.outcome = DiagnosticOutcome::Failed,
        }
        assert_eq!(records.record_diagnostic(input), Err(RecordError::Invalid));
    }
    records.record_diagnostic(diagnostic()).unwrap();
    let mut duplicate = diagnostic();
    duplicate.id = "different-id".to_owned();
    assert_eq!(
        records.record_diagnostic(duplicate),
        Err(RecordError::Conflict)
    );
}

#[test]
fn restore_revalidates_and_rejects_tampered_raw_records_without_private_state_setters() {
    let mut records = seeded();
    records.add_snapshot(snapshot()).unwrap();
    for edit in [0, 1, 2, 3] {
        let mut raw = records.checkpoint();
        match edit {
            0 => raw["schema_version"] = json!(99),
            1 => raw["definitions"][0]["version"] = json!(0),
            2 => raw["snapshots"][0]["mode"]["version"] = json!(99),
            _ => raw["private_state"] = json!("invalid"),
        }
        assert!(DocumentRecords::restore(&raw).is_err());
    }
    let mut raw = records.checkpoint();
    raw["definitions"][3]["body"]["api_key"] = json!("synthetic-sensitive");
    assert!(
        DocumentRecords::restore(&raw).is_err(),
        "restore rejects unnormalized secrets instead of silently repairing stored data"
    );
}

#[test]
fn central_privacy_redacts_supported_patterns_nested_keys_transcripts_and_unicode_safely() {
    let secrets = [
        "sk-syntheticOpenAISecret123456",
        "ghp_syntheticGitHubSecret123456",
        "github_pat_syntheticSecret123456",
        "AIzaSyntheticGoogleSecret123456",
        "AKIASYNTHETICSECRET123456",
        "eyJsyntheticHeader.eyJsyntheticPayload.syntheticSignature",
    ];
    for secret in secrets {
        let text = format!("Descrição com acentuação 日本語 {secret} fim");
        let sanitized = sanitize_text(&text).unwrap();
        assert!(!sanitized.contains(secret));
        assert!(sanitized.contains(REDACTED));
        assert!(sanitized.contains("日本語"));
        assert_eq!(sanitize_text(&sanitized).unwrap(), sanitized);
    }
    for text in [
        "Authorization: Bearer synthetic-auth-secret rest",
        "api_key='synthetic key with spaces'",
        "TOKEN=synthetic-token-secret",
        "password: synthetic-password",
        "Bearer synthetic-bearer-secret",
        "-----BEGIN PRIVATE KEY-----\nsynthetic-key\n-----END PRIVATE KEY-----",
    ] {
        assert!(
            !sanitize_text(text).unwrap().contains("synthetic"),
            "{text}"
        );
    }
    let input = json!({"headers":{"Authorization":"sensitive"},"nested":[{"client_secret":"sensitive","result":"safe"}],"raw_transcript":{"turns":["sensitive"]},"note":"safe"});
    let output = sanitize_json(&input).unwrap();
    assert!(!output.to_string().contains("sensitive"));
    assert_eq!(output["raw_transcript"], OMITTED);
    assert_eq!(output["headers"]["Authorization"], REDACTED);
    assert_eq!(output["nested"][0]["result"], "safe");
    assert_eq!(sanitize_json(&output).unwrap(), output);
}

#[test]
fn persisted_records_only_emit_sanitized_values_and_limits_are_explicit() {
    let mut records = DocumentRecords::default();
    records.add_definition(definition(DefinitionKind::Configuration,"configuration-1",json!({"api_key":"synthetic-private-key","description":"Bearer synthetic-bearer-value","rawTranscript":["synthetic-transcript"]}))).unwrap();
    let raw = records.checkpoint().to_string();
    assert!(!raw.contains("synthetic"));
    assert!(sanitize_text(&"x".repeat(1_048_577)).is_err());
    let mut nested = Value::Null;
    for _ in 0..34 {
        nested = json!({"nested":nested});
    }
    assert!(sanitize_json(&nested).is_err());
    assert!(sanitize_json(&json!(vec![0; 4097])).is_err());
    assert!(DocumentRecords::restore(&records.checkpoint()).is_ok());
}

#[test]
fn aggregate_checkpoint_supports_complete_observer_journal_without_weakening_command_limits() {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap();
    let descriptor = normalize_descriptor(&fixtures["descriptors"]["base"]).unwrap();
    let request = normalize_request(&fixtures["requests"]["base"]).unwrap();
    let mut observer = RuntimeObserver::new(descriptor, request, Limits::default()).unwrap();
    for sequence in 1..=256 {
        let frame = json!({"version":{"major":1,"minor":1},"connectionId":"connection-1","connectionRevision":1,"sessionId":"session-1","attemptId":"attempt-1","sequence":sequence,"type":"future.metadata"});
        observer
            .accept(&frame.to_string(), EventOrigin::Runtime)
            .unwrap();
    }
    let checkpoint = observer.checkpoint();
    assert!(
        sanitize_json(&checkpoint).is_err(),
        "single-command/document budget remains constrained"
    );
    let sanitized = sanitize_checkpoint(&checkpoint).unwrap();
    assert_eq!(sanitized, checkpoint);
    let restored = RuntimeObserver::restore_checkpoint(&sanitized).unwrap();
    assert_eq!(restored.snapshot(), observer.snapshot());
    assert_eq!(restored.snapshot()["budget"]["frames"], 256);
}

#[test]
fn aggregate_checkpoint_has_explicit_depth_and_individual_text_bounds_with_identical_redaction() {
    let entry = json!({"text":"Bearer synthetic-aggregate-secret","api_key":"synthetic-key","rawTranscript":["synthetic-transcript"]});
    let input = json!({"safe":[entry.clone(),entry]});
    assert_eq!(
        sanitize_checkpoint(&input).unwrap(),
        sanitize_json(&input).unwrap()
    );
    assert!(sanitize_checkpoint(&json!({"text":"x".repeat(1_048_577)})).is_err());
    let escaped = json!({"text":"\"".repeat(524_300)});
    assert!(
        sanitize_json(&escaped).is_err(),
        "encoded bytes, including escaping, count against the document budget"
    );
    assert!(sanitize_checkpoint(&escaped).is_ok());
    let mut nested = Value::Null;
    for _ in 0..129 {
        nested = json!({"nested":nested});
    }
    assert!(sanitize_checkpoint(&nested).is_err());
}
