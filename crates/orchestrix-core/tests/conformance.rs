use orchestrix_core::runtime_contract::{assert_admission, capability_decision, normalize_descriptor, normalize_effective, normalize_request, ContractError, EventOrigin, Limits, RuntimeObserver, INPUT_PROFILE_ID, INPUT_PROFILE_REVISION};
use serde_json::{json, Value};

fn outcome(result: Result<Value, ContractError>) -> Value {
    match result { Ok(value) => json!({"ok": true, "value": value}), Err(error) => json!({"ok": false, "code": error.code()}) }
}
fn limits(value: &Value) -> Limits {
    let default = Limits::default();
    Limits { max_frame_bytes: value["maxFrameBytes"].as_u64().unwrap_or(default.max_frame_bytes), max_events: value["maxEvents"].as_u64().unwrap_or(default.max_events), max_total_bytes: value["maxTotalBytes"].as_u64().unwrap_or(default.max_total_bytes) }
}
fn execute_case(corpus: &Value, case: &Value) {
    let name = case["name"].as_str().unwrap();
    let descriptor = if case["descriptor"].is_string() { &corpus["descriptors"][case["descriptor"].as_str().unwrap()] } else { &case["descriptor"] };
    let request = if case["request"].is_string() { &corpus["requests"][case["request"].as_str().unwrap()] } else { &case["request"] };
    let constructor = normalize_descriptor(descriptor).and_then(|d| normalize_request(request).and_then(|r| RuntimeObserver::new(d, r, limits(&case["limits"]))));
    let mut observer = match constructor {
        Ok(value) => {
            assert_eq!(json!({"ok": true, "value": {"descriptor": value.descriptor().as_json(), "request": value.request().as_json(), "snapshot": value.snapshot()}}), case["constructor"], "{name}: construction");
            value
        }
        Err(error) => { assert_eq!(json!({"ok": false, "code": error.code()}), case["constructor"], "{name}: rejected construction"); return; }
    };
    for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
        let action = &step["action"];
        let result = match action["op"].as_str().unwrap() {
            "accept" => {
                // Trusted fixture host passes origin separately from frame JSON.
                let origin = match action["origin"].as_str().unwrap_or("runtime") { "runtime" => EventOrigin::Runtime, "supervisor" => EventOrigin::Supervisor, _ => panic!("Unknown fixture origin") };
                observer.accept(action["frame"].as_str().unwrap(), origin)
            }
            "close" => Ok(observer.close()),
            "transport-lost" => observer.transport_lost(action["reason"].as_str().unwrap()),
            "admission" => assert_admission(observer.descriptor(), observer.request(), action.get("effective").or(Some(&observer.snapshot()["effective"]))).map(|()| json!(true)),
            "capability" => capability_decision(observer.descriptor(), action["name"].as_str().unwrap(), action["modelId"].as_str()),
            "effective" => normalize_effective(action.get("value")),
            _ => panic!("Unknown fixture operation"),
        };
        assert_eq!(outcome(result), step["expected"], "{name}: operation {index}");
        assert_eq!(observer.snapshot(), step["snapshot"], "{name}: snapshot {index}");
    }
}

#[test]
fn shared_wire_corpus_matches_every_js_checkpoint() {
    let corpus: Value = serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap();
    assert_eq!(corpus["schemaVersion"], 1); assert_eq!(corpus["contractVersion"], json!({"major": 1, "minor": 1}));
    assert_eq!(corpus["validationKind"], "offline-conformance");
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 245);
    assert_eq!(corpus["inputProfile"], json!({"id": INPUT_PROFILE_ID, "revision": INPUT_PROFILE_REVISION}));
    let mut names = std::collections::HashSet::new();
    for case in corpus["cases"].as_array().unwrap() { assert!(names.insert(case["name"].as_str().unwrap())); execute_case(&corpus, case); }
}

#[test]
fn required_real_and_synthetic_cases_are_present_and_remain_distinct() {
    let corpus: Value = serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    for name in ["fake-success", "fake-event-invalid", "fake-auth-invalid", "fake-rate-limit", "fake-interruption", "fake-timeout", "fake-crash", "codex-recorded-owned-job-stop-attempt-3", "codex-recorded-cooperative-interrupt-attempt-3", "codex-synthetic-explicit-stop-association-attempt-2"] {
        assert!(cases.iter().any(|case| case["name"] == name), "Missing required case {name}");
    }
    let final_state = |name: &str| cases.iter().find(|case| case["name"] == name).unwrap()["steps"].as_array().unwrap().last().unwrap()["snapshot"].clone();
    let historical = final_state("codex-recorded-owned-job-stop-attempt-3");
    assert_eq!(historical["outcome"], "unknown"); assert_eq!(historical["process"]["runtime"]["exited"], false);
    let synthetic = final_state("codex-synthetic-explicit-stop-association-attempt-2");
    assert_eq!(synthetic["outcome"], "unknown"); assert_eq!(synthetic["process"]["runtime"]["code"], 137); assert_eq!(synthetic["process"]["helper"]["code"], 0);
    assert_eq!(synthetic["process"]["tree"]["termination"], "confirmed");
}

#[test]
fn error_text_cannot_carry_input_secrets_and_snapshots_are_owned_copies() {
    let corpus: Value = serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap();
    let descriptor = normalize_descriptor(&corpus["descriptors"]["base"]).unwrap();
    let request = normalize_request(&corpus["requests"]["base"]).unwrap();
    let mut observer = RuntimeObserver::new(descriptor, request, Limits::default()).unwrap();
    let error = observer.accept("SYNTHETIC_PRIVATE_CREDENTIAL", EventOrigin::Runtime).unwrap_err();
    assert_eq!(error.code(), "protocol"); assert!(!error.to_string().contains("SYNTHETIC_PRIVATE_CREDENTIAL"));
    let mut state = observer.snapshot(); state["outcome"] = json!("completed");
    assert_eq!(observer.snapshot()["outcome"], "unknown");
    assert!(!serde_json::to_string(&observer.snapshot()).unwrap().contains("SYNTHETIC_PRIVATE_CREDENTIAL"));
}
