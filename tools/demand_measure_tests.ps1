# Validation for the demand measurement tools (no performance measurement):
#  1. both measurement scripts must parse without syntax errors;
#  2. Get-Med (AST-extracted from the SHIPPED scripts, both copies) must pass
#     empty/odd/even/decimal/unsorted/single cases plus the two real GUI ready
#     triplets from batch-20261107-161551;
#  3. GUI-script gate helpers (stable-window selection on the sampler clock,
#     minimum-sample rule) behave as specified;
#  4. static guards: no ready*1000 unit conversion, exit_code/shutdown_method
#     recorded, no unbounded WaitForExit(), no force Stop-Process.
# Exits non-zero on any failure. Not a performance experiment; writes nothing
# to any baseline/history file.
$ErrorActionPreference = 'Stop'
$Root = 'D:\Github\Matcha'
$Gui   = "$Root\tools\demand_gui_measure.ps1"
$Proto = "$Root\tools\demand_proto_measure.ps1"
$failed = @()
function Check([string]$Name, [bool]$Ok, [string]$Detail = '') {
    if ($Ok) { Write-Host "PASS $Name" } else { Write-Host "FAIL $Name $Detail"; $script:failed += $Name }
}
function Get-FunctionScriptBlock([string]$Path, [string]$Name) {
    $tokens = $null; $errors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    if ($errors.Count -gt 0) { throw "parse errors in ${Path}: $($errors[0].Message)" }
    $fn = $ast.Find({ param($a) $a -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $a.Name -eq $Name }, $true)
    if ($null -eq $fn) { throw "function $Name not found in $Path" }
    # Parenthesized parameters live on FunctionDefinitionAst.Parameters, not in
    # Body; define the full function text in-session and take its ScriptBlock.
    $marker = "__extracted_${Name}"
    $text = $fn.Extent.Text -replace "function\s+$Name", "function global:$marker"
    Invoke-Expression $text
    $sb = (Get-Item "function:\$marker").ScriptBlock
    Remove-Item "function:\$marker"
    return $sb
}

# 1. syntax
foreach ($f in @($Gui, $Proto)) {
    $tokens = $null; $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($f, [ref]$tokens, [ref]$errors) | Out-Null
    Check "syntax $(Split-Path $f -Leaf)" ($errors.Count -eq 0) (($errors | ForEach-Object Message) -join '; ')
}

# 2. Get-Med from both shipped scripts
$medGui   = Get-FunctionScriptBlock $Gui   'Get-Med'
$medProto = Get-FunctionScriptBlock $Proto 'Get-Med'
$cases = @(
    @{ in = @();                    want = $null  ; name = 'empty' },
    @{ in = @(3, 1, 2);             want = 2.0    ; name = 'odd-unsorted' },
    @{ in = @(5);                   want = 5.0    ; name = 'single' },
    @{ in = @(4, 1, 3, 2);          want = 2.5    ; name = 'even' },
    @{ in = @(0.4, 0.1, 0.3, 0.2);  want = 0.25   ; name = 'even-decimal' },
    @{ in = @(1.5, 1.5, 2.5);       want = 1.5    ; name = 'odd-decimal-keeps-fraction' },
    @{ in = @(4839.1, 4965.8, 4879.8); want = 4879.8; name = 'real-dense-triplet' },
    @{ in = @(7112.1, 7093.1, 7079.3); want = 7093.1; name = 'real-lazy-triplet' }
)
foreach ($case in $cases) {
    $g = & $medGui ($case.in)
    $pr = & $medProto ($case.in)
    if ($null -eq $case.want) {
        Check "Get-Med gui $($case.name)" ($null -eq $g) "got $g"
        Check "Get-Med proto $($case.name)" ($null -eq $pr) "got $pr"
    } else {
        Check "Get-Med gui $($case.name)" ([math]::Abs($g - $case.want) -lt 1e-9) "got $g want $($case.want)"
        Check "Get-Med proto $($case.name)" ([math]::Abs($pr - $case.want) -lt 1e-9) "got $pr want $($case.want)"
    }
}

