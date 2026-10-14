//! Minimal Windows ownership boundary. Every process starts suspended, is assigned to
//! an unnamed, non-breakaway Job, and is resumed only after membership is observed.
use crate::{ProcessError, Result, SpawnSpec, TreeState};
use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::ptr::{null, null_mut};

type Handle = *mut c_void;
type Bool = i32;
const CREATE_SUSPENDED: u32 = 4;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const KILL_ON_JOB_CLOSE: u32 = 0x2000;
#[repr(C)]
struct Security {
    length: u32,
    descriptor: *mut c_void,
    inherit: Bool,
}
#[repr(C)]
struct Startup {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count: u32,
    y_count: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    stdin: Handle,
    stdout: Handle,
    stderr: Handle,
}
#[repr(C)]
struct StartupEx {
    startup: Startup,
    attributes: *mut c_void,
}
#[repr(C)]
struct ProcessInfo {
    process: Handle,
    thread: Handle,
    pid: u32,
    tid: u32,
}
#[repr(C)]
struct BasicLimits {
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
struct IoCounters {
    read_ops: u64,
    write_ops: u64,
    other_ops: u64,
    read_bytes: u64,
    write_bytes: u64,
    other_bytes: u64,
}
#[repr(C)]
struct ExtendedLimits {
    basic: BasicLimits,
    io: IoCounters,
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
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

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const Security, name: *const u16) -> Handle;
    fn SetInformationJobObject(
        job: Handle,
        class: i32,
        information: *const c_void,
        length: u32,
    ) -> Bool;
    fn QueryInformationJobObject(
        job: Handle,
        class: i32,
        information: *mut c_void,
        length: u32,
        returned: *mut u32,
    ) -> Bool;
    fn IsProcessInJob(process: Handle, job: Handle, answer: *mut Bool) -> Bool;
    fn TerminateJobObject(job: Handle, code: u32) -> Bool;
    fn TerminateProcess(process: Handle, code: u32) -> Bool;
    fn CreatePipe(
        read: *mut Handle,
        write: *mut Handle,
        attributes: *const Security,
        size: u32,
    ) -> Bool;
    fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> Bool;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *mut c_void,
        size: usize,
        previous: *mut c_void,
        returned: *mut usize,
    ) -> Bool;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: *const Security,
        thread_attributes: *const Security,
        inherit: Bool,
        flags: u32,
        environment: *const c_void,
        cwd: *const u16,
        startup: *const StartupEx,
        info: *mut ProcessInfo,
    ) -> Bool;
    fn ResumeThread(thread: Handle) -> u32;
    fn WaitForSingleObject(handle: Handle, timeout: u32) -> u32;
    fn GetExitCodeProcess(handle: Handle, code: *mut u32) -> Bool;
}
struct OwnedHandle(Handle);
impl OwnedHandle {
    fn take(&mut self) -> Handle {
        std::mem::replace(&mut self.0, null_mut())
    }
}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
struct Attributes {
    storage: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

pub(crate) struct NativeProcess {
    job: OwnedHandle,
    root: OwnedHandle,
}
// These owned kernel handles remain open for the lifetime of this immutable object.
// Job query/termination and process wait/query APIs are thread-safe; no PID lookup occurs.
unsafe impl Send for NativeProcess {}
unsafe impl Sync for NativeProcess {}
pub(crate) struct Launched {
    pub process: NativeProcess,
    pub input: File,
    pub output: File,
    pub error: File,
}

impl NativeProcess {
    pub fn member_ids(&self) -> Result<Vec<u32>> {
        // This queries only our Job. It never enumerates or opens unrelated PIDs.
        let mut buffer = vec![0usize; (8 + 1024 * size_of::<usize>()).div_ceil(size_of::<usize>())];
        unsafe {
            require(
                QueryInformationJobObject(
                    self.job.0,
                    3,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * size_of::<usize>()) as u32,
                    null_mut(),
                ) != 0,
                ProcessError::Ownership,
            )?;
            let header = buffer.as_ptr().cast::<u32>();
            let assigned = *header;
            let count = *header.add(1);
            require(assigned == count && count <= 1024, ProcessError::Ownership)?;
            let ids = buffer.as_ptr().cast::<u8>().add(8).cast::<usize>();
            (0..count as usize)
                .map(|index| u32::try_from(*ids.add(index)).map_err(|_| ProcessError::Ownership))
                .collect()
        }
    }
    pub fn spawn(spec: &SpawnSpec) -> Result<Launched> {
        let executable = wide(spec.executable.as_os_str())?;
        let cwd = wide(spec.cwd.as_os_str())?;
        let mut command = quote(spec.executable.as_os_str())?;
        for argument in &spec.arguments {
            command.push(b' ' as u16);
            command.extend(quote(argument)?);
        }
        if command.len() >= 32_766 {
            return Err(ProcessError::Arguments);
        }
        command.push(0);
        let environment = environment(spec)?;
        unsafe {
            let job = OwnedHandle(CreateJobObjectW(null(), null()));
            require(!job.0.is_null(), ProcessError::Ownership)?;
            let mut limits: ExtendedLimits = zeroed();
            limits.basic.flags = KILL_ON_JOB_CLOSE;
            require(
                SetInformationJobObject(
                    job.0,
                    9,
                    (&limits as *const ExtendedLimits).cast(),
                    size_of::<ExtendedLimits>() as u32,
                ) != 0,
                ProcessError::Ownership,
            )?;
            let (input_read, mut input_write) = pipe()?;
            let (mut output_read, output_write) = pipe()?;
            let (mut error_read, error_write) = pipe()?;
            for parent in [input_write.0, output_read.0, error_read.0] {
                require(
                    SetHandleInformation(parent, 1, 0) != 0,
                    ProcessError::Ownership,
                )?;
            }
            let mut size = 0;
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut size);
            require(size > 0 && size <= 65_536, ProcessError::Ownership)?;
            let mut attributes = Attributes {
                storage: vec![0; size.div_ceil(size_of::<usize>())],
                initialized: false,
            };
            require(
                InitializeProcThreadAttributeList(attributes.pointer(), 2, 0, &mut size) != 0,
                ProcessError::Ownership,
            )?;
            attributes.initialized = true;
            let mut handles = [input_read.0, output_write.0, error_write.0];
            require(
                UpdateProcThreadAttribute(
                    attributes.pointer(),
                    0,
                    0x0002_0002,
                    handles.as_mut_ptr().cast(),
                    size_of_val(&handles),
                    null_mut(),
                    null_mut(),
                ) != 0,
                ProcessError::Ownership,
            )?;
            let mut job_list = [job.0];
            require(
                UpdateProcThreadAttribute(
                    attributes.pointer(),
                    0,
                    0x0002_000d,
                    job_list.as_mut_ptr().cast(),
                    size_of_val(&job_list),
                    null_mut(),
                    null_mut(),
                ) != 0,
                ProcessError::Ownership,
            )?;
            let mut startup: StartupEx = zeroed();
            startup.startup.cb = size_of::<StartupEx>() as u32;
            startup.startup.flags = 0x100;
            startup.startup.stdin = input_read.0;
            startup.startup.stdout = output_write.0;
            startup.startup.stderr = error_write.0;
            startup.attributes = attributes.pointer();
            let mut info: ProcessInfo = zeroed();
            require(
                CreateProcessW(
                    executable.as_ptr(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    CREATE_SUSPENDED
                        | CREATE_NO_WINDOW
                        | CREATE_UNICODE_ENVIRONMENT
                        | EXTENDED_STARTUPINFO_PRESENT,
                    environment.as_ptr().cast(),
                    cwd.as_ptr(),
                    &startup,
                    &mut info,
                ) != 0,
                ProcessError::Launch,
            )?;
            let root = OwnedHandle(info.process);
            let thread = OwnedHandle(info.thread);
            let owned = NativeProcess { job, root };
            let mut member = 0;
            if IsProcessInJob(owned.root.0, owned.job.0, &mut member) == 0 || member == 0 {
                TerminateProcess(owned.root.0, 137);
                WaitForSingleObject(owned.root.0, 3000);
                return Err(ProcessError::Ownership);
            }
            require(ResumeThread(thread.0) != u32::MAX, ProcessError::Launch)?;
            drop(thread);
            drop(input_read);
            drop(output_write);
            drop(error_write);
            Ok(Launched {
                process: owned,
                input: File::from_raw_handle(input_write.take()),
                output: File::from_raw_handle(output_read.take()),
                error: File::from_raw_handle(error_read.take()),
            })
        }
    }
    pub fn observe(&self) -> Result<TreeState> {
        unsafe {
            let mut member = 0;
            require(
                IsProcessInJob(self.root.0, self.job.0, &mut member) != 0 && member != 0,
                ProcessError::Ownership,
            )?;
            let mut accounting: Accounting = zeroed();
            require(
                QueryInformationJobObject(
                    self.job.0,
                    1,
                    (&mut accounting as *mut Accounting).cast(),
                    size_of::<Accounting>() as u32,
                    null_mut(),
                ) != 0,
                ProcessError::Ownership,
            )?;
            let root_exit_code = match WaitForSingleObject(self.root.0, 0) {
                0 => {
                    let mut code = 0;
                    require(
                        GetExitCodeProcess(self.root.0, &mut code) != 0,
                        ProcessError::Io,
                    )?;
                    Some(code)
                }
                258 => None,
                _ => return Err(ProcessError::Io),
            };
            Ok(TreeState {
                owned: true,
                root_exit_code,
                active_processes: accounting.active,
            })
        }
    }
    pub fn terminate(&self) -> Result<()> {
        unsafe {
            require(
                TerminateJobObject(self.job.0, 137) != 0,
                ProcessError::Ownership,
            )
        }
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}
fn require(value: bool, error: ProcessError) -> Result<()> {
    if value { Ok(()) } else { Err(error) }
}
fn wide(value: &OsStr) -> Result<Vec<u16>> {
    let mut result: Vec<_> = value.encode_wide().collect();
    if result.contains(&0) || result.len() > 32_766 {
        return Err(ProcessError::Arguments);
    }
    result.push(0);
    Ok(result)
}
fn quote(value: &OsStr) -> Result<Vec<u16>> {
    let raw = wide(value)?;
    let mut result = vec![34];
    let mut slashes = 0;
    for ch in &raw[..raw.len() - 1] {
        if *ch == 92 {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            92,
            if *ch == 34 { slashes * 2 + 1 } else { slashes },
        ));
        result.push(*ch);
        slashes = 0;
    }
    result.extend(std::iter::repeat_n(92, slashes * 2));
    result.push(34);
    Ok(result)
}
fn environment(spec: &SpawnSpec) -> Result<Vec<u16>> {
    let mut variables = std::collections::BTreeMap::new();
    for (key, value) in &spec.environment {
        let key_string = key.to_str().ok_or(ProcessError::Arguments)?;
        if key_string.is_empty() || key_string.contains(['=', '\0']) {
            return Err(ProcessError::Arguments);
        }
        if variables
            .insert(key_string.to_ascii_uppercase(), (key, value))
            .is_some()
        {
            return Err(ProcessError::Arguments);
        }
    }
    let mut output = Vec::new();
    for (_, (key, value)) in variables {
        let mut key = wide(key)?;
        key.pop();
        output.extend(key);
        output.push(61);
        output.extend(wide(value)?);
    }
    output.push(0);
    if output.len() == 1 {
        output.push(0);
    }
    if output.len() > 65_536 {
        return Err(ProcessError::Arguments);
    }
    Ok(output)
}
unsafe fn pipe() -> Result<(OwnedHandle, OwnedHandle)> {
    let security = Security {
        length: size_of::<Security>() as u32,
        descriptor: null_mut(),
        inherit: 1,
    };
    let mut read = null_mut();
    let mut write = null_mut();
    unsafe {
        require(
            CreatePipe(&mut read, &mut write, &security, 0) != 0,
            ProcessError::Launch,
        )?;
    }
    Ok((OwnedHandle(read), OwnedHandle(write)))
}
