[CmdletBinding()]
param([string]$Root = (Join-Path $PSScriptRoot 'corpus'))
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$Root = (Resolve-Path -LiteralPath $Root).Path
$utf8Bom = [Text.UTF8Encoding]::new($true)
$utf8 = [Text.UTF8Encoding]::new($false)

function Write-Lines([string]$Name, [string[]]$Lines) {
    [IO.File]::WriteAllText((Join-Path $Root $Name), (($Lines -join "`r`n") + "`r`n"), $utf8Bom)
}

Write-Lines 'text-basic-left.txt' @('alpha', 'beta', 'gamma', 'delta', 'shared')
Write-Lines 'text-basic-right.txt' @('alpha', 'BETA', 'gamma', 'delta', 'shared', 'inserted')
$longLeft = foreach ($line in 1..360) {
    if ($line -eq 80) { 'changed line 080' }
    elseif ($line -eq 180) { 'removed marker' }
    else { 'line {0:D3}: stable content for scrolling and thumbnail measurement' -f $line }
}
$longRight = foreach ($line in 1..365) {
    if ($line -eq 80) { 'changed line 080 (right)' }
    elseif ($line -eq 200) { 'inserted marker' }
    else { 'line {0:D3}: stable content for scrolling and thumbnail measurement' -f $line }
}
Write-Lines 'text-long-left.txt' $longLeft
Write-Lines 'text-long-right.txt' $longRight
# 双引号确保制表符是真正的 U+0009，而不是旧版的字面量反引号+t。
Write-Lines 'text-cjk-tabs-left.txt' @("项目`t值`t备注  ", "第一行`t苹果`t中文", "第二行`t香蕉`t保留", "emoji`t✓`t尾部空格   ", "路径`tC:\tmp\左侧")
Write-Lines 'text-cjk-tabs-right.txt' @("项目`t值`t备注", "第一行`t苹果`t中文", "第二行`t橙子`t修改", "第三行`t梨`t新增", "emoji`t✓`t尾部空格")
[IO.File]::WriteAllText((Join-Path $Root 'text-crlf-left.txt'), "one`r`ntwo`r`nthree`r`n", $utf8)
[IO.File]::WriteAllText((Join-Path $Root 'text-crlf-right.txt'), "one`ntwo`nthree`n", $utf8)
Write-Lines 'table-left.csv' @('name,qty,status', 'alpha,2,same', 'beta,3,changed', 'gamma,4,removed')
Write-Lines 'table-right.csv' @('name,qty,status', 'alpha,2,same', 'beta,5,changed', 'delta,7,inserted')

foreach ($side in @('left', 'right')) {
    foreach ($directory in @('same/deep', 'different', "$side-only/nested", 'empty')) {
        New-Item -ItemType Directory -Force -Path (Join-Path $Root "folder-$side/$directory") | Out-Null
    }
    Write-Lines "folder-$side/same/deep/same.txt" @('same')
    Write-Lines "folder-$side/different/value.txt" @("$side value")
    Write-Lines "folder-$side/$side-only/nested/orphan.txt" @("$side orphan")
    Write-Lines "folder-$side/version.txt" @("$side version")
}
Write-Lines 'folder-left/older.txt' @('older')
Write-Lines 'folder-right/newer.txt' @('newer')
& (Join-Path $PSScriptRoot 'set-corpus-mtime.ps1') -Root $Root
Write-Output "Corpus generated: $Root"
