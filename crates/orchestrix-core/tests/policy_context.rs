use orchestrix_core::policy::*;
use orchestrix_core::runtime_contract::{assert_admission, normalize_descriptor};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const NOW: u64 = 10_000;
fn descriptor() -> Value {
    let capability = json!({"support":"supported","availability":"available","configuration":"enabled","evidence":{"source":"synthetic","scope":"model","reference":"fixture"}});
    let mut models = Vec::new();
    for name in ["small", "medium", "large"] {
        models.push(json!({"id":name,"availability":"available","reasoningLevels":["low","medium","high","maximum"],"capabilities":{"modelSelection":capability,"reasoning":capability,"tools":capability}}));
    }
    json!({"version":{"major":1,"minor":1},"adapterId":"fixture","runtimeVersion":"fixture-1","connection":{"id":"one-account","revision":1,"authentication":"authenticated","billing":"subscription","availability":"available"},"catalog":{"completeness":"complete"},"models":models})
}
fn runtime_from(value: Value) -> ObservedRuntime {
    let mut observations = Vec::new();
    let lease = |fact: &str, model: Option<&str>, n: usize| ObservationLease {
        fact: fact.into(),
        model_id: model.map(str::to_owned),
        evidence_ref: format!("fact-{n}"),
        connection_id: "one-account".into(),
        connection_revision: 1,
        runtime_version: "fixture-1".into(),
        observed_at_ms: NOW - 100,
        valid_until_ms: NOW + 30_000,
    };
    for fact in [
        "identity",
        "billing",
        "availability",
        "catalog",
        "control:workspace-write",
        "control:read-only",
        "control:network-disabled",
        "control:network-enabled",
        "control:approval-never",
        "control:approval-ask",
        "control:native-delegation-disabled",
    ] {
        observations.push(lease(fact, None, observations.len()));
    }
    for model in ["small", "medium", "large"] {
        for fact in [
            "model-availability",
            "capability:modelSelection",
            "capability:reasoning",
            "capability:tools",
        ] {
            observations.push(lease(fact, Some(model), observations.len()));
        }
    }
    ObservedRuntime::new(normalize_descriptor(&value).unwrap(), observations).unwrap()
}
fn runtime() -> ObservedRuntime {
    runtime_from(descriptor())
}
fn document(strategy: Strategy) -> PolicyDocument {
    let mut document = PolicyDocument::default();
    document.mode_defaults.strategy = Some(strategy);
    document.model_ranks = [("small", 1, 1), ("medium", 2, 2), ("large", 3, 3)]
        .into_iter()
        .map(|(model, strength, efficiency_order)| ModelRank {
            connection_id: "one-account".into(),
            model_id: model.into(),
            strength,
            efficiency_order,
            source_ref: "declared-ranking".into(),
        })
        .collect();
    document
}
fn task(level: TaskLevel) -> TaskClassification {
    TaskClassification {
        kind: TaskKind::Implementation,
        complexity: Some(level),
        risk: Some(level),
        importance: Some(level),
        source_ref: "user-task".into(),
    }
}
fn selected(doc: &PolicyDocument, task: &TaskClassification) -> Selection {
    resolve(doc, task, &[runtime()], NOW)
        .unwrap()
        .selection()
        .unwrap()
        .clone()
}
fn layer(
    scope: Scope,
    reference: &str,
    preferences: Preferences,
    requirements: Requirements,
) -> PolicyLayer {
    PolicyLayer {
        scope,
        reference: reference.into(),
        revision: 1,
        preferences,
        requirements,
    }
}
#[test]
fn deterministic_strategies_use_declared_rankings_and_one_serial_account() {
    for (strategy, level, expected, effort) in [
        (
            Strategy::Efficiency,
            TaskLevel::Low,
            "small",
            Reasoning::Low,
        ),
        (
            Strategy::Performance,
            TaskLevel::Low,
            "large",
            Reasoning::High,
        ),
        (
            Strategy::Balanced,
            TaskLevel::Medium,
            "medium",
            Reasoning::Medium,
        ),
        (
            Strategy::Efficiency,
            TaskLevel::High,
            "large",
            Reasoning::High,
        ),
        (
            Strategy::Balanced,
            TaskLevel::Critical,
            "large",
            Reasoning::Maximum,
        ),
    ] {
        let doc = document(strategy);
        let decision = resolve(&doc, &task(level), &[runtime()], NOW).unwrap();
        let selection = decision.selection().unwrap();
        assert_eq!(selection.connection_id, "one-account");
        assert_eq!(selection.model_id.as_deref(), Some(expected));
        assert_eq!(selection.reasoning, Some(effort));
        assert_eq!(selection.concurrency, 1);
        let request = selection
            .runtime_request("session", "attempt", "context")
            .unwrap();
        assert_admission(&runtime().descriptor, &request, None).unwrap();
        let repeated = resolve(&doc, &task(level), &[runtime()], NOW).unwrap();
        assert_eq!(decision.snapshot(), repeated.snapshot());
        assert_eq!(
            EffectivePolicySnapshot::from_json(decision.snapshot().as_json()).unwrap(),
            *decision.snapshot()
        );
        assert_eq!(
            decision.snapshot().as_json()["executionEffective"]["reasoning"],
            "unknown"
        );
    }
    let mut doc = document(Strategy::Custom);
    doc.mode_defaults.preferred_models = Some(vec!["large".into(), "small".into()]);
    assert_eq!(
        selected(&doc, &task(TaskLevel::Low)).model_id.as_deref(),
        Some("large")
    );
}
#[test]
fn scope_precedence_is_normalized_and_mode_profile_defaults_have_explicit_positions() {
    let mut doc = document(Strategy::Efficiency);
    doc.mode_defaults.reasoning = Some(Reasoning::Low);
    doc.profile_defaults.reasoning = Some(Reasoning::High);
    doc.layers = vec![
        layer(
            Scope::Task,
            "task",
            Preferences {
                reasoning: Some(Reasoning::Medium),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::Project,
            "project",
            Preferences {
                reasoning: Some(Reasoning::Low),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::System,
            "system",
            Preferences {
                reasoning: Some(Reasoning::Maximum),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::Execution,
            "execution",
            Preferences {
                reasoning: Some(Reasoning::Maximum),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::CategoryRisk,
            "category",
            Preferences {
                reasoning: Some(Reasoning::Low),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::User,
            "user",
            Preferences {
                reasoning: Some(Reasoning::Medium),
                ..Default::default()
            },
            Requirements::default(),
        ),
        layer(
            Scope::Profile,
            "profile",
            Preferences {
                reasoning: Some(Reasoning::High),
                ..Default::default()
            },
            Requirements::default(),
        ),
    ];
    let decision = resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW).unwrap();
    assert_eq!(
        decision.selection().unwrap().reasoning,
        Some(Reasoning::Medium)
    );
    assert_eq!(
        decision.snapshot().as_json()["origins"]["reasoning"],
        "task@1"
    );
    let old = decision.snapshot().clone();
    doc.revision += 1;
    doc.layers.retain(|l| l.scope <= Scope::Project);
    let new = resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW).unwrap();
    assert_eq!(new.selection().unwrap().reasoning, Some(Reasoning::High));
    assert_eq!(
        new.snapshot().as_json()["origins"]["reasoning"],
        "profile-defaults@profile"
    );
    assert_eq!(old.as_json()["policyRevision"], 1);
    assert_ne!(old.revision(), new.snapshot().revision());
}
#[test]
fn mandatory_constraints_accumulate_and_cannot_be_removed_by_overrides() {
    let mut doc = document(Strategy::Efficiency);
    doc.mode_requirements.read_only = true;
    doc.layers.push(layer(
        Scope::System,
        "system",
        Preferences::default(),
        Requirements {
            subscription_only: true,
            forbid_network: true,
            max_concurrency: Some(1),
            required_checks: ["tests".into()].into(),
            require_review: true,
            ..Default::default()
        },
    ));
    doc.layers.push(layer(
        Scope::Task,
        "task",
        Preferences {
            sandbox_write: Some(true),
            network: Some(true),
            concurrency: Some(2),
            billing: Some(BillingMode::ExplicitApi),
            billing_authorization_ref: Some("paid-consent".into()),
            ..Default::default()
        },
        Requirements::default(),
    ));
    let decision = resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW).unwrap();
    let DecisionRoute::Blocked(reasons) = decision.route() else {
        panic!("must block")
    };
    for reason in [
        "mandatory-read-only",
        "mandatory-network-disabled",
        "mandatory-concurrency-limit",
        "mandatory-subscription-only",
    ] {
        assert!(reasons.iter().any(|r| r == reason));
    }
    assert_eq!(
        decision.snapshot().as_json()["requirements"]["required_checks"],
        json!(["tests"])
    );
    assert_eq!(
        decision.snapshot().as_json()["requirements"]["require_review"],
        true
    );
    doc.layers.push(layer(
        Scope::User,
        "user",
        Preferences::default(),
        Requirements {
            forced_model: Some("small".into()),
            ..Default::default()
        },
    ));
    doc.mode_requirements.forced_model = Some("large".into());
    assert_eq!(
        resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW).unwrap_err(),
        PolicyError::Conflict
    );
}
#[test]
fn manual_reasoning_fallback_is_explicit_and_never_weakens_a_requirement() {
    let mut raw = descriptor();
    for model in raw["models"].as_array_mut().unwrap() {
        model["reasoningLevels"] = json!(["low", "high"]);
    }
    let observed = runtime_from(raw);
    let mut doc = document(Strategy::Custom);
    doc.mode_defaults.model_id = Some("small".into());
    doc.mode_defaults.reasoning = Some(Reasoning::Maximum);
    assert!(matches!(
        resolve(
            &doc,
            &task(TaskLevel::High),
            std::slice::from_ref(&observed),
            NOW
        )
        .unwrap()
        .route(),
        DecisionRoute::Blocked(_)
    ));
    doc.mode_defaults.fallback = Some(Fallback::Compatible);
    let decision = resolve(
        &doc,
        &task(TaskLevel::High),
        std::slice::from_ref(&observed),
        NOW,
    )
    .unwrap();
    let s = decision.selection().unwrap();
    assert_eq!(s.model_id.as_deref(), Some("small"));
    assert_eq!(s.reasoning, Some(Reasoning::High));
    assert!(
        s.fallback_reasons
            .contains(&"reasoning:maximum-to-high".into())
    );
    doc.mode_requirements.minimum_reasoning = Some(Reasoning::Maximum);
    assert!(matches!(
        resolve(&doc, &task(TaskLevel::High), &[observed], NOW)
            .unwrap()
            .route(),
        DecisionRoute::Blocked(_)
    ));
}
#[test]
fn runtime_default_requires_explicit_fallback_and_does_not_invent_entitlement() {
    let mut raw = descriptor();
    for model in raw["models"].as_array_mut().unwrap() {
        model["availability"] = json!("unknown");
    }
    let observed = runtime_from(raw);
    let mut doc = document(Strategy::Balanced);
    assert!(
        resolve(
            &doc,
            &task(TaskLevel::Medium),
            std::slice::from_ref(&observed),
            NOW
        )
        .unwrap()
        .selection()
        .is_none()
    );
    doc.mode_defaults.fallback = Some(Fallback::Compatible);
    let decision = resolve(
        &doc,
        &task(TaskLevel::Medium),
        std::slice::from_ref(&observed),
        NOW,
    )
    .unwrap();
    let s = decision.selection().unwrap();
    assert_eq!(s.model_id, None);
    assert_eq!(s.reasoning, None);
    assert!(
        s.fallback_reasons
            .iter()
            .any(|r| r.contains("runtime-default"))
    );
    assert_admission(
        &observed.descriptor,
        &s.runtime_request("session", "attempt", "context").unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        observed.descriptor.as_json()["models"][0]["availability"],
        "unknown"
    );
    doc.mode_requirements
        .required_capabilities
        .insert("tools".into());
    assert!(
        resolve(&doc, &task(TaskLevel::Medium), &[observed], NOW)
            .unwrap()
            .selection()
            .is_none()
    );
}

#[test]
fn explicit_reasoning_cannot_upgrade_without_compatible_fallback_permission() {
    let mut raw = descriptor();
    for model in raw["models"].as_array_mut().unwrap() {
        model["reasoningLevels"] = json!(["medium", "high"]);
    }
    let observed = runtime_from(raw);
    let mut doc = document(Strategy::Custom);
    doc.mode_defaults.reasoning = Some(Reasoning::Low);
    assert!(
        resolve(
            &doc,
            &task(TaskLevel::Low),
            std::slice::from_ref(&observed),
            NOW
        )
        .unwrap()
        .selection()
        .is_none()
    );
    doc.mode_defaults.fallback = Some(Fallback::Compatible);
    let decision = resolve(&doc, &task(TaskLevel::Low), &[observed], NOW).unwrap();
    assert_eq!(
        decision.selection().unwrap().reasoning,
        Some(Reasoning::Medium)
    );
    assert!(
        decision
            .selection()
            .unwrap()
            .fallback_reasons
            .contains(&"reasoning:low-to-medium".into())
    );
}
#[test]
fn stale_or_wrong_scope_critical_evidence_requires_targeted_revalidation() {
    let doc = document(Strategy::Balanced);
    for fact in ["identity", "billing", "control:native-delegation-disabled"] {
        let mut observed = runtime();
        for lease in &mut observed.observations {
            if lease.fact == fact {
                lease.valid_until_ms = NOW;
            }
        }
        let decision = resolve(&doc, &task(TaskLevel::Medium), &[observed], NOW).unwrap();
        assert!(decision.selection().is_none());
        assert!(
            decision
                .revalidation()
                .iter()
                .any(|r| r.fact == fact && r.model_id.is_none())
        );
    }
    let mut observed = runtime();
    for lease in &mut observed.observations {
        if lease.fact == "identity" {
            lease.connection_revision = 2;
        }
    }
    assert!(
        resolve(&doc, &task(TaskLevel::Medium), &[observed], NOW)
            .unwrap()
            .selection()
            .is_none()
    );
    let mut observed = runtime();
    for lease in &mut observed.observations {
        lease.runtime_version = "changed-version".into();
    }
    assert!(
        resolve(&doc, &task(TaskLevel::Medium), &[observed], NOW)
            .unwrap()
            .selection()
            .is_none()
    );
}
#[test]
fn local_ttls_cap_long_host_leases_and_historical_snapshots_stay_unchanged() {
    let mut doc = document(Strategy::Balanced);
    let observed = runtime();
    let accepted = resolve(
        &doc,
        &task(TaskLevel::Medium),
        std::slice::from_ref(&observed),
        NOW,
    )
    .unwrap();
    assert!(accepted.selection().is_some());
    doc.observation_ttls.identity_billing_ms = 100;
    let blocked = resolve(&doc, &task(TaskLevel::Medium), &[observed], NOW).unwrap();
    assert!(blocked.selection().is_none());
    assert!(accepted.selection().is_some());
    assert_ne!(
        accepted.snapshot().revision(),
        blocked.snapshot().revision()
    );
}
#[test]
fn partial_catalog_missing_model_is_not_a_deprecation_and_cannot_grant_other_model_capabilities() {
    let mut raw = descriptor();
    raw["catalog"]["completeness"] = json!("partial");
    raw["capabilities"]["webSearch"] =
        json!({"support":"supported","availability":"available","configuration":"enabled"});
    let observed = runtime_from(raw);
    let mut doc = document(Strategy::Custom);
    doc.mode_defaults.model_id = Some("missing".into());
    let decision = resolve(
        &doc,
        &task(TaskLevel::Low),
        std::slice::from_ref(&observed),
        NOW,
    )
    .unwrap();
    assert!(decision.selection().is_none());
    assert!(
        decision.snapshot().as_json()["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["reasons"] == json!(["model-not-observed-in-partial-catalog"]))
    );
    assert!(decision.revalidation().iter().any(|r| r.fact == "catalog"));
    assert!(
        !decision
            .snapshot()
            .as_json()
            .to_string()
            .contains("discontinued")
    );
    doc.mode_defaults.model_id = None;
    doc.mode_requirements
        .required_capabilities
        .insert("webSearch".into());
    assert!(
        resolve(&doc, &task(TaskLevel::Low), &[observed], NOW)
            .unwrap()
            .selection()
            .is_none()
    );
}
#[test]
fn specialized_presets_reuse_resolver_without_changing_mode_profile_or_permissions() {
    for preset in [
        SpecializedPreset::ScientificResearch,
        SpecializedPreset::SoftwareArchitecture,
        SpecializedPreset::ComplexPlanning,
        SpecializedPreset::SecurityReview,
    ] {
        let mut doc = document(Strategy::Balanced);
        doc.mode_defaults.preset = Some(preset);
        let decision = resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW).unwrap();
        let s = decision.selection().unwrap();
        assert_eq!(s.reasoning, Some(Reasoning::High));
        assert!(!s.network && !s.native_delegation);
        assert_eq!(decision.snapshot().as_json()["mode"]["id"], "development");
        assert_eq!(
            decision.snapshot().as_json()["profile"]["id"],
            "implementation-engineer"
        );
    }
    let mut doc = document(Strategy::Efficiency);
    doc.mode_defaults.preset = Some(SpecializedPreset::Debugging);
    assert_eq!(
        selected(&doc, &task(TaskLevel::Low)).reasoning,
        Some(Reasoning::Low)
    );
    assert_eq!(
        selected(&doc, &task(TaskLevel::High)).reasoning,
        Some(Reasoning::High)
    );
}
#[test]
fn mechanical_queries_are_routed_without_model_or_provider_observation() {
    let doc = document(Strategy::Performance);
    for kind in [
        TaskKind::GitStatus,
        TaskKind::GitDiff,
        TaskKind::FileExists,
        TaskKind::ProcessStatus,
        TaskKind::CheckResults,
    ] {
        let decision = resolve(&doc, &TaskClassification::routine(kind, "user"), &[], NOW).unwrap();
        assert_eq!(decision.route(), &DecisionRoute::Deterministic(kind));
        assert!(decision.selection().is_none());
        assert!(decision.revalidation().is_empty());
    }
}
#[test]
fn schema_is_versioned_closed_bounded_and_mandatory_strength_uses_declared_evidence() {
    let mut doc = document(Strategy::Balanced);
    assert_eq!(
        PolicyDocument::from_json(&serde_json::to_value(&doc).unwrap()).unwrap(),
        doc
    );
    let mut value = serde_json::to_value(&doc).unwrap();
    value["unknown"] = json!(true);
    assert_eq!(
        PolicyDocument::from_json(&value).unwrap_err(),
        PolicyError::Schema
    );
    doc.schema_version = 2;
    assert_eq!(doc.validate().unwrap_err(), PolicyError::Version);
    doc.schema_version = 1;
    doc.mode_requirements.minimum_strength = 3;
    assert_eq!(
        selected(&doc, &task(TaskLevel::Low)).model_id.as_deref(),
        Some("large")
    );
    doc.model_ranks.clear();
    assert!(
        resolve(&doc, &task(TaskLevel::Low), &[runtime()], NOW)
            .unwrap()
            .selection()
            .is_none()
    );
    doc.mode_defaults.service_tier = Some("Bearer abcdefghijklmnopqrstuvwxyz".into());
    assert_eq!(doc.validate().unwrap_err(), PolicyError::UnsafeContent);
    let mut choice = selected(&document(Strategy::Balanced), &task(TaskLevel::Medium));
    choice.service_tier = Some("fast".into());
    assert_eq!(
        choice
            .runtime_request("session", "attempt", "context")
            .unwrap_err(),
        PolicyError::Schema
    );
    choice.service_tier = None;
    choice.native_delegation = true;
    assert_eq!(
        choice
            .runtime_request("session", "attempt", "context")
            .unwrap_err(),
        PolicyError::Schema
    );
}
#[test]
fn historical_snapshot_restore_rejects_changed_choice_or_unrecognized_fields() {
    let decision = resolve(
        &document(Strategy::Balanced),
        &task(TaskLevel::Medium),
        &[runtime()],
        NOW,
    )
    .unwrap();
    let mut value = decision.snapshot().as_json().clone();
    value["route"]["value"]["model_id"] = json!("large");
    assert_eq!(
        EffectivePolicySnapshot::from_json(&value).unwrap_err(),
        PolicyError::Conflict
    );
    value = decision.snapshot().as_json().clone();
    value["extra"] = json!(true);
    assert_eq!(
        EffectivePolicySnapshot::from_json(&value).unwrap_err(),
        PolicyError::Conflict
    );
}

#[test]
fn documents_created_in_rust_obey_the_same_serialized_limit_as_json() {
    let mut doc = PolicyDocument::default();
    let identifiers: Vec<String> = (0..256)
        .map(|n| format!("id-{n:04}-{}", "a".repeat(68)))
        .collect();
    doc.layers = [
        Scope::System,
        Scope::User,
        Scope::Project,
        Scope::Profile,
        Scope::CategoryRisk,
        Scope::Execution,
        Scope::Task,
    ]
    .into_iter()
    .enumerate()
    .map(|(n, scope)| {
        layer(
            scope,
            &format!("layer-{n}"),
            Preferences {
                preferred_connections: Some(identifiers.clone()),
                preferred_models: Some(identifiers.clone()),
                ..Default::default()
            },
            Requirements::default(),
        )
    })
    .collect();
    assert!(serde_json::to_vec(&doc).unwrap().len() > MAX_POLICY_DOCUMENT_BYTES);
    assert_eq!(doc.validate().unwrap_err(), PolicyError::Limit);
    assert_eq!(
        PolicyDocument::from_json(&serde_json::to_value(&doc).unwrap()).unwrap_err(),
        PolicyError::Limit
    );
    doc.layers.pop();
    assert!(serde_json::to_vec(&doc).unwrap().len() <= MAX_POLICY_DOCUMENT_BYTES);
    doc.validate().unwrap();
    assert_eq!(
        PolicyDocument::from_json(&serde_json::to_value(&doc).unwrap()).unwrap(),
        doc
    );
}
fn context() -> ContextInput {
    ContextInput {
        schema_version: 1,
        id: "focused-context".into(),
        version: 1,
        objective: "Fix integer addition".into(),
        acceptance_criteria: vec!["Adds two integers correctly".into()],
        constraints: vec!["Edit only authorized task files".into()],
        sources: vec![ContextSource {
            identity: ContextSourceIdentity {
                reference: "repository-instructions".into(),
                revision: "sha256-123".into(),
            },
            kind: ContextSourceKind::RepositoryInstructions,
            authorization_ref: Some("read-instructions".into()),
            text: "Preserve the public interface".into(),
            required: true,
            priority: 0,
        }],
        files: vec![AuthorizedContextFile {
            path: "src/math.rs".into(),
            source: ContextSourceIdentity {
                reference: "task-file".into(),
                revision: "commit-123".into(),
            },
            authorization_ref: Some("read-task-file".into()),
            content: "pub fn add(a: i32,b: i32)->i32 { a-b }\r\n".into(),
            required: true,
            priority: 0,
        }],
    }
}
#[test]
fn focused_context_preserves_exact_authorized_bytes_provenance_budget_and_omissions() {
    let mut input = context();
    input.files.push(AuthorizedContextFile {
        path: ".env".into(),
        source: ContextSourceIdentity {
            reference: "environment".into(),
            revision: "commit-123".into(),
        },
        authorization_ref: Some("read-env".into()),
        content: "API_KEY=sk-secretcredential123456".into(),
        required: false,
        priority: 1,
    });
    input.files.push(AuthorizedContextFile {
        path: "private.txt".into(),
        source: ContextSourceIdentity {
            reference: "private".into(),
            revision: "commit-123".into(),
        },
        authorization_ref: None,
        content: "private content must not appear".into(),
        required: false,
        priority: 2,
    });
    let pack = compile_context(&input, ContextBudget::default()).unwrap();
    assert_eq!(pack.as_json()["files"].as_array().unwrap().len(), 1);
    assert_eq!(
        pack.as_json()["files"][0]["content"],
        input.files[0].content
    );
    assert_eq!(
        pack.as_json()["files"][0]["contentHash"],
        format!("{:x}", Sha256::digest(input.files[0].content.as_bytes()))
    );
    assert_eq!(pack.as_json()["omissions"].as_array().unwrap().len(), 2);
    assert!(!pack.rendered().contains("sk-secret"));
    assert!(!pack.rendered().contains("private content"));
    assert_eq!(
        pack.as_json()["budget"]["actualPromptBytes"],
        pack.rendered().len()
    );
    assert_eq!(
        pack.as_json()["budget"]["estimatedTokens"],
        pack.rendered().len().div_ceil(4)
    );
    assert_eq!(
        pack.revision(),
        format!("{:x}", Sha256::digest(pack.rendered().as_bytes()))
    );
    let old = pack.clone();
    input.version = 2;
    input.files[0].content.push_str("// changed\n");
    let changed = compile_context(&input, ContextBudget::default()).unwrap();
    assert_ne!(old.revision(), changed.revision());
    assert_eq!(old.as_json()["version"], 1);
}
#[test]
fn context_required_items_cannot_be_silently_omitted_or_redacted() {
    let mut input = context();
    input.files[0].authorization_ref = None;
    assert_eq!(
        compile_context(&input, ContextBudget::default()).unwrap_err(),
        PolicyError::Ownership
    );
    input.files[0].authorization_ref = Some("read-file".into());
    input.files[0].content = "API_KEY=sk-secretcredential123456".into();
    assert_eq!(
        compile_context(&input, ContextBudget::default()).unwrap_err(),
        PolicyError::UnsafeContent
    );
    input.files[0].content = "a".repeat(20_000);
    assert_eq!(
        compile_context(&input, ContextBudget::default()).unwrap_err(),
        PolicyError::Limit
    );
    input.files[0].content = "a".into();
    assert_eq!(
        compile_context(
            &input,
            ContextBudget {
                max_bytes: 100,
                ..Default::default()
            }
        )
        .unwrap_err(),
        PolicyError::Limit
    );
}
#[test]
fn context_rejects_path_escape_aliases_transcripts_and_prioritizes_required_content() {
    let mut input = context();
    input.files[0].path = "../outside.txt".into();
    assert_eq!(
        compile_context(&input, ContextBudget::default()).unwrap_err(),
        PolicyError::Schema
    );
    input.files[0].path = "src/math.rs".into();
    let mut alias = input.files[0].clone();
    alias.path = "SRC/math.rs".into();
    input.files.push(alias);
    assert_eq!(
        compile_context(&input, ContextBudget::default()).unwrap_err(),
        PolicyError::Schema
    );
    input.files.pop();
    let mut body = serde_json::to_value(&input).unwrap();
    body["runtimeHistory"] = json!(["do not duplicate runtime transcript"]);
    assert!(serde_json::from_value::<ContextInput>(body).is_err());
    input.sources.push(ContextSource {
        identity: ContextSourceIdentity {
            reference: "optional-architecture".into(),
            revision: "revision-2".into(),
        },
        kind: ContextSourceKind::ArchitectureDecision,
        authorization_ref: Some("read-adr".into()),
        text: "x".repeat(4000),
        required: false,
        priority: 0,
    });
    let pack = compile_context(
        &input,
        ContextBudget {
            max_bytes: 2200,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(pack.as_json()["files"].as_array().unwrap().len(), 1);
    assert_eq!(pack.as_json()["sources"].as_array().unwrap().len(), 1);
    assert_eq!(pack.as_json()["omissions"][0]["reason"], "context-budget");
    assert!(pack.rendered().len() <= 2200);
}
