// Experimental synthetic-only Windows supervisor. No provider, authentication or shell launch.
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

namespace Orchestrix.WindowsSpike
{
    public sealed class ProbeException : Exception
    {
        public readonly string Category;
        public readonly int NativeError;
        public ProbeException(string category, int error) : base(category) { Category = category; NativeError = error; }
    }

    public sealed class BoundedLines
    {
        readonly ConcurrentQueue<string> lines = new ConcurrentQueue<string>();
        readonly TextReader reader;
        public volatile bool Ended;
        public volatile bool Faulted;
        public BoundedLines(TextReader input)
        {
            reader = input;
            Thread thread = new Thread(Pump); thread.IsBackground = true; thread.Start();
        }
        void Pump()
        {
            try
            {
                StringBuilder line = new StringBuilder();
                int ch;
                while ((ch = reader.Read()) != -1)
                {
                    if (ch == '\n')
                    {
                        if (lines.Count >= 128) { Faulted = true; break; }
                        lines.Enqueue(line.ToString().TrimEnd('\r')); line.Clear();
                    }
                    else
                    {
                        line.Append((char)ch);
                        if (line.Length > 8192) { Faulted = true; break; }
                    }
                }
                if (line.Length != 0) Faulted = true;
            }
            catch { Faulted = true; }
            finally { Ended = true; }
        }
        public string Take() { string line; return lines.TryDequeue(out line) ? line : null; }
        public bool Empty { get { return lines.IsEmpty; } }
    }

    public sealed class StopResult
    {
        public int ActiveCount = -1;
        public bool RootExited;
    }

    public sealed class NativeJobProbe : IDisposable
    {
        const uint KILL_ON_JOB_CLOSE = 0x2000, CREATE_SUSPENDED = 0x4, CREATE_NO_WINDOW = 0x08000000;
        const uint EXTENDED_STARTUPINFO_PRESENT = 0x00080000, CREATE_UNICODE_ENVIRONMENT = 0x400;
        const uint STARTF_USESTDHANDLES = 0x100, HANDLE_FLAG_INHERIT = 1, QUERY_LIMITED = 0x1000;
        const uint WAIT_OBJECT_0 = 0, WAIT_TIMEOUT = 258;
        const int ERROR_MORE_DATA = 234;
        static readonly IntPtr HANDLE_LIST = new IntPtr(0x20002);
        static readonly IntPtr JOB_LIST = new IntPtr(0x2000D);
        IntPtr job, process, thread;
        readonly object handleSync = new object();
        readonly ManualResetEvent watchdogCancel = new ManualResetEvent(false);
        Thread watchdog;
        volatile bool disposed, watchdogTriggered, controlWriteInProgress;
        StopResult confirmedStop;
        StreamWriter childInput;
        StreamReader childOutput;
        StreamReader childError;
        public BoundedLines Output;
        public BoundedLines Control;
        public uint RootPid;
        public bool JobConfigured;
        public bool RootInJob;
        public bool Resumed;
        public bool StderrFaulted;
        long stderrBytes;
        public long StderrBytes { get { return Interlocked.Read(ref stderrBytes); } }
        public bool WatchdogTriggered { get { return watchdogTriggered; } }
        public bool WatchdogArmed { get { return watchdog != null; } }

        public void ArmWatchdog(int lifetimeMs, int outputGraceMs)
        {
            if (lifetimeMs < 1000 || lifetimeMs > 30000 || outputGraceMs < 1 || outputGraceMs > 3000) throw new ProbeException("arguments", 0);
            lock (handleSync)
            {
                if (disposed || !Resumed || watchdog != null) throw new ProbeException("watchdog-state", 0);
                Stopwatch elapsed = Stopwatch.StartNew();
                watchdog = new Thread(delegate()
                {
                    while (elapsed.ElapsedMilliseconds < lifetimeMs)
                    {
                        if (watchdogCancel.WaitOne(10)) return;
                    }
                    watchdogTriggered = true;
                    bool confirmed = false;
                    try
                    {
                        StopResult stopped = Stop(3000);
                        confirmed = stopped.ActiveCount == 0 && (RootPid == 0 || stopped.RootExited);
                    }
                    catch { /* No fabricated confirmation or blocking diagnostic output. */ }
                    // Give a responsive controller time to emit stopped and Dispose.
                    // A blocked Console writer must not keep this helper alive forever.
                    if (watchdogCancel.WaitOne(outputGraceMs)) return;
                    // Managed Environment.Exit can wait on host shutdown/Console
                    // locks. This pseudo handle can only refer to this helper.
                    TerminateProcess(GetCurrentProcess(), (uint)(confirmed && controlWriteInProgress ? 3 : 4));
                });
                watchdog.IsBackground = true; watchdog.Name = "orchestrix-job-watchdog"; watchdog.Start();
            }
        }

