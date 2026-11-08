# Real-GUI demand-storage measurement: N serial runs per path (dense / lazy),
# per-PID sampling (~300ms target; backend documented at Get-MemSample).
# The window is NOT auto-closed after
# ready: sampling continues through a fixed post-ready stable window with the
# live bounded cache, then the window is closed normally (CloseMainWindow,
# bounded wait; no force-kill by default).
#
# Launch/liveness architecture (minimal, no async I/O framework): the app is
# started via System.Diagnostics.ProcessStartInfo (redirected streams,
# per-child env vars) whose Process object exposes a reliable ExitCode (same
# pattern as tools/demand_proto_measure.ps1); stderr/stdout are drained live
# through ReadLineAsync tasks polled on the main thread (PS 5.1 event callbacks
# are not runspace-safe here). While the process is alive an INDEPENDENT
# Get-Process-by-PID object is captured with its own Handle and exact StartTime
# ticks; Refresh/CloseMainWindow/bounded WaitForExit go through that object.
#
# Aggregation: Get-Med = numeric sort, odd count -> floor-indexed middle,
# even count -> mean of the two middle values; decimals preserved (no int64
# truncation). ready_ms values stay in milliseconds end to end.
#
# Time bases are kept separate on purpose: ready_ms comes from the app's own
# [table-perf] first-ready-frame mark; the stable window is defined entirely in
# the sampler's clock (ready_detected_t_ms .. +StableSeconds) and samples are
# selected on that clock only — the two clocks are never mixed.
#
# Process safety: identity is re-verified by PID (exact start ticks + exe path
# + command line) immediately before any close. CloseMainWindow with a bounded
# wait is the only shutdown path; identity mismatch, a rejected/timed-out
# close, or an unknown exit code each fail the batch with the PID reported,
# abort remaining launches, keep raw logs, and skip the baseline append.
# exit_code is the real process code, never assumed; unknown never counts as 0.
#
# Failure gates: early exit before ready, missing ready mark, missing
# fingerprint, stable window not completed, insufficient stable samples, any
# shutdown/identity failure, fingerprint mismatch across runs or vs the frozen
# comparison statistics. Observed peaks are sampled maxima (NOT instantaneous
# true peaks); os_working_set_peak is the OS history counter.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_gui_measure.ps1 [-Runs 3] [-StableSeconds 12] [-NoBaseline]
param([int]$Runs = 3, [double]$StableSeconds = 12, [switch]$NoBaseline)
$ErrorActionPreference = 'Stop'

$Root   = 'D:\Github\Matcha'
$Exe    = "$Root\target\release\matcha.exe"
$Left   = "$Root\target\table-wave7\audit\inputs\table-left.csv"
$Right  = "$Root\target\table-wave7\audit\inputs\table-right.csv"
$LogDir = "$Root\target\memory-audit\gui"
New-Item -ItemType Directory -Force $LogDir | Out-Null

# Input identity is MEASURED at batch start (full read of both files); the
# constants are expectations used only for validation, not recorded values.
$ExpectedLeftHash  = '1c7f090e6863bc81d4ce3c2802017ced9589363dafaca50be6b259e0410886d7'
$ExpectedRightHash = '53c033e4a8feb2c54eec8636884c690c4c2f1a3635fc461637a846166b479c81'
$ActualLeftHash  = (Get-FileHash $Left  -Algorithm SHA256).Hash.ToLower()
$ActualRightHash = (Get-FileHash $Right -Algorithm SHA256).Hash.ToLower()
if ($ActualLeftHash -ne $ExpectedLeftHash -or $ActualRightHash -ne $ExpectedRightHash) {
    Write-Host "INPUT MISMATCH: left=$ActualLeftHash right=$ActualRightHash; nothing measured"
    exit 1
}
if (-not (Test-Path $Exe)) { Write-Host "missing $Exe (cargo build --release --bin matcha)"; exit 1 }

