//! Host + real daemon + Git/SQLite; no dialog automation or provider access.
use super::*;

#[test]
#[ignore = "requires explicitly selected current Core and synthetic runtime binaries; no provider access"]
fn repository_preflight_requires_native_grants_and_blocks_before_metadata() {
    let core =
        PathBuf::from(std::env::var_os("ORCHESTRIX_TEST_CORE_BIN").expect("current Core binary"))
            .canonicalize()
            .unwrap();
    let runtime = PathBuf::from(
        std::env::var_os("ORCHESTRIX_TEST_RUNTIME_BIN").expect("owned synthetic runtime"),
    )
    .canonicalize()
    .unwrap();
    let root = isolated_directory();
    let fixture = DaemonFixture::start(&root.join("preflight"), &core, &runtime);
    let config = fixture.host.handle(HostRequest::TestFixture).unwrap();
    fixture.link("local-connection");
    let mut draft: TaskDraft = serde_json::from_value(config["draft"].clone()).unwrap();
    draft.conversation_id = "preflight-conversation".into();
    draft.connection_id = "local-connection".into();
    draft.connection_revision = Some(1);
    draft.fallback = "compatible".into();
    draft.allow_metadata_read = true;
    fixture
        .host
        .handle(HostRequest::SendMessage {
            command_id: id("message"),
            expected_revision: fixture.bootstrap()["revision"].as_u64().unwrap(),
            conversation_id: draft.conversation_id.clone(),
            project_id: draft.project_id.clone(),
            content: "Local admission proof, explicitly synthetic".into(),
        })
        .unwrap();
    let reads = fixture.metadata_reads();
    assert!(
        fixture
            .host
            .handle(HostRequest::InspectRepository {
                directory_grant: fixture.config.repository.to_string_lossy().into_owned()
            })
            .is_err(),
        "a path string is not a native grant"
    );
    let report = fixture
        .host
        .handle(HostRequest::InspectRepository {
            directory_grant: draft.repository_grant.clone(),
        })
        .unwrap();
    assert_eq!(report["compatible"], true);
    assert_eq!(report["modelCalled"], false);
    assert_eq!(
        fixture.metadata_reads(),
        reads,
        "inspection must not read account metadata"
    );
    // An edit after the displayed inspection invalidates its usefulness for
    // preparation. Host must freshly inspect before the authorized metadata read.
    fs::write(
        fixture.config.repository.join("fixture.txt"),
        b"binary\0changed",
    )
    .unwrap();
    let error = fixture.host.prepare(draft.clone()).unwrap_err();
    assert!(
        error.starts_with("repository-preflight-blocked:"),
        "{error}"
    );
    assert_eq!(
        fixture.metadata_reads(),
        reads,
        "blocked preparation sends no metadata request"
    );
    assert!(!fixture.config.home.join("dispatch-count").exists());
    fs::write(fixture.config.repository.join("fixture.txt"), b"before\n").unwrap();
    let prepared = fixture.host.prepare(draft).unwrap();
    assert_eq!(prepared["task"]["connection_id"], "local-connection");
    assert_eq!(prepared["task"]["connection_revision"], 1);
    assert_eq!(
        fixture.metadata_reads(),
        reads + 1,
        "eligible preparation still validates its separate metadata scope"
    );
    assert!(!fixture.config.home.join("dispatch-count").exists());
    fs::write(root.join("preflight-evidence.json"),serde_json::to_vec_pretty(&json!({"report":report,"error":error,"prepared":prepared,"providerRead":false,"dispatches":0})).unwrap()).unwrap();
    fixture.stop();
}
