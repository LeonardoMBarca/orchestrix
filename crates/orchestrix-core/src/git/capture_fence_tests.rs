//! Real two-file v2 captures exercise the closing observation after delta assembly.
use super::super::executor::MetadataFixture;
use super::*;

const ORIGINAL: &[u8] = b"original editable text\n";
const EDITED: &[u8] = b"authorized edited text\n";
const OPAQUE: &[u8] = &[0, 1, 2, 255, 254, 3];

struct Fixture {
    owner: MetadataFixture,
    main: PathBuf,
    checkout: PathBuf,
    intent: WorktreeIntent,
    alternate: String,
    wrong_blob: String,
}

fn object(executor: &GitExecutor, root: &Path, kind: &str, body: &[u8]) -> String {
    let output = executor
        .run_with_input(
            root,
            &args(&["hash-object", "-t", kind, "-w", "--stdin"]),
            body,
        )
        .unwrap()
        .checked()
        .unwrap();
    std::str::from_utf8(&output).unwrap().trim().to_owned()
}

fn commit(executor: &GitExecutor, root: &Path, editable: &str, opaque: &str) -> String {
    let mut tree = Vec::new();
    for (path, oid) in [("editable.txt", editable), ("opaque.bin", opaque)] {
        tree.extend_from_slice(format!("100644 {path}\0").as_bytes());
        let (pairs, remainder) = oid.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        for pair in pairs {
            tree.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
    }
    let tree = object(executor, root, "tree", &tree);
    object(
        executor,
        root,
        "commit",
        format!(
            "tree {tree}\nauthor Fixture <fixture@orchestrix.invalid> 1 +0000\ncommitter Fixture <fixture@orchestrix.invalid> 1 +0000\n\nOwned capture closing fence\n"
        )
        .as_bytes(),
    )
}

impl Fixture {
    fn new() -> Self {
        let owner = MetadataFixture::new();
        let main = owner.repository("closing-fence");
        let editable = object(&owner.executor, &main, "blob", ORIGINAL);
        let opaque = object(&owner.executor, &main, "blob", OPAQUE);
        let base = commit(&owner.executor, &main, &editable, &opaque);
        let alternate_blob = object(&owner.executor, &main, "blob", b"alternate editable\n");
        let alternate = commit(&owner.executor, &main, &alternate_blob, &opaque);
        let wrong_blob = object(&owner.executor, &main, "blob", &[0, 1, 9, 255, 254, 3]);
        git(
            &owner.executor,
            &main,
            &["update-ref", "refs/heads/main", &base],
        )
        .unwrap();
        git(&owner.executor, &main, &["read-tree", &base]).unwrap();
        fs::write(main.join("editable.txt"), ORIGINAL).unwrap();
        fs::write(main.join("opaque.bin"), OPAQUE).unwrap();
        let preservation = super::super::preservation::plan(
            &owner.executor,
            &main,
            &base,
            &["editable.txt".into()],
        )
        .unwrap();
        assert_eq!(preservation.schema_version, 2);
        assert_eq!(preservation.entries.len(), 2);
        let authorized = owner.root.join("authorized");
        fs::create_dir(&authorized).unwrap();
        let checkout = authorized.join("checkout");
        let checkout_arg = super::super::paths::git_path(&checkout).unwrap();
        git(
            &owner.executor,
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "--no-checkout",
                "--lock",
                "-b",
                "orchestrix/capture-fence",
                checkout_arg.to_str().unwrap(),
                &base,
            ],
        )
        .unwrap();
        git(&owner.executor, &checkout, &["read-tree", &base]).unwrap();
        fs::write(checkout.join("editable.txt"), EDITED).unwrap();
        fs::write(checkout.join("opaque.bin"), OPAQUE).unwrap();
        let text = |path: &Path| fs::canonicalize(path).unwrap().to_str().unwrap().to_owned();
        let intent = WorktreeIntent {
            preservation: Some(preservation),
            operation_id: "capture-closing-fence".into(),
            project_id: "project".into(),
            repository_id: "repository".into(),
            run_id: "run".into(),
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            base_oid: base,
            domain_base: "base".into(),
            worktree_ref: "owned-reference".into(),
            authorized_root: text(&authorized),
            path: text(&checkout),
            main_repository: text(&main),
            branch: "orchestrix/capture-fence".into(),
            common_git_dir: text(&main.join(".git")),
            ownership_token: "a".repeat(64),
        };
        assert!(intent.validate());
        Self {
            owner,
            main,
            checkout,
            intent,
            alternate,
            wrong_blob,
        }
    }

    fn capture(
        &self,
        after_assembly: impl FnOnce() -> GitResult<()>,
    ) -> GitResult<CapturedArtifact> {
        capture_with_fences(
            &self.owner.executor,
            &self.checkout,
            &self.intent,
            CaptureLimits::default(),
            || Ok(()),
            after_assembly,
        )
    }
}

