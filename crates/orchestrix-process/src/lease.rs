//! Short API, long-lived host ownership. No database transaction or PID probing is used.
use crate::{ProcessError, Result};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};
pub struct HostLease {
    _file: File,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn writer_lease_cannot_be_acquired_twice_and_releases_without_deleting_the_marker() {
        let root = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = root.join(format!(
            "orchestrix-host-lease-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = HostLease::acquire(&path).unwrap().unwrap();
        assert!(HostLease::acquire(&path).unwrap().is_none());
        drop(first);
        let second = HostLease::acquire(&path).unwrap().unwrap();
        drop(second);
        assert!(path.is_file());
        assert_eq!(path.parent(), Some(root.as_path()));
        std::fs::remove_file(path).unwrap();
    }
}
impl HostLease {
    /// None means contention. A killed host releases the OS handle automatically; the empty marker stays.
    pub fn acquire(path: &Path) -> Result<Option<Self>> {
        if !path.is_absolute() || !path.parent().is_some_and(Path::is_dir) {
            return Err(ProcessError::Arguments);
        }
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(ProcessError::Arguments);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(ProcessError::Arguments);
                }
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(1)
                .open(path)
            {
                Ok(file) => Ok(Some(Self { _file: file })),
                Err(error) if error.raw_os_error() == Some(32) => Ok(None),
                Err(_) => Err(ProcessError::Ownership),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = OpenOptions::new();
            Err(ProcessError::Unsupported)
        }
    }
}
