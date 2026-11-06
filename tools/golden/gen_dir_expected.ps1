param(
    [string]$ReferenceExe = 'D:\Program Files\Beyond Compare 5\BCompare.exe',
    [string]$OutputRoot = (Join-Path $PSScriptRoot '..\..\crates\bc-core\tests\golden\dir'),
    [switch]$Probe,
    [string]$CaseName,
    [string]$CriteriaOverride,
    [string]$FlattenValue = 'dirFlatten',
    [string]$WorkerId = ('manual-' + $PID)
)
$ErrorActionPreference = 'Stop'
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
$lockPath = Join-Path ([IO.Path]::GetTempPath()) 'matcha-reference-tool.lock'
$lock = $null
$process = $null
$sessionChanged = $false
$sessionPath = Join-Path $env:APPDATA 'Scooter Software\Beyond Compare 5\BCSessions.xml'
$sessionOriginal = $null
$sessionTime = $null
$deadline = [DateTime]::UtcNow.AddMinutes(10)
while ($null -eq $lock) {
    try { $lock = [IO.File]::Open($lockPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read) }
    catch [IO.IOException] { if ([DateTime]::UtcNow -ge $deadline) { throw 'Reference tool lock remained busy for ten minutes; retry later without removing the owner lock.' }; Start-Sleep -Seconds 1 }
}
$work = Join-Path ([IO.Path]::GetTempPath()) ('matcha-dir-golden-' + [Guid]::NewGuid().ToString('N'))
try {
    $owner = [Text.Encoding]::UTF8.GetBytes("worker=$WorkerId; utc=$([DateTime]::UtcNow.ToString('o')); pid=$PID")
    $lock.Write($owner,0,$owner.Length); $lock.Flush()
    if (Get-Process BCompare -ErrorAction SilentlyContinue) { throw 'Another reference process is running; no settings or process were changed.' }
    if (!(Test-Path -LiteralPath $ReferenceExe -PathType Leaf)) { throw 'Reference executable not found.' }
    [IO.Directory]::CreateDirectory($work) | Out-Null
    [IO.Directory]::CreateDirectory($OutputRoot) | Out-Null
    function F([string]$Side,[string]$Path,[string]$Text,[int]$Seconds=0,[string]$Attrs='') { @{Side=$Side;Path=$Path;Text=$Text;Seconds=$Seconds;Attrs=$Attrs} }
    $cases = @(
        @{Name='same';Criteria='timestamp:2sec size';Files=@((F left 'same.txt' 'equal'),(F right 'same.txt' 'equal'))},
        @{Name='left-orphan';Criteria='timestamp:2sec size';Files=@((F left 'only.txt' 'left'))},
        @{Name='right-orphan';Criteria='timestamp:2sec size';Files=@((F right 'only.txt' 'right'))},
        @{Name='left-newer';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same' 5),(F right 'a.txt' 'same'))},
        @{Name='right-newer';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same'),(F right 'a.txt' 'same' 5))},
        @{Name='tolerance-inside';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same' 1),(F right 'a.txt' 'same'))},
        @{Name='tolerance-edge';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same' 2),(F right 'a.txt' 'same'))},
        @{Name='tolerance-outside';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same' 3),(F right 'a.txt' 'same'))},
        @{Name='dst-ignore';Criteria='timestamp:2sec;IgnoreDST size';Files=@((F left 'a.txt' 'same' 3600),(F right 'a.txt' 'same'))},
        @{Name='dst-compare';Criteria='timestamp:2sec size';Files=@((F left 'a.txt' 'same' 3600),(F right 'a.txt' 'same'))},
        @{Name='timezone-ignore';Criteria='timestamp:2sec size timezone:ignore';Files=@((F left 'a.txt' 'same' 7200),(F right 'a.txt' 'same'))},
        @{Name='timezone-offset';ReferenceUnsupported=$true;Criteria='timestamp:2sec size timezone:left+6';Files=@((F left 'a.txt' 'same'),(F right 'a.txt' 'same' 21600))},
        @{Name='size-different';Criteria='size';Files=@((F left 'a.txt' 'longer'),(F right 'a.txt' 'x'))},
        @{Name='crc-same';Criteria='CRC';Files=@((F left 'a.bin' 'identical'),(F right 'a.bin' 'identical'))},
        @{Name='crc-different';Criteria='CRC';Files=@((F left 'a.bin' 'AAAA'),(F right 'a.bin' 'BBBB'))},
        @{Name='binary-newer-different';Criteria='timestamp:2sec binary';Files=@((F left 'a.bin' 'AAAA' 5),(F right 'a.bin' 'BBBB'))},
        @{Name='binary-override';Criteria='timestamp:2sec binary';Files=@((F left 'a.txt' 'same' 15),(F right 'a.txt' 'same'))},
        @{Name='hidden-attribute';Criteria='attrib:h size';Files=@((F left 'a.txt' 'same' 0 'h'),(F right 'a.txt' 'same'))},
        @{Name='archive-default';Criteria='size';Files=@((F left 'a.txt' 'same' 0 'a'),(F right 'a.txt' 'same'))},
        @{Name='include-masks';Criteria='size';Filter='"*.c;*.h;-*.bak"';Files=@((F left 'a.c' 'x'),(F right 'a.c' 'x'),(F left 'skip.bak' 'x'),(F right 'extra.txt' 'x'))},
        @{Name='exclude-folder';Criteria='size';Filter='"-Backup\"';Files=@((F left 'Backup/skip.txt' 'x'),(F right 'Backup/skip.txt' 'different'),(F left 'keep.txt' 'x'),(F right 'keep.txt' 'x'))},
        @{Name='root-relative';Criteria='size';Filter='".\Windows\*.txt"';Files=@((F left 'Windows/a.txt' 'x'),(F right 'Windows/a.txt' 'x'),(F left 'deep/Windows/a.txt' 'x'))},
        @{Name='any-depth';Criteria='size';Filter='"...\Windows\*.txt"';Files=@((F left 'Windows/a.txt' 'x'),(F right 'deep/Windows/b.txt' 'x'))},
        @{Name='without-extension';Criteria='size';Filter='"*."';Files=@((F left 'Makefile' 'x'),(F right 'Makefile' 'x'),(F left 'other.c' 'x'))},
        @{Name='character-class';Criteria='size';Filter='"[a-c]*.txt"';Files=@((F left 'apple.txt' 'x'),(F right 'cat.txt' 'x'),(F left 'dog.txt' 'x'))},
        @{Name='flatten-shared-paths';Criteria='size';Flatten=$true;FlatRows=3;Files=@((F left 'a/same.txt' 'A'),(F left 'b/same.txt' 'BB'),(F left 'c/same.txt' 'CCC'),(F right 'b/same.txt' 'BB'),(F right 'c/same.txt' 'CCC'))},
        @{Name='flatten-duplicates';Criteria='size';Flatten=$true;FlatRows=2;Files=@((F left 'alpha/same.txt' 'longer'),(F left 'gamma/same.txt' 'same'),(F right 'beta/same.txt' 'same'))},
        @{Name='flatten';Criteria='size';Flatten=$true;FlatRows=1;Files=@((F left 'alpha/same.txt' 'same'),(F right 'beta/same.txt' 'same'))},
        @{Name='rules-comments';Criteria='rules-based';KnownDiff='rules-unavailable';Files=@((F left 'example.cpp' "int value = 1; // left comment`r`n"),(F right 'example.cpp' "int value = 1; // right comment`r`n"))}
    )
    if ($Probe) { $cases = @($cases[0]) }
    if ($CaseName) { $cases = @($cases | Where-Object {$_.Name -eq $CaseName}); if ($cases.Count -eq 0) {throw 'Unknown case'} }
    else { $cases = @($cases | Where-Object {!$_.ReferenceUnsupported}) }
    if ($CriteriaOverride) { if ($cases.Count -ne 1) {throw 'CriteriaOverride requires one selected case'}; $cases[0].Criteria=$CriteriaOverride }
    $utf8 = New-Object Text.UTF8Encoding($false)
    $baseTime = [DateTimeOffset]::Parse('2026-01-01T00:00:00+00:00')
    foreach ($case in $cases) {
        $caseRoot = Join-Path $work $case.Name
        foreach ($side in @('left','right')) { [IO.Directory]::CreateDirectory((Join-Path $caseRoot $side)) | Out-Null }
        $manifest = New-Object Xml.XmlDocument
        $root = $manifest.CreateElement('Case'); $manifest.AppendChild($root) | Out-Null
        $root.SetAttribute('name',$case.Name); $root.SetAttribute('criteria',$case.Criteria)
        $root.SetAttribute('filter',[string]$case.Filter); $root.SetAttribute('knownDiff',[string]$case.KnownDiff)
        $root.SetAttribute('flatten',([bool]$case.Flatten).ToString().ToLowerInvariant())
        foreach ($file in $case.Files) {
            $filePath = Join-Path (Join-Path $caseRoot $file.Side) $file.Path
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($filePath)) | Out-Null
            [IO.File]::WriteAllText($filePath,$file.Text,$utf8)
            [IO.File]::SetLastWriteTimeUtc($filePath,$baseTime.UtcDateTime.AddSeconds($file.Seconds))
            $attributes = [IO.FileAttributes]::Normal
            if ($file.Attrs.Contains('h')) { $attributes = $attributes -bor [IO.FileAttributes]::Hidden }
            if ($file.Attrs.Contains('a')) { $attributes = $attributes -bor [IO.FileAttributes]::Archive }
            if ($file.Attrs.Contains('r')) { $attributes = $attributes -bor [IO.FileAttributes]::ReadOnly }
            if ($file.Attrs.Contains('s')) { $attributes = $attributes -bor [IO.FileAttributes]::System }
            [IO.File]::SetAttributes($filePath,$attributes)
            $node=$manifest.CreateElement('File'); $node.SetAttribute('side',$file.Side); $node.SetAttribute('path',$file.Path)
            $node.SetAttribute('modified',($baseTime.ToUnixTimeSeconds()+$file.Seconds).ToString()); $node.SetAttribute('attrs',$file.Attrs)
            $root.AppendChild($node) | Out-Null
        }
        $report=Join-Path $caseRoot 'expected.xml'; $log=Join-Path $caseRoot 'reference.log'
        $script=Join-Path $caseRoot 'compare.txt'
        $lines=@("log verbose `"$log`"","criteria $($case.Criteria)",'filter "*.*"','filter cutoff:none attrib:none size:none exclude-protected')
        if ($case.Filter) { $lines += "filter $($case.Filter)" }
        if ($case.Flatten) {
            if (!$sessionChanged) {
                if (!(Test-Path -LiteralPath $sessionPath)) { throw 'An existing session file is required for the guarded named-session test.' }
                $sessionOriginal=[IO.File]::ReadAllBytes($sessionPath)
                $sessionTime=[IO.File]::GetLastWriteTimeUtc($sessionPath)
                [IO.File]::WriteAllBytes((Join-Path $work 'BCSessions.backup.xml'),$sessionOriginal)
            }
            $text=[Text.Encoding]::UTF8.GetString($sessionOriginal)
            $marker='</BCSessions>'; $at=$text.LastIndexOf($marker)
            if($at -lt 0 -or !$text.Contains('<BCSessions')){throw 'Unsupported session container; no write performed'}
            $sessionName='__matcha_folder_golden_'+[Guid]::NewGuid().ToString('N')
            $l=[Security.SecurityElement]::Escape((Join-Path $caseRoot 'left'))
            $r=[Security.SecurityElement]::Escape((Join-Path $caseRoot 'right'))
            $handling=[Security.SecurityElement]::Escape($FlattenValue)
            $entry="<TDirCompareSession Value=`"$sessionName`"><LastModified Value=`"$([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss'))`"/><Specs><Left Value=`"$l`"/><Right Value=`"$r`"/></Specs><Rules Value=`"TDirRules`"/><State><DirHandling Value=`"$handling`"/></State></TDirCompareSession>"
            if (Get-Process BCompare -ErrorAction SilentlyContinue) {throw 'Reference process appeared before session write'}
            $sessionChanged=$true
            [IO.File]::WriteAllText($sessionPath,$text.Insert($at,$entry),$utf8)
            $lines += "load `"$sessionName`""
            $lines += "criteria $($case.Criteria)"
        } else {
            $lines += "load `"$(Join-Path $caseRoot 'left')`" `"$(Join-Path $caseRoot 'right')`""
        }
        $lines += 'expand all'
        $lines += "folder-report layout:xml options:display-all output-to:`"$report`""
        [IO.File]::WriteAllLines($script,$lines,$utf8)
        if (Get-Process BCompare -ErrorAction SilentlyContinue) {throw 'Reference process appeared while holding the exclusive lock'}
        $process=Start-Process -FilePath $ReferenceExe -ArgumentList @('/solo','/silent',('@"'+$script+'"')) -PassThru
        $null=$process.Handle
        if (!$process.WaitForExit(60000)) { $process.Kill(); $process.WaitForExit(); throw "Reference timed out for $($case.Name); owned process stopped before restoring settings." }
        if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report)) {
            $diagnostic=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot ("..\..\target\folder-reference-$($case.Name)-error.log")))
            if(Test-Path -LiteralPath $log){[IO.File]::Copy($log,$diagnostic,$true)}
            throw "Reference failed: $($case.Name), exit=$($process.ExitCode), retained log=$diagnostic"
        }
        $destination=Join-Path $OutputRoot $case.Name; [IO.Directory]::CreateDirectory($destination) | Out-Null
        foreach($file in $case.Files) {
            $relative=Join-Path $file.Side $file.Path; $target=Join-Path $destination $relative
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
            [IO.File]::Copy((Join-Path $caseRoot $relative),$target,$true); [IO.File]::SetAttributes($target,[IO.FileAttributes]::Normal)
        }
        $manifest.Save((Join-Path $destination 'case.xml'))
        $raw=[IO.File]::ReadAllText($report)
        if($case.Flatten) {
            $parsed=[xml]$raw
            $rows=@($parsed.SelectNodes('//filecomp'))
            if($rows.Count -ne $case.FlatRows -or $parsed.SelectNodes('/bcreport/foldercomp/foldercomp').Count -ne 0) {throw "Named session did not produce a flat paired report using $FlattenValue"}
        }
        $normalized=$raw.Replace((Join-Path $caseRoot 'left'),'LEFT_ROOT').Replace((Join-Path $caseRoot 'right'),'RIGHT_ROOT')
        [IO.File]::WriteAllText((Join-Path $destination 'expected.xml'),$normalized,$utf8)
        [IO.File]::WriteAllText((Join-Path $destination 'reference.sha256'),(Get-FileHash -LiteralPath $report -Algorithm SHA256).Hash.ToLowerInvariant(),$utf8)
        Write-Output "golden.dir.case=$($case.Name); exit=$($process.ExitCode)"
    }
    $provenance=@{
        reference_version=[Diagnostics.FileVersionInfo]::GetVersionInfo($ReferenceExe).FileVersion
        reference_sha256=(Get-FileHash -LiteralPath $ReferenceExe -Algorithm SHA256).Hash.ToLowerInvariant()
        generator_sha256=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
        invocation='solo + silent script; original reports normalized only for base paths'
        captured_cases=$cases.Count
        captured_utc=[DateTime]::UtcNow.ToString('o')
    }
    if(!$CaseName -and !$Probe){[IO.File]::WriteAllText((Join-Path $OutputRoot 'provenance.json'),($provenance|ConvertTo-Json),$utf8)}
    Write-Output "golden.dir.cases=$($cases.Count)"
} finally {
    if($null -ne $process -and !$process.HasExited){$process.Kill();$process.WaitForExit()}
    if($sessionChanged){
        if(Get-Process BCompare -ErrorAction SilentlyContinue){throw "Unexpected reference process is live; retain lock and backup at $work for recovery"}
        [IO.File]::WriteAllBytes($sessionPath,$sessionOriginal)
        [IO.File]::SetLastWriteTimeUtc($sessionPath,$sessionTime)
        $restored=[IO.File]::ReadAllBytes($sessionPath)
        if([Convert]::ToBase64String($restored) -ne [Convert]::ToBase64String($sessionOriginal)){throw 'Session byte restoration failed; retain lock and backup for recovery'}
        Write-Output 'golden.dir.settings_restored=true'
    }
    if (Test-Path -LiteralPath $work) {
        Get-ChildItem -LiteralPath $work -Recurse -Force -File | ForEach-Object { [IO.File]::SetAttributes($_.FullName,[IO.FileAttributes]::Normal) }
        Remove-Item -LiteralPath $work -Recurse -Force
    }
    if ($null -ne $lock) { $lock.Dispose(); Remove-Item -LiteralPath $lockPath -Force }
}
