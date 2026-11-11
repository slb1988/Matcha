#requires -Version 5.1
<#
.SYNOPSIS
  发布 Matcha dist 产物到 GitHub Releases，成功后把 config.json 里的补丁版本号 +1。

.DESCRIPTION
  流程：读 config.json -> 校验产物 / 工具 / 仓库状态 -> 打 tag 并推送 -> gh release create
  上传 zip + 原始文件 -> 升级 config.json 版本号，并同步根 Cargo.toml 的
  [workspace.package] version（保证下次 cargo xtask dist 目录名一致）。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File publish.ps1
  powershell -ExecutionPolicy Bypass -File publish.ps1 -Notes "修复若干问题" -Prerelease
  powershell -ExecutionPolicy Bypass -File publish.ps1 -DryRun
#>
[CmdletBinding()]
param(
  [string]$Notes,
  [switch]$GenerateNotes,
  [switch]$Draft,
  [switch]$Prerelease,
  [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Fail([string]$msg, [string]$hint = '') {
  Write-Host "ERROR: $msg" -ForegroundColor Red
  if ($hint) { Write-Host "HINT:  $hint" -ForegroundColor Yellow }
  exit 1
}

function Test-Cmd([string]$name) {
  if (-not (Get-Command $name -ErrorAction SilentlyContinue)) { Fail "未找到命令：$name" }
}

# 从 ConvertFrom-Json 对象上安全取值的辅助函数（StrictMode 下访问缺失属性会抛异常）
function Cfg([string]$name, $default) {
  $p = $config.PSObject.Properties[$name]
  if ($null -ne $p -and $null -ne $p.Value) { return $p.Value }
  return $default
}

# 调用外部命令：PS5.1 下重定向 stderr 会产生 ErrorRecord，
# 与 $ErrorActionPreference='Stop' 叠加会把预期内的失败变成终止错误，这里临时降级。
function Invoke-Native([scriptblock]$Body) {
  $prev = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try { & $Body 2>&1 } finally { $ErrorActionPreference = $prev }
  # 调用方读取 $LASTEXITCODE 判断成败
}

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$skillDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $skillDir '..\..\..')).Path
$configPath = Join-Path $skillDir 'config.json'

# ---- 1. 读取配置 ----
if (-not (Test-Path $configPath)) { Fail "缺少配置文件：$configPath" }
$config = Get-Content $configPath -Raw -Encoding UTF8 | ConvertFrom-Json

$version = [string](Cfg 'version' '')
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "config.json 里的 version 格式非法：'$version'（应为 x.y.z）" }

$repo = [string](Cfg 'repo' '')
if (-not $repo) {
  $url = Invoke-Native { git -C $repoRoot remote get-url origin }
  if ($LASTEXITCODE -eq 0 -and "$url" -match 'github\.com[:/]([^/]+/[^/]+?)(\.git)?$') { $repo = $Matches[1] }
  else { Fail "config.json 缺少 repo，且无法从 origin 解析 GitHub 仓库" }
}
$tagPrefix = [string](Cfg 'tagPrefix' 'v')
$remote = [string](Cfg 'remote' 'origin')
$platform = [string](Cfg 'platform' 'windows-x86_64')
$distRoot = [string](Cfg 'distRoot' 'target/dist')
$includeRaw = [bool](Cfg 'includeRawFiles' $true)
$syncCargo = [bool](Cfg 'syncCargoVersion' $true)

$tag = "$tagPrefix$version"
$distName = "matcha-$version-$platform"
$distDir = Join-Path $repoRoot (Join-Path $distRoot $distName)
$zipPath = "$distDir.zip"
$cargoPath = Join-Path $repoRoot 'Cargo.toml'

Write-Host "== 发布配置 =="
Write-Host "  repo    : $repo"
Write-Host "  version : $version  (tag: $tag)"
Write-Host "  dist    : $distDir"
Write-Host "  zip     : $zipPath"

# ---- 2. 前置检查 ----
Test-Cmd git
Test-Cmd gh

$null = Invoke-Native { gh auth status }
if ($LASTEXITCODE -ne 0) { Fail "gh 未登录" "先执行 gh auth login" }

if (-not (Test-Path $distDir)) { Fail "找不到产物目录：$distDir" "先执行 cargo xtask dist" }
if (-not (Test-Path $zipPath)) { Fail "找不到压缩包：$zipPath" "先执行 cargo xtask dist" }

$assets = @($zipPath)
if ($includeRaw) {
  $assets += Get-ChildItem $distDir -File | ForEach-Object { $_.FullName }
}

# workspace 版本漂移（仅告警）
$wsVersion = $null
$cargoRaw = [System.IO.File]::ReadAllText($cargoPath)
$wsReadPattern = '(?ms)\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"'
$wsWritePattern = '(?ms)(\[workspace\.package\][^\[]*?^version\s*=\s*")[^"]*(")'
$m = [regex]::Match($cargoRaw, $wsReadPattern)
if ($m.Success) { $wsVersion = $m.Groups[1].Value }
if ($wsVersion -and $wsVersion -ne $version) {
  Write-Host "WARN: Cargo.toml workspace 版本 ($wsVersion) 与配置版本 ($version) 不一致" -ForegroundColor Yellow
}

