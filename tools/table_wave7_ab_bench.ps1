# table_wave7_ab_bench.ps1 — table-compare-xlsx 6.1 同轮交替 A/B 基准运行器
#
# 说明（按 wave7 审计约束“先说明”）：本脚本由 wave7 审计新增，仅调用既有二进制与
# 既有/确定性生成的输入；不修改任何源码、不写入 tmp/（文本输入只读复制到 audit/inputs
# 后使用）、不重建任何 release。只有在总控专门派发 6.1 时才执行实际基准运行。
#
# 解析协议（与 crates/bc-gui/src/bench.rs 输出一致，分位数与均值同行、前缀只在行首）：
#   文本线: bench.frames_measured=600 / bench.update_ms.mean=0.752 p50=0.651 p95=1.188 p99=1.347 max=1.892
#   表格线: bench.table.frames_measured=600 / bench.table.update_ms.mean=... p99=...（app.rs 加 table 前缀）
# 必需字段（frames_measured/wall_seconds/left_mb/right_mb/update_ms 五分位/frame_cpu_ms 五分位）
# 缺失、帧数不等于 -ExpectedFrames、子进程非 0 退出或超时，都记录失败并使脚本以非 0 退出，
# 绝不产出“空 summary 也算成功”的结果。
#
# 用法（6.1 派发后）：
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/table_wave7_ab_bench.ps1 `
#       -CandidateExe target/release/matcha.exe -Runs 3
# 注意：GUI 基准会打开真实窗口；运行期间不要操作该机器桌面，勿并发其他基准。

[CmdletBinding()]
param(
    [string]$BaselineExe  = "target/table-wave7/baseline-build/release/matcha.exe",
    [string]$CandidateExe = "target/release/matcha.exe",
    [int]$Runs = 3,
    [int]$ExpectedFrames = 600,
    [int]$TimeoutSec = 600,
    [string]$OutDir = "target/table-wave7/audit",
    [string]$TextSourceLeft  = "tmp/left.txt",
    [string]$TextSourceRight = "tmp/right.txt",
    [string]$TextLeft  = "target/table-wave7/audit/inputs/text-left.txt",
    [string]$TextRight = "target/table-wave7/audit/inputs/text-right.txt",
    [string]$TableLeft  = "target/table-wave7/audit/inputs/table-left.csv",
    [string]$TableRight = "target/table-wave7/audit/inputs/table-right.csv",
    [string]$Generator = "target/release/examples/table_gui_inputs.exe",
    [int]$TableRows = 1000000,
    [int]$TableColumns = 10
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# 函数区（dot-source 时仅加载函数，供 tools/table_wave7_parser_tests.ps1 回归）
# ---------------------------------------------------------------------------

function Get-Sha256([string]$p) { (Get-FileHash -Algorithm SHA256 $p).Hash.ToLowerInvariant() }

# 精确解析一条原生日志；$Prefix 为 "bench"（文本）或 "bench.table"（表格）。
# 缺失必需字段或帧数不符即抛错，返回有序哈希。
function Read-BenchLog([string]$Path, [string]$Prefix, [int]$ExpectedFrames) {
    if (-not (Test-Path $Path)) { throw "bench log missing: $Path" }
    $raw = Get-Content $Path -Raw
    $esc = [regex]::Escape($Prefix)
    $result = [ordered]@{ path = $Path }
    foreach ($field in @("frames_measured", "wall_seconds", "fps_overall")) {
        if ($raw -match "(?m)^$esc\.$field=([\d.]+)\s*$") {
            $result[$field] = [double]$Matches[1]
        } else {
            throw "bench log $Path missing required field $Prefix.$field"
        }
    }
    if ([int]$result.frames_measured -ne $ExpectedFrames) {
        throw "bench log $Path frames_measured=$([int]$result.frames_measured), expected $ExpectedFrames"
    }
    # left_mb/right_mb 同行、index_mb 位于 load 行尾，均不做行首锚定；前缀隔离文本/表格日志。
    foreach ($field in @("left_mb", "right_mb", "index_mb")) {
        if ($raw -match "$esc\.$field=([\d.]+)") {
            $result[$field] = [double]$Matches[1]
        } else {
            throw "bench log $Path missing input size field $Prefix.$field"
        }
    }
    # mean/p50/p95/p99/max 与均值同行、不带二次前缀，必须整行精确匹配。
    foreach ($metric in @("update_ms", "frame_cpu_ms")) {
        $pattern = "(?m)^$esc\.$metric\.mean=([\d.]+)\s+p50=([\d.]+)\s+p95=([\d.]+)\s+p99=([\d.]+)\s+max=([\d.]+)\s*$"
        $m = [regex]::Match($raw, $pattern)
        if (-not $m.Success) {
            throw "bench log $Path missing full percentile line $Prefix.$metric.mean/p50/p95/p99/max"
        }
        foreach ($i in 1..5) {
            $name = @("mean", "p50", "p95", "p99", "max")[$i - 1]
            $result["$metric.$name"] = [double]$m.Groups[$i].Value
        }
    }
    return $result
}

# 子进程：明确超时/退出码/失败记录。超时杀进程并在日志尾部留标记。
function Invoke-BenchProcess([string]$Exe, [string[]]$Arguments, [string]$LogPath, [int]$TimeoutSec) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = (Resolve-Path $Exe).Path
    # Windows PowerShell 5.1（.NET Framework）无 ArgumentList/Kill(tree)，用引号参数串。
    $psi.Arguments = ($Arguments | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }) -join ' '
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $stdout = $p.StandardOutput.ReadToEndAsync()
    $stderr = $p.StandardError.ReadToEndAsync()
    $finished = $p.WaitForExit($TimeoutSec * 1000)
    if (-not $finished) {
        try { $p.Kill() } catch {}
        Set-Content -Path $LogPath -Encoding utf8 -Value "BENCH-TIMEOUT after ${TimeoutSec}s: $Exe $($Arguments -join ' ')"
        throw "bench process timeout (${TimeoutSec}s): $Exe; log $LogPath"
    }
    $p.WaitForExit() # 排空异步输出
    Set-Content -Path $LogPath -Encoding utf8 -Value ($stdout.Result + $stderr.Result)
    if ($p.ExitCode -ne 0) {
        Add-Content -Path $LogPath -Encoding utf8 -Value "`nBENCH-EXIT-CODE=$($p.ExitCode)"
        throw "bench process exit code $($p.ExitCode): $Exe; log $LogPath"
    }
    return $p.ExitCode
}

