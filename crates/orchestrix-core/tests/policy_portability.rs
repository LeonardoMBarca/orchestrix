use orchestrix_core::{
    desktop::{DesktopCommand, Settings},
    domain::LocalRef,
    persistence::{Command, FaultPoint, Request, Store, StoreError},
    policy::*,
    policy_portability::*,
};
use serde_json::{Value, json};

fn store() -> Store {
    let folder = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "orchestrix-portable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&folder).unwrap();
    Store::open(folder.join("state.sqlite")).unwrap()
}
fn save(store: &mut Store, settings: Settings, key: &str) {
    let revision = store.load().unwrap().revision();
    store
        .execute(&Request {
            scope: LocalRef::new("test").unwrap(),
            key: LocalRef::new(key).unwrap(),
            expected_revision: revision,
            command: Command::Desktop(DesktopCommand::SaveSettings {
                settings: Box::new(settings),
            }),
        })
        .unwrap();
}
fn import_request(store: &Store, document: String, mappings: ImportMappings, key: &str) -> Request {
    let preview = store.portable_policy_preview(&document, &mappings).unwrap();
    assert!(preview.can_apply, "{:?}", preview.issues);
    Request {
        scope: LocalRef::new("portable").unwrap(),
        key: LocalRef::new(key).unwrap(),
        expected_revision: preview.expected_revision,
        command: Command::Desktop(DesktopCommand::ImportPolicy {
            document,
            mappings,
            expected_preview_digest: preview.preview_digest,
        }),
    }
}
fn current(store: &Store) -> Settings {
    serde_json::from_value(store.desktop_snapshot().unwrap()["settings"].clone()).unwrap()
}