        public void WriteControlLine(string line)
        {
            if (line == null || line.Length > 8192 || line.IndexOf('\n') >= 0 || line.IndexOf('\r') >= 0) throw new ProbeException("control-output-limit", 0);
            controlWriteInProgress = true;
            try { Console.Out.WriteLine(line); Console.Out.Flush(); }
            finally { controlWriteInProgress = false; }
        }

        [StructLayout(LayoutKind.Sequential)] struct SECURITY_ATTRIBUTES { public int Length; public IntPtr Descriptor; [MarshalAs(UnmanagedType.Bool)] public bool Inherit; }
        [StructLayout(LayoutKind.Sequential)] struct STARTUPINFO
        {
            public int cb; public IntPtr reserved, desktop, title;
            public uint x, y, xSize, ySize, xCount, yCount, fill, flags;
            public ushort showWindow, reservedSize; public IntPtr reservedBytes, stdin, stdout, stderr;
        }
        [StructLayout(LayoutKind.Sequential)] struct STARTUPINFOEX { public STARTUPINFO Startup; public IntPtr Attributes; }
        [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION { public IntPtr Process, Thread; public uint Pid, Tid; }
        [StructLayout(LayoutKind.Sequential)] struct BASIC_LIMITS
        {
            public long ProcessTime, JobTime; public uint Flags;
            public UIntPtr MinWorkingSet, MaxWorkingSet; public uint ActiveLimit; public UIntPtr Affinity;
            public uint Priority, Scheduling;
        }
        [StructLayout(LayoutKind.Sequential)] struct IO_COUNTERS { public ulong ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes; }
        [StructLayout(LayoutKind.Sequential)] struct EXTENDED_LIMITS
        {
            public BASIC_LIMITS Basic; public IO_COUNTERS Io;
            public UIntPtr ProcessMemory, JobMemory, PeakProcessMemory, PeakJobMemory;
        }
        [StructLayout(LayoutKind.Sequential)] struct ACCOUNTING
        {
            public long UserTime, KernelTime, PeriodUserTime, PeriodKernelTime;
            public uint PageFaults, TotalProcesses, ActiveProcesses, TerminatedProcesses;
        }
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr CreateJobObjectW(IntPtr attributes, string name);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetInformationJobObject(IntPtr job, int type, ref EXTENDED_LIMITS information, uint length);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool QueryInformationJobObject(IntPtr job, int type, ref ACCOUNTING information, uint length, IntPtr returned);
        [DllImport("kernel32.dll", SetLastError = true, EntryPoint = "QueryInformationJobObject")] static extern bool QueryJobIds(IntPtr job, int type, IntPtr information, uint length, IntPtr returned);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool IsProcessInJob(IntPtr process, IntPtr job, [MarshalAs(UnmanagedType.Bool)] out bool answer);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool TerminateJobObject(IntPtr job, uint exitCode);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr handle);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CreatePipe(out IntPtr read, out IntPtr write, ref SECURITY_ATTRIBUTES attributes, uint size);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetHandleInformation(IntPtr handle, uint mask, uint flags);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetHandleInformation(IntPtr handle, out uint flags);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool InitializeProcThreadAttributeList(IntPtr list, int count, uint flags, ref IntPtr size);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool UpdateProcThreadAttribute(IntPtr list, uint flags, IntPtr attribute, IntPtr value, IntPtr size, IntPtr previous, IntPtr returned);
        [DllImport("kernel32.dll")] static extern void DeleteProcThreadAttributeList(IntPtr list);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool CreateProcessW(string application, StringBuilder commandLine, IntPtr processAttributes, IntPtr threadAttributes, bool inheritHandles, uint flags, IntPtr environment, string cwd, ref STARTUPINFOEX startup, out PROCESS_INFORMATION info);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint ResumeThread(IntPtr thread);
        [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool GetExitCodeProcess(IntPtr process, out uint code);
        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool TerminateProcess(IntPtr process, uint exitCode);

        static void Require(bool ok, string category) { if (!ok) throw new ProbeException(category, Marshal.GetLastWin32Error()); }
        static void Close(ref IntPtr handle) { if (handle != IntPtr.Zero) { CloseHandle(handle); handle = IntPtr.Zero; } }

        // Windows CRT quoting; no cmd.exe, interpolation or string-built shell execution.
        public static string QuoteArgument(string value)
        {
            if (value == null || value.IndexOf('\0') >= 0) throw new ProbeException("arguments", 0);
            StringBuilder result = new StringBuilder("\"");
            int slashes = 0;
            foreach (char ch in value)
            {
                if (ch == '\\') { slashes++; continue; }
                if (ch == '"') { result.Append('\\', slashes * 2 + 1); result.Append('"'); }
                else { result.Append('\\', slashes); result.Append(ch); }
                slashes = 0;
            }
            result.Append('\\', slashes * 2); result.Append('"'); return result.ToString();
        }

        static IntPtr SafeEnvironment()
        {
            // Synthetic Node fixture does not require user credentials, NODE_OPTIONS or package hooks.
            string[] allow = { "SystemRoot", "WINDIR", "SystemDrive", "TEMP", "TMP", "COMSPEC", "PATH", "PATHEXT", "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE" };
            SortedDictionary<string, string> variables = new SortedDictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string name in allow) { string value = Environment.GetEnvironmentVariable(name); if (value != null) variables[name] = value; }
            StringBuilder block = new StringBuilder();
            foreach (KeyValuePair<string, string> entry in variables) block.Append(entry.Key).Append('=').Append(entry.Value).Append('\0');
            block.Append('\0');
            return Marshal.StringToHGlobalUni(block.ToString());
        }