#[test]
fn stable_v2_delta_keeps_the_complete_opaque_base_and_exact_revision() {
    let fixture = Fixture::new();
    let artifact = fixture.capture(|| Ok(())).unwrap();
    assert!(artifact.validate());
    assert_eq!(artifact.revision, artifact.content_revision());
    assert_eq!(artifact.preservation, fixture.intent.preservation);
    assert_eq!(artifact.files.len(), 1);
    assert_eq!(artifact.files[0].path, "editable.txt");
    assert_eq!(
        artifact.files[0].base.as_ref().unwrap().content.as_bytes(),
        ORIGINAL
    );
    assert_eq!(
        artifact.files[0]
            .worktree
            .as_ref()
            .unwrap()
            .content
            .as_bytes(),
        EDITED
    );
    assert_eq!(
        fs::read(fixture.checkout.join("opaque.bin")).unwrap(),
        OPAQUE
    );
}

#[test]
fn editable_write_after_delta_assembly_is_rejected_and_preserved() {
    let fixture = Fixture::new();
    let changed = b"write after immutable object reads\n";
    let result = fixture.capture(|| {
        fs::write(fixture.checkout.join("editable.txt"), changed)?;
        Ok(())
    });
    assert!(matches!(result, Err(GitError::InconsistentCapture)));
    assert_eq!(
        fs::read(fixture.checkout.join("editable.txt")).unwrap(),
        changed
    );
}

#[test]
fn same_length_opaque_write_after_delta_assembly_is_rejected_and_preserved() {
    let fixture = Fixture::new();
    let changed = [0, 1, 9, 255, 254, 3];
    let result = fixture.capture(|| {
        fs::write(fixture.checkout.join("opaque.bin"), changed)?;
        Ok(())
    });
    assert!(matches!(result, Err(GitError::UnpreservedWork)));
    assert_eq!(
        fs::read(fixture.checkout.join("opaque.bin")).unwrap(),
        changed
    );
}

#[test]
fn head_change_after_delta_assembly_is_rejected_without_restoring_the_ref() {
    let fixture = Fixture::new();
    let result = fixture.capture(|| {
        git(
            &fixture.owner.executor,
            &fixture.checkout,
            &["update-ref", "HEAD", &fixture.alternate],
        )?;
        Ok(())
    });
    assert!(matches!(result, Err(GitError::Base)));
    assert_eq!(
        git(
            &fixture.owner.executor,
            &fixture.checkout,
            &["rev-parse", "HEAD"]
        )
        .unwrap(),
        format!("{}\n", fixture.alternate).as_bytes()
    );
}

