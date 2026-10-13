use orchestrix_core::runtime_contract::{
    normalize_descriptor, normalize_effective, normalize_request, EventOrigin, Limits,
    RuntimeObserver, INPUT_PROFILE_ID, INPUT_PROFILE_REVISION,
};
use serde_json::{json, Value};

fn vectors() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/runtime-harness/fixtures/portable-input-v1.json"
    ))
    .unwrap()
}

fn corpus() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}

fn observer(corpus: &Value) -> RuntimeObserver {
    RuntimeObserver::new(
        normalize_descriptor(&corpus["descriptors"]["base"]).unwrap(),
        normalize_request(&corpus["requests"]["base"]).unwrap(),
        Limits::default(),
    )
    .unwrap()
}

fn frame(scope: &Value, sequence: u64, kind: &str, fields: Value) -> String {
    let mut envelope = scope.clone();
    envelope["version"] = json!({"major": 1, "minor": 1});
    envelope["sequence"] = json!(sequence);
    envelope["type"] = json!(kind);
    for (key, value) in fields.as_object().unwrap() {
        envelope[key] = value.clone();
    }
    serde_json::to_string(&envelope).unwrap()
}

#[test]
fn shared_input_vectors_identify_the_implemented_profile() {
    let vectors = vectors();
    assert_eq!(vectors["inputProfile"]["id"], INPUT_PROFILE_ID);
    assert_eq!(vectors["inputProfile"]["revision"], INPUT_PROFILE_REVISION);
    for group in ["timestamps", "frames"] {
        let mut names = std::collections::HashSet::new();
        let rows = vectors[group].as_array().unwrap();
        assert!(!rows.is_empty(), "Missing {group} vectors");
        for row in rows {
            assert!(names.insert(row["name"].as_str().unwrap()), "Duplicate {group} vector");
        }
    }
}

#[test]
fn explicit_timestamp_vectors_match_in_effective_and_capability_evidence() {
    let vectors = vectors();
    let corpus = corpus();
    for row in vectors["timestamps"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let proof = json!({
            "source": "configuration", "scope": "attempt",
            "reference": "portable-timestamp-proof", "observedAt": row["input"]
        });
        let effective = json!({"reasoning": {"state": "known", "value": "low", "evidence": proof}});
        let result = normalize_effective(Some(&effective));
        if row["output"].is_null() {
            assert_eq!(result.unwrap_err().code(), "protocol", "{name}: effective rejection");
        } else {
            let normalized = result.unwrap_or_else(|error| panic!("{name}: effective {error}"));
            assert_eq!(normalized["reasoning"]["evidence"]["observedAt"], row["output"], "{name}: effective UTC");
            assert_eq!(normalize_effective(Some(&normalized)).unwrap(), normalized, "{name}: effective idempotence");
        }

        // Exercise each evidence location independently: one invalid connection
        // capability must not hide whether model-scoped evidence also rejects it.
        for model_scoped in [false, true] {
            let mut descriptor = corpus["descriptors"]["base"].clone();
            let capability = if model_scoped {
                &mut descriptor["models"][0]["capabilities"]["reasoning"]
            } else {
                &mut descriptor["capabilities"]["reasoning"]
            };
            capability["evidence"] = proof.clone();
            capability["evidence"]["scope"] = json!(if model_scoped { "model" } else { "connection" });
            let result = normalize_descriptor(&descriptor);
            if row["output"].is_null() {
                assert_eq!(result.unwrap_err().code(), "protocol", "{name}: capability rejection, model={model_scoped}");
            } else {
                let normalized = result.unwrap_or_else(|error| panic!("{name}: capability {error}, model={model_scoped}"));
                let capability = if model_scoped {
                    &normalized.as_json()["models"][0]["capabilities"]["reasoning"]
                } else {
                    &normalized.as_json()["capabilities"]["reasoning"]
                };
                assert_eq!(capability["evidence"]["observedAt"], row["output"], "{name}: capability UTC, model={model_scoped}");
                assert_eq!(normalize_descriptor(normalized.as_json()).unwrap().as_json(), normalized.as_json(), "{name}: descriptor idempotence, model={model_scoped}");
            }
        }
    }
}

