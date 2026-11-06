# Shared reference-tool lease. Native calls/settings writes must hold this lease.
function Enter-MatchaReferenceLock {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)][string]$WorkerId,
        [ValidateRange(0,600)][int]$TimeoutSeconds=600,
        [string]$Path=(Join-Path $env:TEMP 'matcha-reference-tool.lock')
    )
    $timer=[Diagnostics.Stopwatch]::StartNew()
    while($true) {
        try {
            $stream=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::Read)
        } catch [IO.IOException] {
            if(!(Test-Path -LiteralPath $Path -PathType Leaf)){throw}
            if($timer.Elapsed.TotalSeconds -ge $TimeoutSeconds){throw "Reference-tool lock timeout: $Path"}
            Start-Sleep -Milliseconds 1000
            continue
        }
        try {
            $payload=@{worker_id=$WorkerId;pid=$PID;created_utc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Compress
            $bytes=[Text.Encoding]::UTF8.GetBytes($payload)
            $stream.Write($bytes,0,$bytes.Length)
            $stream.Flush($true)
            $stream.Dispose()
            # Keep a read-only lease so ordinary readers can inspect owner metadata.
            # CreateNew contenders still cannot acquire while the filename exists.
            $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
            return [pscustomobject]@{Path=$Path;Stream=$stream;Released=$false}
        } catch {
            $stream.Dispose()
            [IO.File]::Delete($Path)
            throw
        }
    }
}
function Exit-MatchaReferenceLock {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)]$Lease)
    if($Lease.Released){return}
    $Lease.Stream.Dispose()
    [IO.File]::Delete($Lease.Path)
    $Lease.Released=$true
}
