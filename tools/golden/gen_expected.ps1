<#
Generates raw reference reports from synthetic UTF-8 templates. No key/license files
are accessed. Normal mode builds verified corpus rule templates by default; explicit
custom templates remain supported and are preflighted. Diagnostic mode is deliberately
forbidden from writing to the checked-in oracle corpus.

-Layout xml (default) writes expected.<profile>.xml; -Layout patch writes the three patch
formats per profile (expected.<profile>.patch-<fmt>.txt) plus the extra samples in
$PatchExtras. -DryRun prints every target path and exits before taking the lock or
touching any reference setting (ReferenceExe/WorkerId are then not needed).
#>
[CmdletBinding()]
param(
    [string]$ReferenceExe='',
    [string]$WorkerId='',
    [ValidateSet('xml','patch')][string]$Layout='xml',
    [switch]$DryRun,
    [ValidateRange(0,600)][int]$LockTimeoutSeconds=600,
    [string]$CorpusRoot='',
    [string]$OutputRoot='',
    [string]$ComparisonTemplates='',
    [string]$SessionsPath=(Join-Path $env:APPDATA 'Scooter Software/Beyond Compare 5/BCSessions.xml'),
    [string]$FileFormatsPath=(Join-Path $env:APPDATA 'Scooter Software/Beyond Compare 5/BCFileFormats.xml'),
    [string[]]$CaseNames=@(),
    [switch]$MyersNeverAlignDifferences,
    [switch]$DiagnosticInheritRules
)
$ErrorActionPreference='Stop'
$scriptRoot=Split-Path -Parent $MyInvocation.MyCommand.Path
if(!$CorpusRoot){$CorpusRoot=Join-Path $scriptRoot '../../crates/bc-core/tests/golden/text/2way'}
# Extra patch samples: `title:` placement and `ignore-unimportant` on the cases whose
# rule templates mark something unimportant.
$PatchExtras=@{
    '005-insert-middle-three'=@(@{Mode='myers';Suffix='patch-unified.title';Options='patch-unified';Title='Matcha Golden Title'})
}
foreach($name in @('013-comment-change','014-comment-deletion','016-indent-ignored','017-embedded-whitespace','018-trailing-whitespace','019-case-ignored','030-multiline-comment')){
    $PatchExtras[$name]=@(foreach($mode in @('myers','standard')){@{Mode=$mode;Suffix='patch-unified.ignore-unimportant';Options='patch-unified,ignore-unimportant';Title=''}})
}
# Every report one case produces: target leaf name + the text-report line that writes it.
function Report-Lines([string]$caseName,[string]$destination) {
    $lines=@()
    foreach($mode in @('myers','standard')) {
        if($Layout -eq 'xml') {
            $leaf="expected.$mode.xml"
            $lines+=@{Leaf=$leaf;Command="text-report layout:xml options:display-all output-to:`"$(Join-Path $destination $leaf)`" `"matcha-golden-$mode`""}
        } else {
            foreach($format in @('normal','context','unified')) {
                $leaf="expected.$mode.patch-$format.txt"
                $lines+=@{Leaf=$leaf;Command="text-report layout:patch options:patch-$format output-to:`"$(Join-Path $destination $leaf)`" `"matcha-golden-$mode`""}
            }
        }
    }
    if($Layout -eq 'patch' -and $PatchExtras.ContainsKey($caseName)) {
        foreach($extra in $PatchExtras[$caseName]) {
            $leaf="expected.$($extra.Mode).$($extra.Suffix).txt"
            $title=if($extra.Title){" title:`"$($extra.Title)`""}else{''}
            $lines+=@{Leaf=$leaf;Command="text-report layout:patch options:$($extra.Options)$title output-to:`"$(Join-Path $destination $leaf)`" `"matcha-golden-$($extra.Mode)`""}
        }
    }
    return ,$lines
}
if($DryRun) {
    $root=(Resolve-Path -LiteralPath $CorpusRoot).Path
    $destinationRoot=if($OutputRoot){[IO.Path]::GetFullPath($OutputRoot)}else{$root}
    $count=0
    foreach($case in @(Get-ChildItem -LiteralPath $root -Directory | Where-Object {
        (Test-Path -LiteralPath (Join-Path $_.FullName 'case.properties')) -and (!$CaseNames.Count -or $CaseNames -contains $_.Name)
    } | Sort-Object Name)) {
        foreach($line in (Report-Lines $case.Name (Join-Path $destinationRoot $case.Name))) {
            Write-Output (Join-Path (Join-Path $destinationRoot $case.Name) $line.Leaf)
            $count++
        }
    }
    Write-Output "dry run: $count report(s); no lock taken, no reference setting touched"
    return
}
if(!$ReferenceExe -or !$WorkerId){throw 'ReferenceExe and WorkerId are required unless -DryRun'}
if(!$ComparisonTemplates){$ComparisonTemplates=Join-Path $scriptRoot 'profiles'}
if(!$DiagnosticInheritRules -and !$PSBoundParameters.ContainsKey('ComparisonTemplates')) {
    & (Join-Path $scriptRoot 'build_rule_templates.ps1') -CorpusRoot $CorpusRoot -OutputDirectory $ComparisonTemplates
}
$ReferenceExe=(Resolve-Path -LiteralPath $ReferenceExe).Path
$CorpusRoot=(Resolve-Path -LiteralPath $CorpusRoot).Path
if(!$OutputRoot){$OutputRoot=$CorpusRoot}
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
if($DiagnosticInheritRules -and $OutputRoot.TrimEnd('\','/') -eq $CorpusRoot.TrimEnd('\','/')) {
    throw 'Diagnostic reports must not overwrite checked-in golden expectations'
}
. (Join-Path $scriptRoot 'reference_lock.ps1')
$referenceLease=Enter-MatchaReferenceLock -WorkerId $WorkerId -TimeoutSeconds $LockTimeoutSeconds
try {
if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw 'Close the reference tool before modifying its two temporary sessions'}
if(!(Test-Path -LiteralPath $SessionsPath -PathType Leaf)){throw 'The explicit session XML file does not exist'}
$cases=@(Get-ChildItem -LiteralPath $CorpusRoot -Directory | Where-Object {
    (Test-Path -LiteralPath (Join-Path $_.FullName 'case.properties')) -and (!$CaseNames.Count -or $CaseNames -contains $_.Name)
} | Sort-Object Name)
if(!$cases.Count){throw 'No corpus cases selected'}
$repo=[IO.Path]::GetFullPath((Join-Path $scriptRoot '../..'))
$work=Join-Path $repo 'target/golden-work'
$utf8=New-Object Text.UTF8Encoding($false,$true)
function Properties([string]$path) {
    $result=@{}
    foreach($line in [IO.File]::ReadAllLines($path,$utf8)) {
        $line=$line.Trim();if(!$line -or $line.StartsWith('#')){continue}
        $separator=$line.IndexOf('=')
        if($separator -le 0){throw "Invalid property: $line"}
        $key=$line.Substring(0,$separator)
        if($result.ContainsKey($key)){throw "Duplicate property: $key"}
        $result[$key]=$line.Substring($separator+1)
    }
    return $result
}
function Get-Property($props,[string]$key,[string]$default) {if($props.ContainsKey($key)){return $props[$key]};return $default}
function Escape-Xml([string]$value) {[Security.SecurityElement]::Escape($value)}
function Encode-Input([string]$path,$props,[string]$side) {
    $source=[IO.File]::ReadAllText($path,$utf8).Replace("`r`n","`n").Replace("`r","`n")
    $ending=Get-Property $props ($side+'_ending') 'lf'
    $parts=$source.Split(@("`n"),[StringSplitOptions]::None)
    $text=New-Object Text.StringBuilder
    for($i=0;$i -lt $parts.Length;$i++) {
        $null=$text.Append($parts[$i])
        if($i -lt $parts.Length-1) {
            $eol=switch($ending){'lf'{"`n"};'crlf'{"`r`n"};'cr'{"`r"};'cycle'{@("`r`n","`n","`r")[$i%3]};default{throw 'Unknown line-ending policy'}}
            $null=$text.Append($eol)
        }
    }
    $bom=(Get-Property $props ($side+'_bom') 'false') -eq 'true'
    $encoding=switch(Get-Property $props ($side+'_encoding') 'utf8') {
        'utf8'{New-Object Text.UTF8Encoding($bom,$true)}
        'utf16le'{New-Object Text.UnicodeEncoding($false,$bom,$true)}
        'utf16be'{New-Object Text.UnicodeEncoding($true,$bom,$true)}
        '936'{[Text.Encoding]::GetEncoding(936)}
        '1252'{[Text.Encoding]::GetEncoding(1252)}
        default{throw 'Unknown fixture encoding'}
    }
    return ,([byte[]]($encoding.GetPreamble()+$encoding.GetBytes($text.ToString())))
}
function Rules-Body($case,$props) {
    if($DiagnosticInheritRules){return ''}
    $leaf=Get-Property $props 'native_rules' ($case.Name+'.rules.xml')
    if($leaf -notmatch '^[A-Za-z0-9_.-]+\.rules\.xml$'){throw 'Invalid comparison template filename'}
    $path=Join-Path $ComparisonTemplates $leaf
    if(!(Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Missing verified native comparison template: $path; refusing to guess importance settings"
    }
    $xml=[IO.File]::ReadAllText($path,$utf8)
    if($xml -match '(?i)<!DOCTYPE'){throw 'External document types are not allowed'}
    $xml=[regex]::Replace($xml,'^\s*<\?xml[^>]*>','').Trim()
    $match=[regex]::Match($xml,'(?s)^<Rules\s+Value="TTextRules"\s*>(.*)</Rules>$')
    if(!$match.Success){throw "Expected a Rules Value=TTextRules template: $path"}
    $body=$match.Groups[1].Value
    foreach($name in @('AlignKind','NeverAlignDiffs','SkewTolerance','UseCloseness')) {
        $body=[regex]::Replace($body,"(?s)<$name\b[^>]*/>",'')
    }
    return $body
}
# Preflight every template/input before modifying user settings.
$plans=@()
foreach($case in $cases) {
    if($case.Name -notmatch '^\d{3}-[a-z0-9-]+$'){throw 'Unexpected corpus directory name'}
    $props=Properties (Join-Path $case.FullName 'case.properties')
    $plans+=@{Case=$case;Props=$props;Rules=(Rules-Body $case $props)}
}
$backup=Join-Path $scriptRoot ('backup/'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
$null=New-Item -ItemType Directory -Path $backup -Force
Copy-Item -LiteralPath $SessionsPath -Destination (Join-Path $backup 'BCSessions.xml')
$original=[IO.File]::ReadAllBytes($SessionsPath)
$base=[IO.File]::ReadAllText($SessionsPath,$utf8)
# Only these two explicitly authorized names are replaced; all other XML is retained.
$base=[regex]::Replace($base,'(?s)[\t ]*<TTextCompareSession Value="matcha-golden-(?:myers|standard)">.*?</TTextCompareSession>\r?\n?','')
$close=$base.LastIndexOf('</BCSessions>')
if($close -lt 0){throw 'Unexpected session document root'}
$needsLongFormat=@($plans | Where-Object { $_.Props['reference_extension'] -eq 'matcha-golden-long' }).Count -gt 0
$formatsExisted=$false
$formatsOriginal=$null
$temporaryFormats=$null
if($needsLongFormat){
    $formatsExisted=Test-Path -LiteralPath $FileFormatsPath -PathType Leaf
    if($formatsExisted){
        Copy-Item -LiteralPath $FileFormatsPath -Destination (Join-Path $backup 'BCFileFormats.xml')
        $formatsOriginal=[IO.File]::ReadAllBytes($FileFormatsPath)
        $formatsText=[IO.File]::ReadAllText($FileFormatsPath,$utf8)
    } else {
        $formatsText="<?xml version=`"1.0`" encoding=`"UTF-8`"?>`r`n<BCFileFormats>`r`n</BCFileFormats>`r`n"
    }
    if($formatsText.Contains('matcha-golden-long') -or $formatsText.Contains('Matcha Golden Long Text')){
        throw 'Dedicated golden long-line format collides with an existing user format'
    }
    $formatRoot=[regex]::Match($formatsText,'<BCFileFormats\b[^>]*>')
    if(!$formatRoot.Success){throw 'Unexpected file-format document root'}
    $node="`r`n`t<TTextFormat Value=`"Matcha Golden Long Text`">`r`n`t`t<Mask Value=`"*.matcha-golden-long`"/>`r`n`t`t<CharPerLine Value=`"65536`"/>`r`n`t</TTextFormat>`r`n"
    # The exclusive mask is first so even a user's broad mask cannot intercept it.
    $temporaryFormats=$formatsText.Insert($formatRoot.Index+$formatRoot.Length,$node)
}
$ownedProcess=$null
try {
    if($needsLongFormat){[IO.File]::WriteAllText($FileFormatsPath,$temporaryFormats,$utf8)}
    foreach($plan in $plans) {
        if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw 'Reference process is still running'}
        $case=$plan.Case;$props=$plan.Props
        $directory=Join-Path $work $case.Name
        $destination=Join-Path $OutputRoot $case.Name
        $null=New-Item -ItemType Directory -Path $directory -Force
        $null=New-Item -ItemType Directory -Path $destination -Force
        $extension=switch(Get-Property $props 'format' ''){'CPP'{'.cpp'};'PAS'{'.pas'};default{'.txt'}}
        if($props.ContainsKey('reference_extension')) {
            if($props['reference_extension'] -ne 'matcha-golden-long'){throw 'Unknown dedicated reference extension'}
            $extension='.matcha-golden-long'
        }
        $left=Join-Path $directory ('left'+$extension);$right=Join-Path $directory ('right'+$extension)
        [IO.File]::WriteAllBytes($left,(Encode-Input (Join-Path $case.FullName 'left.txt') $props 'left'))
        [IO.File]::WriteAllBytes($right,(Encode-Input (Join-Path $case.FullName 'right.txt') $props 'right'))
        $nodes=''
        foreach($mode in @('myers','standard')) {
            $align=if($mode -eq 'myers'){'alignQuick'}else{'alignSmart'}
            $closeness=if($mode -eq 'standard'){'True'}else{'False'}
            $never=if($mode -eq 'myers' -and $MyersNeverAlignDifferences){'True'}else{'False'}
            $nodes+="`t<TTextCompareSession Value=`"matcha-golden-$mode`">`r`n"
            $nodes+="`t`t<LastModified Value=`"$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')`"/>`r`n`t`t<Specs>`r`n"
            $nodes+="`t`t`t<Left Value=`"$(Escape-Xml $left)`"/>`r`n`t`t`t<Right Value=`"$(Escape-Xml $right)`"/>`r`n`t`t</Specs>`r`n"
            $nodes+="`t`t<Rules Value=`"TTextRules`">`r`n$($plan.Rules)`r`n"
            $nodes+="`t`t`t<AlignKind Value=`"$align`"/>`r`n`t`t`t<NeverAlignDiffs Value=`"$never`"/>`r`n`t`t`t<SkewTolerance Value=`"2000`"/>`r`n`t`t`t<UseCloseness Value=`"$closeness`"/>`r`n`t`t</Rules>`r`n`t</TTextCompareSession>`r`n"
        }
        [IO.File]::WriteAllText($SessionsPath,$base.Insert($close,$nodes),$utf8)
        $log=Join-Path $directory 'generation.log';$script=Join-Path $directory 'report.txt'
        $commands="log verbose `"$log`"`r`nload <default>`r`n"
        $reports=Report-Lines $case.Name $destination
        foreach($line in $reports) {
            # Fresh reports only: a leftover file must never pass as a new capture.
            $path=Join-Path $destination $line.Leaf
            if(Test-Path -LiteralPath $path){Remove-Item -LiteralPath $path}
            $commands+="$($line.Command)`r`n"
        }
        [IO.File]::WriteAllText($script,$commands,$utf8)
        $ownedProcess=Start-Process -FilePath $ReferenceExe -ArgumentList @(('"@'+$script+'"'),'/silent','/solo') -PassThru
        if(!$ownedProcess.WaitForExit(60000)){$ownedProcess.Kill();$ownedProcess.WaitForExit();throw 'Owned report process timed out'}
        $exit=$ownedProcess.ExitCode;$ownedProcess=$null
        if($exit -ne 0){throw "Reference report failed for $($case.Name), exit $exit; see $log"}
        foreach($line in $reports) {
            $path=Join-Path $destination $line.Leaf
            if(!(Test-Path -LiteralPath $path -PathType Leaf)){throw "Missing report $path"}
        }
        Write-Output "generated $($case.Name)"
    }
} finally {
    if($ownedProcess -and !$ownedProcess.HasExited){$ownedProcess.Kill();$ownedProcess.WaitForExit()}
    if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw "Cannot restore settings while an external reference instance is running; backup: $backup"}
    try { [IO.File]::WriteAllBytes($SessionsPath,$original) }
    finally {
        if($needsLongFormat){
            if($formatsExisted){[IO.File]::WriteAllBytes($FileFormatsPath,$formatsOriginal)}
            elseif(Test-Path -LiteralPath $FileFormatsPath){Remove-Item -LiteralPath $FileFormatsPath}
        }
    }
    if([Convert]::ToBase64String([IO.File]::ReadAllBytes($SessionsPath)) -cne [Convert]::ToBase64String($original)) {throw 'Session restoration verification failed'}
    if($needsLongFormat){
        if($formatsExisted){
            if([Convert]::ToBase64String([IO.File]::ReadAllBytes($FileFormatsPath)) -cne [Convert]::ToBase64String($formatsOriginal)){throw 'Format restoration verification failed'}
        } elseif(Test-Path -LiteralPath $FileFormatsPath){throw 'Temporary format file was not removed'}
    }
    Write-Output "session/temporary-format settings restored and byte-verified; backup: $backup"
}
} finally {
    Exit-MatchaReferenceLock -Lease $referenceLease
}
