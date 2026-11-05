# Does not launch the reference tool or touch its settings.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'reference_lock.ps1')
$path=Join-Path $env:TEMP ("matcha-reference-lock-test-"+$PID+".lock")
$lease=Enter-MatchaReferenceLock -WorkerId 'lock-test' -Path $path
try {
    $json=[IO.File]::ReadAllText($path) | ConvertFrom-Json
    if($json.worker_id -ne 'lock-test' -or !$json.created_utc){throw 'Missing owner/time'}
    $blocked=$false
    try {$other=Enter-MatchaReferenceLock -WorkerId 'contender' -Path $path -TimeoutSeconds 0}
    catch {if($_.Exception.Message -like 'Reference-tool lock timeout:*'){$blocked=$true}else{throw}}
    if(!$blocked){Exit-MatchaReferenceLock $other;throw 'Exclusive create did not exclude contender'}
} finally {Exit-MatchaReferenceLock $lease}
if(Test-Path -LiteralPath $path){throw 'Lock was not removed'}
$lease=Enter-MatchaReferenceLock -WorkerId 'next-owner' -Path $path -TimeoutSeconds 0
Exit-MatchaReferenceLock $lease
Write-Output 'exclusive creation, metadata, contention timeout and finally cleanup passed'
