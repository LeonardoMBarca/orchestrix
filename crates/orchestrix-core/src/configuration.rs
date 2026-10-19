//! Versioned local action modes, executor profiles and scoped policy preferences.
//!
//! The existing immutable Definition records are the only source of authority.
//! Reading, composing and previewing these records performs no provider, file,
//! process or inference operation and confers no execution permission.
use crate::{
    desktop::Settings,
    domain::{LocalRef, ProjectId},
    persistence::{Command, Disposition, Request, SavedState, Store, StoreError},
    persistence_records::{DefinitionInput, DefinitionKind, RecordsCommand},
    policy::{
        self, ContextInput, ContextSource, ContextSourceIdentity, ContextSourceKind,
        DefinitionBinding, PolicyDocument, PolicyLayer, Preferences, Requirements, Scope,
        SpecializedPreset, TaskClassification, TaskKind,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CONFIGURATION_SCHEMA_VERSION: u32 = 1;
pub const NATIVE_DEFINITION_VERSION: u64 = 2;
const MAX_DEFINITION_BYTES: usize = 32_768;
const MODE_PREFIX: &str = "mode-definition-";
const PROFILE_PREFIX: &str = "profile-definition-";
const SCOPE_PREFIX: &str = "scoped-policy-";
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionType {
    Mode,
    Profile,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionOrigin {
    Native,
    Local,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerRole {
    Implementation,
    Review,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionSupport {
    ManualWorker,
    ArtifactFlowUnproven,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoleBinding {
    pub role: WorkerRole,
    pub profile: DefinitionBinding,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeDefinition {
    pub id: String,
    pub version: u64,
    pub edit_revision: u64,
    pub origin: DefinitionOrigin,
    pub base: Option<DefinitionBinding>,
    pub archived: bool,
    pub name: String,
    pub purpose: String,
    pub instructions: String,
    pub roles: Vec<RoleBinding>,
    pub outputs: Vec<String>,
    pub criteria: Vec<String>,
    pub preferences: Preferences,
    pub requirements: Requirements,
    pub execution_support: ExecutionSupport,
}
impl ModeDefinition {
    pub fn binding(&self) -> DefinitionBinding {
        DefinitionBinding {
            id: self.id.clone(),
            version: self.version.max(1),
        }
    }
    pub fn review_profile(&self) -> Result<DefinitionBinding> {
        self.roles
            .iter()
            .find(|r| r.role == WorkerRole::Review)
            .map(|r| r.profile.clone())
            .ok_or_else(|| "configuration-review-role".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDefinition {
    pub id: String,
    pub version: u64,
    pub edit_revision: u64,
    pub origin: DefinitionOrigin,
    pub base: Option<DefinitionBinding>,
    pub archived: bool,
    pub name: String,
    pub role: WorkerRole,
    pub instructions: String,
    pub output_language: String,
    pub conventions: Vec<String>,
    pub specialty: String,
    pub preferences: Preferences,
    pub requirements: Requirements,
}
impl ProfileDefinition {
    pub fn binding(&self) -> DefinitionBinding {
        DefinitionBinding {
            id: self.id.clone(),
            version: self.version.max(1),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopedPolicy {
    pub scope: Scope,
    pub project_id: Option<String>,
    pub version: u64,
    pub edit_revision: u64,
    pub removed: bool,
    pub preferences: Preferences,
    pub requirements: Requirements,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationCatalog {
    pub schema_version: u32,
    pub revision: u64,
    pub execution_fingerprint: String,
    pub modes: Vec<ModeDefinition>,
    pub profiles: Vec<ProfileDefinition>,
    pub scopes: Vec<ScopedPolicy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConfigurationRequest {
    Catalog {},
    Get {
        kind: DefinitionType,
        id: String,
        version: u64,
    },
    Preview {
        mode: DefinitionBinding,
        profile: DefinitionBinding,
        project_id: Option<String>,
        #[serde(default)]
        mode_draft: Option<Box<ModeDefinition>>,
        #[serde(default)]
        profile_draft: Option<Box<ProfileDefinition>>,
        #[serde(default)]
        scope_draft: Option<Box<ScopedPolicy>>,
    },
    SaveMode {
        command_id: String,
        expected_revision: u64,
        definition: ModeDefinition,
    },
    SaveProfile {
        command_id: String,
        expected_revision: u64,
        definition: ProfileDefinition,
    },
    Restore {
        command_id: String,
        expected_revision: u64,
        kind: DefinitionType,
        id: String,
    },
    Archive {
        command_id: String,
        expected_revision: u64,
        kind: DefinitionType,
        id: String,
    },
    SaveScope {
        command_id: String,
        expected_revision: u64,
        scope: ScopedPolicy,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConfigurationCommand {
    SaveMode { definition: ModeDefinition },
    SaveProfile { definition: ProfileDefinition },
    Restore { kind: DefinitionType, id: String },
    Archive { kind: DefinitionType, id: String },
    SaveScope { scope: ScopedPolicy },
}

pub fn native_profiles() -> Vec<ProfileDefinition> {
    [
        ("implementation-engineer", "Implementation Engineer", WorkerRole::Implementation,
            "Implement only the authorized objective. Preserve unrelated work and report observable checks honestly."),
        ("independent-reviewer", "Independent Reviewer", WorkerRole::Review,
            "Review the exact supplied artifact and evidence independently. Do not edit files; return the structured verdict required by the review contract."),
    ].into_iter().map(|(id, name, role, instructions)| ProfileDefinition {
        id: id.into(), version: NATIVE_DEFINITION_VERSION, edit_revision: 0,
        origin: DefinitionOrigin::Native, base: None, archived: false,
        name: name.into(), role, instructions: instructions.into(),
        output_language: "inherit".into(), conventions: vec![], specialty: String::new(),
        preferences: if role == WorkerRole::Review { Preferences { sandbox_write: Some(false), network: Some(false), ..Preferences::default() } } else { Preferences::default() },
        requirements: Requirements { subscription_only: true, read_only: role == WorkerRole::Review,
            max_concurrency: Some(1), ..Requirements::default() },
    }).collect()
}
pub fn native_modes() -> Vec<ModeDefinition> {
    [
        ("development", "Development", "Implement features and fix code within an authorized objective.",
            "Make focused changes and expose their verification and review evidence.", "Identified code changes", "Meet the supplied acceptance criteria and checks", None, ExecutionSupport::ManualWorker),
        ("research", "Research", "Investigate supplied scientific material and distinguish evidence, hypotheses and interpretation.",
            "Identify sources and uncertainties. External discovery requires separately authorized, supported tools.", "Traceable research synthesis", "Claims and references must be verifiable", Some(SpecializedPreset::ScientificResearch), ExecutionSupport::ArtifactFlowUnproven),
        ("documentation", "Documentation", "Create or update consistent project documentation from observed authorized material.",
            "Follow the project structure and document discrepancies and reference sources.", "Project documentation", "References and statements must match observed material", None, ExecutionSupport::ManualWorker),
        ("presentations", "Presentations", "Prepare a presentation narrative and supported deliverables from supplied content.",
            "Separate a presentation outline from an actually generated, verified PPTX or PDF. Unsupported formats require an explicit alternative.", "Presentation structure and supported artifacts", "Verify content and the actual output format", None, ExecutionSupport::ArtifactFlowUnproven),
        ("refactoring", "Refactoring", "Restructure code within an explicit scope while preserving the agreed behavior.",
            "Identify behavior changes explicitly and preserve requirements with relevant checks.", "Scoped code restructuring", "Demonstrate preservation of the agreed behavior", None, ExecutionSupport::ManualWorker),
        ("improvement-review", "Improvement Review", "Investigate code or documents and propose specific, evidenced improvements.",
            "Provide rationale, impact and an implementation path. Analysis does not authorize applying changes.", "Actionable improvement proposals", "Each proposal identifies evidence, impact and implementation steps", None, ExecutionSupport::ArtifactFlowUnproven),
    ].into_iter().map(|(id,name,purpose,instructions,output,criterion,preset,support)| ModeDefinition {
        id:id.into(), version:NATIVE_DEFINITION_VERSION, edit_revision:0, origin:DefinitionOrigin::Native,
        base:None, archived:false, name:name.into(), purpose:purpose.into(), instructions:instructions.into(),
        roles: vec![
            RoleBinding { role:WorkerRole::Implementation, profile:DefinitionBinding { id:"implementation-engineer".into(), version:NATIVE_DEFINITION_VERSION } },
            RoleBinding { role:WorkerRole::Review, profile:DefinitionBinding { id:"independent-reviewer".into(), version:NATIVE_DEFINITION_VERSION } },
        ],
        outputs:vec![output.into()], criteria:vec![criterion.into()],
        preferences:Preferences { preset, ..Preferences::default() },
        requirements:Requirements { subscription_only:true, require_review:true, max_concurrency:Some(1),
            read_only:id == "improvement-review", ..Requirements::default() }, execution_support:support,
    }).collect()
}

fn key(kind: DefinitionType, id: &str) -> String {
    format!(
        "{}{id}",
        match kind {
            DefinitionType::Mode => MODE_PREFIX,
            DefinitionType::Profile => PROFILE_PREFIX,
        }
    )
}
fn stored<T: for<'de> Deserialize<'de>>(
    state: &SavedState,
    key: &str,
    version: Option<u64>,
) -> Result<Option<T>> {
    state
        .records()
        .definitions()
        .iter()
        .filter(|d| {
            d.kind == DefinitionKind::Configuration
                && d.id == key
                && version.is_none_or(|v| v == d.version)
        })
        .max_by_key(|d| d.version)
        .map(|d| {
            if d.body["schemaVersion"] != CONFIGURATION_SCHEMA_VERSION {
                return Err("configuration-version".into());
            }
            serde_json::from_value(d.body["definition"].clone())
                .map_err(|_| "configuration-record".into())
        })
        .transpose()
}
pub fn mode_definition(state: &SavedState, binding: &DefinitionBinding) -> Result<ModeDefinition> {
    binding
        .validate()
        .map_err(|_| "configuration-mode-reference")?;
    if let Some(value) = stored(
        state,
        &key(DefinitionType::Mode, &binding.id),
        Some(binding.version),
    )? {
        return Ok(value);
    }
    native_modes()
        .into_iter()
        .find(|d| d.id == binding.id && d.version == binding.version)
        .ok_or_else(|| "configuration-mode-unresolved".into())
}
pub fn profile_definition(
    state: &SavedState,
    binding: &DefinitionBinding,
) -> Result<ProfileDefinition> {
    binding
        .validate()
        .map_err(|_| "configuration-profile-reference")?;
    if let Some(value) = stored(
        state,
        &key(DefinitionType::Profile, &binding.id),
        Some(binding.version),
    )? {
        return Ok(value);
    }
    native_profiles()
        .into_iter()
        .find(|d| d.id == binding.id && d.version == binding.version)
        .ok_or_else(|| "configuration-profile-unresolved".into())
}
fn latest_mode(state: &SavedState, id: &str) -> Result<Option<ModeDefinition>> {
    stored(state, &key(DefinitionType::Mode, id), None)
        .map(|v| v.or_else(|| native_modes().into_iter().find(|d| d.id == id)))
}
fn latest_profile(state: &SavedState, id: &str) -> Result<Option<ProfileDefinition>> {
    stored(state, &key(DefinitionType::Profile, id), None)
        .map(|v| v.or_else(|| native_profiles().into_iter().find(|d| d.id == id)))
}
pub fn catalog(state: &SavedState) -> Result<ConfigurationCatalog> {
    let mut modes = native_modes();
    let mut profiles = native_profiles();
    let mut scopes: Vec<ScopedPolicy> = vec![];
    for d in state
        .records()
        .definitions()
        .iter()
        .filter(|d| d.kind == DefinitionKind::Configuration)
    {
        if d.id.starts_with(MODE_PREFIX) {
            let item: ModeDefinition = stored(state, &d.id, None)?.ok_or("configuration-record")?;
            modes.retain(|v| v.id != item.id);
            modes.push(item);
        } else if d.id.starts_with(PROFILE_PREFIX) {
            let item: ProfileDefinition =
                stored(state, &d.id, None)?.ok_or("configuration-record")?;
            profiles.retain(|v| v.id != item.id);
            profiles.push(item);
        } else if d.id.starts_with(SCOPE_PREFIX) {
            let item: ScopedPolicy = stored(state, &d.id, None)?.ok_or("configuration-record")?;
            scopes.retain(|v| !(v.scope == item.scope && v.project_id == item.project_id));
            scopes.push(item);
        }
    }
    modes.sort_by(|a, b| a.id.cmp(&b.id));
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    scopes.sort_by(|a, b| (a.scope, &a.project_id).cmp(&(b.scope, &b.project_id)));
    let settings = crate::policy_portability::settings_from_state(state)?;
    Ok(ConfigurationCatalog {
        schema_version: CONFIGURATION_SCHEMA_VERSION,
        revision: state.revision(),
        execution_fingerprint: execution_fingerprint(state, &settings)?,
        modes,
        profiles,
        scopes,
    })
}

fn validate_id(id: &str) -> Result<()> {
    if !policy::id(id) || id.len() > 48 {
        return Err("configuration-id".into());
    }
    Ok(())
}
fn text(value: &str, max: usize, required: bool) -> Result<()> {
    policy::safe_text(value, max).map_err(|_| "configuration-text")?;
    if required && value.trim().is_empty() {
        return Err("configuration-text-empty".into());
    }
    Ok(())
}
fn texts(values: &[String], required: bool) -> Result<()> {
    if values.len() > 16 || (required && values.is_empty()) {
        return Err("configuration-list-limit".into());
    }
    for v in values {
        text(v, 1024, true)?;
    }
    Ok(())
}
fn validate_policy_fields(preferences: &Preferences, requirements: &Requirements) -> Result<()> {
    // Configurable preferences cannot carry a billing authorization, credentials or
    // remove the product's mandatory single-runtime/Subscription Only boundary.
    if preferences
        .billing
        .is_some_and(|b| b != policy::BillingMode::SubscriptionOnly)
        || preferences.billing_authorization_ref.is_some()
        || preferences.concurrency.is_some_and(|n| n != 1)
        || requirements.max_concurrency.is_some_and(|n| n != 1)
    {
        return Err("configuration-mandatory-boundary".into());
    }
    let document = PolicyDocument {
        mode_defaults: preferences.clone(),
        mode_requirements: requirements.clone(),
        ..PolicyDocument::default()
    };
    document
        .validate()
        .map_err(|_| "configuration-policy-fields".into())
}
fn validate_mode(state: &SavedState, d: &ModeDefinition, saving: bool) -> Result<()> {
    if d.version >= i64::MAX as u64 || d.edit_revision >= i64::MAX as u64 {
        return Err("configuration-version".into());
    }
    validate_id(&d.id)?;
    text(&d.name, 120, true)?;
    text(&d.purpose, 2048, true)?;
    text(&d.instructions, 8192, false)?;
    texts(&d.outputs, true)?;
    texts(&d.criteria, true)?;
    validate_policy_fields(&d.preferences, &d.requirements)?;
    if serde_json::to_vec(d)
        .map_err(|_| "configuration-schema")?
        .len()
        > MAX_DEFINITION_BYTES
    {
        return Err("configuration-size".into());
    }
    if d.roles.len() != 2
        || d.roles
            .iter()
            .filter(|r| r.role == WorkerRole::Implementation)
            .count()
            != 1
        || d.roles
            .iter()
            .filter(|r| r.role == WorkerRole::Review)
            .count()
            != 1
    {
        return Err("configuration-role-sequence-unsupported".into());
    }
    for r in &d.roles {
        let p = profile_definition(state, &r.profile)?;
        if p.role != r.role
            || p.archived
            || latest_profile(state, &p.id)?.is_some_and(|v| v.archived)
        {
            return Err("configuration-profile-role-or-archived".into());
        }
    }
    match d.origin {
        DefinitionOrigin::Native => {
            let native = native_modes()
                .into_iter()
                .find(|v| v.id == d.id)
                .ok_or("configuration-native-identity")?;
            if d.base.is_some()
                || d.name != native.name
                || d.purpose != native.purpose
                || d.requirements != native.requirements
                || d.execution_support != native.execution_support
            {
                return Err("configuration-native-purpose-requirements".into());
            }
        }
        DefinitionOrigin::Local => {
            if native_modes().iter().any(|v| v.id == d.id) {
                return Err("configuration-native-identity".into());
            }
            let base =
                mode_definition(state, d.base.as_ref().ok_or("configuration-base-required")?)?;
            if base.origin != DefinitionOrigin::Native
                || d.execution_support != base.execution_support
                || !requirements_include(&d.requirements, &base.requirements)
            {
                return Err("configuration-base-requirements".into());
            }
        }
    }
    if saving {
        let prior = latest_mode(state, &d.id)?;
        if prior.as_ref().is_some_and(|p| {
            p.version != d.version
                || p.edit_revision != d.edit_revision
                || p.origin != d.origin
                || p.base != d.base
        }) || (prior.is_none() && (d.version != 0 || d.edit_revision != 0))
        {
            return Err("configuration-definition-stale".into());
        }
    }
    Ok(())
}
fn validate_profile(state: &SavedState, d: &ProfileDefinition, saving: bool) -> Result<()> {
    if d.version >= i64::MAX as u64 || d.edit_revision >= i64::MAX as u64 {
        return Err("configuration-version".into());
    }
    validate_id(&d.id)?;
    text(&d.name, 120, true)?;
    text(&d.instructions, 8192, false)?;
    text(&d.specialty, 2048, false)?;
    texts(&d.conventions, false)?;
    if !["inherit", "en", "pt-BR", "es"].contains(&d.output_language.as_str()) {
        return Err("configuration-output-language".into());
    }
    validate_policy_fields(&d.preferences, &d.requirements)?;
    if serde_json::to_vec(d)
        .map_err(|_| "configuration-schema")?
        .len()
        > MAX_DEFINITION_BYTES
    {
        return Err("configuration-size".into());
    }
    let baseline = match d.origin {
        DefinitionOrigin::Native => {
            if d.base.is_some() {
                return Err("configuration-native-identity".into());
            }
            native_profiles()
                .into_iter()
                .find(|v| v.id == d.id)
                .ok_or("configuration-native-identity")?
        }
        DefinitionOrigin::Local => {
            if native_profiles().iter().any(|v| v.id == d.id) {
                return Err("configuration-native-identity".into());
            }
            let base =
                profile_definition(state, d.base.as_ref().ok_or("configuration-base-required")?)?;
            if base.origin != DefinitionOrigin::Native {
                return Err("configuration-base-chain-unsupported".into());
            }
            base
        }
    };
    if d.role != baseline.role
        || !requirements_include(&d.requirements, &baseline.requirements)
        || (d.role == WorkerRole::Review
            && (!d.requirements.read_only || d.preferences.sandbox_write == Some(true)))
    {
        return Err("configuration-role-permissions".into());
    }
    if saving {
        let prior = latest_profile(state, &d.id)?;
        if prior.as_ref().is_some_and(|p| {
            p.version != d.version
                || p.edit_revision != d.edit_revision
                || p.origin != d.origin
                || p.base != d.base
                || p.role != d.role
        }) || (prior.is_none() && (d.version != 0 || d.edit_revision != 0))
        {
            return Err("configuration-definition-stale".into());
        }
    }
    Ok(())
}
fn requirements_include(value: &Requirements, base: &Requirements) -> bool {
    (!base.subscription_only || value.subscription_only)
        && (!base.read_only || value.read_only)
        && (!base.forbid_network || value.forbid_network)
        && (!base.require_review || value.require_review)
        && base
            .required_capabilities
            .is_subset(&value.required_capabilities)
        && base.required_checks.is_subset(&value.required_checks)
        && value.minimum_strength >= base.minimum_strength
        && base
            .minimum_reasoning
            .is_none_or(|v| value.minimum_reasoning.is_some_and(|n| n >= v))
        && base
            .max_concurrency
            .is_none_or(|v| value.max_concurrency.is_some_and(|n| n <= v))
        && base
            .max_context_bytes
            .is_none_or(|v| value.max_context_bytes.is_some_and(|n| n <= v))
        && base
            .forced_model
            .as_ref()
            .is_none_or(|v| value.forced_model.as_ref() == Some(v))
        && base
            .forced_connection
            .as_ref()
            .is_none_or(|v| value.forced_connection.as_ref() == Some(v))
}
fn scope_key(scope: &ScopedPolicy) -> Result<String> {
    match (scope.scope, &scope.project_id) {
        (Scope::User, None) => Ok(format!("{SCOPE_PREFIX}user")),
        (Scope::Project, Some(id)) if policy::id(id) => Ok(format!(
            "{SCOPE_PREFIX}project-{}",
            &policy::hash(id.as_bytes())[..32]
        )),
        _ => Err("configuration-scope".into()),
    }
}
fn validate_scope(state: &SavedState, scope: &ScopedPolicy, saving: bool) -> Result<()> {
    if scope.version >= i64::MAX as u64 || scope.edit_revision >= i64::MAX as u64 {
        return Err("configuration-version".into());
    }
    let key = scope_key(scope)?;
    if let Some(id) = &scope.project_id {
        let id = ProjectId::new(id).map_err(|_| "configuration-project")?;
        if !state.projects().contains_key(&id) {
            return Err("configuration-project-unresolved".into());
        }
    }
    validate_policy_fields(&scope.preferences, &scope.requirements)?;
    if scope.removed && scope.preferences != Preferences::default() {
        return Err("configuration-removed-scope-content".into());
    }
    if saving {
        let prior: Option<ScopedPolicy> = stored(state, &key, None)?;
        if prior
            .as_ref()
            .is_some_and(|p| p.version != scope.version || p.edit_revision != scope.edit_revision)
            || (prior.is_none() && (scope.version != 0 || scope.edit_revision != 0))
        {
            return Err("configuration-scope-stale".into());
        }
    }
    Ok(())
}

fn minimal_mode(d: &ModeDefinition) -> DefinitionInput {
    DefinitionInput {
        kind: DefinitionKind::Mode,
        id: d.id.clone(),
        version: d.version,
        body: json!({"name":d.name,"purpose":d.purpose}),
        references: vec![],
    }
}
fn minimal_profile(d: &ProfileDefinition) -> DefinitionInput {
    DefinitionInput {
        kind: DefinitionKind::Profile,
        id: d.id.clone(),
        version: d.version,
        body: json!({"name":d.name,"role":match d.role { WorkerRole::Implementation=>"implementer",WorkerRole::Review=>"reviewer" },
            "instructions_ref":format!("profile-instructions-{}",&policy::hash(format!("{}@{}",d.id,d.version).as_bytes())[..32])}),
        references: vec![],
    }
}
fn rich_mode(d: &ModeDefinition) -> DefinitionInput {
    DefinitionInput {
        kind: DefinitionKind::Configuration,
        id: key(DefinitionType::Mode, &d.id),
        version: d.version,
        body: json!({"schemaVersion":CONFIGURATION_SCHEMA_VERSION,"kind":"mode","definition":d}),
        references: vec![minimal_mode(d).reference()],
    }
}
fn rich_profile(d: &ProfileDefinition) -> DefinitionInput {
    DefinitionInput {
        kind: DefinitionKind::Configuration,
        id: key(DefinitionType::Profile, &d.id),
        version: d.version,
        body: json!({"schemaVersion":CONFIGURATION_SCHEMA_VERSION,"kind":"profile","definition":d}),
        references: vec![minimal_profile(d).reference()],
    }
}
fn put(state: &mut SavedState, d: DefinitionInput) -> std::result::Result<(), StoreError> {
    if let Some(old) = state.records().definition(&d.reference()) {
        return if old == &d {
            Ok(())
        } else {
            Err(StoreError::Configuration)
        };
    }
    Command::Records(RecordsCommand::AddDefinition(d))
        .apply(state)
        .map(|_| ())
}
fn store_mode(state: &mut SavedState, d: &ModeDefinition) -> std::result::Result<(), StoreError> {
    put(state, minimal_mode(d))?;
    put(state, rich_mode(d))
}
fn store_profile(
    state: &mut SavedState,
    d: &ProfileDefinition,
) -> std::result::Result<(), StoreError> {
    put(state, minimal_profile(d))?;
    put(state, rich_profile(d))
}
impl ConfigurationCommand {
    pub fn fingerprint(&self) -> Value {
        json!(self)
    }
    pub fn bounded(&self) -> bool {
        serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 65_536)
    }
    pub fn apply(&self, state: &mut SavedState) -> std::result::Result<Value, StoreError> {
        if !self.bounded() {
            return Err(StoreError::Configuration);
        }
        let revision = state.revision() + 1;
        match self {
            Self::SaveMode { definition } => {
                validate_mode(state, definition, true).map_err(|_| StoreError::Configuration)?;
                let mut d = definition.clone();
                d.version += 1;
                d.edit_revision = revision;
                if d.origin == DefinitionOrigin::Native {
                    let baseline = native_modes()
                        .into_iter()
                        .find(|m| m.id == d.id)
                        .ok_or(StoreError::Configuration)?;
                    store_mode(state, &baseline)?;
                }
                for r in &d.roles {
                    let p = profile_definition(state, &r.profile)
                        .map_err(|_| StoreError::Configuration)?;
                    store_profile(state, &p)?;
                }
                if let Some(base) = &d.base {
                    let base =
                        mode_definition(state, base).map_err(|_| StoreError::Configuration)?;
                    store_mode(state, &base)?;
                }
                store_mode(state, &d)?;
                Ok(Value::Null)
            }
            Self::SaveProfile { definition } => {
                validate_profile(state, definition, true).map_err(|_| StoreError::Configuration)?;
                let mut d = definition.clone();
                d.version += 1;
                d.edit_revision = revision;
                if d.origin == DefinitionOrigin::Native {
                    let baseline = native_profiles()
                        .into_iter()
                        .find(|p| p.id == d.id)
                        .ok_or(StoreError::Configuration)?;
                    store_profile(state, &baseline)?;
                }
                if let Some(base) = &d.base {
                    let base =
                        profile_definition(state, base).map_err(|_| StoreError::Configuration)?;
                    store_profile(state, &base)?;
                }
                store_profile(state, &d)?;
                Ok(Value::Null)
            }
            Self::Restore { kind, id } | Self::Archive { kind, id } => {
                let archive = matches!(self, Self::Archive { .. });
                match kind {
                    DefinitionType::Mode => {
                        let old = latest_mode(state, id)
                            .map_err(|_| StoreError::Configuration)?
                            .ok_or(StoreError::Configuration)?;
                        let mut d = if archive {
                            old.clone()
                        } else {
                            match old.origin {
                                DefinitionOrigin::Native => native_modes()
                                    .into_iter()
                                    .find(|v| &v.id == id)
                                    .ok_or(StoreError::Configuration)?,
                                DefinitionOrigin::Local => {
                                    let mut b = mode_definition(
                                        state,
                                        old.base.as_ref().ok_or(StoreError::Configuration)?,
                                    )
                                    .map_err(|_| StoreError::Configuration)?;
                                    b.id = old.id.clone();
                                    b.origin = DefinitionOrigin::Local;
                                    b.base = old.base.clone();
                                    b
                                }
                            }
                        };
                        d.version = old.version + 1;
                        d.edit_revision = revision;
                        d.archived = archive;
                        validate_mode(state, &d, false).map_err(|_| StoreError::Configuration)?;
                        if d.origin == DefinitionOrigin::Native {
                            let baseline = native_modes()
                                .into_iter()
                                .find(|m| m.id == d.id)
                                .ok_or(StoreError::Configuration)?;
                            store_mode(state, &baseline)?;
                        }
                        for r in &d.roles {
                            let p = profile_definition(state, &r.profile)
                                .map_err(|_| StoreError::Configuration)?;
                            store_profile(state, &p)?;
                        }
                        store_mode(state, &d)?;
                        Ok(Value::Null)
                    }
                    DefinitionType::Profile => {
                        let old = latest_profile(state, id)
                            .map_err(|_| StoreError::Configuration)?
                            .ok_or(StoreError::Configuration)?;
                        let mut d = if archive {
                            old.clone()
                        } else {
                            match old.origin {
                                DefinitionOrigin::Native => native_profiles()
                                    .into_iter()
                                    .find(|v| &v.id == id)
                                    .ok_or(StoreError::Configuration)?,
                                DefinitionOrigin::Local => {
                                    let mut b = profile_definition(
                                        state,
                                        old.base.as_ref().ok_or(StoreError::Configuration)?,
                                    )
                                    .map_err(|_| StoreError::Configuration)?;
                                    b.id = old.id.clone();
                                    b.origin = DefinitionOrigin::Local;
                                    b.base = old.base.clone();
                                    b
                                }
                            }
                        };
                        d.version = old.version + 1;
                        d.edit_revision = revision;
                        d.archived = archive;
                        validate_profile(state, &d, false)
                            .map_err(|_| StoreError::Configuration)?;
                        if d.origin == DefinitionOrigin::Native {
                            let baseline = native_profiles()
                                .into_iter()
                                .find(|p| p.id == d.id)
                                .ok_or(StoreError::Configuration)?;
                            store_profile(state, &baseline)?;
                        }
                        store_profile(state, &d)?;
                        Ok(Value::Null)
                    }
                }
            }
            Self::SaveScope { scope } => {
                validate_scope(state, scope, true).map_err(|_| StoreError::Configuration)?;
                let mut d = scope.clone();
                if d.removed {
                    // Removing a preference override is not authorization to
                    // remove its accumulated mandatory restrictions.
                    let prior: Option<ScopedPolicy> = stored(
                        state,
                        &scope_key(&d).map_err(|_| StoreError::Configuration)?,
                        None,
                    )
                    .map_err(|_| StoreError::Configuration)?;
                    if let Some(prior) = prior {
                        merge_requirements(&mut d.requirements, &prior.requirements)
                            .map_err(|_| StoreError::Configuration)?;
                    }
                }
                d.version += 1;
                d.edit_revision = revision;
                put(
                    state,
                    DefinitionInput {
                        kind: DefinitionKind::Configuration,
                        id: scope_key(&d).map_err(|_| StoreError::Configuration)?,
                        version: d.version,
                        body: json!({"schemaVersion":CONFIGURATION_SCHEMA_VERSION,"kind":"scope","definition":d}),
                        references: vec![],
                    },
                )?;
                Ok(Value::Null)
            }
        }
    }
}

pub fn handle(store: &mut Store, request: &ConfigurationRequest) -> Result<Value> {
    let (id, revision, command) = match request {
        ConfigurationRequest::Catalog {} => {
            return serde_json::to_value(catalog(
                &store.load().map_err(|_| "configuration-read")?,
            )?)
            .map_err(|_| "configuration-schema".into());
        }
        ConfigurationRequest::Get { kind, id, version } => {
            let state = store.load().map_err(|_| "configuration-read")?;
            let binding = DefinitionBinding {
                id: id.clone(),
                version: *version,
            };
            return match kind {
                DefinitionType::Mode => mode_definition(&state, &binding).map(|d| json!(d)),
                DefinitionType::Profile => profile_definition(&state, &binding).map(|d| json!(d)),
            };
        }
        ConfigurationRequest::Preview {
            mode,
            profile,
            project_id,
            mode_draft,
            profile_draft,
            scope_draft,
        } => {
            let state = store.load().map_err(|_| "configuration-read")?;
            let mut result = preview(
                &state,
                mode,
                profile,
                project_id.as_deref(),
                mode_draft.as_deref(),
                profile_draft.as_deref(),
                scope_draft.as_deref(),
            )?;
            let policy: PolicyDocument = serde_json::from_value(result["policy"].clone())
                .map_err(|_| "configuration-preview")?;
            apply_stored_observations(store, &policy, &mut result)?;
            return Ok(result);
        }
        ConfigurationRequest::SaveMode {
            command_id,
            expected_revision,
            definition,
        } => (
            command_id,
            *expected_revision,
            ConfigurationCommand::SaveMode {
                definition: definition.clone(),
            },
        ),
        ConfigurationRequest::SaveProfile {
            command_id,
            expected_revision,
            definition,
        } => (
            command_id,
            *expected_revision,
            ConfigurationCommand::SaveProfile {
                definition: definition.clone(),
            },
        ),
        ConfigurationRequest::Restore {
            command_id,
            expected_revision,
            kind,
            id,
        } => (
            command_id,
            *expected_revision,
            ConfigurationCommand::Restore {
                kind: *kind,
                id: id.clone(),
            },
        ),
        ConfigurationRequest::Archive {
            command_id,
            expected_revision,
            kind,
            id,
        } => (
            command_id,
            *expected_revision,
            ConfigurationCommand::Archive {
                kind: *kind,
                id: id.clone(),
            },
        ),
        ConfigurationRequest::SaveScope {
            command_id,
            expected_revision,
            scope,
        } => (
            command_id,
            *expected_revision,
            ConfigurationCommand::SaveScope {
                scope: scope.clone(),
            },
        ),
    };
    let receipt = store
        .execute(&Request {
            scope: LocalRef::new("local-configuration").expect("constant"),
            key: LocalRef::new(id).map_err(|_| "configuration-command-id")?,
            expected_revision: revision,
            command: Command::Configuration(Box::new(command)),
        })
        .map_err(|e| match e {
            StoreError::StaleRevision => "stale-revision-resync",
            StoreError::IdempotencyConflict => "command-id-conflict",
            StoreError::Configuration => "configuration-invalid-definition",
            StoreError::InconsistentCommand => "configuration-inconsistent-command",
            StoreError::InvalidState => "configuration-invalid-state",
            StoreError::Records(_) => "configuration-records-rejected",
            StoreError::Privacy(_) => "configuration-privacy-rejected",
            _ => "configuration-command",
        })?;
    if receipt.result.disposition != Disposition::Applied {
        return Err(receipt
            .result
            .error_code
            .unwrap_or_else(|| "configuration-rejected".into()));
    }
    Ok(json!({"replayed":receipt.replayed,"result":receipt.result}))
}

fn overlay_preferences(target: &mut Preferences, patch: &Preferences) {
    macro_rules! fields {($($field:ident),*)=>{$(if patch.$field.is_some(){target.$field=patch.$field.clone();})*};}
    fields!(
        strategy,
        preset,
        connection_id,
        model_id,
        reasoning,
        preferred_connections,
        preferred_models,
        fallback,
        billing,
        billing_authorization_ref,
        sandbox_write,
        network,
        approval_ask,
        concurrency,
        context_bytes,
        service_tier
    );
}
fn merge_requirements(target: &mut Requirements, from: &Requirements) -> Result<()> {
    target.subscription_only |= from.subscription_only;
    target.forbid_network |= from.forbid_network;
    target.read_only |= from.read_only;
    target.require_review |= from.require_review;
    target
        .required_capabilities
        .extend(from.required_capabilities.clone());
    target.required_checks.extend(from.required_checks.clone());
    target.minimum_reasoning = target.minimum_reasoning.max(from.minimum_reasoning);
    target.minimum_strength = target.minimum_strength.max(from.minimum_strength);
    target.max_concurrency = match (target.max_concurrency, from.max_concurrency) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    target.max_context_bytes = match (target.max_context_bytes, from.max_context_bytes) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    for (a, b) in [
        (&mut target.forced_model, &from.forced_model),
        (&mut target.forced_connection, &from.forced_connection),
    ] {
        if let Some(b) = b {
            if a.as_ref().is_some_and(|a| a != b) {
                return Err("configuration-mandatory-conflict".into());
            }
            *a = Some(b.clone());
        }
    }
    Ok(())
}
fn apply_scope(document: &mut PolicyDocument, scope: &ScopedPolicy) -> Result<()> {
    if scope.removed && scope.requirements == Requirements::default() {
        return Ok(());
    }
    if let Some(layer) = document.layers.iter_mut().find(|l| l.scope == scope.scope) {
        overlay_preferences(&mut layer.preferences, &scope.preferences);
        merge_requirements(&mut layer.requirements, &scope.requirements)?;
        layer.reference = format!("local-{:?}-policy", scope.scope).to_ascii_lowercase();
        layer.revision = scope.version.max(1);
    } else {
        document.layers.push(PolicyLayer {
            scope: scope.scope,
            reference: match scope.scope {
                Scope::User => "local-user-policy",
                Scope::Project => "local-project-policy",
                _ => "local-policy",
            }
            .into(),
            revision: scope.version.max(1),
            preferences: scope.preferences.clone(),
            requirements: scope.requirements.clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ResolvedConfiguration {
    pub policy: PolicyDocument,
    pub mode: Option<ModeDefinition>,
    pub profile: Option<ProfileDefinition>,
    pub review_profile: Option<ProfileDefinition>,
    pub context_sources: Vec<ContextSource>,
    pub definitions: Vec<DefinitionInput>,
    pub fingerprint: String,
}
fn managed(state: &SavedState) -> Vec<&DefinitionInput> {
    state
        .records()
        .definitions()
        .iter()
        .filter(|d| {
            d.kind == DefinitionKind::Configuration
                && (d.id.starts_with(MODE_PREFIX)
                    || d.id.starts_with(PROFILE_PREFIX)
                    || d.id.starts_with(SCOPE_PREFIX))
        })
        .collect()
}
pub fn execution_fingerprint(state: &SavedState, settings: &Settings) -> Result<String> {
    let base = settings.execution_fingerprint()?;
    let records = managed(state);
    if records.is_empty() {
        return Ok(base);
    }
    Ok(policy::hash(
        serde_json::to_string(&json!({"settings":base,"definitions":records}))
            .map_err(|_| "configuration-schema")?
            .as_bytes(),
    ))
}
/// Portability references are explicit bindings, never matches on display name.
/// A historical exact version remains inspectable but an archived identity is
/// not eligible for new selections.
pub fn validate_selection(
    state: &SavedState,
    mode: &DefinitionBinding,
    profile: &DefinitionBinding,
) -> Result<()> {
    mode.validate()
        .map_err(|_| "configuration-mode-reference")?;
    profile
        .validate()
        .map_err(|_| "configuration-profile-reference")?;
    if latest_mode(state, &mode.id)?.is_some_and(|m| m.archived) {
        return Err("configuration-mode-archived".into());
    }
    if latest_profile(state, &profile.id)?.is_some_and(|p| p.archived) {
        return Err("configuration-profile-archived".into());
    }
    if !legacy_binding(state, DefinitionType::Mode, mode) {
        let m = mode_definition(state, mode)?;
        if m.archived {
            return Err("configuration-mode-archived".into());
        }
    }
    if !legacy_binding(state, DefinitionType::Profile, profile) {
        let p = profile_definition(state, profile)?;
        if p.archived {
            return Err("configuration-profile-archived".into());
        }
    }
    Ok(())
}
/// Portable v1 can represent the User layer. Identified project overrides and
/// personal definition contents remain local; no paths or rich definitions are
/// added to the portable document.
pub fn export_settings(state: &SavedState) -> Result<Settings> {
    let mut settings = crate::policy_portability::settings_from_state(state)?;
    let scopes: Vec<ScopedPolicy> = catalog(state)?.scopes;
    for scope in scopes.iter().filter(|s| s.scope == Scope::User) {
        apply_scope(&mut settings.policy, scope)?;
    }
    settings
        .policy
        .validate()
        .map_err(|_| "configuration-portable-user-layer")?;
    Ok(settings)
}
/// Called inside the existing atomic import command. Preserve one preference
/// authority when this installation already has a canonical User scope record.
/// This does not install a mode/profile, contact a provider or import consent.
pub fn apply_imported_user_scope(
    state: &mut SavedState,
    settings: &mut Settings,
) -> std::result::Result<(), StoreError> {
    let key = format!("{SCOPE_PREFIX}user");
    let Some(mut scope): Option<ScopedPolicy> =
        stored(state, &key, None).map_err(|_| StoreError::Configuration)?
    else {
        return Ok(());
    };
    let layer = settings
        .policy
        .layers
        .iter()
        .find(|l| l.scope == Scope::User)
        .cloned();
    if let Some(layer) = layer {
        let mut requirements = scope.requirements.clone();
        merge_requirements(&mut requirements, &layer.requirements)
            .map_err(|_| StoreError::Configuration)?;
        scope.preferences = layer.preferences;
        scope.requirements = requirements;
        scope.removed = false;
    } else {
        // The import preview replaces preference layers. Retaining a hidden
        // local User preference here would make the actual result differ from
        // that preview. Only mandatory restrictions survive restoring inheritance.
        scope.preferences = Preferences::default();
        scope.removed = true;
    }
    ConfigurationCommand::SaveScope { scope }.apply(state)?;
    settings.policy.layers.retain(|l| l.scope != Scope::User);
    Ok(())
}
pub fn guard_context_pin(state: &SavedState, context: &ContextInput) -> Result<()> {
    let pins: Vec<_> = context
        .sources
        .iter()
        .filter(|s| s.identity.reference == "configuration-pin")
        .collect();
    if pins.is_empty() && legacy_unconfigured(state) {
        return Ok(());
    }
    if pins.len() != 1 {
        return Err("configuration-preparation-pin-missing".into());
    }
    let settings = crate::policy_portability::settings_from_state(state)?;
    if pins[0].identity.revision != execution_fingerprint(state, &settings)? {
        return Err("configuration-preparation-stale".into());
    }
    Ok(())
}
/// Admission-only check for a compiled Context Pack. Historical validation must
/// not reinterpret an admitted attempt using today's definitions.
pub fn guard_context_snapshot(state: &SavedState, context: &Value) -> Result<()> {
    let sources = context["sources"]
        .as_array()
        .ok_or("configuration-context-sources")?;
    let pins: Vec<_> = sources
        .iter()
        .filter(|s| s["identity"]["reference"] == "configuration-pin")
        .collect();
    if pins.is_empty() && legacy_unconfigured(state) {
        return Ok(());
    }
    if pins.len() != 1 {
        return Err("configuration-preparation-pin-missing".into());
    }
    let settings = crate::policy_portability::settings_from_state(state)?;
    if pins[0]["identity"]["revision"] != execution_fingerprint(state, &settings)? {
        return Err("configuration-preparation-stale".into());
    }
    Ok(())
}
fn legacy_unconfigured(state: &SavedState) -> bool {
    managed(state).is_empty()
        && !state
            .records()
            .definitions()
            .iter()
            .any(|d| d.kind == DefinitionKind::Configuration && d.id == "desktop-settings")
}
fn source(reference: String, version: u64, text: String) -> ContextSource {
    ContextSource {
        identity: ContextSourceIdentity {
            reference,
            revision: format!("definition-v{version}"),
        },
        kind: ContextSourceKind::UserInstructions,
        authorization_ref: Some("local-configuration-selection".into()),
        text,
        required: true,
        priority: 1,
    }
}
fn legacy_binding(_state: &SavedState, kind: DefinitionType, binding: &DefinitionBinding) -> bool {
    binding.version == 1
        && match kind {
            // Only Development had the validated generic manual-worker v1
            // contract. Other catalog names must resolve a managed definition;
            // accepting their invented v1 aliases would skip artifact support.
            DefinitionType::Mode => binding.id == "development",
            DefinitionType::Profile => matches!(
                binding.id.as_str(),
                "implementation-engineer" | "independent-reviewer"
            ),
        }
}
fn canonical_policy(state: &SavedState, input: &PolicyDocument) -> Result<PolicyDocument> {
    // Existing CLI callers supply a complete policy. Once Desktop preferences
    // exist, a renderer cannot replace their mandatory restrictions. Task and
    // execution preferences are still resolved at their documented scopes.
    let settings = crate::policy_portability::settings_from_state(state)?;
    let mut result = input.clone();
    merge_requirements(
        &mut result.mode_requirements,
        &settings.policy.mode_requirements,
    )?;
    for layer in &settings.policy.layers {
        merge_requirements(&mut result.mode_requirements, &layer.requirements)?;
    }
    result.mode_requirements.subscription_only = true;
    Ok(result)
}
pub fn resolve_for_task(
    state: &SavedState,
    input: &PolicyDocument,
    project_id: Option<&str>,
) -> Result<ResolvedConfiguration> {
    // The legacy reviewer lacks a rich profile record. Its absence must not
    // turn that read-only review identity into an implementation executor.
    if input.profile.id == "independent-reviewer" && input.profile.version == 1 {
        return Err("configuration-implementation-profile-role".into());
    }
    compose(state, input, project_id, None, None, None)
}
pub fn resolve_for_review(
    state: &SavedState,
    input: &PolicyDocument,
    project_id: Option<&str>,
) -> Result<ResolvedConfiguration> {
    let mut review = input.clone();
    if legacy_binding(state, DefinitionType::Mode, &input.mode) {
        // Explicit legacy-v1 compatibility, without attributing rich definitions
        // to historical generic worker records. New managed modes use roles.
        review.profile = DefinitionBinding {
            id: "independent-reviewer".into(),
            version: 1,
        };
    } else {
        let mode = mode_definition(state, &input.mode)?;
        review.profile = mode.review_profile()?;
    }
    // The implementer's profile defaults cannot overwrite the independent role.
    review.profile_defaults = Preferences::default();
    review.layers.retain(|l| l.scope != Scope::Profile);
    let mut result = compose(state, &review, project_id, None, None, None)?;
    if result
        .profile
        .as_ref()
        .is_some_and(|p| p.role != WorkerRole::Review)
    {
        return Err("configuration-review-profile-role".into());
    }
    result.policy.mode_requirements.read_only = true;
    result.policy.profile_defaults.sandbox_write = Some(false);
    Ok(result)
}
fn compose(
    state: &SavedState,
    input: &PolicyDocument,
    project_id: Option<&str>,
    mode_draft: Option<&ModeDefinition>,
    profile_draft: Option<&ProfileDefinition>,
    scope_draft: Option<&ScopedPolicy>,
) -> Result<ResolvedConfiguration> {
    input.validate().map_err(|_| "configuration-policy")?;
    if mode_draft.is_none() && profile_draft.is_none() {
        validate_selection(state, &input.mode, &input.profile)?;
    }
    // A manual CLI task can introduce its project at reservation. Saving an
    // override still requires a previously identified persisted project.
    if let Some(id) = project_id {
        ProjectId::new(id).map_err(|_| "configuration-project")?;
    }
    let mut policy = canonical_policy(state, input)?;
    let mode = if let Some(d) = mode_draft {
        validate_mode(state, d, false)?;
        Some(d.clone())
    } else if legacy_binding(state, DefinitionType::Mode, &input.mode) {
        None
    } else {
        Some(mode_definition(state, &input.mode)?)
    };
    let profile = if let Some(d) = profile_draft {
        validate_profile(state, d, false)?;
        Some(d.clone())
    } else if legacy_binding(state, DefinitionType::Profile, &input.profile) {
        None
    } else {
        Some(profile_definition(state, &input.profile)?)
    };
    let settings = crate::policy_portability::settings_from_state(state)?;
    let fingerprint = execution_fingerprint(state, &settings)?;
    let mut definitions = vec![];
    let mut sources = vec![];
    let mut review_profile = None;
    if let Some(m) = &mode {
        if m.archived || latest_mode(state, &m.id)?.is_some_and(|d| d.archived) {
            return Err("configuration-mode-archived".into());
        }
        validate_mode(state, m, false)?;
        policy.mode = m.binding();
        let mut defaults = m.preferences.clone();
        overlay_preferences(&mut defaults, &policy.mode_defaults);
        policy.mode_defaults = defaults;
        merge_requirements(&mut policy.mode_requirements, &m.requirements)?;
        definitions.push(minimal_mode(m));
        definitions.push(rich_mode(m));
        sources.push(source(format!("mode-{}",m.id),m.version,json!({"name":m.name,"purpose":m.purpose,"instructions":m.instructions,"outputs":m.outputs,"criteria":m.criteria,"roles":m.roles,"executionSupport":m.execution_support,"authority":"instructions-do-not-grant-tools-or-execution"}).to_string()));
        let r = profile_definition(state, &m.review_profile()?)?;
        if r.role != WorkerRole::Review
            || r.archived
            || latest_profile(state, &r.id)?.is_some_and(|p| p.archived)
        {
            return Err("configuration-review-profile-role-or-archived".into());
        }
        review_profile = Some(r);
    }
    if let Some(p) = &profile {
        if p.archived || latest_profile(state, &p.id)?.is_some_and(|d| d.archived) {
            return Err("configuration-profile-archived".into());
        }
        validate_profile(state, p, false)?;
        policy.profile = p.binding();
        let mut defaults = p.preferences.clone();
        overlay_preferences(&mut defaults, &policy.profile_defaults);
        policy.profile_defaults = defaults;
        if p.role == WorkerRole::Review {
            policy.profile_defaults.sandbox_write = Some(false);
        }
        let layer = PolicyLayer {
            scope: Scope::Profile,
            reference: format!("profile-{}", p.id),
            revision: p.version.max(1),
            preferences: Preferences::default(),
            requirements: p.requirements.clone(),
        };
        if let Some(old) = policy.layers.iter_mut().find(|l| l.scope == Scope::Profile) {
            merge_requirements(&mut old.requirements, &layer.requirements)?;
        } else {
            policy.layers.push(layer);
        }
        definitions.push(minimal_profile(p));
        definitions.push(rich_profile(p));
        sources.push(source(format!("profile-{}",p.id),p.version,json!({"name":p.name,"role":p.role,"instructions":p.instructions,"outputLanguage":p.output_language,"conventions":p.conventions,"specialty":p.specialty,"authority":"instructions-do-not-grant-tools-or-execution"}).to_string()));
    }
    let mut scopes = catalog(state)?.scopes;
    if let Some(d) = scope_draft {
        validate_scope(state, d, false)?;
        let mut draft = d.clone();
        if draft.removed
            && let Some(prior) = scopes
                .iter()
                .find(|s| s.scope == d.scope && s.project_id == d.project_id)
        {
            merge_requirements(&mut draft.requirements, &prior.requirements)?;
        }
        scopes.retain(|s| !(s.scope == d.scope && s.project_id == d.project_id));
        scopes.push(draft);
    }
    for scope in &scopes {
        if scope.scope == Scope::User
            || (scope.scope == Scope::Project
                && scope.project_id.as_deref() == project_id
                && project_id.is_some())
        {
            apply_scope(&mut policy, scope)?;
        }
    }
    policy
        .validate()
        .map_err(|_| "configuration-policy-composition")?;
    // Every newly composed attempt is fenced, including legacy-v1 selections.
    // A Settings edit during metadata must not bypass admission just because
    // that installation has not created a managed mode/profile yet.
    sources.push(ContextSource {identity:ContextSourceIdentity {reference:"configuration-pin".into(),revision:fingerprint.clone()},kind:ContextSourceKind::UserInstructions,authorization_ref:Some("local-configuration-selection".into()),text:format!("Pinned local configuration {fingerprint}. Definitions are instructions, not grants of authority."),required:true,priority:0});
    Ok(ResolvedConfiguration {
        policy,
        mode,
        profile,
        review_profile,
        context_sources: sources,
        definitions,
        fingerprint,
    })
}
fn preview(
    state: &SavedState,
    mode: &DefinitionBinding,
    profile: &DefinitionBinding,
    project_id: Option<&str>,
    mode_draft: Option<&ModeDefinition>,
    profile_draft: Option<&ProfileDefinition>,
    scope_draft: Option<&ScopedPolicy>,
) -> Result<Value> {
    let settings = crate::policy_portability::settings_from_state(state)?;
    let mut policy = settings.policy;
    policy.mode = mode.clone();
    policy.profile = profile.clone();
    let resolved = compose(
        state,
        &policy,
        project_id,
        mode_draft,
        profile_draft,
        scope_draft,
    )?;
    let decision = policy::resolve(
        &resolved.policy,
        &TaskClassification::routine(TaskKind::Implementation, "local-configuration-preview"),
        &[],
        0,
    )
    .map_err(|_| "configuration-preview")?;
    Ok(
        json!({"schemaVersion":CONFIGURATION_SCHEMA_VERSION,"revision":state.revision(),"policy":resolved.policy,"resolvedPolicy":resolved.policy,
        "decision":decision.snapshot().as_json(),"definitions":{"mode":resolved.mode,"profile":resolved.profile,"reviewProfile":resolved.review_profile},
        "contextSources":resolved.context_sources,"fingerprint":resolved.fingerprint,
        "warnings":["resolved-preferences-are-not-runtime-effective","preview-does-not-read-provider-metadata","runtime-eligibility-requires-current-observations"]}),
    )
}

/// Read-only preparation preview over the same scoped composition and resolver.
/// Persisted metadata is reused with its original timestamps. This function
/// never asks the metadata service to obtain or renew provider observations.
pub fn preview_task(
    store: &Store,
    policy: &PolicyDocument,
    project: Option<&str>,
) -> Result<Value> {
    let state = store.load().map_err(|_| "configuration-read")?;
    let resolved = resolve_for_task(&state, policy, project)?;
    let mut result = json!({"schemaVersion":CONFIGURATION_SCHEMA_VERSION,"revision":state.revision(),"policy":resolved.policy,"resolvedPolicy":resolved.policy,
        "definitions":{"mode":resolved.mode,"profile":resolved.profile,"reviewProfile":resolved.review_profile},"contextSources":resolved.context_sources,"fingerprint":resolved.fingerprint,
        "warnings":["resolved-preferences-are-not-runtime-effective","preview-does-not-read-provider-metadata","runtime-eligibility-requires-current-observations"]});
    apply_stored_observations(store, &resolved.policy, &mut result)?;
    Ok(result)
}

fn apply_stored_observations(
    store: &Store,
    policy: &PolicyDocument,
    result: &mut Value,
) -> Result<()> {
    let now = crate::connections::now_ms();
    let list = store
        .connection_list()
        .map_err(|_| "configuration-observations")?;
    let mut runtimes = vec![];
    let mut reports = vec![];
    for connection in list["connections"]
        .as_array()
        .ok_or("configuration-observations")?
        .iter()
        .take(32)
    {
        let id = connection["id"]
            .as_str()
            .ok_or("configuration-observations")?;
        let report = store
            .connection_diagnostics(id)
            .map_err(|_| "configuration-observations")?;
        reports.push(json!({"connectionId":id,"enabled":connection["enabled"],"linked":connection["linked"],
            "bindingRevision":connection["bindingRevision"],"lastSuccessful":report["lastSuccessful"],"identityChanged":report["identityChanged"]}));
        if connection["enabled"] != true
            || connection["linked"] != true
            || report["catalog"].is_null()
        {
            continue;
        }
        let descriptor = crate::runtime_contract::normalize_descriptor(&report["catalog"])
            .map_err(|_| "configuration-observations")?;
        let d = descriptor.as_json();
        let observed = &report["lastSuccessful"];
        let mut leases = vec![];
        if observed["current"] == true
            && report["identityChanged"] != true
            && connection["bindingRevision"] == d["connection"]["revision"]
        {
            let at = observed["observedAtMs"]
                .as_u64()
                .ok_or("configuration-observations")?;
            let until = observed["effectiveValidUntilMs"]
                .as_u64()
                .ok_or("configuration-observations")?;
            let evidence = report["catalogDiagnosticId"]
                .as_str()
                .ok_or("configuration-observations")?;
            let version = d["runtimeVersion"]
                .as_str()
                .ok_or("configuration-observations")?;
            let revision = d["connection"]["revision"]
                .as_u64()
                .ok_or("configuration-observations")?;
            let mut add = |fact: String, model: Option<String>| {
                leases.push(policy::ObservationLease {
                    fact,
                    model_id: model,
                    evidence_ref: evidence.into(),
                    connection_id: id.into(),
                    connection_revision: revision,
                    runtime_version: version.into(),
                    observed_at_ms: at,
                    valid_until_ms: until,
                });
            };
            for fact in ["identity", "billing", "availability", "catalog"] {
                add(fact.into(), None);
            }
            if let Some(controls) = report["controlObservations"].as_array() {
                for control in controls.iter().filter_map(Value::as_str) {
                    add(control.into(), None);
                }
            }
            for model in d["models"].as_array().ok_or("configuration-observations")? {
                let model_id = model["id"].as_str().ok_or("configuration-observations")?;
                add("model-availability".into(), Some(model_id.into()));
                for capability in crate::runtime_contract::CAPABILITIES {
                    add(format!("capability:{capability}"), Some(model_id.into()));
                }
            }
        }
        runtimes.push(
            policy::ObservedRuntime::new(descriptor, leases)
                .map_err(|_| "configuration-observations")?,
        );
    }
    let decision = policy::resolve(
        policy,
        &TaskClassification::routine(TaskKind::Implementation, "local-configuration-preview"),
        &runtimes,
        now,
    )
    .map_err(|_| "configuration-preview")?;
    result["decision"] = decision.snapshot().as_json().clone();
    result["cachedObservations"] = json!(reports);
    result["previewedAtMs"] = json!(now);
    result["metadataReadsStarted"] = json!(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn store() -> Store {
        let dir = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchestrix-configuration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        Store::open(dir.join("state.sqlite")).unwrap()
    }
    fn save_mode(store: &mut Store, id: &str, d: ModeDefinition) -> Value {
        handle(
            store,
            &ConfigurationRequest::SaveMode {
                command_id: id.into(),
                expected_revision: store.load().unwrap().revision(),
                definition: d,
            },
        )
        .unwrap()
    }
    fn local_mode(id: &str) -> ModeDefinition {
        let mut d = native_modes().remove(0);
        let base = d.binding();
        d.id = id.into();
        d.version = 0;
        d.origin = DefinitionOrigin::Local;
        d.base = Some(base);
        d.name = "Local development".into();
        d.purpose = "Project-specific authorized work".into();
        d
    }
    fn local_profile(id: &str, review: bool) -> ProfileDefinition {
        let mut d = native_profiles().remove(usize::from(review));
        let base = d.binding();
        d.id = id.into();
        d.version = 0;
        d.origin = DefinitionOrigin::Local;
        d.base = Some(base);
        d.name = "Local executor".into();
        d
    }
    fn policy() -> PolicyDocument {
        PolicyDocument {
            mode: DefinitionBinding {
                id: "development".into(),
                version: 2,
            },
            profile: DefinitionBinding {
                id: "implementation-engineer".into(),
                version: 2,
            },
            ..PolicyDocument::default()
        }
    }
    fn project(store: &mut Store, id: &str) {
        let request = crate::desktop::DesktopRequest::CreateProject {
            command_id: format!("create-{id}"),
            expected_revision: store.load().unwrap().revision(),
            id: id.into(),
            name: id.into(),
            directory: store.path().parent().unwrap().to_string_lossy().into(),
        };
        crate::desktop::handle(store, &request).unwrap();
    }
    fn scoped(scope: Scope, project: Option<&str>) -> ScopedPolicy {
        ScopedPolicy {
            scope,
            project_id: project.map(str::to_owned),
            version: 0,
            edit_revision: 0,
            removed: false,
            preferences: Preferences::default(),
            requirements: Requirements::default(),
        }
    }
    fn save_scope(store: &mut Store, id: &str, scope: ScopedPolicy) {
        handle(
            store,
            &ConfigurationRequest::SaveScope {
                command_id: id.into(),
                expected_revision: store.load().unwrap().revision(),
                scope,
            },
        )
        .unwrap();
    }
    #[test]
    fn catalog_is_pure_and_preserves_the_six_contract_names() {
        let store = store();
        let s = store.load().unwrap();
        let c = catalog(&s).unwrap();
        let names: BTreeSet<_> = c.modes.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(
            names,
            BTreeSet::from([
                "Development",
                "Research",
                "Documentation",
                "Presentations",
                "Refactoring",
                "Improvement Review"
            ])
        );
        assert_eq!(s.revision(), 0);
        assert!(s.records().definitions().is_empty());
        assert_eq!(
            c.modes
                .iter()
                .filter(|m| m.execution_support == ExecutionSupport::ArtifactFlowUnproven)
                .count(),
            3
        );
    }
    #[test]
    fn native_customization_restoration_and_legacy_are_not_rewritten() {
        let mut store = store();
        let mut d = native_modes().remove(0);
        d.instructions = "Prefer small, documented changes.".into();
        save_mode(&mut store, "native-edit", d);
        let s = store.load().unwrap();
        let v3 = mode_definition(
            &s,
            &DefinitionBinding {
                id: "development".into(),
                version: 3,
            },
        )
        .unwrap();
        assert_eq!(v3.name, "Development");
        assert_eq!(v3.version, 3);
        assert_eq!(v3.edit_revision, 1);
        assert_ne!(
            mode_definition(
                &s,
                &DefinitionBinding {
                    id: "development".into(),
                    version: 2
                }
            )
            .unwrap()
            .instructions,
            v3.instructions
        );
        let revision = s.revision();
        handle(
            &mut store,
            &ConfigurationRequest::Restore {
                command_id: "restore".into(),
                expected_revision: revision,
                kind: DefinitionType::Mode,
                id: "development".into(),
            },
        )
        .unwrap();
        let s = store.load().unwrap();
        assert_eq!(latest_mode(&s, "development").unwrap().unwrap().version, 4);
        assert_eq!(mode_definition(&s, &v3.binding()).unwrap(), v3);
        assert!(s.records().definitions().iter().all(|d| d.version != 1));
    }
    #[test]
    fn native_purpose_requirements_and_origin_cannot_be_replaced() {
        let s = SavedState::default();
        let d = native_modes().remove(0);
        for mut changed in [d.clone(), d.clone(), d.clone()] {
            if changed == d {
                changed.purpose = "Replace native contract".into();
            }
            assert!(validate_mode(&s, &changed, true).is_err());
        }
        let mut changed = d.clone();
        changed.requirements.require_review = false;
        assert!(validate_mode(&s, &changed, true).is_err());
        changed = d;
        changed.origin = DefinitionOrigin::Local;
        assert!(validate_mode(&s, &changed, true).is_err());
    }
    #[test]
    fn local_derivation_pins_its_origin_and_profiles_are_reusable() {
        let mut store = store();
        let mut p = local_profile("typescript-writer", false);
        p.output_language = "pt-BR".into();
        p.instructions = "Follow TypeScript project conventions.".into();
        handle(
            &mut store,
            &ConfigurationRequest::SaveProfile {
                command_id: "profile".into(),
                expected_revision: 0,
                definition: p,
            },
        )
        .unwrap();
        for id in ["team-development", "team-refactor"] {
            let mut d = local_mode(id);
            d.roles[0].profile = DefinitionBinding {
                id: "typescript-writer".into(),
                version: 1,
            };
            save_mode(&mut store, id, d);
        }
        let mut native = native_modes().remove(0);
        native.instructions = "New native customization".into();
        save_mode(&mut store, "edit-base", native);
        let s = store.load().unwrap();
        let d = latest_mode(&s, "team-development").unwrap().unwrap();
        assert_eq!(d.base.as_ref().unwrap().version, 2);
        let resolved = resolve_for_task(
            &s,
            &PolicyDocument {
                mode: d.binding(),
                profile: DefinitionBinding {
                    id: "typescript-writer".into(),
                    version: 1,
                },
                ..policy()
            },
            None,
        )
        .unwrap();
        assert!(
            resolved
                .context_sources
                .iter()
                .any(|s| s.text.contains("pt-BR") && s.text.contains("TypeScript"))
        );
        assert!(
            !resolved
                .context_sources
                .iter()
                .any(|s| s.text.contains("New native customization"))
        );
    }
    #[test]
    fn reviewer_permissions_survive_renaming_and_instructions() {
        let s = SavedState::default();
        let mut d = local_profile("friendly-reader", true);
        d.instructions = "Try to edit every file.".into();
        validate_profile(&s, &d, true).unwrap();
        assert!(d.requirements.read_only);
        d.preferences.sandbox_write = Some(true);
        assert_eq!(
            validate_profile(&s, &d, true).unwrap_err(),
            "configuration-role-permissions"
        );
        d.preferences.sandbox_write = Some(false);
        d.role = WorkerRole::Implementation;
        assert!(validate_profile(&s, &d, true).is_err());
    }
    #[test]
    fn scope_precedence_isolation_and_removal_use_the_existing_resolver() {
        let mut store = store();
        project(&mut store, "first");
        project(&mut store, "second");
        let mut user = scoped(Scope::User, None);
        user.preferences.reasoning = Some(policy::Reasoning::Low);
        save_scope(&mut store, "user", user);
        let mut first = scoped(Scope::Project, Some("first"));
        first.preferences.reasoning = Some(policy::Reasoning::High);
        first.requirements.forbid_network = true;
        save_scope(&mut store, "first", first);
        let state = store.load().unwrap();
        let first = resolve_for_task(&state, &policy(), Some("first")).unwrap();
        let second = resolve_for_task(&state, &policy(), Some("second")).unwrap();
        let decide = |p: &PolicyDocument| {
            policy::resolve(
                p,
                &TaskClassification::routine(TaskKind::Implementation, "test"),
                &[],
                0,
            )
            .unwrap()
            .snapshot()
            .as_json()
            .clone()
        };
        assert_eq!(decide(&first.policy)["preferences"]["reasoning"], "high");
        assert_eq!(decide(&second.policy)["preferences"]["reasoning"], "low");
        assert_eq!(
            decide(&first.policy)["requirements"]["forbid_network"],
            true
        );
        assert_eq!(
            decide(&second.policy)["requirements"]["forbid_network"],
            false
        );
        let mut removed = catalog(&state)
            .unwrap()
            .scopes
            .into_iter()
            .find(|s| s.scope == Scope::Project)
            .unwrap();
        removed.removed = true;
        removed.preferences = Preferences::default();
        removed.requirements = Requirements::default();
        save_scope(&mut store, "remove", removed);
        let state = store.load().unwrap();
        let resolved = resolve_for_task(&state, &policy(), Some("first")).unwrap();
        assert_eq!(decide(&resolved.policy)["preferences"]["reasoning"], "low");
        assert_eq!(
            decide(&resolved.policy)["requirements"]["forbid_network"],
            true
        );
    }
    #[test]
    fn profile_and_task_scopes_override_project_preferences_not_mandatory_limits() {
        let mut store = store();
        project(&mut store, "first");
        let mut scope = scoped(Scope::Project, Some("first"));
        scope.preferences.reasoning = Some(policy::Reasoning::Low);
        scope.requirements.forbid_network = true;
        save_scope(&mut store, "scope", scope);
        let mut p = native_profiles().remove(0);
        p.preferences.reasoning = Some(policy::Reasoning::High);
        let revision = store.load().unwrap().revision();
        handle(
            &mut store,
            &ConfigurationRequest::SaveProfile {
                command_id: "profile".into(),
                expected_revision: revision,
                definition: p,
            },
        )
        .unwrap();
        let mut input = policy();
        input.profile.version = 3;
        input.layers.push(PolicyLayer {
            scope: Scope::Task,
            reference: "manual-task".into(),
            revision: 1,
            preferences: Preferences {
                reasoning: Some(policy::Reasoning::Medium),
                network: Some(true),
                ..Preferences::default()
            },
            requirements: Requirements::default(),
        });
        let resolved = resolve_for_task(&store.load().unwrap(), &input, Some("first")).unwrap();
        let d = policy::resolve(
            &resolved.policy,
            &TaskClassification::routine(TaskKind::Implementation, "test"),
            &[],
            0,
        )
        .unwrap();
        assert_eq!(d.snapshot().as_json()["preferences"]["reasoning"], "medium");
        assert!(
            d.snapshot().as_json()["route"]
                .to_string()
                .contains("mandatory-network-disabled")
        );
    }
    #[test]
    fn sqlite_cas_idempotency_reopen_and_archiving_preserve_history() {
        let mut store = store();
        let path = store.path().to_owned();
        let request = ConfigurationRequest::SaveMode {
            command_id: "same-command".into(),
            expected_revision: 0,
            definition: local_mode("private-mode"),
        };
        handle(&mut store, &request).unwrap();
        let count = store.load().unwrap().records().definitions().len();
        assert_eq!(handle(&mut store, &request).unwrap()["replayed"], true);
        assert_eq!(store.load().unwrap().records().definitions().len(), count);
        assert!(
            handle(
                &mut store,
                &ConfigurationRequest::SaveMode {
                    command_id: "other-window".into(),
                    expected_revision: 0,
                    definition: local_mode("other-mode")
                }
            )
            .is_err()
        );
        let revision = store.load().unwrap().revision();
        let mut detached = store.load().unwrap();
        ConfigurationCommand::Archive {
            kind: DefinitionType::Mode,
            id: "private-mode".into(),
        }
        .apply(&mut detached)
        .expect("archive must compose valid immutable records");
        crate::persistence_records::DocumentRecords::restore(&detached.records().checkpoint())
            .expect("archived records must survive restoration");
        handle(
            &mut store,
            &ConfigurationRequest::Archive {
                command_id: "archive".into(),
                expected_revision: revision,
                kind: DefinitionType::Mode,
                id: "private-mode".into(),
            },
        )
        .unwrap();
        drop(store);
        let reopened = Store::open(path).unwrap();
        let s = reopened.load().unwrap();
        assert!(
            !mode_definition(
                &s,
                &DefinitionBinding {
                    id: "private-mode".into(),
                    version: 1
                }
            )
            .unwrap()
            .archived
        );
        assert!(
            resolve_for_task(
                &s,
                &PolicyDocument {
                    mode: DefinitionBinding {
                        id: "private-mode".into(),
                        version: 1
                    },
                    ..policy()
                },
                None
            )
            .is_err()
        );
    }
    #[test]
    fn committed_reply_loss_does_not_create_another_definition_version() {
        let mut store = store();
        let definition = local_mode("reply-loss-mode");
        let request = Request {
            scope: LocalRef::new("local-configuration").unwrap(),
            key: LocalRef::new("reply-loss").unwrap(),
            expected_revision: 0,
            command: Command::Configuration(Box::new(ConfigurationCommand::SaveMode {
                definition: definition.clone(),
            })),
        };
        assert_eq!(
            store
                .execute_with_fault(&request, crate::persistence::FaultPoint::AfterCommit)
                .unwrap_err(),
            StoreError::ReplyLost
        );
        let result = handle(
            &mut store,
            &ConfigurationRequest::SaveMode {
                command_id: "reply-loss".into(),
                expected_revision: 0,
                definition,
            },
        )
        .unwrap();
        assert_eq!(result["replayed"], true);
        let state = store.load().unwrap();
        assert_eq!(
            latest_mode(&state, "reply-loss-mode")
                .unwrap()
                .unwrap()
                .version,
            1
        );
        assert_eq!(state.revision(), 1);
    }
    #[test]
    fn portable_user_scope_preserves_one_authority_and_excludes_identified_projects() {
        let mut store = store();
        project(&mut store, "private-project");
        let mut user = scoped(Scope::User, None);
        user.preferences.strategy = Some(policy::Strategy::Efficiency);
        user.requirements.forbid_network = true;
        save_scope(&mut store, "global-scope", user);
        let mut project = scoped(Scope::Project, Some("private-project"));
        project.preferences.model_id = Some("private-project-model".into());
        save_scope(&mut store, "project-scope", project);
        let mut state = store.load().unwrap();
        let mut exported = export_settings(&state).unwrap();
        assert!(
            exported
                .policy
                .layers
                .iter()
                .any(|l| l.scope == Scope::User)
        );
        assert!(
            !exported
                .policy
                .layers
                .iter()
                .any(|l| l.scope == Scope::Project)
        );
        let encoded =
            serde_json::to_string(&crate::policy_portability::export(&exported).unwrap()).unwrap();
        assert!(!encoded.contains("private-project"));
        assert!(!encoded.contains("instructions"));
        exported
            .policy
            .layers
            .iter_mut()
            .find(|l| l.scope == Scope::User)
            .unwrap()
            .preferences
            .strategy = Some(policy::Strategy::Performance);
        apply_imported_user_scope(&mut state, &mut exported).unwrap();
        assert!(
            !exported
                .policy
                .layers
                .iter()
                .any(|l| l.scope == Scope::User)
        );
        let current = catalog(&state)
            .unwrap()
            .scopes
            .into_iter()
            .find(|s| s.scope == Scope::User)
            .unwrap();
        assert_eq!(
            current.preferences.strategy,
            Some(policy::Strategy::Performance)
        );
        assert!(current.requirements.forbid_network);
    }
    #[test]
    fn import_without_user_layer_restores_inheritance_matching_the_preview() {
        let mut store = store();
        let mut user = scoped(Scope::User, None);
        user.preferences.strategy = Some(policy::Strategy::Efficiency);
        user.requirements.forbid_network = true;
        save_scope(&mut store, "local-user-preferences", user);
        let mut state = store.load().unwrap();
        let current = export_settings(&state).unwrap();
        let mut source = Settings::default();
        source.policy.mode_defaults.strategy = Some(policy::Strategy::Performance);
        let bundle = crate::policy_portability::export(&source).unwrap();
        let mut candidate = bundle
            .resolve(
                &current,
                &crate::policy_portability::ImportMappings::default(),
            )
            .unwrap();
        assert!(
            !candidate
                .policy
                .layers
                .iter()
                .any(|l| l.scope == Scope::User)
        );
        let before = policy::resolve(
            &candidate.policy,
            &TaskClassification::routine(TaskKind::Implementation, "import-preview"),
            &[],
            0,
        )
        .unwrap();
        assert_eq!(
            before.snapshot().as_json()["preferences"]["strategy"],
            "performance"
        );
        apply_imported_user_scope(&mut state, &mut candidate).unwrap();
        let scope = catalog(&state)
            .unwrap()
            .scopes
            .into_iter()
            .find(|s| s.scope == Scope::User)
            .unwrap();
        assert!(scope.removed);
        assert_eq!(scope.preferences, Preferences::default());
        assert!(scope.requirements.forbid_network);
        let resolved = resolve_for_task(&state, &candidate.policy, None).unwrap();
        let after = policy::resolve(
            &resolved.policy,
            &TaskClassification::routine(TaskKind::Implementation, "import-result"),
            &[],
            0,
        )
        .unwrap();
        assert_eq!(
            after.snapshot().as_json()["preferences"]["strategy"],
            before.snapshot().as_json()["preferences"]["strategy"]
        );
        assert_eq!(
            after.snapshot().as_json()["requirements"]["forbid_network"],
            true
        );
    }
    #[test]
    fn malformed_fields_billing_and_unbounded_versions_are_rejected_without_grants() {
        let state = SavedState::default();
        let mut profile = local_profile("restricted-profile", false);
        profile.preferences.billing = Some(policy::BillingMode::ExplicitApi);
        assert!(validate_profile(&state, &profile, true).is_err());
        profile.preferences.billing = None;
        profile.version = u64::MAX;
        assert!(validate_profile(&state, &profile, true).is_err());
        let mut mode = local_mode("oversized-mode");
        mode.instructions = "x".repeat(8193);
        assert!(validate_mode(&state, &mode, true).is_err());
        assert!(
            serde_json::from_value::<ProfileDefinition>(json!({"id":"profile","fullAccess":true}))
                .is_err()
        );
    }
    #[test]
    fn preparation_pin_changes_only_for_execution_configuration_not_reads() {
        let mut store = store();
        let s = store.load().unwrap();
        let resolved = resolve_for_task(&s, &policy(), None).unwrap();
        let mut context = ContextInput {
            schema_version: 1,
            id: "test-context".into(),
            version: 1,
            objective: "test".into(),
            acceptance_criteria: vec!["test".into()],
            constraints: vec![],
            sources: resolved.context_sources,
            files: vec![],
        };
        guard_context_pin(&s, &context).unwrap();
        assert_eq!(
            execution_fingerprint(&s, &Settings::default()).unwrap(),
            Settings::default().execution_fingerprint().unwrap()
        );
        let rendered = policy::compile_context(&context, policy::ContextBudget::default()).unwrap();
        guard_context_snapshot(&s, rendered.as_json()).unwrap();
        let mut d = native_modes().remove(0);
        d.instructions = "Changed future work".into();
        save_mode(&mut store, "change", d);
        assert!(guard_context_pin(&store.load().unwrap(), &context).is_err());
        assert!(guard_context_snapshot(&store.load().unwrap(), rendered.as_json()).is_err());
        context.sources.clear();
        assert!(guard_context_pin(&store.load().unwrap(), &context).is_err());
    }
    #[test]
    fn legacy_selection_is_fenced_against_settings_changes_during_metadata() {
        let mut store = store();
        let state = store.load().unwrap();
        let legacy = resolve_for_task(&state, &PolicyDocument::default(), None).unwrap();
        assert!(legacy.mode.is_none());
        assert!(legacy.profile.is_none());
        assert_eq!(
            legacy
                .context_sources
                .iter()
                .filter(|s| s.identity.reference == "configuration-pin")
                .count(),
            1
        );
        let mut context = ContextInput {
            schema_version: 1,
            id: "legacy-prepared-context".into(),
            version: 1,
            objective: "Legacy task with current restrictions".into(),
            acceptance_criteria: vec!["Preserve admission authority".into()],
            constraints: vec![],
            sources: legacy.context_sources,
            files: vec![],
        };
        let compiled = policy::compile_context(&context, policy::ContextBudget::default()).unwrap();
        guard_context_pin(&state, &context).unwrap();
        guard_context_snapshot(&state, compiled.as_json()).unwrap();
        context.sources.clear();
        // Compatibility for old raw reservations in an unconfigured fixture.
        guard_context_pin(&state, &context).unwrap();
        let mut settings = Settings::default();
        settings.policy.mode_requirements.read_only = true;
        crate::desktop::handle(
            &mut store,
            &crate::desktop::DesktopRequest::SaveSettings {
                command_id: "settings-during-metadata".into(),
                expected_revision: 0,
                settings: Box::new(settings),
            },
        )
        .unwrap();
        let changed = store.load().unwrap();
        assert!(managed(&changed).is_empty());
        assert_eq!(
            guard_context_snapshot(&changed, compiled.as_json()).unwrap_err(),
            "configuration-preparation-stale"
        );
        assert_eq!(
            guard_context_pin(&changed, &context).unwrap_err(),
            "configuration-preparation-pin-missing"
        );
    }
    #[test]
    fn legacy_modes_are_limited_to_the_proven_development_contract() {
        let state = SavedState::default();
        let development = resolve_for_task(&state, &PolicyDocument::default(), None).unwrap();
        assert!(development.mode.is_none());
        let mut reviewer_as_implementer = PolicyDocument::default();
        reviewer_as_implementer.profile.id = "independent-reviewer".into();
        assert_eq!(
            resolve_for_task(&state, &reviewer_as_implementer, None).unwrap_err(),
            "configuration-implementation-profile-role"
        );
        let independent = resolve_for_review(&state, &PolicyDocument::default(), None).unwrap();
        assert_eq!(independent.policy.profile.id, "independent-reviewer");
        assert!(independent.policy.mode_requirements.read_only);
        for id in [
            "research",
            "presentations",
            "documentation",
            "refactoring",
            "improvement-review",
        ] {
            let mut invented = PolicyDocument::default();
            invented.mode.id = id.into();
            assert_eq!(
                resolve_for_task(&state, &invented, None).unwrap_err(),
                "configuration-mode-unresolved"
            );
        }
        let managed_research = PolicyDocument {
            mode: DefinitionBinding {
                id: "research".into(),
                version: NATIVE_DEFINITION_VERSION,
            },
            ..PolicyDocument::default()
        };
        let research = resolve_for_task(&state, &managed_research, None).unwrap();
        assert_eq!(
            research.mode.unwrap().execution_support,
            ExecutionSupport::ArtifactFlowUnproven
        );
    }
    #[test]
    fn preview_unknowns_and_invalid_drafts_have_no_side_effects() {
        let mut store = store();
        let mut mode = local_mode("draft-mode");
        mode.instructions = "Draft instructions".into();
        let result = handle(
            &mut store,
            &ConfigurationRequest::Preview {
                mode: policy().mode,
                profile: policy().profile,
                project_id: None,
                mode_draft: Some(Box::new(mode)),
                profile_draft: None,
                scope_draft: None,
            },
        )
        .unwrap();
        assert!(result["decision"]["route"].to_string().contains("blocked"));
        assert_eq!(result["definitions"]["mode"]["id"], "draft-mode");
        assert_eq!(store.load().unwrap().revision(), 0);
        let mut profile = local_profile("bad-review", true);
        profile.preferences.sandbox_write = Some(true);
        assert!(
            handle(
                &mut store,
                &ConfigurationRequest::Preview {
                    mode: policy().mode,
                    profile: policy().profile,
                    project_id: None,
                    mode_draft: None,
                    profile_draft: Some(Box::new(profile)),
                    scope_draft: None
                }
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<ConfigurationRequest>(
                json!({"type":"catalog","token":"not-allowed"})
            )
            .is_err()
        );
    }
    #[test]
    fn review_resolves_selected_role_and_never_current_global_mode() {
        let mut store = store();
        let mut reviewer = local_profile("specialist-reviewer", true);
        reviewer.instructions = "Check contract boundaries carefully.".into();
        handle(
            &mut store,
            &ConfigurationRequest::SaveProfile {
                command_id: "reviewer".into(),
                expected_revision: 0,
                definition: reviewer,
            },
        )
        .unwrap();
        let mut mode = local_mode("reviewed-development");
        mode.roles[1].profile = DefinitionBinding {
            id: "specialist-reviewer".into(),
            version: 1,
        };
        save_mode(&mut store, "mode", mode);
        let input = PolicyDocument {
            mode: DefinitionBinding {
                id: "reviewed-development".into(),
                version: 1,
            },
            ..policy()
        };
        let state = store.load().unwrap();
        let resolved = resolve_for_review(&state, &input, None).unwrap();
        assert_eq!(resolved.policy.profile.id, "specialist-reviewer");
        assert_eq!(resolved.policy.profile_defaults.sandbox_write, Some(false));
        assert!(resolved.policy.mode_requirements.read_only);
        assert!(
            resolved
                .context_sources
                .iter()
                .any(|s| s.text.contains("Check contract boundaries carefully."))
        );
    }
    use std::collections::BTreeSet;
}