# Sampler backend (2026-11-08, coordinator-approved): PB/WS/WS-peak read from
# the retained Get-Process object (PrivateMemorySize64/WorkingSet64/
# PeakWorkingSet64), PWS from the per-instance performance counter
# '\Process(<instance>)\Working Set - Private'. The PID->instance mapping is
# resolved ONCE per run at launch via the 'ID Process' counter; zero or
# multiple matches fail the whole batch (no guessing). The previous CIM
# Win32_PerfRawData_PerfProc_Process query enumerates every process row on
# the host (~350-410ms/call here, ~690 rows), overruning the 300ms cadence
# and starving the 12s stable window below its minimum-sample gate. Metric
# definitions, cadence, window and all gates are unchanged; these are the
# same OS counters the CIM provider exposes.
function Resolve-PwsCounter {
    param([int]$TargetPid)
    $category = [System.Diagnostics.PerformanceCounterCategory]::new('Process')
    $found = @()
    foreach ($instance in $category.GetInstanceNames()) {
        if ($instance -eq '_Total' -or $instance -eq 'Idle') { continue }
        try {
            $idCounter = [System.Diagnostics.PerformanceCounter]::new('Process', 'ID Process', $instance, $true)
            $id = [int]$idCounter.NextValue()
            $idCounter.Dispose()
            if ($id -eq $TargetPid) { $found += $instance }
        } catch { }
    }
    $found = @($found | Sort-Object -Unique)
    if ($found.Count -ne 1) { return $null }
    $counter = [System.Diagnostics.PerformanceCounter]::new('Process', 'Working Set - Private', $found[0], $true)
    [void]$counter.NextValue()  # prime; first read can be stale
    [pscustomobject]@{ Instance = $found[0]; Counter = $counter }
}
function Get-MemSample([System.Diagnostics.Process]$Proc, $Pws, [double]$T0Ms) {
    try {
        $Proc.Refresh()
        if ($Proc.HasExited) { return $null }
        $pwsValue = $Pws.Counter.NextValue()
        $now = [Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)
        [pscustomobject]@{
            t_ms              = [math]::Round($now - $T0Ms, 1)
            PrivateBytes      = [int64]$Proc.PrivateMemorySize64
            WorkingSet        = [int64]$Proc.WorkingSet64
            WorkingSetPrivate = [int64]$pwsValue
            WorkingSetPeak    = [int64]$Proc.PeakWorkingSet64
        }
    } catch { return $null }
}
# Median: numeric sort; odd count -> floor-indexed middle element; even count
# -> mean of the two middle elements. Decimal-preserving (no integer cast).
function Get-Med($values) {
    $arr = @($values | Sort-Object)
    if ($arr.Count -eq 0) { return $null }
    $mid = [math]::Floor($arr.Count / 2)
    if ($arr.Count % 2 -eq 1) { return [double]$arr[$mid] }
    return ([double]$arr[$mid - 1] + [double]$arr[$mid]) / 2
}
function Get-Mark($marks, [string]$name) {
    $m = $marks | Where-Object { $_.mark -eq $name } | Select-Object -First 1
    if ($null -eq $m) { return $null }
    return $m.t_ms
}

# --- live stream drain (main-thread polling, no PS event handlers) ---------
function New-LineDrain {
    param([System.IO.StreamReader]$Reader)
    [pscustomobject]@{ Reader = $Reader; Pending = $Reader.ReadLineAsync(); Text = [System.Text.StringBuilder]::new() }
}
function Read-LineDrain {
    param([pscustomobject]$Drain)
    while ($null -ne $Drain.Pending -and $Drain.Pending.IsCompleted) {
        $line = $Drain.Pending.Result
        if ($null -eq $line) { $Drain.Pending = $null; break }  # EOF
        [void]$Drain.Text.AppendLine($line)
        $Drain.Pending = $Drain.Reader.ReadLineAsync()
    }
    return $Drain.Text.ToString()
}
# Drain until EOF on both pipes (bounded); used after the process exits.
function Complete-LineDrains {
    param([pscustomobject]$Err, [pscustomobject]$Out, [double]$WaitSeconds = 5)
    $deadline = (Get-Date).AddSeconds($WaitSeconds)
    while ((Get-Date) -lt $deadline) {
        Read-LineDrain $Err | Out-Null
        Read-LineDrain $Out | Out-Null
        if ($null -eq $Err.Pending -and $null -eq $Out.Pending) { return }
        Start-Sleep -Milliseconds 100
    }
}

# Per-child environment for a measured launch. ProcessStartInfo.
# EnvironmentVariables inherits the PARENT environment on first access, so the
# experiment switches must be removed explicitly before setting ours:
# a user who enabled lazy/PERF_EXIT in the parent shell must not leak them
# into a run labeled dense (or cause a premature exit).
function Set-MeasuredProcessEnv {
    param([System.Diagnostics.ProcessStartInfo]$Psi, [bool]$Lazy)
    [void]$Psi.EnvironmentVariables.Remove('MATCHA_TABLE_CSV_LAZY')
    [void]$Psi.EnvironmentVariables.Remove('MATCHA_TABLE_PERF_EXIT')
    $Psi.EnvironmentVariables['MATCHA_TABLE_PERF'] = '1'
    if ($Lazy) { $Psi.EnvironmentVariables['MATCHA_TABLE_CSV_LAZY'] = '1' }
}

