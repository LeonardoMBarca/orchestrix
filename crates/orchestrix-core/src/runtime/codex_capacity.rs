//! Mapping of the pinned, generated Codex v2 schema. No extra RPC, endpoint or credential access.
use crate::capacity::{
    CapacityBucket, CapacityField as Field, CapacityObservation, CapacityWindow, DerivedPercentage,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn field<T>(v: Option<&Value>, parse: impl FnOnce(&Value) -> Option<T>) -> Field<T> {
    match v {
        None => Field::Missing,
        Some(Value::Null) => Field::Null,
        Some(v) => parse(v).map_or(Field::Invalid, |value| Field::Reported { value }),
    }
}
fn label(v: Option<&Value>) -> Field<String> {
    field(v, |v| {
        v.as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 256
                    && !s.chars().any(char::is_control)
                    && crate::persistence_privacy::sanitize_text(s).ok().as_deref() == Some(*s)
            })
            .map(Into::into)
    })
}
fn window(v: Option<&Value>, slot: &str, issues: &mut Vec<String>) -> CapacityWindow {
    let state = match v {
        None => "missing",
        Some(Value::Null) => "null",
        Some(v) if v.is_object() => "reported",
        _ => "invalid",
    };
    let get = |key| {
        if state == "reported" {
            v.and_then(|v| v.get(key))
        } else {
            None
        }
    };
    let used = field(get("usedPercent"), |v| {
        v.as_i64().and_then(|n| i32::try_from(n).ok())
    });
    let duration = field(get("windowDurationMins"), |v| {
        v.as_u64().filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
    });
    let reset = field(get("resetsAt"), |v| {
        v.as_u64().filter(|n| *n <= 8_640_000_000_000)
    });
    if state == "invalid"
        || matches!(used, Field::Invalid)
        || matches!(duration, Field::Invalid)
        || matches!(reset, Field::Invalid)
        || state == "reported" && !matches!(used, Field::Reported { .. })
    {
        issues.push("invalid-window-field".into());
    }
    let remaining = used
        .reported()
        .filter(|n| (0..=100).contains(*n))
        .map(|n| DerivedPercentage {
            value: 100 - *n,
            origin: "derived".into(),
            basis: "100-minus-used-percent".into(),
        });
    if used.reported().is_some() && remaining.is_none() {
        issues.push("usage-outside-percentage-range".into());
    }
    CapacityWindow {
        slot: slot.into(),
        state: state.into(),
        unit: "percent-of-window".into(),
        used_percent: used,
        remaining_percent: remaining,
        duration_minutes: duration,
        resets_at_seconds: reset,
    }
}
fn bucket(v: &Value, key: &str, view: &str, issues: &mut Vec<String>) -> Option<CapacityBucket> {
    if !v.is_object() {
        issues.push("invalid-bucket".into());
        return None;
    }
    let limit_id = label(v.get("limitId"));
    let name = label(v.get("limitName"));
    let model = label(v.get("normalModelSlug"));
    let reason = field(v.get("rateLimitReachedType"), |v| {
        v.as_str()
            .filter(|s| {
                [
                    "rate_limit_reached",
                    "workspace_owner_credits_depleted",
                    "workspace_member_credits_depleted",
                    "workspace_owner_usage_limit_reached",
                    "workspace_member_usage_limit_reached",
                ]
                .contains(s)
            })
            .map(Into::into)
    });
    if [&limit_id, &name, &model, &reason]
        .iter()
        .any(|v| matches!(v, Field::Invalid))
    {
        issues.push("invalid-bucket-field".into());
    }
    Some(CapacityBucket {
        reference: format!(
            "meter-{}",
            &format!("{:x}", Sha256::digest(format!("{view}:{key}").as_bytes()))[..48]
        ),
        limit_id,
        name,
        model_display_alias: model,
        reached_reason: reason,
        windows: ["primary", "secondary"]
            .into_iter()
            .map(|s| window(v.get(s), s, issues))
            .collect(),
    })
}
pub(super) fn normalize(
    value: &Value,
    id: &str,
    revision: u64,
    version: &str,
    now: u64,
    synthetic: bool,
) -> CapacityObservation {
    let mut issues = Vec::new();
    let ordinary = field(value.get("ordinaryUsageAllowed"), Value::as_bool);
    if matches!(ordinary, Field::Invalid) {
        issues.push("invalid-ordinary-usage-permission".into());
    }
    let mut buckets = Vec::new();
    let mut legacy = None;
    let mut view = "not-reported";
    // Quantitative fields are pinned by the retained generated alpha.17.2 and
    // 0.164.0-alpha.11.1 schemas; no new RPC or estimated capability is added.
    // The older supported executable keeps its strict qualitative permission only.
    if synthetic || matches!(version, "0.162.0-alpha.17.2" | "0.164.0-alpha.11.1") {
        if !value.get("rateLimits").is_some_and(Value::is_object) {
            issues.push("legacy-bucket-not-reported".into());
        }
        let old = value.get("rateLimits").filter(|v| !v.is_null());
        if let Some(map) = value
            .get("rateLimitsByLimitId")
            .and_then(Value::as_object)
            .filter(|map| !map.is_empty())
        {
            view = "multi";
            if map.len() > 16 {
                issues.push("bucket-count-limit".into());
            }
            for (key, v) in map.iter().take(16) {
                if let Some(b) = bucket(v, key, "multi", &mut issues) {
                    buckets.push(b);
                }
            }
            legacy = old.and_then(|v| bucket(v, "legacy", "legacy", &mut issues));
        } else {
            if value
                .get("rateLimitsByLimitId")
                .is_some_and(|v| !v.is_null() && !v.is_object())
            {
                issues.push("invalid-multi-bucket-view".into());
            }
            if let Some(old) = old {
                view = "legacy";
                if let Some(b) = bucket(old, "legacy", "legacy", &mut issues) {
                    buckets.push(b);
                }
            }
        }
    } else {
        issues.push("quantitative-schema-not-pinned".into());
    }
    issues.sort();
    issues.dedup();
    issues.truncate(32);
    CapacityObservation {
        schema_version: 1,
        source: "codex-account-rate-limits".into(),
        connection_id: id.into(),
        connection_revision: revision,
        runtime_version: version.into(),
        observed_at_ms: now,
        synthetic,
        ordinary_usage: ordinary,
        bucket_view: view.into(),
        buckets,
        legacy_bucket: legacy,
        completeness: if issues.is_empty() {
            "complete"
        } else {
            "partial"
        }
        .into(),
        coverage: "reported-subset".into(),
        issues,
        capacity_group_ids: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn sample(v: Value) -> CapacityObservation {
        let result = normalize(&v, "connection", 1, "0.162.0-alpha.17.2", 1_000, false);
        result.validate().unwrap();
        result
    }
    #[test]
    fn schema_0164_preserves_window_semantics_and_older_qualitative_profile() {
        let raw = json!({"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":0,"windowDurationMins":300,"resetsAt":2000},"secondary":{"usedPercent":75,"windowDurationMins":10080}},"rateLimitsByLimitId":null});
        let old = normalize(&raw, "connection", 1, "0.162.0-alpha.17.2", 1000, false);
        let new = normalize(&raw, "connection", 1, "0.164.0-alpha.11.1", 1000, false);
        new.validate().unwrap();
        assert_eq!(new.usage_allowed(), Some(false));
        assert_eq!(
            serde_json::to_value(&new.buckets).unwrap(),
            serde_json::to_value(&old.buckets).unwrap()
        );
        assert_eq!(
            new.buckets[0].windows[0]
                .remaining_percent
                .as_ref()
                .unwrap()
                .value,
            100
        );
        let qualitative = normalize(&raw, "connection", 1, "0.162.0-alpha.2", 1000, false);
        assert!(qualitative.buckets.is_empty());
        assert_eq!(qualitative.usage_allowed(), Some(false));
        let mut missing = raw.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("ordinaryUsageAllowed");
        assert_eq!(
            normalize(&missing, "connection", 1, "0.164.0-alpha.11.1", 1000, false).usage_allowed(),
            None
        );
    }
    #[test]
    fn capacity_permission_zero_absence_null_and_invalid_remain_distinct() {
        let p =
            sample(json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":0}}}));
        assert_eq!(p.usage_allowed(), Some(true));
        let w = &p.buckets[0].windows[0];
        assert_eq!(w.used_percent, Field::Reported { value: 0 });
        assert_eq!(w.remaining_percent.as_ref().unwrap().value, 100);
        assert_eq!(w.resets_at_seconds, Field::Missing);
        for (raw, expected) in [
            (json!({}), Field::Missing),
            (json!({"ordinaryUsageAllowed":null}), Field::Null),
            (json!({"ordinaryUsageAllowed":"true"}), Field::Invalid),
        ] {
            let observation = sample(raw);
            assert_eq!(observation.ordinary_usage, expected);
            assert_eq!(observation.usage_allowed(), None);
        }
        let invalid = sample(
            json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":120,"windowDurationMins":-1,"resetsAt":"123"},"secondary":null}}),
        );
        assert_eq!(
            invalid.buckets[0].windows[0].used_percent.reported(),
            Some(&120)
        );
        assert!(invalid.buckets[0].windows[0].remaining_percent.is_none());
        assert_eq!(
            invalid.buckets[0].windows[0].duration_minutes,
            Field::Invalid
        );
        assert_eq!(invalid.buckets[0].windows[1].state, "null");
        assert_eq!(invalid.completeness, "partial");
        assert_eq!(invalid.usage_allowed(), Some(true));
    }
    #[test]
    fn capacity_multi_resources_and_legacy_never_sum_or_infer_model_independence() {
        let empty_multi = sample(
            json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":50}},"rateLimitsByLimitId":{}}),
        );
        assert_eq!(empty_multi.bucket_view, "legacy");
        assert_eq!(
            empty_multi.buckets[0].windows[0].used_percent.reported(),
            Some(&50)
        );
        let p = sample(
            json!({"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":75}},"rateLimitsByLimitId":{"opaque-a":{"limitId":"resource-a","normalModelSlug":"display-only","primary":{"usedPercent":80,"resetsAt":20},"secondary":{"usedPercent":10,"windowDurationMins":10080}},"opaque-b":{"limitName":"Other resource","primary":{"usedPercent":40}}}}),
        );
        assert_eq!(p.usage_allowed(), Some(false));
        assert_eq!(p.buckets.len(), 2);
        assert!(p.legacy_bucket.is_some());
        assert_eq!(p.valid_until(61_000), 20_000);
        assert_eq!(p.capacity_group_ids.len(), 0);
        assert!(!serde_json::to_string(&p).unwrap().contains("opaque-a"));
        let fresh = normalize(
            &json!({"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":100,"resetsAt":20}}}),
            "connection",
            1,
            "0.162.0-alpha.17.2",
            21_000,
            false,
        );
        assert_eq!(fresh.usage_allowed(), Some(false));
        assert_eq!(fresh.valid_until(81_000), 81_000);
    }
    #[test]
    fn capacity_unsupported_quantities_sensitive_fields_and_limits_are_explicit() {
        let p = sample(
            json!({"ordinaryUsageAllowed":true,"accountId":"private-account","rateLimits":{"limitName":"API_KEY=sk-syntheticfixturecredential123456","credits":{"balance":"500.0"},"individualLimit":{"used":"123"}},"rateLimitUpsell":{"secret":"do-not-copy"}}),
        );
        let text = serde_json::to_string(&p).unwrap();
        assert!(!text.contains("private-account"));
        assert!(!text.contains("500.0"));
        assert!(!text.contains("do-not-copy"));
        assert!(!text.contains("sk-synthetic"));
        let old = normalize(
            &json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":50}}}),
            "connection",
            1,
            "0.162.0-alpha.2",
            0,
            false,
        );
        assert!(old.buckets.is_empty());
        assert_eq!(old.usage_allowed(), Some(true));
        assert!(
            old.issues
                .contains(&"quantitative-schema-not-pinned".into())
        );
        let map = (0..20)
            .map(|i| (format!("r{i}"), json!({"primary":{"usedPercent":0}})))
            .collect::<serde_json::Map<_, _>>();
        let bounded =
            sample(json!({"ordinaryUsageAllowed":true,"rateLimits":{},"rateLimitsByLimitId":map}));
        assert_eq!(bounded.buckets.len(), 16);
        assert_eq!(bounded.completeness, "partial");
        assert!(bounded.issues.contains(&"bucket-count-limit".into()));
        let unsafe_number = sample(
            json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":1.5,"windowDurationMins":9007199254740992u64,"resetsAt":8640000000001u64}}}),
        );
        let w = &unsafe_number.buckets[0].windows[0];
        assert_eq!(w.used_percent, Field::Invalid);
        assert_eq!(w.resets_at_seconds, Field::Invalid);
        assert_eq!(w.duration_minutes, Field::Invalid);
    }
}
