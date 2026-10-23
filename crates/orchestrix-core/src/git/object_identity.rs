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

pub(super) fn native_oid(kind: &str, width: usize, bytes: &[u8]) -> GitResult<String> {
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

#[derive(Clone, Debug)]
pub(super) struct TreeObject {
    pub oid: String,
    pub bytes: Vec<u8>,
}

/// Operation-local identities and exact tree bodies derived from the complete
/// flat inventory. This is not proof that any object physically exists.
#[derive(Clone, Debug)]
pub(super) struct TreeProjection {
    pub trees: BTreeMap<String, TreeObject>,
    files: BTreeMap<String, String>,
    width: usize,
}

fn encode_tree(
    mut entries: Vec<(&str, bool, String)>,
    width: usize,
    encoded_bytes: &mut usize,
) -> GitResult<TreeObject> {
    // Git compares a directory as name + slash and a regular file as name + NUL.
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
    Ok(TreeObject {
        oid: native_oid("tree", width, &bytes)?,
        bytes,
    })
}

fn project_tree(
    files: &[(&str, &str)],
    width: usize,
    depth: usize,
    path: &str,
    encoded_bytes: &mut usize,
    trees: &mut BTreeMap<String, TreeObject>,
) -> GitResult<TreeObject> {
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
        let child_path = if path.is_empty() {
            name.to_owned()
        } else {
            format!("{path}/{name}")
        };
        entries.push((
            name,
            true,
            project_tree(
                &children,
                width,
                depth + 1,
                &child_path,
                encoded_bytes,
                trees,
            )?
            .oid,
        ));
    }
    let tree = encode_tree(entries, width, encoded_bytes)?;
    trees.insert(path.to_owned(), tree.clone());
    Ok(tree)
}

fn projection_for_entries(
    entries: &[PreservedBaseEntry],
    width: usize,
) -> GitResult<TreeProjection> {
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
        .map(|e| (e.path.clone(), e.git_oid.clone()))
        .collect();
    validate_files(&files, width)?;
    let mut projection = TreeProjection {
        files,
        trees: BTreeMap::new(),
        width,
    };
    let files = projection
        .files
        .iter()
        .map(|(p, o)| (p.as_str(), o.as_str()))
        .collect::<Vec<_>>();
    project_tree(&files, width, 0, "", &mut 0, &mut projection.trees)?;
    Ok(projection)
}

#[cfg(test)]
fn projected_tree(entries: &[PreservedBaseEntry], width: usize) -> GitResult<String> {
    projection_for_entries(entries, width).map(|p| p.root().to_owned())
}

fn validate_files(files: &BTreeMap<String, String>, width: usize) -> GitResult<()> {
    if files.len() > MAX_BASE_FILES {
        return Err(GitError::Limit);
    }
    let mut prefixes: BTreeMap<String, (String, bool)> = BTreeMap::new();
    let mut total = 0usize;
    for (path, oid) in files {
        if !super::preflight::source_path(path, &mut BTreeSet::new())
            || !valid_oid(oid)
            || oid.len() != width
        {
            return Err(GitError::Unsupported);
        }
        total = total.checked_add(path.len() + 256).ok_or(GitError::Limit)?;
        if total > MAX_PRESERVATION_BYTES || path.split('/').count() > 33 {
            return Err(GitError::Limit);
        }
        let parts = path.split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for (i, part) in parts.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let directory = i + 1 != parts.len();
            match prefixes.get(&prefix.to_lowercase()) {
                Some((existing, _)) if existing != &prefix => return Err(GitError::Unsupported),
                Some((_, role)) if *role != directory => return Err(GitError::Conflict),
                Some(_) => (),
                None => {
                    prefixes.insert(prefix.to_lowercase(), (prefix.clone(), directory));
                }
            }
        }
    }
    Ok(())
}

/// The complete flat inventory must reconstruct the exact tree named by the
/// authenticated commit. Missing or altered tree entries cannot acquire a new
/// preservation hash merely because ls-tree returned a plausible projection.
pub(super) fn inventory_tree(tree: &str, entries: &[PreservedBaseEntry]) -> GitResult<()> {
    inventory_projection(tree, entries).map(|_| ())
}

pub(super) fn inventory_projection(
    tree: &str,
    entries: &[PreservedBaseEntry],
) -> GitResult<TreeProjection> {
    if !valid_oid(tree) {
        return Err(GitError::Unsupported);
    }
    // Preserve the inventory grammar/alias/mode/size guarantees before keeping
    // any locally projected body. This does not read or normalize file content.
    let projection = projection_for_entries(entries, tree.len())?;
    if projection.root() != tree {
        return Err(GitError::InconsistentCapture);
    }
    Ok(projection)
}

