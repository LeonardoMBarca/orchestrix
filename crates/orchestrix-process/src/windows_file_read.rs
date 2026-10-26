//! Read-only Windows file-handle primitives for one scoped Core observation.
//!
//! These functions do not grant filesystem authority. The caller owns scope,
//! coherent snapshots and closing name/identity fences. Sharing remains
//! READ|WRITE|DELETE and does not make a directory immutable or form a sandbox.
//!
//! The native contracts are documented by Microsoft:
//! https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntcreatefile
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex
use std::{
    ffi::c_void,
    fmt,
    fs::{File, Metadata},
    mem::size_of,
    os::windows::{
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, Prefix},
    ptr::{null, null_mut},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOnlyFileError {
    Path,
    Io,
    Unsupported,
    Link,
}
impl fmt::Display for ReadOnlyFileError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Path => "read-only-file-path",
            Self::Io => "read-only-file-io",
            Self::Unsupported => "read-only-file-unsupported",
            Self::Link => "read-only-file-link",
        })
    }
}
impl std::error::Error for ReadOnlyFileError {}
type Result<T> = std::result::Result<T, ReadOnlyFileError>;
type Handle = *mut c_void;

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_READ_DATA: u32 = 1;
const FILE_TRAVERSE: u32 = 0x20;
const FILE_READ_ATTRIBUTES: u32 = 0x80;
const SYNCHRONIZE: u32 = 0x0010_0000;
const SHARE_ALL: u32 = 1 | 2 | 4;
const FILE_OPEN: u32 = 1;
const FILE_DIRECTORY_FILE: u32 = 1;
const FILE_NON_DIRECTORY_FILE: u32 = 0x40;
const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const OBJ_CASE_INSENSITIVE: u32 = 0x40;
const FILE_ID_INFO_CLASS: u32 = 18;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}
#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: Handle,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}
#[repr(C)]
struct IoStatusBlock {
    // The native first member is a union of NTSTATUS and a pointer.
    status_or_pointer: usize,
    information: usize,
}

/// Native volume + 128-bit identifier. Fields are deliberately opaque; an
/// unavailable identity is an error, never a fabricated default identity.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    volume: u64,
    id: [u8; 16],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        handle: *mut Handle,
        access: u32,
        attributes: *mut ObjectAttributes,
        io_status: *mut IoStatusBlock,
        allocation_size: *const i64,
        file_attributes: u32,
        share_access: u32,
        disposition: u32,
        options: u32,
        extended_attributes: *const c_void,
        extended_attributes_length: u32,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandleEx(
        handle: Handle,
        class: u32,
        buffer: *mut c_void,
        bytes: u32,
    ) -> i32;
}

/// Opens an existing absolute Disk/VerbatimDisk directory without following
/// the final reparse point. The caller still validates the root's ancestry and
/// repeats its path/identity fence; an absolute open is not a read sandbox.
pub fn open_root(root: &Path) -> Result<File> {
    let native = root_name(root)?;
    let file = native_open(null_mut(), &native, true, false)?.ok_or(ReadOnlyFileError::Io)?;
    check_type(&file, true)?;
    Ok(file)
}

/// Opens exactly one component relative to an actual previously opened parent.
/// FILE_OPEN never creates/truncates a file; no write/delete access is requested.
/// A missing entry is distinct from a link/type/access/protocol failure.
pub fn open_child(parent: &File, name: &str, directory: bool, read: bool) -> Result<Option<File>> {
    component(name, true)?;
    if directory && read {
        return Err(ReadOnlyFileError::Path);
    }
    check_type(parent, true)?;
    let file = native_open(parent.as_raw_handle(), name, directory, read)?;
    if let Some(file) = &file {
        check_type(file, directory)?;
    }
    Ok(file)
}

/// Reads only native identification information from a live handle. Compare
/// both volume and ID; unsupported filesystems/handles do not receive a default.
pub fn identity(file: &File) -> Result<FileIdentity> {
    let mut identity = FileIdentity {
        volume: 0,
        id: [0; 16],
    };
    // SAFETY: The borrowed File handle remains live, and the initialized output
    // has FILE_ID_INFO's native layout and exact size. No content is accessed.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FILE_ID_INFO_CLASS,
            (&mut identity as *mut FileIdentity).cast(),
            size_of::<FileIdentity>() as u32,
        )
    };
    if ok == 0 || identity.id == [0; 16] {
        return Err(ReadOnlyFileError::Unsupported);
    }
    Ok(identity)
}

fn root_name(root: &Path) -> Result<String> {
    if !root.is_absolute() || root.as_os_str().len() > 4096 {
        return Err(ReadOnlyFileError::Path);
    }
    let mut parts = root.components();
    let drive = match parts.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(ReadOnlyFileError::Unsupported),
        },
        _ => return Err(ReadOnlyFileError::Path),
    };
    if !drive.is_ascii_alphabetic() || !matches!(parts.next(), Some(Component::RootDir)) {
        return Err(ReadOnlyFileError::Path);
    }
    // Construct only an NT drive path from parsed Windows components, rather
    // than accepting a renderer-supplied NT/object-manager namespace string.
    let mut native = format!("\\??\\{}:\\", char::from(drive));
    for part in parts {
        let Component::Normal(name) = part else {
            return Err(ReadOnlyFileError::Path);
        };
        let name = name.to_str().ok_or(ReadOnlyFileError::Path)?;
        component(name, false)?;
        if !native.ends_with('\\') {
            native.push('\\');
        }
        native.push_str(name);
    }
    Ok(native)
}