# --- process identity & safe shutdown (safe-close protocol) ---------------

# Capture identity once, immediately after launch, on an INDEPENDENT handle
# object (Get-Process by PID, not the launcher return value). The Process
# object is RETAINED in the identity record: its open handle pins the PID (no
# PID reuse while held), and all later Refresh/CloseMainWindow/bounded
# WaitForExit/ExitCode go through this same object, never a fresh lookup.
function New-ProcessIdentity {
    param([System.Diagnostics.Process]$Process, [string[]]$ExpectedArgs)
    # Actually open the handle now: accessing .Handle pins the OS handle on the
    # retained Process object, so the PID cannot be reused while we hold it.
    # The reference is kept through exit-code reading; never serialized to JSON.
    [void]$Process.Handle
    [pscustomobject]@{
        Process      = $Process
        Pid          = $Process.Id
        StartTicks   = $Process.StartTime.ToUniversalTime().Ticks
        ExePath      = $Process.Path
        ExpectedArgs = $ExpectedArgs
    }
}
# Re-verify on the retained object right before any close: exact start ticks
# and exe path on the same handle, plus every expected command-line substring
# via CIM for that PID.
function Test-ProcessIdentity {
    param([pscustomobject]$Identity)
    $proc = $Identity.Process
    if ($null -eq $proc) { return $false }
    $proc.Refresh()
    if ($proc.HasExited) { return $false }
    if ($proc.Path -ne $Identity.ExePath) { return $false }
    if ($proc.StartTime.ToUniversalTime().Ticks -ne $Identity.StartTicks) { return $false }
    $cim = Get-CimInstance Win32_Process -Filter "ProcessId=$($Identity.Pid)" -ErrorAction SilentlyContinue
    if ($null -eq $cim -or $null -eq $cim.CommandLine) { return $false }
    foreach ($a in $Identity.ExpectedArgs) {
        if (-not $cim.CommandLine.Contains($a)) { return $false }
    }
    return $true
}
# Normal close only, on the retained identity object: Refresh + CloseMainWindow
# + bounded wait. Never force-kills. Returns 'AlreadyExited' |
# 'ClosedByMainWindow' | "Refused:<reason>".
function Stop-OwnProcess {
    param([pscustomobject]$Identity, [double]$WaitSeconds = 20)
    $proc = $Identity.Process
    if ($null -eq $proc) { return 'Refused:no-retained-handle' }
    $proc.Refresh()
    if ($proc.HasExited) { return 'AlreadyExited' }
    if (-not (Test-ProcessIdentity $Identity)) { return 'Refused:identity-mismatch' }
    if (-not $proc.CloseMainWindow()) { return 'Refused:close-main-window-rejected' }
    if ($proc.WaitForExit([int]($WaitSeconds * 1000))) { return 'ClosedByMainWindow' }
    return 'Refused:close-main-window-timeout'
}

# Stable-window selection on the SAMPLER clock only (never the app mark clock).
function Get-StableWindowSamples {
    param($Samples, [double]$ReadyDetectedTms, [double]$WindowEndTms)
    @($Samples | Where-Object { $_.t_ms -ge $ReadyDetectedTms -and $_.t_ms -lt $WindowEndTms })
}
# Minimum samples for a complete window: half of the nominal 300ms cadence.
function Get-StableMinSamples([double]$StableSeconds) {
    [math]::Max(2, [math]::Floor($StableSeconds * 1000 / 300 / 2))
}