function Assert-DifferentBinaries([string]$A, [string]$B) {
    $ha = Get-Sha256 $A
    $hb = Get-Sha256 $B
    if ($ha -eq $hb) { throw "baseline and candidate binaries are identical ($ha); A/A run refused" }
    return @($ha, $hb)
}

# 覆盖判定：每侧每线至少 $MinRuns 条成功记录，否则失败（防空 CSV 成功）。
function Assert-RunCoverage($SummaryRows, [int]$MinRuns) {
    foreach ($side in @("A", "B")) {
        foreach ($line in @("text", "table")) {
            $n = @($SummaryRows | Where-Object { $_.side -eq $side -and $_.line -eq $line -and $_.ok }).Count
            if ($n -lt $MinRuns) { throw "insufficient successful runs for side=$side line=$line : $n < $MinRuns" }
        }
    }
}

function Get-Median([double[]]$values) {
    if ($values.Count -eq 0) { throw "median of empty set" }
    $sorted = $values | Sort-Object
    # 注意 PowerShell [int] 转换是四舍五入，必须显式 Floor。
    $mid = [int][Math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 1) { return $sorted[$mid] }
    return ($sorted[$mid - 1] + $sorted[$mid]) / 2.0
}

# 表格输入规模核对：数据行 >= TableRows 且首行列数 >= TableColumns。
function Assert-TableInputShape([string]$Path, [int]$MinRows, [int]$MinColumns) {
    $sr = [System.IO.File]::OpenText($Path)
    try {
        $header = $sr.ReadLine()
        if ($null -eq $header) { throw "empty table input: $Path" }
        $cols = $header.Split(",").Count
        if ($cols -lt $MinColumns) { throw "table input $Path has $cols columns, expected >= $MinColumns" }
        $rows = 0
        while ($null -ne $sr.ReadLine()) { $rows++ }
        if ($rows -lt $MinRows) { throw "table input $Path has $rows data rows, expected >= $MinRows" }
        return [ordered]@{ data_rows = $rows; columns = $cols }
    } finally { $sr.Close() }
}

