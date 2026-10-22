use orchestrix_core::git_records::*;
use serde_json::json;

fn legacy() -> serde_json::Value {
    let mut value = json!({"id":"artifact-legacy","revision":"","operation_id":"operation-legacy","repository_id":"repo","run_id":"run","task_id":"task","attempt_id":"attempt","base_oid":"a".repeat(40),"origin":std::env::temp_dir().to_str().unwrap(),"files":[],"exclusions":[],"patch":"","patch_sha256":content_hash(b"")});
    let mut hash_input = value.clone();
    hash_input.as_object_mut().unwrap().remove("revision");
    value["revision"] = json!(content_hash(hash_input.to_string().as_bytes()));
    value
}
#[test]
fn legacy_artifact_keeps_its_original_representation_and_hash() {
    let value = legacy();
    let original_fields = [
        "id",
        "revision",
        "operation_id",
        "repository_id",
        "run_id",
        "task_id",
        "attempt_id",
        "base_oid",
        "origin",
        "files",
        "exclusions",
        "patch",
        "patch_sha256",
    ];
    let bytes = format!(
        "{{{}}}",
        original_fields
            .iter()
            .map(|key| format!("\"{key}\":{}", value[*key]))
            .collect::<Vec<_>>()
            .join(",")
    )
    .into_bytes();
    let artifact: CapturedArtifact = serde_json::from_slice(&bytes).unwrap();
    assert!(artifact.validate());
    assert!(artifact.preservation.is_none());
    assert_eq!(serde_json::to_vec(&artifact).unwrap(), bytes);
    assert_eq!(artifact.content_revision(), value["revision"]);
    let mut explicit_null = value;
    explicit_null["preservation"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<CapturedArtifact>(explicit_null).is_err());
}
#[test]
fn opaque_manifest_carries_only_integrity_and_has_independent_bounds() {
    let before = FileContent::new("before\n".into(), "100644".into()).unwrap();
    let manifest = BasePreservation {
        schema_version: 2,
        base_oid: "a".repeat(40),
        tree_oid: "b".repeat(40),
        editable_paths: vec!["text.md".into()],
        entries: vec![
            PreservedBaseEntry {
                path: "opaque.bin".into(),
                mode: "100644".into(),
                size: 2,
                git_oid: "c".repeat(40),
                sha256: content_hash(&[0, 255]),
            },
            PreservedBaseEntry {
                path: "text.md".into(),
                mode: "100644".into(),
                size: before.content.len() as u64,
                git_oid: "d".repeat(40),
                sha256: before.sha256.clone(),
            },
        ],
    };
    assert!(manifest.validate());
    assert!(!serde_json::to_string(&manifest).unwrap().contains("before"));
    let mut artifact: CapturedArtifact = serde_json::from_value(legacy()).unwrap();
    artifact.preservation = Some(manifest.clone());
    artifact.files.push(CapturedFile {
        path: "text.md".into(),
        base: Some(before.clone()),
        index: Some(before),
        worktree: Some(FileContent::new("after\n".into(), "100644".into()).unwrap()),
        untracked: false,
    });
    artifact.revision = artifact.content_revision();
    assert!(artifact.validate());
    let revision = artifact.revision.clone();
    artifact
        .preservation
        .as_mut()
        .unwrap()
        .editable_paths
        .push("z.md".into());
    assert_ne!(artifact.content_revision(), revision);
    assert!(!artifact.validate());
    let mut oversized = manifest.clone();
    oversized.entries[0].size = MAX_OPAQUE_FILE_BYTES + 1;
    assert!(!oversized.validate());
    let mut selected = manifest.clone();
    selected.entries[1].size = MAX_FILE_BYTES as u64 + 1;
    assert!(!selected.validate());
    let mut alias = manifest.clone();
    alias.entries[1].path = "OPAQUE.BIN".into();
    assert!(!alias.validate());
    let mut future = manifest;
    future.schema_version = 3;
    assert!(!future.validate());
}

