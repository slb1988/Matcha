[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$ReferenceExe,
 [string]$WorkerId='table-golden',
 [int]$LockTimeoutSeconds=30,
 [string[]]$CaseNames=@(),
 [string]$SessionsPath=(Join-Path $env:APPDATA 'Scooter Software/Beyond Compare 5/BCSessions.xml'),
 [string]$CorpusRoot=''
)
$ErrorActionPreference='Stop'
if(!$CorpusRoot){$CorpusRoot=Join-Path $PSScriptRoot '../../crates/bc-core/tests/golden/table'}
. (Join-Path $PSScriptRoot 'reference_lock.ps1')
$lease=Enter-MatchaReferenceLock -WorkerId $WorkerId -TimeoutSeconds $LockTimeoutSeconds
try {
 if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw 'Close reference process before generating table reports'}
 $utf8=New-Object Text.UTF8Encoding($false,$true)
 $cases=@(Get-ChildItem -LiteralPath $CorpusRoot -Directory | Where-Object { (Test-Path -LiteralPath (Join-Path $_.FullName 'case.json')) -and (!$CaseNames.Count -or $CaseNames -contains $_.Name) } | Sort-Object Name)
 if(!$cases.Count){throw 'No table cases selected'}
 $backup=Join-Path $PSScriptRoot ('backup/table-'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
 $null=New-Item -ItemType Directory -Path $backup -Force
 $original=[IO.File]::ReadAllBytes($SessionsPath)
 [IO.File]::WriteAllBytes((Join-Path $backup 'BCSessions.xml'),$original)
 $base=[Text.Encoding]::UTF8.GetString($original)
 if($base.Contains('matcha-golden-table')){throw 'Dedicated table session name collision'}
 $close=$base.LastIndexOf('</BCSessions>');if($close -lt 0){throw 'Unexpected sessions root'}
 $work=Join-Path ([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))) 'target/table-golden-work'
 $null=New-Item -ItemType Directory -Path $work -Force
 function EscapeXml([string]$s){[Security.SecurityElement]::Escape($s)}
 function Value([string]$name,$v) {
   if($v -is [bool]){$v=if($v){'True'}else{'False'}}
   elseif($v -is [double]){$v=$v.ToString([Globalization.CultureInfo]::InvariantCulture)}
   return '<'+$name+' Value="'+(EscapeXml ([string]$v))+'"/>'
 }
 $owned=$null
 try {
  foreach($case in $cases) {
   if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw 'Another reference process is running'}
   $p=[IO.File]::ReadAllText((Join-Path $case.FullName 'case.json'),$utf8)|ConvertFrom-Json
   $dir=Join-Path $work $case.Name;$null=New-Item -ItemType Directory -Path $dir -Force
   $left=Join-Path $case.FullName 'left.csv';$right=Join-Path $case.FullName 'right.csv'
   if($p.xlsx) {
    foreach($side in @('left','right')){
     $path=Join-Path $dir ($side+'.xlsx')
     $arguments=@((Join-Path $PSScriptRoot 'gen_table_cases.py'),'--xlsx',(Join-Path $case.FullName ($side+'.csv')),$path)
     if($p.multiple){$arguments+='--multiple'}
     & python @arguments;if($LASTEXITCODE -ne 0){throw 'OOXML generation failed'}
     if($side -eq 'left'){$left=$path}else{$right=$path}
    }
   }
   $align=switch($p.strategy){'None'{'alignNone'};'Patience'{'alignPatience'};'Smart'{'alignSmart'};default{'alignQuick'}}
   $rules=(Value 'AlignKind' $align)+(Value 'NeverAlignDiffs' $true)+(Value 'UseCloseness' $false)+(Value 'SkewTolerance' 2000)
   # Default and sheet-specific structures verified by the targeted serializer inspection.
   $sheetIds=if($p.multiple){@(0,1)}else{@(0)}
   $rules+='<AlignDef>'
   foreach($sheetId in $sheetIds){$rules+='<_'+$sheetId+'>'+(Value 'Kind' $align)+(Value 'NeverAlignDiffs' $true)+(Value 'UseCloseness' $false)+(Value 'SkewTolerance' 2000)+'</_'+$sheetId+'>'}
   $rules+='</AlignDef>'
   $rules+=(Value 'AlignSort' $(if($p.sort){$sheetIds -join ';'}else{''}))
   $rules+='<ColKeys>'
   foreach($sheetId in $sheetIds){$rules+='<_'+$sheetId+' Value="'+(($p.keys|ForEach-Object{[string]$_}) -join ';')+'"/>'}
   $rules+='</ColKeys>'
   foreach($name in @('AsText','Insig','StrCaseInsig','StrWSInsig','NumToleranceRelative')){$rules+=Value ('Col'+$name) $false}
   $rules+=Value 'ColNumTolerance' 0
   $rules+=Value 'ColDateTimeTolerance' 0
   if($p.handling.PSObject.Properties.Count -gt 0){
    $rules+='<ColHandling>'
    foreach($sheetId in $sheetIds){$rules+='<_'+$sheetId+'>'
    foreach($column in $p.handling.PSObject.Properties){
     $rules+='<_'+$column.Name+'>'
     foreach($property in $column.Value.PSObject.Properties){$rules+=Value $property.Name $property.Value}
     $rules+='</_'+$column.Name+'>'
    }
    $rules+='</_'+$sheetId+'>'}
    $rules+='</ColHandling>'
   }
   $node='<TTableCompareSession Value="matcha-golden-table"><Specs><Left Value="'+(EscapeXml $left)+'"/><Right Value="'+(EscapeXml $right)+'"/></Specs><Rules Value="TTableRules">'+$rules+'</Rules></TTableCompareSession>'
   [IO.File]::WriteAllText($SessionsPath,$base.Insert($close,$node),$utf8)
   $report=Join-Path $dir 'expected.html';$log=Join-Path $dir 'generation.log';$script=Join-Path $dir 'report.txt'
   $commands='log verbose "'+$log+'"'+"`r`n"+'load <default>'+"`r`n"+'table-report layout:side-by-side options:display-all output-to:"'+$report+'" output-options:html-color "matcha-golden-table"'+"`r`n"
   [IO.File]::WriteAllText($script,$commands,$utf8)
   $owned=Start-Process -FilePath $ReferenceExe -ArgumentList @(('"@'+$script+'"'),'/silent','/solo') -PassThru
   if(!$owned.WaitForExit(60000)){$owned.Kill();$owned.WaitForExit();throw 'Owned table report process timed out'}
   $exit=$owned.ExitCode;$owned=$null;if($exit -ne 0){throw "Table report failed ($exit): $log"}
   if(!(Test-Path -LiteralPath $report)){throw "Missing report $report"}
   # Native multiline cells can contain NULs. A reversible JSON string keeps
   # those exact HTML bytes without committing a binary-classified fixture.
   $rawPath=Join-Path $case.FullName 'expected.html'
   $jsonPath=Join-Path $case.FullName 'expected.html.json'
   $html=[IO.File]::ReadAllText($report,$utf8)
   if($html.IndexOf([char]0) -ge 0) {
    [IO.File]::WriteAllText($jsonPath,(ConvertTo-Json -InputObject $html -Compress)+"`n",$utf8)
    if(Test-Path -LiteralPath $rawPath){Remove-Item -LiteralPath $rawPath -Force}
   } else {
    Copy-Item -LiteralPath $report -Destination $rawPath -Force
    if(Test-Path -LiteralPath $jsonPath){Remove-Item -LiteralPath $jsonPath -Force}
   }
   [IO.File]::WriteAllText((Join-Path $case.FullName 'native-rules.xml'),'<Rules Value="TTableRules">'+$rules+'</Rules>'+"`n",$utf8)
   Write-Output "generated $($case.Name)"
  }
 } finally {
  if($owned -and !$owned.HasExited){$owned.Kill();$owned.WaitForExit()}
  if(Get-Process -Name BCompare -ErrorAction SilentlyContinue){throw "Cannot restore while external reference process runs; backup: $backup"}
  [IO.File]::WriteAllBytes($SessionsPath,$original)
  if([Convert]::ToBase64String([IO.File]::ReadAllBytes($SessionsPath)) -cne [Convert]::ToBase64String($original)){throw 'Session restore mismatch'}
  Write-Output "table sessions restored and byte-verified; backup: $backup"
 }
} finally {Exit-MatchaReferenceLock -Lease $lease}
