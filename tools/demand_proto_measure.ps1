# demand_proto measurement: 3 serial runs per mode, per-PID WMI sampling.
# Metrics raw bytes (MiB = /1048576 at analysis). Sampling ~300ms target; observed peaks
# are sampled maxima (NOT true instantaneous peaks); os_working_set_peak is the OS counter.
# mapped_virtual_bytes = virtual mapping size only; mapped-page residency is NOT separately
# measured (unknown) and is included in total WorkingSet once touched.
# Phase times: core_compare_ms = existing [proto] compare mark; digest/viewport separate.
# Failure gates: missing summary/cells/compare/hold:end, non-zero exit code, or digest
# mismatch across runs/modes => script writes nothing to baseline and exits non-zero.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_proto_measure.ps1 [-Runs 3]
param([int]$Runs = 3, [switch]$NoBaseline)
$ErrorActionPreference = 'Stop'

$Root   = 'D:\Github\Matcha'
$Exe    = "$Root\target\release\examples\demand_proto.exe"
$Left   = "$Root\target\table-wave7\audit\inputs\table-left.csv"
$Right  = "$Root\target\table-wave7\audit\inputs\table-right.csv"
$LogDir = "$Root\target\memory-audit\prototype"
New-Item -ItemType Directory -Force $LogDir | Out-Null

# Input identity is MEASURED at batch start (full read of both files); the
# historical constants below are expectations used only for validation, not a
# substitute for the measured values. This hash pass also means every batch
# runs in a just-pre-read condition of these two files.
$ExpectedLeftHash  = '1c7f090e6863bc81d4ce3c2802017ced9589363dafaca50be6b259e0410886d7'
$ExpectedRightHash = '53c033e4a8feb2c54eec8636884c690c4c2f1a3635fc461637a846166b479c81'
$ActualLeftHash  = (Get-FileHash $Left  -Algorithm SHA256).Hash.ToLower()
$ActualRightHash = (Get-FileHash $Right -Algorithm SHA256).Hash.ToLower()
# Input identity gate: refuse to measure unknown inputs (expected values are
# validation constants; the measured values are what gets recorded).
if ($ActualLeftHash -ne $ExpectedLeftHash -or $ActualRightHash -ne $ExpectedRightHash) {
    Write-Host "INPUT MISMATCH: left=$ActualLeftHash right=$ActualRightHash (expected $ExpectedLeftHash / $ExpectedRightHash); nothing measured"
    exit 1
}

