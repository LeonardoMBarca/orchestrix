use super::{Diagnostic, Error, MAX_FRAME, Result, checked_deadline, record_diagnostic};
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::Arc,
    time::{Duration, Instant},
};
type Handle = *mut c_void;
const INVALID: Handle = -1isize as Handle;
#[repr(C)]
struct Security {
    size: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct Overlapped {
    internal: usize,
    high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: Handle) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn DuplicateHandle(
        source_process: Handle,
        source: Handle,
        target_process: Handle,
        target: *mut Handle,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetLastError() -> u32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn CreateNamedPipeW(
        name: *const u16,
        access: u32,
        mode: u32,
        instances: u32,
        out: u32,
        input: u32,
        timeout: u32,
        security: *const Security,
    ) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlap: *mut Overlapped) -> i32;
    fn DisconnectNamedPipe(pipe: Handle) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        sharing: u32,
        security: *const c_void,
        creation: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16)
    -> Handle;
    fn WaitForSingleObject(handle: Handle, timeout: u32) -> u32;
    fn CancelIoEx(handle: Handle, overlap: *mut Overlapped) -> i32;
    fn GetOverlappedResult(
        handle: Handle,
        overlap: *mut Overlapped,
        bytes: *mut u32,
        wait: i32,
    ) -> i32;
    fn ReadFile(
        handle: Handle,
        buffer: *mut c_void,
        size: u32,
        bytes: *mut u32,
        overlap: *mut Overlapped,
    ) -> i32;
    fn WriteFile(
        handle: Handle,
        buffer: *const c_void,
        size: u32,
        bytes: *mut u32,
        overlap: *mut Overlapped,
    ) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(
        token: Handle,
        class: u32,
        buffer: *mut c_void,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: *const c_void, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
}
struct Owned(Handle);
// These OS handles own their operations; Stream serializes access and no I/O outlives its call.
unsafe impl Send for Owned {}
unsafe impl Sync for Owned {}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn user(process: Handle) -> Result<String> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(process, 8, &mut token) } == 0 {
        return Err(Error::Identity);
    }
    let token = Owned(token);
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token.0, 1, null_mut(), 0, &mut needed);
    }
    if needed == 0 || needed > 65_536 {
        return Err(Error::Identity);
    }
    // usize allocation supplies TOKEN_USER/SID pointer alignment.
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    if unsafe { GetTokenInformation(token.0, 1, buffer.as_mut_ptr().cast(), needed, &mut needed) }
        == 0
    {
        return Err(Error::Identity);
    }
    let sid = buffer[0] as *const c_void;
    let mut text = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(Error::Identity);
    }
    let mut count = 0;
    while count < 256 && unsafe { *text.add(count) } != 0 {
        count += 1;
    }
    let result = if count == 256 {
        Err(Error::Identity)
    } else {
        String::from_utf16(unsafe { std::slice::from_raw_parts(text, count) })
            .map_err(|_| Error::Identity)
    };
    unsafe {
        LocalFree(text.cast());
    }
    result
}
pub fn current_user_scope() -> Result<String> {
    user(unsafe { GetCurrentProcess() })
}
fn endpoint(scope: &str) -> Result<Vec<u16>> {
    if scope.len() != 64 || !scope.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Identity);
    }
    Ok(wide(&format!(r"\\.\pipe\orchestrix-v1-{scope}")))
}
struct ServerPipe {
    handle: Owned,
}
pub struct Listener {
    pipe: Arc<ServerPipe>,
}
pub struct Stream {
    handle: Arc<ServerPipe>,
    server: bool,
}
impl Drop for Stream {
    fn drop(&mut self) {
        if self.server {
            unsafe {
                DisconnectNamedPipe(self.handle.handle.0);
            }
        }
    }
}
impl Listener {
    pub fn bind(scope: &str) -> Result<Self> {
        let name = endpoint(scope)?;
        let sddl = wide(&format!("D:P(A;;GA;;;{})", current_user_scope()?));
        let mut descriptor = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(Error::Identity);
        }
        let security = Security {
            size: size_of::<Security>() as u32,
            descriptor,
            inherit: 0,
        };
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                3 | 0x40000000 | 0x80000,
                8,
                1,
                65_536,
                65_536,
                100,
                &security,
            )
        };
        unsafe {
            LocalFree(descriptor);
        }
        if handle == INVALID {
            return Err(if matches!(unsafe { GetLastError() }, 5 | 231) {
                Error::Busy
            } else {
                Error::Io
            });
        }
        Ok(Self {
            pipe: Arc::new(ServerPipe {
                handle: Owned(handle),
            }),
        })
    }
    pub fn accept(&mut self, timeout: Duration) -> Result<Option<Stream>> {
        if Arc::strong_count(&self.pipe) != 1 {
            return Err(Error::Busy);
        }
        let deadline = checked_deadline(timeout)?;
        let event = event()?;
        let mut op: Overlapped = unsafe { zeroed() };
        op.event = event.0;
        let result = unsafe { ConnectNamedPipe(self.pipe.handle.0, &mut op) };
        if result == 0 {
            match unsafe { GetLastError() } {
                535 => {}
                997 => match complete(
                    self.pipe.handle.0,
                    &mut op,
                    deadline,
                    "accept-pending",
                    Instant::now(),
                ) {
                    Ok(_) => {}
                    Err(Error::Deadline) => return Ok(None),
                    Err(e) => return Err(e),
                },
                _ => return Err(Error::Io),
            }
        }
        Ok(Some(Stream {
            handle: self.pipe.clone(),
            server: true,
        }))
    }
}
impl Stream {
    pub(crate) fn startup_writer(&self) -> Result<std::process::Stdio> {
        use std::os::windows::io::FromRawHandle;
        if self.server {
            return Err(Error::Identity);
        }
        let mut copied = null_mut();
        let process = unsafe { GetCurrentProcess() };
        if unsafe { DuplicateHandle(process, self.handle.handle.0, process, &mut copied, 0, 0, 2) }
            == 0
        {
            return Err(Error::Io);
        }
        Ok(std::process::Stdio::from(unsafe {
            std::fs::File::from_raw_handle(copied)
        }))
    }
    /// Single raw overlapped read for the private startup channel. Cancellation
    /// completion is dequeued before the buffer is returned or released.
    pub(crate) fn startup_read_chunk(
        &mut self,
        maximum: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        if !self.server || maximum == 0 || maximum > 2048 || timeout > Duration::from_millis(100) {
            return Err(Error::Budget);
        }
        let deadline = checked_deadline(timeout)?;
        let mut bytes = vec![0; maximum];
        super::clear_diagnostic();
        let count = match transfer(
            self.handle.handle.0,
            bytes.as_mut_ptr(),
            bytes.len(),
            false,
            deadline,
        ) {
            Ok(count) => count,
            Err(Error::Deadline) => {
                let Some(fact) = super::last_diagnostic() else {
                    return Err(Error::Deadline);
                };
                // A completion may win the race with cancellation. Those bytes
                // have been consumed and must remain in the incremental decoder.
                if fact.native_code.is_none() && fact.transferred_bytes > 0 {
                    fact.transferred_bytes as usize
                } else {
                    return Err(Error::Deadline);
                }
            }
            Err(error) => return Err(error),
        };
        if count == 0 {
            return Err(Error::Closed);
        }
        if count > maximum {
            return Err(Error::Budget);
        }
        bytes.truncate(count);
        Ok(bytes)
    }
    pub fn connect(scope: &str, timeout: Duration) -> Result<Self> {
        let deadline = checked_deadline(timeout)?;
        let name = endpoint(scope)?;
        loop {
            // Identification-level SQOS prevents a server from impersonating client credentials.
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    0xC0000000,
                    0,
                    null(),
                    3,
                    0x40000000 | 0x100000 | 0x10000,
                    null_mut(),
                )
            };
            if handle != INVALID {
                let owned = Owned(handle);
                let mut pid = 0;
                if unsafe { GetNamedPipeServerProcessId(handle, &mut pid) } == 0 {
                    return Err(Error::Identity);
                }
                let process = unsafe { OpenProcess(0x1000, 0, pid) };
                if process.is_null() {
                    return Err(Error::Identity);
                }
                let process = Owned(process);
                if user(process.0)? != current_user_scope()? {
                    return Err(Error::Identity);
                }
                return Ok(Self {
                    handle: Arc::new(ServerPipe { handle: owned }),
                    server: false,
                });
            }
            let code = unsafe { GetLastError() };
            if code != 231 && code != 2 {
                return Err(Error::Unavailable);
            }
            if Instant::now() >= deadline {
                return Err(Error::Deadline);
            }
            if code == 231 {
                unsafe {
                    WaitNamedPipeW(name.as_ptr(), 20);
                }
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    pub fn read(&mut self, timeout: Duration) -> Result<Vec<u8>> {
        let deadline = checked_deadline(timeout)?;
        let mut length = [0; 4];
        self.read_exact(&mut length, deadline)?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 || length > MAX_FRAME {
            return Err(Error::Budget);
        }
        let mut body = vec![0; length];
        self.read_exact(&mut body, deadline)?;
        Ok(body)
    }
    pub fn write(&mut self, body: &[u8], timeout: Duration) -> Result<()> {
        if body.is_empty() || body.len() > MAX_FRAME {
            return Err(Error::Budget);
        }
        let deadline = checked_deadline(timeout)?;
        self.write_all(&(body.len() as u32).to_le_bytes(), deadline)?;
        self.write_all(body, deadline)
    }
    fn read_exact(&self, mut buffer: &mut [u8], deadline: Instant) -> Result<()> {
        while !buffer.is_empty() {
            let n = transfer(
                self.handle.handle.0,
                buffer.as_mut_ptr(),
                buffer.len(),
                false,
                deadline,
            )?;
            if n == 0 {
                return Err(diagnostic(
                    "read-zero",
                    Error::Closed,
                    None,
                    None,
                    Instant::now(),
                    0,
                ));
            }
            buffer = &mut buffer[n..];
        }
        Ok(())
    }
    fn write_all(&self, mut buffer: &[u8], deadline: Instant) -> Result<()> {
        while !buffer.is_empty() {
            let n = transfer(
                self.handle.handle.0,
                buffer.as_ptr().cast_mut(),
                buffer.len(),
                true,
                deadline,
            )?;
            if n == 0 {
                return Err(diagnostic(
                    "write-zero",
                    Error::Closed,
                    None,
                    None,
                    Instant::now(),
                    0,
                ));
            }
            buffer = &buffer[n..];
        }
        Ok(())
    }
}
fn event() -> Result<Owned> {
    let e = unsafe { CreateEventW(null(), 1, 0, null()) };
    if e.is_null() {
        Err(Error::Io)
    } else {
        Ok(Owned(e))
    }
}
fn diagnostic(
    stage: &'static str,
    error: Error,
    native_code: Option<u32>,
    wait_result: Option<u32>,
    started: Instant,
    transferred_bytes: u32,
) -> Error {
    record_diagnostic(Diagnostic {
        stage,
        error,
        native_code,
        wait_result,
        elapsed_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        transferred_bytes,
    });
    error
}
fn complete(
    handle: Handle,
    op: &mut Overlapped,
    deadline: Instant,
    stage: &'static str,
    started: Instant,
) -> Result<usize> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let wait = unsafe {
        WaitForSingleObject(op.event, remaining.as_millis().min(u32::MAX as u128) as u32)
    };
    let mut bytes = 0;
    if wait != 0 {
        // Cancellation is not completion: dequeue before freeing OVERLAPPED and buffer.
        let result = unsafe {
            CancelIoEx(handle, op);
            GetOverlappedResult(handle, op, &mut bytes, 1)
        };
        let native_code = if result == 0 {
            Some(unsafe { GetLastError() })
        } else {
            None
        };
        return Err(diagnostic(
            stage,
            if wait == 258 {
                Error::Deadline
            } else {
                Error::Io
            },
            native_code,
            Some(wait),
            started,
            bytes,
        ));
    }
    if unsafe { GetOverlappedResult(handle, op, &mut bytes, 0) } == 0 {
        let code = unsafe { GetLastError() };
        return Err(diagnostic(
            stage,
            Error::Closed,
            Some(code),
            Some(wait),
            started,
            bytes,
        ));
    }
    Ok(bytes as usize)
}
fn transfer(
    handle: Handle,
    buffer: *mut u8,
    len: usize,
    write: bool,
    deadline: Instant,
) -> Result<usize> {
    let started = Instant::now();
    let stage = if write { "write" } else { "read" };
    if Instant::now() >= deadline {
        return Err(diagnostic(stage, Error::Deadline, None, None, started, 0));
    }
    let event = event()?;
    let mut op: Overlapped = unsafe { zeroed() };
    op.event = event.0;
    let result = unsafe {
        // FILE_FLAG_OVERLAPPED requires the byte-count parameter to be NULL.
        // GetOverlappedResult is authoritative even if the operation finishes
        // immediately (ReadFile/WriteFile documentation).
        if write {
            WriteFile(handle, buffer.cast(), len as u32, null_mut(), &mut op)
        } else {
            ReadFile(handle, buffer.cast(), len as u32, null_mut(), &mut op)
        }
    };
    if result != 0 {
        #[cfg(test)]
        observe_transfer(false);
        let mut bytes = 0;
        if unsafe { GetOverlappedResult(handle, &mut op, &mut bytes, 0) } == 0 {
            let code = unsafe { GetLastError() };
            return Err(diagnostic(
                stage,
                Error::Closed,
                Some(code),
                None,
                started,
                bytes,
            ));
        }
        return Ok(bytes as usize);
    }
    let code = unsafe { GetLastError() };
    if code != 997 {
        return Err(diagnostic(
            stage,
            Error::Closed,
            Some(code),
            None,
            started,
            0,
        ));
    }
    #[cfg(test)]
    observe_transfer(true);
    complete(
        handle,
        &mut op,
        deadline,
        if write {
            "write-pending"
        } else {
            "read-pending"
        },
        started,
    )
}

