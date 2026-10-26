//! One coherent Windows capture observation, using retained directory handles.
//!
//! Each native relative open contains exactly one validated component and uses
//! FILE_OPEN_REPARSE_POINT. Held handles are not immutable: sharing permits
//! writes/deletes, every traversed directory is checked again, and `finish`
//! resolves all retained directory names and the absolute root afresh. This is
//! not a filesystem sandbox or an atomic snapshot against hostile writers.
//!
//! Native contracts (no private NT flags or additional dependency):
//! https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
use super::{GitError, GitResult, paths};
use orchestrix_process::windows_file_read::{self as native, FileIdentity, ReadOnlyFileError};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};

const MAX_DIRECTORIES: usize = 4096;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

struct Directory {
    file: File,
    identity: FileIdentity,
    parent: Option<usize>,
    component: String,
}

/// A bounded set of actual directories for one `inspect_at_profile` call.
/// Callers must invoke `finish` before accepting any captured bytes.
pub(super) struct ScopedRead {
    root: PathBuf,
    directories: Vec<Directory>,
    by_relative_path: BTreeMap<String, usize>,
}

impl ScopedRead {
    pub(super) fn new(root: &Path) -> GitResult<Self> {
        // Keep the previous absolute-path admission, including drive ancestry,
        // aliases, device names, junctions and all other reparse tags.
        let root = paths::canonical_directory(root)?;
        let file = open_root(&root)?;
        let identity = directory_identity(&file)?;
        paths::reject_links(&root)?;
        let fresh = open_root(&root)?;
        if directory_identity(&fresh)? != identity {
            return Err(GitError::InconsistentCapture);
        }
        Ok(Self {
            root,
            directories: vec![Directory {
                file,
                identity,
                parent: None,
                component: String::new(),
            }],
            by_relative_path: BTreeMap::new(),
        })
    }

    pub(super) fn open(&mut self, relative: &str) -> GitResult<Option<File>> {
        let components = relative_components(&self.root, relative)?;
        let Some(parent) = self.parent(&components)? else {
            return Ok(None);
        };
        let file = open_child(
            &self.directories[parent].file,
            components[components.len() - 1],
            false,
            true,
        )?;
        if let Some(file) = &file {
            file_metadata(file)?;
            identity(file)?;
        }
        Ok(file)
    }

    pub(super) fn verify_file(&mut self, relative: &str, file: &File) -> GitResult<Metadata> {
        let components = relative_components(&self.root, relative)?;
        let parent = self
            .parent(&components)?
            .ok_or(GitError::InconsistentCapture)?;
        // Inspect the original handle too: reparse attributes are mutable, and
        // sharing modes deliberately do not claim to freeze the file object.
        file_metadata(file)?;
        let original = identity(file)?;
        let fresh = open_child(
            &self.directories[parent].file,
            components[components.len() - 1],
            false,
            false,
        )?
        .ok_or(GitError::InconsistentCapture)?;
        let metadata = file_metadata(&fresh)?;
        if identity(&fresh)? != original {
            return Err(GitError::InconsistentCapture);
        }
        Ok(metadata)
    }

    pub(super) fn finish(&self) -> GitResult<()> {
        paths::reject_links(&self.root)?;
        let fresh_root = open_root(&self.root)?;
        if directory_identity(&fresh_root)? != self.directories[0].identity {
            return Err(GitError::InconsistentCapture);
        }
        // Entries are inserted after their parent. Verify both the held object
        // and its current name binding; neither the map nor a prior ID lookup is
        // accepted as current metadata authority.
        for directory in &self.directories {
            if directory_identity(&directory.file)? != directory.identity {
                return Err(GitError::InconsistentCapture);
            }
            if let Some(parent) = directory.parent {
                let parent = &self.directories[parent];
                if directory_identity(&parent.file)? != parent.identity {
                    return Err(GitError::InconsistentCapture);
                }
                let fresh = open_child(&parent.file, &directory.component, true, false)?
                    .ok_or(GitError::InconsistentCapture)?;
                if directory_identity(&fresh)? != directory.identity {
                    return Err(GitError::InconsistentCapture);
                }
            }
        }
        // A root replacement/reparse at the closing edge must still block even
        // if all relative handles continue to refer to the original objects.
        paths::reject_links(&self.root)?;
        let closing_root = open_root(&self.root)?;
        if directory_identity(&closing_root)? != self.directories[0].identity {
            return Err(GitError::InconsistentCapture);
        }
        Ok(())
    }

