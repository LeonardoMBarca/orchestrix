// Reused read-only handle checks from the preserved v1 proof; no provider or writes.
use crate::{MAXIMUM_FILE_BYTES, RELATIVE_PATH, error, size_rejection, verify_bytes};
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    device: u64,
    identity: u64,
    mode: u64,
    size: u64,
    modified: i128,
    changed: i128,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    };
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct HandleInformation {
        attributes: u32,
        created: FileTime,
        accessed: FileTime,
        written: FileTime,
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct BasicInformation {
        created: i64,
        accessed: i64,
        written: i64,
        changed: i64,
        attributes: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut HandleInformation,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            handle: *mut std::ffi::c_void,
            class: i32,
            information: *mut std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    pub fn link(metadata: &fs::Metadata) -> bool {
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    pub fn file(file: &File) -> io::Result<Fingerprint> {
        let mut information = HandleInformation::default();
        let mut basic = BasicInformation::default();
        // Read-only handle metadata: no process, write, registry or network API.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0
            || unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    0,
                    (&mut basic as *mut BasicInformation).cast(),
                    std::mem::size_of::<BasicInformation>() as u32,
                )
            } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Fingerprint {
            device: information.volume as u64,
            identity: ((information.index_high as u64) << 32) | information.index_low as u64,
            mode: (information.attributes & 1) as u64,
            size: ((information.size_high as u64) << 32) | information.size_low as u64,
            modified: basic.written as i128,
            changed: basic.changed as i128,
        })
    }
    pub fn path(path: &Path) -> io::Result<Fingerprint> {
        let handle = fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x0020_0000)
            .open(path)?;
        file(&handle)
    }
}
#[cfg(unix)]
mod platform {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    pub fn link(metadata: &fs::Metadata) -> bool {
        metadata.file_type().is_symlink()
    }
    fn metadata(value: fs::Metadata) -> Fingerprint {
        Fingerprint {
            device: value.dev(),
            identity: value.ino(),
            mode: value.mode() as u64,
            size: value.size(),
            modified: value.mtime() as i128 * 1_000_000_000 + value.mtime_nsec() as i128,
            changed: value.ctime() as i128 * 1_000_000_000 + value.ctime_nsec() as i128,
        }
    }
    pub fn file(file: &File) -> io::Result<Fingerprint> {
        file.metadata().map(metadata)
    }
    pub fn path(path: &Path) -> io::Result<Fingerprint> {
        fs::symlink_metadata(path).map(metadata)
    }
}

fn read_fixed(root: &Path) -> io::Result<Value> {
    for directory in [root.to_path_buf(), root.join("docs")] {
        let m = fs::symlink_metadata(directory)?;
        if platform::link(&m) || !m.is_dir() {
            return Ok(error("path-not-regular"));
        }
    }
    let target = root.join(RELATIVE_PATH);
    let m = fs::symlink_metadata(&target)?;
    if platform::link(&m) || !m.is_file() {
        return Ok(error("path-not-regular"));
    }
    if m.len() > MAXIMUM_FILE_BYTES as u64 {
        return Ok(size_rejection(m.len()));
    }
    let initial = platform::path(&target)?;
    let mut file = File::open(&target)?;
    let before = platform::file(&file)?;
    if initial != before {
        return Ok(error("file-changed-during-read"));
    }
    let mut buffer = [0; MAXIMUM_FILE_BYTES + 1];
    let mut count = 0;
    while count < buffer.len() {
        let n = file.read(&mut buffer[count..])?;
        if n == 0 {
            break;
        }
        count += n;
    }
    let after = platform::file(&file)?;
    let final_meta = fs::symlink_metadata(&target)?;
    let final_id = platform::path(&target)?;
    if before != after
        || after != final_id
        || platform::link(&final_meta)
        || !final_meta.is_file()
        || count as u64 != after.size
    {
        return Ok(error("file-changed-during-read"));
    }
    Ok(verify_bytes(&buffer[..count]))
}
pub fn inspect() -> Value {
    match std::env::current_dir().and_then(|root| read_fixed(&root)) {
        Ok(v) => v,
        Err(e) => error(match e.kind() {
            io::ErrorKind::NotFound => "file-not-found",
            io::ErrorKind::PermissionDenied => "permission-denied",
            _ => "file-read-failed",
        }),
    }
}
