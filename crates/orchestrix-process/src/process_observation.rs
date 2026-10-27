//! Bounded, read-only image observations from an already owned `Child`.
//!
//! This borrows the process handle returned by creation. It never opens a PID,
//! scans processes, grants ownership, waits for exit, or proves tree closure.
//! Native contracts:
//! https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew
//! https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessid
//! https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes

use std::{fmt, path::PathBuf, process::Child};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    Unsupported,
    ProcessId(u32),
    IdentityMismatch,
    ImageQuery(u32),
    ImagePath,
}
impl fmt::Display for ObservationError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Unsupported => "child-observation-unsupported",
            Self::ProcessId(_) => "child-observation-process-id",
            Self::IdentityMismatch => "child-observation-identity-mismatch",
            Self::ImageQuery(_) => "child-observation-image-query",
            Self::ImagePath => "child-observation-image-path",
        })
    }
}
impl std::error::Error for ObservationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessObservation {
    /// Queried through the live borrowed handle, checked against `Child::id()`.
    pub process_id: u32,
    /// Win32 image path reported for this handle, without lossy conversion.
    pub image_path: PathBuf,
    /// Reported FILETIME: 100 ns units since 1601-01-01 UTC, not a duration.
    pub creation_time_100ns: Option<u64>,
    /// GetProcessTimes failure code, preserving the image observation when the
    /// optional creation-time query is unavailable. No default time is invented.
    pub creation_time_error: Option<u32>,
}

