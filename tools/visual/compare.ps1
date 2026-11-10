[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Reference,
    [Parameter(Mandatory)][string]$Actual,
    [Parameter(Mandatory)][string]$Scene,
    [string]$Tokens = (Join-Path $PSScriptRoot '../../docs/visual-parity/tokens.json'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../../target/visual/compare'),
    [ValidateRange(3,30)][int]$SearchRadius = 8
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$tokensData = Get-Content -LiteralPath $Tokens -Raw | ConvertFrom-Json -AsHashtable
if (-not $tokensData.scenes.ContainsKey($Scene)) { throw "tokens 中没有场景 $Scene" }
$ref = [Drawing.Bitmap]::FromFile((Resolve-Path -LiteralPath $Reference))
$act = [Drawing.Bitmap]::FromFile((Resolve-Path -LiteralPath $Actual))
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path

function Get-ColorDistance($A, $B) {
    return [Math]::Abs([int]$A.R-$B.R) + [Math]::Abs([int]$A.G-$B.G) + [Math]::Abs([int]$A.B-$B.B)
}

function Find-RegionShift([int[]]$Rectangle) {
    # 扫描矩形周边的背景/分隔线色带，而非匹配文字或整张图片。
    # 每边前后 2 px 的法向剖面能区分同色大平面与真实分隔线。
    # 在 ±SearchRadius 内联合搜索 x/y；平分时优先最小位移，避免平坦边界虚报。
    $points = [Collections.Generic.List[object]]::new()
    $x0,$y0,$width,$height = $Rectangle
    foreach ($offset in @(-2,-1,0,1,2)) {
        $stepX=[Math]::Max(1,[int]($width/48))
        $stepY=[Math]::Max(1,[int]($height/48))
        for($x=$x0;$x -lt ($x0+$width);$x+=$stepX) {
            foreach($y in @(($y0+$offset),($y0+$height-1+$offset))) {
                if($x -ge 0 -and $y -ge 0 -and $x -lt $ref.Width -and $y -lt $ref.Height) {
                    $color=$ref.GetPixel($x,$y)
                    if(($color.R -eq $color.G -and $color.G -eq $color.B) -or $color.GetBrightness() -gt .85) {
                        $points.Add(@($x,$y,$color))
                    }
                }
            }
        }
        for($y=$y0;$y -lt ($y0+$height);$y+=$stepY) {
            foreach($x in @(($x0+$offset),($x0+$width-1+$offset))) {
                if($x -ge 0 -and $y -ge 0 -and $x -lt $ref.Width -and $y -lt $ref.Height) {
                    $color=$ref.GetPixel($x,$y)
                    if(($color.R -eq $color.G -and $color.G -eq $color.B) -or $color.GetBrightness() -gt .85) {
                        $points.Add(@($x,$y,$color))
                    }
                }
            }
        }
    }
    $best=[double]::PositiveInfinity
    $bestX=0; $bestY=0; $ties=0
    for($dy=-$SearchRadius;$dy -le $SearchRadius;$dy++) {
        for($dx=-$SearchRadius;$dx -le $SearchRadius;$dx++) {
            [double]$sum=0; $valid=0
            foreach($point in $points) {
                $x=$point[0]+$dx; $y=$point[1]+$dy
                if($x -ge 0 -and $y -ge 0 -and $x -lt $act.Width -and $y -lt $act.Height) {
                    $c=$act.GetPixel($x,$y); $r=$point[2]
                    $sum += [Math]::Abs([int]$r.R-$c.R)+[Math]::Abs([int]$r.G-$c.G)+[Math]::Abs([int]$r.B-$c.B)
                    $valid++
                }
            }
            if($valid -lt ($points.Count*.8)) { continue }
            $score=$sum/[Math]::Max(1,$valid)
            if($score -lt ($best-.000001)) { $best=$score; $bestX=$dx; $bestY=$dy; $ties=1 }
            elseif([Math]::Abs($score-$best) -lt .000001) {
                $ties++
                if(([Math]::Abs($dx)+[Math]::Abs($dy)) -lt ([Math]::Abs($bestX)+[Math]::Abs($bestY))) { $bestX=$dx; $bestY=$dy }
            }
        }
    }
    return @{dx=$bestX;dy=$bestY;score=[Math]::Round($best,3);samples=$points.Count;ambiguous=($ties -gt 1)}
}

function Find-Boundary([int[]]$Rect, [string]$Edge, [int]$TangentShift, [int]$Preferred) {
    $vertical=$Edge -in @('left','right')
    $line=switch($Edge){left{$Rect[0]} right{$Rect[0]+$Rect[2]-1} top{$Rect[1]} bottom{$Rect[1]+$Rect[3]-1}}
    $start=if($vertical){$Rect[1]}else{$Rect[0]}
    $length=if($vertical){$Rect[3]}else{$Rect[2]}
    $step=[Math]::Max(1,[int]($length/100))
    $best=[double]::PositiveInfinity
    $chosen=$Preferred
    for($delta=-$SearchRadius;$delta -le $SearchRadius;$delta++) {
        [double]$score=0; $count=0
        for($t=$start;$t -lt ($start+$length);$t+=$step) {
            foreach($normal in @(-2,-1,0,1,2)) {
                $rx=if($vertical){$line+$normal}else{$t}
                $ry=if($vertical){$t}else{$line+$normal}
                $ax=if($vertical){$rx+$delta}else{$rx+$TangentShift}
                $ay=if($vertical){$ry+$TangentShift}else{$ry+$delta}
                if($rx -lt 0 -or $ry -lt 0 -or $rx -ge $ref.Width -or $ry -ge $ref.Height -or $ax -lt 0 -or $ay -lt 0 -or $ax -ge $act.Width -or $ay -ge $act.Height){continue}
                $r=$ref.GetPixel($rx,$ry)
                if(-not (($r.R -eq $r.G -and $r.G -eq $r.B) -or $r.GetBrightness() -gt .85)){continue}
                $a=$act.GetPixel($ax,$ay)
                $score += [Math]::Abs([int]$r.R-$a.R)+[Math]::Abs([int]$r.G-$a.G)+[Math]::Abs([int]$r.B-$a.B)
                $count++
            }
        }
        if($count -eq 0){continue}
        $score/=$count
        if($score -lt ($best-.000001) -or ([Math]::Abs($score-$best) -lt .000001 -and [Math]::Abs($delta-$Preferred) -lt [Math]::Abs($chosen-$Preferred))) {
            $best=$score; $chosen=$delta
        }
    }
    return $chosen
}

try {
    $width=[Math]::Max($ref.Width,$act.Width)
    $height=[Math]::Max($ref.Height,$act.Height)
    $side=[Drawing.Bitmap]::new(($width*2),$height)
    $g=[Drawing.Graphics]::FromImage($side)
    try { $g.Clear([Drawing.Color]::White); $g.DrawImageUnscaled($ref,0,0); $g.DrawImageUnscaled($act,$width,0) }
    finally { $g.Dispose() }
    $side.Save((Join-Path $OutputDirectory "$Scene-side.png"),[Drawing.Imaging.ImageFormat]::Png)
    $side.Dispose()
    $heat=[Drawing.Bitmap]::new($width,$height)
    $different=0
    for($y=0;$y -lt $height;$y++) {
        for($x=0;$x -lt $width;$x++) {
            $a=if($x -lt $ref.Width -and $y -lt $ref.Height){$ref.GetPixel($x,$y)}else{[Drawing.Color]::Black}
            $b=if($x -lt $act.Width -and $y -lt $act.Height){$act.GetPixel($x,$y)}else{[Drawing.Color]::Black}
            $delta=[Math]::Abs([int]$a.R-$b.R)+[Math]::Abs([int]$a.G-$b.G)+[Math]::Abs([int]$a.B-$b.B)
            if($delta -gt 0){$different++}
            $heat.SetPixel($x,$y,[Drawing.Color]::FromArgb([Math]::Min(255,$delta*4),0,0))
        }
    }
    $heat.Save((Join-Path $OutputDirectory "$Scene-heat.png"),[Drawing.Imaging.ImageFormat]::Png)
    $heat.Dispose()
    $rows=[Collections.Generic.List[object]]::new()
    foreach($entry in $tokensData.scenes[$Scene].regions.GetEnumerator() | Sort-Object Key) {
        [int[]]$rect=$entry.Value
        $found=Find-RegionShift $rect
        $left=Find-Boundary $rect 'left' $found.dy $found.dx
        $right=Find-Boundary $rect 'right' $found.dy $found.dx
        $top=Find-Boundary $rect 'top' $found.dx $found.dy
        $bottom=Find-Boundary $rect 'bottom' $found.dx $found.dy
        $edges=@($left,$top,$right,$bottom)
        $maximum=($edges | ForEach-Object {[Math]::Abs($_)} | Measure-Object -Maximum).Maximum
        $measured=@(($rect[0]+$left),($rect[1]+$top),($rect[2]+$right-$left),($rect[3]+$bottom-$top))
        $rows.Add(@{region=$entry.Key;reference=@($rect);actual=$measured;dx=$left;dy=$top;edgeDeltas=$edges;maxDeviation=$maximum;score=$found.score;samples=$found.samples;ambiguous=$found.ambiguous;pass=($maximum -le 2)})
    }
    $colors=[Collections.Generic.List[object]]::new()
    foreach($entry in $tokensData.colors.GetEnumerator() | Sort-Object Key) {
        $token=$entry.Value
        if($token -isnot [System.Collections.IDictionary] -or $token.scene -ne $Scene) { continue }
        $x=$token.point[0]; $y=$token.point[1]
        if($x -ge $act.Width -or $y -ge $act.Height) { throw "颜色坐标越界: $($entry.Key)" }
        $expected=[Drawing.ColorTranslator]::FromHtml($token.hex)
        $observed=$act.GetPixel($x,$y)
        $maxDelta=[Math]::Max([Math]::Abs([int]$expected.R-$observed.R),[Math]::Max([Math]::Abs([int]$expected.G-$observed.G),[Math]::Abs([int]$expected.B-$observed.B)))
        $colors.Add(@{name=$entry.Key;expected=$token.hex;actual=('#{0:X2}{1:X2}{2:X2}' -f $observed.R,$observed.G,$observed.B);point=@($x,$y);delta=$maxDelta;pass=($maxDelta -le 2)})
    }
    $maxDeviation=0
    foreach($row in $rows){$maxDeviation=[Math]::Max($maxDeviation,$row.maxDeviation)}
    $result=@{scene=$Scene;reference=(Resolve-Path $Reference).Path;actual=(Resolve-Path $Actual).Path;pixelDifferences=$different;maxDeviation=$maxDeviation;regions=@($rows);colors=@($colors);dimensionMatch=($ref.Size -eq $act.Size)}
    $result | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $OutputDirectory "$Scene-result.json") -Encoding utf8
    $lines=[Collections.Generic.List[string]]::new()
    $lines.Add("# 视觉对照：$Scene")
    $lines.Add("参考：$Reference；实测：$Actual；差异像素：$different；最大区域位移：$maxDeviation px。")
    $lines.Add('方法：背景色/分隔线法向色带扫描，先局部定位，再独立检测左/上/右/下四条边（±8 px）；尺寸不同单独报告。ambiguous 表示平坦色带可能有多解，需要人工看并排图；本工具不将文字抗锯齿差异作为验收阈值。')
    $lines.Add("![并排]($Scene-side.png)`n![热图]($Scene-heat.png)")
    $lines.Add('|区域|参考 x,y,w,h|实测 x,y,w,h|左,上,右,下偏差|色差均值|样本数|多解|结果|')
    $lines.Add('|---|---|---|---|---|---|---|---|')
    foreach($row in $rows){
        $state=if($row.pass){'PASS'}else{'<span style="color:red">FAIL (>2 px)</span>'}
        $lines.Add("|$($row.region)|$($row.reference -join ',')|$($row.actual -join ',')|$($row.edgeDeltas -join ',')|$($row.score)|$($row.samples)|$($row.ambiguous)|$state|")
    }
    $lines.Add('## 颜色抽样（单通道容差 2）')
    $lines.Add('|token|坐标|参考|实测|最大通道差|通过|')
    $lines.Add('|---|---|---|---|---|---|')
    foreach($color in $colors){$lines.Add("|$($color.name)|$($color.point -join ',')|$($color.expected)|$($color.actual)|$($color.delta)|$($color.pass)|")}
    $lines | Set-Content (Join-Path $OutputDirectory 'report.md') -Encoding utf8
    Write-Output "Compared $Scene; pixel differences=$different; regions=$($rows.Count); max deviation=$maxDeviation; color samples=$($colors.Count)"
} finally { $ref.Dispose(); $act.Dispose() }
