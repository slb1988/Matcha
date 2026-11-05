<# Build verified ordinary TTextRules properties from the corpus metadata.
The native names and ElementSigs item shape were checked with RTTI and positive/
negative report probes. No installation or user-settings files are accessed here.
#>
[CmdletBinding()]
param(
    [string]$CorpusRoot='',
    [string]$OutputDirectory=''
)
$ErrorActionPreference='Stop'
$scriptRoot=Split-Path -Parent $MyInvocation.MyCommand.Path
if(!$CorpusRoot){$CorpusRoot=Join-Path $scriptRoot '../../crates/bc-core/tests/golden/text/2way'}
if(!$OutputDirectory){$OutputDirectory=Join-Path $scriptRoot 'profiles'}
$utf8=New-Object Text.UTF8Encoding($false,$true)
$null=New-Item -ItemType Directory -Path $OutputDirectory -Force
$elements=@('Comment','Directive','EnvironmentVar','Identifier','Indent','Keyword','LineNum','Number','Operator','String')
foreach($case in (Get-ChildItem -LiteralPath $CorpusRoot -Directory | Sort-Object Name)) {
    $metadata=Join-Path $case.FullName 'case.properties'
    if(!(Test-Path -LiteralPath $metadata -PathType Leaf)){continue}
    $props=@{}
    foreach($line in [IO.File]::ReadAllLines($metadata,$utf8)) {
        $line=$line.Trim();if(!$line -or $line.StartsWith('#')){continue}
        $separator=$line.IndexOf('=')
        if($separator -le 0){throw "Invalid case property: $line"}
        $key=$line.Substring(0,$separator)
        if($props.ContainsKey($key)){throw "Duplicate case property: $key"}
        $props[$key]=$line.Substring($separator+1)
    }
    if($props.ContainsKey('native_rules')){throw 'Use explicit ComparisonTemplates for a custom native_rules override'}
    function Flag([string]$key) {
        if(!$props.ContainsKey($key)){return $false}
        if($props[$key] -cnotin @('true','false')){throw "Invalid boolean $key"}
        return $props[$key] -ceq 'true'
    }
    function Bool([bool]$value) {if($value){'True'}else{'False'}}
    $fields=[ordered]@{
        CaseSig=!(Flag 'case_unimportant')
        CompareEndings=(Flag 'compare_endings')
        LeadWSSig=!(Flag 'leading_unimportant')
        EmbeddedWSSig=!(Flag 'embedded_unimportant')
        TrailWSSig=!(Flag 'trailing_unimportant')
        NonWSSig=$true
        OrphansAlwaysSig=$false
    }
    $lines=New-Object 'Collections.Generic.List[string]'
    $lines.Add('<Rules Value="TTextRules">')
    foreach($name in $fields.Keys){$lines.Add(('  <{0} Value="{1}"/>' -f $name,(Bool $fields[$name])))}
    $lines.Add('  <ElementSigs>')
    for($i=0;$i -lt $elements.Count;$i++){
        $significant=!($elements[$i] -eq 'Comment' -and (Flag 'comment_unimportant'))
        $lines.Add(('    <_{0}>' -f $i))
        $lines.Add(('      <ElementStr Value="{0}"/>' -f $elements[$i]))
        $lines.Add(('      <ValueBool Value="{0}"/>' -f (Bool $significant)))
        $lines.Add(('    </_{0}>' -f $i))
    }
    $lines.Add('  </ElementSigs>')
    $lines.Add('</Rules>')
    [IO.File]::WriteAllText((Join-Path $OutputDirectory ($case.Name+'.rules.xml')),($lines -join "`n")+"`n",$utf8)
}
