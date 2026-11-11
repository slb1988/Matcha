# Matcha 确定性客户区快照；所有路径相对仓库根，与调用者当前目录无关。
[CmdletBinding()]
param(
    [ValidateSet('icon-catalog','chrome-demo','text-basic','folder-basic','text-diffs-filter','text-cursor-on-diff','folder-expanded','folder-diffs-filter','text-long-scrolled','home','text-menu-edit','text-find-bar')]
    [string]$Scene = 'text-basic',
    [switch]$All,
    [switch]$Verify,
    [string]$WindowSize = '1384x961'
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $root
try {
    # 语料 mtime 决定文件夹视图的 Modified 列文本；先复位再构建。
    & (Join-Path $root 'tools/visual/set-corpus-mtime.ps1') | Out-Null
    & cargo build --release -p bcompare
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed: $LASTEXITCODE" }
    $exe = Join-Path $root 'target/release/matcha.exe'
    $scenes = if ($All) { @('icon-catalog','chrome-demo','text-basic','folder-basic','text-diffs-filter','text-cursor-on-diff','folder-expanded','folder-diffs-filter','text-long-scrolled','home','text-menu-edit','text-find-bar') } else { @($Scene) }
    $outDir = Join-Path $root 'target/visual/matcha'
    New-Item -ItemType Directory -Force $outDir | Out-Null
    $results = @()
    foreach ($name in $scenes) {
        $path = Join-Path $outDir "$name.png"
        & $exe "--snapshot=$name" "--snapshot-out=$path" "--window-size=$WindowSize"
        if ($LASTEXITCODE -ne 0) { throw "$name snapshot failed: $LASTEXITCODE" }
        $bytes = [IO.File]::ReadAllBytes($path)
        if ([Convert]::ToHexString($bytes[0..7]) -ne '89504E470D0A1A0A') { throw "$path is not PNG" }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
        $repeatHash = $null
        if ($Verify) {
            $repeat = Join-Path $outDir "$name.repeat.png"
            & $exe "--snapshot=$name" "--snapshot-out=$repeat" "--window-size=$WindowSize"
            if ($LASTEXITCODE -ne 0) { throw "$name repeat failed: $LASTEXITCODE" }
            $repeatHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $repeat).Hash
            if ($hash -ne $repeatHash) { throw "$name not deterministic: $hash != $repeatHash" }
        }
        $results += [pscustomobject]@{ Scene=$name; Path=$path; SHA256=$hash; RepeatSHA256=$repeatHash; Verified=[bool]$Verify }
        Write-Output "$name SHA256=$hash verified=$([bool]$Verify)"
    }
    $results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $outDir 'manifest.json')
} finally { Pop-Location }
