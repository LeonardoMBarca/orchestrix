//! Contract 1.1 observation/validation, separate from execution authorization.
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use serde_core::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::collections::HashSet;
use std::fmt;

const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
pub const CAPABILITIES: [&str; 11] = ["modelSelection", "reasoning", "resume", "quota", "tools", "webSearch", "streaming", "sandbox", "approvals", "nativeDelegation", "contextManagement"];
const REASONING: [&str; 7] = ["auto", "runtime-default", "minimal", "low", "medium", "high", "maximum"];
const ERRORS: [&str; 17] = ["authentication", "authentication-unknown", "unavailable", "availability-unknown", "evidence-missing", "rate-limit", "unsupported", "policy", "timeout", "crash", "transport-loss", "protocol", "ownership", "sequence", "version", "budget", "busy"];
const FATAL: [&str; 5] = ["protocol", "ownership", "sequence", "version", "budget"];
const FACTUAL: [&str; 7] = ["configuration.observation-recorded", "attempt.admission-observed", "attempt.start-observed", "attempt.terminal-observed", "attempt.interrupt-result", "output.observed", "process.streams-observed"];

/// Fixed code/message only: never retains raw input or provider messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractError { code: &'static str }
impl ContractError { pub fn code(&self) -> &'static str { self.code } }
impl fmt::Display for ContractError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result { write!(out, "Runtime contract rejected: {}", self.code) }
}
impl std::error::Error for ContractError {}
pub type Result<T> = std::result::Result<T, ContractError>;
fn error(code: &'static str) -> ContractError { ContractError { code } }
fn ensure(ok: bool, code: &'static str) -> Result<()> { if ok { Ok(()) } else { Err(error(code)) } }
fn object(value: &Value) -> Result<()> { ensure(value.is_object(), "protocol") }
fn fallback<'a>(value: &'a Value, default: &'a Value) -> &'a Value { if value.is_null() { default } else { value } }
fn enum_value(value: &Value, choices: &[&str]) -> Result<Value> {
    ensure(value.as_str().is_some_and(|s| choices.contains(&s)), "protocol")?; Ok(value.clone())
}
fn default_enum(value: &Value, default: &str, choices: &[&str]) -> Result<Value> { enum_value(fallback(value, &json!(default)), choices) }
fn int(value: &Value, min: f64, max: f64, code: &'static str) -> Result<u64> {
    let number = value.as_f64().ok_or_else(|| error(code))?;
    ensure(number.is_finite() && number.fract() == 0.0 && number >= min && number <= max && number.abs() <= SAFE_INTEGER, code)?;
    Ok(number as u64)
}
fn safe_int(value: &Value, code: &'static str) -> Result<u64> { int(value, 0.0, SAFE_INTEGER, code) }
fn local_id(value: &Value) -> Result<Value> {
    ensure(value.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 80 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')), "ownership")?;
    Ok(value.clone())
}
fn token(value: &Value) -> Result<Value> {
    ensure(value.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 100 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))), "protocol")?;
    Ok(value.clone())
}
fn optional_id(value: &Value) -> Result<Value> { if value.is_null() { Ok(Value::Null) } else { local_id(value) } }
fn version(value: &Value) -> Result<Value> {
    ensure(value.is_object(), "version")?;
    let major = safe_int(&value["major"], "version")?;
    let minor = safe_int(&value["minor"], "version")?;
    ensure(major == 1, "version")?; Ok(json!({"major": 1, "minor": minor}))
}
fn scope(value: &Value) -> Result<Value> {
    ensure(value.is_object(), "ownership")?;
    Ok(json!({"connectionId": local_id(&value["connectionId"])?, "sessionId": local_id(&value["sessionId"])?, "attemptId": local_id(&value["attemptId"])?, "turnId": optional_id(&value["turnId"])?}))
}
fn evidence(value: Option<&Value>) -> Result<Value> {
    let empty = json!({}); let input = value.unwrap_or(&empty); object(input)?;
    let source = default_enum(&input["source"], "none", &["none", "declared", "catalog", "configuration", "execution", "synthetic"])?;
    let mut observed = Value::Null;
    if !input["observedAt"].is_null() {
        let text = input["observedAt"].as_str().ok_or_else(|| error("protocol"))?;
        let prefix = text.as_bytes();
        ensure(prefix.len() >= 11 && prefix[..4].iter().all(u8::is_ascii_digit) && prefix[4] == b'-' && prefix[5..7].iter().all(u8::is_ascii_digit) && prefix[7] == b'-' && prefix[8..10].iter().all(u8::is_ascii_digit) && prefix[10] == b'T', "protocol")?;
        let date = DateTime::parse_from_rfc3339(text).map_err(|_| error("protocol"))?;
        ensure(date.timestamp_subsec_nanos() < 1_000_000_000, "protocol")?;
        observed = json!(date.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    let reference = optional_id(&input["reference"])?;
    ensure(source != "none" || (reference.is_null() && observed.is_null()), "protocol")?;
    Ok(json!({"source": source, "scope": default_enum(&input["scope"], "connection", &["connection", "model", "attempt"])?, "reference": reference, "observedAt": observed}))
}
fn capability(value: Option<&Value>) -> Result<Value> {
    let empty = json!({}); let input = value.unwrap_or(&empty); object(input)?;
    Ok(json!({"support": default_enum(&input["support"], "unknown", &["supported", "unsupported", "unknown", "runtime-managed"])?, "availability": default_enum(&input["availability"], "unknown", &["available", "unavailable", "unknown"])?, "configuration": default_enum(&input["configuration"], "unknown", &["enabled", "disabled", "unknown"])?, "evidence": evidence(input.get("evidence"))?}))
}
fn capabilities(value: Option<&Value>) -> Result<Value> {
    let empty = json!({}); let input = value.unwrap_or(&empty); object(input)?;
    let mut result = Map::new();
    for name in CAPABILITIES { result.insert(name.into(), capability(input.get(name))?); }
    Ok(Value::Object(result))
}

/// Validated, whitelisted wire data; callers cannot mutate its internals.
#[derive(Clone, Debug)]
pub struct RuntimeDescriptor(Value);
impl RuntimeDescriptor { pub fn as_json(&self) -> &Value { &self.0 } }
#[derive(Clone, Debug)]
pub struct RuntimeRequest(Value);
impl RuntimeRequest { pub fn as_json(&self) -> &Value { &self.0 } }

pub fn normalize_descriptor(value: &Value) -> Result<RuntimeDescriptor> {
    object(value)?; object(&value["connection"])?;
    let input = value["models"].as_array().ok_or_else(|| error("protocol"))?;
    ensure(input.len() <= 64, "protocol")?;
    let mut ids = HashSet::new(); let mut models = Vec::new();
    for row in input {
        object(row)?; let id = local_id(&row["id"])?;
        ensure(ids.insert(id.as_str().unwrap().to_owned()), "protocol")?;
        let mut levels = Value::Null;
        if !row["reasoningLevels"].is_null() {
            let raw = row["reasoningLevels"].as_array().ok_or_else(|| error("protocol"))?;
            ensure(raw.len() <= 5, "protocol")?; let mut unique = HashSet::new();
            for item in raw { enum_value(item, &REASONING[2..])?; ensure(unique.insert(item.as_str().unwrap()), "protocol")?; }
            levels = json!(raw);
        }
        models.push(json!({"id": id, "availability": default_enum(&row["availability"], "unknown", &["available", "unavailable", "unknown"])?, "reasoningLevels": levels, "capabilities": capabilities(row.get("capabilities"))?}));
    }
    let connection = &value["connection"];
    let revision = int(&connection["revision"], 1.0, SAFE_INTEGER, "ownership")?;
    Ok(RuntimeDescriptor(json!({"version": version(&value["version"])?, "adapterId": token(&value["adapterId"])?, "runtimeVersion": token(&value["runtimeVersion"])?, "connection": {"id": local_id(&connection["id"])?, "revision": revision, "authentication": enum_value(&connection["authentication"], &["authenticated", "unauthenticated", "unknown"])?, "billing": enum_value(&connection["billing"], &["subscription", "api", "unknown"])?, "availability": default_enum(&connection["availability"], "unknown", &["available", "unavailable", "unknown"])?}, "catalog": {"completeness": default_enum(&value["catalog"]["completeness"], "unknown", &["complete", "partial", "unknown"])?}, "capabilities": capabilities(value.get("capabilities"))?, "models": models, "contextOwnership": context_ownership()})))
}
pub fn normalize_request(value: &Value) -> Result<RuntimeRequest> {
    object(value)?; object(&value["requested"])?; let mut owner = scope(value)?;
    let raw = &value["requested"];
    let model_policy = default_enum(&raw["modelPolicy"], "auto", &["auto", "specific", "runtime-default"])?;
    let model_id = optional_id(&raw["modelId"])?;
    ensure((model_policy == "specific") != model_id.is_null(), "policy")?;
    let billing = default_enum(&value["billingMode"], "subscription-only", &["subscription-only", "explicit-api"])?;
    ensure(value.get("paidFallback").is_none() || value["paidFallback"] == "forbidden", "policy")?;
    let consent = optional_id(&value["billingAuthorizationRef"])?;
    ensure(billing != "explicit-api" || !consent.is_null(), "policy")?;
    owner["version"] = version(&value["version"])?;
    owner["connectionRevision"] = json!(int(&value["connectionRevision"], 1.0, SAFE_INTEGER, "ownership")?);
    owner["billingMode"] = billing; owner["paidFallback"] = json!("forbidden"); owner["billingAuthorizationRef"] = consent;
    owner["contextPackRef"] = optional_id(&value["contextPackRef"])?;
    owner["requested"] = json!({"strategy": default_enum(&raw["strategy"], "balanced", &["efficiency", "performance", "balanced", "custom"])?, "presetRef": optional_id(&raw["presetRef"])?, "modelPolicy": model_policy, "modelId": model_id, "reasoning": default_enum(&raw["reasoning"], "auto", &REASONING)?, "sandbox": default_enum(&raw["sandbox"], "read-only", &["read-only", "workspace-write"])?, "network": default_enum(&raw["network"], "disabled", &["enabled", "disabled"])?, "approval": default_enum(&raw["approval"], "never", &["never", "ask", "runtime-managed"])?});
    Ok(RuntimeRequest(owner))
}
pub fn normalize_effective(value: Option<&Value>) -> Result<Value> {
    let empty = json!({}); let raw = value.unwrap_or(&empty); object(raw)?; let mut result = Map::new();
    for key in ["model", "reasoning", "sandbox", "network", "approval"] {
        let item = fallback(&raw[key], &empty); object(item)?;
        let state = default_enum(&item["state"], "unknown", &["known", "unknown", "runtime-managed"])?;
        let proof = evidence(item.get("evidence"))?;
        let mut resolved = Value::Null;
        if state == "known" {
            resolved = match key { "model" => local_id(&item["value"]), "reasoning" => enum_value(&item["value"], &REASONING[1..]), "sandbox" => enum_value(&item["value"], &["read-only", "workspace-write"]), "network" => enum_value(&item["value"], &["enabled", "disabled"]), _ => enum_value(&item["value"], &["never", "ask", "runtime-managed"]) }?;
            ensure([json!("configuration"), json!("execution"), json!("synthetic")].contains(&proof["source"]), "protocol")?;
        } else { ensure(item["value"].is_null(), "protocol")?; }
        result.insert(key.into(), json!({"state": state, "value": resolved, "evidence": proof}));
    }
    Ok(Value::Object(result))
}
pub fn capability_decision(descriptor: &RuntimeDescriptor, name: &str, model_id: Option<&str>) -> Result<Value> {
    ensure(CAPABILITIES.contains(&name), "unsupported")?;
    let empty = capability(None)?;
    let cap = match model_id { Some(id) => find_model(&descriptor.0, &json!(id)).map(|m| &m["capabilities"][name]).unwrap_or(&empty), None => &descriptor.0["capabilities"][name] };
    let reason = if cap["support"] != "supported" { cap["support"].clone() } else if cap["availability"] != "available" { json!(format!("availability-{}", cap["availability"].as_str().unwrap())) } else if cap["configuration"] != "enabled" { json!(format!("configuration-{}", cap["configuration"].as_str().unwrap())) } else { Value::Null };
    Ok(json!({"allowed": reason.is_null(), "reason": reason, "capability": cap}))
}
fn find_model<'a>(descriptor: &'a Value, id: &Value) -> Option<&'a Value> { descriptor["models"].as_array()?.iter().find(|m| m["id"] == *id) }
fn require_capability(descriptor: &RuntimeDescriptor, name: &str, model_id: &str) -> Result<()> {
    let decision = capability_decision(descriptor, name, Some(model_id))?;
    if decision["allowed"] == true { return Ok(()); }
    Err(error(match decision["reason"].as_str().unwrap() { "unsupported" | "runtime-managed" => "unsupported", "configuration-disabled" => "policy", "availability-unavailable" => "unavailable", _ => "evidence-missing" }))
}
fn require_level(descriptor: &RuntimeDescriptor, model: Option<&Value>, level: &Value) -> Result<()> {
    let row = model.ok_or_else(|| error("evidence-missing"))?;
    require_capability(descriptor, "reasoning", row["id"].as_str().unwrap())?;
    let levels = row["reasoningLevels"].as_array().ok_or_else(|| error("evidence-missing"))?;
    ensure(levels.contains(level), "unsupported")
}
pub fn assert_admission(descriptor: &RuntimeDescriptor, request: &RuntimeRequest, effective: Option<&Value>) -> Result<()> {
    let runtime = &descriptor.0; let intent = &request.0;
    ensure(intent["connectionId"] == runtime["connection"]["id"] && intent["connectionRevision"] == runtime["connection"]["revision"], "ownership")?;
    let auth = runtime["connection"]["authentication"].as_str().unwrap();
    ensure(auth == "authenticated", if auth == "unknown" { "authentication-unknown" } else { "authentication" })?;
    let available = runtime["connection"]["availability"].as_str().unwrap();
    ensure(available == "available", if available == "unknown" { "availability-unknown" } else { "unavailable" })?;
    ensure(if intent["billingMode"] == "subscription-only" { runtime["connection"]["billing"] == "subscription" } else { runtime["connection"]["billing"] == "api" }, "policy")?;
    let resolved = normalize_effective(effective)?;
    if resolved["model"]["state"] == "known" { let row = find_model(runtime, &resolved["model"]["value"]).ok_or_else(|| error("evidence-missing"))?; ensure(row["availability"] != "unavailable", "unavailable")?; }
    let requested = &intent["requested"]; let explicit_level = ![json!("auto"), json!("runtime-default")].contains(&requested["reasoning"]);
    if requested["modelPolicy"] == "specific" {
        let row = find_model(runtime, &requested["modelId"]).ok_or_else(|| error("evidence-missing"))?;
        ensure(row["availability"] == "available", if row["availability"] == "unknown" { "availability-unknown" } else { "unavailable" })?;
        require_capability(descriptor, "modelSelection", row["id"].as_str().unwrap())?;
        if explicit_level { require_level(descriptor, Some(row), &requested["reasoning"])?; }
    } else if explicit_level { require_level(descriptor, if resolved["model"]["state"] == "known" { find_model(runtime, &resolved["model"]["value"]) } else { None }, &requested["reasoning"])?; }
    Ok(())
}
fn context_ownership() -> Value { json!({"conversation": "runtime", "projectMemory": "orchestrix"}) }
fn runtime_error(value: &Value) -> Result<Value> {
    object(value)?;
    Ok(json!({"code": enum_value(&value["code"], &ERRORS)?, "phase": enum_value(&value["phase"], &["connect", "admission", "running", "cleanup"])?, "dispatch": default_enum(&value["dispatch"], "unknown", &["not-sent", "sent", "unknown"])?, "retryable": value["retryable"] == true}))
}
fn streams(value: &Value) -> Result<Value> {
    object(value)?; let mut result = Map::new();
    for key in ["stdin", "stdout", "stderr", "faulted"] { ensure(value[key].is_boolean(), "protocol")?; result.insert(key.into(), value[key].clone()); }
    Ok(Value::Object(result))
}
fn process_state() -> Value { json!({"exited": false, "code": null, "signal": "unknown", "streams": null, "stdioClosed": false}) }

// Project only contract fields while validating syntax of ignored data. Unknown
// nested payloads are skipped by the decoder, not materialized as recursive Value
// trees. The retained schema has finite depth; frame bytes are budgeted beforehand.
#[derive(Clone, Copy)]
enum Projection { Event, Version, Effective, Setting, Evidence, Error, Streams, Scalar }
impl Projection {
    fn child(self, key: &str) -> Option<Self> {
        use Projection::*;
        match (self, key) {
            (Event, "version") => Some(Version), (Event, "effective") => Some(Effective),
            (Event, "error") => Some(Error), (Event, "streams") => Some(Streams),
            (Event, "connectionId" | "sessionId" | "attemptId" | "turnId" | "connectionRevision" | "sequence" | "type" | "acknowledged" | "outcome" | "startObserved" | "channel" | "bytes" | "fatal" | "role" | "code" | "signal" | "ownership" | "source" | "termination" | "activeCount" | "evidenceRef") => Some(Scalar),
            (Version, "major" | "minor") => Some(Scalar),
            (Effective, "model" | "reasoning" | "sandbox" | "network" | "approval") => Some(Setting),
            (Setting, "evidence") => Some(Evidence), (Setting, "state" | "value") => Some(Scalar),
            (Evidence, "source" | "scope" | "reference" | "observedAt") => Some(Scalar),
            (Error, "code" | "phase" | "dispatch" | "retryable") => Some(Scalar),
            (Streams, "stdin" | "stdout" | "stderr" | "faulted") => Some(Scalar),
            _ => None,
        }
    }
}
impl<'de> DeserializeSeed<'de> for Projection {
    type Value = Value;
    fn deserialize<D: serde_core::Deserializer<'de>>(self, decoder: D) -> std::result::Result<Value, D::Error> { decoder.deserialize_any(self) }
}
impl<'de> Visitor<'de> for Projection {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("a runtime contract field") }
    fn visit_bool<E: serde_core::de::Error>(self, value: bool) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_i64<E: serde_core::de::Error>(self, value: i64) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_u64<E: serde_core::de::Error>(self, value: u64) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_f64<E: serde_core::de::Error>(self, value: f64) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_str<E: serde_core::de::Error>(self, value: &str) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_string<E: serde_core::de::Error>(self, value: String) -> std::result::Result<Value, E> { Ok(json!(value)) }
    fn visit_unit<E: serde_core::de::Error>(self) -> std::result::Result<Value, E> { Ok(Value::Null) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> std::result::Result<Value, A::Error> {
        while items.next_element::<IgnoredAny>()?.is_some() {}
        Ok(json!([])) // Wrong-type arrays remain wrong-type, without their payload.
    }
    fn visit_map<A: MapAccess<'de>>(self, mut items: A) -> std::result::Result<Value, A::Error> {
        let mut result = Map::new();
        while let Some(key) = items.next_key::<String>()? {
            if let Some(child) = self.child(&key) { result.insert(key, items.next_value_seed(child)?); }
            else { items.next_value::<IgnoredAny>()?; }
        }
        Ok(Value::Object(result))
    }
}
fn parse_event(frame: &str) -> Result<Value> {
    let mut decoder = serde_json::Deserializer::from_str(frame);
    let input = Projection::Event.deserialize(&mut decoder).map_err(|_| error("protocol"))?;
    decoder.end().map_err(|_| error("protocol"))?;
    Ok(input)
}

/// Provenance is supplied by the trusted host, never extracted from provider JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventOrigin { Runtime, Supervisor }
impl EventOrigin { fn label(self) -> &'static str { match self { Self::Runtime => "runtime", Self::Supervisor => "supervisor" } } }
#[derive(Clone, Copy, Debug)]
pub struct Limits { pub max_frame_bytes: u64, pub max_events: u64, pub max_total_bytes: u64 }
impl Default for Limits { fn default() -> Self { Self { max_frame_bytes: 65_536, max_events: 256, max_total_bytes: 1_048_576 } } }

/// One owned attempt; no I/O, clock, retry, spawn or persistent side effects.
pub struct RuntimeObserver { descriptor: RuntimeDescriptor, request: RuntimeRequest, state: Value, sequence: u64, bytes: u64, limits: Limits }
impl RuntimeObserver {
    pub fn new(descriptor: RuntimeDescriptor, request: RuntimeRequest, limits: Limits) -> Result<Self> {
        ensure((1..=1_048_576).contains(&limits.max_frame_bytes) && (1..=4096).contains(&limits.max_events) && (1..=16_777_216).contains(&limits.max_total_bytes), "budget")?;
        ensure(descriptor.0["connection"]["id"] == request.0["connectionId"] && descriptor.0["connection"]["revision"] == request.0["connectionRevision"], "ownership")?;
        let mut owner = scope(&request.0)?; owner["connectionRevision"] = request.0["connectionRevision"].clone();
        let state = json!({"version": {"major": 1, "minor": 1}, "scope": owner, "phase": "created", "admissionObserved": false, "startObserved": false, "observation": "healthy", "fatal": false, "outcome": "unknown", "terminalConfirmed": false, "terminal": null, "requested": request.0["requested"], "effective": normalize_effective(None)?, "interrupt": {"requested": false, "acknowledged": false}, "output": {"assistant": 0, "tool": 0, "other": 0, "bytes": 0, "events": 0}, "process": {"runtime": process_state(), "helper": process_state(), "tree": {"ownership": "unknown", "source": "unknown", "termination": "unknown", "activeCount": null, "evidenceRef": null}}, "fault": null, "events": [], "contextOwnership": context_ownership()});
        Ok(Self { descriptor, request, state, sequence: 0, bytes: 0, limits })
    }
    pub fn descriptor(&self) -> &RuntimeDescriptor { &self.descriptor }
    pub fn request(&self) -> &RuntimeRequest { &self.request }
    pub fn snapshot(&self) -> Value {
        let mut result = self.state.clone(); result["budget"] = json!({"frames": self.sequence, "bytes": self.bytes, "maxEvents": self.limits.max_events, "maxFrameBytes": self.limits.max_frame_bytes, "maxTotalBytes": self.limits.max_total_bytes}); result
    }
    fn faulted(&mut self, code: &'static str, fatal: bool) {
        if self.state["fatal"] != true { self.state["fault"] = json!({"code": code}); }
        if fatal { self.state["fatal"] = json!(true); }
        if self.state["observation"] != "closed" { self.state["observation"] = json!("uncertain"); }
    }
    pub fn transport_lost(&mut self, reason: &str) -> Result<Value> {
        let code = match reason { "timeout" => "timeout", "transport-loss" => "transport-loss", "crash" => "crash", _ => return Err(error("protocol")) };
        self.faulted(code, false); Ok(self.snapshot())
    }
    pub fn close(&mut self) -> Value { self.state["observation"] = json!("closed"); self.snapshot() }
    pub fn accept(&mut self, frame: &str, origin: EventOrigin) -> Result<Value> {
        let result = self.accept_inner(frame, origin);
        if let Err(ref fault) = result { self.faulted(fault.code, true); }
        result
    }
    fn latched_error(&self) -> ContractError { error(ERRORS.iter().copied().find(|code| self.state["fault"]["code"] == *code).unwrap_or("protocol")) }
    fn accept_inner(&mut self, frame: &str, origin: EventOrigin) -> Result<Value> {
        ensure(self.state["observation"] != "closed", "transport-loss")?;
        if self.state["fatal"] == true && origin != EventOrigin::Supervisor { return Err(self.latched_error()); }
        let bytes = frame.len() as u64;
        ensure(bytes <= self.limits.max_frame_bytes && self.bytes + bytes <= self.limits.max_total_bytes && self.sequence < self.limits.max_events, "budget")?;
        let input = parse_event(frame)?; object(&input)?;
        let wire_version = version(&input["version"])?; let owner = scope(&input)?;
        for key in ["connectionId", "sessionId", "attemptId", "turnId"] { ensure(owner[key] == self.request.0[key], "ownership")?; }
        ensure(safe_int(&input["connectionRevision"], "ownership")? == self.request.0["connectionRevision"].as_u64().unwrap(), "ownership")?;
        let sequence = int(&input["sequence"], 1.0, SAFE_INTEGER, "sequence")?; ensure(sequence == self.sequence + 1, "sequence")?;
        let kind = input["type"].as_str().ok_or_else(|| error("protocol"))?; ensure(kind.encode_utf16().count() <= 100, "protocol")?;
        if self.state["fatal"] == true && !kind.starts_with("process.") { return Err(self.latched_error()); }
        ensure(!kind.starts_with("process.") || origin == EventOrigin::Supervisor, "ownership")?;
        ensure(!(kind.starts_with("attempt.") || kind.starts_with("configuration.") || kind == "output.observed") || origin == EventOrigin::Runtime, "ownership")?;
        let mut event = owner; event["version"] = wire_version.clone(); event["connectionRevision"] = self.request.0["connectionRevision"].clone(); event["sequence"] = json!(sequence); event["type"] = json!(if wire_version["minor"] == 0 && FACTUAL.contains(&kind) { "runtime.diagnostic" } else { kind }); event["origin"] = json!(origin.label());
        self.apply(&mut event, &input)?;
        self.sequence = sequence; self.bytes += bytes; self.state["events"].as_array_mut().unwrap().push(event.clone());
        Ok(event)
    }
    fn apply(&mut self, event: &mut Value, input: &Value) -> Result<()> {
        let kind = event["type"].as_str().unwrap().to_owned();
        match kind.as_str() {
            "configuration.observed" | "configuration.observation-recorded" => {
                let factual = kind == "configuration.observation-recorded";
                if !factual { ensure(self.state["phase"] == "created", "protocol")?; }
                let observed = normalize_effective(input.get("effective"))?;
                let model = if observed["model"]["state"] == "known" { Some(find_model(&self.descriptor.0, &observed["model"]["value"]).ok_or_else(|| error(if factual { "ownership" } else { "unsupported" }))?) } else { None };
                if !factual {
                    if let Some(row) = model { ensure(row["availability"] != "unavailable", "unavailable")?; }
                    for key in ["sandbox", "network", "approval"] { ensure(observed[key]["state"] != "known" || observed[key]["value"] == self.request.0["requested"][key], "policy")?; }
                    ensure(observed["model"]["state"] != "known" || self.request.0["requested"]["modelPolicy"] != "specific" || observed["model"]["value"] == self.request.0["requested"]["modelId"], "policy")?;
                    let requested = &self.request.0["requested"]["reasoning"];
                    ensure(observed["reasoning"]["state"] != "known" || [json!("auto"), json!("runtime-default")].contains(requested) || observed["reasoning"]["value"] == *requested, "policy")?;
                    if observed["reasoning"]["state"] == "known" && observed["reasoning"]["value"] != "runtime-default" { require_level(&self.descriptor, model, &observed["reasoning"]["value"])?; }
                }
                self.state["effective"] = observed.clone(); event["effective"] = observed;
            }
            "attempt.accepted" => {
                ensure(self.state["phase"] == "created" && self.state["observation"] == "healthy", "protocol")?;
                assert_admission(&self.descriptor, &self.request, Some(&self.state["effective"]))?;
                self.state["admissionObserved"] = json!(true); self.state["phase"] = json!("accepted");
            }
            "attempt.started" => { ensure(self.state["phase"] == "accepted", "protocol")?; self.state["startObserved"] = json!(true); self.state["phase"] = json!("running"); }
            "attempt.admission-observed" => {
                ensure(self.state["admissionObserved"] != true && [json!("created"), json!("running"), json!("terminal")].contains(&self.state["phase"]), "protocol")?;
                self.state["admissionObserved"] = json!(true); if self.state["terminal"].is_null() && self.state["startObserved"] != true { self.state["phase"] = json!("accepted"); }
            }
            "attempt.start-observed" => {
                ensure(self.state["startObserved"] != true && self.state["terminal"].is_null() && [json!("created"), json!("accepted")].contains(&self.state["phase"]), "protocol")?;
                self.state["startObserved"] = json!(true); self.state["phase"] = json!("running");
            }
            "attempt.interrupt-requested" => { ensure([json!("accepted"), json!("running")].contains(&self.state["phase"]) && self.state["interrupt"]["requested"] != true, "protocol")?; self.state["interrupt"]["requested"] = json!(true); }
            "attempt.interrupt-acknowledged" => { ensure(self.state["interrupt"]["requested"] == true && self.state["interrupt"]["acknowledged"] != true && self.state["terminal"].is_null(), "protocol")?; self.state["interrupt"]["acknowledged"] = json!(true); }
            "attempt.interrupt-result" => {
                let ack = input["acknowledged"].as_bool().ok_or_else(|| error("protocol"))?;
                ensure(self.state["interrupt"]["acknowledged"] != true || ack, "protocol")?;
                if ack { self.state["interrupt"]["acknowledged"] = json!(true); } event["acknowledged"] = json!(ack);
            }
            "attempt.terminal" | "attempt.terminal-observed" => {
                let factual = kind == "attempt.terminal-observed";
                let outcome = enum_value(&input["outcome"], &["completed", "failed", "interrupted"])?;
                let phases = if factual { vec![json!("created"), json!("accepted"), json!("running")] } else { vec![json!("accepted"), json!("running")] };
                ensure(self.state["terminal"].is_null() && phases.contains(&self.state["phase"]) && (factual || outcome != "completed" || self.state["phase"] == "running"), "protocol")?;
                if factual && input.get("startObserved").is_some() { ensure(input["startObserved"].is_boolean() && input["startObserved"] == self.state["startObserved"], "protocol")?; }
                let fault = if input["error"].is_null() { Value::Null } else { runtime_error(&input["error"])? };
                ensure(outcome != "completed" || fault.is_null(), "protocol")?;
                self.state["terminal"] = json!({"outcome": outcome, "error": fault}); self.state["phase"] = json!("terminal"); self.state["outcome"] = outcome.clone(); self.state["terminalConfirmed"] = json!(true);
                event["outcome"] = outcome; event["error"] = fault; if factual { event["startObserved"] = self.state["startObserved"].clone(); }
            }
            "output.observed" => {
                let channel = enum_value(&input["channel"], &["assistant", "tool", "other"])?; let name = channel.as_str().unwrap(); let bytes = safe_int(&input["bytes"], "protocol")?;
                let total = self.state["output"]["bytes"].as_u64().unwrap() + bytes; let subtotal = self.state["output"][name].as_u64().unwrap() + bytes;
                ensure(total as f64 <= SAFE_INTEGER && subtotal as f64 <= SAFE_INTEGER, "budget")?;
                let count = self.state["output"]["events"].as_u64().unwrap() + 1;
                self.state["output"]["bytes"] = json!(total); self.state["output"][name] = json!(subtotal); self.state["output"]["events"] = json!(count); event["channel"] = channel; event["bytes"] = json!(bytes);
            }
            "runtime.error" => {
                let fault = runtime_error(&input["error"])?; event["error"] = fault.clone();
                if input.get("fatal").is_some() { ensure(input["fatal"].is_boolean(), "protocol")?; }
                if input["fatal"] == true { let code = FATAL.iter().copied().find(|code| fault["code"] == *code).ok_or_else(|| error("protocol"))?; event["fatal"] = json!(true); self.faulted(code, true); }
                if self.state["terminal"].is_null() { if self.state["phase"] == "created" && fault["dispatch"] == "not-sent" { self.state["phase"] = json!("rejected"); } else { self.state["observation"] = json!("uncertain"); } }
            }
            "process.exited" => {
                let role = enum_value(&input["role"], &["runtime", "helper"])?; let name = role.as_str().unwrap();
                ensure(self.state["process"][name]["exited"] != true, "protocol")?;
                let code = if input["code"].is_null() && input.get("code").is_some() { Value::Null } else { let raw = input["code"].as_f64().ok_or_else(|| error("protocol"))?; ensure(raw.fract() == 0.0 && (-2_147_483_648.0..=4_294_967_295.0).contains(&raw), "protocol")?; json!(raw as i64) };
                let signal = default_enum(&input["signal"], "unknown", &["none", "terminated", "killed", "unknown"])?;
                self.state["process"][name]["exited"] = json!(true); self.state["process"][name]["code"] = code.clone(); self.state["process"][name]["signal"] = signal.clone();
                event["role"] = role.clone(); event["code"] = code; event["signal"] = signal;
                if name == "runtime" && self.state["terminal"].is_null() { self.state["observation"] = json!("uncertain"); }
            }
            "process.streams-closed" | "process.streams-observed" => {
                let role = enum_value(&input["role"], &["runtime", "helper"])?; let name = role.as_str().unwrap(); let incoming = streams(&input["streams"])?; let prior = &self.state["process"][name]["streams"];
                if kind == "process.streams-closed" { ensure(prior.is_null(), "protocol")?; }
                let mut merged = incoming.clone(); if kind == "process.streams-observed" { for key in ["stdin", "stdout", "stderr", "faulted"] { merged[key] = json!(incoming[key] == true || prior[key] == true); } }
                let drained = ["stdin", "stdout", "stderr"].iter().all(|key| merged[*key] == true) && merged["faulted"] != true;
                self.state["process"][name]["streams"] = merged; self.state["process"][name]["stdioClosed"] = json!(drained); event["role"] = role; event["streams"] = incoming;
            }
            "process.tree-observed" => {
                let ownership = enum_value(&input["ownership"], &["confirmed", "unknown"])?; let source = enum_value(&input["source"], &["owned-job", "owned-process-group", "unknown"])?; let termination = enum_value(&input["termination"], &["confirmed", "unknown"])?;
                let count = if input["activeCount"].is_null() && input.get("activeCount").is_some() { Value::Null } else { json!(safe_int(&input["activeCount"], "protocol")?) };
                let proof = optional_id(&input["evidenceRef"])?;
                ensure(ownership != "confirmed" || (source != "unknown" && !proof.is_null()), "ownership")?;
                ensure(termination != "confirmed" || (ownership == "confirmed" && count == 0 && self.state["process"]["runtime"]["exited"] == true), "protocol")?;
                ensure(self.state["process"]["tree"]["termination"] != "confirmed" || termination == "confirmed", "protocol")?;
                let tree = json!({"ownership": ownership, "source": source, "termination": termination, "activeCount": count, "evidenceRef": proof});
                self.state["process"]["tree"] = tree.clone(); for (key, value) in tree.as_object().unwrap() { event[key] = value.clone(); }
            }
            _ => { event["type"] = json!("runtime.diagnostic"); event["code"] = json!("unknown-event"); }
        }
        Ok(())
    }
}