#[test]
fn allowlist_excludes_private_content_authorizations_and_binding_identifiers() {
    let mut s = Settings {
        system_instructions: "PRIVATE TRANSCRIPT AND NOTES".into(),
        ..Default::default()
    };
    s.policy.mode_defaults.connection_id = Some("private-account".into());
    s.policy.mode_defaults.model_id = Some("model-code".into());
    s.policy.mode_defaults.billing = Some(BillingMode::ExplicitApi);
    s.policy.mode_defaults.billing_authorization_ref = Some("private-execution-consent".into());
    s.policy.layers.push(PolicyLayer {
        scope: Scope::Project,
        reference: "private-project-ref".into(),
        revision: 99,
        preferences: Preferences::default(),
        requirements: Requirements::default(),
    });
    let bundle = export(&s).unwrap();
    let text = serde_json::to_string(&bundle).unwrap();
    for forbidden in [
        "PRIVATE",
        "private-account",
        "private-execution-consent",
        "private-project-ref",
        "systemInstructions",
        "billingAuthorization",
        "authStore",
        "diagnostic",
        "transcript",
        "consent",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    assert_eq!(bundle.connection_references[0].alias, "connection-1");
    assert_eq!(bundle.model_references[0].source_model_id, "model-code");
    assert_eq!(bundle.policy.layers[0].scope, Scope::Project);
}
#[test]
fn four_strategies_presets_overrides_and_appearance_roundtrip_to_new_store() {
    for strategy in [
        Strategy::Efficiency,
        Strategy::Performance,
        Strategy::Balanced,
        Strategy::Custom,
    ] {
        let mut source = Settings {
            language: "es".into(),
            theme: "medieval".into(),
            ..Default::default()
        };
        source.policy.mode_defaults.strategy = Some(strategy);
        source.policy.profile_defaults.preset = Some(SpecializedPreset::ScientificResearch);
        source.policy.layers.push(PolicyLayer {
            scope: Scope::Task,
            reference: "source-only-ref".into(),
            revision: 11,
            preferences: Preferences {
                context_bytes: Some(9000),
                reasoning: Some(Reasoning::High),
                ..Default::default()
            },
            requirements: Requirements {
                require_review: true,
                ..Default::default()
            },
        });
        let document = serde_json::to_string(&export(&source).unwrap()).unwrap();
        let mut destination = store();
        let before = destination.desktop_snapshot().unwrap();
        let request = import_request(&destination, document, ImportMappings::default(), "import");
        let result = destination.execute(&request).unwrap();
        assert!(!result.replayed);
        let s = current(&destination);
        assert_eq!(s.policy.mode_defaults.strategy, Some(strategy));
        assert_eq!(
            s.policy.profile_defaults.preset,
            Some(SpecializedPreset::ScientificResearch)
        );
        assert_eq!(s.policy.layers[0].preferences.context_bytes, Some(9000));
        assert_eq!(s.policy.revision, 2);
        assert_eq!(s.theme, "medieval");
        assert_eq!(s.language, "es");
        assert_eq!(s.policy.layers[0].revision, 1);
        assert_eq!(
            before["conversations"],
            destination.desktop_snapshot().unwrap()["conversations"]
        );
        assert!(destination.load().unwrap().workers().is_empty());
        assert!(
            destination.connection_list().unwrap()["connections"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(destination.execute(&request).unwrap().replayed);
        let path = destination.path().to_path_buf();
        drop(destination);
        assert_eq!(current(&Store::open(path).unwrap()).policy.revision, 2);
    }
}
#[test]
fn invalid_future_unknown_duplicate_and_oversize_documents_are_rejected() {
    let document = serde_json::to_value(export(&Settings::default()).unwrap()).unwrap();
    let mut future = document.clone();
    future["schemaVersion"] = json!(2);
    assert_eq!(parse(&future.to_string()).unwrap_err(), "portable-version");
    for field in [
        "credentials",
        "systemInstructions",
        "refreshConsent",
        "authStore",
    ] {
        let mut invalid = document.clone();
        invalid[field] = json!("forbidden");
        assert_eq!(parse(&invalid.to_string()).unwrap_err(), "portable-schema");
    }
    let mut inner = document.clone();
    inner["policy"]["modeDefaults"]["billingAuthorizationRef"] = json!("grant");
    assert!(parse(&inner.to_string()).is_err());
    assert_eq!(
        parse(&" ".repeat(MAX_PORTABLE_POLICY_BYTES + 1)).unwrap_err(),
        "portable-size-limit"
    );
    let duplicate = document.to_string().replacen(
        "\"schemaVersion\":1",
        "\"schemaVersion\":1,\"schemaVersion\":1",
        1,
    );
    assert!(parse(&duplicate).is_err());
}
#[test]
fn unresolved_connection_and_model_references_block_without_matching_names() {
    let mut s = Settings::default();
    s.policy.mode_defaults.connection_id = Some("same-textual-name".into());
    s.policy.mode_defaults.model_id = Some("same-model-name".into());
    let document = serde_json::to_string(&export(&s).unwrap()).unwrap();
    let destination = store();
    let before = destination.desktop_snapshot().unwrap();
    let preview = destination
        .portable_policy_preview(&document, &ImportMappings::default())
        .unwrap();
    assert!(!preview.can_apply);
    assert!(
        preview
            .issues
            .iter()
            .any(|i| i.code == "portable-connection-unresolved")
    );
    assert_eq!(before, destination.desktop_snapshot().unwrap());
    let mappings = ImportMappings {
        connections: vec![ConnectionMapping {
            alias: "connection-1".into(),
            connection_id: "same-textual-name".into(),
            binding_revision: 1,
        }],
        models: vec![ModelMapping {
            alias: "model-1".into(),
            connection_alias: "connection-1".into(),
            model_id: "same-model-name".into(),
        }],
        ..Default::default()
    };
    let preview = destination
        .portable_policy_preview(&document, &mappings)
        .unwrap();
    assert!(!preview.can_apply);
    assert!(
        preview
            .issues
            .iter()
            .any(|i| i.code == "portable-connection-not-found")
    );
    assert_eq!(before, destination.desktop_snapshot().unwrap());
}
#[test]
fn all_local_mandatory_scopes_and_personal_instructions_survive_import() {
    let mut destination = store();
    let mut local = Settings {
        system_instructions: "LOCAL STYLE".into(),
        ..Default::default()
    };
    local.policy.revision = 17;
    local.policy.observation_ttls.availability_ms = 500;
    local.policy.layers.push(PolicyLayer {
        scope: Scope::System,
        reference: "local-security".into(),
        revision: 1,
        preferences: Preferences::default(),
        requirements: Requirements {
            forbid_network: true,
            read_only: true,
            require_review: true,
            max_concurrency: Some(1),
            max_context_bytes: Some(4096),
            minimum_reasoning: Some(Reasoning::High),
            required_checks: ["local-check".into()].into(),
            ..Default::default()
        },
    });
    save(&mut destination, local, "save-local");
    let mut imported = Settings::default();
    imported.policy.mode_defaults.network = Some(true);
    imported.policy.mode_requirements.subscription_only = false;
    let request = import_request(
        &destination,
        serde_json::to_string(&export(&imported).unwrap()).unwrap(),
        ImportMappings::default(),
        "import-local",
    );
    destination.execute(&request).unwrap();
    let result = current(&destination);
    assert_eq!(result.system_instructions, "LOCAL STYLE");
    assert_eq!(result.policy.revision, 18);
    assert_eq!(result.policy.observation_ttls.availability_ms, 500);
    let r = result.policy.mode_requirements;
    assert!(r.subscription_only && r.forbid_network && r.read_only && r.require_review);
    assert_eq!(r.max_context_bytes, Some(4096));
    assert_eq!(r.minimum_reasoning, Some(Reasoning::High));
    assert!(r.required_checks.contains("local-check"));
    assert_eq!(
        result.policy.mode_defaults.billing,
        Some(BillingMode::SubscriptionOnly)
    );
    assert!(
        result
            .policy
            .mode_defaults
            .billing_authorization_ref
            .is_none()
    );
}
#[test]
fn stale_revision_wrong_preview_and_transaction_fault_preserve_prior_settings() {
    let mut destination = store();
    let imported = Settings {
        theme: "dawn".into(),
        ..Default::default()
    };
    let document = serde_json::to_string(&export(&imported).unwrap()).unwrap();
    let request = import_request(
        &destination,
        document.clone(),
        ImportMappings::default(),
        "import-stale",
    );
    save(&mut destination, Settings::default(), "new-local");
    assert!(matches!(
        destination.execute(&request),
        Err(StoreError::StaleRevision)
    ));
    let mut wrong = import_request(
        &destination,
        document.clone(),
        ImportMappings::default(),
        "import-wrong",
    );
    if let Command::Desktop(DesktopCommand::ImportPolicy {
        expected_preview_digest,
        ..
    }) = &mut wrong.command
    {
        *expected_preview_digest = "0".repeat(64);
    }
    assert!(destination.execute(&wrong).is_err());
    assert_eq!(current(&destination).theme, "studio");
    for (i, fault) in [
        FaultPoint::AfterState,
        FaultPoint::AfterEvent,
        FaultPoint::AfterResult,
    ]
    .into_iter()
    .enumerate()
    {
        let request = import_request(
            &destination,
            document.clone(),
            ImportMappings::default(),
            &format!("fault-{i}"),
        );
        assert!(matches!(
            destination.execute_with_fault(&request, fault),
            Err(StoreError::InjectedRollback)
        ));
        assert_eq!(current(&destination).theme, "studio");
    }
    let request = import_request(
        &destination,
        document,
        ImportMappings::default(),
        "import-lost",
    );
    assert!(matches!(
        destination.execute_with_fault(&request, FaultPoint::AfterCommit),
        Err(StoreError::ReplyLost)
    ));
    assert_eq!(current(&destination).theme, "dawn");
    assert!(destination.execute(&request).unwrap().replayed);
}
#[test]
fn explicit_definition_remapping_is_required_for_nonbuiltin_references() {
    let mut source = Settings::default();
    source.policy.mode.id = "private-mode".into();
    let doc = serde_json::to_string(&export(&source).unwrap()).unwrap();
    let destination = store();
    assert!(
        !destination
            .portable_policy_preview(&doc, &ImportMappings::default())
            .unwrap()
            .can_apply
    );
    let mapping = ImportMappings {
        mode: Some(Settings::default().policy.mode),
        ..Default::default()
    };
    assert!(
        destination
            .portable_policy_preview(&doc, &mapping)
            .unwrap()
            .can_apply
    );
    let future: Value = serde_json::from_str(&doc).unwrap();
    assert_eq!(future["policy"]["mode"]["id"], "private-mode");
}

#[test]
fn separate_account_model_ranks_keep_distinct_remapping_references() {
    let mut settings = Settings::default();
    settings.policy.mode_defaults.connection_id = Some("account-b".into());
    settings.policy.mode_defaults.model_id = Some("shared-name".into());
    settings.policy.model_ranks = vec![
        ModelRank {
            connection_id: "account-a".into(),
            model_id: "shared-name".into(),
            strength: 1,
            efficiency_order: 1,
            source_ref: "private-rank-a".into(),
        },
        ModelRank {
            connection_id: "account-b".into(),
            model_id: "shared-name".into(),
            strength: 3,
            efficiency_order: 2,
            source_ref: "private-rank-b".into(),
        },
    ];
    let bundle = export(&settings).unwrap();
    assert_eq!(bundle.connection_references.len(), 2);
    assert_eq!(bundle.model_references.len(), 3);
    let ranks = &bundle.policy.model_ranks;
    assert_ne!(ranks[0].model, ranks[1].model);
    for rank in ranks {
        assert_eq!(
            bundle
                .model_references
                .iter()
                .find(|m| m.alias == rank.model)
                .unwrap()
                .connection_alias
                .as_ref(),
            Some(&rank.connection)
        );
    }
    assert!(parse(&serde_json::to_string(&bundle).unwrap()).is_ok());
}

#[test]
fn cancelling_valid_preview_leaves_settings_receipts_and_history_untouched() {
    let destination = store();
    let before = destination.desktop_snapshot().unwrap();
    let count = destination.command_count().unwrap();
    let source = Settings {
        language: "pt-BR".into(),
        ..Default::default()
    };
    let document = serde_json::to_string(&export(&source).unwrap()).unwrap();
    assert!(
        destination
            .portable_policy_preview(&document, &ImportMappings::default())
            .unwrap()
            .can_apply
    );
    assert_eq!(before, destination.desktop_snapshot().unwrap());
    assert_eq!(count, destination.command_count().unwrap());
    assert!(destination.load().unwrap().workers().is_empty());
}
