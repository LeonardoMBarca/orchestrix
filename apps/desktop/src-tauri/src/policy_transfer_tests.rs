//! Host/IPC acceptance; these tests do not drive a native dialog or certify its UI.
#![cfg(all(windows, feature = "test-fixtures"))]
use super::*;
use orchestrix_core::{
    connection_refresh::{RefreshMode, RefreshSettings},
    policy::{Fallback, Strategy},
    policy_portability::{ConnectionMapping, ImportMappings, ModelMapping, PortableBundle},
};
use std::{
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    time::Instant,
};

fn isolated_directory() -> PathBuf {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(id("orchestrix-host-policy"));
    fs::create_dir(&root).unwrap();
    root
}

struct DaemonFixture {
    host: Host,
    child: Child,
    config: FixtureConfig,
}
impl DaemonFixture {
    fn start(root: &Path, core: &Path, runtime: &Path) -> Self {
        fs::create_dir(root).unwrap();
        let home = root.join("runtime-home");
        let repository = root.join("repository");
        let worktrees = root.join("worktrees");
        for path in [&home, &repository, &worktrees] {
            fs::create_dir(path).unwrap();
        }
        let empty_config = root.join("empty-git-config");
        fs::write(&empty_config, b"").unwrap();
        // Git for Windows accepts the extended cwd, but its config-file reader
        // rejects the Win32 extended prefix. Use the same own empty file's DOS path.
        let git_config = empty_config.to_string_lossy().into_owned();
        let git_config = git_config.strip_prefix(r"\\?\").unwrap_or(&git_config);
        let git = |arguments: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args([
                    "-c",
                    "core.autocrlf=false",
                    "-c",
                    "core.hooksPath=",
                    "-c",
                    "user.name=Orchestrix Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                ])
                .args(arguments)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", git_config)
                .env("GIT_TERMINAL_PROMPT", "0")
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "own fixture Git command failed {arguments:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        fs::write(repository.join("fixture.txt"), b"before\n").unwrap();
        git(&["add", "--", "fixture.txt"]);
        git(&["commit", "--quiet", "-m", "owned fixture"]);
        let mut host = Host::new(root.join("host-data")).unwrap();
        let config = FixtureConfig {
            root: root.to_owned(),
            executable: runtime.to_owned(),
            home: home.canonicalize().unwrap(),
            repository: repository.canonicalize().unwrap(),
            worktrees: worktrees.canonicalize().unwrap(),
            review_scenario: None,
            worker_scenario: None,
            connections_onboarding: true,
        };
        host.fixture = Some(config.clone());
        let child = Command::new(core)
            .args(["daemon", "--database"])
            .arg(&host.database)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("daemon.log")).unwrap())
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap();
        let mut value = Self {
            host,
            child,
            config,
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        while value.host.call(Operation::Snapshot).is_err() {
            assert!(
                value.child.try_wait().unwrap().is_none(),
                "own daemon exited before readiness"
            );
            assert!(Instant::now() < deadline, "own daemon readiness deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
        value
    }
    fn metadata_reads(&self) -> u64 {
        fs::read_to_string(self.config.home.join("metadata-reads"))
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }
    fn bootstrap(&self) -> Value {
        self.host.handle(HostRequest::Bootstrap).unwrap()
    }
    fn link(&self, connection: &str) -> Value {
        let (executable_ref, home_ref) = {
            let mut editor = self.host.editor.lock().unwrap();
            (
                editor
                    .grant_dialog_selection(self.config.executable.clone())
                    .unwrap()
                    .id,
                editor
                    .grant_dialog_selection(self.config.home.clone())
                    .unwrap()
                    .id,
            )
        };
        self.host
            .handle(HostRequest::LinkConnection {
                command_id: id("link"),
                id: Some(connection.into()),
                expected_revision: None,
                name: "Synthetic portable connection".into(),
                version: "synthetic-rust-1".into(),
                executable_ref,
                home_ref,
                executable_sha256: connection_discovery::binary_hash(&self.config.executable)
                    .unwrap(),
                allow_metadata_read: true,
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let report = self
                .host
                .connections(ConnectionRequest::Diagnostics {
                    id: connection.into(),
                    diagnostic_id: None,
                })
                .unwrap();
            if report["latest"]["state"] == "complete" {
                assert_eq!(report["latest"]["provenance"], "synthetic");
                return report;
            }
            assert!(
                report["latest"]["state"] == "running",
                "metadata report: {}",
                report["latest"]["state"]
            );
            assert!(Instant::now() < deadline, "own metadata deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn stop(mut self) {
        self.host
            .call(Operation::Shutdown {
                command_id: id("stop"),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(exit) = self.child.try_wait().unwrap() {
                assert!(exit.success());
                break;
            }
            assert!(Instant::now() < deadline, "own daemon shutdown deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for DaemonFixture {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.host.call(Operation::Shutdown {
                command_id: id("cleanup"),
            });
            let deadline = Instant::now() + Duration::from_secs(20);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            // Only the exact Child created above, only on failed-test cleanup.
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected, current Core and synthetic runtime binaries; no provider access"]
fn real_daemon_policy_transfer_remaps_then_prepares_without_execution_or_imported_consent() {
    let core = PathBuf::from(
        std::env::var_os("ORCHESTRIX_TEST_CORE_BIN")
            .expect("set the current owned Core executable"),
    )
    .canonicalize()
    .unwrap();
    let runtime = PathBuf::from(
        std::env::var_os("ORCHESTRIX_TEST_RUNTIME_BIN")
            .expect("set the owned synthetic runtime executable"),
    )
    .canonicalize()
    .unwrap();
    assert_eq!(
        runtime.file_name().and_then(|s| s.to_str()),
        Some("orchestrix-runtime-fixture.exe")
    );
    assert!(
        core.parent()
            .unwrap()
            .join("orchestrix-supervisor.exe")
            .is_file(),
        "owned supervision sidecar required"
    );
    let root = isolated_directory();
    eprintln!(
        "policy-transfer-fixture={} core-sha256={} runtime-sha256={}",
        root.display(),
        connection_discovery::binary_hash(&core).unwrap(),
        connection_discovery::binary_hash(&runtime).unwrap()
    );
    let source = DaemonFixture::start(&root.join("source"), &core, &runtime);
    let destination = DaemonFixture::start(&root.join("destination"), &core, &runtime);
    source.host.handle(HostRequest::TestFixture).unwrap();
    let destination_fixture = destination.host.handle(HostRequest::TestFixture).unwrap();
    let source_report = source.link("source-connection");
    let destination_report = destination.link("destination-connection");
    let source_model = source_report["catalog"]["models"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let destination_model = destination_report["catalog"]["models"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut settings: Settings =
        serde_json::from_value(source.bootstrap()["settings"].clone()).unwrap();
    settings.theme = "horizon".into();
    settings.language = "es".into();
    settings.system_instructions = "PRIVATE SOURCE STYLE".into();
    settings.policy.mode_defaults.strategy = Some(Strategy::Efficiency);
    settings.policy.mode_defaults.connection_id = Some("source-connection".into());
    settings.policy.mode_defaults.model_id = Some(source_model);
    settings.policy.mode_defaults.fallback = Some(Fallback::Block);
    source
        .host
        .handle(HostRequest::SaveSettings {
            command_id: id("source-settings"),
            expected_revision: source.bootstrap()["revision"].as_u64().unwrap(),
            settings: Box::new(settings),
        })
        .unwrap();
    let recurring = RefreshSettings {
        mode: RefreshMode::Automatic,
        interval_ms: 1_000,
        recurring_consent: true,
        ..RefreshSettings::default()
    };
    source
        .host
        .handle(HostRequest::ConfigureConnectionRefresh {
            command_id: id("source-recurring"),
            id: "source-connection".into(),
            expected_revision: 1,
            expected_refresh_revision: 0,
            settings: recurring,
        })
        .unwrap();
    assert_eq!(
        source.metadata_reads(),
        1,
        "configuration must not issue a new read"
    );
    let refresh = source
        .host
        .handle(HostRequest::ConnectionRefreshStatus {
            id: Some("source-connection".into()),
        })
        .unwrap();
    assert_eq!(refresh["refresh"][0]["requested"]["intervalMs"], 1_000);
    assert_eq!(refresh["refresh"][0]["effective"]["intervalMs"], 60_000);
    // One real daemon cadence smoke complements the controlled-clock matrix.
    // It does not use a UI timer, alter the clock or grant any provider access.
    let until = Instant::now() + Duration::from_secs(80);
    loop {
        let report = source
            .host
            .connections(ConnectionRequest::Diagnostics {
                id: "source-connection".into(),
                diagnostic_id: None,
            })
            .unwrap();
        if source.metadata_reads() == 2 && report["latest"]["state"] == "complete" {
            assert_ne!(report["latest"]["id"], source_report["latest"]["id"]);
            assert!(
                report["latest"]["observedAtMs"].as_u64().unwrap()
                    > source_report["latest"]["observedAtMs"].as_u64().unwrap()
            );
            break;
        }
        assert!(
            Instant::now() < until,
            "one bounded daemon periodic opportunity"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    source
        .host
        .handle(HostRequest::ConfigureConnectionRefresh {
            command_id: id("stop-source-recurring"),
            id: "source-connection".into(),
            expected_revision: 1,
            expected_refresh_revision: 1,
            settings: RefreshSettings::default(),
        })
        .unwrap();
    // The destination's prior observation has expired during the real cadence.
    // Only this separate explicit read may refresh it; importing never does.
    destination
        .host
        .handle(HostRequest::DiagnoseConnection {
            command_id: id("destination-explicit-recheck"),
            id: "destination-connection".into(),
            expected_revision: 1,
            allow_metadata_read: true,
        })
        .unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let report = destination
            .host
            .connections(ConnectionRequest::Diagnostics {
                id: "destination-connection".into(),
                diagnostic_id: None,
            })
            .unwrap();
        if report["latest"]["state"] == "complete" {
            break;
        }
        assert!(
            Instant::now() < until,
            "explicit destination metadata completion"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let exported = source
        .host
        .handle(HostRequest::PreparePolicyExport)
        .unwrap();
    let text = exported["document"].as_str().unwrap();
    for forbidden in [
        "PRIVATE SOURCE STYLE",
        "source-connection",
        "recurringConsent",
        "authStore",
        "metadataDirectory",
        "executableSha256",
    ] {
        assert!(
            !text.contains(forbidden),
            "forbidden export field {forbidden}"
        );
    }
    assert!(!text.contains(&source.config.home.to_string_lossy().to_string()));
    let bundle: PortableBundle = serde_json::from_str(text).unwrap();
    assert_eq!(
        (
            bundle.connection_references.len(),
            bundle.model_references.len()
        ),
        (1, 1)
    );
    let file = root.join("transported-policy.json");
    source
        .host
        .write_policy_export(exported["exportRef"].as_str().unwrap(), &file)
        .unwrap();
    let before = destination.bootstrap();
    let reads = destination.metadata_reads();
    assert!(
        destination
            .host
            .handle(HostRequest::PreviewPolicyImport {
                import_ref: file.to_string_lossy().into(),
                mappings: ImportMappings::default()
            })
            .is_err(),
        "a renderer path is not a selection grant"
    );
    let selected = destination.host.select_policy_import(&file).unwrap();
    assert_eq!(
        selected["preview"]["canApply"], false,
        "same model/name must not resolve automatically"
    );
    let cancelled_reference = selected["importRef"].as_str().unwrap().to_owned();
    destination
        .host
        .handle(HostRequest::ReleasePolicyImport {
            import_ref: cancelled_reference.clone(),
        })
        .unwrap();
    assert!(
        destination
            .host
            .handle(HostRequest::PreviewPolicyImport {
                import_ref: cancelled_reference,
                mappings: ImportMappings::default()
            })
            .is_err()
    );
    assert_eq!(destination.bootstrap()["settings"], before["settings"]);
    let selected = destination.host.select_policy_import(&file).unwrap();
    let reference = selected["importRef"].as_str().unwrap().to_owned();
    let mappings = ImportMappings {
        connections: vec![ConnectionMapping {
            alias: bundle.connection_references[0].alias.clone(),
            connection_id: "destination-connection".into(),
            binding_revision: 1,
        }],
        models: vec![ModelMapping {
            alias: bundle.model_references[0].alias.clone(),
            connection_alias: bundle.connection_references[0].alias.clone(),
            model_id: destination_model.clone(),
        }],
        ..Default::default()
    };
    let blocked_preview = destination
        .host
        .handle(HostRequest::PreviewPolicyImport {
            import_ref: reference.clone(),
            mappings: mappings.clone(),
        })
        .unwrap();
    assert_eq!(blocked_preview["canApply"], false);
    assert!(
        blocked_preview["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "portable-model-availability-unconfirmed")
    );
    assert_eq!(destination.bootstrap()["settings"], before["settings"]);
    destination
        .host
        .handle(HostRequest::ReleasePolicyImport {
            import_ref: reference,
        })
        .unwrap();

    // A catalog is not entitlement. Only an explicit soft Compatible preference
    // can be transported while availability stays unknown; Block stays blocked.
    let mut settings: Settings =
        serde_json::from_value(source.bootstrap()["settings"].clone()).unwrap();
    settings.policy.mode_defaults.fallback = Some(Fallback::Compatible);
    settings.policy.revision += 1;
    source
        .host
        .handle(HostRequest::SaveSettings {
            command_id: id("source-compatible-preference"),
            expected_revision: source.bootstrap()["revision"].as_u64().unwrap(),
            settings: Box::new(settings),
        })
        .unwrap();
    let compatible_export = source
        .host
        .handle(HostRequest::PreparePolicyExport)
        .unwrap();
    let compatible_file = root.join("transported-compatible-policy.json");
    source
        .host
        .write_policy_export(
            compatible_export["exportRef"].as_str().unwrap(),
            &compatible_file,
        )
        .unwrap();
    let selected = destination
        .host
        .select_policy_import(&compatible_file)
        .unwrap();
    let reference = selected["importRef"].as_str().unwrap().to_owned();
    let preview = destination
        .host
        .handle(HostRequest::PreviewPolicyImport {
            import_ref: reference.clone(),
            mappings: mappings.clone(),
        })
        .unwrap();
    assert_eq!(
        preview["canApply"], true,
        "preview issues {}",
        preview["issues"]
    );
    let apply = |confirm: bool| HostRequest::ApplyPolicyImport {
        command_id: id("import"),
        expected_revision: preview["expectedRevision"].as_u64().unwrap(),
        import_ref: reference.clone(),
        mappings: mappings.clone(),
        expected_preview_digest: preview["previewDigest"].as_str().unwrap().into(),
        confirm,
    };
    assert_eq!(
        destination.host.handle(apply(false)).unwrap_err(),
        "policy-import-confirmation-required"
    );
    assert_eq!(destination.bootstrap()["settings"], before["settings"]);
    let original_apply = apply(true);
    let applied = destination.host.handle(original_apply.clone()).unwrap();
    assert_eq!(applied["replayed"], false);
    let committed_revision = destination.bootstrap()["revision"].clone();
    destination
        .host
        .handle(HostRequest::ReleasePolicyImport {
            import_ref: reference.clone(),
        })
        .unwrap();
    let replayed = destination.host.handle(original_apply.clone()).unwrap();
    assert_eq!(replayed["replayed"], true);
    assert_eq!(destination.bootstrap()["revision"], committed_revision);
    let mut reopened = Host::new(destination.host.directory.clone()).unwrap();
    reopened.fixture = Some(destination.config.clone());
    let replayed = reopened.handle(original_apply).unwrap();
    assert_eq!(replayed["replayed"], true);
    assert_eq!(destination.bootstrap()["revision"], committed_revision);
    assert_eq!(
        destination.metadata_reads(),
        reads,
        "select/preview/import must not issue metadata reads"
    );
    let imported: Settings =
        serde_json::from_value(destination.bootstrap()["settings"].clone()).unwrap();
    assert_eq!(imported.theme, "horizon");
    assert_eq!(imported.language, "es");
    assert_eq!(
        imported.policy.mode_defaults.strategy,
        Some(Strategy::Efficiency)
    );
    assert_eq!(
        imported.policy.mode_defaults.connection_id.as_deref(),
        Some("destination-connection")
    );
    assert_eq!(
        imported.policy.mode_defaults.model_id.as_deref(),
        Some(destination_model.as_str())
    );
    assert!(imported.system_instructions.is_empty());
    assert!(imported.policy.mode_requirements.subscription_only);
    let refresh = destination
        .host
        .handle(HostRequest::ConnectionRefreshStatus {
            id: Some("destination-connection".into()),
        })
        .unwrap();
    assert_eq!(refresh["refresh"][0]["refreshRevision"], 0);
    assert_eq!(refresh["refresh"][0]["recurringAuthorized"], false);
    let mut draft: TaskDraft =
        serde_json::from_value(destination_fixture["draft"].clone()).unwrap();
    draft.conversation_id = "destination-conversation".into();
    draft.connection_id = "destination-connection".into();
    draft.connection_revision = Some(1);
    draft.model = destination_model;
    draft.strategy = "efficiency".into();
    draft.fallback = "compatible".into();
    draft.allow_metadata_read = true;
    let revision = destination.bootstrap()["revision"].as_u64().unwrap();
    destination
        .host
        .handle(HostRequest::SendMessage {
            command_id: id("message"),
            expected_revision: revision,
            conversation_id: draft.conversation_id.clone(),
            project_id: draft.project_id.clone(),
            content: "Prepare an owned fixture change".into(),
        })
        .unwrap();
    let prepared = destination.host.prepare(draft).unwrap();
    assert_eq!(prepared["task"]["connection_id"], "destination-connection");
    assert_eq!(prepared["task"]["connection_revision"], 1);
    assert_eq!(
        prepared["task"]["policy"]["mode_defaults"]["strategy"],
        "efficiency"
    );
    assert_eq!(
        prepared["task"]["policy"]["mode_defaults"]["connection_id"],
        "destination-connection"
    );
    assert_eq!(prepared["scopeEvidence"]["connectionRevision"], 1);
    assert_eq!(
        prepared["capacity"]["observation"]["connectionId"],
        "destination-connection"
    );
    assert_eq!(prepared["capacity"]["observation"]["connectionRevision"], 1);
    assert_eq!(
        prepared["capacity"]["observation"]["ordinaryUsage"]["value"],
        true
    );
    assert_eq!(
        prepared["capacity"]["diagnosticId"],
        prepared["scopeEvidence"]["diagnosticId"]
    );
    assert_eq!(
        destination.metadata_reads(),
        reads + 1,
        "preparation has a separate authorized directory scope"
    );
    assert!(!source.config.home.join("dispatch-count").exists());
    assert!(!destination.config.home.join("dispatch-count").exists());
    let evidence = json!({"kind":"host-real-daemon-synthetic-runtime","uiDialogsDriven":false,"sourceReads":source.metadata_reads(),"destinationReads":destination.metadata_reads(),
        "sourceRecurringObservedThenRevoked":true,"destinationRecurringAuthorized":false,"mappedConnection":"destination-connection","preparedTask":prepared,
        "sourceSnapshot":source.host.call(Operation::Snapshot).unwrap(),"destinationSnapshot":destination.host.call(Operation::Snapshot).unwrap()});
    fs::write(
        root.join("host-transfer-evidence.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    source.stop();
    destination.stop();
}

#[test]
fn export_grants_do_not_overwrite_and_invalid_files_do_not_create_import_authority() {
    let root = isolated_directory();
    let mut host = Host::new(root.join("data")).unwrap();
    host.test_transport = Some(Arc::new(|operation| match operation {
        Operation::Desktop { request } if matches!(*request, DesktopRequest::ExportPolicy) => Ok(
            json!({"document":serde_json::to_string(&orchestrix_core::policy_portability::export(&Settings::default()).unwrap()).unwrap(),"summary":{}}),
        ),
        Operation::Desktop { request }
            if matches!(*request, DesktopRequest::PreviewPolicyImport { .. }) =>
        {
            Err("portable-schema".into())
        }
        _ => panic!("unexpected provider/execution operation"),
    }));
    let existing = root.join("existing.json");
    fs::write(&existing, b"preserve original").unwrap();
    let export = host.handle(HostRequest::PreparePolicyExport).unwrap();
    let export_ref = export["exportRef"].as_str().unwrap();
    assert_eq!(
        host.write_policy_export(export_ref, &existing).unwrap_err(),
        "policy-export-target-exists-or-unavailable"
    );
    assert_eq!(fs::read(&existing).unwrap(), b"preserve original");
    assert!(
        !fs::read_dir(&root).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
    let output = root.join("new.json");
    host.write_policy_export(export_ref, &output).unwrap();
    assert!(
        host.write_policy_export(export_ref, &root.join("replayed.json"))
            .is_err(),
        "successful save consumes only its export reference"
    );
    let invalid = root.join("invalid.json");
    fs::write(&invalid, b"{\"credentials\":\"not allowed\"}").unwrap();
    assert_eq!(
        host.select_policy_import(&invalid).unwrap_err(),
        "portable-schema"
    );
    assert!(
        host.handle(HostRequest::PreviewPolicyImport {
            import_ref: invalid.to_string_lossy().into(),
            mappings: ImportMappings::default()
        })
        .is_err()
    );
    assert_eq!(
        fs::read(&invalid).unwrap(),
        b"{\"credentials\":\"not allowed\"}"
    );
}
