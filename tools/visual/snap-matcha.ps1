# Matcha 确定性客户区快照；所有路径相对仓库根，与调用者当前目录无关。
[CmdletBinding()]
param(
    # Any scene in the registry (`matcha /?` prints the list); -All runs every one.
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
    # The scene registry is the single source: `matcha /?` ends with "scenes: a, b, ...".
    $line = (& $exe '/?' | Select-String -Pattern '^scenes: ' | Select-Object -First 1).Line
    if (-not $line) { throw 'matcha /? printed no scene list' }
    $registry = $line.Substring('scenes: '.Length).Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ }
    if (-not $All -and $registry -notcontains $Scene) { throw "unknown scene '$Scene'; known: $($registry -join ', ')" }
    $scenes = if ($All) { $registry } else { @($Scene) }
    $outDir = Join-Path $root 'target/visual/matcha'
    New-Item -ItemType Directory -Force $outDir | Out-Null
    $results = @()
    foreach ($name in $scenes) {
        $path = Join-Path $outDir "$name.png"
        # matcha.exe 是 GUI 子系统：管道输出才让 PowerShell 等它结束并设置 $LASTEXITCODE。
        & $exe "--snapshot=$name" "--snapshot-out=$path" "--window-size=$WindowSize" | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "$name snapshot failed: $LASTEXITCODE" }
        $bytes = [IO.File]::ReadAllBytes($path)
        if (([BitConverter]::ToString($bytes[0..7]) -replace '-', '') -ne '89504E470D0A1A0A') { throw "$path is not PNG" }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
        $repeatHash = $null
        if ($Verify) {
            $repeat = Join-Path $outDir "$name.repeat.png"
            & $exe "--snapshot=$name" "--snapshot-out=$repeat" "--window-size=$WindowSize" | Out-Host
            if ($LASTEXITCODE -ne 0) { throw "$name repeat failed: $LASTEXITCODE" }
            $repeatHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $repeat).Hash
            if ($hash -ne $repeatHash) { throw "$name not deterministic: $hash != $repeatHash" }
        }
        $results += [pscustomobject]@{ Scene=$name; Path=$path; SHA256=$hash; RepeatSHA256=$repeatHash; Verified=[bool]$Verify }
        Write-Output "$name SHA256=$hash verified=$([bool]$Verify)"
    }
    $results | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $outDir 'manifest.json')
} finally { Pop-Location }
