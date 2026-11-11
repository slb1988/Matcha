<#
Collects the reference `text-report layout:patch` samples (patch-normal, patch-context,
patch-unified per profile, plus the title/ignore-unimportant extras) for the two-way text
corpus. Thin wrapper: the lock, backup, temporary named sessions and byte-verified restore
are exactly those of gen_expected.ps1 (-Layout patch), so there is one protocol only.

  ./tools/golden/gen_patch_expected.ps1 -DryRun
  ./tools/golden/gen_patch_expected.ps1 -ReferenceExe '<reference exe>' -WorkerId '<run id>'
#>
[CmdletBinding()]
param(
    [string]$ReferenceExe='',
    [string]$WorkerId='',
    [ValidateRange(0,600)][int]$LockTimeoutSeconds=600,
    [string]$CorpusRoot='',
    [string[]]$CaseNames=@(),
    [switch]$DryRun
)
$ErrorActionPreference='Stop'
$arguments=@{Layout='patch';LockTimeoutSeconds=$LockTimeoutSeconds;CaseNames=$CaseNames}
if($CorpusRoot){$arguments.CorpusRoot=$CorpusRoot}
if($DryRun){$arguments.DryRun=$true}
if($ReferenceExe){$arguments.ReferenceExe=$ReferenceExe}
if($WorkerId){$arguments.WorkerId=$WorkerId}
& (Join-Path $PSScriptRoot 'gen_expected.ps1') @arguments
