//! Native-selected policy documents. Opaque references never grant filesystem execution.
use super::Settings;
use super::{Host, id};
use orchestrix_core::{
    desktop::DesktopRequest,
    policy_portability::{ImportMappings, MAX_PORTABLE_POLICY_BYTES},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

#[derive(Default)]
pub(super) struct PolicyFiles {
    imports: HashMap<String, String>,
    exports: HashMap<String, String>,
}

pub(super) struct ImportExpectation {
    pub revision: u64,
    pub digest: String,
}

impl Host {
    /// Bind Settings bytes and the configuration fingerprint to one Store revision.
    /// A concurrent writer makes this preparation fail closed, never pair an old
    /// preference snapshot with a newer fingerprint that would grant admission.
    pub(super) fn preparation_settings(&self) -> Result<(Value, Settings, String), String> {
        let catalog = self.desktop(DesktopRequest::Configurations {
            request: Box::new(orchestrix_core::configuration::ConfigurationRequest::Catalog {}),
        })?;
        let snapshot = self.desktop(DesktopRequest::Bootstrap)?;
        if catalog["revision"].as_u64().is_none()
            || catalog["revision"].as_u64() != snapshot["revision"].as_u64()
        {
            return Err("prepared-policy-changed-prepare-again".into());
        }
        let settings =
            serde_json::from_value(snapshot["settings"].clone()).map_err(|_| "settings-schema")?;
        let fingerprint = catalog["executionFingerprint"]
            .as_str()
            .ok_or("configuration-fingerprint-required")?
            .to_owned();
        Ok((snapshot, settings, fingerprint))
    }
    pub fn select_policy_import(&self, path: &Path) -> Result<Value, String> {
        let metadata = fs::metadata(path).map_err(|_| "policy-file-unavailable")?;
        if !metadata.is_file() || metadata.len() > MAX_PORTABLE_POLICY_BYTES as u64 {
            return Err("policy-file-size".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|_| "policy-file-unavailable")?
            .take(MAX_PORTABLE_POLICY_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "policy-file-read")?;
        if bytes.len() > MAX_PORTABLE_POLICY_BYTES {
            return Err("policy-file-size".into());
        }
        let document = String::from_utf8(bytes).map_err(|_| "policy-file-utf8")?;
        // Core validates strict version/schema/fields before any grant is retained.
        let preview = self.desktop(DesktopRequest::PreviewPolicyImport {
            document: document.clone(),
            mappings: ImportMappings::default(),
        })?;
        let reference = id("policy-file");
        let mut files = self.policy_files.lock().map_err(|_| "policy-file-lock")?;
        if files.imports.len() >= 8 {
            return Err("policy-file-limit".into());
        }
        files.imports.insert(reference.clone(), document);
        Ok(
            json!({"importRef":reference,"name":path.file_name().unwrap_or_default().to_string_lossy(),"preview":preview}),
        )
    }
    pub(super) fn preview_policy_file(
        &self,
        reference: &str,
        mappings: ImportMappings,
    ) -> Result<Value, String> {
        let document = self.policy_document(reference)?;
        self.desktop(DesktopRequest::PreviewPolicyImport { document, mappings })
    }
    fn policy_document(&self, reference: &str) -> Result<String, String> {
        self.policy_files
            .lock()
            .map_err(|_| "policy-file-lock")?
            .imports
            .get(reference)
            .cloned()
            .ok_or_else(|| "policy-file-selection-required".into())
    }
    pub(super) fn apply_policy_file(
        &self,
        command_id: String,
        expected: ImportExpectation,
        reference: &str,
        mappings: ImportMappings,
        confirm: bool,
        fingerprint: &str,
    ) -> Result<Value, String> {
        if !confirm {
            return Err("policy-import-confirmation-required".into());
        }
        let document = self.policy_document(reference)?;
        let result = self.effect(
            &command_id,
            fingerprint,
            orchestrix_core::daemon::Operation::Desktop {
                request: Box::new(DesktopRequest::ApplyPolicyImport {
                    command_id: command_id.clone(),
                    expected_revision: expected.revision,
                    document,
                    mappings,
                    expected_preview_digest: expected.digest,
                }),
            },
            None,
            None,
            None,
        )?;
        self.prepared
            .lock()
            .map_err(|_| "preparation-lock")?
            .clear();
        Ok(result)
    }
    pub(super) fn release_policy_file(&self, reference: &str) -> Result<Value, String> {
        self.policy_files
            .lock()
            .map_err(|_| "policy-file-lock")?
            .imports
            .remove(reference);
        Ok(json!({"released":true,"settingsChanged":false}))
    }
    pub(super) fn prepare_policy_export(&self) -> Result<Value, String> {
        let value = self.desktop(DesktopRequest::ExportPolicy)?;
        let document = value["document"]
            .as_str()
            .ok_or("policy-export-schema")?
            .to_owned();
        let reference = id("policy-export");
        let mut files = self.policy_files.lock().map_err(|_| "policy-file-lock")?;
        // Only the latest preview can be saved; cancelling never exhausts a grant quota.
        files.exports.clear();
        files.exports.insert(reference.clone(), document.clone());
        Ok(json!({"exportRef":reference,"summary":value["summary"],"document":document}))
    }
    pub fn write_policy_export(&self, reference: &str, path: &Path) -> Result<Value, String> {
        let document = self
            .policy_files
            .lock()
            .map_err(|_| "policy-file-lock")?
            .exports
            .get(reference)
            .cloned()
            .ok_or("policy-export-prepare-required")?;
        // Never overwrite an existing file. A denied/failed export preserves it.
        let parent = path.parent().ok_or("policy-export-directory")?;
        let temporary = parent.join(format!(".{}.tmp", id("orchestrix-policy")));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "policy-export-write")?;
        let result: Result<(), String> = (|| {
            file.write_all(document.as_bytes())
                .map_err(|_| "policy-export-write")?;
            file.sync_all().map_err(|_| "policy-export-write")?;
            drop(file);
            fs::hard_link(&temporary, path)
                .map_err(|_| "policy-export-target-exists-or-unavailable")?;
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        result?;
        self.policy_files
            .lock()
            .map_err(|_| "policy-file-lock")?
            .exports
            .remove(reference);
        Ok(
            json!({"saved":true,"name":path.file_name().unwrap_or_default().to_string_lossy(),"credentialsIncluded":false,"executionAuthorityIncluded":false}),
        )
    }
    #[cfg(test)]
    pub(super) fn settings_fingerprint(settings: &Settings) -> Result<String, String> {
        settings.execution_fingerprint()
    }
    pub(super) fn guard_prepared_settings(&self, fingerprint: &str) -> Result<(), String> {
        let catalog = self.desktop(DesktopRequest::Configurations {
            request: Box::new(orchestrix_core::configuration::ConfigurationRequest::Catalog {}),
        })?;
        if catalog["executionFingerprint"].as_str() != Some(fingerprint) {
            return Err("prepared-policy-changed-prepare-again".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrix_core::{daemon::Operation, policy::Strategy};
    use std::sync::{Arc, Mutex};

    #[test]
    fn preparation_never_pairs_old_settings_with_a_newer_configuration_fingerprint() {
        for (catalog_revision, snapshot_revision, valid) in
            [(3, 3, true), (3, 4, false), (4, 3, false)]
        {
            let mut host =
                Host::new(std::env::temp_dir().join(id("preparation-snapshot-test"))).unwrap();
            let mut settings = Settings::default();
            settings.policy.mode_defaults.strategy = Some(Strategy::Efficiency);
            let expected = settings.execution_fingerprint().unwrap();
            let fingerprint = expected.clone();
            host.test_transport = Some(Arc::new(move |operation| match operation {
                Operation::Desktop { request } => match *request {
                    DesktopRequest::Configurations { .. } => {
                        Ok(json!({"revision":catalog_revision,"executionFingerprint":fingerprint}))
                    }
                    DesktopRequest::Bootstrap => {
                        Ok(json!({"revision":snapshot_revision,"settings":settings}))
                    }
                    _ => panic!("preparation reads no provider or worker operation"),
                },
                _ => panic!("preparation reads no provider or worker operation"),
            }));
            let result = host.preparation_settings();
            if valid {
                let (_, settings, actual) = result.unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    settings.policy.mode_defaults.strategy,
                    Some(Strategy::Efficiency)
                );
            } else {
                assert_eq!(result.unwrap_err(), "prepared-policy-changed-prepare-again");
            }
        }
    }

    #[test]
    fn changed_policy_or_instructions_withdraw_preparation_but_appearance_does_not() {
        let root = std::env::temp_dir().join(id("policy-preparation-test"));
        let mut host = Host::new(root).unwrap();
        let settings = Arc::new(Mutex::new(Settings::default()));
        let observed = Arc::clone(&settings);
        host.test_transport = Some(Arc::new(move |operation| match operation {
            Operation::Desktop { request }
                if matches!(*request, DesktopRequest::Configurations { .. }) =>
            {
                Ok(
                    json!({"executionFingerprint":observed.lock().unwrap().execution_fingerprint().unwrap()}),
                )
            }
            _ => panic!("preparation validation must never execute or query a provider"),
        }));
        let original = Host::settings_fingerprint(&settings.lock().unwrap()).unwrap();
        host.guard_prepared_settings(&original).unwrap();
        settings.lock().unwrap().theme = "horizon".into();
        host.guard_prepared_settings(&original).unwrap();
        settings.lock().unwrap().policy.mode_defaults.strategy = Some(Strategy::Performance);
        assert_eq!(
            host.guard_prepared_settings(&original).unwrap_err(),
            "prepared-policy-changed-prepare-again"
        );
        let changed = Host::settings_fingerprint(&settings.lock().unwrap()).unwrap();
        settings.lock().unwrap().system_instructions = "Local response preference".into();
        assert_eq!(
            host.guard_prepared_settings(&changed).unwrap_err(),
            "prepared-policy-changed-prepare-again"
        );
    }
}
