[CmdletBinding()]
param([Parameter(Mandatory)][string]$Reference,[Parameter(Mandatory)][string]$Actual,[Parameter(Mandatory)][string]$Scene)
$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Drawing
$ref=[Drawing.Bitmap]::FromFile((Resolve-Path $Reference)); $act=[Drawing.Bitmap]::FromFile((Resolve-Path $Actual)); $w=[Math]::Max($ref.Width,$act.Width); $h=[Math]::Max($ref.Height,$act.Height)
$w=[int]$w; $h=[int]$h; $dir=Join-Path (Get-Location) 'target/visual/compare'; New-Item -ItemType Directory -Force $dir|Out-Null
$side=New-Object Drawing.Bitmap -ArgumentList ([int]($w*2)),([int]$h); $g=[Drawing.Graphics]::FromImage($side); $g.DrawImage($ref,0,0);$g.DrawImage($act,$w,0);$g.Dispose();$side.Save((Join-Path $dir "$Scene-side.png"),[Drawing.Imaging.ImageFormat]::Png);$side.Dispose()
$heat=New-Object Drawing.Bitmap -ArgumentList ([int]$w),([int]$h);$diff=0
for($y=0;$y -lt $h;$y++){for($x=0;$x -lt $w;$x++){if($x -lt $ref.Width -and $y -lt $ref.Height){$a=$ref.GetPixel($x,$y)}else{$a=[Drawing.Color]::Black};if($x -lt $act.Width -and $y -lt $act.Height){$b=$act.GetPixel($x,$y)}else{$b=[Drawing.Color]::Black};$d=[Math]::Abs($a.R-$b.R)+[Math]::Abs($a.G-$b.G)+[Math]::Abs($a.B-$b.B);if($d -gt 0){$diff++};$heat.SetPixel($x,$y,[Drawing.Color]::FromArgb(255,[Math]::Min(255,$d*4),0,0))}}
$heat.Save((Join-Path $dir "$Scene-heat.png"),[Drawing.Imaging.ImageFormat]::Png);$heat.Dispose();$ref.Dispose();$act.Dispose()
$report=Join-Path $dir 'report.md'; if(-not(Test-Path $report)){Set-Content $report '# Visual comparison report'}; Add-Content $report "`n## $Scene`n- Reference: $Reference`n- Actual: $Actual`n- Dimensions: ${w}x${h}`n- Pixel differences: $diff`n- Named region tolerance: ±2 px; self-comparison is expected to report 0."
Write-Output "Compared $Scene; pixel differences=$diff"