#[test]
fn raw_json_vectors_reject_nonportable_data_without_blocking_owned_cleanup() {
    let vectors = vectors();
    let corpus = corpus();
    for row in vectors["frames"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let mut observer = observer(&corpus);
        let initial = observer.snapshot();
        let scope = &initial["scope"];
        let prefix = frame(scope, 1, row["type"].as_str().unwrap(), json!({}));
        let input = format!("{}{}{}", &prefix[..prefix.len() - 1], row["tail"].as_str().unwrap(), "}");
        let result = observer.accept(&input, EventOrigin::Runtime);
        if row["code"].is_null() {
            let event = result.unwrap_or_else(|error| panic!("{name}: unexpected {error}"));
            let snapshot = observer.snapshot();
            assert_eq!(snapshot["observation"], "healthy", "{name}: accepted health");
            assert_eq!(snapshot["fatal"], false, "{name}: accepted fatal flag");
            assert_eq!(snapshot["budget"]["frames"], 1, "{name}: accepted count");
            assert_eq!(snapshot["budget"]["bytes"], input.len() as u64, "{name}: original UTF-8 bytes");
            assert_eq!(snapshot["events"].as_array().unwrap().len(), 1, "{name}: retained count");
            if !row["bytes"].is_null() {
                assert_eq!(event["bytes"], row["bytes"], "{name}: normalized event bytes");
                assert_eq!(snapshot["output"]["assistant"], row["bytes"], "{name}: assistant bytes");
                assert_eq!(snapshot["output"]["bytes"], row["bytes"], "{name}: total bytes");
            }
            assert!(!serde_json::to_string(&snapshot).unwrap().contains("SYNTHETIC_SECRET"), "{name}: discarded payload");
        } else {
            let error = result.unwrap_err();
            assert_eq!(error.code(), row["code"].as_str().unwrap(), "{name}: exact rejection");
            assert!(!error.to_string().contains("SYNTHETIC_SECRET"), "{name}: fixed error");
            let rejected = observer.snapshot();
            assert_eq!(rejected["events"], json!([]), "{name}: rejected events");
            assert_eq!(rejected["effective"], initial["effective"], "{name}: no effective observation");
            assert_eq!(rejected["terminal"], Value::Null, "{name}: no terminal");
            assert_eq!(rejected["outcome"], "unknown", "{name}: no result");
            assert_eq!(rejected["fatal"], true, "{name}: quarantined");
            assert_eq!(rejected["budget"]["frames"], 0, "{name}: no accepted frame");
            assert_eq!(rejected["budget"]["bytes"], 0, "{name}: no accepted bytes");
            assert!(!serde_json::to_string(&rejected).unwrap().contains("SYNTHETIC_SECRET"), "{name}: no raw payload");

            // Quarantine must still accept separately trusted supervisor facts;
            // this validates their structure, without claiming native ownership.
            observer.accept(
                &frame(scope, 1, "process.exited", json!({"role": "runtime", "code": 137, "signal": "killed"})),
                EventOrigin::Supervisor,
            ).unwrap_or_else(|error| panic!("{name}: runtime cleanup {error}"));
            observer.accept(
                &frame(scope, 2, "process.tree-observed", json!({
                    "ownership": "confirmed", "source": "owned-process-group",
                    "termination": "confirmed", "activeCount": 0,
                    "evidenceRef": "portable-cleanup-proof"
                })),
                EventOrigin::Supervisor,
            ).unwrap_or_else(|error| panic!("{name}: tree cleanup {error}"));
            let cleaned = observer.snapshot();
            assert_eq!(cleaned["process"]["runtime"]["code"], 137, "{name}: root exit");
            assert_eq!(cleaned["process"]["tree"]["termination"], "confirmed", "{name}: tree fact");
            assert_eq!(cleaned["events"].as_array().unwrap().len(), 2, "{name}: cleanup-only events");
            assert_eq!(cleaned["effective"], initial["effective"], "{name}: cleanup cannot change configuration");
            assert_eq!(cleaned["terminal"], Value::Null, "{name}: cleanup cannot create terminal");
            assert_eq!(cleaned["outcome"], "unknown", "{name}: cleanup cannot create result");
            assert_eq!(cleaned["fault"]["code"], row["code"], "{name}: original fault remains");
        }
    }
}