/// Makes at most three fixed native queries using only the owned child handle.
/// A successful image observation identifies the root process, not a launcher
/// descendant. An exited/unqueryable process remains an explicit error.
pub fn observe_child(child: &Child) -> Result<ProcessObservation, ObservationError> {
    #[cfg(windows)]
    {
        windows::observe(child)
    }
    #[cfg(not(windows))]
    {
        let _ = child;
        Err(ObservationError::Unsupported)
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::{OsString, c_void},
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };

    const IMAGE_CAPACITY: usize = 32_768;
    type Handle = *mut c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetProcessId(process: Handle) -> u32;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            image: *mut u16,
            characters: *mut u32,
        ) -> i32;
        fn GetProcessTimes(
            process: Handle,
            creation: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
        fn GetLastError() -> u32;
    }

    pub(super) fn observe(child: &Child) -> Result<ProcessObservation, ObservationError> {
        let handle = child.as_raw_handle();
        // SAFETY: Child owns this process handle and remains borrowed throughout
        // all queries. No ownership transfer or CloseHandle is performed here.
        let process_id = unsafe { GetProcessId(handle) };
        if process_id == 0 {
            // SAFETY: Read the failure code immediately on the calling thread.
            return Err(ObservationError::ProcessId(unsafe { GetLastError() }));
        }
        if process_id != child.id() {
            return Err(ObservationError::IdentityMismatch);
        }
        let mut buffer = vec![0u16; IMAGE_CAPACITY];
        let mut characters = IMAGE_CAPACITY as u32;
        // SAFETY: The live handle is borrowed, and the fixed initialized buffer
        // has exactly the declared character capacity. Flags 0 requests Win32
        // paths; insufficient capacity is returned, never retried unboundedly.
        if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut characters) }
            == 0
        {
            // SAFETY: As above, before another native call overwrites the code.
            return Err(ObservationError::ImageQuery(unsafe { GetLastError() }));
        }
        let image_path = image_path(&buffer, characters as usize)?;
        let (mut creation, mut exit, mut kernel, mut user) = (0u64, 0u64, 0u64, 0u64);
        // SAFETY: Each initialized, aligned u64 has FILETIME's eight-byte output
        // storage on supported little-endian Windows targets. This matches the
        // crate's existing native declaration. Exit time is not interpreted: it
        // is undefined before exit. CPU values are not wall durations.
        let times_ok =
            unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
        let (creation_time_100ns, creation_time_error) = if times_ok == 0 {
            // SAFETY: Read the error immediately after the failed query.
            (None, Some(unsafe { GetLastError() }))
        } else {
            (Some(creation), None)
        };
        Ok(ProcessObservation {
            process_id,
            image_path,
            creation_time_100ns,
            creation_time_error,
        })
    }

    fn image_path(buffer: &[u16], characters: usize) -> Result<PathBuf, ObservationError> {
        if characters == 0
            || characters >= buffer.len()
            || buffer[characters] != 0
            || buffer[..characters].contains(&0)
        {
            return Err(ObservationError::ImagePath);
        }
        let path = PathBuf::from(OsString::from_wide(&buffer[..characters]));
        if !path.is_absolute() {
            return Err(ObservationError::ImagePath);
        }
        Ok(path)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{
            fs,
            io::{Read, Write},
            os::windows::{ffi::OsStrExt, process::CommandExt},
            process::{Command, Stdio},
        };

        struct ChildGuard(Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                if !matches!(self.0.try_wait(), Ok(Some(_))) {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
        }

        #[test]
        fn image_decoding_rejects_empty_truncated_relative_and_embedded_nul() {
            assert_eq!(image_path(&[0], 0), Err(ObservationError::ImagePath));
            assert_eq!(
                image_path(&[b'x' as u16], 1),
                Err(ObservationError::ImagePath)
            );
            assert_eq!(
                image_path(&[b'x' as u16, 0], 1),
                Err(ObservationError::ImagePath)
            );
            let mut valid = OsString::from("C:\\owned\\child.exe")
                .encode_wide()
                .collect::<Vec<_>>();
            valid.push(0);
            assert_eq!(
                image_path(&valid, valid.len() - 1).unwrap(),
                PathBuf::from("C:\\owned\\child.exe")
            );
            valid[4] = 0;
            assert_eq!(
                image_path(&valid, valid.len() - 1),
                Err(ObservationError::ImagePath)
            );
        }

        #[test]
        fn observes_owned_child_image_and_creation_time_without_consuming_handle() {
            let command = std::env::current_exe().unwrap();
            let arguments = [
                "--exact",
                "process_observation::windows::tests::observation_child_fixture",
                "--ignored",
            ];
            let mut child = ChildGuard(
                Command::new(&command)
                    .args(arguments)
                    .env("ORCHESTRIX_PROCESS_OBSERVATION_CHILD", "owned-child")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(0x0800_0000)
                    .spawn()
                    .unwrap(),
            );
            let observed = observe_child(&child.0).unwrap();
            assert_eq!(observed.process_id, child.0.id());
            assert_eq!(
                fs::canonicalize(&observed.image_path).unwrap(),
                fs::canonicalize(&command).unwrap()
            );
            assert!(observed.creation_time_100ns.is_some_and(|time| time > 0));
            assert_eq!(observed.creation_time_error, None);
            // The child remains live during observation; only this byte permits
            // its exact fixture to exit. Guard cleanup also covers assert panic.
            child.0.stdin.take().unwrap().write_all(b"!").unwrap();
            let status = child.0.wait().unwrap();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            child
                .0
                .stdout
                .take()
                .unwrap()
                .take(4096)
                .read_to_end(&mut stdout)
                .unwrap();
            child
                .0
                .stderr
                .take()
                .unwrap()
                .take(4096)
                .read_to_end(&mut stderr)
                .unwrap();
            assert!(
                status.success(),
                "owned child failed: {status:?}, code {:?}, command {command:?}, args {arguments:?}, stdout {:?}, stderr {:?}",
                status.code(),
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            // The caller can still use its process handle after observation.
            match observe_child(&child.0) {
                Ok(after_exit) => assert_eq!(after_exit, observed),
                // Windows can stop exposing the image after exit even though
                // the borrowed handle's PID query still succeeds. Preserve the
                // explicit native failure instead of fabricating an image.
                Err(ObservationError::ImageQuery(code)) => assert_ne!(code, 0),
                Err(error) => panic!("unexpected post-exit observation error: {error:?}"),
            }
        }

        #[test]
        #[ignore = "only launched by the exact owned-child observation test"]
        fn observation_child_fixture() {
            assert_eq!(
                std::env::var("ORCHESTRIX_PROCESS_OBSERVATION_CHILD").as_deref(),
                Ok("owned-child")
            );
            let mut release = [0u8; 1];
            std::io::stdin().read_exact(&mut release).unwrap();
            assert_eq!(release, [b'!']);
            std::process::exit(0);
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_does_not_fabricate_an_image() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        assert_eq!(observe_child(&child), Err(ObservationError::Unsupported));
        let _ = child.wait().unwrap();
    }
}
