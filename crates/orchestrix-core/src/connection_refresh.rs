//! Connection diagnostic cadence, distinct from worker scheduling and source evidence leases.
use crate::connections::ConnectionLink;
use serde::{Deserialize, Serialize};

pub const SOURCE_TTL_MS: u64 = 60_000;
pub const MIN_INTERVAL_MS: u64 = 60_000;
pub const MAX_INTERVAL_MS: u64 = 86_400_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RefreshMode {
    Manual,
    Automatic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationValidity {
    pub identity: u64,
    pub configuration: u64,
    pub availability: u64,
    pub models: u64,
}
impl Default for ObservationValidity {
    fn default() -> Self {
        Self {
            identity: 300_000,
            configuration: 900_000,
            availability: 60_000,
            models: 900_000,
        }
    }
}
impl ObservationValidity {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.identity,
            self.configuration,
            self.availability,
            self.models,
        ]
        .iter()
        .any(|n| !(1..=86_400_000).contains(n))
        {
            return Err("refresh-validity-bounds".into());
        }
        Ok(())
    }
    pub fn effective(&self, policy: &Self) -> Self {
        Self {
            identity: self.identity.min(policy.identity).min(SOURCE_TTL_MS),
            configuration: self
                .configuration
                .min(policy.configuration)
                .min(SOURCE_TTL_MS),
            availability: self
                .availability
                .min(policy.availability)
                .min(SOURCE_TTL_MS),
            models: self.models.min(policy.models).min(SOURCE_TTL_MS),
        }
    }
    pub fn minimum(&self) -> u64 {
        self.identity
            .min(self.configuration)
            .min(self.availability)
            .min(self.models)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshSettings {
    pub schema_version: u32,
    pub mode: RefreshMode,
    /// Requested cadence, never evidence validity. Adapter frequency bounds still win.
    pub interval_ms: u64,
    pub validity_ms: ObservationValidity,
    /// Explicit consent for this binding scope; never exported by policy portability.
    pub recurring_consent: bool,
}
impl Default for RefreshSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            mode: RefreshMode::Manual,
            interval_ms: 300_000,
            validity_ms: ObservationValidity::default(),
            recurring_consent: false,
        }
    }
}
impl RefreshSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || !(1_000..=MAX_INTERVAL_MS).contains(&self.interval_ms) {
            return Err("refresh-settings-bounds".into());
        }
        self.validity_ms.validate()?;
        if [
            self.validity_ms.identity,
            self.validity_ms.configuration,
            self.validity_ms.availability,
            self.validity_ms.models,
        ]
        .iter()
        .any(|n| !(1_000..=900_000).contains(n))
        {
            return Err("refresh-validity-bounds".into());
        }
        if self.mode == RefreshMode::Automatic && !self.recurring_consent {
            return Err("recurring-metadata-not-authorized".into());
        }
        if self.mode == RefreshMode::Manual && self.recurring_consent {
            return Err("manual-refresh-consent".into());
        }
        Ok(())
    }
    pub fn effective_interval_ms(&self) -> u64 {
        self.interval_ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RefreshConfiguration {
    pub schema_version: u32,
    pub connection_id: String,
    pub revision: u64,
    pub binding_revision: u64,
    pub scope_key: String,
    pub settings: RefreshSettings,
    pub configured_at_ms: u64,
}
impl RefreshConfiguration {
    pub(crate) fn validate(&self) -> Result<(), String> {
        crate::domain::ConnectionId::new(&self.connection_id)
            .map_err(|_| "refresh-connection-id")?;
        self.settings.validate()?;
        if self.schema_version != 1
            || self.revision == 0
            || self.revision > i64::MAX as u64
            || self.binding_revision == 0
            || !crate::connections::hash(&self.scope_key)
        {
            return Err("refresh-configuration-schema".into());
        }
        Ok(())
    }
    pub(crate) fn authorized_for(&self, link: &ConnectionLink) -> bool {
        self.settings.mode == RefreshMode::Automatic
            && self.settings.recurring_consent
            && self.binding_revision == link.binding_revision
            && self.scope_key == binding_scope(link)
            && link.enabled
            && link.linked
    }
}
/// This opaque key includes the auth store, executable/hash/version and registration directory.
/// It grants no access and is never a credential or a portable account identity.
pub(crate) fn binding_scope(link: &ConnectionLink) -> String {
    crate::git_records::content_hash(
        serde_json::to_vec(&serde_json::json!({
            "connectionId":link.id,"bindingRevision":link.binding_revision,
            "runtime":link.runtime,"executableSha256":link.executable_sha256,
            "directory":link.metadata_directory
        }))
        .expect("serializable binding")
        .as_slice(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_default_never_authorizes_recurring_reads() {
        let mut s = RefreshSettings::default();
        assert!(s.validate().is_ok());
        s.mode = RefreshMode::Automatic;
        assert_eq!(
            s.validate().unwrap_err(),
            "recurring-metadata-not-authorized"
        );
        s.recurring_consent = true;
        s.interval_ms = 1_000;
        assert_eq!(s.effective_interval_ms(), 60_000);
    }
    #[test]
    fn leases_shorten_without_renewing_observation() {
        let s = ObservationValidity::default();
        let mut policy = s.clone();
        policy.identity = 5_000;
        let effective = s.effective(&policy);
        assert_eq!(effective.identity, 5_000);
        assert_eq!(effective.models, 60_000);
        let observed: u64 = 100;
        assert_eq!(observed.saturating_add(effective.minimum()), 5_100);
        assert_eq!(observed.saturating_add(s.effective(&s).minimum()), 60_100);
    }
}