# ---------------------------------------------------------------------------
# 主流程（dot-source 加载时跳过）
# ---------------------------------------------------------------------------
if ($MyInvocation.InvocationName -ne ".") {
    $root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    Set-Location $root
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path $TableLeft) | Out-Null
    if ($Runs -lt 3) { throw "Runs must be >= 3 for a same-round A/B verdict" }

    $failures = New-Object System.Collections.Generic.List[string]
    $summary = New-Object System.Collections.Generic.List[object]
    $runOrder = New-Object System.Collections.Generic.List[string]

    try {
        # --- 1. 二进制证据：A/B 哈希必须不同 -------------------------------
        foreach ($exe in @($BaselineExe, $CandidateExe)) {
            if (-not (Test-Path $exe)) { throw "missing executable: $exe" }
        }
        $hashes = Assert-DifferentBinaries $BaselineExe $CandidateExe

        # --- 2. 输入准备：文本只读复制、表格确定性生成、规模核对 -----------
        foreach ($pair in @(@($TextSourceLeft, $TextLeft), @($TextSourceRight, $TextRight))) {
            if (-not (Test-Path $pair[1])) {
                Copy-Item $pair[0] $pair[1]
                Write-Host "copied $($pair[0]) -> $($pair[1]) (read-only source)"
            }
            if ((Get-Sha256 $pair[0]) -ne (Get-Sha256 $pair[1])) { throw "text input copy diverged: $($pair[1])" }
        }
        if (-not (Test-Path $TableLeft) -or -not (Test-Path $TableRight)) {
            if (-not (Test-Path $Generator)) { throw "missing input generator: $Generator" }
            $genLog = Join-Path $OutDir "inputs-gen.log"
            Invoke-BenchProcess $Generator @([string]$TableRows) $genLog 300 | Out-Null
            $out = Get-Content $genLog
            $left  = ($out | Where-Object { $_ -like "left=*"  }).Substring(5)
            $right = ($out | Where-Object { $_ -like "right=*" }).Substring(6)
            $cleanup = ($out | Where-Object { $_ -like "cleanup=*" }).Substring(8)
            Copy-Item $left  $TableLeft
            Copy-Item $right $TableRight
            Remove-Item -Recurse -Force $cleanup
            Write-Host "generated table inputs -> $(Split-Path $TableLeft)"
        }
        $shapeL = Assert-TableInputShape $TableLeft $TableRows $TableColumns
        $shapeR = Assert-TableInputShape $TableRight $TableRows $TableColumns

        $manifest = [ordered]@{
            captured_utc  = (Get-Date).ToUniversalTime().ToString("o")
            git_head      = (git rev-parse HEAD).Trim()
            baseline_exe  = @{ path = $BaselineExe;  sha256 = $hashes[0] }
            candidate_exe = @{ path = $CandidateExe; sha256 = $hashes[1] }
            runs_per_side = $Runs
            frames        = $ExpectedFrames
            timeout_sec   = $TimeoutSec
            table_shape   = @{ left = $shapeL; right = $shapeR }
            inputs        = [ordered]@{
                text_left   = @{ path = $TextLeft;  sha256 = Get-Sha256 $TextLeft }
                text_right  = @{ path = $TextRight; sha256 = Get-Sha256 $TextRight }
                table_left  = @{ path = $TableLeft;  sha256 = Get-Sha256 $TableLeft }
                table_right = @{ path = $TableRight;  sha256 = Get-Sha256 $TableRight }
            }
        }
        ($manifest | ConvertTo-Json -Depth 6) | Set-Content -Encoding utf8 (Join-Path $OutDir "bench-ab-manifest.json")

        # --- 3. 同轮交替 A/B ---------------------------------------------
        for ($i = 1; $i -le $Runs; $i++) {
            $order = if ($i % 2 -eq 0) { @("B", "A") } else { @("A", "B") }
            foreach ($side in $order) {
                $exe = if ($side -eq "A") { $BaselineExe } else { $CandidateExe }
                foreach ($line in @("text", "table")) {
                    $log = Join-Path $OutDir "bench-$line-$side-run$i.log"
                    $row = [ordered]@{ side = $side; run = $i; line = $line; log = $log; ok = $false }
                    try {
                        if ($line -eq "text") {
                            Invoke-BenchProcess $exe @($TextLeft, $TextRight, "--bench=$ExpectedFrames") $log $TimeoutSec | Out-Null
                            $parsed = Read-BenchLog $log "bench" $ExpectedFrames
                        } else {
                            Invoke-BenchProcess $exe @($TableLeft, $TableRight, "--bench-table") $log $TimeoutSec | Out-Null
                            $parsed = Read-BenchLog $log "bench.table" $ExpectedFrames
                        }
                        foreach ($k in $parsed.Keys) { if ($k -ne "path") { $row[$k] = $parsed[$k] } }
                        $row.ok = $true
                    } catch {
                        $row.error = $_.Exception.Message
                        $failures.Add("run $i side $side line $line : $($_.Exception.Message)")
                    }
                    $summary.Add([pscustomobject]$row)
                }
                $runOrder.Add("run${i}:$($order -join ',')")
                Write-Host "run $i side $side done"
            }
        }

        # --- 4. 汇总与 median（可审计） -----------------------------------
        $summary | Export-Csv -NoTypeInformation -Encoding utf8 (Join-Path $OutDir "bench-ab-summary.csv")
        Assert-RunCoverage $summary $Runs
        $medians = [ordered]@{}
        foreach ($side in @("A", "B")) {
            foreach ($line in @("text", "table")) {
                $rows = @($summary | Where-Object { $_.side -eq $side -and $_.line -eq $line -and $_.ok })
                $medians["$side-$line"] = [ordered]@{
                    update_p99      = Get-Median ($rows | ForEach-Object { [double]$_.'update_ms.p99' })
                    update_mean     = Get-Median ($rows | ForEach-Object { [double]$_.'update_ms.mean' })
                    frame_cpu_p99   = Get-Median ($rows | ForEach-Object { [double]$_.'frame_cpu_ms.p99' })
                    update_p99_all  = @($rows | ForEach-Object { [double]$_.'update_ms.p99' })
                }
            }
        }
        ([ordered]@{ run_order = $runOrder; medians = $medians } | ConvertTo-Json -Depth 5) |
            Set-Content -Encoding utf8 (Join-Path $OutDir "bench-ab-results.json")

        # --- 5. 运行后输入完整性复验 ---------------------------------------
        foreach ($k in @("text_left", "text_right", "table_left", "table_right")) {
            $after = Get-Sha256 $manifest.inputs[$k].path
            if ($after -ne $manifest.inputs[$k].sha256) { throw "input mutated during bench: $k" }
        }

        if ($failures.Count -gt 0) {
            ($failures | ConvertTo-Json) | Set-Content -Encoding utf8 (Join-Path $OutDir "bench-ab-failures.json")
            throw "bench completed with $($failures.Count) recorded failure(s); see bench-ab-failures.json"
        }
        Write-Host "A/B bench complete. Median table update p99 A=$($medians['A-table'].update_p99) B=$($medians['B-table'].update_p99)"
    } catch {
        if ($failures.Count -gt 0) {
            ($failures | ConvertTo-Json) | Set-Content -Encoding utf8 (Join-Path $OutDir "bench-ab-failures.json")
        }
        if ($summary.Count -gt 0) {
            $summary | Export-Csv -NoTypeInformation -Encoding utf8 (Join-Path $OutDir "bench-ab-summary.csv")
        }
        Write-Error $_
        exit 1
    }
}
