//! Deterministic scheduling without RPC, sleep, provider account or inference.
use orchestrix_core::runtime::{
    CodexMetadataService, ManualRefreshClock, MetadataRefreshCoordinator, MetadataRegistration,
    MetadataScheduleBounds, RuntimeLaunch, refresh::RefreshBounds,
};
use std::sync::Arc;

fn coordinator(clock: Arc<ManualRefreshClock>) -> MetadataRefreshCoordinator {
    let service = CodexMetadataService::new(clock.clone(), RefreshBounds::default()).unwrap();
    MetadataRefreshCoordinator::new(
        service,
        clock,
        MetadataScheduleBounds {
            max_connections: 64,
            max_reads_per_tick: 1,
            max_in_flight: 1,
            minimum_interval_ms: 60_000,
        },
    )
    .unwrap()
}
fn registration(id: &str) -> MetadataRegistration {
    let exe = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let directory = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    MetadataRegistration {
        connection_id: id.into(),
        revision: 1,
        launch: RuntimeLaunch::fixture(exe, Vec::new(), directory.clone(), directory),
        authorized: true,
        enabled: true,
        interval_ms: 60_000,
    }
}
#[test]
fn no_early_read_two_consumers_share_one_claim_and_no_catchup() {
    let clock = Arc::new(ManualRefreshClock::new(100));
    let coordinator = coordinator(clock.clone());
    coordinator
        .register_deferred(registration("first"))
        .unwrap();
    assert!(coordinator.claim_due().unwrap().is_none());
    clock.set(60_099);
    assert!(coordinator.claim_due().unwrap().is_none());
    clock.set(60_100);
    let claim = coordinator.claim_due().unwrap().unwrap();
    assert!(coordinator.clone().claim_due().unwrap().is_none());
    coordinator.complete_claim(&claim, true, None, false);
    assert_eq!(coordinator.registration_status("first").unwrap().0, 120_100);
    clock.set(9_000_000);
    assert!(coordinator.claim_due().unwrap().is_none());
    assert_eq!(
        coordinator.registration_status("first").unwrap().0,
        9_060_000
    );
    assert!(coordinator.claim_due().unwrap().is_none());
}
#[test]
fn restart_and_backwards_clock_schedule_one_future_opportunity() {
    let clock = Arc::new(ManualRefreshClock::new(100));
    let old = coordinator(clock.clone());
    old.register_deferred(registration("one")).unwrap();
    clock.set(9_000_000);
    let restarted = coordinator(clock.clone());
    restarted.register_deferred(registration("one")).unwrap();
    assert!(restarted.claim_due().unwrap().is_none());
    assert_eq!(restarted.registration_status("one").unwrap().0, 9_060_000);
    clock.set(10);
    assert!(restarted.claim_due().unwrap().is_none());
    assert_eq!(restarted.registration_status("one").unwrap().0, 60_010);
}
#[test]
fn revoke_or_new_revision_rejects_late_completion_without_resurrection() {
    let clock = Arc::new(ManualRefreshClock::new(100));
    let coordinator = coordinator(clock.clone());
    coordinator.register_deferred(registration("one")).unwrap();
    clock.set(60_100);
    let old = coordinator.claim_due().unwrap().unwrap();
    coordinator.unregister("one");
    coordinator.complete_claim(&old, true, None, false);
    assert!(coordinator.registration_status("one").is_none());
    let mut revised = registration("one");
    revised.revision = 2;
    coordinator.register_deferred(revised).unwrap();
    let date = coordinator.registration_status("one").unwrap().0;
    coordinator.complete_claim(&old, false, Some(u64::MAX), true);
    let state = coordinator.registration_status("one").unwrap();
    assert_eq!(state, (date, false, false, 0));
}
#[test]
fn global_serial_bound_partial_failures_backoff_and_pause_are_distinct() {
    let clock = Arc::new(ManualRefreshClock::new(100));
    let coordinator = coordinator(clock.clone());
    coordinator
        .register_deferred(registration("first"))
        .unwrap();
    coordinator
        .register_deferred(registration("second"))
        .unwrap();
    clock.set(60_100);
    let first = coordinator.claim_due().unwrap().unwrap();
    assert_eq!(first.connection_id, "first");
    assert!(coordinator.claim_due().unwrap().is_none());
    coordinator.complete_claim(&first, false, Some(90_000), false);
    assert_eq!(
        coordinator.registration_status("first").unwrap(),
        (120_100, false, false, 1)
    );
    let second = coordinator.claim_due().unwrap().unwrap();
    coordinator.complete_claim(&second, false, None, true);
    assert!(coordinator.registration_status("second").unwrap().1);
    assert!(coordinator.claim_due().unwrap().is_none());
    clock.set(120_100);
    let first = coordinator.claim_due().unwrap().unwrap();
    coordinator.complete_claim(&first, true, None, false);
    assert_eq!(
        coordinator.registration_status("first").unwrap(),
        (180_100, false, false, 0)
    );
}
