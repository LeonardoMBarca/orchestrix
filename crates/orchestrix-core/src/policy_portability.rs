//! Allowlisted, non-authorizing policy transport. No filesystem or provider reads.
use crate::{desktop::Settings, policy::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const PORTABLE_POLICY_SCHEMA_VERSION: u32 = 1;
pub const MAX_PORTABLE_POLICY_BYTES: usize = 262_144;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableAppearance {
    pub language: String,
    pub theme: String,
    pub density: String,
    pub text_scale: u16,
    pub close_behavior: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortablePreferences {
    pub strategy: Option<Strategy>,
    pub preset: Option<SpecializedPreset>,
    pub connection: Option<String>,
    pub model: Option<String>,
    pub reasoning: Option<Reasoning>,
    pub preferred_connections: Option<Vec<String>>,
    pub preferred_models: Option<Vec<String>>,
    pub fallback: Option<Fallback>,
    pub sandbox_write: Option<bool>,
    pub network: Option<bool>,
    pub approval_ask: Option<bool>,
    pub concurrency: Option<u16>,
    pub context_bytes: Option<usize>,
    pub service_tier: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableRequirements {
    pub subscription_only: bool,
    pub forbid_network: bool,
    pub read_only: bool,
    pub required_capabilities: BTreeSet<String>,
    pub minimum_reasoning: Option<Reasoning>,
    pub minimum_strength: u8,
    pub forced_model: Option<String>,
    pub forced_connection: Option<String>,
    pub max_concurrency: Option<u16>,
    pub max_context_bytes: Option<usize>,
    /// Check names are constraints only, never commands or execution consent.
    pub required_checks: BTreeSet<String>,
    pub require_review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableLayer {
    pub scope: Scope,
    pub preferences: PortablePreferences,
    pub requirements: PortableRequirements,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableRank {
    pub connection: String,
    pub model: String,
    pub strength: u8,
    pub efficiency_order: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionReference {
    pub alias: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelReference {
    pub alias: String,
    /// Hint only. The destination must explicitly select a current local model.
    pub source_model_id: String,
    pub connection_alias: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortablePolicy {
    pub mode: DefinitionBinding,
    pub profile: DefinitionBinding,
    pub mode_defaults: PortablePreferences,
    pub mode_requirements: PortableRequirements,
    pub profile_defaults: PortablePreferences,
    pub layers: Vec<PortableLayer>,
    pub model_ranks: Vec<PortableRank>,
    pub observation_ttls: ObservationTtls,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableBundle {
    pub schema_version: u32,
    pub format: String,
    pub appearance: PortableAppearance,
    pub policy: PortablePolicy,
    pub connection_references: Vec<ConnectionReference>,
    pub model_references: Vec<ModelReference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionMapping {
    pub alias: String,
    pub connection_id: String,
    pub binding_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelMapping {
    pub alias: String,
    pub connection_alias: String,
    pub model_id: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportMappings {
    #[serde(default)]
    pub connections: Vec<ConnectionMapping>,
    #[serde(default)]
    pub models: Vec<ModelMapping>,
    #[serde(default)]
    pub mode: Option<DefinitionBinding>,
    #[serde(default)]
    pub profile: Option<DefinitionBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportIssue {
    pub code: String,
    pub reference: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportPreview {
    pub schema_version: u32,
    pub expected_revision: u64,
    pub document_digest: String,
    pub preview_digest: String,
    pub can_apply: bool,
    pub issues: Vec<ImportIssue>,
    pub changes: Vec<String>,
    pub connection_references: Vec<ConnectionReference>,
    pub model_references: Vec<ModelReference>,
    pub definition_references: Value,
    pub mappings: ImportMappings,
}

fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
fn hash(v: &impl Serialize) -> String {
    crate::git_records::content_hash(&serde_json::to_vec(v).expect("serializable policy"))
}
fn alias(map: &mut BTreeMap<String, String>, value: &str, prefix: &str) -> String {
    let next = format!("{prefix}-{}", map.len() + 1);
    map.entry(value.into()).or_insert(next).clone()
}
fn export_preferences(
    p: &Preferences,
    c: &mut BTreeMap<String, String>,
    m: &mut BTreeMap<String, String>,
) -> PortablePreferences {
    PortablePreferences {
        strategy: p.strategy,
        preset: p.preset,
        connection: p.connection_id.as_ref().map(|v| alias(c, v, "connection")),
        model: p.model_id.as_ref().map(|v| alias(m, v, "model")),
        reasoning: p.reasoning,
        preferred_connections: p
            .preferred_connections
            .as_ref()
            .map(|v| v.iter().map(|v| alias(c, v, "connection")).collect()),
        preferred_models: p
            .preferred_models
            .as_ref()
            .map(|v| v.iter().map(|v| alias(m, v, "model")).collect()),
        fallback: p.fallback,
        sandbox_write: p.sandbox_write,
        network: p.network,
        approval_ask: p.approval_ask,
        concurrency: p.concurrency,
        context_bytes: p.context_bytes,
        service_tier: p.service_tier.clone(),
    }
}
fn export_requirements(
    r: &Requirements,
    c: &mut BTreeMap<String, String>,
    m: &mut BTreeMap<String, String>,
) -> PortableRequirements {
    PortableRequirements {
        subscription_only: true,
        forbid_network: r.forbid_network,
        read_only: r.read_only,
        required_capabilities: r.required_capabilities.clone(),
        minimum_reasoning: r.minimum_reasoning,
        minimum_strength: r.minimum_strength,
        forced_model: r.forced_model.as_ref().map(|v| alias(m, v, "model")),
        forced_connection: r
            .forced_connection
            .as_ref()
            .map(|v| alias(c, v, "connection")),
        max_concurrency: r.max_concurrency,
        max_context_bytes: r.max_context_bytes,
        required_checks: r.required_checks.clone(),
        require_review: r.require_review,
    }
}

pub fn export(settings: &Settings) -> Result<PortableBundle> {
    if !settings.validate() {
        return Err("portable-settings-invalid".into());
    }
    let mut c = BTreeMap::new();
    let mut m = BTreeMap::new();
    let p = &settings.policy;
    let policy = PortablePolicy {
        mode: p.mode.clone(),
        profile: p.profile.clone(),
        mode_defaults: export_preferences(&p.mode_defaults, &mut c, &mut m),
        mode_requirements: export_requirements(&p.mode_requirements, &mut c, &mut m),
        profile_defaults: export_preferences(&p.profile_defaults, &mut c, &mut m),
        layers: p
            .layers
            .iter()
            .map(|l| PortableLayer {
                scope: l.scope,
                preferences: export_preferences(&l.preferences, &mut c, &mut m),
                requirements: export_requirements(&l.requirements, &mut c, &mut m),
            })
            .collect(),
        model_ranks: p
            .model_ranks
            .iter()
            .map(|r| PortableRank {
                connection: alias(&mut c, &r.connection_id, "connection"),
                model: alias(
                    &mut m,
                    &format!("rank:{}:{}", r.connection_id, r.model_id),
                    "model",
                ),
                strength: r.strength,
                efficiency_order: r.efficiency_order,
            })
            .collect(),
        observation_ttls: p.observation_ttls,
    };
    // A model without an explicit account still requires an explicit destination binding.
    if !m.is_empty() && c.is_empty() {
        c.insert(String::new(), "connection-1".into());
    }
    let only_connection = (c.len() == 1).then(|| c.values().next().unwrap().clone());
    let mut owners: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for preferences in [&p.mode_defaults, &p.profile_defaults]
        .into_iter()
        .chain(p.layers.iter().map(|l| &l.preferences))
    {
        if let Some(connection) = &preferences.connection_id {
            for model in preferences
                .model_id
                .iter()
                .chain(preferences.preferred_models.iter().flatten())
            {
                owners
                    .entry(model.clone())
                    .or_default()
                    .insert(connection.clone());
            }
        }
    }
    for requirements in
        std::iter::once(&p.mode_requirements).chain(p.layers.iter().map(|l| &l.requirements))
    {
        if let (Some(connection), Some(model)) =
            (&requirements.forced_connection, &requirements.forced_model)
        {
            owners
                .entry(model.clone())
                .or_default()
                .insert(connection.clone());
        }
    }
    let bundle = PortableBundle {
        schema_version: 1,
        format: "orchestrix-policy".into(),
        appearance: PortableAppearance {
            language: settings.language.clone(),
            theme: settings.theme.clone(),
            density: settings.density.clone(),
            text_scale: settings.text_scale,
            close_behavior: settings.close_behavior.clone(),
        },
        policy,
        connection_references: c
            .values()
            .map(|a| ConnectionReference { alias: a.clone() })
            .collect(),
        model_references: m
            .into_iter()
            .map(|(key, alias)| {
                if let Some(rank) = key.strip_prefix("rank:") {
                    let (source_connection, source_model_id) =
                        rank.split_once(':').expect("validated policy IDs");
                    ModelReference {
                        alias,
                        source_model_id: source_model_id.into(),
                        connection_alias: c.get(source_connection).cloned(),
                    }
                } else {
                    let connection_alias = owners
                        .get(&key)
                        .filter(|o| o.len() == 1)
                        .and_then(|o| o.first())
                        .and_then(|o| c.get(o))
                        .cloned()
                        .or_else(|| only_connection.clone());
                    ModelReference {
                        alias,
                        source_model_id: key,
                        connection_alias,
                    }
                }
            })
            .collect(),
    };
    bundle.validate()?;
    Ok(bundle)
}

pub fn parse(document: &str) -> Result<PortableBundle> {
    if document.len() > MAX_PORTABLE_POLICY_BYTES {
        return Err("portable-size-limit".into());
    }
    let value: Value = serde_json::from_str(document).map_err(|_| "portable-schema")?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("portable-version".into());
    }
    // Deserialize original bytes too: struct deserialization rejects duplicate fields.
    let b: PortableBundle = serde_json::from_str(document).map_err(|_| "portable-schema")?;
    b.validate()?;
    Ok(b)
}
impl PortableBundle {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err("portable-version".into());
        }
        if self.format != "orchestrix-policy"
            || self.connection_references.len() > 64
            || self.model_references.len() > 256
            || self.policy.layers.len() > 7
            || self.policy.model_ranks.len() > 256
        {
            return Err("portable-schema".into());
        }
        if [
            self.policy.observation_ttls.identity_billing_ms,
            self.policy.observation_ttls.availability_ms,
            self.policy.observation_ttls.capability_configuration_ms,
        ]
        .iter()
        .any(|n| *n == 0 || *n > 86_400_000)
        {
            return Err("portable-policy-limit".into());
        }
        let c = self
            .connection_references
            .iter()
            .map(|v| v.alias.clone())
            .collect::<BTreeSet<_>>();
        let m = self
            .model_references
            .iter()
            .map(|v| v.alias.clone())
            .collect::<BTreeSet<_>>();
        if c.len() != self.connection_references.len()
            || m.len() != self.model_references.len()
            || c.iter().chain(m.iter()).any(|s| !id(s))
            || self.model_references.iter().any(|r| {
                !id(&r.source_model_id)
                    || r.connection_alias.as_ref().is_some_and(|a| !c.contains(a))
            })
        {
            return Err("portable-references".into());
        }
        // Identity aliases are substituted by valid placeholders solely for schema checking.
        let mappings = ImportMappings {
            connections: c
                .iter()
                .enumerate()
                .map(|(i, a)| ConnectionMapping {
                    alias: a.clone(),
                    connection_id: format!("local-connection-{i}"),
                    binding_revision: 1,
                })
                .collect(),
            models: self
                .model_references
                .iter()
                .enumerate()
                .map(|(i, r)| ModelMapping {
                    alias: r.alias.clone(),
                    connection_alias: r
                        .connection_alias
                        .clone()
                        .or_else(|| c.first().cloned())
                        .unwrap_or_default(),
                    model_id: format!("local-model-{i}"),
                })
                .collect(),
            mode: Some(self.policy.mode.clone()),
            profile: Some(self.policy.profile.clone()),
        };
        let mut local = Settings::default();
        local.policy.mode = self.policy.mode.clone();
        local.policy.profile = self.policy.profile.clone();
        let candidate = self.resolve(&local, &mappings)?;
        if !candidate.validate()
            || serde_json::to_vec(self)
                .map_err(|_| "portable-schema")?
                .len()
                > MAX_PORTABLE_POLICY_BYTES
        {
            return Err("portable-policy-invalid".into());
        }
        Ok(())
    }
    /// Pure conversion; eligibility must be checked by the owning Store transaction.
    pub(crate) fn resolve(
        &self,
        current: &Settings,
        mappings: &ImportMappings,
    ) -> Result<Settings> {
        validate_mappings(self, mappings)?;
        let connection = |a: &str| {
            mappings
                .connections
                .iter()
                .find(|v| v.alias == a)
                .map(|v| v.connection_id.clone())
                .ok_or_else(|| "portable-connection-unresolved".to_string())
        };
        let model = |a: &str| {
            mappings
                .models
                .iter()
                .find(|v| v.alias == a)
                .map(|v| v.model_id.clone())
                .ok_or_else(|| "portable-model-unresolved".to_string())
        };
        let preference = |p: &PortablePreferences| -> Result<Preferences> {
            let mut v = Preferences {
                strategy: p.strategy,
                preset: p.preset,
                connection_id: p.connection.as_deref().map(connection).transpose()?,
                model_id: p.model.as_deref().map(model).transpose()?,
                reasoning: p.reasoning,
                preferred_connections: p
                    .preferred_connections
                    .as_ref()
                    .map(|v| v.iter().map(|v| connection(v)).collect::<Result<Vec<_>>>())
                    .transpose()?,
                preferred_models: p
                    .preferred_models
                    .as_ref()
                    .map(|v| v.iter().map(|v| model(v)).collect::<Result<Vec<_>>>())
                    .transpose()?,
                fallback: p.fallback,
                billing: Some(BillingMode::SubscriptionOnly),
                billing_authorization_ref: None,
                sandbox_write: p.sandbox_write,
                network: p.network,
                approval_ask: p.approval_ask,
                concurrency: p.concurrency,
                context_bytes: p.context_bytes,
                service_tier: p.service_tier.clone(),
            };
            if let Some(a) = &p.model {
                let owner = mappings
                    .models
                    .iter()
                    .find(|v| &v.alias == a)
                    .ok_or("portable-model-unresolved")?;
                let binding = connection(&owner.connection_alias)?;
                if v.connection_id.as_ref().is_some_and(|c| c != &binding) {
                    return Err("portable-model-connection-mismatch".into());
                }
                v.connection_id = Some(binding);
            }
            // Preferred models remain soft preferences, but their explicit
            // destination owners must survive conversion to the Core's two
            // ranking lists. Preserve a source connection order when provided.
            if p.preferred_connections.is_none()
                && let Some(models) = &p.preferred_models
            {
                let mut owners = Vec::new();
                let mut seen = BTreeSet::new();
                for alias in models {
                    let model = mappings
                        .models
                        .iter()
                        .find(|m| &m.alias == alias)
                        .ok_or("portable-model-unresolved")?;
                    let owner = connection(&model.connection_alias)?;
                    if seen.insert(owner.clone()) {
                        owners.push(owner);
                    }
                }
                v.preferred_connections = Some(owners);
            }
            Ok(v)
        };
        let requirement = |r: &PortableRequirements| -> Result<Requirements> {
            let mut result = Requirements {
                subscription_only: true,
                forbid_network: r.forbid_network,
                read_only: r.read_only,
                required_capabilities: r.required_capabilities.clone(),
                minimum_reasoning: r.minimum_reasoning,
                minimum_strength: r.minimum_strength,
                forced_model: r.forced_model.as_deref().map(model).transpose()?,
                forced_connection: r.forced_connection.as_deref().map(connection).transpose()?,
                max_concurrency: r.max_concurrency,
                max_context_bytes: r.max_context_bytes,
                required_checks: r.required_checks.clone(),
                require_review: r.require_review,
            };
            if let Some(alias) = &r.forced_model {
                let owner = mappings
                    .models
                    .iter()
                    .find(|m| &m.alias == alias)
                    .ok_or("portable-model-unresolved")?;
                let binding = connection(&owner.connection_alias)?;
                if result
                    .forced_connection
                    .as_ref()
                    .is_some_and(|c| c != &binding)
                {
                    return Err("portable-model-connection-mismatch".into());
                }
                result.forced_connection = Some(binding);
            }
            Ok(result)
        };
        if (mappings.mode.is_none()
            && (self.policy.mode.id != "development" || self.policy.mode.version != 1))
            || (mappings.profile.is_none()
                && (self.policy.profile.id != "implementation-engineer"
                    || self.policy.profile.version != 1))
        {
            return Err("portable-definition-remapping-required".into());
        }
        let mode = mappings.mode.as_ref().unwrap_or(&self.policy.mode).clone();
        let profile = mappings
            .profile
            .as_ref()
            .unwrap_or(&self.policy.profile)
            .clone();
        // The owning Store validates exact destination definitions in the same
        // transaction as preview/confirmation. Names or source IDs grant nothing.
        for rank in &self.policy.model_ranks {
            let owner = mappings
                .models
                .iter()
                .find(|m| m.alias == rank.model)
                .ok_or("portable-model-unresolved")?;
            if owner.connection_alias != rank.connection {
                return Err("portable-model-connection-mismatch".into());
            }
        }
        let mut policy = PolicyDocument {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: current
                .policy
                .revision
                .checked_add(1)
                .ok_or("portable-revision-limit")?,
            mode,
            profile,
            mode_defaults: preference(&self.policy.mode_defaults)?,
            mode_requirements: requirement(&self.policy.mode_requirements)?,
            profile_defaults: preference(&self.policy.profile_defaults)?,
            layers: self
                .policy
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    Ok(PolicyLayer {
                        scope: l.scope,
                        reference: format!("portable-layer-{}", i + 1),
                        revision: 1,
                        preferences: preference(&l.preferences)?,
                        requirements: requirement(&l.requirements)?,
                    })
                })
                .collect::<Result<_>>()?,
            model_ranks: self
                .policy
                .model_ranks
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    Ok(ModelRank {
                        connection_id: connection(&r.connection)?,
                        model_id: model(&r.model)?,
                        strength: r.strength,
                        efficiency_order: r.efficiency_order,
                        source_ref: format!("portable-rank-{}", i + 1),
                    })
                })
                .collect::<Result<_>>()?,
            observation_ttls: self.policy.observation_ttls,
        };
        // Every local requirement scope is retained, collapsed into the mandatory baseline.
        merge_requirements(
            &mut policy.mode_requirements,
            &current.policy.mode_requirements,
        )?;
        for l in &current.policy.layers {
            merge_requirements(&mut policy.mode_requirements, &l.requirements)?;
        }
        for (target, old) in [
            (
                &mut policy.observation_ttls.identity_billing_ms,
                current.policy.observation_ttls.identity_billing_ms,
            ),
            (
                &mut policy.observation_ttls.availability_ms,
                current.policy.observation_ttls.availability_ms,
            ),
            (
                &mut policy.observation_ttls.capability_configuration_ms,
                current.policy.observation_ttls.capability_configuration_ms,
            ),
        ] {
            *target = (*target).min(old);
        }
        let a = &self.appearance;
        let settings = Settings {
            language: a.language.clone(),
            theme: a.theme.clone(),
            density: a.density.clone(),
            text_scale: a.text_scale,
            close_behavior: a.close_behavior.clone(),
            system_instructions: current.system_instructions.clone(),
            policy,
        };
        if !settings.validate() {
            return Err("portable-policy-invalid".into());
        }
        Ok(settings)
    }
}
pub(crate) fn validate_mappings(bundle: &PortableBundle, m: &ImportMappings) -> Result<()> {
    if m.connections.len() > 64 || m.models.len() > 256 {
        return Err("portable-mapping-limit".into());
    }
    let mut cs = BTreeSet::new();
    let mut ms = BTreeSet::new();
    for c in &m.connections {
        if !id(&c.connection_id)
            || c.binding_revision == 0
            || !cs.insert(&c.alias)
            || !bundle
                .connection_references
                .iter()
                .any(|v| v.alias == c.alias)
        {
            return Err("portable-connection-mapping".into());
        }
    }
    for r in &m.models {
        if !id(&r.model_id)
            || !ms.insert(&r.alias)
            || !cs.contains(&r.connection_alias)
            || !bundle.model_references.iter().any(|v| {
                v.alias == r.alias
                    && v.connection_alias
                        .as_ref()
                        .is_none_or(|a| a == &r.connection_alias)
            })
        {
            return Err("portable-model-mapping".into());
        }
    }
    for r in &bundle.connection_references {
        if !cs.contains(&r.alias) {
            return Err("portable-connection-unresolved".into());
        }
    }
    for r in &bundle.model_references {
        if !ms.contains(&r.alias) {
            return Err("portable-model-unresolved".into());
        }
    }
    Ok(())
}
fn merge_requirements(target: &mut Requirements, local: &Requirements) -> Result<()> {
    target.subscription_only = true;
    target.forbid_network |= local.forbid_network;
    target.read_only |= local.read_only;
    target.require_review |= local.require_review;
    target
        .required_capabilities
        .extend(local.required_capabilities.clone());
    target.required_checks.extend(local.required_checks.clone());
    target.minimum_reasoning = target.minimum_reasoning.max(local.minimum_reasoning);
    target.minimum_strength = target.minimum_strength.max(local.minimum_strength);
    target.max_concurrency = minimum(target.max_concurrency, local.max_concurrency);
    target.max_context_bytes = minimum(target.max_context_bytes, local.max_context_bytes);
    for (a, b) in [
        (&mut target.forced_connection, &local.forced_connection),
        (&mut target.forced_model, &local.forced_model),
    ] {
        if let Some(b) = b {
            if a.as_ref().is_some_and(|a| a != b) {
                return Err("portable-mandatory-conflict".into());
            }
            *a = Some(b.clone());
        }
    }
    Ok(())
}
fn minimum<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

pub(crate) fn settings_from_state(state: &crate::persistence::SavedState) -> Result<Settings> {
    state
        .records()
        .definitions()
        .iter()
        .filter(|d| {
            d.kind == crate::persistence_records::DefinitionKind::Configuration
                && d.id == "desktop-settings"
        })
        .max_by_key(|d| d.version)
        .map(|d| {
            serde_json::from_value(d.body.clone()).map_err(|_| "portable-current-settings".into())
        })
        .unwrap_or_else(|| Ok(Settings::default()))
}
pub(crate) fn preview(
    bundle: &PortableBundle,
    current: &Settings,
    mappings: &ImportMappings,
    revision: u64,
    mut issues: Vec<ImportIssue>,
) -> ImportPreview {
    let candidate = match bundle.resolve(current, mappings) {
        Ok(s) => Some(s),
        Err(code) => {
            if !issues
                .iter()
                .any(|i| i.code == code && i.reference.is_none())
            {
                issues.push(ImportIssue {
                    code,
                    reference: None,
                });
            }
            None
        }
    };
    let changes = vec![
        "appearance".into(),
        "strategy-presets-and-overrides".into(),
        "context-review-and-local-mandatory-restrictions".into(),
        "personal-instructions-preserved".into(),
        "recurring-consents-not-imported".into(),
    ];
    let document_digest = hash(bundle);
    let preview_digest = hash(
        &json!({"documentDigest":document_digest,"mappings":mappings,"expectedRevision":revision,"candidate":candidate,"issues":issues}),
    );
    ImportPreview {
        schema_version: 1,
        expected_revision: revision,
        document_digest,
        preview_digest,
        can_apply: issues.is_empty(),
        issues,
        changes,
        connection_references: bundle.connection_references.clone(),
        model_references: bundle.model_references.clone(),
        definition_references: json!({"mode":bundle.policy.mode,"profile":bundle.policy.profile}),
        mappings: mappings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Settings {
        let mut source = Settings::default();
        source.policy.mode_defaults.preferred_models =
            Some(vec!["model-b".into(), "model-a".into(), "model-a2".into()]);
        source.policy.mode_defaults.fallback = Some(Fallback::Compatible);
        source.policy.model_ranks = [
            ("account-a", "model-a"),
            ("account-a", "model-a2"),
            ("account-b", "model-b"),
        ]
        .into_iter()
        .map(|(connection, model)| ModelRank {
            connection_id: connection.into(),
            model_id: model.into(),
            strength: 2,
            efficiency_order: 1,
            source_ref: "declared-preference".into(),
        })
        .collect();
        source
    }
    fn mappings(bundle: &PortableBundle) -> ImportMappings {
        ImportMappings {
            connections: vec![
                ConnectionMapping {
                    alias: "connection-1".into(),
                    connection_id: "destination-a".into(),
                    binding_revision: 3,
                },
                ConnectionMapping {
                    alias: "connection-2".into(),
                    connection_id: "destination-b".into(),
                    binding_revision: 4,
                },
            ],
            models: bundle
                .model_references
                .iter()
                .map(|m| ModelMapping {
                    alias: m.alias.clone(),
                    connection_alias: if m.source_model_id == "model-b" {
                        "connection-2"
                    } else {
                        "connection-1"
                    }
                    .into(),
                    model_id: format!("destination-{}", m.source_model_id),
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn preferred_model_owners_keep_order_without_becoming_mandatory_connections() {
        let bundle = export(&source()).unwrap();
        let mappings = mappings(&bundle);
        let imported = bundle.resolve(&Settings::default(), &mappings).unwrap();
        let preferences = &imported.policy.mode_defaults;
        assert_eq!(
            preferences.preferred_connections,
            Some(vec!["destination-b".into(), "destination-a".into()])
        );
        assert_eq!(
            preferences.preferred_models,
            Some(vec![
                "destination-model-b".into(),
                "destination-model-a".into(),
                "destination-model-a2".into()
            ])
        );
        assert_eq!(preferences.connection_id, None);
        assert_eq!(imported.policy.mode_requirements.forced_connection, None);
        assert_eq!(preferences.fallback, Some(Fallback::Compatible));
    }
    #[test]
    fn explicit_source_connection_preferences_keep_their_order() {
        let mut source = source();
        source.policy.mode_defaults.preferred_connections = Some(vec!["account-a".into()]);
        let bundle = export(&source).unwrap();
        let mappings = mappings(&bundle);
        let imported = bundle.resolve(&Settings::default(), &mappings).unwrap();
        assert_eq!(
            imported.policy.mode_defaults.preferred_connections,
            Some(vec!["destination-a".into()])
        );
        assert_eq!(imported.policy.mode_defaults.connection_id, None);
        assert_eq!(
            imported.policy.mode_defaults.fallback,
            Some(Fallback::Compatible)
        );
    }
}
