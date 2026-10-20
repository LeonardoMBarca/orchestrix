//! Opt-in mass preparation uses valid Store transitions and an owned synthetic runtime only.
use super::*;
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn oversized_terminal_is_an_explicit_failure_without_erasing_identity_fence() {
    let mut d: ConnectionDiagnostic = serde_json::from_value(json!({
        "schemaVersion":1,"id":"oversized","connectionId":"connection-one","connectionRevision":1,
        "registrationRevision":1,"daemonGeneration":1,"synthetic":true,"state":"complete",
        "startedAtMs":100,"finishedAtMs":200,"observedAtMs":100,"validUntilMs":60100,
        "nextAllowedAtMs":null,"errorCode":null,"identityAssurance":"synthetic",
        "observationFingerprint":"a".repeat(64),"identityChanged":true,
        "descriptor":{"oversizedInput":"x".repeat(MAX_DIAGNOSTIC_BODY_BYTES)},
        "catalogLabels":[],"controlObservations":[],"structuredResultSupport":true,
        "complete":true,"cached":false,"purpose":"registration","scopeKey":"b".repeat(64)
    }))
    .unwrap();
    bound_terminal_evidence(&mut d);
    assert_eq!(d.state, "failed");
    assert_eq!(
        d.error_code.as_deref(),
        Some("metadata-evidence-size-limit")
    );
    assert!(d.identity_changed);
    assert_eq!(d.observed_at_ms, Some(100));
    assert_eq!(d.valid_until_ms, Some(60100));
    assert!(d.descriptor.is_none());
    assert!(!d.complete);
    d.validate().unwrap();
    assert!(serde_json::to_vec(&d).unwrap().len() < MAX_DIAGNOSTIC_BODY_BYTES);
    let bytes = serde_json::to_vec(&d).unwrap();
    bound_terminal_evidence(&mut d);
    assert_eq!(serde_json::to_vec(&d).unwrap(), bytes);
}

#[test]
#[ignore = "explicit owned native fixture preparation; no real provider reads"]
fn prepare_native_retention_fixture() {
    let config_path = PathBuf::from(
        std::env::var_os("ORCHESTRIX_RETENTION_FIXTURE_CONFIG").expect("owned fixture config"),
    );
    let config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    let root = PathBuf::from(config["root"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    let allowed = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/native-desktop")
        .canonicalize()
        .unwrap();
    assert!(
        root.starts_with(allowed)
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("qa-")
    );
    assert_eq!(
        config_path.canonicalize().unwrap(),
        root.join("fixture.json")
    );
    let executable = PathBuf::from(config["executable"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(
        executable.file_name().unwrap(),
        "orchestrix-runtime-fixture.exe"
    );
    let home = PathBuf::from(config["home"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert!(home.starts_with(&root));
    let directory = root.join("desktop-client");
    fs::create_dir_all(&directory).unwrap();
    let database = directory.join("orchestrix.sqlite");
    assert!(
        !database.exists(),
        "never seed an existing native fixture/database"
    );
    let mut store = Store::open(&database).unwrap();
    let clock = Arc::new(crate::runtime::ManualRefreshClock::new(
        now_ms().saturating_sub(3_600_000),
    ));
    let mut service = ConnectionService::with_clock(clock);
    let started = Instant::now();
    let link_request = ConnectionRequest::Link {
        command_id: "retention-fixture-link".into(),
        id: "native-fixture-account".into(),
        expected_revision: None,
        name: "Synthetic diagnostic history".into(),
        runtime: RuntimeChoice::Synthetic {
            executable: executable.clone(),
            home: home.clone(),
            scenario: "connection-metadata".into(),
        },
        metadata_directory: directory,
        executable_sha256: runtime_executable_sha256(&executable).unwrap(),
        allow_metadata_read: true,
    };
    let link_receipt = service.handle(&mut store, &link_request, false).unwrap();
    while !service.is_idle() {
        service.tick(&mut store).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "synthetic metadata deadline"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
    let first = store
        .connection_diagnostics("native-fixture-account")
        .unwrap();
    let first_id = first["latest"]["id"].as_str().unwrap();
    let source = store
        .connection_diagnostic_exact_raw("native-fixture-account", first_id)
        .unwrap();
    assert_eq!(source.state, "complete");
    assert!(source.synthetic);
    let source_detail = store
        .connection_diagnostic_observation("native-fixture-account", first_id, Some(2))
        .unwrap();
    let seed_started = Instant::now();
    for i in 0..280 {
        let mut running = source.clone();
        running.id = format!("retention-cache-{i:04}");
        running.state = "running".into();
        running.finished_at_ms = None;
        running.observed_at_ms = None;
        running.valid_until_ms = None;
        running.identity_assurance = "unknown".into();
        running.observation_fingerprint = None;
        running.descriptor = None;
        running.catalog_labels.clear();
        running.control_observations.clear();
        running.structured_result_support = None;
        running.complete = false;
        running.cached = false;
        running.scope_key = None;
        store.connection_begin_diagnostic(&running, None).unwrap();
        let mut terminal = source.clone();
        terminal.id = running.id;
        terminal.cached = true;
        store.connection_record_diagnostic(&terminal).unwrap();
    }
    let storage = store
        .connection_diagnostic_storage("native-fixture-account")
        .unwrap();
    assert_eq!(storage["totalRows"], 562);
    assert!(storage["operationalRows"].as_u64().unwrap() <= 512);
    assert!(storage["archivedRows"].as_u64().unwrap() > 0);
    let old = store
        .connection_diagnostic_observation("native-fixture-account", first_id, Some(2))
        .unwrap();
    assert_eq!(source_detail["digest"], old["digest"]);
    assert_eq!(source_detail["diagnostic"], old["diagnostic"]);
    assert_eq!(old["archived"], true);
    let receipt_replay = service.handle(&mut store, &link_request, false).unwrap();
    assert_eq!(receipt_replay, link_receipt);
    assert!(
        service.is_idle(),
        "an old receipt never redispatches metadata"
    );
    let reads_before_reopen = fs::read_to_string(home.join("metadata-reads")).unwrap();
    drop(service);
    drop(store);
    let store = Store::open(&database).unwrap();
    assert_eq!(
        store
            .connection_diagnostic_observation("native-fixture-account", first_id, Some(2))
            .unwrap(),
        old
    );
    assert_eq!(
        fs::read_to_string(home.join("metadata-reads")).unwrap(),
        reads_before_reopen
    );
    let evidence = json!({"synthetic":true,"database":database,"oldestDiagnosticId":first_id,"oldest":old,"storage":storage,"originalLinkReceipt":link_receipt,"replayedLinkReceipt":receipt_replay,"metadataPreparationMs":seed_started.duration_since(started).as_millis(),"validTransitionSeedMs":seed_started.elapsed().as_millis(),"realProviderReads":0,"turns":0});
    fs::write(
        root.join("retention-seed-evidence.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string(&evidence).unwrap());
}