# 3. gate helpers (GUI script)
$selectStable = Get-FunctionScriptBlock $Gui 'Get-StableWindowSamples'
$minSamples   = Get-FunctionScriptBlock $Gui 'Get-StableMinSamples'
$mk = { param($t) [pscustomobject]@{ t_ms = $t; PrivateBytes = [int64]1; WorkingSet = [int64]1; WorkingSetPrivate = [int64]1; WorkingSetPeak = [int64]1 } }
$samples = @((& $mk 100), (& $mk 500), (& $mk 800), (& $mk 1100), (& $mk 1400))
# sampler-clock window [500, 1400): picks exactly t=500,800,1100
$sel = & $selectStable $samples 500 1400
Check 'stable-window sampler-clock selection' (($sel.Count -eq 3) -and ($sel[0].t_ms -eq 500) -and ($sel[2].t_ms -eq 1100)) "count=$($sel.Count)"
$selAll = & $selectStable $samples 0 2000
Check 'stable-window full range' ($selAll.Count -eq 5) "count=$($selAll.Count)"
Check 'min-samples 12s -> 20' ((& $minSamples 12) -eq 20) "got $(& $minSamples 12)"
Check 'min-samples 6s -> 10' ((& $minSamples 6) -eq 10) "got $(& $minSamples 6)"
Check 'min-samples tiny floor 2' ((& $minSamples 0.1) -eq 2) "got $(& $minSamples 0.1)"

# 4. static guards on the shipped GUI script
$guiText = [IO.File]::ReadAllText($Gui)
Check 'no ready*1000 unit conversion' (-not [regex]::IsMatch($guiText, 'ready_ms\s*\*\s*1000')) ''
Check 'ready medians aggregated in ms' ($guiText.Contains('ready_ms_dense_median = Get-Med ($dense | ForEach-Object { $_.ready_ms })')) ''
Check 'records real exit_code' ($guiText.Contains('exit_code = $exitCode')) ''
Check 'records shutdown_method' ($guiText.Contains('shutdown_method = $shutdownMethod')) ''
Check 'no unbounded WaitForExit()' (-not [regex]::IsMatch($guiText, 'WaitForExit\(\)')) ''
Check 'no force Stop-Process' (-not [regex]::IsMatch($guiText, 'Stop-Process\s+-Id')) ''
Check 'identity uses exact start ticks' ($guiText.Contains('StartTime.ToUniversalTime().Ticks')) ''
$protoText = [IO.File]::ReadAllText($Proto)
Check 'proto Get-Med fixed in place' ($protoText.Contains('[math]::Floor($arr.Count / 2)')) ''
Check 'identity pins handle via .Handle access' ([regex]::IsMatch($guiText, '\[void\]\$Process\.Handle')) ''

# 5. Set-MeasuredProcessEnv (AST-extracted from the shipped script): with a
# polluted PARENT environment, the dense child config must contain neither the
# lazy switch nor PERF_EXIT, and the lazy child only the lazy switch. Object
# construction only; no process is started.
$setEnv = Get-FunctionScriptBlock $Gui 'Set-MeasuredProcessEnv'
$env:MATCHA_TABLE_CSV_LAZY = '1'
$env:MATCHA_TABLE_PERF_EXIT = '1'
try {
    $psiDense = [System.Diagnostics.ProcessStartInfo]::new()
    & $setEnv $psiDense $false
    Check 'dense child: no lazy var' (-not $psiDense.EnvironmentVariables.ContainsKey('MATCHA_TABLE_CSV_LAZY')) ''
    Check 'dense child: no perf-exit var' (-not $psiDense.EnvironmentVariables.ContainsKey('MATCHA_TABLE_PERF_EXIT')) ''
    Check 'dense child: perf on' ($psiDense.EnvironmentVariables['MATCHA_TABLE_PERF'] -eq '1') ''
    $psiLazy = [System.Diagnostics.ProcessStartInfo]::new()
    & $setEnv $psiLazy $true
    Check 'lazy child: lazy var set' ($psiLazy.EnvironmentVariables['MATCHA_TABLE_CSV_LAZY'] -eq '1') ''
    Check 'lazy child: no perf-exit var' (-not $psiLazy.EnvironmentVariables.ContainsKey('MATCHA_TABLE_PERF_EXIT')) ''
    Check 'lazy child: perf on' ($psiLazy.EnvironmentVariables['MATCHA_TABLE_PERF'] -eq '1') ''
} finally {
    Remove-Item Env:MATCHA_TABLE_CSV_LAZY -ErrorAction SilentlyContinue
    Remove-Item Env:MATCHA_TABLE_PERF_EXIT -ErrorAction SilentlyContinue
}

if ($failed.Count -gt 0) { Write-Host "FAILED: $($failed -join ', ')"; exit 1 }
Write-Host "ALL TESTS PASSED"
exit 0