#[test]
fn symbolic_ref_change_with_identical_head_after_assembly_is_rejected() {
    let fixture = Fixture::new();
    git(
        &fixture.owner.executor,
        &fixture.checkout,
        &[
            "update-ref",
            "refs/heads/other-fence",
            &fixture.intent.base_oid,
        ],
    )
    .unwrap();
    let result = fixture.capture(|| {
        git(
            &fixture.owner.executor,
            &fixture.checkout,
            &["symbolic-ref", "HEAD", "refs/heads/other-fence"],
        )?;
        Ok(())
    });
    assert!(matches!(result, Err(GitError::InconsistentCapture)));
    assert_eq!(
        git(
            &fixture.owner.executor,
            &fixture.checkout,
            &["symbolic-ref", "HEAD"]
        )
        .unwrap(),
        b"refs/heads/other-fence\n"
    );
}

#[test]
fn index_change_after_delta_assembly_is_rejected_without_touching_working_bytes() {
    let fixture = Fixture::new();
    let result = fixture.capture(|| {
        git(
            &fixture.owner.executor,
            &fixture.checkout,
            &["read-tree", &fixture.alternate],
        )?;
        Ok(())
    });
    assert!(matches!(result, Err(GitError::InconsistentCapture)));
    assert_eq!(
        fs::read(fixture.checkout.join("editable.txt")).unwrap(),
        EDITED
    );
    let index = git(
        &fixture.owner.executor,
        &fixture.checkout,
        &["ls-files", "-s", "--", "editable.txt"],
    )
    .unwrap();
    let expected_blob = git(
        &fixture.owner.executor,
        &fixture.checkout,
        &["rev-parse", &format!("{}:editable.txt", fixture.alternate)],
    )
    .unwrap();
    assert!(
        std::str::from_utf8(&index)
            .unwrap()
            .contains(std::str::from_utf8(&expected_blob).unwrap().trim())
    );
}

#[test]
fn restored_bytes_with_a_changed_stamp_after_assembly_are_not_the_first_snapshot() {
    let fixture = Fixture::new();
    let path = fixture.checkout.join("editable.txt");
    let before = stamp(&fs::symlink_metadata(&path).unwrap()).unwrap();
    let result = fixture.capture(|| {
        fs::write(&path, b"transient changed text\n")?;
        fs::write(&path, EDITED)?;
        let modified = UNIX_EPOCH
            + std::time::Duration::from_nanos(before.modified_nanos as u64)
            + std::time::Duration::from_secs(2);
        File::options()
            .write(true)
            .open(&path)?
            .set_times(fs::FileTimes::new().set_modified(modified))?;
        assert_ne!(stamp(&fs::symlink_metadata(&path)?).unwrap(), before);
        Ok(())
    });
    assert!(matches!(result, Err(GitError::InconsistentCapture)));
    assert_eq!(fs::read(path).unwrap(), EDITED);
}

#[test]
fn valid_compression_under_the_wrong_native_oid_after_assembly_is_rejected() {
    let fixture = Fixture::new();
    let opaque = fixture
        .intent
        .preservation
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.path == "opaque.bin")
        .unwrap();
    let loose = |oid: &str| {
        fixture
            .main
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..])
    };
    let replacement = fs::read(loose(&fixture.wrong_blob)).unwrap();
    let expected = loose(&opaque.git_oid);
    let owned_objects = fs::canonicalize(fixture.main.join(".git/objects")).unwrap();
    let expected = fs::canonicalize(expected).unwrap();
    assert!(expected.starts_with(owned_objects) && expected.is_file());
    super::super::paths::reject_links(&expected).unwrap();
    let mut written = false;
    let result = fixture.capture(|| {
        #[cfg(windows)]
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Windows-only owned loose-object fixture; never a developer repository"
        )]
        {
            let mut permissions = fs::metadata(&expected)?.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&expected, permissions)?;
        }
        fs::write(&expected, &replacement)?;
        written = true;
        Ok(())
    });
    assert!(written, "owned loose-object replacement must have occurred");
    assert!(
        matches!(result, Err(GitError::InconsistentCapture)),
        "actual error: {:?}",
        result.as_ref().err()
    );
    assert_eq!(fs::read(expected).unwrap(), replacement);
    assert_eq!(
        fs::read(fixture.checkout.join("opaque.bin")).unwrap(),
        OPAQUE
    );
}
