//! Native Git object identities for the opt-in preservation profile.
//! No mutable-state cache or filesystem effects; callers retain bounded reads.
use super::{GitError, GitExecutor, GitResult, worktrees::valid_oid};
use crate::git_records::{
    MAX_BASE_FILES, MAX_PRESERVATION_BYTES, PreservedBaseEntry, relative_file_path,
};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn native_oid(kind: &str, width: usize, bytes: &[u8]) -> GitResult<String> {
    if !matches!(kind, "blob" | "tree" | "commit") || !matches!(width, 40 | 64) {
        return Err(GitError::Unsupported);
    }
    let header = format!("{kind} {}\0", bytes.len());
    Ok(if width == 40 {
        let mut hash = Sha1::new();
        hash.update(header.as_bytes());
        hash.update(bytes);
        format!("{:x}", hash.finalize())
    } else {
        let mut hash = Sha256::new();
        hash.update(header.as_bytes());
        hash.update(bytes);
        format!("{:x}", hash.finalize())
    })
}

/// cat-file framing alone does not certify the requested native identity.
pub(super) fn verify(kind: &str, oid: &str, bytes: &[u8]) -> GitResult<()> {
    if !valid_oid(oid) {
        return Err(GitError::Unsupported);
    }
    if native_oid(kind, oid.len(), bytes)? == oid {
        Ok(())
    } else {
        Err(GitError::InconsistentCapture)
    }
}

fn commit_tree(oid: &str, bytes: &[u8]) -> GitResult<String> {
    verify("commit", oid, bytes)?;
    let end = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or(GitError::Base)?;
    let first = std::str::from_utf8(&bytes[..end]).map_err(|_| GitError::Base)?;
    let tree = first.strip_prefix("tree ").ok_or(GitError::Base)?;
    if !valid_oid(tree) || tree.len() != oid.len() {
        return Err(GitError::Base);
    }
    Ok(tree.to_owned())
}

/// Read only the pinned commit, never its parents/history or a HEAD fallback.
pub(super) fn base_tree(executor: &GitExecutor, root: &Path, base: &str) -> GitResult<String> {
    let bytes = executor.read_preservation_commit(root, base)?;
    commit_tree(base, &bytes)
}

fn append_oid(bytes: &mut Vec<u8>, oid: &str) -> GitResult<()> {
    for pair in oid.as_bytes().as_chunks::<2>().0 {
        bytes.push(
            u8::from_str_radix(
                std::str::from_utf8(pair).map_err(|_| GitError::Unsupported)?,
                16,
            )
            .map_err(|_| GitError::Unsupported)?,
        );
    }
    Ok(())
}

fn tree_oid(
    files: &[(&str, &str)],
    width: usize,
    depth: usize,
    encoded_bytes: &mut usize,
) -> GitResult<String> {
    if depth > 32 {
        return Err(GitError::Limit);
    }
    let mut direct = BTreeMap::new();
    let mut directories: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for &(path, oid) in files {
        if let Some((name, tail)) = path.split_once('/') {
            directories.entry(name).or_default().push((tail, oid));
        } else if direct.insert(path, oid).is_some() {
            return Err(GitError::Conflict);
        }
    }
    let mut entries = Vec::new();
    for (name, oid) in direct {
        if directories.contains_key(name) {
            return Err(GitError::Conflict);
        }
        entries.push((name, false, oid.to_owned()));
    }
    for (name, children) in directories {
        entries.push((
            name,
            true,
            tree_oid(&children, width, depth + 1, encoded_bytes)?,
        ));
    }
    // Git compares a directory as name + slash and a regular file as name + NUL.
    // This matches write_oid_tree without invoking Git once per directory.
    entries.sort_by(|a, b| {
        format!("{}{}", a.0, if a.1 { "/" } else { "\0" })
            .as_bytes()
            .cmp(format!("{}{}", b.0, if b.1 { "/" } else { "\0" }).as_bytes())
    });
    let mut bytes = Vec::new();
    for (name, directory, oid) in entries {
        let prefix = format!("{} {name}\0", if directory { "40000" } else { "100644" });
        *encoded_bytes = encoded_bytes
            .checked_add(prefix.len() + width / 2)
            .ok_or(GitError::Limit)?;
        if *encoded_bytes > MAX_PRESERVATION_BYTES {
            return Err(GitError::Limit);
        }
        bytes.extend_from_slice(prefix.as_bytes());
        append_oid(&mut bytes, &oid)?;
    }
    native_oid("tree", width, &bytes)
}

fn projected_tree(entries: &[PreservedBaseEntry], width: usize) -> GitResult<String> {
    if entries.len() > MAX_BASE_FILES || !matches!(width, 40 | 64) {
        return Err(GitError::Limit);
    }
    let mut aliases = BTreeSet::new();
    for entry in entries {
        if !relative_file_path(&entry.path)
            || entry
                .path
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c))
            || !aliases.insert(entry.path.to_lowercase())
            || entry.mode != "100644"
            || !valid_oid(&entry.git_oid)
            || entry.git_oid.len() != width
        {
            return Err(GitError::Unsupported);
        }
    }
    let files = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.git_oid.as_str()))
        .collect::<Vec<_>>();
    let mut encoded_bytes = 0;
    tree_oid(&files, width, 0, &mut encoded_bytes)
}

