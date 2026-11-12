# 日常编译：结束从本仓库 target\debug\ 运行的 Matcha 进程，然后 debug 构建 matcha.exe。
# 不打包、不改版本号。产物：target\debug\matcha.exe。
#
#   powershell -ExecutionPolicy Bypass -File tools\dev-build.ps1
#   可选：-NoKill 不结束进程（exe 被占用时链接会失败）。
param(
    [switch]$NoKill
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$debugDir = Join-Path $repo 'target\debug'

if (-not $NoKill) {
    # 只结束路径在 target\debug\ 下的进程；dist/release/安装版不受影响。
    $running = Get-Process -Name 'matcha', 'matcha-comp' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($debugDir, [StringComparison]::OrdinalIgnoreCase) }
    foreach ($p in $running) {
        Write-Host "Stopping $($p.ProcessName) (pid $($p.Id)): $($p.Path)"
        Stop-Process -Id $p.Id -Force
    }
    foreach ($p in $running) {
        try { $p.WaitForExit(10000) | Out-Null } catch {}
    }
}

Push-Location $repo
try {
    & cargo build -p bcompare
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo build -p bcompare failed (exit $LASTEXITCODE)"
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}
Write-Host "Debug build ready: $(Join-Path $debugDir 'matcha.exe')"
