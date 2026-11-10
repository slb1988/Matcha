[CmdletBinding()]
param(
    [string]$Root = (Join-Path $PSScriptRoot 'corpus'),
    [datetime]$Timestamp = [datetime]'2026-01-02 03:04:05'
)
$ErrorActionPreference = 'Stop'
# 使用本地时刻，截图为 1/2/2026 3:04:05 AM；不依赖运行日期。
foreach ($item in Get-ChildItem -LiteralPath $Root -Recurse) {
    $item.LastWriteTime = $Timestamp
    $item.CreationTime = $Timestamp
    $item.LastAccessTime = $Timestamp
}
# 同名 version.txt 给出真正的较新/较旧配对；其余 mtime 完全相同。
$older = Join-Path $Root 'folder-left/version.txt'
if (Test-Path -LiteralPath $older) {
    (Get-Item -LiteralPath $older).LastWriteTime = $Timestamp.AddDays(-1)
}
Write-Output "Corpus mtime: $($Timestamp.ToString('yyyy-MM-dd HH:mm:ss')); version-left: -1 day"
