#requires -Version 5.1
# Offline regression: uses only a temporary distribution and read-only git/gh stubs.
$ErrorActionPreference = 'Stop'
$temp = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$dist = Join-Path $temp 'matcha-0.0.0-windows-x86_64'
$skill = Join-Path $temp '.agents\skills\matcha-publish'
$verify = Join-Path $skill 'verify-release.ps1'
$fixtureCommit = 'a' * 40
$body = "## $([char]0x53D8)$([char]0x5316)`n`n- **GUI**"
$release = @{tagName='v0.0.0'; isDraft=$false; isPrerelease=$false; body=$body; assets=@(); url='release-url'}
$params = @{Tag='v0.0.0'; Commit=$fixtureCommit; DistDir=$dist; Notes=$body}

function git {
    if ($args -notcontains 'ls-remote' -or $args[-1] -cne 'refs/tags/v0.0.0^{}') { throw 'Unexpected git invocation' }
    $global:LASTEXITCODE = 0
    "$fixtureCommit`trefs/tags/v0.0.0^{}"
}
function gh {
    if ($args[0] -ne 'release' -or $args[1] -ne 'view') { throw 'Unexpected gh invocation' }
    $global:LASTEXITCODE = 0
    $release | ConvertTo-Json -Depth 5 -Compress
}
function Expect-Failure([string]$message) {
    try { & $verify @params | Out-Null } catch {
        if (-not $_.Exception.Message.Contains($message)) { throw }
        return
    }
    throw "Expected verification to fail: $message"
}

try {
    $null = New-Item -ItemType Directory -Path $skill, $dist -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\verify-release.ps1') -Destination $verify
    $config = Join-Path $skill 'config.json'
    '{"remote":"origin","repo":"fixture/repo","includeRawFiles":true}' | Set-Content $config -Encoding Ascii
    $paths = @("$dist.zip") + @('matcha.exe','matcha.com','matcha-comp.exe','README.txt' | ForEach-Object { Join-Path $dist $_ })
    foreach ($path in $paths) {
        [IO.File]::WriteAllBytes($path, [byte[]](1, 2, 3))
        $release.assets += @{ name=[IO.Path]::GetFileName($path); size=3; state='uploaded'; url='asset-url'
            digest='sha256:' + (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant() }
    }

    & $verify @params | Out-Null
    $release.body = 'wrong'; Expect-Failure 'Release notes differ'; $release.body = $body
    $params.Commit = 'b' * 40; Expect-Failure 'does not resolve'; $params.Commit = $fixtureCommit
    $release.isDraft = $true; Expect-Failure 'draft/prerelease status'; $release.isDraft = $false
    [IO.File]::WriteAllBytes((Join-Path $dist 'matcha.exe'), [byte[]](3, 2, 1))
    Expect-Failure 'SHA-256 mismatch: matcha.exe'

    $release.assets = @($release.assets[0]) # Only the ZIP is allowed with includeRawFiles=false.
    Expect-Failure 'Expected 5 release assets'
    '{"remote":"origin","repo":"fixture/repo","includeRawFiles":false}' | Set-Content $config -Encoding Ascii
    & $verify @params | Out-Null
    Write-Output 'OK: raw+ZIP, ZIP-only, and rejection of wrong notes, tag target, status, checksum and asset count.'
} finally {
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Recurse -Force }
}
