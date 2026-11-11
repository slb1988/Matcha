# 真实 PowerShell 控制台窗口中的验收（任务 6.1）。用法（在新控制台窗口里运行，不能经管道）：
#   Start-Process -Wait powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',
#     '<本文件>','-Dist','<target\dist\matcha-…>','-Log','<日志>'
# 末尾转储本窗口的控制台缓冲区，证明报告确实显示在控制台里（而非仅写入管道）。
param([string]$Dist, [string]$Log)
$ErrorActionPreference = 'Continue'
$env:PATH = "$Dist;$env:PATH"
$work = Join-Path $env:TEMP "matcha-6.1-ps"
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
Set-Location $work
[IO.File]::WriteAllText("$work\a.txt", "one`ntwo`nthree`n")
[IO.File]::WriteAllText("$work\b.txt", "one`n2`nthree`nfour`n")
[IO.File]::WriteAllText("$work\s.txt", "text-report layout:patch options:patch-unified output-to:- `"%1`" `"%2`"`r`n")
[IO.File]::WriteAllText("$work\bad.txt", "text-reprot layout:patch output-to:- a.txt b.txt`r`n")
[IO.File]::WriteAllText("$work\miss.txt", "text-report layout:patch output-to:- nope.txt b.txt`r`n")

$lines = @()
$lines += "resolved: " + (Get-Command matcha).Source
Write-Host "=== /report=patch-unified (console) ==="
matcha /report=patch-unified a.txt b.txt
$lines += "report to console exit: $LASTEXITCODE"
$plus = @(matcha /report=patch-unified a.txt b.txt | Select-String '^\+[^+]')
$lines += "pipe Select-String '^+': " + (($plus | ForEach-Object { $_.Line }) -join ' | ')
Write-Host "=== @s.txt a.txt b.txt (console) ==="
matcha '@s.txt' a.txt b.txt
$lines += "script exit: $LASTEXITCODE"
matcha '@bad.txt' 2> bad.err
$lines += "syntax exit: $LASTEXITCODE; " + ((Get-Content bad.err | Select-Object -First 1) -replace '^matcha : ', '')
matcha '@miss.txt' 2> miss.err
$lines += "missing-file exit: $LASTEXITCODE; " + ((Get-Content miss.err | Select-Object -First 1) -replace '^matcha : ', '')
matcha '@nope.txt' 2> nope.err
$lines += "missing-script exit: $LASTEXITCODE"
matcha /report=side-by-side a.txt b.txt 2> layout.err
$lines += "unsupported layout exit: $LASTEXITCODE"

$raw = $Host.UI.RawUI
$rect = New-Object System.Management.Automation.Host.Rectangle 0, 0, ($raw.BufferSize.Width - 1), $raw.CursorPosition.Y
$cells = $raw.GetBufferContents($rect)
$screen = @()
for ($y = 0; $y -le $raw.CursorPosition.Y; $y++) {
    $row = -join (0..($raw.BufferSize.Width - 1) | ForEach-Object { $cells[$y, $_].Character })
    if ($row.Trim()) { $screen += $row.TrimEnd() }
}
($lines + '---- console buffer ----' + $screen) | Set-Content -Encoding utf8 $Log
