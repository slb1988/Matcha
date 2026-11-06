# table_wave7_parser_tests.ps1 — table_wave7_ab_bench.ps1 解析/守卫的离线回归
#
# 只解析既有日志与构造样例，不运行任何原生基准、不重建 release。
# 正例：target/table-wave6/bench-{text,table}.log 真实日志。
# 负例：缺字段 / 帧数不足 / 子进程非 0 退出 / 子进程超时 / 轮次覆盖不足 / A/A 同哈希。
[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root
. (Join-Path $PSScriptRoot "table_wave7_ab_bench.ps1")   # dot-source：仅加载函数

$passed = 0; $failed = 0; $cases = New-Object System.Collections.Generic.List[string]
function Case([string]$name, [scriptblock]$body, [bool]$expectThrow = $false, [string]$needle = "") {
    try {
        $out = & $body
        if ($expectThrow) {
            $script:failed++; $cases.Add("FAIL $name : expected throw, got success")
        } else { $script:passed++; $cases.Add("PASS $name") }
    } catch {
        if ($expectThrow -and ($needle -eq "" -or $_.Exception.Message -like "*$needle*")) {
            $script:passed++; $cases.Add("PASS $name (threw: $($_.Exception.Message))")
        } else {
            $script:failed++; $cases.Add("FAIL $name : $($_.Exception.Message)")
        }
    }
}

# --- 正例：真实 wave6 日志 -------------------------------------------------
Case "parse text log p99=1.347" {
    $r = Read-BenchLog "target/table-wave6/bench-text.log" "bench" 600
    if ([double]$r.'update_ms.p99' -ne 1.347) { throw "text update p99 $($r.'update_ms.p99') != 1.347" }
    if ([double]$r.'update_ms.mean' -ne 0.752) { throw "text update mean mismatch" }
    if ([double]$r.'frame_cpu_ms.p99' -ne 2.200) { throw "text frame cpu p99 mismatch" }
    if ([double]$r.left_mb -ne 110.0) { throw "text left_mb mismatch" }
}
Case "parse table log p99=1.314" {
    $r = Read-BenchLog "target/table-wave6/bench-table.log" "bench.table" 600
    if ([double]$r.'update_ms.p99' -ne 1.314) { throw "table update p99 $($r.'update_ms.p99') != 1.314" }
    if ([double]$r.'update_ms.max' -ne 1.715) { throw "table update max mismatch" }
    if ([double]$r.'frame_cpu_ms.p99' -ne 4.361) { throw "table frame cpu p99 mismatch" }
}

# --- 负例：缺字段 / 帧数不足 ------------------------------------------------
$tmpLog = Join-Path $env:TEMP "table-wave7-parser-test-$PID.log"
Case "missing p99 rejected" -expectThrow $true -needle "percentile" {
    @"
bench.frames_measured=600
bench.wall_seconds=11.44
bench.fps_overall=55.1
bench.left_mb=110.0 bench.right_mb=110.0
bench.index_mb=101.7
bench.update_ms.mean=0.752 p50=0.651 p95=1.188 max=1.892
bench.frame_cpu_ms.mean=1.384 p50=1.337 p95=1.857 p99=2.200 max=2.709
"@ | Set-Content -Encoding utf8 $tmpLog
    Read-BenchLog $tmpLog "bench" 600
}
Case "missing input size rejected" -expectThrow $true -needle "left_mb" {
    @"
bench.frames_measured=600
bench.wall_seconds=11.44
bench.fps_overall=55.1
bench.index_mb=101.7
bench.update_ms.mean=0.752 p50=0.651 p95=1.188 p99=1.347 max=1.892
bench.frame_cpu_ms.mean=1.384 p50=1.337 p95=1.857 p99=2.200 max=2.709
"@ | Set-Content -Encoding utf8 $tmpLog
    Read-BenchLog $tmpLog "bench" 600
}
Case "insufficient frames rejected" -expectThrow $true -needle "frames_measured" {
    @"
bench.frames_measured=300
bench.wall_seconds=5.0
bench.fps_overall=55.1
bench.left_mb=110.0 bench.right_mb=110.0
bench.index_mb=101.7
bench.update_ms.mean=0.752 p50=0.651 p95=1.188 p99=1.347 max=1.892
bench.frame_cpu_ms.mean=1.384 p50=1.337 p95=1.857 p99=2.200 max=2.709
"@ | Set-Content -Encoding utf8 $tmpLog
    Read-BenchLog $tmpLog "bench" 600
}

# --- 负例：子进程退出码 / 超时 ----------------------------------------------
Case "nonzero exit recorded" -expectThrow $true -needle "exit code 3" {
    Invoke-BenchProcess "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        @("-NoProfile", "-Command", "exit 3") $tmpLog 30
}
Case "timeout recorded" -expectThrow $true -needle "timeout" {
    Invoke-BenchProcess "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        @("-NoProfile", "-Command", "Start-Sleep 30") $tmpLog 2
}

# --- 负例：轮次覆盖不足 / A 与 B 同哈希 --------------------------------------
Case "fewer than 3 runs rejected" -expectThrow $true -needle "insufficient successful runs" {
    $rows = @(
        [pscustomobject]@{ side = "A"; line = "text"; ok = $true },
        [pscustomobject]@{ side = "A"; line = "text"; ok = $true }
    )
    Assert-RunCoverage $rows 3
}
Case "identical A/A binary rejected" -expectThrow $true -needle "identical" {
    Assert-DifferentBinaries "target/table-wave7/baseline/matcha.exe" "target/table-wave7/baseline/matcha.exe"
}

# --- 工具函数抽样 ------------------------------------------------------------
Case "median odd/even" {
    if ((Get-Median @(3.0, 1.0, 2.0)) -ne 2.0) { throw "odd median wrong" }
    if ((Get-Median @(4.0, 1.0, 2.0, 3.0)) -ne 2.5) { throw "even median wrong" }
}
Case "table input shape check" {
    $r = Assert-TableInputShape "target/table-wave7/audit/inputs/table-left.csv" 1000000 10
    if ($r.data_rows -ne 1000000 -or $r.columns -ne 10) { throw "shape mismatch: $($r | ConvertTo-Json -Compress)" }
}

Remove-Item -Force $tmpLog -ErrorAction SilentlyContinue
$cases | ForEach-Object { Write-Host $_ }
Write-Host "parser tests: $passed passed / $failed failed"
if ($failed -gt 0) { exit 1 }
