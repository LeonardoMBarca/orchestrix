//! Actual host logic without a WebView. This is not a native-window acceptance proof.
#[path = "../../../apps/desktop/src-tauri/src/editor.rs"]
#[allow(dead_code)] // The production module is linked by the Tauri host, not this isolated harness.
mod editor;
#[path = "../../../apps/desktop/src-tauri/src/host.rs"]
#[allow(dead_code)]
#[allow(unexpected_cfgs)]
// Host's nondefault native-only feature is intentionally absent from Core.
mod host;
#[path = "../../../apps/desktop/src-tauri/src/journal.rs"]
mod journal;

#[test]
fn renderer_cannot_smuggle_raw_operations_paths_sql_or_synthetic_runtime() {
    for body in [
        serde_json::json!({"type":"sql","query":"SELECT * FROM connections"}),
        serde_json::json!({"type":"submit-task","commandId":"x","preparationId":"x","authorizeRuntime":true,"authorizeChecks":true,"runtime":{"kind":"synthetic"}}),
        serde_json::json!({"type":"create-project","commandId":"x","expectedRevision":0,"id":"p","name":"Project","directory":"C:/arbitrary"}),
        serde_json::json!({"type":"save-connection","commandId":"x","label":"x","version":"0.162.0-alpha.17.2","executableGrant":"x","homeGrant":"y","apiKey":"secret"}),
    ] {
        assert!(serde_json::from_value::<host::HostRequest>(body).is_err());
    }
    assert!(serde_json::from_value::<editor::EditorRequest>(serde_json::json!({"type":"open","grant":"x","relative":"safe.txt","path":"C:/arbitrary"})).is_err());
}
