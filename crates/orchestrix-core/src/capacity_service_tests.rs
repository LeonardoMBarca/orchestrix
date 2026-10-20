//! Real synthetic metadata processes and Store guards; opt-in requires the built fixture.
use super::*;
use crate::domain::{
    CapacityGroup, CapacityGroupId, Connection, ConnectionId, LocalRef, SharedCapacity,
};
use crate::persistence::{Command, Request};
use std::{
    fs,
    time::{Duration, Instant},
};
#[test]
#[ignore = "requires the built owned runtime fixture; no authenticated reads"]
fn capacity_negative_fences_other_scope_and_membership_error_finishes_reserved_round() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("orchestrix-runtime-fixture.exe");
    assert!(executable.is_file());
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "orchestrix-capacity-service-{}-{}",
        std::process::id(),
        now_ms()
    ));
    fs::create_dir(&root).unwrap();
    let home = root.join("runtime-home");
    fs::create_dir(&home).unwrap();
    let response = home.join("synthetic-capacity-response.json");
    fs::write(
        &response,
        json!({"ordinaryUsageAllowed":true,"rateLimits":{}}).to_string(),
    )
    .unwrap();
    let clock = Arc::new(crate::runtime::ManualRefreshClock::new(now_ms() - 2000));
    let mut service = ConnectionService::with_clock(clock.clone());
    let mut store = Store::open(root.join("state.sqlite")).unwrap();
    let settle = |service: &mut ConnectionService, store: &mut Store| {
        let began = Instant::now();
        while !service.is_idle() {
            service.drain(store).unwrap();
            assert!(began.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    service
        .handle(
            &mut store,
            &ConnectionRequest::Link {
                command_id: "link".into(),
                id: "capacity".into(),
                expected_revision: None,
                name: "Synthetic capacity".into(),
                runtime: RuntimeChoice::Synthetic {
                    executable: executable.clone(),
                    home: home.clone(),
                    scenario: "connection-metadata".into(),
                },
                metadata_directory: root.clone(),
                executable_sha256: runtime_executable_sha256(&executable).unwrap(),
                allow_metadata_read: true,
            },
            false,
        )
        .unwrap();
    settle(&mut service, &mut store);
    let launch = crate::runtime::RuntimeLaunch::fixture(
        executable.clone(),
        vec!["--scenario".into(), "connection-metadata".into()],
        home.clone(),
        root.clone(),
    );
    let MetadataRead::Ready { metadata: old, .. } = service
        .metadata
        .read(
            launch.clone(),
            "capacity",
            1,
            true,
            &CancellationToken::new(),
        )
        .unwrap()
    else {
        panic!()
    };
    guard_metadata(&store, "capacity", 1, &old).unwrap();
    let scope = |key: &str| {
        let directory = root.join(key);
        fs::create_dir(&directory).unwrap();
        ConnectionRequest::DiagnoseScope {
            command_id: key.into(),
            id: "capacity".into(),
            expected_revision: 1,
            metadata_directory: directory,
            allow_metadata_read: true,
        }
    };
    clock.set(old.observed_at_ms + 100);
    fs::write(
        &response,
        json!({"ordinaryUsageAllowed":false,"rateLimits":{}}).to_string(),
    )
    .unwrap();
    service
        .handle(&mut store, &scope("negative"), false)
        .unwrap();
    settle(&mut service, &mut store);
    assert_eq!(
        guard_metadata(&store, "capacity", 1, &old).unwrap_err(),
        "connection-usage-unavailable"
    );
    clock.set(old.observed_at_ms + 200);
    fs::write(
        &response,
        json!({"ordinaryUsageAllowed":true,"rateLimits":{}}).to_string(),
    )
    .unwrap();
    service
        .handle(&mut store, &scope("positive"), false)
        .unwrap();
    settle(&mut service, &mut store);
    let MetadataRead::Ready {
        metadata: fresh, ..
    } = service
        .metadata
        .read(launch, "capacity", 1, true, &CancellationToken::new())
        .unwrap()
    else {
        panic!()
    };
    guard_metadata(&store, "capacity", 1, &fresh).unwrap();
    assert!(!store.connection_identity_changed("capacity", 1).unwrap());
    let execute = |store: &mut Store, key: String, command| {
        let revision = store.load().unwrap().revision();
        store
            .execute(&Request {
                scope: LocalRef::new("test").unwrap(),
                key: LocalRef::new(key).unwrap(),
                expected_revision: revision,
                command,
            })
            .unwrap();
    };
    execute(
        &mut store,
        "domain-connection".into(),
        Command::AddConnection(Connection::new(fresh.descriptor.clone()).unwrap()),
    );
    let observed = crate::worker::observed(&fresh, "capacity-worker-snapshot").unwrap();
    let diagnostic =
        crate::worker::diagnostic_input(&store, &fresh, &observed, "capacity-worker-snapshot")
            .unwrap();
    assert_eq!(
        diagnostic.values["capacity"],
        serde_json::to_value(&fresh.capacity).unwrap()
    );
    execute(
        &mut store,
        "snapshot".into(),
        Command::Records(crate::persistence_records::RecordsCommand::Diagnostic(
            diagnostic,
        )),
    );
    assert_eq!(
        store
            .load()
            .unwrap()
            .records()
            .diagnostic("capacity-worker-snapshot")
            .unwrap()
            .values["capacity"]["ordinaryUsage"]["value"],
        true
    );
    for index in 0..65 {
        let mut group = CapacityGroup::new(
            CapacityGroupId::new(format!("group-{index}")).unwrap(),
            SharedCapacity::Unknown,
        );
        group
            .add_member(ConnectionId::new("capacity").unwrap())
            .unwrap();
        execute(
            &mut store,
            format!("group-{index}"),
            Command::SaveCapacityGroup(group),
        );
    }
    clock.set(old.observed_at_ms + 300);
    let started = service
        .handle(&mut store, &scope("too-many-groups"), false)
        .unwrap();
    settle(&mut service, &mut store);
    let terminal = store
        .connection_diagnostics_run("capacity", started["diagnosticId"].as_str())
        .unwrap();
    assert_eq!(terminal["latest"]["state"], "failed");
    assert_eq!(
        terminal["latest"]["errorCode"],
        "capacity-group-evidence-invalid"
    );
    assert_eq!(
        terminal["latest"]["capacity"]["ordinaryUsage"]["value"],
        true
    );
    assert_eq!(
        store.connection_diagnostic_storage("capacity").unwrap()["reservedBytes"],
        0
    );
    assert!(!home.join("dispatch-count").exists());
}