        public void Launch(string executable, string[] arguments, string cwd, bool failBeforeResume)
        {
            lock (handleSync)
            {
            if (disposed || job != IntPtr.Zero) throw new ProbeException("launch-state", 0);
            IntPtr inputRead = IntPtr.Zero, inputWrite = IntPtr.Zero, outputRead = IntPtr.Zero, outputWrite = IntPtr.Zero;
            IntPtr errorRead = IntPtr.Zero, errorWrite = IntPtr.Zero, attributes = IntPtr.Zero, handles = IntPtr.Zero, jobs = IntPtr.Zero, environment = IntPtr.Zero;
            bool attributesInitialized = false;
            try
            {
                job = CreateJobObjectW(IntPtr.Zero, null); Require(job != IntPtr.Zero, "job-create");
                uint jobHandleFlags;
                Require(GetHandleInformation(job, out jobHandleFlags), "job-handle-query");
                if ((jobHandleFlags & HANDLE_FLAG_INHERIT) != 0) throw new ProbeException("job-handle-inherited", 0);
                EXTENDED_LIMITS limits = new EXTENDED_LIMITS(); limits.Basic.Flags = KILL_ON_JOB_CLOSE;
                Require(SetInformationJobObject(job, 9, ref limits, (uint)Marshal.SizeOf(typeof(EXTENDED_LIMITS))), "job-configure");
                JobConfigured = true;
                SECURITY_ATTRIBUTES security = new SECURITY_ATTRIBUTES(); security.Length = Marshal.SizeOf(typeof(SECURITY_ATTRIBUTES)); security.Inherit = true;
                Require(CreatePipe(out inputRead, out inputWrite, ref security, 0), "pipe-create");
                Require(CreatePipe(out outputRead, out outputWrite, ref security, 0), "pipe-create");
                Require(CreatePipe(out errorRead, out errorWrite, ref security, 0), "pipe-create");
                Require(SetHandleInformation(inputWrite, HANDLE_FLAG_INHERIT, 0), "pipe-configure");
                Require(SetHandleInformation(outputRead, HANDLE_FLAG_INHERIT, 0), "pipe-configure");
                Require(SetHandleInformation(errorRead, HANDLE_FLAG_INHERIT, 0), "pipe-configure");

                IntPtr size = IntPtr.Zero;
                bool sized = InitializeProcThreadAttributeList(IntPtr.Zero, 2, 0, ref size);
                int sizeError = Marshal.GetLastWin32Error();
                if (sized || sizeError != 122 || size == IntPtr.Zero) throw new ProbeException("attributes-size", sizeError);
                attributes = Marshal.AllocHGlobal(size);
                Require(InitializeProcThreadAttributeList(attributes, 2, 0, ref size), "attributes-initialize");
                attributesInitialized = true;
                handles = Marshal.AllocHGlobal(IntPtr.Size * 3);
                Marshal.WriteIntPtr(handles, 0, inputRead); Marshal.WriteIntPtr(handles, IntPtr.Size, outputWrite); Marshal.WriteIntPtr(handles, IntPtr.Size * 2, errorWrite);
                Require(UpdateProcThreadAttribute(attributes, 0, HANDLE_LIST, handles, new IntPtr(IntPtr.Size * 3), IntPtr.Zero, IntPtr.Zero), "handle-list");
                jobs = Marshal.AllocHGlobal(IntPtr.Size); Marshal.WriteIntPtr(jobs, job);
                Require(UpdateProcThreadAttribute(attributes, 0, JOB_LIST, jobs, new IntPtr(IntPtr.Size), IntPtr.Zero, IntPtr.Zero), "job-list");
                STARTUPINFOEX startup = new STARTUPINFOEX(); startup.Startup.cb = Marshal.SizeOf(typeof(STARTUPINFOEX)); startup.Attributes = attributes;
                startup.Startup.flags = STARTF_USESTDHANDLES; startup.Startup.stdin = inputRead; startup.Startup.stdout = outputWrite; startup.Startup.stderr = errorWrite;
                StringBuilder command = new StringBuilder(QuoteArgument(executable));
                foreach (string argument in arguments) command.Append(' ').Append(QuoteArgument(argument));
                if (command.Length >= 32767) throw new ProbeException("arguments", 0);
                environment = SafeEnvironment();
                PROCESS_INFORMATION info;
                Require(CreateProcessW(executable, command, IntPtr.Zero, IntPtr.Zero, true,
                    CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
                    environment, cwd, ref startup, out info), "launch");
                process = info.Process; thread = info.Thread; RootPid = info.Pid;
                bool member;
                Require(IsProcessInJob(process, job, out member), "membership");
                Require(member, "membership"); RootInJob = true;
                if (failBeforeResume) throw new ProbeException("injected-before-resume", 0);
                uint previousSuspendCount = ResumeThread(thread);
                Require(previousSuspendCount != UInt32.MaxValue, "resume");
                if (previousSuspendCount != 1) throw new ProbeException("resume-state", 0);
                Resumed = true;
                Close(ref thread);
                Close(ref inputRead); Close(ref outputWrite); Close(ref errorWrite);
                childInput = new StreamWriter(new FileStream(new SafeFileHandle(inputWrite, true), FileAccess.Write), new UTF8Encoding(false)); inputWrite = IntPtr.Zero; childInput.AutoFlush = true;
                childOutput = new StreamReader(new FileStream(new SafeFileHandle(outputRead, true), FileAccess.Read), new UTF8Encoding(false, true)); outputRead = IntPtr.Zero;
                childError = new StreamReader(new FileStream(new SafeFileHandle(errorRead, true), FileAccess.Read), new UTF8Encoding(false, true)); errorRead = IntPtr.Zero;
                Output = new BoundedLines(childOutput);
                Control = new BoundedLines(new StreamReader(Console.OpenStandardInput(), new UTF8Encoding(false, true)));
                Thread drain = new Thread(DrainError); drain.IsBackground = true; drain.Start();
            }
            finally
            {
                if (attributesInitialized) DeleteProcThreadAttributeList(attributes);
                if (attributes != IntPtr.Zero) Marshal.FreeHGlobal(attributes);
                if (handles != IntPtr.Zero) Marshal.FreeHGlobal(handles);
                if (jobs != IntPtr.Zero) Marshal.FreeHGlobal(jobs);
                if (environment != IntPtr.Zero) Marshal.FreeHGlobal(environment);
                Close(ref inputRead); Close(ref inputWrite); Close(ref outputRead); Close(ref outputWrite); Close(ref errorRead); Close(ref errorWrite);
            }
            }
        }

