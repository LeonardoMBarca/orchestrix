# Synthetic-only spike. Keep this separate from JsonRpcProcess and real runtime/provider paths.
param(
    [Parameter(Mandatory = $true)][string]$NodePath,
    [Parameter(Mandatory = $true)][string]$FixturePath,
    [string]$Scenario = 'tree',
    [int]$LifetimeMs = 20000,
    [ValidateSet('none', 'before-resume')][string]$FailureStage = 'none'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$script:sequence = 0
$script:ownerId = [Guid]::NewGuid().ToString('N')
$session = $null
$reason = 'failure'
$exitCode = 1
function Emit([string]$type, [hashtable]$fields) {
    $record = [ordered]@{ schemaVersion = 1; sequence = ++$script:sequence; type = $type; ownerId = $script:ownerId }
    foreach ($key in $fields.Keys) { $record[$key] = $fields[$key] }
    $line = $record | ConvertTo-Json -Depth 4 -Compress
    if ($null -ne $session) { $session.WriteControlLine($line) }
    else { [Console]::Out.WriteLine($line); [Console]::Out.Flush() }
}
try {
    if (-not [Environment]::Is64BitProcess -or [Environment]::OSVersion.Version.Major -lt 10) { throw 'platform' }
    if ($LifetimeMs -lt 1000 -or $LifetimeMs -gt 30000 -or $Scenario -notmatch '^[a-zA-Z0-9_-]{1,30}$') { throw 'arguments' }
    if (-not [IO.Path]::IsPathRooted($NodePath) -or -not [IO.Path]::IsPathRooted($FixturePath)) { throw 'arguments' }
    $NodePath = [IO.Path]::GetFullPath($NodePath)
    $FixturePath = [IO.Path]::GetFullPath($FixturePath)
    if ([IO.Path]::GetFileName($NodePath) -ine 'node.exe' -or -not [IO.File]::Exists($FixturePath)) { throw 'arguments' }
    # Copies with spaces/Unicode are allowed only when their bytes match our synthetic fixture.
    $referenceFixture = Join-Path $PSScriptRoot '../fixtures/windows-job-tree.mjs'
    if ((Get-FileHash -LiteralPath $FixturePath -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $referenceFixture -Algorithm SHA256).Hash) { throw 'arguments' }
    Add-Type -Path (Join-Path $PSScriptRoot 'NativeJobProbe.cs') -ErrorAction Stop
    $session = [Orchestrix.WindowsSpike.NativeJobProbe]::new()
    $fixtureArgs = [string[]]@($FixturePath, '--scenario', $Scenario, '--lifetime-ms', [string]$LifetimeMs)
    $session.Launch($NodePath, $fixtureArgs, [IO.Path]::GetDirectoryName($FixturePath), $FailureStage -eq 'before-resume')
    $session.ArmWatchdog($LifetimeMs, 1000)
    Emit 'ready' @{ rootPid = $session.RootPid; jobConfigured = $session.JobConfigured; rootInJob = $session.RootInJob; resumed = $session.Resumed; atomicAssignment = $true; jobHandleInherited = $false; watchdogArmed = $session.WatchdogArmed }
    $rootReported = $false
    $fixtureReported = $false
    while ($true) {
        if ($session.WatchdogTriggered) { $reason = 'deadline'; $exitCode = 2; break }
        if ($session.Control.Faulted -or $session.Output.Faulted -or $session.StderrFaulted) { throw 'protocol' }
        $controlLine = $session.Control.Take()
        if ($null -ne $controlLine) {
            $command = $controlLine | ConvertFrom-Json -ErrorAction Stop
            if ($command.type -eq 'stop') { $reason = 'requested'; $exitCode = 0; break }
            if ($command.type -eq 'exit-root') { $session.ExitRoot() }
            elseif ($command.type -eq 'inspect') {
                $pidValue = [uint32]$command.pid
                if ($pidValue -eq 0) { throw 'protocol' }
                Emit 'inspect' @{ pid = $pidValue; inOwnedJob = $session.InOwnedJob($pidValue); activeCount = $session.ActiveCount() }
            }
            elseif ($command.type -eq 'output-flood') {
                # Synthetic proof hook: bounded known data, one control command.
                # It can block the writer without overflowing the input queue.
                Emit 'output.flood.start' @{ frameCount = 4096; paddingCharacters = 4096 }
                $padding = '.' * 4096
                for ($frameIndex = 0; $frameIndex -lt 4096; $frameIndex++) {
                    Emit 'output.flood' @{ index = $frameIndex; padding = $padding }
                }
            }
            else { throw 'protocol' }
        }
        if ($session.Control.Ended -and $session.Control.Empty) { $reason = 'stdin-eof'; $exitCode = 0; break }
        $fixtureLine = $session.Output.Take()
        if ($null -ne $fixtureLine) {
            $fixture = $fixtureLine | ConvertFrom-Json -ErrorAction Stop
            if ([uint32]$fixture.rootPid -ne $session.RootPid) { throw 'protocol' }
            if ($fixture.type -eq 'fixture.rootExiting' -and $fixtureReported) {
                # Intention is not a terminal: emit root.exit only after the owned handle signals.
            }
            elseif ($fixture.type -eq 'fixture.ready' -and -not $fixtureReported) {
                $descendantPid = [uint32]$fixture.descendantPid
                $rootMember = $session.InOwnedJob($session.RootPid)
                $descendantMember = $session.InOwnedJob($descendantPid)
                if ($descendantPid -eq 0 -or -not $rootMember -or -not $descendantMember) { throw 'membership' }
                Emit 'fixture.ready' @{ rootPid = $session.RootPid; descendantPid = $descendantPid; lifetimeMs = [int]$fixture.lifetimeMs; rootInJob = $rootMember; descendantInJob = $descendantMember }
                $fixtureReported = $true
            }
            else { throw 'protocol' }
        }
        if (-not $rootReported -and $session.RootExited) {
            Emit 'root.exit' @{ code = $session.RootExitCode; activeCount = $session.ActiveCount() }
            $rootReported = $true
        }
        if ($session.ActiveCount() -eq 0) {
            if ($session.WatchdogTriggered) { $reason = 'deadline'; $exitCode = 2 }
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
    elseif ($exception.Message -in @('platform', 'arguments', 'protocol', 'membership')) { $category = $exception.Message }
    if ($category -eq 'injected-before-resume') { $exitCode = 2 }
    Emit 'error' @{ category = $category; nativeError = $nativeError }
}
finally {
    if ($null -ne $session) {
        try {
            $stopped = $session.Stop(3000)
            $count = if ($stopped.ActiveCount -ge 0) { $stopped.ActiveCount } else { $null }
            $confirmed = $stopped.ActiveCount -eq 0 -and ($session.RootPid -eq 0 -or $stopped.RootExited)
            Emit 'stopped' @{ reason = $reason; activeCount = $count; rootStarted = ($session.RootPid -ne 0); rootExited = $stopped.RootExited; termination = 'forced'; treeTermination = $(if ($confirmed) { 'confirmed' } else { 'unknown' }) }
            if (-not $confirmed) { $exitCode = 1 }
        }
        catch { Emit 'stopped' @{ reason = $reason; activeCount = $null; rootStarted = ($session.RootPid -ne 0); rootExited = $session.RootExited; termination = 'forced'; treeTermination = 'unknown' }; $exitCode = 1 }
        finally {
            try { $session.Dispose() }
            catch { Emit 'error' @{ category = 'cleanup'; nativeError = $null }; $exitCode = 1 }
        }
    }
}
exit $exitCode