    fn parent(&mut self, components: &[&str]) -> GitResult<Option<usize>> {
        let mut parent = 0;
        let mut key = String::new();
        self.verify_directory(parent)?;
        for component in &components[..components.len() - 1] {
            if !key.is_empty() {
                key.push('/');
            }
            key.push_str(component);
            if let Some(&existing) = self.by_relative_path.get(&key) {
                if self.directories[existing].parent != Some(parent) {
                    return Err(GitError::InconsistentCapture);
                }
                self.verify_directory(existing)?;
                parent = existing;
            } else {
                directory_capacity(self.directories.len())?;
                let Some(file) =
                    open_child(&self.directories[parent].file, component, true, false)?
                else {
                    // Negative lookups are never retained as authority.
                    return Ok(None);
                };
                let identity = directory_identity(&file)?;
                let next = self.directories.len();
                self.directories.push(Directory {
                    file,
                    identity,
                    parent: Some(parent),
                    component: (*component).to_owned(),
                });
                self.by_relative_path.insert(key.clone(), next);
                parent = next;
            }
        }
        // This also covers a leaf directly below the root and the final parent
        // after the last relative open. No metadata observation survives a call.
        self.verify_directory(parent)?;
        Ok(Some(parent))
    }

    fn verify_directory(&self, index: usize) -> GitResult<()> {
        let directory = &self.directories[index];
        if directory_identity(&directory.file)? != directory.identity {
            return Err(GitError::InconsistentCapture);
        }
        Ok(())
    }
}

fn directory_capacity(current: usize) -> GitResult<()> {
    if current >= MAX_DIRECTORIES {
        Err(GitError::Limit)
    } else {
        Ok(())
    }
}

fn relative_components<'a>(root: &Path, relative: &'a str) -> GitResult<Vec<&'a str>> {
    if !crate::git_records::relative_file_path(relative) {
        return Err(GitError::Path);
    }
    // This admission is pure lexical validation. It retains the exact Windows
    // device/trailing-alias rules formerly reached through reject_links without
    // performing another ancestor metadata walk for every preserved file.
    paths::validate_absolute(&root.join(relative))?;
    let components = relative.split('/').collect::<Vec<_>>();
    for component in &components {
        validate_component(component)?;
    }
    Ok(components)
}

fn validate_component(component: &str) -> GitResult<()> {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.eq_ignore_ascii_case(".git")
        || component
            .chars()
            .any(|c| c.is_control() || "<>\"|?*/\\:".contains(c))
    {
        return Err(GitError::Path);
    }
    Ok(())
}

fn native_error(error: ReadOnlyFileError) -> GitError {
    match error {
        ReadOnlyFileError::Path => GitError::Path,
        ReadOnlyFileError::Link => GitError::Link,
        ReadOnlyFileError::Unsupported => GitError::Unsupported,
        ReadOnlyFileError::Io => GitError::Io,
    }
}

fn open_root(root: &Path) -> GitResult<File> {
    paths::validate_absolute(root)?;
    native::open_root(root).map_err(native_error)
}

fn open_child(parent: &File, name: &str, directory: bool, read: bool) -> GitResult<Option<File>> {
    validate_component(name)?;
    native::open_child(parent, name, directory, read).map_err(native_error)
}

fn identity(file: &File) -> GitResult<FileIdentity> {
    native::identity(file).map_err(native_error)
}

fn metadata(file: &File) -> GitResult<Metadata> {
    let metadata = file.metadata().map_err(|_| GitError::Io)?;
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(GitError::Link);
    }
    Ok(metadata)
}

fn directory_identity(file: &File) -> GitResult<FileIdentity> {
    if !metadata(file)?.is_dir() {
        return Err(GitError::Unsupported);
    }
    identity(file)
}

