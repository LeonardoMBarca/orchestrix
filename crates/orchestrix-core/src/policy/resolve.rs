use super::schema::{merge_preferences, validate_requirements};
use super::*;
use crate::runtime_contract::{
    RuntimeDescriptor, RuntimeRequest, capability_decision, normalize_descriptor, normalize_request,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// A host-issued dated fact, scoped to one exact connection/runtime revision.
/// A lease never changes an unknown/unsupported value in the descriptor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservationLease {
    pub fact: String,
    pub model_id: Option<String>,
    pub evidence_ref: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub runtime_version: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}
#[derive(Clone, Debug)]
pub struct ObservedRuntime {
    pub descriptor: RuntimeDescriptor,
    pub observations: Vec<ObservationLease>,
}
impl ObservedRuntime {
    pub fn new(
        descriptor: RuntimeDescriptor,
        observations: Vec<ObservationLease>,
    ) -> PolicyResult<Self> {
        let value = Self {
            descriptor,
            observations,
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> PolicyResult<()> {
        if self.observations.len() > 4096 {
            return Err(PolicyError::Limit);
        }
        for lease in &self.observations {
            if !id(&lease.connection_id)
                || !id(&lease.evidence_ref)
                || lease.model_id.as_ref().is_some_and(|v| !id(v))
                || lease.connection_revision == 0
                || lease.valid_until_ms <= lease.observed_at_ms
                || !valid_fact(&lease.fact)
                || lease.runtime_version.len() > 100
                || lease.runtime_version.is_empty()
            {
                return Err(PolicyError::Schema);
            }
            safe_text(&lease.runtime_version, 100)?;
        }
        Ok(())
    }
    fn lease(
        &self,
        fact: &str,
        model: Option<&str>,
        now: u64,
        ttls: ObservationTtls,
    ) -> Option<&ObservationLease> {
        let d = self.descriptor.as_json();
        self.observations
            .iter()
            .filter(|l| {
                l.fact == fact
                    && l.model_id.as_deref() == model
                    && l.connection_id == d["connection"]["id"].as_str().unwrap()
                    && l.connection_revision == d["connection"]["revision"].as_u64().unwrap()
                    && l.runtime_version == d["runtimeVersion"].as_str().unwrap()
                    && l.observed_at_ms <= now
                    && now < l.valid_until_ms
                    && now < l.observed_at_ms.saturating_add(ttls.maximum_age(fact))
            })
            .max_by_key(|l| (l.observed_at_ms, l.evidence_ref.as_str()))
    }
}
fn valid_fact(fact: &str) -> bool {
    matches!(
        fact,
        "identity"
            | "billing"
            | "availability"
            | "catalog"
            | "model-availability"
            | "control:read-only"
            | "control:workspace-write"
            | "control:network-disabled"
            | "control:network-enabled"
            | "control:approval-never"
            | "control:approval-ask"
            | "control:native-delegation-disabled"
    ) || fact
        .strip_prefix("capability:")
        .is_some_and(|c| crate::runtime_contract::CAPABILITIES.contains(&c))
        || fact.strip_prefix("service-tier:").is_some_and(id)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Revalidation {
    pub connection_id: String,
    pub model_id: Option<String>,
    pub fact: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidateExplanation {
    pub connection_id: String,
    pub model_id: Option<String>,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub connection_id: String,
    pub connection_revision: u64,
    pub adapter_id: String,
    pub runtime_version: String,
    /// None is an explicit runtime-default choice, never a model entitlement.
    pub model_id: Option<String>,
    /// None is runtime-default; the adapter must still record observed effective settings.
    pub reasoning: Option<Reasoning>,
    pub strategy: Strategy,
    pub preset: Option<SpecializedPreset>,
    pub billing: BillingMode,
    pub billing_authorization_ref: Option<String>,
    pub sandbox_write: bool,
    pub network: bool,
    pub approval_ask: bool,
    pub native_delegation: bool,
    pub service_tier: Option<String>,
    pub concurrency: u16,
    pub context_bytes: usize,
    pub evidence_refs: Vec<String>,
    pub fallback_reasons: Vec<String>,
}
impl Selection {
    pub fn runtime_request(
        &self,
        session_id: &str,
        attempt_id: &str,
        context_pack_ref: &str,
    ) -> PolicyResult<RuntimeRequest> {
        // Wire 1.1 has no service-tier or enabled native-delegation field. Never
        // silently drop an explicit choice while building that request.
        if self.service_tier.is_some() || self.native_delegation {
            return Err(PolicyError::Schema);
        }
        normalize_request(&json!({
            "version":{"major":1,"minor":1},
            "connectionId":self.connection_id,"connectionRevision":self.connection_revision,
            "sessionId":session_id,"attemptId":attempt_id,"turnId":null,
            "billingMode":self.billing.as_str(),"paidFallback":"forbidden",
            "billingAuthorizationRef":self.billing_authorization_ref,
            "contextPackRef":context_pack_ref,
            "requested":{
                "strategy":self.strategy.as_str(),"presetRef":self.preset.map(preset_id),
                "modelPolicy":if self.model_id.is_some(){"specific"}else{"runtime-default"},
                "modelId":self.model_id,"reasoning":self.reasoning.map_or("runtime-default",Reasoning::as_str),
                "sandbox":if self.sandbox_write{"workspace-write"}else{"read-only"},
                "network":if self.network{"enabled"}else{"disabled"},
                "approval":if self.approval_ask{"ask"}else{"never"}
            }
        })).map_err(|_| PolicyError::Schema)
    }
}
fn preset_id(p: SpecializedPreset) -> &'static str {
    match p {
        SpecializedPreset::ScientificResearch => "scientific-research",
        SpecializedPreset::SoftwareArchitecture => "software-architecture",
        SpecializedPreset::ComplexPlanning => "complex-planning",
        SpecializedPreset::Debugging => "debugging",
        SpecializedPreset::SecurityReview => "security-review",
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum DecisionRoute {
    Model(Box<Selection>),
    Deterministic(TaskKind),
    Blocked(Vec<String>),
}
/// Owned value with no mutable setters. Persist as a versioned definition body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectivePolicySnapshot {
    value: Value,
    revision: String,
}
impl EffectivePolicySnapshot {
    pub fn as_json(&self) -> &Value {
        &self.value
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    /// Restore a historical decision by replaying its pure resolution at its
    /// captured time. A restored snapshot is not fresh authorization to execute.
    pub fn from_json(value: &Value) -> PolicyResult<Self> {
        if serde_json::to_vec(value)
            .map_err(|_| PolicyError::Schema)?
            .len()
            > 1_048_576
        {
            return Err(PolicyError::Limit);
        }
        let document = PolicyDocument::from_json(&value["policy"])?;
        let task: TaskClassification =
            serde_json::from_value(value["task"].clone()).map_err(|_| PolicyError::Schema)?;
        let now = value["resolvedAtMs"].as_u64().ok_or(PolicyError::Schema)?;
        let entries = value["runtimeObservations"]
            .as_array()
            .ok_or(PolicyError::Schema)?;
        if entries.len() > 32 {
            return Err(PolicyError::Limit);
        }
        let mut runtimes = Vec::new();
        for entry in entries {
            let descriptor =
                normalize_descriptor(&entry["descriptor"]).map_err(|_| PolicyError::Schema)?;
            let observations = serde_json::from_value(entry["observations"].clone())
                .map_err(|_| PolicyError::Schema)?;
            runtimes.push(ObservedRuntime::new(descriptor, observations)?);
        }
        let snapshot = resolve(&document, &task, &runtimes, now)?.snapshot;
        if &snapshot.value != value {
            return Err(PolicyError::Conflict);
        }
        Ok(snapshot)
    }
}
#[derive(Clone, Debug)]
pub struct Decision {
    route: DecisionRoute,
    snapshot: EffectivePolicySnapshot,
    revalidation: Vec<Revalidation>,
}
impl Decision {
    pub fn route(&self) -> &DecisionRoute {
        &self.route
    }
    pub fn selection(&self) -> Option<&Selection> {
        if let DecisionRoute::Model(s) = &self.route {
            Some(s)
        } else {
            None
        }
    }
    pub fn snapshot(&self) -> &EffectivePolicySnapshot {
        &self.snapshot
    }
    pub fn revalidation(&self) -> &[Revalidation] {
        &self.revalidation
    }
}

/// No clocks, process calls, metadata refresh or model inference occurs here.
pub fn resolve(
    document: &PolicyDocument,
    task: &TaskClassification,
    runtimes: &[ObservedRuntime],
    now_ms: u64,
) -> PolicyResult<Decision> {
    document.validate()?;
    if !id(&task.source_ref) {
        return Err(PolicyError::Schema);
    }
    if runtimes.len() > 32 {
        return Err(PolicyError::Limit);
    }
    let mut connections = BTreeSet::new();
    for runtime in runtimes {
        runtime.validate()?;
        if !connections.insert(
            runtime.descriptor.as_json()["connection"]["id"]
                .as_str()
                .unwrap(),
        ) {
            return Err(PolicyError::Ownership);
        }
    }
    let mut preferences = EffectivePreferences::default();
    let mut origins: BTreeMap<String, String> = serde_json::to_value(&preferences)
        .map_err(|_| PolicyError::Schema)?
        .as_object()
        .unwrap()
        .keys()
        .map(|k| (k.clone(), "schema-v1-system-default".into()))
        .collect();
    let mut requirements = Requirements::default();
    merge_requirements(&mut requirements, &document.mode_requirements)?;
    merge_preferences(
        &mut preferences,
        &document.mode_defaults,
        "mode-defaults@system",
        &mut origins,
    );
    let mut layers = document.layers.clone();
    layers.sort_by_key(|l| l.scope);
    let mut profile_applied = false;
    for layer in &layers {
        if layer.scope >= Scope::Profile && !profile_applied {
            merge_preferences(
                &mut preferences,
                &document.profile_defaults,
                "profile-defaults@profile",
                &mut origins,
            );
            profile_applied = true;
        }
        merge_preferences(
            &mut preferences,
            &layer.preferences,
            &format!("{}@{}", layer.reference, layer.revision),
            &mut origins,
        );
        merge_requirements(&mut requirements, &layer.requirements)?;
    }
    if !profile_applied {
        merge_preferences(
            &mut preferences,
            &document.profile_defaults,
            "profile-defaults@profile",
            &mut origins,
        );
    }
    let mut blocks = mandatory_conflicts(&preferences, &requirements);
    let mut candidates = Vec::new();
    let mut eligible = Vec::new();
    let mut refresh = BTreeSet::new();
    let mechanical = task.kind.is_mechanical();
    let route = if mechanical {
        // This is a request to a separately authorized deterministic service.
        // It grants neither filesystem/process access nor permission to run checks.
        DecisionRoute::Deterministic(task.kind)
    } else if blocks.is_empty() {
        let desired = desired_reasoning(&preferences, task);
        for runtime in runtimes {
            let d = runtime.descriptor.as_json();
            if let Some(requested) = preferences
                .model_id
                .as_ref()
                .or(requirements.forced_model.as_ref())
                && !d["models"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["id"] == requested.as_str())
            {
                let is_partial = d["catalog"]["completeness"] != "complete";
                candidates.push(CandidateExplanation {
                    connection_id: d["connection"]["id"].as_str().unwrap().into(),
                    model_id: Some(requested.clone()),
                    eligible: false,
                    reasons: vec![
                        if is_partial {
                            "model-not-observed-in-partial-catalog"
                        } else {
                            "model-not-present-in-observed-catalog"
                        }
                        .into(),
                    ],
                    evidence_refs: vec![],
                });
                if is_partial
                    || runtime
                        .lease("catalog", None, now_ms, document.observation_ttls)
                        .is_none()
                {
                    refresh.insert(Revalidation {
                        connection_id: d["connection"]["id"].as_str().unwrap().into(),
                        model_id: None,
                        fact: "catalog".into(),
                    });
                }
            }
            for model in d["models"].as_array().unwrap() {
                let rank = document.model_ranks.iter().find(|r| {
                    r.connection_id == d["connection"]["id"].as_str().unwrap()
                        && r.model_id == model["id"].as_str().unwrap()
                });
                let result = evaluate(
                    runtime,
                    Some(model),
                    &preferences,
                    &requirements,
                    desired,
                    now_ms,
                    &mut refresh,
                    document.observation_ttls,
                    rank.map(|r| r.strength),
                );
                candidates.push(result.0);
                if let Some(s) = result.1 {
                    eligible.push(s);
                }
            }
            // No automatic inference of an entitlement from catalog membership.
            if preferences.fallback == Fallback::Compatible {
                let result = evaluate(
                    runtime,
                    None,
                    &preferences,
                    &requirements,
                    desired,
                    now_ms,
                    &mut refresh,
                    document.observation_ttls,
                    None,
                );
                candidates.push(result.0);
                if let Some(s) = result.1 {
                    eligible.push(s);
                }
            }
        }
        eligible.sort_by_key(|s| score(s, &preferences, document, task));
        if let Some(selected) = eligible.into_iter().next() {
            DecisionRoute::Model(Box::new(selected))
        } else {
            blocks.push(
                if refresh.is_empty() {
                    "no-eligible-runtime"
                } else {
                    "critical-evidence-revalidation-required"
                }
                .into(),
            );
            DecisionRoute::Blocked(blocks)
        }
    } else {
        DecisionRoute::Blocked(blocks)
    };
    candidates
        .sort_by(|a, b| (&a.connection_id, &a.model_id).cmp(&(&b.connection_id, &b.model_id)));
    let revalidation: Vec<_> = refresh.into_iter().collect();
    let mut observation_snapshots:Vec<Value> = runtimes.iter().map(|runtime|json!({"descriptor":runtime.descriptor.as_json(),"observations":runtime.observations})).collect();
    observation_snapshots.sort_by(|a, b| {
        a["descriptor"]["connection"]["id"]
            .as_str()
            .cmp(&b["descriptor"]["connection"]["id"].as_str())
    });
    let value = json!({
        "schemaVersion":POLICY_SCHEMA_VERSION,"policyRevision":document.revision,
        "policy":document,"task":task,"classificationDefaults":"missing-is-medium",
        "resolvedAtMs":now_ms,"mode":document.mode,"profile":document.profile,
        "preferences":preferences,"requirements":requirements,"origins":origins,
        "nativeDelegation":"mandatory-disabled-in-initial-worker-profile",
        "candidates":candidates,"route":route,"revalidation":revalidation,
        "runtimeObservations":observation_snapshots,"rankingRule":"declared-ordinals-only-unknown-rank-cannot-outrank-known-facts",
        "executionEffective":{"model":"unknown","reasoning":"unknown","serviceTier":"unknown"},
        "contextOwnership":{"conversation":"runtime","projectMemory":"orchestrix"},
        "quota":"unknown-unless-separately-observed"
    });
    let bytes = serde_json::to_vec(&value).map_err(|_| PolicyError::Schema)?;
    if bytes.len() > 1_048_576 {
        return Err(PolicyError::Limit);
    }
    let revision = hash(&bytes);
    Ok(Decision {
        route,
        snapshot: EffectivePolicySnapshot { value, revision },
        revalidation,
    })
}

fn merge_requirements(to: &mut Requirements, from: &Requirements) -> PolicyResult<()> {
    validate_requirements(from)?;
    to.subscription_only |= from.subscription_only;
    to.forbid_network |= from.forbid_network;
    to.read_only |= from.read_only;
    to.require_review |= from.require_review;
    to.required_capabilities
        .extend(from.required_capabilities.iter().cloned());
    to.required_checks
        .extend(from.required_checks.iter().cloned());
    to.minimum_reasoning = to.minimum_reasoning.max(from.minimum_reasoning);
    to.minimum_strength = to.minimum_strength.max(from.minimum_strength);
    to.max_concurrency = min_optional(to.max_concurrency, from.max_concurrency);
    to.max_context_bytes = min_optional(to.max_context_bytes, from.max_context_bytes);
    for (target, patch) in [
        (&mut to.forced_model, &from.forced_model),
        (&mut to.forced_connection, &from.forced_connection),
    ] {
        if let Some(value) = patch {
            if target.as_ref().is_some_and(|v| v != value) {
                return Err(PolicyError::Conflict);
            }
            *target = Some(value.clone());
        }
    }
    Ok(())
}
fn min_optional<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, None) | (None, a) => a,
    }
}
fn mandatory_conflicts(p: &EffectivePreferences, r: &Requirements) -> Vec<String> {
    let mut reasons = Vec::new();
    for (violated, code) in [
        (
            r.subscription_only && p.billing != BillingMode::SubscriptionOnly,
            "mandatory-subscription-only",
        ),
        (r.forbid_network && p.network, "mandatory-network-disabled"),
        (r.read_only && p.sandbox_write, "mandatory-read-only"),
        (
            r.max_concurrency.is_some_and(|n| p.concurrency > n),
            "mandatory-concurrency-limit",
        ),
        (
            r.max_context_bytes.is_some_and(|n| p.context_bytes > n),
            "mandatory-context-limit",
        ),
        (
            p.billing == BillingMode::ExplicitApi && p.billing_authorization_ref.is_none(),
            "api-authorization-missing",
        ),
        (
            r.forced_model
                .as_ref()
                .zip(p.model_id.as_ref())
                .is_some_and(|(a, b)| a != b),
            "mandatory-model-conflict",
        ),
        (
            r.forced_connection
                .as_ref()
                .zip(p.connection_id.as_ref())
                .is_some_and(|(a, b)| a != b),
            "mandatory-connection-conflict",
        ),
        (
            r.minimum_reasoning
                .zip(p.reasoning)
                .is_some_and(|(min, pref)| pref < min),
            "mandatory-reasoning-conflict",
        ),
    ] {
        if violated {
            reasons.push(code.into());
        }
    }
    reasons
}
fn desired_reasoning(p: &EffectivePreferences, task: &TaskClassification) -> Reasoning {
    if let Some(explicit) = p.reasoning {
        return explicit;
    }
    let severity = task.severity();
    let specialized = matches!(
        p.preset,
        Some(
            SpecializedPreset::ScientificResearch
                | SpecializedPreset::SoftwareArchitecture
                | SpecializedPreset::ComplexPlanning
                | SpecializedPreset::SecurityReview
        )
    );
    if severity == TaskLevel::Critical {
        return Reasoning::Maximum;
    }
    if severity >= TaskLevel::High
        || specialized
        || matches!(task.kind, TaskKind::Architecture | TaskKind::SecurityReview)
    {
        return Reasoning::High;
    }
    match (p.strategy, severity) {
        (Strategy::Efficiency, TaskLevel::Low) => Reasoning::Low,
        (Strategy::Performance, _) => Reasoning::High,
        (_, TaskLevel::Low) => Reasoning::Low,
        _ => Reasoning::Medium,
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate(
    runtime: &ObservedRuntime,
    model: Option<&Value>,
    p: &EffectivePreferences,
    r: &Requirements,
    desired: Reasoning,
    now: u64,
    refresh: &mut BTreeSet<Revalidation>,
    ttls: ObservationTtls,
    strength: Option<u8>,
) -> (CandidateExplanation, Option<Selection>) {
    let d = runtime.descriptor.as_json();
    let connection_id = d["connection"]["id"].as_str().unwrap();
    let model_id = model.map(|m| m["id"].as_str().unwrap());
    let mut reasons = Vec::new();
    let mut evidence = BTreeSet::new();
    let mut fallback = Vec::new();
    let mut require_lease = |fact: &str, scope: Option<&str>| {
        if let Some(lease) = runtime.lease(fact, scope, now, ttls) {
            evidence.insert(lease.evidence_ref.clone());
            true
        } else {
            refresh.insert(Revalidation {
                connection_id: connection_id.into(),
                model_id: scope.map(str::to_owned),
                fact: fact.into(),
            });
            reasons.push(format!("revalidation-required:{fact}"));
            false
        }
    };
    for fact in ["identity", "billing", "availability"] {
        require_lease(fact, None);
    }
    for fact in [
        if p.sandbox_write {
            "control:workspace-write"
        } else {
            "control:read-only"
        },
        if p.network {
            "control:network-enabled"
        } else {
            "control:network-disabled"
        },
        if p.approval_ask {
            "control:approval-ask"
        } else {
            "control:approval-never"
        },
        "control:native-delegation-disabled",
    ] {
        require_lease(fact, None);
    }
    if let Some(tier) = &p.service_tier {
        require_lease(&format!("service-tier:{tier}"), model_id);
    }
    if let Some(mid) = model_id {
        require_lease("catalog", None);
        require_lease("model-availability", Some(mid));
        require_lease("capability:modelSelection", Some(mid));
        for capability in &r.required_capabilities {
            require_lease(&format!("capability:{capability}"), Some(mid));
        }
    }
    if d["connection"]["authentication"] != "authenticated" {
        reasons.push("authentication-unconfirmed".into());
    }
    if d["connection"]["availability"] != "available" {
        reasons.push("connection-unavailable-or-unknown".into());
    }
    let expected_billing = match p.billing {
        BillingMode::SubscriptionOnly => "subscription",
        BillingMode::ExplicitApi => "api",
    };
    if d["connection"]["billing"] != expected_billing {
        reasons.push("billing-incompatible-or-unknown".into());
    }
    if p.connection_id
        .as_deref()
        .or(r.forced_connection.as_deref())
        .is_some_and(|v| v != connection_id)
    {
        reasons.push("connection-selection-mismatch".into());
    }
    let mut reasoning = None;
    if let (Some(mid), Some(model)) = (model_id, model) {
        if p.model_id
            .as_deref()
            .or(r.forced_model.as_deref())
            .is_some_and(|v| v != mid)
        {
            reasons.push("model-selection-mismatch".into());
        }
        if model["availability"] != "available" {
            reasons.push("model-availability-unconfirmed".into());
        }
        if r.minimum_strength > 0 && strength.is_none_or(|s| s < r.minimum_strength) {
            reasons.push("mandatory-strength-not-established".into());
        }
        let mut capabilities = r.required_capabilities.clone();
        capabilities.insert("modelSelection".into());
        for capability in &capabilities {
            if capability_decision(&runtime.descriptor, capability, Some(mid))
                .map_or(true, |c| c["allowed"] != true)
            {
                reasons.push(format!("capability-not-enabled:{capability}"));
            }
        }
        let levels: Vec<Reasoning> = model["reasoningLevels"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        let reasoning_capable = capability_decision(&runtime.descriptor, "reasoning", Some(mid))
            .is_ok_and(|c| c["allowed"] == true);
        let reasoning_lease = runtime.lease("capability:reasoning", Some(mid), now, ttls);
        if let Some(lease) = reasoning_lease.filter(|_| reasoning_capable && !levels.is_empty()) {
            evidence.insert(lease.evidence_ref.clone());
            let minimum = r.minimum_reasoning.unwrap_or(Reasoning::Minimal);
            let mut allowed: Vec<_> = levels.into_iter().filter(|l| *l >= minimum).collect();
            allowed.sort();
            reasoning = if p.reasoning.is_some() && p.fallback == Fallback::Block {
                allowed.iter().copied().find(|l| *l == desired)
            } else {
                allowed.iter().copied().find(|l| *l >= desired).or_else(|| {
                    if p.fallback == Fallback::Compatible {
                        allowed.last().copied()
                    } else {
                        None
                    }
                })
            };
            match reasoning {
                None => reasons.push("reasoning-level-unsupported".into()),
                Some(chosen) if chosen != desired => fallback.push(format!(
                    "reasoning:{}-to-{}",
                    desired.as_str(),
                    chosen.as_str()
                )),
                _ => {}
            }
        } else if p.reasoning.is_some() || r.minimum_reasoning.is_some() {
            reasons.push("reasoning-evidence-missing-or-unsupported".into());
            if reasoning_capable && reasoning_lease.is_none() {
                refresh.insert(Revalidation {
                    connection_id: connection_id.into(),
                    model_id: Some(mid.into()),
                    fact: "capability:reasoning".into(),
                });
            }
        } else {
            fallback.push("automatic-reasoning-not-confirmed:runtime-default".into());
        }
    } else {
        if r.forced_model.is_some()
            || r.minimum_strength > 0
            || r.minimum_reasoning.is_some()
            || !r.required_capabilities.is_empty()
        {
            reasons.push("runtime-default-cannot-prove-mandatory-model-requirements".into());
        }
        fallback.push("explicit-compatible-fallback:runtime-default-model-and-reasoning".into());
        if p.model_id.is_some() {
            fallback.push("requested-model-unconfirmed:runtime-managed-choice-permitted".into());
        }
        if p.reasoning.is_some() {
            fallback.push("requested-reasoning-unconfirmed:runtime-default-permitted".into());
        }
    }
    if r.required_capabilities.contains("nativeDelegation") {
        reasons.push("native-delegation-disabled-by-initial-worker-profile".into());
    }
    let eligible = reasons.is_empty();
    let explanation = CandidateExplanation {
        connection_id: connection_id.into(),
        model_id: model_id.map(str::to_owned),
        eligible,
        reasons,
        evidence_refs: evidence.iter().cloned().collect(),
    };
    let selection = eligible.then(|| Selection {
        connection_id: connection_id.into(),
        connection_revision: d["connection"]["revision"].as_u64().unwrap(),
        adapter_id: d["adapterId"].as_str().unwrap().into(),
        runtime_version: d["runtimeVersion"].as_str().unwrap().into(),
        model_id: model_id.map(str::to_owned),
        reasoning,
        strategy: p.strategy,
        preset: p.preset,
        billing: p.billing,
        billing_authorization_ref: p.billing_authorization_ref.clone(),
        sandbox_write: p.sandbox_write,
        network: p.network,
        approval_ask: p.approval_ask,
        native_delegation: false,
        service_tier: p.service_tier.clone(),
        concurrency: p.concurrency,
        context_bytes: p.context_bytes,
        evidence_refs: evidence.into_iter().collect(),
        fallback_reasons: fallback,
    });
    (explanation, selection)
}
type Score = (bool, usize, usize, i32, u16, String, String);
fn score(
    s: &Selection,
    p: &EffectivePreferences,
    d: &PolicyDocument,
    task: &TaskClassification,
) -> Score {
    let rank = d.model_ranks.iter().find(|r| {
        r.connection_id == s.connection_id && Some(r.model_id.as_str()) == s.model_id.as_deref()
    });
    let strength = rank.map_or(0, |r| i32::from(r.strength));
    let efficiency = rank.map_or(u16::MAX, |r| r.efficiency_order);
    let quality = match p.strategy {
        Strategy::Performance => -strength,
        Strategy::Efficiency => {
            if strength == 0 {
                10
            } else if task.severity() >= TaskLevel::High {
                -strength
            } else {
                strength
            }
        }
        Strategy::Balanced => {
            let target = if task.severity() >= TaskLevel::High {
                3
            } else if task.severity() == TaskLevel::Low {
                1
            } else {
                2
            };
            if strength == 0 {
                10
            } else {
                (target - strength).abs()
            }
        }
        Strategy::Custom => 0,
    };
    (
        s.model_id.is_none(),
        p.preferred_connections
            .iter()
            .position(|v| v == &s.connection_id)
            .unwrap_or(usize::MAX),
        p.preferred_models
            .iter()
            .position(|v| Some(v.as_str()) == s.model_id.as_deref())
            .unwrap_or(usize::MAX),
        quality,
        efficiency,
        s.connection_id.clone(),
        s.model_id.clone().unwrap_or_default(),
    )
}
