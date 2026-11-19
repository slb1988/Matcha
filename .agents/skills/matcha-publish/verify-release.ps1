#requires -Version 5.1
# Read-only: verify an existing Windows release against the exact build that was published.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$Commit,
    [Parameter(Mandatory)][string]$DistDir,
    [ValidateNotNullOrEmpty()][string]$Notes,
    [switch]$Draft,
    [switch]$Prerelease
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$cfg = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'config.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$dist = (Get-Item -LiteralPath $DistDir).FullName

# An annotated tag's object ID is not the commit ID. Ask the remote for the peeled ref.
$ref = git -C $root ls-remote --exit-code --tags $cfg.remote "refs/tags/${Tag}^{}"
if ($LASTEXITCODE -ne 0 -or ($ref -split '\s+')[0] -ne $Commit) {
    throw "Remote tag $Tag does not resolve to $Commit (or could not be read)."
}

# Windows PowerShell 5.1 decodes native stdout using the console output encoding.
$encoding = [Console]::OutputEncoding
try {
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $json = gh release view $Tag --repo $cfg.repo --json 'tagName,isDraft,isPrerelease,body,assets,url'
    if ($LASTEXITCODE -ne 0) { throw "Could not read GitHub Release $Tag." }
} finally { [Console]::OutputEncoding = $encoding }
$release = ($json -join "`n") | ConvertFrom-Json
if ($release.tagName -cne $Tag -or $release.isDraft -ne [bool]$Draft -or $release.isPrerelease -ne [bool]$Prerelease) {
    throw "Unexpected release tag or draft/prerelease status: $Tag"
}
if ([string]::IsNullOrWhiteSpace($release.body)) { throw "Empty release notes: $Tag" }
function Normalize-Notes([string]$value) { $value.Replace("`r`n", "`n").TrimEnd([char]10) }
if ($PSBoundParameters.ContainsKey('Notes') -and (Normalize-Notes $release.body) -cne (Normalize-Notes $Notes)) {
    throw 'Release notes differ from the approved Markdown.'
}

$paths = @("$dist.zip")
if ($cfg.includeRawFiles) {
    $paths += 'matcha.exe', 'matcha.com', 'matcha-comp.exe', 'README.txt' | ForEach-Object { Join-Path $dist $_ }
}
if (@($release.assets).Count -ne $paths.Count) { throw "Expected $($paths.Count) release assets, found $(@($release.assets).Count)." }
foreach ($path in $paths) {
    $file = Get-Item -LiteralPath $path
    $found = @($release.assets | Where-Object { $_.name -ceq $file.Name })
    if ($found.Count -ne 1) { throw "Missing or duplicate asset: $($file.Name)" }
    $asset = $found[0]
    if ($file.PSIsContainer -or $file.Length -eq 0) { throw "Invalid local asset: $path" }
    if (-not $asset.PSObject.Properties['digest'] -or -not $asset.digest) {
        throw "GitHub returned no digest for $($file.Name); compare the downloaded asset separately."
    }
    $digest = 'sha256:' + (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($asset.state -ne 'uploaded' -or $asset.size -ne $file.Length -or $asset.digest -ne $digest) {
        throw "Upload state, size or SHA-256 mismatch: $($file.Name)"
    }
    Write-Output "OK $($file.Name) $digest"
    if ($path -eq "$dist.zip") { $zipUrl = $asset.url }
}
Write-Output "Verified: $($release.url)`nZIP: $zipUrl"
