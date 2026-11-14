<#
Collects reference `/automerge` results for the three-way merge corpus
(crates/bc-engine-text/tests/golden/merge/<case>/{left,right,center}.txt).

For every case and every variant the reference launcher is run on copies of the inputs in
target/golden-work/merge, with a pre-existing output file holding a sentinel. Recorded per
variant, next to the inputs:

  expected.<variant>.code   exit code, then `output: written|untouched|missing`, then the
                            names of any extra files the run left behind (backups)
  expected.<variant>.out    the output bytes, only when the run wrote the output file

No reference setting is written: the runs use the installed default Text Merge rules. The
shared lock (reference_lock.ps1) is still held, and the script refuses to run while another
reference instance is open. No key/license file is read.

  ./tools/golden/gen_merge_expected.ps1 -DryRun
  ./tools/golden/gen_merge_expected.ps1 -ReferenceExe '<reference BComp.com>' -WorkerId '<run id>'
#>
[CmdletBinding()]
param(
    [string]$ReferenceExe='',
    [string]$WorkerId='',
    [ValidateRange(0,600)][int]$LockTimeoutSeconds=600,
    [string]$CorpusRoot='',
    [string[]]$CaseNames=@(),
    [ValidateRange(5,600)][int]$RunTimeoutSeconds=60,
    [switch]$DryRun
)
$ErrorActionPreference='Stop'
$scriptRoot=Split-Path -Parent $MyInvocation.MyCommand.Path
if(!$CorpusRoot){$CorpusRoot=Join-Path $scriptRoot '../../crates/bc-engine-text/tests/golden/merge'}
$root=(Resolve-Path -LiteralPath $CorpusRoot).Path
$Variants=[ordered]@{
    'automerge'=@('/automerge')
    'force'=@('/automerge','/force')
    'iu'=@('/automerge','/iu')
    'iu-favorleft'=@('/automerge','/iu','/favorleft')
}
$Sentinel=[Text.Encoding]::ASCII.GetBytes("MATCHA-PREEXISTING-OUTPUT`n")
$cases=@(Get-ChildItem -LiteralPath $root -Directory | Where-Object {
    (Test-Path -LiteralPath (Join-Path $_.FullName 'center.txt')) -and (!$CaseNames.Count -or $CaseNames -contains $_.Name)
} | Sort-Object Name)
if($DryRun) {
    $count=0
    foreach($case in $cases){foreach($variant in $Variants.Keys){
        Write-Output (Join-Path $case.FullName "expected.$variant.code")
        Write-Output (Join-Path $case.FullName "expected.$variant.out")
        $count++
    }}
    Write-Output "dry run: $count run(s); no lock taken, no reference setting touched"
    return
}
if(!$ReferenceExe -or !$WorkerId){throw 'ReferenceExe and WorkerId are required unless -DryRun'}
if(!(Test-Path -LiteralPath $ReferenceExe -PathType Leaf)){throw "Reference launcher not found: $ReferenceExe"}
. (Join-Path $scriptRoot 'reference_lock.ps1')
$work=[IO.Path]::GetFullPath((Join-Path $scriptRoot '../../target/golden-work/merge'))
$lease=Enter-MatchaReferenceLock -WorkerId $WorkerId -TimeoutSeconds $LockTimeoutSeconds
$owned=$null
try {
    if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw 'Close the reference tool before collecting merge samples'}
    foreach($case in $cases) {
        foreach($variant in $Variants.Keys) {
            $dir=Join-Path $work "$($case.Name)/$variant"
            if(Test-Path -LiteralPath $dir){Remove-Item -LiteralPath $dir -Recurse -Force}
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
            foreach($leaf in @('left.txt','right.txt','center.txt')){Copy-Item -LiteralPath (Join-Path $case.FullName $leaf) -Destination (Join-Path $dir $leaf)}
            $out=Join-Path $dir 'out.txt'
            [IO.File]::WriteAllBytes($out,$Sentinel)
            $arguments=@($Variants[$variant])+@('/solo',"`"$(Join-Path $dir 'left.txt')`"","`"$(Join-Path $dir 'right.txt')`"","`"$(Join-Path $dir 'center.txt')`"","`"$out`"")
            $owned=Start-Process -FilePath $ReferenceExe -ArgumentList $arguments -PassThru -WindowStyle Hidden
            if(!$owned.WaitForExit($RunTimeoutSeconds*1000)) {
                $owned.Kill();$owned.WaitForExit()
                Get-Process -Name BCompare -ErrorAction SilentlyContinue | Where-Object {$_.StartTime -ge $owned.StartTime} | ForEach-Object {$_.Kill()}
                throw "Reference run timed out: $($case.Name) [$variant]"
            }
            $code=$owned.ExitCode
            $owned=$null
            $state='missing'
            $bytes=$null
            if(Test-Path -LiteralPath $out -PathType Leaf) {
                $bytes=[IO.File]::ReadAllBytes($out)
                $state=if([Linq.Enumerable]::SequenceEqual([byte[]]$bytes,[byte[]]$Sentinel)){'untouched'}else{'written'}
            }
            $extra=@(Get-ChildItem -LiteralPath $dir -File | Where-Object {@('left.txt','right.txt','center.txt','out.txt') -notcontains $_.Name} | Sort-Object Name | ForEach-Object {$_.Name})
            $report=@("$code","output: $state")+@($extra | ForEach-Object {"extra: $_"})
            [IO.File]::WriteAllText((Join-Path $case.FullName "expected.$variant.code"),(($report -join "`n")+"`n"),(New-Object Text.UTF8Encoding($false)))
            $target=Join-Path $case.FullName "expected.$variant.out"
            if($state -eq 'written'){[IO.File]::WriteAllBytes($target,$bytes)}elseif(Test-Path -LiteralPath $target){Remove-Item -LiteralPath $target -Force}
            Write-Output "$($case.Name) [$variant] -> $code, $state"
        }
    }
} finally {
    if($owned -and !$owned.HasExited){$owned.Kill();$owned.WaitForExit()}
    Exit-MatchaReferenceLock -Lease $lease
}
