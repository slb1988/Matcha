$ErrorActionPreference = 'Stop'
$Root = Join-Path $PSScriptRoot 'corpus'
New-Item -ItemType Directory -Force $Root | Out-Null
$basicL = @('alpha','beta','gamma','delta','shared')
$basicR = @('alpha','BETA','gamma','delta','shared','inserted')
Set-Content (Join-Path $Root 'text-basic-left.txt') ($basicL -join "`r`n") -Encoding UTF8
Set-Content (Join-Path $Root 'text-basic-right.txt') ($basicR -join "`r`n") -Encoding UTF8
$longL = 1..360 | ForEach-Object { if ($_ -eq 80) {'changed line 080'} elseif ($_ -eq 180) {'removed marker'} else { ('line {0:D3}: stable content for scrolling and thumbnail measurement' -f $_) } }
$longR = 1..365 | ForEach-Object { if ($_ -eq 80) {'changed line 080 (right)'} elseif ($_ -eq 200) {'inserted marker'} else { ('line {0:D3}: stable content for scrolling and thumbnail measurement' -f $_) } }
Set-Content (Join-Path $Root 'text-long-left.txt') ($longL -join "`r`n") -Encoding UTF8
Set-Content (Join-Path $Root 'text-long-right.txt') ($longR -join "`r`n") -Encoding UTF8
$cjkL = @('项目`t值`t备注  ','第一行`t苹果`t中文','第二行`t香蕉`t保留','emoji`t✓`t尾部空格   ','路径`tC:\tmp\左侧')
$cjkR = @('项目`t值`t备注','第一行`t苹果`t中文','第二行`t橙子`t修改','第三行`t梨`t新增','emoji`t✓`t尾部空格')
Set-Content (Join-Path $Root 'text-cjk-tabs-left.txt') ($cjkL -join "`r`n") -Encoding UTF8
Set-Content (Join-Path $Root 'text-cjk-tabs-right.txt') ($cjkR -join "`r`n") -Encoding UTF8
[IO.File]::WriteAllText((Join-Path $Root 'text-crlf-left.txt'),"one`r`ntwo`r`nthree`r`n",(New-Object Text.UTF8Encoding($false)))
[IO.File]::WriteAllText((Join-Path $Root 'text-crlf-right.txt'),"one`ntwo`nthree`n",(New-Object Text.UTF8Encoding($false)))
Set-Content (Join-Path $Root 'table-left.csv') "name,qty,status`nalpha,2,same`nbeta,3,changed`ngamma,4,removed`n" -Encoding UTF8
Set-Content (Join-Path $Root 'table-right.csv') "name,qty,status`nalpha,2,same`nbeta,5,changed`ndelta,7,inserted`n" -Encoding UTF8
$left = Join-Path $Root 'folder-left'; $right = Join-Path $Root 'folder-right'
Remove-Item $left,$right -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$left\same\deep", "$left\different", "$left\left-only\nested", "$left\empty", "$right\same\deep", "$right\different", "$right\right-only\nested", "$right\empty" | Out-Null
Set-Content "$left\same\deep\same.txt" 'same' -Encoding UTF8; Set-Content "$right\same\deep\same.txt" 'same' -Encoding UTF8
Set-Content "$left\different\value.txt" 'left value' -Encoding UTF8; Set-Content "$right\different\value.txt" 'right value' -Encoding UTF8
Set-Content "$left\left-only\nested\orphan.txt" 'left orphan' -Encoding UTF8; Set-Content "$right\right-only\nested\orphan.txt" 'right orphan' -Encoding UTF8
Set-Content "$left\older.txt" 'older' -Encoding UTF8; Set-Content "$right\newer.txt" 'newer' -Encoding UTF8
& (Join-Path $PSScriptRoot 'set-corpus-mtime.ps1') -Root $Root
