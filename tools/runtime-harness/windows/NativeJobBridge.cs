// Experimental raw stdio bridge. Control JSONL never carries runtime bytes.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.Win32.SafeHandles;

namespace Orchestrix.WindowsSpike
{
    public sealed class NativeJobBridge : IDisposable
    {
        sealed class Pump
        {
            public Thread Thread;
            public IntPtr NativeThread;
            public int CloserStarted;
            public readonly object Sync = new object();
            public volatile bool Closed, CloseRequested;
        }

        readonly Pump input = new Pump(), output = new Pump(), error = new Pump();
        NamedPipeClientStream inputPipe, outputPipe, errorPipe;
        volatile bool disposing, stopping, faulted;
        public readonly NativeJobProbe Job = new NativeJobProbe();
        public bool InputClosed { get { return input.Closed; } }
        public bool OutputClosed { get { return output.Closed; } }
        public bool ErrorClosed { get { return error.Closed; } }
        public bool Faulted { get { return faulted; } }

        [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenThread(uint access, bool inherit, uint id);
        [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CancelSynchronousIo(IntPtr thread);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CancelIoEx(IntPtr handle, IntPtr overlapped);
        [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr handle);

        static string PipeName(string value, string role, out string nonce)
        {
            const string prefix = @"\\.\pipe\";
            if (value != null && value.StartsWith(prefix, StringComparison.Ordinal)) value = value.Substring(prefix.Length);
            Match match = Regex.Match(value ?? "", "^orchestrix-([a-f0-9]{48})-" + role + "$");
            if (!match.Success) throw new ProbeException("pipe-name", 0);
            nonce = match.Groups[1].Value; return value;
        }

        static NamedPipeClientStream Connect(string name, PipeDirection direction)
        {
            NamedPipeClientStream pipe = new NamedPipeClientStream(".", name, direction, PipeOptions.Asynchronous, TokenImpersonationLevel.Anonymous);
            try { pipe.Connect(5000); return pipe; }
            catch { pipe.Dispose(); throw new ProbeException("pipe-connect", 0); }
        }

        public void Start(string executable, string[] arguments, string cwd, IDictionary<string, string> environment,
            string stdinPipe, string stdoutPipe, string stderrPipe, int lifetimeMs)
        {
            if (inputPipe != null || outputPipe != null || errorPipe != null || disposing || lifetimeMs < 1000 || lifetimeMs > 300000) throw new ProbeException("arguments", 0);
            string inputNonce, outputNonce, errorNonce;
            string inputName = PipeName(stdinPipe, "stdin", out inputNonce);
            string outputName = PipeName(stdoutPipe, "stdout", out outputNonce);
            string errorName = PipeName(stderrPipe, "stderr", out errorNonce);
            if (inputNonce != outputNonce || inputNonce != errorNonce) throw new ProbeException("pipe-name", 0);
            // Connect every raw channel before any owned child can run. The
            // server cannot impersonate this helper, and channels are local.
            inputPipe = Connect(inputName, PipeDirection.In);
            outputPipe = Connect(outputName, PipeDirection.Out);
            errorPipe = Connect(errorName, PipeDirection.Out);
            Job.LaunchRaw(executable, arguments, cwd, environment);
            Job.ArmBridgeWatchdog(lifetimeMs, 1000);
            StartPump(input, inputPipe, Job.RawInput, true);
            StartPump(output, Job.RawOutput, outputPipe, false);
            StartPump(error, Job.RawError, errorPipe, false);
        }

        void StartPump(Pump pump, Stream source, Stream destination, bool stdin)
        {
            pump.Thread = new Thread(delegate()
            {
                try
                {
                    // A retained thread handle lets close-stdin/cleanup cancel
                    // an owned synchronous anonymous-pipe write without using
                    // a PID search or terminating an unrelated thread.
                    lock (pump.Sync)
                    {
                        pump.NativeThread = OpenThread(1, false, GetCurrentThreadId());
                        if (pump.NativeThread == IntPtr.Zero) throw new ProbeException("pump-thread", Marshal.GetLastWin32Error());
                    }
                    byte[] bytes = new byte[8192];
                    while (!disposing && !pump.CloseRequested)
                    {
                        int count = source.Read(bytes, 0, bytes.Length);
                        if (count == 0 || disposing || pump.CloseRequested) break;
                        destination.Write(bytes, 0, count);
                        destination.Flush();
                    }
                }
                catch
                {
                    if (!disposing && !stopping && !pump.CloseRequested && !(stdin && Job.RootExited)) faulted = true;
                }
                finally
                {
                    try { destination.Dispose(); } catch { if (!disposing && !stopping) faulted = true; }
                    try { source.Dispose(); } catch { if (!disposing && !stopping) faulted = true; }
                    lock (pump.Sync)
                    {
                        if (pump.NativeThread != IntPtr.Zero) { CloseHandle(pump.NativeThread); pump.NativeThread = IntPtr.Zero; }
                    }
                    pump.Closed = true;
                }
            });
            pump.Thread.IsBackground = true;
            pump.Thread.Name = stdin ? "orchestrix-raw-stdin" : "orchestrix-raw-output";
            pump.Thread.Start();
        }

        static void Cancel(Pump pump)
        {
            lock (pump.Sync) { if (pump.NativeThread != IntPtr.Zero) CancelSynchronousIo(pump.NativeThread); }
        }

        void ClosePipeAndCancel(Pump pump, NamedPipeClientStream pipe)
        {
            pump.CloseRequested = true;
            if (Interlocked.CompareExchange(ref pump.CloserStarted, 1, 0) != 0) { Cancel(pump); return; }
            SafePipeHandle lease = null;
            bool retained = false;
            IntPtr native = IntPtr.Zero;
            try
            {
                if (pipe != null)
                {
                    lease = pipe.SafePipeHandle;
                    lease.DangerousAddRef(ref retained);
                    native = lease.DangerousGetHandle();
                }
            }
            catch (ObjectDisposedException) { /* Already closed by its pump. */ }
            catch { faulted = true; }
            if (pipe != null) { try { pipe.Dispose(); } catch { faulted = true; } }
            Cancel(pump);
            if (native != IntPtr.Zero) CancelIoEx(native, IntPtr.Zero);
            if (pump.Thread == null || pump.Closed)
            {
                if (retained) lease.DangerousRelease();
                return;
            }
            // Both overlapped named-pipe I/O and synchronous anonymous-pipe I/O
            // can race between a flag check and submission. Retry cancellation
            // until this pump acknowledges closure. Retain the exact SafeHandle
            // throughout so the native handle cannot close or be reused.
            Thread closer = new Thread(delegate()
            {
                try
                {
                    while (!pump.Closed)
                    {
                        Cancel(pump);
                        if (native != IntPtr.Zero) CancelIoEx(native, IntPtr.Zero);
                        Thread.Sleep(10);
                    }
                }
                finally { if (retained) lease.DangerousRelease(); }
            });
            closer.IsBackground = true; closer.Name = "orchestrix-close-raw-pipe";
            try { closer.Start(); }
            catch { if (retained) lease.DangerousRelease(); faulted = true; }
        }

        public void CloseInput() { ClosePipeAndCancel(input, inputPipe); }

        public void BeginStop() { stopping = true; }

        public bool WaitForStreams(int timeoutMs)
        {
            if (timeoutMs < 1 || timeoutMs > 3000) throw new ProbeException("arguments", 0);
            CloseInput();
            Stopwatch deadline = Stopwatch.StartNew();
            while (!InputClosed || !OutputClosed || !ErrorClosed)
            {
                if (deadline.ElapsedMilliseconds >= timeoutMs) return false;
                Thread.Sleep(10);
            }
            return true;
        }

        public void Dispose()
        {
            if (disposing) return;
            disposing = true; stopping = true;
            CloseInput();
            ClosePipeAndCancel(output, outputPipe);
            ClosePipeAndCancel(error, errorPipe);
            // Stop/confirmation is performed by the controller before Dispose.
            // Closing the sole Job handle remains the final containment fallback.
            Job.Dispose();
            Stopwatch deadline = Stopwatch.StartNew();
            foreach (Pump pump in new Pump[] { input, output, error })
            {
                Thread thread = pump.Thread;
                int remaining = 1000 - (int)deadline.ElapsedMilliseconds;
                if (thread != null && thread != Thread.CurrentThread && remaining > 0) thread.Join(remaining);
            }
        }
    }
}