$batchId = Get-Date -Format 'yyyyMMdd-HHmmss'
$RunDir = "$LogDir\batch-$batchId"
New-Item -ItemType Directory -Force $RunDir | Out-Null
$batch = @()
$failures = @()
$abort = $false
foreach ($mode in @('dense', 'lazy')) {
    if ($abort) { break }
    for ($i = 1; $i -le $Runs; $i++) {
        if ($abort) { break }
        $errFile = "$RunDir\$mode-run$i.stderr.log"
        $outFile = "$RunDir\$mode-run$i.stdout.log"
        Remove-Item $errFile, $outFile -ErrorAction SilentlyContinue
        # ProcessStartInfo launch: per-child env vars, redirected streams, and a
        # launcher Process object with a reliable ExitCode (prototype-proven).
        $psi = [System.Diagnostics.ProcessStartInfo]::new()
        $psi.FileName = $Exe
        $psi.Arguments = "`"$Left`" `"$Right`""
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        Set-MeasuredProcessEnv $psi ($mode -eq 'lazy')
        $t0 = [Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)
        $p = [System.Diagnostics.Process]::Start($psi)
        $errDrain = New-LineDrain $p.StandardError
        $outDrain = New-LineDrain $p.StandardOutput
        # Independent handle object captured while the process is alive and
        # retained in the identity (pins the PID; all close/exit status below).
        $app = Get-Process -Id $p.Id -ErrorAction Stop
        $identity = New-ProcessIdentity $app @($Exe, $Left, $Right)
        $pid_ = $identity.Pid
        Write-Host "$mode run $i pid=$pid_"
        $runFailed = $false

        # Resolve the PID -> Process perf-counter instance ONCE at launch
        # (bounded wait for counter registration); unresolved or ambiguous
        # mapping fails the whole batch, no guessing.
        $pwsCounter = $null
        for ($k = 0; $k -lt 20 -and $null -eq $pwsCounter; $k++) {
            $pwsCounter = Resolve-PwsCounter $pid_
            if ($null -eq $pwsCounter) { Start-Sleep -Milliseconds 250 }
        }
        if ($null -eq $pwsCounter) {
            $failures += "${mode} run${i}: no unique Process perf-counter instance for pid=$pid_"
            $runFailed = $true
        }

        # Phase 1: wait for the real ready mark (first-ready-frame in stderr).
        $readyDetectedTms = $null
        $samples = @()
        while (-not $runFailed -and -not $app.HasExited -and $null -eq $readyDetectedTms) {
            $s = Get-MemSample $app $pwsCounter $t0
            if ($null -ne $s) { $samples += $s }
            if ((Read-LineDrain $errDrain) -match 'first-ready-frame') {
                $readyDetectedTms = [Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0) - $t0
            }
            Start-Sleep -Milliseconds 300
            if ((([Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)) - $t0) -gt 180000) { break }
        }
        if (-not $runFailed) {
            if ($app.HasExited -and $null -eq $readyDetectedTms) {
                $earlyExit = try { $p.ExitCode } catch { 'unknown' }
                $failures += "${mode} run${i}: early exit before ready (pid=$pid_, exit=$earlyExit)"
                $runFailed = $true
            } elseif ($null -eq $readyDetectedTms) {
                $failures += "${mode} run${i}: no first-ready-frame within 180s (pid=$pid_)"
                $runFailed = $true
            }
        }

        # Phase 2: stable window, sampler clock only; window open, cache live.
        $windowEndTms = $null
        $windowCompleted = $false
        if (-not $runFailed) {
            $windowEndTms = $readyDetectedTms + ($StableSeconds * 1000)
            while ((([Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)) - $t0) -lt $windowEndTms) {
                if ($app.HasExited) { break }
                $s = Get-MemSample $app $pwsCounter $t0
                if ($null -ne $s) { $samples += $s }
                Read-LineDrain $errDrain | Out-Null
                Read-LineDrain $outDrain | Out-Null
                Start-Sleep -Milliseconds 300
            }
            $windowCompleted = (([Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)) - $t0) -ge $windowEndTms
            if (-not $windowCompleted) {
                $failures += "${mode} run${i}: stable window incomplete (pid=$pid_, exited=$($app.HasExited))"
                $runFailed = $true
            }
        }

        # Shutdown: normal close only, identity re-verified, bounded wait.
        $shutdownMethod = 'AlreadyExited'
        if (-not $app.HasExited) {
            $shutdownMethod = Stop-OwnProcess $identity
            if ($shutdownMethod.StartsWith('Refused')) {
                $failures += "${mode} run${i}: shutdown $shutdownMethod (pid=$pid_)"
                $runFailed = $true
            }
        }
        # Real exit code, primary read on the retained identity object, with
        # one launcher-object fallback; unknown is never treated as 0 and fails
        # the batch (no unbounded waits anywhere).
        $exitCode = $null
        for ($k = 0; $k -lt 20 -and -not $app.HasExited; $k++) {
            Start-Sleep -Milliseconds 250
            $app.Refresh()
        }
        if ($app.HasExited) {
            $exitCode = try { $app.ExitCode } catch { $null }
        }
        if ($null -eq $exitCode) {
            $p.Refresh()
            if ($p.HasExited) {
                $exitCode = try { $p.ExitCode } catch { $null }
            }
        }
        if ($null -eq $exitCode) {
            $failures += "${mode} run${i}: exit unknown after close (pid=$pid_, exited=$($app.HasExited))"
            $runFailed = $true
        }
        Complete-LineDrains $errDrain $outDrain
        if ($null -ne $pwsCounter) { $pwsCounter.Counter.Dispose() }
        $stderr = $errDrain.Text.ToString()
        $stdout = $outDrain.Text.ToString()
        [IO.File]::WriteAllText($errFile, $stderr)
        [IO.File]::WriteAllText($outFile, $stdout)

        $marks = @([regex]::Matches($stderr, '\[table-perf\] t=\s*([0-9.]+)ms (\S+)') | ForEach-Object { @{ t_ms = [double]$_.Groups[1].Value; mark = $_.Groups[2].Value } })
        $readyMs = Get-Mark $marks 'app:first-ready-frame'
        $fingerprint = ([regex]::Match($stderr, '\[table-perf\] fingerprint .*')).Value.TrimEnd("`r")
        $mappedBytes = [string]([regex]::Matches($stderr, 'demand-mapped-bytes=(\d+)') | ForEach-Object { $_.Groups[1].Value } | Measure-Object -Sum).Sum
        $stable = if ($null -ne $readyDetectedTms -and $null -ne $windowEndTms) {
            Get-StableWindowSamples $samples $readyDetectedTms $windowEndTms
        } else { @() }
        $load = if ($null -ne $readyDetectedTms) {
            @($samples | Where-Object { $_.t_ms -lt $readyDetectedTms })
        } else { @($samples) }
        if ($null -eq $readyMs) { $failures += "${mode} run${i}: missing ready mark"; $runFailed = $true }
        if ([string]::IsNullOrEmpty($fingerprint)) { $failures += "${mode} run${i}: missing fingerprint"; $runFailed = $true }
        $minStable = Get-StableMinSamples $StableSeconds
        if (-not $runFailed -and $stable.Count -lt $minStable) {
            $failures += "${mode} run${i}: insufficient stable samples ($($stable.Count) < $minStable)"
            $runFailed = $true
        }

        $rec = [ordered]@{
            mode = $mode; run = $i; pid = $pid_
            start_ticks_utc = $identity.StartTicks
            exit_code = $exitCode
            shutdown_method = $shutdownMethod
            ready_ms = $readyMs
            ready_detected_t_ms = $readyDetectedTms
            stable_window_start_t_ms = $readyDetectedTms
            stable_window_end_t_ms = $windowEndTms
            stable_window_completed = $windowCompleted
            fingerprint = $fingerprint
            sampler_backend = 'Get-Process(PrivateMemorySize64/WorkingSet64/PeakWorkingSet64)+PerformanceCounter(Process/Working Set - Private); PID->instance resolved once at launch via ID Process counter'
            sampler_instance = if ($null -ne $pwsCounter) { $pwsCounter.Instance } else { $null }
            mapped_virtual_bytes = $mappedBytes
            marks = $marks
            sample_interval_ms_target = 300
            stable_n = $stable.Count
            stable_private_bytes = Get-Med ($stable | ForEach-Object PrivateBytes)
            stable_working_set = Get-Med ($stable | ForEach-Object WorkingSet)
            stable_working_set_private = Get-Med ($stable | ForEach-Object WorkingSetPrivate)
            load_observed_peak_private_bytes = ($load | ForEach-Object PrivateBytes | Measure-Object -Maximum).Maximum
            observed_peak_private_bytes = ($samples | ForEach-Object PrivateBytes | Measure-Object -Maximum).Maximum
            os_working_set_peak = ($samples | ForEach-Object WorkingSetPeak | Measure-Object -Maximum).Maximum
            samples = $samples
        }
        $rec | ConvertTo-Json -Depth 5 | Set-Content "$RunDir\$mode-run$i.json" -Encoding UTF8
        $batch += $rec
        $readyStr = if ($null -ne $readyMs) { [math]::Round($readyMs, 1) } else { 'n/a' }
        Write-Host "$mode run $i done (ready=${readyStr}ms exit=$exitCode shutdown=$shutdownMethod stable_n=$($stable.Count))"
        if ($runFailed) {
            # Never keep launching instances after a failed run: identity,
            # close, exit, ready, fingerprint, window or sample problems all
            # abort the batch; raw logs are kept, no baseline is appended.
            $abort = $true
        }
        Start-Sleep -Seconds 2
    }
}

