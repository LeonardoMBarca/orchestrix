//! Bounded, provider-neutral capacity facts. These observations are not reserves or execution grants.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CapacityField<T> {
    Reported { value: T },
    Missing,
    Null,
    Invalid,
}
impl<T> CapacityField<T> {
    pub fn reported(&self) -> Option<&T> {
        match self {
            Self::Reported { value } => Some(value),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DerivedPercentage {
    pub value: i32,
    pub origin: String,
    pub basis: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityWindow {
    pub slot: String,
    pub state: String,
    pub unit: String,
    pub used_percent: CapacityField<i32>,
    pub remaining_percent: Option<DerivedPercentage>,
    pub duration_minutes: CapacityField<u64>,
    pub resets_at_seconds: CapacityField<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityBucket {
    pub reference: String,
    pub limit_id: CapacityField<String>,
    pub name: CapacityField<String>,
    /// Provider display alias, never proof that a selected model has an independent quota.
    pub model_display_alias: CapacityField<String>,
    pub reached_reason: CapacityField<String>,
    pub windows: Vec<CapacityWindow>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityObservation {
    pub schema_version: u32,
    pub source: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub runtime_version: String,
    pub observed_at_ms: u64,
    pub synthetic: bool,
    pub ordinary_usage: CapacityField<bool>,
    pub bucket_view: String,
    pub buckets: Vec<CapacityBucket>,
    pub legacy_bucket: Option<CapacityBucket>,
    pub completeness: String,
    pub coverage: String,
    pub issues: Vec<String>,
    /// Existing local memberships only; absence never invents an account or independent balance.
    pub capacity_group_ids: Vec<String>,
}
impl CapacityObservation {
    pub fn usage_allowed(&self) -> Option<bool> {
        self.ordinary_usage.reported().copied()
    }
    /// A reported future reset shortens an old observation; crossing it never replenishes a balance.
    /// A genuinely new read after that timestamp retains its own, newly observed permission.
    pub fn valid_until(&self, source_until: u64) -> u64 {
        self.buckets
            .iter()
            .chain(self.legacy_bucket.iter())
            .flat_map(|b| &b.windows)
            .filter_map(|w| {
                w.resets_at_seconds
                    .reported()
                    .and_then(|s| s.checked_mul(1000))
            })
            .filter(|t| *t > self.observed_at_ms)
            .fold(source_until, u64::min)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1
            || self.source != "codex-account-rate-limits"
            || crate::domain::ConnectionId::new(&self.connection_id).is_err()
            || self.connection_revision == 0
            || self.runtime_version.is_empty()
            || self.runtime_version.len() > 80
            || self.observed_at_ms > 8_640_000_000_000_000
            || self.buckets.len() > 16
            || !["multi", "legacy", "not-reported"].contains(&self.bucket_view.as_str())
            || !["complete", "partial"].contains(&self.completeness.as_str())
            || self.coverage != "reported-subset"
            || self.issues.len() > 32
            || self.issues.iter().any(|s| {
                s.len() > 100
                    || !s
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            })
            || self.capacity_group_ids.len() > 64
            || self
                .capacity_group_ids
                .iter()
                .any(|s| crate::domain::CapacityGroupId::new(s).is_err())
        {
            return Err("capacity-schema");
        }
        let mut references = std::collections::BTreeSet::new();
        for b in self.buckets.iter().chain(self.legacy_bucket.iter()) {
            if crate::domain::LocalRef::new(&b.reference).is_err()
                || !references.insert(&b.reference)
                || b.windows.len() != 2
            {
                return Err("capacity-bucket");
            }
            for field in [
                &b.limit_id,
                &b.name,
                &b.model_display_alias,
                &b.reached_reason,
            ] {
                if field.reported().is_some_and(|s| {
                    s.is_empty() || s.len() > 256 || s.chars().any(char::is_control)
                }) {
                    return Err("capacity-label");
                }
            }
            for (index, w) in b.windows.iter().enumerate() {
                if w.slot != ["primary", "secondary"][index]
                    || !["reported", "missing", "null", "invalid"].contains(&w.state.as_str())
                    || w.unit != "percent-of-window"
                    || w.duration_minutes
                        .reported()
                        .is_some_and(|n| *n == 0 || *n > 9_007_199_254_740_991)
                    || w.resets_at_seconds
                        .reported()
                        .is_some_and(|n| *n > 8_640_000_000_000)
                {
                    return Err("capacity-window");
                }
                let expected = w
                    .used_percent
                    .reported()
                    .filter(|n| (0..=100).contains(*n))
                    .map(|n| 100 - *n);
                if let Some(r) = &w.remaining_percent
                    && (Some(r.value) != expected
                        || r.origin != "derived"
                        || r.basis != "100-minus-used-percent")
                {
                    return Err("capacity-derived-value");
                }
                if w.state != "reported"
                    && (w.used_percent.reported().is_some()
                        || w.remaining_percent.is_some()
                        || w.duration_minutes.reported().is_some()
                        || w.resets_at_seconds.reported().is_some())
                {
                    return Err("capacity-window");
                }
            }
        }
        Ok(())
    }
}