/// The complete flat inventory must reconstruct the exact tree named by the
/// authenticated commit. Missing or altered tree entries cannot acquire a new
/// preservation hash merely because ls-tree returned a plausible projection.
pub(super) fn inventory_tree(tree: &str, entries: &[PreservedBaseEntry]) -> GitResult<()> {
    if !valid_oid(tree) {
        return Err(GitError::Unsupported);
    }
    if projected_tree(entries, tree.len())? == tree {
        Ok(())
    } else {
        Err(GitError::InconsistentCapture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
    const HELLO_BLOB: &str = "ce013625030ba8dba906f756967f9e9ca394464a";
    const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    fn entry(path: &str, oid: &str) -> PreservedBaseEntry {
        PreservedBaseEntry {
            path: path.into(),
            mode: "100644".into(),
            size: 0,
            git_oid: oid.into(),
            sha256: "0".repeat(64),
        }
    }
    fn frame(mode: &str, name: &str, oid: &str) -> Vec<u8> {
        let mut bytes = format!("{mode} {name}\0").into_bytes();
        append_oid(&mut bytes, oid).unwrap();
        bytes
    }

    #[test]
    fn raw_native_identity_covers_type_header_and_exact_body() {
        let empty_sha256 = "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813";
        let empty_tree_sha256 = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";
        assert_eq!(verify("blob", EMPTY_BLOB, b""), Ok(()));
        assert_eq!(verify("blob", HELLO_BLOB, b"hello\n"), Ok(()));
        assert_eq!(verify("blob", empty_sha256, b""), Ok(()));
        assert_eq!(verify("tree", EMPTY_TREE, b""), Ok(()));
        assert_eq!(verify("tree", empty_tree_sha256, b""), Ok(()));
        assert_eq!(
            verify("blob", HELLO_BLOB, b"jello\n"),
            Err(GitError::InconsistentCapture)
        );
        assert_eq!(
            verify("tree", EMPTY_BLOB, b""),
            Err(GitError::InconsistentCapture)
        );
        assert_eq!(
            verify("blob", "not-an-oid", b"hello\n"),
            Err(GitError::Unsupported)
        );
    }

    #[test]
    fn commit_identity_and_first_tree_are_both_required() {
        let body = format!(
            "tree {EMPTY_TREE}\nauthor QA <qa@invalid.example> 1 +0000\ncommitter QA <qa@invalid.example> 1 +0000\n\nOwned fixture\n"
        );
        let oid = native_oid("commit", 40, body.as_bytes()).unwrap();
        assert_eq!(commit_tree(&oid, body.as_bytes()), Ok(EMPTY_TREE.into()));
        let changed = body.replace(EMPTY_TREE, &"0".repeat(40));
        assert_eq!(
            commit_tree(&oid, changed.as_bytes()),
            Err(GitError::InconsistentCapture)
        );
        let wrong_first = format!("parent {}\ntree {EMPTY_TREE}\n\nbody\n", "0".repeat(40));
        let wrong_first_oid = native_oid("commit", 40, wrong_first.as_bytes()).unwrap();
        assert_eq!(
            commit_tree(&wrong_first_oid, wrong_first.as_bytes()),
            Err(GitError::Base)
        );
    }

    #[test]
    fn projected_tree_uses_git_directory_order_and_exact_native_entries() {
        let inventory = vec![
            entry("foobar", EMPTY_BLOB),
            entry("foo/baz", HELLO_BLOB),
            entry("foo.bar", EMPTY_BLOB),
        ];
        let child = native_oid("tree", 40, &frame("100644", "baz", HELLO_BLOB)).unwrap();
        let mut root = frame("100644", "foo.bar", EMPTY_BLOB);
        root.extend(frame("40000", "foo", &child));
        root.extend(frame("100644", "foobar", EMPTY_BLOB));
        let expected = native_oid("tree", 40, &root).unwrap();
        assert_eq!(inventory_tree(&expected, &inventory), Ok(()));
        assert_eq!(
            inventory_tree(&"0".repeat(40), &inventory),
            Err(GitError::InconsistentCapture)
        );
        let mut missing = inventory.clone();
        missing.pop();
        assert_eq!(
            inventory_tree(&expected, &missing),
            Err(GitError::InconsistentCapture)
        );
        assert_eq!(inventory_tree(EMPTY_TREE, &[]), Ok(()));
    }

    #[test]
    fn projection_rejects_prefix_collisions_modes_aliases_and_algorithm_mixing() {
        assert_eq!(
            projected_tree(
                &[entry("foo", EMPTY_BLOB), entry("foo/bar", HELLO_BLOB)],
                40
            ),
            Err(GitError::Conflict)
        );
        assert_eq!(
            projected_tree(&[entry("Foo", EMPTY_BLOB), entry("foo", HELLO_BLOB)], 40),
            Err(GitError::Unsupported)
        );
        let mut executable = entry("file", EMPTY_BLOB);
        executable.mode = "100755".into();
        assert_eq!(
            projected_tree(&[executable], 40),
            Err(GitError::Unsupported)
        );
        assert_eq!(
            projected_tree(&[entry("file", &"a".repeat(64))], 40),
            Err(GitError::Unsupported)
        );
        let too_deep = format!("{}file", "d/".repeat(33));
        assert_eq!(
            projected_tree(&[entry(&too_deep, EMPTY_BLOB)], 40),
            Err(GitError::Limit)
        );
    }
}