impl TreeProjection {
    pub fn root(&self) -> &str {
        &self.trees[""].oid
    }

    /// Rebuild only ancestors of *actual* native blob changes. Unchanged tree
    /// bodies/OIDs remain exactly those authenticated against the pinned base.
    pub fn apply(&self, changes: &BTreeMap<String, Option<String>>) -> GitResult<Self> {
        let mut result = self.clone();
        let mut affected = BTreeSet::new();
        for (path, value) in changes {
            if !super::preflight::source_path(path, &mut BTreeSet::new())
                || value
                    .as_ref()
                    .is_some_and(|o| !valid_oid(o) || o.len() != self.width)
            {
                return Err(GitError::Unsupported);
            }
            if self.files.get(path) == value.as_ref() {
                continue;
            }
            if let Some(oid) = value {
                result.files.insert(path.clone(), oid.clone());
            } else {
                result.files.remove(path);
            }
            affected.insert(String::new());
            let mut parent = path.as_str();
            while let Some((ancestor, _)) = parent.rsplit_once('/') {
                affected.insert(ancestor.to_owned());
                parent = ancestor;
            }
        }
        validate_files(&result.files, self.width)?;
        let mut affected = affected.into_iter().collect::<Vec<_>>();
        affected.sort_by_key(|p| {
            std::cmp::Reverse(p.matches('/').count() + usize::from(!p.is_empty()))
        });
        for path in affected {
            let prefix = if path.is_empty() {
                String::new()
            } else {
                format!("{path}/")
            };
            let mut direct = BTreeMap::new();
            let mut directories = BTreeSet::new();
            for (file, oid) in result.files.range(prefix.clone()..) {
                let Some(tail) = file.strip_prefix(&prefix) else {
                    break;
                };
                if let Some((name, _)) = tail.split_once('/') {
                    directories.insert(name);
                } else {
                    direct.insert(tail, oid.clone());
                }
            }
            if !path.is_empty() && direct.is_empty() && directories.is_empty() {
                result.trees.remove(&path);
                continue;
            }
            let mut entries = Vec::new();
            for (name, oid) in direct {
                if directories.contains(name) {
                    return Err(GitError::Conflict);
                }
                entries.push((name, false, oid));
            }
            for name in directories {
                let child = format!("{prefix}{name}");
                let oid = result
                    .trees
                    .get(&child)
                    .ok_or(GitError::Conflict)?
                    .oid
                    .clone();
                entries.push((name, true, oid));
            }
            let tree = encode_tree(entries, self.width, &mut 0)?;
            result.trees.insert(path, tree);
        }
        let total = result.trees.values().try_fold(0usize, |n, t| {
            n.checked_add(t.bytes.len()).ok_or(GitError::Limit)
        })?;
        if total > MAX_PRESERVATION_BYTES {
            return Err(GitError::Limit);
        }
        Ok(result)
    }

    pub fn changed<'a>(&'a self, base: &'a Self) -> Vec<(&'a str, &'a TreeObject)> {
        let mut changed = self
            .trees
            .iter()
            .filter(|(path, tree)| base.trees.get(*path).is_none_or(|old| old.oid != tree.oid))
            .map(|(p, t)| (p.as_str(), t))
            .collect::<Vec<_>>();
        changed.sort_by_key(|(p, _)| {
            std::cmp::Reverse(p.matches('/').count() + usize::from(!p.is_empty()))
        });
        changed
    }

    pub fn verify_objects(&self, executor: &GitExecutor, root: &Path) -> GitResult<u64> {
        let objects = self
            .trees
            .values()
            .map(|t| ("tree", t.oid.as_str(), t.bytes.as_slice()))
            .collect::<Vec<_>>();
        verify_objects(executor, root, &objects)
    }
}

