use super::{PolicyError, PolicyResult, id, safe_text};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const POLICY_SCHEMA_VERSION: u32 = 1;
pub const MAX_POLICY_DOCUMENT_BYTES: usize = 262_144;
/// Local metadata defaults, not provider promises or a paid-probe schedule.
pub const IDENTITY_TTL_MS: u64 = 300_000;
pub const AVAILABILITY_TTL_MS: u64 = 60_000;
pub const CAPABILITY_TTL_MS: u64 = 900_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservationTtls {
    pub identity_billing_ms: u64,
    pub availability_ms: u64,
    pub capability_configuration_ms: u64,
}
impl Default for ObservationTtls {
    fn default() -> Self {
        Self {
            identity_billing_ms: IDENTITY_TTL_MS,
            availability_ms: AVAILABILITY_TTL_MS,
            capability_configuration_ms: CAPABILITY_TTL_MS,
        }
    }
}
impl ObservationTtls {
    pub(crate) fn maximum_age(self, fact: &str) -> u64 {
        match fact {
            "identity" | "billing" => self.identity_billing_ms,
            "availability" | "model-availability" => self.availability_ms,
            _ => self.capability_configuration_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    Efficiency,
    Performance,
    #[default]
    Balanced,
    Custom,
}
impl Strategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Efficiency => "efficiency",
            Self::Performance => "performance",
            Self::Balanced => "balanced",
            Self::Custom => "custom",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Reasoning {
    Minimal,
    Low,
    Medium,
    High,
    Maximum,
}
impl Reasoning {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Maximum => "maximum",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    System,
    User,
    Project,
    Profile,
    CategoryRisk,
    Execution,
    Task,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SpecializedPreset {
    ScientificResearch,
    SoftwareArchitecture,
    ComplexPlanning,
    Debugging,
    SecurityReview,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Fallback {
    #[default]
    Block,
    Compatible,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BillingMode {
    #[default]
    SubscriptionOnly,
    ExplicitApi,
}
impl BillingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SubscriptionOnly => "subscription-only",
            Self::ExplicitApi => "explicit-api",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DefinitionBinding {
    pub id: String,
    pub version: u64,
}
impl DefinitionBinding {
    pub fn validate(&self) -> PolicyResult<()> {
        if !id(&self.id) || self.version == 0 {
            return Err(PolicyError::Schema);
        }
        Ok(())
    }
}
/// No global ranking is inferred. These are declared ordinals with visible origin.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelRank {
    pub connection_id: String,
    pub model_id: String,
    pub strength: u8,
    pub efficiency_order: u16,
    pub source_ref: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub strategy: Option<Strategy>,
    pub preset: Option<SpecializedPreset>,
    pub connection_id: Option<String>,
    pub model_id: Option<String>,
    pub reasoning: Option<Reasoning>,
    pub preferred_connections: Option<Vec<String>>,
    pub preferred_models: Option<Vec<String>>,
    pub fallback: Option<Fallback>,
    pub billing: Option<BillingMode>,
    pub billing_authorization_ref: Option<String>,
    pub sandbox_write: Option<bool>,
    pub network: Option<bool>,
    pub approval_ask: Option<bool>,
    pub concurrency: Option<u16>,
    pub context_bytes: Option<usize>,
    pub service_tier: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Requirements {
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
    pub required_checks: BTreeSet<String>,
    pub require_review: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyLayer {
    pub scope: Scope,
    pub reference: String,
    pub revision: u64,
    pub preferences: Preferences,
    pub requirements: Requirements,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyDocument {
    pub schema_version: u32,
    pub revision: u64,
    pub mode: DefinitionBinding,
    pub profile: DefinitionBinding,
    /// Explicitly applied at the system baseline, before the system layer.
    pub mode_defaults: Preferences,
    pub mode_requirements: Requirements,
    /// Explicitly applied at profile scope, before that scope's optional override.
    pub profile_defaults: Preferences,
    pub layers: Vec<PolicyLayer>,
    pub model_ranks: Vec<ModelRank>,
    #[serde(default)]
    pub observation_ttls: ObservationTtls,
}
impl Default for PolicyDocument {
    fn default() -> Self {
        Self {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: 1,
            mode: DefinitionBinding {
                id: "development".into(),
                version: 1,
            },
            profile: DefinitionBinding {
                id: "implementation-engineer".into(),
                version: 1,
            },
            mode_defaults: Preferences::default(),
            mode_requirements: Requirements::default(),
            profile_defaults: Preferences::default(),
            layers: vec![],
            model_ranks: vec![],
            observation_ttls: ObservationTtls::default(),
        }
    }
}
impl PolicyDocument {
    pub fn from_json(value: &serde_json::Value) -> PolicyResult<Self> {
        if serde_json::to_vec(value)
            .map_err(|_| PolicyError::Schema)?
            .len()
            > MAX_POLICY_DOCUMENT_BYTES
        {
            return Err(PolicyError::Limit);
        }
        let document: Self =
            serde_json::from_value(value.clone()).map_err(|_| PolicyError::Schema)?;
        document.validate()?;
        Ok(document)
    }
    pub fn validate(&self) -> PolicyResult<()> {
        if self.schema_version != POLICY_SCHEMA_VERSION || self.revision == 0 {
            return Err(PolicyError::Version);
        }
        self.mode.validate()?;
        self.profile.validate()?;
        validate_preferences(&self.mode_defaults)?;
        validate_preferences(&self.profile_defaults)?;
        validate_requirements(&self.mode_requirements)?;
        if [
            self.observation_ttls.identity_billing_ms,
            self.observation_ttls.availability_ms,
            self.observation_ttls.capability_configuration_ms,
        ]
        .iter()
        .any(|n| *n == 0 || *n > 86_400_000)
        {
            return Err(PolicyError::Limit);
        }
        if self.layers.len() > 7 || self.model_ranks.len() > 256 {
            return Err(PolicyError::Limit);
        }
        let mut scopes = BTreeSet::new();
        for layer in &self.layers {
            if !scopes.insert(layer.scope) || !id(&layer.reference) || layer.revision == 0 {
                return Err(PolicyError::Schema);
            }
            validate_preferences(&layer.preferences)?;
            validate_requirements(&layer.requirements)?;
        }
        let mut ranks = BTreeSet::new();
        for rank in &self.model_ranks {
            if !id(&rank.connection_id)
                || !id(&rank.model_id)
                || !id(&rank.source_ref)
                || !(1..=3).contains(&rank.strength)
                || !ranks.insert((&rank.connection_id, &rank.model_id))
            {
                return Err(PolicyError::Schema);
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| PolicyError::Schema)?
            .len()
            > MAX_POLICY_DOCUMENT_BYTES
        {
            return Err(PolicyError::Limit);
        }
        Ok(())
    }
}
fn validate_preferences(p: &Preferences) -> PolicyResult<()> {
    for value in [&p.connection_id, &p.model_id, &p.billing_authorization_ref]
        .into_iter()
        .flatten()
    {
        if !id(value) {
            return Err(PolicyError::Schema);
        }
    }
    for values in [&p.preferred_connections, &p.preferred_models]
        .into_iter()
        .flatten()
    {
        if values.len() > 256
            || values.iter().any(|v| !id(v))
            || values.iter().collect::<BTreeSet<_>>().len() != values.len()
        {
            return Err(PolicyError::Schema);
        }
    }
    if p.concurrency.is_some_and(|n| n == 0 || n > 64)
        || p.context_bytes.is_some_and(|n| n == 0 || n > 1_048_576)
    {
        return Err(PolicyError::Limit);
    }
    if let Some(tier) = &p.service_tier {
        safe_text(tier, 80)?;
        if !id(tier) {
            return Err(PolicyError::Schema);
        }
    }
    Ok(())
}
pub(crate) fn validate_requirements(r: &Requirements) -> PolicyResult<()> {
    if r.minimum_strength > 3
        || r.max_concurrency.is_some_and(|n| n == 0 || n > 64)
        || r.max_context_bytes.is_some_and(|n| n == 0 || n > 1_048_576)
        || r.required_capabilities.len() > 11
        || r.required_checks.len() > 32
    {
        return Err(PolicyError::Limit);
    }
    for value in [&r.forced_model, &r.forced_connection]
        .into_iter()
        .flatten()
    {
        if !id(value) {
            return Err(PolicyError::Schema);
        }
    }
    if r.required_capabilities
        .iter()
        .any(|v| !crate::runtime_contract::CAPABILITIES.contains(&v.as_str()))
        || r.required_checks.iter().any(|v| !id(v))
    {
        return Err(PolicyError::Schema);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum TaskLevel {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskKind {
    Implementation,
    Documentation,
    Research,
    Architecture,
    Planning,
    Debugging,
    SecurityReview,
    GitStatus,
    GitDiff,
    FileExists,
    ProcessStatus,
    CheckResults,
}
impl TaskKind {
    /// Classification only: a host must separately authorize the local query.
    pub fn is_mechanical(self) -> bool {
        matches!(
            self,
            Self::GitStatus
                | Self::GitDiff
                | Self::FileExists
                | Self::ProcessStatus
                | Self::CheckResults
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskClassification {
    pub kind: TaskKind,
    pub complexity: Option<TaskLevel>,
    pub risk: Option<TaskLevel>,
    pub importance: Option<TaskLevel>,
    pub source_ref: String,
}
impl TaskClassification {
    pub fn routine(kind: TaskKind, source_ref: impl Into<String>) -> Self {
        Self {
            kind,
            complexity: None,
            risk: None,
            importance: None,
            source_ref: source_ref.into(),
        }
    }
    pub fn severity(&self) -> TaskLevel {
        self.complexity
            .unwrap_or_default()
            .max(self.risk.unwrap_or_default())
            .max(self.importance.unwrap_or_default())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectivePreferences {
    pub strategy: Strategy,
    pub preset: Option<SpecializedPreset>,
    pub connection_id: Option<String>,
    pub model_id: Option<String>,
    pub reasoning: Option<Reasoning>,
    pub preferred_connections: Vec<String>,
    pub preferred_models: Vec<String>,
    pub fallback: Fallback,
    pub billing: BillingMode,
    pub billing_authorization_ref: Option<String>,
    pub sandbox_write: bool,
    pub network: bool,
    pub approval_ask: bool,
    pub concurrency: u16,
    pub context_bytes: usize,
    pub service_tier: Option<String>,
}
impl Default for EffectivePreferences {
    fn default() -> Self {
        Self {
            strategy: Strategy::Balanced,
            preset: None,
            connection_id: None,
            model_id: None,
            reasoning: None,
            preferred_connections: vec![],
            preferred_models: vec![],
            fallback: Fallback::Block,
            billing: BillingMode::SubscriptionOnly,
            billing_authorization_ref: None,
            sandbox_write: true,
            network: false,
            approval_ask: false,
            concurrency: 1,
            context_bytes: 65_536,
            service_tier: None,
        }
    }
}
pub(crate) fn merge_preferences(
    target: &mut EffectivePreferences,
    patch: &Preferences,
    origin: &str,
    origins: &mut BTreeMap<String, String>,
) {
    macro_rules! copy_field { ($($f:ident),*) => {$({ if let Some(value) = &patch.$f { target.$f = value.clone(); origins.insert(stringify!($f).into(), origin.into()); } })*}; }
    macro_rules! optional { ($($f:ident),*) => {$({ if patch.$f.is_some() { target.$f = patch.$f.clone(); origins.insert(stringify!($f).into(), origin.into()); } })*}; }
    copy_field!(
        strategy,
        preferred_connections,
        preferred_models,
        fallback,
        billing,
        sandbox_write,
        network,
        approval_ask,
        concurrency,
        context_bytes
    );
    optional!(
        preset,
        connection_id,
        model_id,
        reasoning,
        billing_authorization_ref,
        service_tier
    );
}