# git 状态：上游 / 未推送提交 / 本地与远端是否已有同名 tag / 是否已有同名 release
$ahead = Invoke-Native { git -C $repoRoot rev-list --count '@{u}..HEAD' }
if ($LASTEXITCODE -ne 0) { Fail "当前分支没有上游分支" "先执行 git push -u $remote <branch>" }
if ([int]"$ahead" -gt 0) { Fail "有 $ahead 个本地提交尚未推送" "tag 会打在远端分支头上，请先 git push" }

if (& git -C $repoRoot tag -l $tag) { Fail "本地已存在 tag：$tag" "git tag -d $tag 后重试，或修改 config.json 版本号" }
$remoteTag = Invoke-Native { git -C $repoRoot ls-remote --tags $remote $tag }
if ($LASTEXITCODE -ne 0) { Write-Host "WARN: 无法查询远端 tag（网络问题？），继续" -ForegroundColor Yellow }
elseif ($remoteTag) { Fail "远端已存在 tag：$tag" "该版本可能已发布过；检查 config.json 版本号" }

$null = Invoke-Native { gh release view $tag --repo $repo }
if ($LASTEXITCODE -eq 0) { Fail "GitHub 已存在 release：$tag" "该版本可能已发布过；检查 config.json 版本号" }

$parts = $version.Split('.')
$newVersion = '{0}.{1}.{2}' -f $parts[0], $parts[1], ([int]$parts[2] + 1)

# ---- DryRun：只打印计划，不产生任何副作用 ----
if ($DryRun) {
  Write-Host ""
  Write-Host "== DryRun：以下操作不会真正执行 ==" -ForegroundColor Cyan
  Write-Host "  1. 创建并推送 tag : $tag -> $remote"
  Write-Host "  2. 创建 release   : Matcha $tag @ $repo$(if ($Draft) { ' (draft)' })$(if ($Prerelease) { ' (prerelease)' })"
  Write-Host "  3. 上传资产:"
  $assets | ForEach-Object { Write-Host "       $_" }
  Write-Host "  4. 版本号升级     : $version -> $newVersion (config.json$(if ($syncCargo) { ' + Cargo.toml' }))"
  Write-Host ""
  Write-Host "所有前置检查已通过。" -ForegroundColor Green
  exit 0
}

# ---- 3. 打 tag 并推送 ----
$null = Invoke-Native { git -C $repoRoot tag -a $tag -m "Matcha $tag" }
if ($LASTEXITCODE -ne 0) { Fail "创建 tag 失败：$tag" }
$null = Invoke-Native { git -C $repoRoot push $remote $tag }
if ($LASTEXITCODE -ne 0) { Fail "推送 tag 失败：$tag" "重试：git push $remote $tag" }

# ---- 4. 创建 release 并上传资产 ----
$ghArgs = @('release', 'create', $tag, '--repo', $repo, '--title', "Matcha $tag")
if ($GenerateNotes) { $ghArgs += '--generate-notes' }
elseif ($Notes) { $ghArgs += @('--notes', $Notes) }
else { $ghArgs += @('--notes', "Matcha $tag") }
if ($Draft) { $ghArgs += '--draft' }
if ($Prerelease) { $ghArgs += '--prerelease' }
$ghArgs += $assets

$releaseOutput = Invoke-Native { gh @ghArgs }
if ($LASTEXITCODE -ne 0) {
  Write-Host ($releaseOutput | Out-String)
  Fail "gh release create 失败" "tag $tag 已推送；如需重来可执行 git push $remote :$tag 删除远端 tag"
}
$releaseUrl = ($releaseOutput | Out-String).Trim()

# ---- 5. 升级版本号（config.json + Cargo.toml）----
$config.version = $newVersion
[System.IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json), $utf8NoBom)

$cargoSynced = $false
if ($syncCargo -and $wsVersion -ne $newVersion) {
  $replacement = '${1}' + $newVersion + '${2}'
  $newRaw = [regex]::Replace($cargoRaw, $wsWritePattern, $replacement)
  if ($newRaw -ne $cargoRaw) {
    [System.IO.File]::WriteAllText($cargoPath, $newRaw, $utf8NoBom)
    $cargoSynced = $true
  } else {
    Write-Host "WARN: 未能更新 Cargo.toml 的 workspace 版本号，请手工检查" -ForegroundColor Yellow
  }
}

Write-Host ""
Write-Host "== 发布完成 ==" -ForegroundColor Green
Write-Host "  release : $releaseUrl"
Write-Host "  已发布  : $version"
Write-Host "  下一版本: $newVersion (config.json 已更新$(if ($cargoSynced) { '，Cargo.toml 已同步' }))"
Write-Host ""
Write-Host "后续提示："
Write-Host "  - config.json / Cargo.toml 的版本变更留在工作区，请审阅后自行提交。"
Write-Host "  - Cargo.lock 会在下次 cargo 构建时自动刷新。"
Write-Host "  - 下次发布前先运行 cargo xtask dist 生成 matcha-$newVersion-$platform 目录。"