        void DrainError()
        {
            try { char[] buffer = new char[1024]; int count; while ((count = childError.Read(buffer, 0, buffer.Length)) > 0) { Interlocked.Add(ref stderrBytes, count); if (StderrBytes > 65536) { StderrFaulted = true; break; } } }
            catch { StderrFaulted = true; }
        }

        bool RootExitedUnlocked { get { return process != IntPtr.Zero && WaitForSingleObject(process, 0) == WAIT_OBJECT_0; } }
        public bool RootExited { get { lock (handleSync) { return RootExitedUnlocked; } } }
        public uint RootExitCode { get { lock (handleSync) { uint code = 0; Require(process != IntPtr.Zero && GetExitCodeProcess(process, out code), "exit-query"); return code; } } }
        public int ActiveCount()
        {
            lock (handleSync) { return ActiveCountUnlocked(); }
        }
        int ActiveCountUnlocked()
        {
            if (job == IntPtr.Zero) return -1;
            ACCOUNTING accounting = new ACCOUNTING();
            Require(QueryInformationJobObject(job, 1, ref accounting, (uint)Marshal.SizeOf(typeof(ACCOUNTING)), IntPtr.Zero), "job-query");
            return (int)accounting.ActiveProcesses;
        }
        HashSet<uint> OwnedIds()
        {
            int capacity = 16;
            while (capacity <= 1024)
            {
                IntPtr buffer = Marshal.AllocHGlobal(8 + capacity * IntPtr.Size);
                try
                {
                    if (QueryJobIds(job, 3, buffer, (uint)(8 + capacity * IntPtr.Size), IntPtr.Zero))
                    {
                        int assigned = Marshal.ReadInt32(buffer, 0), count = Marshal.ReadInt32(buffer, 4);
                        if (assigned < 0 || count < 0 || count > capacity || count > assigned) throw new ProbeException("job-query", 0);
                        if (assigned != count) { capacity = Math.Max(capacity * 2, assigned); continue; }
                        HashSet<uint> result = new HashSet<uint>();
                        for (int i = 0; i < count; i++) result.Add((uint)Marshal.ReadIntPtr(buffer, 8 + i * IntPtr.Size).ToInt64());
                        return result;
                    }
                    if (Marshal.GetLastWin32Error() != ERROR_MORE_DATA) throw new ProbeException("job-query", Marshal.GetLastWin32Error());
                }
                finally { Marshal.FreeHGlobal(buffer); }
                capacity *= 2;
            }
            throw new ProbeException("job-query-limit", 0);
        }
        public bool InOwnedJob(uint pid)
        {
            lock (handleSync)
            {
            if (job == IntPtr.Zero) return false;
            // Query our Job list first. Never open or terminate an arbitrary outsider PID.
            if (!OwnedIds().Contains(pid)) return false;
            IntPtr handle = OpenProcess(QUERY_LIMITED, false, pid);
            if (handle == IntPtr.Zero) return false;
            try { bool member; Require(IsProcessInJob(handle, job, out member), "membership"); return member; }
            finally { CloseHandle(handle); }
            }
        }
        public void ExitRoot()
        {
            StreamWriter input;
            lock (handleSync)
            {
                if (disposed || childInput == null || RootExitedUnlocked) throw new ProbeException("root-unavailable", 0);
                input = childInput;
            }
            // Pipe I/O is deliberately outside the handle lock.
            input.WriteLine("{\"type\":\"exit-root\"}");
        }
        public StopResult Stop(int timeoutMs)
        {
            if (timeoutMs < 1 || timeoutMs > 10000) throw new ProbeException("arguments", 0);
            lock (handleSync)
            {
            if (confirmedStop != null) return new StopResult { ActiveCount = confirmedStop.ActiveCount, RootExited = confirmedStop.RootExited };
            StopResult result = new StopResult();
            if (job == IntPtr.Zero) { result.RootExited = RootExitedUnlocked; return result; }
            Require(TerminateJobObject(job, 137), "job-terminate");
            Stopwatch deadline = Stopwatch.StartNew();
            do
            {
                result.ActiveCount = ActiveCountUnlocked(); result.RootExited = RootExitedUnlocked;
                if (result.ActiveCount == 0 && (process == IntPtr.Zero || result.RootExited))
                {
                    confirmedStop = new StopResult { ActiveCount = result.ActiveCount, RootExited = result.RootExited };
                    return result;
                }
                Thread.Sleep(10);
            } while (deadline.ElapsedMilliseconds < timeoutMs);
            return result;
            }
        }
        public void Dispose()
        {
            Thread owner;
            lock (handleSync)
            {
            if (disposed) return;
            watchdogCancel.Set();
            disposed = true; owner = watchdog;
            // Closing this sole, non-inherited Job handle also enforces KILL_ON_JOB_CLOSE.
            Close(ref job); Close(ref thread); Close(ref process);
            }
            // Never wait for a watchdog or flush/dispose streams under handleSync.
            if (owner != null && owner != Thread.CurrentThread && !owner.Join(3500)) throw new ProbeException("watchdog-join", 0);
            if (childInput != null) childInput.Dispose();
            if (childOutput != null) childOutput.Dispose();
            if (childError != null) childError.Dispose();
            watchdogCancel.Dispose();
        }
    }
}