fn file_metadata(file: &File) -> GitResult<Metadata> {
    let metadata = metadata(file)?;
    if !metadata.is_file() {
        return Err(GitError::Unsupported);
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::Read,
        os::windows::process::CommandExt,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        temporary: PathBuf,
        basename: String,
        container: PathBuf,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NONCE: AtomicU64 = AtomicU64::new(0);
            let temporary = paths::canonical_directory(&std::env::temp_dir()).unwrap();
            let name = format!(
                "orchestrix-scoped-read-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NONCE.fetch_add(1, Ordering::Relaxed)
            );
            let container = temporary.join(&name);
            assert!(paths::strict_descendant(&temporary, &container));
            fs::create_dir(&container).unwrap();
            let root = container.join("repository");
            fs::create_dir(&root).unwrap();
            Self {
                temporary,
                basename: name,
                container,
                root,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // This known absolute owned container is never renamed or made a
            // reparse point by these tests. Fixtures only replace descendants.
            let metadata = fs::symlink_metadata(&self.container).ok();
            let safe_container = metadata.is_some_and(|m| {
                m.is_dir()
                    && !m.file_type().is_symlink()
                    && m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
            });
            if !self.container.is_absolute()
                || self.container != self.temporary.join(&self.basename)
                || self.container.parent() != Some(self.temporary.as_path())
                || self.container.file_name() != Some(std::ffi::OsStr::new(&self.basename))
                || !self.basename.starts_with("orchestrix-scoped-read-")
                || !safe_container
                || paths::canonical_directory(&self.temporary).ok().as_ref()
                    != Some(&self.temporary)
                || paths::canonical_directory(&self.container).ok().as_ref()
                    != Some(&self.container)
            {
                return;
            }
            let _ = fs::remove_dir_all(&self.container);
        }
    }

    fn junction(link: &Path, target: &Path) {
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = Command::new(shell)
            .args([
                "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
                "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:ORCHESTRIX_SCOPED_LINK -Target $env:ORCHESTRIX_SCOPED_TARGET | Out-Null",
            ])
            .env("ORCHESTRIX_SCOPED_LINK", paths::git_path(link).unwrap())
            .env("ORCHESTRIX_SCOPED_TARGET", paths::git_path(target).unwrap())
            .current_dir(link.parent().unwrap())
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "owned junction fixture could not be created"
        );
    }

    #[test]
    fn nested_files_are_read_and_verified_without_sharing_a_content_cache() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("nested")).unwrap();
        fs::write(fixture.root.join("nested/one.txt"), b"first").unwrap();
        fs::write(fixture.root.join("nested/two.txt"), b"second").unwrap();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        for (path, expected) in [("nested/one.txt", "first"), ("nested/two.txt", "second")] {
            let mut file = scope.open(path).unwrap().unwrap();
            let mut bytes = String::new();
            file.read_to_string(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(
                scope.verify_file(path, &file).unwrap().len(),
                expected.len() as u64
            );
        }
        assert_eq!(
            scope.directories.len(),
            2,
            "one actual nested directory is retained"
        );
        scope.finish().unwrap();
        fs::write(fixture.root.join("nested/one.txt"), b"newer").unwrap();
        let mut next = ScopedRead::new(&fixture.root).unwrap();
        let mut bytes = String::new();
        next.open("nested/one.txt")
            .unwrap()
            .unwrap()
            .read_to_string(&mut bytes)
            .unwrap();
        assert_eq!(
            bytes, "newer",
            "another observation performs another file read"
        );
        next.finish().unwrap();
    }

    #[test]
    fn forbidden_paths_and_windows_device_aliases_are_rejected_before_open() {
        let fixture = Fixture::new();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        for path in [
            "",
            ".",
            "..",
            "../outside",
            "/absolute",
            "C:/absolute",
            "a\\b",
            "a//b",
            "a/../b",
            "a/./b",
            ".git/config",
            "a/.GIT/config",
            "a\0b",
            "a\nb",
            "a:stream",
            "CON",
            "con.txt",
            "PRN",
            "AUX",
            "NUL",
            "COM0",
            "COM9.txt",
            "LPT1",
            "LPT9.log",
            "a.",
            "a ",
            "nested./file",
            "nested /file",
            "a?b",
            "a*b",
            "a<b",
            "a>b",
            "a|b",
            "a\"b",
        ] {
            assert!(
                matches!(scope.open(path), Err(GitError::Path)),
                "accepted forbidden path"
            );
        }
        scope.finish().unwrap();
    }

    #[test]
    fn missing_leaf_and_parent_are_observed_without_negative_cache() {
        let fixture = Fixture::new();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        assert!(scope.open("missing.txt").unwrap().is_none());
        assert!(scope.open("missing/leaf.txt").unwrap().is_none());
        fs::create_dir(fixture.root.join("missing")).unwrap();
        fs::write(fixture.root.join("missing/leaf.txt"), b"available").unwrap();
        assert!(scope.open("missing/leaf.txt").unwrap().is_some());
        scope.finish().unwrap();
    }

    #[test]
    fn directory_junction_is_rejected_as_parent_leaf_and_root() {
        let fixture = Fixture::new();
        let outside = fixture.container.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("private.txt"), b"not-to-be-read").unwrap();
        let link = fixture.root.join("junction");
        junction(&link, &outside);
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        assert!(matches!(
            scope.open("junction/private.txt"),
            Err(GitError::Link | GitError::Unsupported)
        ));
        assert!(scope.open("junction").is_err());
        assert!(matches!(ScopedRead::new(&link), Err(GitError::Link)));
        scope.finish().unwrap();
        // Remove only the junction entry itself, never recurse into its target.
        assert_eq!(link.parent(), Some(fixture.root.as_path()));
        fs::remove_dir(&link).unwrap();
    }

    #[test]
    fn same_size_leaf_replacement_is_detected_by_native_identity() {
        let fixture = Fixture::new();
        let path = fixture.root.join("file.txt");
        fs::write(&path, b"aaaa").unwrap();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        let file = scope.open("file.txt").unwrap().unwrap();
        fs::rename(&path, fixture.root.join("old.txt")).unwrap();
        fs::write(&path, b"bbbb").unwrap();
        assert_eq!(
            scope.verify_file("file.txt", &file).unwrap_err(),
            GitError::InconsistentCapture
        );
    }

    #[test]
    fn retained_directory_name_replacement_fails_the_closing_fence() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("nested")).unwrap();
        fs::write(fixture.root.join("nested/file.txt"), b"same").unwrap();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        let file = scope.open("nested/file.txt").unwrap().unwrap();
        // Production closes each streamed leaf before the directory fence.
        // An open descendant itself prevents this rename on Windows.
        drop(file);
        fs::rename(fixture.root.join("nested"), fixture.root.join("original")).unwrap();
        fs::create_dir(fixture.root.join("nested")).unwrap();
        fs::write(fixture.root.join("nested/file.txt"), b"same").unwrap();
        assert_eq!(scope.finish().unwrap_err(), GitError::InconsistentCapture);
    }

    #[test]
    fn root_name_replacement_fails_even_with_original_handles_alive() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("file.txt"), b"same").unwrap();
        let mut scope = ScopedRead::new(&fixture.root).unwrap();
        let file = scope.open("file.txt").unwrap().unwrap();
        drop(file);
        fs::rename(&fixture.root, fixture.container.join("original-root")).unwrap();
        fs::create_dir(&fixture.root).unwrap();
        fs::write(fixture.root.join("file.txt"), b"same").unwrap();
        assert_eq!(scope.finish().unwrap_err(), GitError::InconsistentCapture);
    }

    #[test]
    fn directory_resource_bounds_are_explicit() {
        assert_eq!(directory_capacity(MAX_DIRECTORIES - 1), Ok(()));
        assert_eq!(directory_capacity(MAX_DIRECTORIES), Err(GitError::Limit));
        assert_eq!(
            directory_capacity(MAX_DIRECTORIES + 1),
            Err(GitError::Limit)
        );
    }
}
