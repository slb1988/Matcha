# 无控制台启动的验收（任务 6.1）：写到标准输出的报告应弹出 “Matcha” 消息框并以 100 结束；
# 写文件的报告正常完成、无消息框。
#
# 为什么经 matcha-comp.exe 启动：Explorer 双击时 matcha.exe 的父进程没有控制台。直接
# Start-Process matcha.exe 时父进程是本 PowerShell——在自动化环境里它可能带一个隐藏控制台，
# AttachConsole 会成功、报告写进看不见的控制台，不代表真实场景。matcha-comp.exe 是 GUI 子系统，
# 经 ShellExecute 启动时既没有控制台也没有 std 句柄，并把这一状态原样传给它启动的 matcha.exe。
# 不用 Start-Process -Wait：要在 matcha.exe 退出前拿到其句柄，退出码才可读。
param([string]$Dist, [string]$Log)
$launcher = Join-Path $Dist 'matcha-comp.exe'
$work = Join-Path $env:TEMP 'matcha-6.1-noconsole'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
[IO.File]::WriteAllText("$work\a.txt", "x`n")
[IO.File]::WriteAllText("$work\b.txt", "y`n")
$lines = @()

function Launch([string[]]$arguments) {
    $known = @(Get-Process matcha -ErrorAction SilentlyContinue | ForEach-Object Id)
    $null = Start-Process -FilePath $launcher -ArgumentList $arguments -WorkingDirectory $work -PassThru
    for ($i = 0; $i -lt 100; $i++) {
        $p = Get-Process matcha -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -like "$Dist*" -and $known -notcontains $_.Id } | Select-Object -First 1
        if ($p) { $null = $p.Handle; return $p }
        Start-Sleep -Milliseconds 20
    }
}

$p = Launch @('/report=patch', 'a.txt', 'b.txt')
$title = ''
for ($i = 0; $p -and $i -lt 50 -and -not $p.HasExited -and -not $title; $i++) {
    Start-Sleep -Milliseconds 100
    $p.Refresh()
    $title = $p.MainWindowTitle
}
$lines += "stdout report: message box title = '$title'"
if ($p -and -not $p.HasExited) { $null = $p.CloseMainWindow() }
if ($p) { $null = $p.WaitForExit(5000); $lines += "stdout report: exit after closing the box = $($p.ExitCode)" }
else { $lines += 'stdout report: matcha.exe was not observed' }

$p = Launch @('/report=patch', '/reportfile=r.patch', 'a.txt', 'b.txt')
if ($p) {
    $exited = $p.WaitForExit(10000)
    $p.Refresh()
    $lines += "file report: exited by itself = $exited; window title = '$($p.MainWindowTitle)'; exit = $($p.ExitCode)"
} else { $lines += 'file report: matcha.exe finished before it was observed' }
$lines += "file report: file = $(Test-Path "$work\r.patch")"
if (Test-Path "$work\r.patch") { $lines += (Get-Content "$work\r.patch" | Select-Object -Skip 1) -join ' | ' }
$lines | Set-Content -Encoding utf8 $Log
