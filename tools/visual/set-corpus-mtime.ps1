param([string]$Root = (Join-Path $PSScriptRoot 'corpus'), [datetime]$Timestamp = [datetime]'2026-01-02 03:04:05')
$ErrorActionPreference='Stop'
Get-ChildItem -LiteralPath $Root -Recurse -File | ForEach-Object { $_.LastWriteTime=$Timestamp; $_.CreationTime=$Timestamp; $_.LastAccessTime=$Timestamp }
Get-ChildItem -LiteralPath $Root -Recurse -Directory | ForEach-Object { $_.LastWriteTime=$Timestamp; $_.CreationTime=$Timestamp; $_.LastAccessTime=$Timestamp }
