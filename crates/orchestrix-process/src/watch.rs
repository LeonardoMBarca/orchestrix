//! Conservative verification guard: any change under the worktree invalidates a check.
//! Outputs must be explicitly directed outside the worktree. This is not a sandbox.
use crate::{ProcessError, Result};
use std::path::Path;

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32)
        -> *mut c_void;
        fn FindCloseChangeNotification(handle: *mut c_void) -> i32;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    }
    pub struct DirectoryWatch {
        handle: usize,
    }
    impl DirectoryWatch {
        pub fn start(path: &Path) -> Result<Self> {
            if !path.is_absolute() || !path.is_dir() {
                return Err(ProcessError::Arguments);
            }
            let mut wide: Vec<_> = path.as_os_str().encode_wide().collect();
            if wide.contains(&0) {
                return Err(ProcessError::Arguments);
            }
            wide.push(0);
            // SAFETY: nul-terminated live UTF-16 path; returned handle is owned
            // exclusively here and closed with its matching Windows API.
            let handle =
                unsafe { FindFirstChangeNotificationW(wide.as_ptr(), 1, 1 | 2 | 4 | 8 | 16 | 256) };
            if handle.is_null() || handle as isize == -1 {
                return Err(ProcessError::Io);
            }
            Ok(Self {
                handle: handle as usize,
            })
        }
        pub fn changed(&self) -> Result<bool> {
            // The event stays signalled. Never reset/coalesce it into a clean result.
            match unsafe { WaitForSingleObject(self.handle as *mut c_void, 0) } {
                0 => Ok(true),
                258 => Ok(false),
                _ => Err(ProcessError::Io),
            }
        }
    }
    impl Drop for DirectoryWatch {
        fn drop(&mut self) {
            // SAFETY: this notification handle is uniquely owned and still open.
            unsafe {
                FindCloseChangeNotification(self.handle as *mut c_void);
            }
        }
    }
}
#[cfg(not(windows))]
mod platform {
    use super::*;
    pub struct DirectoryWatch;
    impl DirectoryWatch {
        pub fn start(_: &Path) -> Result<Self> {
            Err(ProcessError::Unsupported)
        }
        pub fn changed(&self) -> Result<bool> {
            Err(ProcessError::Unsupported)
        }
    }
}
pub use platform::DirectoryWatch;