/// Bounded physical existence, framing, exact bytes and native identity. A
/// locally computed OID, ls-tree projection or matching size is insufficient.
pub(super) fn verify_objects(
    executor: &GitExecutor,
    root: &Path,
    objects: &[(&str, &str, &[u8])],
) -> GitResult<u64> {
    const BATCH: usize = 7 * 1_048_576;
    let mut position = 0;
    let mut verified = 0u64;
    while position < objects.len() {
        let mut end = position;
        let mut size = 0usize;
        let mut input_size = 0usize;
        while end < objects.len()
            && size
                .saturating_add(objects[end].2.len())
                .saturating_add(128)
                <= BATCH
            && input_size.saturating_add(objects[end].1.len() + 1) <= 1_048_576
        {
            size += objects[end].2.len() + 128;
            input_size += objects[end].1.len() + 1;
            end += 1;
        }
        if end == position {
            return Err(GitError::Limit);
        }
        let input = objects[position..end]
            .iter()
            .map(|(_, o, _)| format!("{o}\n"))
            .collect::<String>();
        let args = ["cat-file", "--batch"].map(std::ffi::OsString::from);
        let output = executor
            .run_with_input(root, &args, input.as_bytes())?
            .checked()?;
        let mut rest = output.as_slice();
        for (kind, oid, body) in &objects[position..end] {
            let newline = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or(GitError::InconsistentCapture)?;
            if rest[..newline] != *format!("{oid} {kind} {}", body.len()).as_bytes() {
                return Err(GitError::InconsistentCapture);
            }
            rest = &rest[newline + 1..];
            if rest.len() < body.len() + 1
                || rest[body.len()] != b'\n'
                || &rest[..body.len()] != *body
            {
                return Err(GitError::InconsistentCapture);
            }
            verify(kind, oid, &rest[..body.len()])?;
            verified += body.len() as u64;
            rest = &rest[body.len() + 1..];
        }
        if !rest.is_empty() {
            return Err(GitError::InconsistentCapture);
        }
        position = end;
    }
    Ok(verified)
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

    fn projection(files: &[(&str, &str)]) -> TreeProjection {
        let entries = files.iter().map(|(p, o)| entry(p, o)).collect::<Vec<_>>();
        let tree = projected_tree(&entries, 40).unwrap();
        inventory_projection(&tree, &entries).unwrap()
    }

    #[test]
    fn sparse_projection_rebuilds_only_actual_delta_ancestors() {
        let base = projection(&[
            ("deep/a/b/edit.txt", EMPTY_BLOB),
            ("sibling/other.txt", EMPTY_BLOB),
            ("deep/a/keep.bin", HELLO_BLOB),
            ("root.txt", EMPTY_BLOB),
        ]);
        let result = base
            .apply(&BTreeMap::from([(
                "deep/a/b/edit.txt".into(),
                Some(HELLO_BLOB.into()),
            )]))
            .unwrap();
        assert_eq!(
            result
                .changed(&base)
                .iter()
                .map(|(p, _)| *p)
                .collect::<Vec<_>>(),
            ["deep/a/b", "deep/a", "deep", ""]
        );
        assert_eq!(result.trees["sibling"].oid, base.trees["sibling"].oid);
        assert_eq!(result.trees["sibling"].bytes, base.trees["sibling"].bytes);
        let expected = projection(&[
            ("deep/a/b/edit.txt", HELLO_BLOB),
            ("sibling/other.txt", EMPTY_BLOB),
            ("deep/a/keep.bin", HELLO_BLOB),
            ("root.txt", EMPTY_BLOB),
        ]);
        assert_eq!(result.root(), expected.root());
        let unchanged = base.apply(&BTreeMap::new()).unwrap();
        assert!(unchanged.changed(&base).is_empty());
        let restored = result
            .apply(&BTreeMap::from([(
                "deep/a/b/edit.txt".into(),
                Some(EMPTY_BLOB.into()),
            )]))
            .unwrap();
        assert_eq!(restored.root(), base.root());
        assert!(restored.changed(&base).is_empty());
    }

    #[test]
    fn sparse_projection_deletes_empty_directories_and_adds_siblings() {
        let base = projection(&[("gone/last.txt", EMPTY_BLOB), ("stay/file.txt", EMPTY_BLOB)]);
        let result = base
            .apply(&BTreeMap::from([
                ("gone/last.txt".into(), None),
                ("new/nested/á file.txt".into(), Some(HELLO_BLOB.into())),
                ("stay/file.txt".into(), Some(HELLO_BLOB.into())),
            ]))
            .unwrap();
        assert!(!result.trees.contains_key("gone"));
        assert_eq!(
            result.root(),
            projection(&[
                ("new/nested/á file.txt", HELLO_BLOB),
                ("stay/file.txt", HELLO_BLOB)
            ])
            .root()
        );
        let empty = base
            .apply(&BTreeMap::from([
                ("gone/last.txt".into(), None),
                ("stay/file.txt".into(), None),
            ]))
            .unwrap();
        assert_eq!(empty.root(), EMPTY_TREE);
        assert_eq!(empty.trees.len(), 1);
        let replaced = base
            .apply(&BTreeMap::from([
                ("gone/last.txt".into(), None),
                ("gone".into(), Some(HELLO_BLOB.into())),
            ]))
            .unwrap();
        assert_eq!(
            replaced.root(),
            projection(&[("gone", HELLO_BLOB), ("stay/file.txt", EMPTY_BLOB)]).root()
        );
    }

    #[test]
    fn sparse_projection_preserves_conflicts_aliases_depth_and_byte_limits() {
        let base = projection(&[("keep/file.txt", EMPTY_BLOB)]);
        for changes in [
            BTreeMap::from([("keep".into(), Some(HELLO_BLOB.into()))]),
            BTreeMap::from([("keep/file.txt/more".into(), Some(HELLO_BLOB.into()))]),
        ] {
            assert!(matches!(base.apply(&changes), Err(GitError::Conflict)));
        }
        assert!(matches!(
            base.apply(&BTreeMap::from([(
                "KEEP/FILE.TXT".into(),
                Some(HELLO_BLOB.into())
            )])),
            Err(GitError::Unsupported)
        ));
        assert!(matches!(
            base.apply(&BTreeMap::from([(
                "KEEP/other.txt".into(),
                Some(HELLO_BLOB.into())
            )])),
            Err(GitError::Unsupported)
        ));
        assert!(matches!(
            base.apply(&BTreeMap::from([("Keep".into(), Some(HELLO_BLOB.into()))])),
            Err(GitError::Unsupported)
        ));
        let accented = projection(&[("Á/file.txt", EMPTY_BLOB)]);
        assert!(matches!(
            accented.apply(&BTreeMap::from([(
                "á/other.txt".into(),
                Some(HELLO_BLOB.into())
            )])),
            Err(GitError::Unsupported)
        ));
        assert!(matches!(
            base.apply(&BTreeMap::from([(
                format!("{}file", "d/".repeat(33)),
                Some(HELLO_BLOB.into())
            )])),
            Err(GitError::Limit)
        ));
        let mut changes = BTreeMap::new();
        for i in 0..300 {
            changes.insert(
                format!("{i:04}{}", "n".repeat(4000)),
                Some(HELLO_BLOB.into()),
            );
        }
        assert!(matches!(base.apply(&changes), Err(GitError::Limit)));
    }

    fn stored_object(
        fixture: &super::super::executor::MetadataFixture,
        root: &Path,
        kind: &str,
        body: &[u8],
    ) -> String {
        if body.len() > 1_048_576 {
            assert_eq!(kind, "blob");
            assert!(body.len() <= 6 * 1_048_576);
            let path = root.join("large-owned-object-input");
            std::fs::write(&path, body).unwrap();
            let args = [
                "hash-object",
                "-w",
                "--no-filters",
                "--",
                "large-owned-object-input",
            ]
            .map(std::ffi::OsString::from);
            let output = fixture
                .executor
                .run(root, &args)
                .unwrap()
                .checked()
                .unwrap();
            std::fs::remove_file(path).unwrap();
            return String::from_utf8(output).unwrap().trim().to_owned();
        }
        let args = ["hash-object", "-t", kind, "-w", "--stdin"].map(std::ffi::OsString::from);
        let output = fixture
            .executor
            .run_with_input(root, &args, body)
            .unwrap()
            .checked()
            .unwrap();
        String::from_utf8(output).unwrap().trim().to_owned()
    }

    // Independent serialization/order oracle: Git mktree reads typed, NUL-
    // terminated entries. It does not use this module's frame/hash serializer.
    fn git_oracle_tree(root: &Path, files: &BTreeMap<String, String>) -> String {
        use std::{
            io::{Read, Write},
            process::{Command, Stdio},
            thread,
            time::{Duration, Instant},
        };
        let mut direct = BTreeMap::new();
        let mut directories: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (path, oid) in files {
            if let Some((name, tail)) = path.split_once('/') {
                directories
                    .entry(name.into())
                    .or_default()
                    .insert(tail.into(), oid.clone());
            } else {
                direct.insert(path.clone(), oid.clone());
            }
        }
        let mut input = Vec::new();
        for (name, oid) in direct {
            input.extend_from_slice(format!("100644 blob {oid}\t{name}\0").as_bytes());
        }
        for (name, children) in directories {
            input.extend_from_slice(
                format!("040000 tree {}\t{name}\0", git_oracle_tree(root, &children)).as_bytes(),
            );
        }
        assert!(input.len() <= MAX_PRESERVATION_BYTES);
        let mut command = Command::new("git");
        command.env_clear();
        for key in ["PATH", "SystemRoot", "WINDIR", "SystemDrive", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", null)
            .env("GIT_CONFIG_GLOBAL", null)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .args(["mktree", "-z"])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = thread::spawn(move || {
            let mut b = Vec::new();
            stdout.by_ref().take(1_048_577).read_to_end(&mut b).unwrap();
            b
        });
        let err = thread::spawn(move || {
            let mut b = Vec::new();
            stderr.by_ref().take(1_048_577).read_to_end(&mut b).unwrap();
            b
        });
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(15) {
                child.kill().unwrap();
                let status = child.wait().unwrap();
                break status;
            }
            thread::sleep(Duration::from_millis(5));
        };
        let stdout = out.join().unwrap();
        let stderr = err.join().unwrap();
        assert!(stdout.len() <= 1_048_576 && stderr.len() <= 1_048_576);
        assert!(
            status.success(),
            "owned Git mktree failed: {}",
            String::from_utf8_lossy(&stderr)
        );
        String::from_utf8(stdout).unwrap().trim().to_owned()
    }

    fn object_inventory(root: &Path) -> BTreeMap<std::path::PathBuf, String> {
        fn visit(root: &Path, out: &mut BTreeMap<std::path::PathBuf, String>) {
            for entry in std::fs::read_dir(root).unwrap() {
                let entry = entry.unwrap();
                let meta = entry.file_type().unwrap();
                assert!(!meta.is_symlink());
                if meta.is_dir() {
                    visit(&entry.path(), out);
                } else {
                    let bytes = std::fs::read(entry.path()).unwrap();
                    assert!(bytes.len() <= 8 * 1_048_576);
                    out.insert(entry.path(), crate::git_records::content_hash(&bytes));
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(&root.join(".git/objects"), &mut result);
        result
    }

    #[test]
    fn sparse_projection_matches_independent_git_mktree_and_verifies_without_writes() {
        let fixture = super::super::executor::MetadataFixture::new();
        let root = fixture.repository("tree-oracle");
        let original = stored_object(&fixture, &root, "blob", b"original\n");
        let edited = stored_object(&fixture, &root, "blob", b"edited\n");
        let binary = stored_object(&fixture, &root, "blob", &[0, 255, 2, 128]);
        let large = stored_object(&fixture, &root, "blob", &vec![b'x'; 2 * 1_048_576]);
        let canary = stored_object(
            &fixture,
            &root,
            "blob",
            b"API_KEY=fixture-only-secret-canary\n",
        );
        let files = BTreeMap::from([
            ("deep/a/b/edit.txt".into(), original.clone()),
            ("sibling/edit.txt".into(), original.clone()),
            ("gone/last.txt".into(), original.clone()),
            ("foo.bar".into(), original.clone()),
            ("foobar".into(), original.clone()),
            ("foo/baz".into(), original.clone()),
            ("á space/keep.bin".into(), binary),
            ("assets/large.txt".into(), large),
            ("assets/canary.txt".into(), canary),
        ]);
        let base_oid = git_oracle_tree(&root, &files);
        let entries = files.iter().map(|(p, o)| entry(p, o)).collect::<Vec<_>>();
        let base = inventory_projection(&base_oid, &entries).unwrap();
        base.verify_objects(&fixture.executor, &root).unwrap();
        let changes = BTreeMap::from([
            ("deep/a/b/edit.txt".into(), Some(edited.clone())),
            ("sibling/edit.txt".into(), Some(edited.clone())),
            ("gone/last.txt".into(), None),
            ("new/é file.txt".into(), Some(edited)),
        ]);
        let result = base.apply(&changes).unwrap();
        let mut final_files = files;
        for (p, o) in changes {
            if let Some(o) = o {
                final_files.insert(p, o);
            } else {
                final_files.remove(&p);
            }
        }
        assert_eq!(result.root(), git_oracle_tree(&root, &final_files));
        assert_eq!(result.trees["assets"].oid, base.trees["assets"].oid);
        assert_eq!(result.trees["á space"].oid, base.trees["á space"].oid);
        assert!(!result.trees.contains_key("gone"));
        let before = object_inventory(&root);
        result.verify_objects(&fixture.executor, &root).unwrap();
        assert_eq!(
            object_inventory(&root),
            before,
            "verification wrote an object"
        );
        let full = fixture
            .executor
            .run(
                &root,
                &["ls-tree", "-r", "-z", result.root()].map(std::ffi::OsString::from),
            )
            .unwrap()
            .checked()
            .unwrap();
        assert_eq!(
            full.split(|b| *b == 0).filter(|r| !r.is_empty()).count(),
            final_files.len()
        );
        for row in full.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let row = std::str::from_utf8(row).unwrap();
            let (meta, path) = row.split_once('\t').unwrap();
            assert_eq!(meta, format!("100644 blob {}", final_files[path]));
        }
    }

    fn replace_owned_loose(root: &Path, expected: &str, wrong: &str) {
        let objects = root.join(".git/objects");
        let path = objects.join(&expected[..2]).join(&expected[2..]);
        assert!(path.starts_with(&objects) && path.is_file());
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Only a confirmed disposable fixture loose object is made writable for corruption testing"
        )]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        std::fs::copy(objects.join(&wrong[..2]).join(&wrong[2..]), path).unwrap();
    }

    fn make_owned_loose_writable(root: &Path, oid: &str) {
        let objects = root.join(".git/objects");
        let path = objects.join(&oid[..2]).join(&oid[2..]);
        assert!(path.starts_with(&objects) && path.is_file());
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "Only a confirmed disposable fixture loose object is restored for the missing-object test"
        )]
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn reused_tree_missing_or_valid_body_under_wrong_oid_is_not_accepted() {
        let fixture = super::super::executor::MetadataFixture::new();
        let root = fixture.repository("tree-integrity");
        let blob = stored_object(&fixture, &root, "blob", b"a\n");
        let other = stored_object(&fixture, &root, "blob", b"b\n");
        let files = BTreeMap::from([("protected/child.txt".into(), blob)]);
        let tree = git_oracle_tree(&root, &files);
        let projected = inventory_projection(
            &tree,
            &files.iter().map(|(p, o)| entry(p, o)).collect::<Vec<_>>(),
        )
        .unwrap();
        let child = projected.trees["protected"].oid.clone();
        let other_tree = git_oracle_tree(&root, &BTreeMap::from([("child.txt".into(), other)]));
        projected.verify_objects(&fixture.executor, &root).unwrap();
        let object = root
            .join(".git/objects")
            .join(&child[..2])
            .join(&child[2..]);
        let original = std::fs::read(&object).unwrap();
        replace_owned_loose(&root, &child, &other_tree);
        assert_eq!(
            projected.verify_objects(&fixture.executor, &root),
            Err(GitError::InconsistentCapture)
        );
        make_owned_loose_writable(&root, &child);
        std::fs::write(&object, original).unwrap();
        std::fs::remove_file(&object).unwrap();
        assert_eq!(
            projected.verify_objects(&fixture.executor, &root),
            Err(GitError::InconsistentCapture)
        );
    }

    #[test]
    fn pinned_commit_and_blob_native_integrity_are_not_inferred_from_oid_names() {
        let fixture = super::super::executor::MetadataFixture::new();
        let root = fixture.repository("commit-blob-integrity");
        let a = stored_object(&fixture, &root, "blob", b"a\n");
        let b = stored_object(&fixture, &root, "blob", b"b\n");
        let tree = git_oracle_tree(&root, &BTreeMap::from([("file.txt".into(), a.clone())]));
        let body = format!(
            "tree {tree}\nauthor QA <qa@invalid.example> 1 +0000\ncommitter QA <qa@invalid.example> 1 +0000\n\nExact native object\n"
        );
        let commit = stored_object(&fixture, &root, "commit", body.as_bytes());
        let wrong = stored_object(
            &fixture,
            &root,
            "commit",
            body.replace("Exact native object", "Other native object")
                .as_bytes(),
        );
        assert_eq!(base_tree(&fixture.executor, &root, &commit), Ok(tree));
        replace_owned_loose(&root, &commit, &wrong);
        assert_eq!(
            base_tree(&fixture.executor, &root, &commit),
            Err(GitError::InconsistentCapture)
        );
        replace_owned_loose(&root, &a, &b);
        assert_eq!(
            verify_objects(&fixture.executor, &root, &[("blob", &a, b"a\n")]),
            Err(GitError::InconsistentCapture)
        );
        std::fs::remove_file(root.join(".git/objects").join(&a[..2]).join(&a[2..])).unwrap();
        assert_eq!(
            verify_objects(&fixture.executor, &root, &[("blob", &a, b"a\n")]),
            Err(GitError::InconsistentCapture)
        );
    }
}