function Get-MemSample([int]$TargetPid, [double]$T0Ms) {
    $s = Get-CimInstance Win32_PerfRawData_PerfProc_Process -Filter "IDProcess=$TargetPid" -ErrorAction SilentlyContinue
    if ($null -eq $s) { return $null }
    $now = [Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)
    [pscustomobject]@{
        t_ms              = [math]::Round($now - $T0Ms, 1)
        PrivateBytes      = [int64]$s.PrivateBytes
        WorkingSet        = [int64]$s.WorkingSet
        WorkingSetPrivate = [int64]$s.WorkingSetPrivate
        WorkingSetPeak    = [int64]$s.WorkingSetPeak
    }
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

$batchId = Get-Date -Format 'yyyyMMdd-HHmmss'
$RunDir = "$LogDir\batch-$batchId"
New-Item -ItemType Directory -Force $RunDir | Out-Null
$batch = @()
$failures = @()
foreach ($mode in @('reference', 'demand')) {
    for ($i = 1; $i -le $Runs; $i++) {
        $outFile = "$RunDir\$mode-run$i.stdout.log"
        $errFile = "$RunDir\$mode-run$i.stderr.log"
        Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue

        $psi = [System.Diagnostics.ProcessStartInfo]::new()
        $psi.FileName = $Exe
        $psi.Arguments = "--mode=$mode --left=$Left --right=$Right --hold=12"
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.CreateNoWindow = $true
        $t0 = [Diagnostics.Stopwatch]::GetTimestamp() / ([Diagnostics.Stopwatch]::Frequency / 1000.0)
        $p = [System.Diagnostics.Process]::Start($psi)
        $pid_ = $p.Id
        Write-Host "$mode run $i pid=$pid_"
        $samples = @()
        while (-not $p.HasExited) {
            $s = Get-MemSample $pid_ $t0
            if ($null -ne $s) { $samples += $s }
            Start-Sleep -Milliseconds 300
        }
        $p.WaitForExit()
        $exitCode = $p.ExitCode
        $stdout = $p.StandardOutput.ReadToEnd()
        $stderr = $p.StandardError.ReadToEnd()
        [IO.File]::WriteAllText($outFile, $stdout)
        [IO.File]::WriteAllText($errFile, $stderr)

        $summary = ([regex]::Match($stdout, 'summary .*')).Value
        $cellsLine = ([regex]::Match($stdout, 'cells .*')).Value
        $viewport = ([regex]::Match($stdout, 'viewport .*')).Value
        $mappedBytes = ([regex]::Match($stderr, 'mapped_bytes=(\d+)')).Groups[1].Value
        $marks = @([regex]::Matches($stderr, '\[proto\] t=\s*([0-9.]+)ms (\S+)') | ForEach-Object { @{ t_ms = [double]$_.Groups[1].Value; mark = $_.Groups[2].Value } })
        $holdBeginMs = Get-Mark $marks 'hold:begin'
        $holdEndMs = Get-Mark $marks 'hold:end'
        $compareMs = Get-Mark $marks 'compare'
        $stable = @($samples | Where-Object { $null -ne $holdBeginMs -and $_.t_ms -ge $holdBeginMs })
        $load   = @($samples | Where-Object { $null -eq $holdBeginMs -or $_.t_ms -lt $holdBeginMs })

        # gates for THIS run
        if ([string]::IsNullOrEmpty($summary))  { $failures += "${mode} run${i}: missing summary" }
        if ([string]::IsNullOrEmpty($cellsLine)) { $failures += "${mode} run${i}: missing cells line" }
        if ($null -eq $compareMs)  { $failures += "${mode} run${i}: missing compare mark" }
        if ($null -eq $holdEndMs)  { $failures += "${mode} run${i}: missing hold:end mark" }
        if ($exitCode -ne 0)       { $failures += "${mode} run${i}: exit code $exitCode" }

        $rec = [ordered]@{
            mode = $mode; run = $i; pid = $pid_; started_at = (Get-Date).ToString('o')
            exit_code = $exitCode
            summary = $summary; cells = $cellsLine; viewport = $viewport
            mapped_virtual_bytes = $mappedBytes
            core_compare_ms = $compareMs
            digest_rows_ms = Get-Mark $marks 'digest:rows'
            digest_cells_ms = Get-Mark $marks 'digest:cells'
            digest_values_ms = Get-Mark $marks 'digest:values'
            viewport_ms = Get-Mark $marks 'viewport_driver'
            wall_to_hold_ms = $holdBeginMs
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
        Write-Host "$mode run $i done"
        Start-Sleep -Seconds 2
    }
}

# cross-run / cross-mode digest consistency gate
$digests = @($batch | ForEach-Object {
    $rd = ([regex]::Match($_.summary, 'rows_digest=([0-9a-f]+)')).Groups[1].Value
    $cd = ([regex]::Match($_.cells, 'cell_digest=([0-9a-f]+)')).Groups[1].Value
    $vd = ([regex]::Match($_.cells, 'value_digest=([0-9a-f]+)')).Groups[1].Value
    $cold = ([regex]::Match($_.cells, 'columns_digest=([0-9a-f]+)')).Groups[1].Value
    if (-not $rd -or -not $cd -or -not $vd -or -not $cold) { $script:failures += "$($_.mode) run$($_.run): unparsable digest" }
    "$rd/$cd/$vd/$cold"
})
if (($digests | Sort-Object -Unique).Count -ne 1) {
    $failures += "digest mismatch across runs/modes: $($digests -join ' ; ')"
}
if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Host "GATE FAIL: $_" }
    Write-Host "baseline NOT appended"
    exit 1
}
if ($NoBaseline) {
    Write-Host "probe mode: baseline NOT appended (raw under $RunDir)"
    exit 0
}

$history = [ordered]@{
    batch_at = (Get-Date).ToString('o')
    timing_basis = 'compare-mark'   # core_compare_ms = [proto] compare mark; digest/viewport separate
    raw_dir = "target/memory-audit/prototype/batch-$batchId"
    revision = (git -C $Root rev-parse HEAD)
    dirty = ((git -C $Root status --porcelain).Length -gt 0)
    exe_sha256 = (Get-FileHash $Exe -Algorithm SHA256).Hash.ToLower()
    input_left_sha256 = $ActualLeftHash
    input_right_sha256 = $ActualRightHash
    input_expected_left_sha256 = $ExpectedLeftHash
    input_expected_right_sha256 = $ExpectedRightHash
    input_preread = 'both inputs fully read for SHA256 immediately before batch'
    variant = 'fastsplit+rawcompare'
    digests = $digests[0]
    runs = @($batch | ForEach-Object {
        [ordered]@{ mode = $_.mode; run = $_.run; exit_code = $_.exit_code
            stable_private_bytes = $_.stable_private_bytes
            stable_working_set = $_.stable_working_set
            stable_working_set_private = $_.stable_working_set_private
            load_observed_peak_private_bytes = $_.load_observed_peak_private_bytes
            os_working_set_peak = $_.os_working_set_peak
            mapped_virtual_bytes = $_.mapped_virtual_bytes
            core_compare_ms = $_.core_compare_ms
            digest_cells_ms = $_.digest_cells_ms
            digest_values_ms = $_.digest_values_ms
            viewport_ms = $_.viewport_ms
            wall_to_hold_ms = $_.wall_to_hold_ms
            summary = $_.summary; cells = $_.cells; viewport = $_.viewport }
    })
}
$histPath = "$Root\tools\demand_proto_baseline.json"
$existing = @()
if (Test-Path $histPath) {
    $parsed = Get-Content $histPath -Raw | ConvertFrom-Json
    if ($parsed.batches) { $existing = @($parsed.batches) } elseif ($parsed.runs) { $existing = @($parsed) }
}
@{ batches = @($existing + $history) } | ConvertTo-Json -Depth 6 | Set-Content $histPath -Encoding UTF8
Write-Host "wrote $histPath"
exit 0
