//! Offline host-bundle consumer used to verify the adapter → Core boundary.
use orchestrix_core::runtime_contract::{normalize_descriptor, normalize_request, EventOrigin, Limits, RuntimeObserver, INPUT_PROFILE_ID, INPUT_PROFILE_REVISION};
use serde_json::{json, Value};
use std::io::{self, Read};

fn observe(input: Value) -> Result<Value, &'static str> {
    // Host negotiation is mandatory; semantic wire minors cannot downgrade it.
    let profile = json!({"id": INPUT_PROFILE_ID, "revision": INPUT_PROFILE_REVISION});
    if input["inputProfile"]["id"] != profile["id"] || input["inputProfile"]["revision"].as_f64() != Some(f64::from(INPUT_PROFILE_REVISION)) { return Err("version"); }
    let descriptor = normalize_descriptor(&input["descriptor"]).map_err(|e| e.code())?;
    let request = normalize_request(&input["request"]).map_err(|e| e.code())?;
    let mut observer = RuntimeObserver::new(descriptor, request, Limits::default()).map_err(|e| e.code())?;
    let frames = input["frames"].as_array().ok_or("protocol")?;
    if frames.len() > 4096 { return Err("budget"); }
    for envelope in frames {
        // The surrounding bundle is supplied by the trusted host/test driver.
        // This is not an endpoint accepting provider-supplied origin assertions.
        let origin = match envelope["origin"].as_str() {
            Some("runtime") => EventOrigin::Runtime,
            Some("supervisor") => EventOrigin::Supervisor,
            _ => return Err("ownership"),
        };
        let frame = envelope["frame"].as_str().ok_or("protocol")?;
        if let Err(error) = observer.accept(frame, origin) {
            return Ok(json!({"status": "rejected", "code": error.code(), "snapshot": observer.snapshot()}));
        }
    }
    Ok(json!({"status": "observed", "inputProfile": profile, "descriptor": observer.descriptor().as_json(), "request": observer.request().as_json(), "snapshot": observer.snapshot()}))
}

fn main() {
    const MAX_INPUT: u64 = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    let result = if io::stdin().take(MAX_INPUT + 1).read_to_end(&mut bytes).is_err() {
        Err("protocol")
    } else if bytes.len() as u64 > MAX_INPUT {
        Err("budget")
    } else {
        serde_json::from_slice(&bytes).map_err(|_| "protocol").and_then(observe)
    };
    let output = match result {
        Ok(value) => value,
        Err(code) => json!({"status": "rejected", "code": code}),
    };
    println!("{output}");
    if output["status"] == "rejected" { std::process::exit(1); }
}