fn many_manifest(count: usize, path_bytes: usize) -> BasePreservation {
    BasePreservation {
        schema_version: 2,
        base_oid: "a".repeat(40),
        tree_oid: "b".repeat(40),
        editable_paths: vec!["editable.txt".into()],
        entries: (0..count)
            .map(|index| PreservedBaseEntry {
                path: format!("file-{index:04}/{}", "x".repeat(path_bytes)),
                mode: "100644".into(),
                size: 0,
                git_oid: "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391".into(),
                sha256: content_hash(b""),
            })
            .collect(),
    }
}
fn worktree(manifest: Option<BasePreservation>) -> WorktreeIntent {
    let root = std::env::temp_dir();
    WorktreeIntent {
        preservation: manifest,
        operation_id: "opaque-operation".into(),
        project_id: "project".into(),
        repository_id: "repository".into(),
        run_id: "run".into(),
        task_id: "task".into(),
        attempt_id: "attempt".into(),
        base_oid: "a".repeat(40),
        domain_base: "base".into(),
        worktree_ref: "worktree".into(),
        authorized_root: root.join("owned").to_str().unwrap().into(),
        path: root.join("owned/attempt").to_str().unwrap().into(),
        main_repository: root.join("repository").to_str().unwrap().into(),
        common_git_dir: root.join("repository/.git").to_str().unwrap().into(),
        branch: "orchestrix/attempt".into(),
        ownership_token: "c".repeat(64),
    }
}
#[test]
fn large_typed_manifest_has_a_bounded_exact_fingerprint_without_changing_v1() {
    let intent = worktree(Some(many_manifest(1000, 8)));
    assert!(intent.validate());
    let original = serde_json::to_vec(&intent).unwrap();
    assert!(
        orchestrix_core::persistence_privacy::sanitize_json(&json!(["git.plan", intent])).is_err()
    );
    let command = GitCommand::Plan(intent.clone());
    assert!(command.input_bounded());
    assert!(
        orchestrix_core::persistence_privacy::sanitize_json(&command.fingerprint_body()).is_ok()
    );
    let fingerprint = command.fingerprint_body();
    let mut changed = intent.clone();
    changed.preservation.as_mut().unwrap().entries[0].sha256 = "d".repeat(64);
    assert_ne!(GitCommand::Plan(changed).fingerprint_body(), fingerprint);
    assert_eq!(serde_json::to_vec(&intent).unwrap(), original);
    let old = worktree(None);
    assert_eq!(
        GitCommand::Plan(old.clone()).fingerprint_body(),
        json!(["git.plan", old])
    );
}

#[test]
#[cfg(windows)]
fn full_ownership_envelope_is_rejected_before_planning_when_it_exceeds_stdin() {
    let mut intent = worktree(None);
    let root = std::env::temp_dir();
    // Native Windows separators also consume JSON escaping bytes in the exact
    // ownership blob; a manifest-only bound does not include that envelope.
    let long = "a\\".repeat(1900);
    intent.authorized_root = root.join(&long).to_str().unwrap().into();
    intent.path = root.join(&long).join("attempt").to_str().unwrap().into();
    intent.main_repository = root.join("source").join(&long).to_str().unwrap().into();
    intent.common_git_dir = root
        .join("source")
        .join(&long)
        .join(".git")
        .to_str()
        .unwrap()
        .into();
    let mut found = false;
    for path_bytes in 3700..4050 {
        let manifest = many_manifest(250, path_bytes);
        if !manifest.validate() {
            continue;
        }
        intent.preservation = Some(manifest);
        if serde_json::to_vec(&intent).unwrap().len() > MAX_WORKTREE_INTENT_BYTES {
            found = true;
            assert!(!intent.validate());
            assert!(!GitCommand::Plan(intent.clone()).input_bounded());
            break;
        }
    }
    assert!(
        found,
        "exercise the valid-manifest/oversized-envelope boundary"
    );
}
