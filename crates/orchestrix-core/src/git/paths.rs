//! Filesystem admission for the explicit local Git scope.
use super::{GitError, GitResult};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Reject links in every existing component, including Windows junctions and
/// other reparse points. This is validation, not an OS-level filesystem sandbox.
pub(crate) fn reject_links(path: &Path) -> GitResult<()> {
    validate_absolute(path)?;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        // A Windows drive prefix alone (C: or \\?\C:) is drive-relative;
        // inspect it only after its RootDir component has been appended.
        if matches!(component, Component::Prefix(_)) || !current.is_absolute() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                    return Err(GitError::Link);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(GitError::Io),
        }
    }
    Ok(())
}

/// Git for Windows' MSYS argument conversion does not accept Rust's verbatim
/// drive prefix as a worktree argument. Keep canonical paths for filesystem
/// checks/storage and remove only that prefix at the Git argv boundary.
pub(crate) fn git_path(path: &Path) -> GitResult<std::ffi::OsString> {
    validate_absolute(path)?;
    let text = path.to_str().ok_or(GitError::Path)?;
    #[cfg(windows)]
    let text = text.strip_prefix("\\\\?\\").unwrap_or(text);
    Ok(std::ffi::OsString::from(text))
}

pub(crate) fn canonical_directory(path: &Path) -> GitResult<PathBuf> {
    reject_links(path)?;
    let result = fs::canonicalize(path).map_err(|_| GitError::Path)?;
    reject_links(&result)?;
    if !result.is_dir() {
        return Err(GitError::Path);
    }
    Ok(result)
}

/// Return a canonical existing descendant, or a new direct child of an existing
/// canonical parent. Equality with the authorized root is deliberately refused.
pub(crate) fn authorized_child(root: &Path, path: &Path, existing: bool) -> GitResult<PathBuf> {
    let root = canonical_directory(root)?;
    reject_links(path)?;
    let resolved = if existing {
        fs::canonicalize(path).map_err(|_| GitError::Path)?
    } else {
        if fs::symlink_metadata(path).is_ok() {
            return Err(GitError::Collision);
        }
        let parent = canonical_directory(path.parent().ok_or(GitError::Path)?)?;
        parent.join(path.file_name().ok_or(GitError::Path)?)
    };
    if !strict_descendant(&root, &resolved) {
        return Err(GitError::Scope);
    }
    reject_links(&resolved)?;
    Ok(resolved)
}

pub(crate) fn strict_descendant(root: &Path, candidate: &Path) -> bool {
    let mut candidate_parts = candidate.components();
    for root_part in root.components() {
        let Some(candidate_part) = candidate_parts.next() else {
            return false;
        };
        if !same_component(root_part, candidate_part) {
            return false;
        }
    }
    candidate_parts.next().is_some()
}

fn same_component(left: Component<'_>, right: Component<'_>) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub(super) fn validate_absolute(path: &Path) -> GitResult<()> {
    if !path.is_absolute() {
        return Err(GitError::Path);
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => return Err(GitError::Path),
            Component::Normal(name) => {
                let name = name.to_str().ok_or(GitError::Path)?;
                if name.is_empty() || name.chars().any(|value| value.is_control()) {
                    return Err(GitError::Path);
                }
                #[cfg(windows)]
                {
                    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                    if name.contains(':')
                        || name.ends_with(['.', ' '])
                        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        || (stem.len() == 4
                            && (stem.starts_with("COM") || stem.starts_with("LPT"))
                            && stem.as_bytes()[3].is_ascii_digit())
                    {
                        return Err(GitError::Path);
                    }
                }
            }
            #[cfg(windows)]
            Component::Prefix(prefix)
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) =>
            {
                return Err(GitError::Unsupported);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_uses_components_and_rejects_root_and_sibling_prefix() {
        let root = std::env::temp_dir().join("orchestrix-paths-test");
        assert!(!strict_descendant(&root, &root));
        assert!(strict_descendant(&root, &root.join("child")));
        assert!(!strict_descendant(
            &root,
            &std::env::temp_dir().join("orchestrix-paths-test-evil")
        ));
        assert_eq!(reject_links(Path::new("relative")), Err(GitError::Path));
        assert_eq!(
            reject_links(&root.join("..").join("outside")),
            Err(GitError::Path)
        );
    }

    #[test]
    #[cfg(windows)]
    fn junction_and_equivalent_existing_paths_are_checked() {
        use std::{
            os::windows::process::CommandExt,
            process::Command,
            time::{SystemTime, UNIX_EPOCH},
        };
        let temporary = canonical_directory(&std::env::temp_dir()).unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = temporary.join(format!("orchestrix-paths-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let target = root.join("target");
        let junction = root.join("junction");
        fs::create_dir(&target).unwrap();
        // PowerShell is used only to prepare this owned Windows fixture;
        // production Git execution never invokes cmd or another shell.
        let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = Command::new(shell)
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:ORCHESTRIX_TEST_JUNCTION -Target $env:ORCHESTRIX_TEST_TARGET | Out-Null"])
            .env("ORCHESTRIX_TEST_JUNCTION", git_path(&junction).unwrap())
            .env("ORCHESTRIX_TEST_TARGET", git_path(&target).unwrap())
            .current_dir(&root)
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(reject_links(&junction), Err(GitError::Link));
        assert_eq!(reject_links(&junction.join("child")), Err(GitError::Link));
        assert_eq!(
            authorized_child(&root, &junction, true),
            Err(GitError::Link)
        );
        assert_eq!(authorized_child(&root, &root, true), Err(GitError::Scope));
        assert_eq!(
            authorized_child(&root, &root.join("new"), false).unwrap(),
            root.join("new")
        );
        let ordinary = PathBuf::from(git_path(&target).unwrap());
        assert_eq!(canonical_directory(&ordinary).unwrap(), target);
        // Remove the junction itself without following it, then the empty owned
        // fixture directories. No recursive cleanup or arbitrary target paths.
        assert!(strict_descendant(&temporary, &root));
        fs::remove_dir(&junction).unwrap();
        reject_links(&target).unwrap();
        fs::remove_dir(&target).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