#[cfg(test)]
thread_local! {static TRANSFER_OBSERVER:std::cell::RefCell<Option<std::sync::mpsc::Sender<bool>>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
fn observe_transfer(pending: bool) {
    TRANSFER_OBSERVER.with(|slot| {
        if let Some(sender) = slot.borrow().as_ref() {
            let _ = sender.send(pending);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[repr(C)]
    struct SidAttributes {
        sid: *mut c_void,
        attributes: u32,
    }
    #[repr(C)]
    struct Acl {
        revision: u8,
        reserved: u8,
        size: u16,
        count: u16,
        reserved2: u16,
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn GetSecurityInfo(
            handle: Handle,
            kind: u32,
            information: u32,
            owner: *mut *mut c_void,
            group: *mut *mut c_void,
            dacl: *mut *mut Acl,
            sacl: *mut *mut c_void,
            descriptor: *mut *mut c_void,
        ) -> u32;
        fn GetAce(acl: *mut Acl, index: u32, ace: *mut *mut c_void) -> i32;
        fn EqualSid(left: *const c_void, right: *const c_void) -> i32;
        fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut c_void) -> i32;
        fn CreateRestrictedToken(
            existing: Handle,
            flags: u32,
            disabled_count: u32,
            disabled: *const SidAttributes,
            delete_count: u32,
            deleted: *const c_void,
            restricted_count: u32,
            restricted: *const c_void,
            new_token: *mut Handle,
        ) -> i32;
        fn SetThreadToken(thread: *const Handle, token: Handle) -> i32;
        fn DuplicateToken(existing: Handle, level: u32, token: *mut Handle) -> i32;
        fn RevertToSelf() -> i32;
    }
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            assert_ne!(unsafe { RevertToSelf() }, 0);
        }
    }
    #[test]
    fn immediate_and_pending_transfers_get_their_exact_completed_byte_count() {
        let unique = format!(
            "{:064x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut listener = Listener::bind(&unique).unwrap();
        let scope = unique.clone();
        let (buffered, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let client = std::thread::spawn(move || {
            let client = Stream::connect(&scope, Duration::from_secs(1)).unwrap();
            client
                .write_all(
                    b"buffered-byte-count",
                    Instant::now() + Duration::from_secs(1),
                )
                .unwrap();
            buffered.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(1)).unwrap();
            client
                .write_all(
                    b"pending-byte-count",
                    Instant::now() + Duration::from_secs(1),
                )
                .unwrap();
        });
        let stream = listener.accept(Duration::from_secs(1)).unwrap().unwrap();
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        let (events, observed) = std::sync::mpsc::channel();
        TRANSFER_OBSERVER.with(|slot| *slot.borrow_mut() = Some(events));
        let mut first = [0u8; 19];
        let count = transfer(
            stream.handle.handle.0,
            first.as_mut_ptr(),
            first.len(),
            false,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(count, 19);
        assert_eq!(&first, b"buffered-byte-count");
        assert!(
            !observed.recv_timeout(Duration::from_secs(1)).unwrap(),
            "prebuffered read must finish immediately"
        );
        let pending_observer = std::thread::spawn(move || {
            assert!(
                observed.recv_timeout(Duration::from_secs(1)).unwrap(),
                "unbuffered read must be pending before writer release"
            );
            release.send(()).unwrap();
        });
        let mut second = [0u8; 18];
        let count = transfer(
            stream.handle.handle.0,
            second.as_mut_ptr(),
            second.len(),
            false,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(count, 18);
        assert_eq!(&second, b"pending-byte-count");
        TRANSFER_OBSERVER.with(|slot| *slot.borrow_mut() = None);
        pending_observer.join().unwrap();
        client.join().unwrap();
    }
    #[test]
    fn immediate_and_pending_writes_report_the_completed_count_before_buffer_reuse() {
        let unique = format!(
            "{:064x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut listener = Listener::bind(&unique).unwrap();
        let (events, observed) = std::sync::mpsc::channel();
        let sender = std::thread::spawn(move || {
            let client = Stream::connect(&unique, Duration::from_secs(1)).unwrap();
            TRANSFER_OBSERVER.with(|slot| *slot.borrow_mut() = Some(events));
            let first = b"immediate-write-count";
            assert_eq!(
                transfer(
                    client.handle.handle.0,
                    first.as_ptr().cast_mut(),
                    first.len(),
                    true,
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap(),
                first.len()
            );
            let second = vec![0xA7; 131_072];
            assert_eq!(
                transfer(
                    client.handle.handle.0,
                    second.as_ptr().cast_mut(),
                    second.len(),
                    true,
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap(),
                second.len()
            );
            TRANSFER_OBSERVER.with(|slot| *slot.borrow_mut() = None);
        });
        let stream = listener.accept(Duration::from_secs(1)).unwrap().unwrap();
        assert!(
            !observed.recv_timeout(Duration::from_secs(1)).unwrap(),
            "small write must complete immediately with available buffer"
        );
        assert!(
            observed.recv_timeout(Duration::from_secs(1)).unwrap(),
            "write larger than pipe buffer must be pending before peer reads"
        );
        let mut first = [0u8; 21];
        stream
            .read_exact(&mut first, Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert_eq!(&first, b"immediate-write-count");
        let mut second = vec![0; 131_072];
        stream
            .read_exact(&mut second, Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert!(second.iter().all(|b| *b == 0xA7));
        sender.join().unwrap();
    }
    #[test]
    fn dacl_is_exact_current_user_and_disabled_user_token_cannot_connect() {
        let unique = format!(
            "{:064x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let listener = Listener::bind(&unique).unwrap();
        assert!(matches!(Listener::bind(&unique), Err(Error::Busy)));
        let sid_text = wide(&current_user_scope().unwrap());
        let mut sid = null_mut();
        assert_ne!(
            unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &mut sid) },
            0
        );
        let mut dacl = null_mut();
        let mut descriptor = null_mut();
        assert_eq!(
            unsafe {
                GetSecurityInfo(
                    listener.pipe.handle.0,
                    6,
                    4,
                    null_mut(),
                    null_mut(),
                    &mut dacl,
                    null_mut(),
                    &mut descriptor,
                )
            },
            0
        );
        assert!(!dacl.is_null());
        assert_eq!(unsafe { (*dacl).count }, 1);
        let mut ace = null_mut();
        assert_ne!(unsafe { GetAce(dacl, 0, &mut ace) }, 0);
        // ACCESS_ALLOWED_ACE has ACE_HEADER (4), ACCESS_MASK (4), then its SID.
        assert_eq!(unsafe { *(ace as *const u8) }, 0);
        assert_ne!(
            unsafe { EqualSid((ace as *const u8).add(8).cast(), sid) },
            0
        );
        unsafe {
            LocalFree(descriptor);
        }
        let mut token = null_mut();
        assert_ne!(
            unsafe { OpenProcessToken(GetCurrentProcess(), 8 | 2 | 4, &mut token) },
            0
        );
        let token = Owned(token);
        let mut restricted = null_mut();
        let disabled = SidAttributes { sid, attributes: 0 };
        assert_ne!(
            unsafe {
                CreateRestrictedToken(
                    token.0,
                    0,
                    1,
                    &disabled,
                    0,
                    null(),
                    0,
                    null(),
                    &mut restricted,
                )
            },
            0
        );
        let restricted = Owned(restricted);
        let mut impersonation = null_mut();
        assert_ne!(
            unsafe { DuplicateToken(restricted.0, 2, &mut impersonation) },
            0
        );
        let impersonation = Owned(impersonation);
        assert_ne!(
            unsafe { SetThreadToken(null(), impersonation.0) },
            0,
            "restricted-token impersonation error {}",
            unsafe { GetLastError() }
        );
        let revert = Revert;
        let name = endpoint(&unique).unwrap();
        let denied = unsafe {
            CreateFileW(
                name.as_ptr(),
                0xC0000000,
                0,
                null(),
                3,
                0x40000000,
                null_mut(),
            )
        };
        assert_eq!(denied, INVALID);
        assert_eq!(unsafe { GetLastError() }, 5);
        drop(revert);
        unsafe {
            LocalFree(sid);
        }
        // Reverting the thread restores this user's ordinary local access.
        let _client = Stream::connect(&unique, Duration::from_secs(1)).unwrap();
    }
    #[test]
    fn raw_frame_limit_deadline_and_reused_listener_are_enforced() {
        let unique = format!(
            "{:064x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut listener = Listener::bind(&unique).unwrap();
        assert!(listener.accept(Duration::from_millis(5)).unwrap().is_none());
        let scope = unique.clone();
        let sender = std::thread::spawn(move || {
            let client = Stream::connect(&scope, Duration::from_secs(1)).unwrap();
            client
                .write_all(
                    &((MAX_FRAME + 1) as u32).to_le_bytes(),
                    Instant::now() + Duration::from_secs(1),
                )
                .unwrap();
        });
        let mut stream = listener.accept(Duration::from_secs(1)).unwrap().unwrap();
        assert_eq!(stream.read(Duration::from_secs(1)), Err(Error::Budget));
        drop(stream);
        sender.join().unwrap();
        let scope = unique.clone();
        let stalled = std::thread::spawn(move || {
            let _client = Stream::connect(&scope, Duration::from_secs(1)).unwrap();
            std::thread::sleep(Duration::from_millis(100));
        });
        let mut stream = listener.accept(Duration::from_secs(1)).unwrap().unwrap();
        let before = Instant::now();
        assert_eq!(stream.read(Duration::from_millis(10)), Err(Error::Deadline));
        assert!(before.elapsed() < Duration::from_secs(1));
        drop(stream);
        stalled.join().unwrap();
        let sender = std::thread::spawn(move || {
            let mut client = Stream::connect(&unique, Duration::from_secs(1)).unwrap();
            client.write(b"request", Duration::from_secs(1)).unwrap();
            assert_eq!(client.read(Duration::from_secs(1)).unwrap(), b"response");
        });
        let mut stream = listener.accept(Duration::from_secs(1)).unwrap().unwrap();
        assert_eq!(stream.read(Duration::from_secs(1)).unwrap(), b"request");
        stream.write(b"response", Duration::from_secs(1)).unwrap();
        sender.join().unwrap();
    }
}
