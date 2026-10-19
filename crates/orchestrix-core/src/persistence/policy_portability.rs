//! Portable policy reads and transaction-local binding checks. Never launches a runtime.
use super::{SavedState, Store, StoreError, StoreResult};
use crate::{desktop::DesktopCommand, policy_portability::*};
use rusqlite::Connection;
use serde_json::{Value, json};

fn mapping_issues(
    c: &Connection,
    bundle: &PortableBundle,
    mappings: &ImportMappings,
    now: u64,
) -> StoreResult<Vec<ImportIssue>> {
    if let Err(code) = validate_mappings(bundle, mappings) {
        return Ok(vec![ImportIssue {
            code,
            reference: None,
        }]);
    }
    let generation = c.query_row(
        "SELECT generation FROM daemon_meta WHERE singleton=1",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    let mut issues = Vec::new();
    for mapping in &mappings.connections {
        let link = super::connections::current(c, &mapping.connection_id)?;
        let Some(link) = link else {
            issues.push(ImportIssue {
                code: "portable-connection-not-found".into(),
                reference: Some(mapping.alias.clone()),
            });
            continue;
        };
        let code = link.guard_binding(mapping.binding_revision).err();
        if let Some(code) = code {
            issues.push(ImportIssue {
                code,
                reference: Some(mapping.alias.clone()),
            });
            continue;
        }
        let rows = super::connections::diagnostics(c, &mapping.connection_id)?;
        if rows
            .iter()
            .any(|d| d.connection_revision == link.binding_revision && d.identity_changed)
        {
            issues.push(ImportIssue {
                code: "portable-identity-reconfirmation-required".into(),
                reference: Some(mapping.alias.clone()),
            });
            continue;
        }
        let diagnostic = rows
            .iter()
            .filter(|d| {
                d.connection_revision == link.binding_revision
                    && d.daemon_generation == generation
                    && d.purpose == "registration"
                    && matches!(d.state.as_str(), "complete" | "partial")
                    && d.observed_at_ms.is_some_and(|t| t <= now)
                    && d.valid_until_ms.is_some_and(|t| now < t)
            })
            .max_by_key(|d| d.observed_at_ms);
        let Some(diagnostic) = diagnostic else {
            issues.push(ImportIssue {
                code: "portable-current-observation-required".into(),
                reference: Some(mapping.alias.clone()),
            });
            continue;
        };
        let validity = super::connections::diagnostic_effective_validity(c, diagnostic)?;
        if diagnostic
            .observed_at_ms
            .is_none_or(|observed| now >= observed.saturating_add(validity.minimum()))
        {
            issues.push(ImportIssue {
                code: "portable-current-observation-required".into(),
                reference: Some(mapping.alias.clone()),
            });
            continue;
        }
        let Some(descriptor) = &diagnostic.descriptor else {
            return Err(StoreError::InvalidState);
        };
        if descriptor["connection"]["authentication"] != "authenticated"
            || descriptor["connection"]["billing"] != "subscription"
            || descriptor["connection"]["availability"] != "available"
        {
            issues.push(ImportIssue {
                code: "portable-subscription-identity-required".into(),
                reference: Some(mapping.alias.clone()),
            });
            continue;
        }
        for model in mappings
            .models
            .iter()
            .filter(|m| m.connection_alias == mapping.alias)
        {
            if !bundle
                .model_references
                .iter()
                .any(|r| r.alias == model.alias)
            {
                continue;
            }
            let found = descriptor["models"]
                .as_array()
                .and_then(|models| models.iter().find(|m| m["id"] == model.model_id));
            match found {
                None => issues.push(ImportIssue {
                    code: "portable-model-not-observed".into(),
                    reference: Some(model.alias.clone()),
                }),
                Some(m) if m["availability"] != "available" => issues.push(ImportIssue {
                    code: "portable-model-availability-unconfirmed".into(),
                    reference: Some(model.alias.clone()),
                }),
                Some(observed) => {
                    for preference in [
                        &bundle.policy.mode_defaults,
                        &bundle.policy.profile_defaults,
                    ]
                    .into_iter()
                    .chain(bundle.policy.layers.iter().map(|l| &l.preferences))
                    {
                        if preference.model.as_deref() == Some(&model.alias)
                            && preference.reasoning.is_some_and(|r| {
                                observed["reasoningLevels"]
                                    .as_array()
                                    .is_none_or(|levels| !levels.iter().any(|v| v == r.as_str()))
                            })
                        {
                            issues.push(ImportIssue {
                                code: "portable-reasoning-not-observed".into(),
                                reference: Some(model.alias.clone()),
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(issues)
}
fn make_preview(
    c: &Connection,
    state: &SavedState,
    document: &str,
    mappings: &ImportMappings,
    now: u64,
) -> StoreResult<ImportPreview> {
    let bundle = parse(document).map_err(|_| StoreError::Configuration)?;
    let current = settings_from_state(state).map_err(|_| StoreError::InvalidState)?;
    let issues = mapping_issues(c, &bundle, mappings, now)?;
    Ok(crate::policy_portability::preview(
        &bundle,
        &current,
        mappings,
        state.revision(),
        issues,
    ))
}
/// Called from Store::execute while its single short writer transaction is held.
/// Revalidates time-sensitive identity/model evidence and exact preview before mutation.
pub(super) fn validate_import_command(
    c: &Connection,
    state: &SavedState,
    command: &super::Command,
) -> StoreResult<()> {
    let super::Command::Desktop(DesktopCommand::ImportPolicy {
        document,
        mappings,
        expected_preview_digest,
    }) = command
    else {
        return Ok(());
    };
    let preview = make_preview(c, state, document, mappings, crate::connections::now_ms())?;
    if !preview.can_apply || preview.preview_digest != *expected_preview_digest {
        return Err(StoreError::Configuration);
    }
    Ok(())
}
impl Store {
    pub fn portable_policy_export(&self) -> StoreResult<Value> {
        let state = self.load()?;
        let settings = settings_from_state(&state).map_err(|_| StoreError::InvalidState)?;
        let bundle = export(&settings).map_err(|_| StoreError::Configuration)?;
        let document = serde_json::to_string(&bundle).map_err(|_| StoreError::InvalidState)?;
        Ok(
            json!({"schemaVersion":1,"document":document,"bundle":bundle,"summary":{"connectionReferences":bundle.connection_references.len(),"modelReferences":bundle.model_references.len(),"layers":bundle.policy.layers.len(),"credentialsIncluded":false,"executionAuthorityIncluded":false,"recurringConsentIncluded":false,"personalInstructionsIncluded":false}}),
        )
    }
    pub fn portable_policy_preview(
        &self,
        document: &str,
        mappings: &ImportMappings,
    ) -> StoreResult<ImportPreview> {
        self.portable_policy_preview_at(document, mappings, crate::connections::now_ms())
    }
    pub fn portable_policy_preview_at(
        &self,
        document: &str,
        mappings: &ImportMappings,
        now: u64,
    ) -> StoreResult<ImportPreview> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let state = super::state::load(&self.connection)?;
            make_preview(&self.connection, &state, document, mappings, now)
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        connections::{ConnectionChange, ConnectionDiagnostic, ConnectionLink},
        daemon::RuntimeChoice,
        desktop::{DesktopCommand, Settings},
        domain::LocalRef,
        persistence::{Command, Request},
        policy::Strategy,
    };
    fn store() -> Store {
        let parent = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchestrix-portable-binding-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&parent).unwrap();
        Store::open(parent.join("state.sqlite")).unwrap()
    }
    fn registration(store: &mut Store, now: u64) -> ConnectionDiagnostic {
        let parent = store.path().parent().unwrap().to_path_buf();
        let link = ConnectionLink {
            schema_version: 1,
            id: "destination-account".into(),
            revision: 1,
            binding_revision: 1,
            name: "Same name is not identity".into(),
            enabled: true,
            linked: true,
            runtime: RuntimeChoice::Synthetic {
                executable: std::env::current_exe().unwrap(),
                home: parent.clone(),
                scenario: "connection-metadata".into(),
            },
            metadata_directory: parent,
            executable_sha256: "a".repeat(64),
            baseline_observation: Some("b".repeat(64)),
            identity_assurance: "synthetic".into(),
        };
        store
            .connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let mut d = ConnectionDiagnostic {
            schema_version: 1,
            id: "destination-diagnostic".into(),
            connection_id: link.id,
            connection_revision: 1,
            registration_revision: 1,
            daemon_generation: store.daemon_state().unwrap().generation,
            synthetic: true,
            state: "running".into(),
            started_at_ms: now - 10,
            finished_at_ms: None,
            observed_at_ms: None,
            valid_until_ms: None,
            next_allowed_at_ms: None,
            error_code: None,
            identity_assurance: "unknown".into(),
            observation_fingerprint: None,
            identity_changed: false,
            descriptor: None,
            catalog_labels: vec![],
            control_observations: vec![],
            structured_result_support: None,
            complete: false,
            cached: false,
            purpose: "registration".into(),
            scope_key: None,
        };
        store.connection_begin_diagnostic(&d, None).unwrap();
        d.state = "complete".into();
        d.finished_at_ms = Some(now);
        d.observed_at_ms = Some(now);
        d.valid_until_ms = Some(now + 60_000);
        d.scope_key = Some("c".repeat(64));
        d.observation_fingerprint = Some("b".repeat(64));
        d.identity_assurance = "synthetic".into();
        d.complete = true;
        d.descriptor = Some(
            json!({"version":{"major":1,"minor":1},"adapterId":"fixture","runtimeVersion":"synthetic-rust-1","connection":{"id":"destination-account","revision":1,"authentication":"authenticated","billing":"subscription","availability":"available"},"catalog":{"completeness":"complete"},"models":[{"id":"destination-model","availability":"available","reasoningLevels":["low","high"]}]}),
        );
        store.connection_record_diagnostic(&d).unwrap();
        d
    }
    fn document() -> String {
        let mut s = Settings::default();
        s.policy.mode_defaults.strategy = Some(Strategy::Custom);
        s.policy.mode_defaults.connection_id = Some("source-account".into());
        s.policy.mode_defaults.model_id = Some("source-model".into());
        serde_json::to_string(&export(&s).unwrap()).unwrap()
    }
    fn mappings() -> ImportMappings {
        ImportMappings {
            connections: vec![ConnectionMapping {
                alias: "connection-1".into(),
                connection_id: "destination-account".into(),
                binding_revision: 1,
            }],
            models: vec![ModelMapping {
                alias: "model-1".into(),
                connection_alias: "connection-1".into(),
                model_id: "destination-model".into(),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn persisted_current_registration_remaps_explicitly_without_rpc_and_imports_atomically() {
        let mut store = store();
        let now = crate::connections::now_ms();
        registration(&mut store, now);
        let document = document();
        let mappings = mappings();
        let preview = store
            .portable_policy_preview_at(&document, &mappings, now)
            .unwrap();
        assert!(preview.can_apply, "{:?}", preview.issues);
        let state_before = store.load().unwrap();
        assert!(state_before.workers().is_empty());
        let request = Request {
            scope: LocalRef::new("desktop").unwrap(),
            key: LocalRef::new("import-mapped").unwrap(),
            expected_revision: preview.expected_revision,
            command: Command::Desktop(DesktopCommand::ImportPolicy {
                document,
                mappings,
                expected_preview_digest: preview.preview_digest,
            }),
        };
        store.execute(&request).unwrap();
        let settings = settings_from_state(&store.load().unwrap()).unwrap();
        assert_eq!(
            settings.policy.mode_defaults.connection_id.as_deref(),
            Some("destination-account")
        );
        assert_eq!(
            settings.policy.mode_defaults.model_id.as_deref(),
            Some("destination-model")
        );
        assert_eq!(
            settings.policy.mode_defaults.strategy,
            Some(Strategy::Custom)
        );
        assert!(store.load().unwrap().workers().is_empty());
        assert_eq!(
            super::super::connections::diagnostics(&store.connection, "destination-account")
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn binding_revoked_after_preview_rejects_before_settings_write() {
        let mut store = store();
        let now = crate::connections::now_ms();
        registration(&mut store, now);
        let document = document();
        let mappings = mappings();
        let p = store
            .portable_policy_preview_at(&document, &mappings, now)
            .unwrap();
        assert!(p.can_apply);
        store
            .connection_update(
                "disable",
                &"2".repeat(64),
                "destination-account",
                1,
                &ConnectionChange::Disable,
            )
            .unwrap();
        let request = Request {
            scope: LocalRef::new("desktop").unwrap(),
            key: LocalRef::new("import-stale-binding").unwrap(),
            expected_revision: p.expected_revision,
            command: Command::Desktop(DesktopCommand::ImportPolicy {
                document,
                mappings,
                expected_preview_digest: p.preview_digest,
            }),
        };
        assert!(store.execute(&request).is_err());
        assert_eq!(
            settings_from_state(&store.load().unwrap())
                .unwrap()
                .policy
                .mode_defaults
                .connection_id,
            None
        );
    }
    #[test]
    fn expiry_generation_and_unknown_model_are_distinct_mapping_blocks() {
        let mut store = store();
        let now = 10_000;
        registration(&mut store, now);
        let document = document();
        let mappings = mappings();
        assert!(
            store
                .portable_policy_preview_at(&document, &mappings, now)
                .unwrap()
                .can_apply
        );
        let expired = store
            .portable_policy_preview_at(&document, &mappings, now + 60_000)
            .unwrap();
        assert!(!expired.can_apply);
        assert!(
            expired
                .issues
                .iter()
                .any(|i| i.code == "portable-current-observation-required")
        );
        let mut unknown = mappings.clone();
        unknown.models[0].model_id = "unknown-model".into();
        let p = store
            .portable_policy_preview_at(&document, &unknown, now)
            .unwrap();
        assert!(
            p.issues
                .iter()
                .any(|i| i.code == "portable-model-not-observed")
        );
        store.daemon_start().unwrap();
        assert!(
            !store
                .portable_policy_preview_at(&document, &mappings, now)
                .unwrap()
                .can_apply
        );
    }

    #[test]
    fn model_only_preferences_and_requirements_gain_an_explicit_destination_binding() {
        for mandatory in [false, true] {
            let mut store = store();
            let now = crate::connections::now_ms();
            registration(&mut store, now);
            let mut source = Settings::default();
            if mandatory {
                source.policy.mode_requirements.forced_model = Some("source-model".into());
            } else {
                source.policy.mode_defaults.model_id = Some("source-model".into());
            }
            let bundle = export(&source).unwrap();
            assert_eq!(bundle.connection_references.len(), 1);
            assert_eq!(
                bundle.model_references[0].connection_alias.as_deref(),
                Some("connection-1")
            );
            let document = serde_json::to_string(&bundle).unwrap();
            let mappings = mappings();
            let p = store
                .portable_policy_preview_at(&document, &mappings, now)
                .unwrap();
            assert!(p.can_apply, "{:?}", p.issues);
            store
                .execute(&Request {
                    scope: LocalRef::new("desktop").unwrap(),
                    key: LocalRef::new("model-only-import").unwrap(),
                    expected_revision: p.expected_revision,
                    command: Command::Desktop(DesktopCommand::ImportPolicy {
                        document,
                        mappings,
                        expected_preview_digest: p.preview_digest,
                    }),
                })
                .unwrap();
            let local = settings_from_state(&store.load().unwrap()).unwrap();
            if mandatory {
                assert_eq!(
                    local.policy.mode_requirements.forced_connection.as_deref(),
                    Some("destination-account")
                );
                assert_eq!(
                    local.policy.mode_requirements.forced_model.as_deref(),
                    Some("destination-model")
                );
            } else {
                assert_eq!(
                    local.policy.mode_defaults.connection_id.as_deref(),
                    Some("destination-account")
                );
                assert_eq!(
                    local.policy.mode_defaults.model_id.as_deref(),
                    Some("destination-model")
                );
            }
        }
    }
}