# Same-comparison gate: identical fingerprint across every run of both paths.
$fingerprints = @($batch | ForEach-Object { $_.fingerprint } | Where-Object { -not [string]::IsNullOrEmpty($_) } | Sort-Object -Unique)
$expectedFp = '[table-perf] fingerprint rows=1000000 same=990000 unimportant=0 important=10000 left_only=0 right_only=0'
if ($batch.Count -lt (2 * $Runs)) {
    $failures += "batch aborted: $($batch.Count) of $(2 * $Runs) runs completed"
}
if ($fingerprints.Count -ne 1) {
    $failures += "fingerprint mismatch across runs/paths: $($fingerprints -join ' ; ')"
} elseif ($fingerprints[0] -ne $expectedFp) {
    $failures += "fingerprint differs from frozen comparison statistics: $($fingerprints[0])"
}
if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Host "GATE FAIL: $_" }
    Write-Host "history NOT appended (raw kept under $RunDir)"
    exit 1
}
if ($NoBaseline) {
    Write-Host "probe mode: history NOT appended (raw under $RunDir)"
    exit 0
}

$dense = @($batch | Where-Object { $_.mode -eq 'dense' })
$lazy  = @($batch | Where-Object { $_.mode -eq 'lazy' })
$history = [ordered]@{
    batch_at = (Get-Date).ToString('o')
    timing_basis = 'ready_ms = app:first-ready-frame mark (app clock); stable window = sampler clock from ready detection (+StableSeconds); normal close, no auto-exit'
    sampler_backend = 'Get-Process(PrivateMemorySize64/WorkingSet64/PeakWorkingSet64)+PerformanceCounter(Process/Working Set - Private, per-PID instance); replaces CIM Win32_PerfRawData_PerfProc_Process (~350-410ms/query overran the 300ms cadence on this host); same OS counters, gates unchanged'
    aggregator = 'Get-Med v2: numeric sort; odd -> floor-indexed middle; even -> mean of middle two; decimal-preserving; ready medians in ms'
    raw_dir = "target/memory-audit/gui/batch-$batchId"
    revision = (git -C $Root rev-parse HEAD)
    dirty = ((git -C $Root status --porcelain).Length -gt 0)
    exe_sha256 = (Get-FileHash $Exe -Algorithm SHA256).Hash.ToLower()
    input_left_sha256 = $ActualLeftHash
    input_right_sha256 = $ActualRightHash
    input_preread = 'both inputs fully read for SHA256 immediately before batch'
    entry = 'matcha.exe <left> <right> (+MATCHA_TABLE_CSV_LAZY=1 for lazy); MATCHA_TABLE_PERF=1 both'
    stable_seconds = $StableSeconds
    fingerprint = $fingerprints[0]
    ready_ms_dense_median = Get-Med ($dense | ForEach-Object { $_.ready_ms })
    ready_ms_lazy_median = Get-Med ($lazy | ForEach-Object { $_.ready_ms })
    exit_codes = @($batch | ForEach-Object { $_.exit_code })
    shutdown_methods = @($batch | ForEach-Object { $_.shutdown_method } | Sort-Object -Unique)
    runs = @($batch | ForEach-Object {
        [ordered]@{ mode = $_.mode; run = $_.run; pid = $_.pid
            exit_code = $_.exit_code
            shutdown_method = $_.shutdown_method
            ready_ms = $_.ready_ms
            stable_private_bytes = $_.stable_private_bytes
            stable_working_set = $_.stable_working_set
            stable_working_set_private = $_.stable_working_set_private
            load_observed_peak_private_bytes = $_.load_observed_peak_private_bytes
            observed_peak_private_bytes = $_.observed_peak_private_bytes
            os_working_set_peak = $_.os_working_set_peak
            mapped_virtual_bytes = $_.mapped_virtual_bytes
            fingerprint = $_.fingerprint }
    })
}
$histPath = "$Root\tools\demand_gui_baseline.json"
$existing = @()
if (Test-Path $histPath) {
    $parsed = Get-Content $histPath -Raw | ConvertFrom-Json
    if ($parsed.batches) { $existing = @($parsed.batches) } elseif ($parsed.runs) { $existing = @($parsed) }
}
@{ batches = @($existing + $history) } | ConvertTo-Json -Depth 6 | Set-Content $histPath -Encoding UTF8
Write-Host "wrote $histPath"
exit 0