fn component(name: &str, reject_git: bool) -> Result<()> {
    if name.is_empty()
        || name.len() > 4096
        || matches!(name, "." | "..")
        || reject_git && name.eq_ignore_ascii_case(".git")
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>\"|?*/\\:".contains(c))
    {
        return Err(ReadOnlyFileError::Path);
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        return Err(ReadOnlyFileError::Path);
    }
    Ok(())
}

fn check_type(file: &File, directory: bool) -> Result<Metadata> {
    let metadata = file.metadata().map_err(|_| ReadOnlyFileError::Io)?;
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ReadOnlyFileError::Link);
    }
    if directory && !metadata.is_dir() || !directory && !metadata.is_file() {
        return Err(ReadOnlyFileError::Unsupported);
    }
    Ok(metadata)
}

fn native_open(parent: Handle, name: &str, directory: bool, read: bool) -> Result<Option<File>> {
    let mut wide = name.encode_utf16().collect::<Vec<_>>();
    let bytes = wide.len().checked_mul(2).ok_or(ReadOnlyFileError::Path)?;
    let length = u16::try_from(bytes).map_err(|_| ReadOnlyFileError::Path)?;
    let mut name = UnicodeString {
        length,
        maximum_length: length,
        buffer: wide.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: size_of::<ObjectAttributes>() as u32,
        root_directory: parent,
        object_name: &mut name,
        attributes: OBJ_CASE_INSENSITIVE,
        security_descriptor: null_mut(),
        security_quality_of_service: null_mut(),
    };
    let mut io_status = IoStatusBlock {
        status_or_pointer: 0,
        information: 0,
    };
    let mut handle = null_mut();
    let access = FILE_READ_ATTRIBUTES
        | SYNCHRONIZE
        | if directory {
            FILE_TRAVERSE
        } else if read {
            FILE_READ_DATA
        } else {
            0
        };
    // SAFETY: All pointers reference initialized live native-layout values. A
    // nonnull parent names exactly one validated component; only open_root uses
    // a null parent and a fully qualified NT drive path built by root_name.
    // FILE_OPEN, read-only access and synchronous I/O request no mutation.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &mut attributes,
            &mut io_status,
            null(),
            0,
            SHARE_ALL,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT
                | FILE_SYNCHRONOUS_IO_NONALERT
                | if directory {
                    FILE_DIRECTORY_FILE
                } else {
                    FILE_NON_DIRECTORY_FILE
                },
            null(),
            0,
        )
    };
    // On failure FileHandle has no documented owned value. Do not adopt or
    // close an arbitrary output value. Synchronous FILE_OPEN must complete as
    // STATUS_SUCCESS; pending/informational responses never grant a file.
    if status != 0 {
        return match status as u32 {
            0xc000_000f | 0xc000_0034 | 0xc000_003a => Ok(None),
            0xc000_0033 | 0xc000_003b | 0xc000_0106 => Err(ReadOnlyFileError::Path),
            0xc000_0024 | 0xc000_00ba | 0xc000_0103 => Err(ReadOnlyFileError::Unsupported),
            _ => Err(ReadOnlyFileError::Io),
        };
    }
    if handle.is_null() || handle as isize == -1 {
        return Err(ReadOnlyFileError::Io);
    }
    // SAFETY: STATUS_SUCCESS transferred one valid owned handle. File closes it
    // exactly once, including when the separate completion/type checks fail.
    let file = unsafe { File::from_raw_handle(handle) };
    if io_status.status_or_pointer as u32 != 0 {
        return Err(ReadOnlyFileError::Io);
    }
    Ok(Some(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_components_cannot_inject_paths_devices_or_aliases() {
        for name in [
            "", ".", "..", "a/b", "a\\b", "a:stream", ".git", ".GIT", "NUL", "con.txt", "COM0",
            "LPT9.log", "name.", "name ", "a?b", "a*b", "a\0b", "a\nb",
        ] {
            assert_eq!(component(name, true), Err(ReadOnlyFileError::Path));
        }
        assert_eq!(component("ordinary-file.txt", true), Ok(()));
    }

    #[test]
    fn native_root_namespace_only_comes_from_an_absolute_drive_path() {
        assert_eq!(
            root_name(Path::new("C:\\owned\\repository")).unwrap(),
            "\\??\\C:\\owned\\repository"
        );
        assert_eq!(
            root_name(Path::new("\\\\?\\C:\\owned\\repository")).unwrap(),
            "\\??\\C:\\owned\\repository"
        );
        for root in [
            "relative",
            "C:relative",
            "\\??\\C:\\owned",
            "C:\\owned\\..\\outside",
            "C:\\CON",
        ] {
            assert!(root_name(Path::new(root)).is_err());
        }
        assert!(matches!(
            root_name(Path::new("\\\\server\\share\\repository")),
            Err(ReadOnlyFileError::Unsupported)
        ));
        assert!(matches!(
            root_name(Path::new("\\\\.\\C:\\repository")),
            Err(ReadOnlyFileError::Unsupported)
        ));
    }

    #[test]
    fn native_struct_layouts_match_the_windows_abi() {
        assert_eq!(size_of::<FileIdentity>(), 24);
        assert_eq!(size_of::<IoStatusBlock>(), 2 * size_of::<usize>());
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<UnicodeString>(), 16);
            assert_eq!(size_of::<ObjectAttributes>(), 48);
        }
    }
}
