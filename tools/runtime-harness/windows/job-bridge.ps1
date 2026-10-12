# Experimental supervised raw transport; no runtime bytes enter control JSONL.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false, $true)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$script:sequence = 0
$script:ownerId = [Guid]::NewGuid().ToString('N')
$bridge = $null
$reason = 'failure'
$exitCode = 1
function Emit([string]$type, [hashtable]$fields) {
    $record = [ordered]@{ schemaVersion = 1; sequence = ++$script:sequence; ownerId = $script:ownerId; type = $type }
    foreach ($key in $fields.Keys) { $record[$key] = $fields[$key] }
    $line = $record | ConvertTo-Json -Depth 4 -Compress
    if ($null -ne $bridge) { $bridge.Job.WriteControlLine($line) }
    else { [Console]::Out.WriteLine($line); [Console]::Out.Flush() }
}
function AbsolutePath([object]$value) {
    if ($value -isnot [string] -or $value.IndexOf([char]0) -ge 0 -or $value -notmatch '^(?:[A-Za-z]:[\\/]|\\\\[^\\/]+\\[^\\/]+(?:\\|$))') { throw 'arguments' }
    return [IO.Path]::GetFullPath($value)
}
try {
    if (-not [Environment]::Is64BitProcess -or [Environment]::OSVersion.Version.Major -lt 10) { throw 'platform' }
    # One bounded launch frame is read before starting the control pump. Reuse
    # Console.In afterwards: a second reader could lose already buffered bytes.
    $launchLine = [Text.StringBuilder]::new()
    while ($true) {
        $ch = [Console]::In.Read()
        if ($ch -lt 0) { throw 'launch-eof' }
        if ($ch -eq 10) { break }
        if ($launchLine.Length -ge 131072) { throw 'launch-limit' }
        [void]$launchLine.Append([char]$ch)
    }
    $spec = $launchLine.ToString().TrimEnd([char]13) | ConvertFrom-Json -ErrorAction Stop
    if ($spec.type -cne 'launch' -or $spec.args -isnot [Array] -or $spec.args.Length -gt 256 -or $null -eq $spec.env -or $null -eq $spec.pipes) { throw 'arguments' }
    $executable = AbsolutePath $spec.executable
    $cwd = AbsolutePath $spec.cwd
    if (-not [IO.File]::Exists($executable) -or -not [IO.Directory]::Exists($cwd)) { throw 'arguments' }
    $arguments = [Collections.Generic.List[string]]::new()
    foreach ($argument in $spec.args) {
        if ($argument -isnot [string] -or $argument.Length -gt 32767 -or $argument.IndexOf([char]0) -ge 0) { throw 'arguments' }
        $arguments.Add($argument)
    }
    if ($spec.lifetimeMs -isnot [int] -or $spec.lifetimeMs -lt 1000 -or $spec.lifetimeMs -gt 300000) { throw 'arguments' }
    $environment = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($property in $spec.env.PSObject.Properties) {
        if ($property.Value -isnot [string] -or $environment.ContainsKey($property.Name)) { throw 'environment' }
        $environment.Add($property.Name, $property.Value)
    }
    Add-Type -Path @((Join-Path $PSScriptRoot 'NativeJobProbe.cs'), (Join-Path $PSScriptRoot 'NativeJobBridge.cs')) -ErrorAction Stop
    $bridge = [Orchestrix.WindowsSpike.NativeJobBridge]::new()
    $control = [Orchestrix.WindowsSpike.BoundedLines]::new([Console]::In)
    $bridge.Start($executable, $arguments.ToArray(), $cwd, $environment, $spec.pipes.stdin, $spec.pipes.stdout, $spec.pipes.stderr, $spec.lifetimeMs)
    Emit 'ready' @{ rootPid = $bridge.Job.RootPid; jobConfigured = $bridge.Job.JobConfigured; rootInJob = $bridge.Job.RootInJob; resumed = $bridge.Job.Resumed; atomicAssignment = $true; jobHandleInherited = $false; watchdogArmed = $bridge.Job.WatchdogArmed }
    $rootReported = $false
    while ($true) {
        if ($bridge.Job.WatchdogTriggered) { $reason = 'deadline'; $exitCode = 2; break }
        if ($control.Faulted -or $bridge.Faulted) { throw 'bridge-io' }
        $controlLine = $control.Take()
        if ($null -ne $controlLine) {
            $command = $controlLine | ConvertFrom-Json -ErrorAction Stop
            if ($command.type -ceq 'stop') { $reason = 'requested'; $exitCode = 0; break }
            elseif ($command.type -ceq 'close-stdin') { $bridge.CloseInput() }
            elseif ($command.type -ceq 'inspect') {
                if (($command.pid -isnot [int] -and $command.pid -isnot [long]) -or $command.pid -le 0 -or $command.pid -gt [uint32]::MaxValue) { throw 'protocol' }
                $pidValue = [uint32]$command.pid
                Emit 'inspect' @{ pid = $pidValue; inOwnedJob = $bridge.Job.InOwnedJob($pidValue); activeCount = $bridge.Job.ActiveCount() }
            }
            else { throw 'protocol' }
        }
        if ($control.Ended -and $control.Empty) { $reason = 'control-eof'; $exitCode = 0; break }
        if (-not $rootReported -and $bridge.Job.RootExited) {
            Emit 'root.exit' @{ code = $bridge.Job.RootExitCode; activeCount = $bridge.Job.ActiveCount() }
            $rootReported = $true
        }
        if ($bridge.Job.ActiveCount() -eq 0) {
            if ($bridge.Job.WatchdogTriggered) { $reason = 'deadline'; $exitCode = 2 }
            else { $reason = 'natural-exit'; $exitCode = 0 }
            break
        }
        [Threading.Thread]::Sleep(10)
    }
}
catch {
    $exception = $_.Exception.GetBaseException()
    $category = 'helper'
    $nativeError = $null
    if ($exception.GetType().FullName -eq 'Orchestrix.WindowsSpike.ProbeException') { $category = $exception.Category; $nativeError = $exception.NativeError }
    elseif ($exception.Message -in @('platform', 'arguments', 'environment', 'protocol', 'bridge-io', 'launch-eof', 'launch-limit')) { $category = $exception.Message }
    Emit 'error' @{ category = $category; nativeError = $nativeError }
}
finally {
    if ($null -ne $bridge) {
        try {
            $bridge.BeginStop()
            # A graceful result needs observed emptiness and root exit BEFORE
            # requesting TerminateJobObject; helper exit alone proves neither.
            $count = $bridge.Job.ActiveCount()
            $rootExited = $bridge.Job.RootExited
            $graceful = $count -eq 0 -and $bridge.Job.RootPid -ne 0 -and $rootExited -and -not $bridge.Job.WatchdogTriggered
            if ($graceful) { $stopped = [Orchestrix.WindowsSpike.StopResult]::new(); $stopped.ActiveCount = 0; $stopped.RootExited = $true }
            else { $stopped = $bridge.Job.Stop(3000) }
            if (-not $rootReported -and $bridge.Job.RootPid -ne 0 -and $stopped.RootExited) {
                Emit 'root.exit' @{ code = $bridge.Job.RootExitCode; activeCount = $stopped.ActiveCount }
            }
            $streamsClosed = $bridge.WaitForStreams(1500)
            Emit 'streams.closed' @{ stdinClosed = $bridge.InputClosed; stdoutClosed = $bridge.OutputClosed; stderrClosed = $bridge.ErrorClosed; faulted = $bridge.Faulted }
            $confirmed = $stopped.ActiveCount -eq 0 -and ($bridge.Job.RootPid -eq 0 -or $stopped.RootExited)
            Emit 'stopped' @{ reason = $reason; activeCount = $(if ($stopped.ActiveCount -ge 0) { $stopped.ActiveCount } else { $null }); rootStarted = ($bridge.Job.RootPid -ne 0); rootExited = $stopped.RootExited; termination = $(if ($graceful) { 'graceful' } else { 'forced' }); treeTermination = $(if ($confirmed) { 'confirmed' } else { 'unknown' }) }
            if (-not $confirmed -or -not $streamsClosed -or $bridge.Faulted) { $exitCode = 1 }
        }
        catch {
            Emit 'stopped' @{ reason = $reason; activeCount = $null; rootStarted = ($bridge.Job.RootPid -ne 0); rootExited = $bridge.Job.RootExited; termination = 'forced'; treeTermination = 'unknown' }
            $exitCode = 1
        }
        finally {
            try { $bridge.Dispose() }
            catch { Emit 'error' @{ category = 'cleanup'; nativeError = $null }; $exitCode = 1 }
        }
    }
}
exit $exitCode
