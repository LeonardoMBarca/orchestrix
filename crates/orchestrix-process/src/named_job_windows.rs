use super::JobObservation;
use crate::{ProcessError, Result};
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
type Handle = *mut c_void;
const JOB_ACCESS: u32 = 0x001f003f;
#[repr(C)]
struct Security {
    size: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct Acl {
    revision: u8,
    reserved: u8,
    size: u16,
    count: u16,
    reserved2: u16,
}
#[repr(C)]
struct Accounting {
    user: i64,
    kernel: i64,
    period_user: i64,
    period_kernel: i64,
    faults: u32,
    total: u32,
    active: u32,
    terminated: u32,
}
#[repr(C)]
struct Limits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    minimum: usize,
    maximum: usize,
    active_limit: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
struct Extended {
    basic: Limits,
    io: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(security: *const Security, name: *const u16) -> Handle;
    fn OpenJobObjectW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn SetLastError(error: u32);
    fn LocalFree(value: *mut c_void) -> *mut c_void;
    fn QueryInformationJobObject(
        job: Handle,
        class: i32,
        info: *mut c_void,
        size: u32,
        returned: *mut u32,
    ) -> i32;
    fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, size: u32) -> i32;
    fn TerminateJobObject(job: Handle, code: u32) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetCurrentProcessId() -> u32;
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
    fn GetProcessTimes(
        process: Handle,
        created: *mut u64,
        exit: *mut u64,
        kernel: *mut u64,
        user: *mut u64,
    ) -> i32;
    fn WaitForSingleObject(handle: Handle, timeout: u32) -> u32;
    fn CreateDirectoryW(path: *const u16, security: *const Security) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn GetSecurityInfo(
        handle: Handle,
        kind: u32,
        info: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut Acl,
        sacl: *mut *mut c_void,
        descriptor: *mut *mut c_void,
    ) -> u32;
    fn GetAce(acl: *mut Acl, index: u32, ace: *mut *mut c_void) -> i32;
    fn ConvertSidToStringSidW(sid: *const c_void, text: *mut *mut u16) -> i32;
    fn IsValidSid(sid: *const c_void) -> i32;
    fn IsValidAcl(acl: *const Acl) -> i32;
}
#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, size: u32, flags: u32) -> i32;
}
struct Owned(Handle);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Memory(*mut c_void);
impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn name(scope: &str) -> Vec<u16> {
    wide(&format!("Global\\orchestrix-job-v1-{scope}"))
}
fn sid_text(sid: *const c_void) -> Result<String> {
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(ProcessError::Ownership);
    }
    let mut text = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(ProcessError::Ownership);
    }
    let _memory = Memory(text.cast());
    let mut length = 0;
    while length < 256 && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    if length == 256 {
        return Err(ProcessError::Ownership);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
        .map_err(|_| ProcessError::Ownership)
}
pub(super) fn nonce() -> Result<String> {
    let mut bytes = [0u8; 32];
    if unsafe { BCryptGenRandom(null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, 2) } < 0 {
        return Err(ProcessError::Ownership);
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub(super) fn create(scope: &str) -> Result<Handle> {
    let user = crate::ipc::current_user_scope().map_err(|_| ProcessError::Ownership)?;
    let sddl = wide(&format!("O:{user}D:P(A;;0x{JOB_ACCESS:x};;;{user})"));
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
        return Err(ProcessError::Ownership);
    }
    let _memory = Memory(descriptor);
    let security = Security {
        size: size_of::<Security>() as u32,
        descriptor,
        inherit: 0,
    };
    unsafe {
        SetLastError(0);
    }
    let job = unsafe { CreateJobObjectW(&security, name(scope).as_ptr()) };
    let error = unsafe { GetLastError() };
    if job.is_null() {
        return Err(ProcessError::Ownership);
    }
    if error == 183 {
        unsafe {
            CloseHandle(job);
        }
        return Err(ProcessError::Ownership);
    }
    Ok(job)
}
pub(super) fn inspect(scope: &str) -> Result<JobObservation> {
    let job = unsafe { OpenJobObjectW(4 | 0x00020000, 0, name(scope).as_ptr()) };
    if job.is_null() {
        return if unsafe { GetLastError() } == 2 {
            Ok(JobObservation::Absent)
        } else {
            Err(ProcessError::Ownership)
        };
    }
    let job = Owned(job);
    validate(&job)?;
    let count = active(&job)?;
    Ok(if count == 0 {
        JobObservation::Empty
    } else {
        JobObservation::Active(count)
    })
}
fn validate(job: &Owned) -> Result<()> {
    let user = crate::ipc::current_user_scope().map_err(|_| ProcessError::Ownership)?;
    let mut owner = null_mut();
    let mut acl = null_mut();
    let mut descriptor = null_mut();
    let code = unsafe {
        GetSecurityInfo(
            job.0,
            6,
            1 | 4,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        )
    };
    if code != 0 {
        return Err(ProcessError::Ownership);
    }
    let _memory = Memory(descriptor);
    if sid_text(owner)? != user
        || acl.is_null()
        || unsafe { IsValidAcl(acl) } == 0
        || unsafe { (*acl).count } != 1
    {
        return Err(ProcessError::Ownership);
    }
    let mut ace = null_mut();
    if unsafe { GetAce(acl, 0, &mut ace) } == 0 || ace.is_null() {
        return Err(ProcessError::Ownership);
    }
    let pointer = ace.cast::<u8>();
    if unsafe { *pointer } != 0
        || unsafe { *pointer.add(1) } != 0
        || unsafe { std::ptr::read_unaligned(pointer.add(4).cast::<u32>()) } != JOB_ACCESS
        || sid_text(unsafe { pointer.add(8).cast() })? != user
    {
        return Err(ProcessError::Ownership);
    }
    let mut limits: Extended = unsafe { zeroed() };
    if unsafe {
        QueryInformationJobObject(
            job.0,
            9,
            (&mut limits as *mut Extended).cast(),
            size_of::<Extended>() as u32,
            null_mut(),
        )
    } == 0
        || limits.basic.flags != 0x2000
    {
        return Err(ProcessError::Ownership);
    }
    Ok(())
}
fn active(job: &Owned) -> Result<u32> {
    let mut count: Accounting = unsafe { zeroed() };
    if unsafe {
        QueryInformationJobObject(
            job.0,
            1,
            (&mut count as *mut Accounting).cast(),
            size_of::<Accounting>() as u32,
            null_mut(),
        )
    } == 0
    {
        return Err(ProcessError::Ownership);
    }
    Ok(count.active)
}
pub(super) struct Scope(Owned);
// Immutable owned Job handles are queried/duplicated by thread-safe kernel APIs.
unsafe impl Send for Scope {}
unsafe impl Sync for Scope {}
impl Scope {
    pub(super) fn create(scope: &str) -> Result<Self> {
        let job = Owned(create(scope)?);
        let mut limits: Extended = unsafe { zeroed() };
        limits.basic.flags = 0x2000;
        if unsafe {
            SetInformationJobObject(
                job.0,
                9,
                (&limits as *const Extended).cast(),
                size_of::<Extended>() as u32,
            )
        } == 0
        {
            return Err(ProcessError::Ownership);
        }
        validate(&job)?;
        Ok(Self(job))
    }
    pub(super) fn open(scope: &str) -> Result<Self> {
        let job = unsafe { OpenJobObjectW(2 | 4 | 8 | 0x00020000, 0, name(scope).as_ptr()) };
        if job.is_null() {
            return Err(ProcessError::Ownership);
        }
        let job = Owned(job);
        validate(&job)?;
        Ok(Self(job))
    }
    pub(super) fn active(&self) -> Result<u32> {
        active(&self.0)
    }
    pub(super) fn terminate(&self) -> Result<()> {
        if unsafe { TerminateJobObject(self.0.0, 137) } == 0 {
            Err(ProcessError::Ownership)
        } else {
            Ok(())
        }
    }
    pub(super) fn seal(&self) -> Result<()> {
        let mut limits: Extended = unsafe { zeroed() };
        limits.basic.flags = 0x2000 | 8;
        limits.basic.active_limit = 0;
        if unsafe {
            SetInformationJobObject(
                self.0.0,
                9,
                (&limits as *const Extended).cast(),
                size_of::<Extended>() as u32,
            )
        } == 0
        {
            return Err(ProcessError::Ownership);
        }
        let mut actual: Extended = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.0.0,
                9,
                (&mut actual as *mut Extended).cast(),
                size_of::<Extended>() as u32,
                null_mut(),
            )
        } == 0
            || actual.basic.flags != 0x2000 | 8
            || actual.basic.active_limit != 0
        {
            return Err(ProcessError::Ownership);
        }
        Ok(())
    }
    pub(super) fn duplicate(&self) -> Result<Handle> {
        let mut result = null_mut();
        let current = unsafe { GetCurrentProcess() };
        if unsafe { DuplicateHandle(current, self.0.0, current, &mut result, 0, 0, 2) } == 0 {
            return Err(ProcessError::Ownership);
        }
        Ok(result)
    }
}
fn created(process: Handle) -> Result<u64> {
    let (mut c, mut e, mut k, mut u) = (0, 0, 0, 0);
    if unsafe { GetProcessTimes(process, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return Err(ProcessError::Ownership);
    }
    Ok(c)
}
pub(super) fn host_identity() -> Result<(u32, u64)> {
    Ok((
        unsafe { GetCurrentProcessId() },
        created(unsafe { GetCurrentProcess() })?,
    ))
}
pub(super) struct HostWatch(Owned);
impl HostWatch {
    pub(super) fn open(pid: u32, creation: u64) -> Result<Self> {
        let process = unsafe { OpenProcess(0x00100000 | 0x1000, 0, pid) };
        if process.is_null() {
            return Err(ProcessError::Ownership);
        }
        let owned = Owned(process);
        if created(process)? != creation {
            return Err(ProcessError::Ownership);
        }
        Ok(Self(owned))
    }
    pub(super) fn exited(&self) -> Result<bool> {
        match unsafe { WaitForSingleObject(self.0.0, 0) } {
            0 => Ok(true),
            258 => Ok(false),
            _ => Err(ProcessError::Ownership),
        }
    }
}
pub(super) fn private_directory(path: &std::path::Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    if !path.is_absolute() {
        return Err(ProcessError::Arguments);
    }
    let user = crate::ipc::current_user_scope().map_err(|_| ProcessError::Ownership)?;
    let sddl = wide(&format!("O:{user}D:P(A;OICI;FA;;;{user})"));
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
        return Err(ProcessError::Ownership);
    }
    let _memory = Memory(descriptor);
    let security = Security {
        size: size_of::<Security>() as u32,
        descriptor,
        inherit: 0,
    };
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    if unsafe { CreateDirectoryW(path.as_ptr(), &security) } == 0 {
        return Err(ProcessError::Ownership);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[repr(C)]
    struct SidAttributes {
        sid: *mut c_void,
        attributes: u32,
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
        fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut c_void) -> i32;
        fn CreateRestrictedToken(
            token: Handle,
            flags: u32,
            disabled_count: u32,
            disabled: *const SidAttributes,
            delete_count: u32,
            deleted: *const c_void,
            restricted_count: u32,
            restricted: *const c_void,
            result: *mut Handle,
        ) -> i32;
        fn DuplicateToken(token: Handle, level: u32, result: *mut Handle) -> i32;
        fn SetThreadToken(thread: *const Handle, token: Handle) -> i32;
        fn RevertToSelf() -> i32;
    }
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            assert_ne!(unsafe { RevertToSelf() }, 0);
        }
    }
    #[test]
    fn private_scope_files_inherit_user_only_access_not_the_token_default_dacl() {
        let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let directory = parent.join(format!("orchestrix-private-{}", nonce().unwrap()));
        private_directory(&directory).unwrap();
        let file = directory.join("receipt.json");
        std::fs::write(&file, b"private").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"private");
        let mut token = null_mut();
        assert_ne!(
            unsafe { OpenProcessToken(GetCurrentProcess(), 8 | 2 | 4, &mut token) },
            0
        );
        let token = Owned(token);
        let mut sid = null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSidToSidW(
                    wide(&crate::ipc::current_user_scope().unwrap()).as_ptr(),
                    &mut sid,
                )
            },
            0
        );
        let _sid = Memory(sid);
        let disabled = SidAttributes { sid, attributes: 0 };
        let mut restricted = null_mut();
        assert_ne!(
            unsafe {
                CreateRestrictedToken(
                    token.0,
                    0,
                    1,
                    &disabled,
                    0,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
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
            unsafe { SetThreadToken(std::ptr::null(), impersonation.0) },
            0
        );
        let revert = Revert;
        assert_eq!(std::fs::read(&file).unwrap_err().raw_os_error(), Some(5));
        drop(revert);
        assert_eq!(std::fs::read(&file).unwrap(), b"private");
        assert_eq!(directory.parent(), Some(parent.as_path()));
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    #[ignore = "explicitly launched own helper"]
    fn sealed_child_marker() {
        std::fs::write(
            std::env::var_os("ORCHESTRIX_SEALED_MARKER").unwrap(),
            b"unexpected execution",
        )
        .unwrap();
    }
    #[test]
    fn zero_active_limit_fences_creation_before_any_child_code_runs() {
        let scope = crate::named_job::JobScope::create(&nonce().unwrap()).unwrap();
        let marker = std::env::temp_dir().join(format!("orchestrix-seal-{}", nonce().unwrap()));
        let mut environment = std::env::vars_os()
            .filter(|(k, _)| {
                ["SystemRoot", "SystemDrive", "WINDIR", "TEMP", "TMP", "PATH"]
                    .iter()
                    .any(|s| k.to_string_lossy().eq_ignore_ascii_case(s))
            })
            .collect::<Vec<_>>();
        environment.push(("ORCHESTRIX_SEALED_MARKER".into(), marker.as_os_str().into()));
        let spec = crate::SpawnSpec {
            executable: std::env::current_exe().unwrap(),
            arguments: vec![
                "--ignored".into(),
                "--exact".into(),
                "named_job::native::tests::sealed_child_marker".into(),
            ],
            cwd: std::fs::canonicalize(std::env::temp_dir()).unwrap(),
            environment,
            limits: crate::ProcessLimits::default(),
        };
        for _ in 0..2 {
            let positive = crate::OwnedProcess::spawn_identified(spec.clone(), &scope)
                .expect("the same helper/spec must run in an open owned Job");
            let begin = std::time::Instant::now();
            loop {
                let state = positive.tree().unwrap();
                if state.root_exit_code == Some(0) && state.active_processes == 0 {
                    break;
                }
                assert!(begin.elapsed() < std::time::Duration::from_secs(3));
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert_eq!(std::fs::read(&marker).unwrap(), b"unexpected execution");
            drop(positive);
            std::fs::remove_file(&marker).unwrap();
        }
        scope
            .seal()
            .expect("Windows must accept and report an active-process limit of zero");
        let launched = crate::OwnedProcess::spawn_identified(spec, &scope);
        assert!(
            launched.is_err(),
            "JOB_LIST must refuse creation in the sealed Job"
        );
        assert!(!marker.exists());
        assert_eq!(scope.active().unwrap(), 0);
    }
}
