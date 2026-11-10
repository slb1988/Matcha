[CmdletBinding()]
param(
    [string]$EvidenceRoot = (Join-Path $PSScriptRoot '../../docs/visual-parity'),
    [string]$HelpRoot = (Join-Path $env:TEMP 'matcha-bc5-help'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../../docs/visual-parity')
)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$EvidenceRoot=(Resolve-Path $EvidenceRoot).Path
$manifest=@(Get-Content (Join-Path $EvidenceRoot 'reference/capture-manifest.json') -Raw | ConvertFrom-Json -AsHashtable)
if($manifest.Count -ne 23){throw '必须先完成完整的 23 场景批次。'}
$bitmaps=@{}
foreach($scene in $manifest){$bitmaps[$scene.scene]=[Drawing.Bitmap]::FromFile((Join-Path $EvidenceRoot "reference/$($scene.scene).png"))}
$colors=[ordered]@{}
function Add-Color([string]$Name,[string]$Scene,[int[]]$Point,[string]$Meaning) {
    $c=$bitmaps[$Scene].GetPixel($Point[0],$Point[1])
    $colors[$Name]=@{hex=('#{0:X2}{1:X2}{2:X2}' -f $c.R,$c.G,$c.B);scene=$Scene;point=$Point;meaning=$Meaning}
}
function Find-Color([string]$Name,[string]$Scene,[int[]]$Rect,[string]$Hex,[string]$Meaning) {
    $expected=[Drawing.ColorTranslator]::FromHtml($Hex)
    for($y=$Rect[1];$y -lt ($Rect[1]+$Rect[3]);$y++) {
        for($x=$Rect[0];$x -lt ($Rect[0]+$Rect[2]);$x++) {
            if($bitmaps[$Scene].GetPixel($x,$y).ToArgb() -eq $expected.ToArgb()) {Add-Color $Name $Scene @($x,$y) $Meaning;return}
        }
    }
    throw "指定语义区域中无颜色 $Name $Hex；禁止猜测 token。"
}
try {
    Add-Color 'panel' 'text-basic' @(10,30) '共享面板底色'
    Add-Color 'menubar' 'text-basic' @(500,10) '菜单栏空白底色'
    Add-Color 'pane' 'text-basic' @(500,175) '文本相同行底色'
    Add-Color 'eof' 'text-basic' @(500,300) '文件结束后的面板底色'
    Add-Color 'gutter' 'text-basic' @(56,165) '行号与箭头槽底色'
    Add-Color 'minorBg' 'text-basic' @(500,146) '大小写不重要差异行底色'
    Add-Color 'diffRow' 'text-basic' @(1000,205) '右侧新增行底色'
    Add-Color 'separator' 'text-basic' @(400,123) '编辑窗格上边线'
    Add-Color 'toolbarToggle' 'text-basic' @(150,35) 'All 按钮选中填充'
    Add-Color 'toolbarToggleBorder' 'text-basic' @(145,35) 'All 选中边框'
    Add-Color 'statusbar' 'text-basic' @(1000,950) '全局状态栏底色'
    Add-Color 'pathbar' 'text-basic' @(620,85) '路径编辑框非选中底色'
    Add-Color 'pathSelection' 'text-basic' @(400,90) '路径文字选中底色'
    Add-Color 'paneStatus' 'text-basic' @(500,877) '每侧状态栏底色'
    Add-Color 'details' 'text-basic' @(500,909) '行详情底色'
    Find-Color 'textForeground' 'text-basic' @(92,124,150,15) '#000000' '非抗锯齿的普通字符核心像素'
    Find-Color 'minorForeground' 'text-basic' @(92,139,150,16) '#0000FF' '不重要差异字符核心像素'
    Find-Color 'diffForeground' 'text-basic' @(760,199,150,16) '#FC3030' '重要差异字符核心像素'
    Find-Color 'missingHatch' 'text-basic' @(92,199,150,30) '#E6E6E6' '缺行斜纹线'
    Find-Color 'missingBackground' 'text-basic' @(92,199,150,30) '#FFFFFF' '缺行斜纹底色'
    Find-Color 'lineNumber' 'text-basic' @(74,124,15,76) '#696969' '行号字符核心像素'
    Add-Color 'menuBackground' 'text-menu-session' @(240,80) '下拉菜单底色'
    Add-Color 'menuBorder' 'text-menu-session' @(2,100) '下拉菜单左边框（阴影不计）'
    Add-Color 'folderStripe' 'folder-basic' @(300,195) '文件夹树交替条纹'
    Add-Color 'folderSelection' 'folder-basic' @(300,174) '选中文件夹行底色'
    Find-Color 'folderOrphan' 'folder-expanded' @(60,248,70,14) '#6A00D5' '孤儿名称核心像素'
    Find-Color 'folderNewer' 'folder-expanded' @(718,429,82,13) '#FF0000' '较新版本名称核心像素'
    Find-Color 'folderOlder' 'folder-expanded' @(17,429,83,13) '#808080' '较旧版本名称核心像素'
    Find-Color 'folderSame' 'folder-expanded' @(60,369,80,13) '#000000' '相同文件名称核心像素'
    Find-Color 'folderDifferent' 'folder-expanded' @(40,189,80,13) '#FF0000' '不同文件名称核心像素'
    Add-Color 'tableDiff' 'table-basic' @(130,215) '表格差异单元格底色'
    Add-Color 'tableGrid' 'table-basic' @(132,175) '表格竖网格线'
    Add-Color 'homeTree' 'home' @(200,300) 'Home 会话树底色'
    Add-Color 'homeCanvas' 'home' @(500,300) 'Home 图标区域底色'

    $metrics=[ordered]@{}
    function Metric([string]$Name,$Value,[string]$Scene,[string]$Evidence) {
        $metrics[$Name]=@{value=$Value;unit='px';scene=$Scene;evidence=$Evidence}
    }
    Metric 'clientWidth' 1384 'text-basic' 'GetClientRect'
    Metric 'clientHeight' 961 'text-basic' 'GetClientRect'
    Metric 'menubarHeight' 27 'text-basic' 'y=0..26，工具栏从 27 开始'
    Metric 'menubarTextBaseline' 18 'text-basic' '菜单标签黑色核心像素最下行'
    Metric 'toolbarHeight' 43 'text-basic' 'y=27..69'
    Metric 'toolbarButtonWidth' 59 'text-basic' 'All 选中边框 x145..203；按钮按文字宽度变化'
    Metric 'toolbarButtonHeight' 39 'text-basic' 'All 边框 y29..67'
    Metric 'toolbarIconSize' @(16,16) 'text-basic' 'All 图标槽 x167..182,y32..47'
    Metric 'toolbarTextBaseline' 61 'text-basic' '按钮标签下缘，客户区绝对 y'
    Metric 'toolbarSeparatorWidth' 1 'text-basic' 'x140,y30..65'
    Metric 'toolbarSeparatorHeight' 36 'text-basic' 'y30..65'
    Metric 'toolbarPadding' @(3,2) 'text-basic' '按钮框到 toolbar 区域'
    Metric 'pathbarHeight' 26 'text-basic' 'y70..95'
    Metric 'infoHeight' 27 'text-basic' 'y96..122'
    Metric 'thumbnailWidth' 52 'text-basic' 'x0..51；实际缩略条在 x7..44'
    Metric 'thumbnailIndicatorWidth' 38 'text-basic' 'x7..44'
    Metric 'lineHeight' 15 'text-basic' '相邻行文本起点 y128,143,158'
    Metric 'lineNumberWidth' 20 'text-basic' 'x72..91'
    Metric 'arrowGutterWidth' 20 'text-basic' 'x52..71'
    Metric 'paneStatusHeight' 19 'text-basic' 'y868..886'
    Metric 'detailsHeight' 49 'text-basic' 'y887..935，含两行详情及留白'
    Metric 'detailsRowHeight' 16 'text-basic' '详情行 y887..902,903..918'
    Metric 'statusbarHeight' 25 'text-basic' 'y936..960'
    Metric 'statusFieldWidths' @(293,392,92,607) 'text-basic' '竖分隔 x293,685,777'
    Metric 'folderRowHeight' 20 'folder-basic' '行 y165,185,205'
    Metric 'folderHeaderHeight' 24 'folder-basic' 'y141..164'
    Metric 'folderColumnWidthsLeft' @(437,78,150) 'folder-basic' 'Name/Size/Modified：x0,437,515,665'
    Metric 'folderColumnWidthsRight' @(437,78,148) 'folder-basic' 'x702,1139,1217,1365'
    Metric 'folderPathbarHeight' 26 'folder-basic' 'y115..140'
    Metric 'folderFilterbarHeight' 45 'folder-basic' 'y70..114'
    Metric 'folderLogHeight' 106 'folder-basic' 'y830..935'
    Metric 'tableRowHeight' 18 'table-basic' '网格线 y151,169,187,205,223'
    Metric 'tableHeaderHeight' 28 'table-basic' 'y123..150'
    Metric 'tableColumnWidths' @(47,33,67) 'table-basic' '左表 x86,133,166,233'
    Metric 'tableNumberGutterWidth' 34 'table-basic' 'x52..85'
    Metric 'tableDetailsHeight' 81 'table-basic' 'y855..935'
    Metric 'homeSessionTreeWidth' 276 'home' '垂直分隔 x275，右区从276开始'
    Metric 'homeTreeRowHeight' 20 'home' 'New / Auto-saved 中心 y71,91'
    Metric 'homeIconGridCell' @(158,100) 'home' '中心 x676,834,992；图标与标签槽 y367..466'
    Metric 'homeIconSize' @(64,64) 'home' '图标槽约 x645..708,y371..434'
    Metric 'findbarHeight' 38 'text-find-bar' 'y70..107，新路径栏 y108'
    Metric 'menuItemHeight' 21 'text-menu-session' 'MSAA 项矩形高度'
    Metric 'menuSeparatorHeight' 10 'text-menu-session' 'MSAA role=21 矩形高度'

    $fontEvidence=@(Get-Content (Join-Path $EvidenceRoot 'supplemental/text-basic-fonts.json') -Raw | ConvertFrom-Json -AsHashtable)
    $fonts=[ordered]@{
        ui=@{family=$fontEvidence[0].family;sizePt=([Math]::Abs($fontEvidence[0].heightPx)*72/96);source='supplemental/text-basic-fonts.json';method='WM_GETFONT/GetObject，lfHeight=-12 @96 DPI';nativeEvidence=$fontEvidence}
        editor=@{family='Consolas';sizePt=10;sizePx=13.333333;source='supplemental/options-editor-font.png';method='Options > Appearance > File Views > Editor Font，读取后 Esc 取消'}
        lineNumber=@{family='Arial';sizePt=6.5;glyphHeightPx=7;confidence='offline-best-fit-not-native-font-query';source='supplemental/line-number-font-estimate.json';method='现有截图数字1–5与42组GDI候选字体离线比较，Arial 6.5pt误差最低；不是原生HFONT确认值'}
    }

    $inventories=[ordered]@{}
    $menuOrder=@{home=@('Session','View','Tools','Help');text=@('Session','File','Edit','Search','View','Tools','Help');folder=@('Session','Actions','Edit','Search','View','Tools','Help');sync=@('Session','Actions','Edit','Search','View','Tools','Help');table=@('Session','File','Edit','Search','View','Tools','Help')}
    $commandPage=@{home='commandshome.html';text='commandstext.html';folder='commandsdir.html';sync='commandsdirsync.html';table='commandstable.html'}
    $toolbarSpecs=@{
        text=@('Home|button|0','Sessions|split|0','All|toggle:on|1','Diffs|toggle|1','Same|toggle|1','Context|toggle:on|2','Minor|toggle|2','Rules|button|3','Format|split|3','Copy|button|4','Next Section|button|5','Prev Section|button|5','Swap|button|6','Reload|button|6')
        folder=@('Home|button|0','Sessions|split|0','All|toggle:on|1','Diffs|split|1','Same|toggle|1','Files|split:on|2','Minor|toggle|2','Rules|button|3','Copy|button|4','Expand|button|5','Collapse|button|5','Select|button|5','Files|button|6','Refresh|button|6','Swap|button|7','Stop|disabled|7','Filters|button|8','Peek|toggle|8')
        sync=@('Home|button|0','Sessions|split|0','Minor|toggle|1','Expand|button|2','Collapse|button|2','Select|button|2','Refresh|button|3','Stop|disabled|4','Peek|toggle|5')
        table=@('Home|button|0','Sessions|split|0','All|toggle:on|1','Diffs|toggle|1','Same|toggle|1','Minor|toggle|2','Rules|button|3','Format|split|3','Copy|button|4','Next Diff|button|5','Prev Diff|button|5','Swap|button|6','Reload|button|6')
        home=@('+|button|0','−|button|0','Search|edit|1')
    }
    function Get-ChmCommands([string]$File) {
        $html=Get-Content -LiteralPath (Join-Path $HelpRoot $File) -Raw
        foreach($match in [regex]::Matches($html,'<p class="p_Command">(.*?)</p>', 'Singleline')) {
            [Net.WebUtility]::HtmlDecode([regex]::Replace($match.Groups[1].Value,'<[^>]+>','')).Trim()
        }
    }
    function Flatten-Menu($Items) {
        foreach($item in $Items){if(-not $item.separator){$item.name};if($item.children.Count){Flatten-Menu $item.children}}
    }
    $crosscheck=[ordered]@{}
    foreach($view in @('home','text','folder','sync','table')) {
        $menus=@(foreach($name in $menuOrder[$view]) {
            Get-Content (Join-Path $EvidenceRoot "supplemental/$view-menu-$name.tree.json") -Raw | ConvertFrom-Json -AsHashtable
        })
        $toolbar=@(foreach($spec in $toolbarSpecs[$view]) {
            $parts=$spec.Split('|')
            @{name=$parts[0];type=$parts[1].Split(':')[0];checked=$parts[1].Contains(':on');enabled=($parts[1] -ne 'disabled');group=[int]$parts[2];sourceScene=@{home='home';text='text-basic';folder='folder-basic';sync='folder-sync';table='table-basic'}[$view]}
        })
        $status=switch($view){
            text{@('difference section count','line comparison status','Insert/Overwrite','Load time')}
            table{@('different row count','current cell status','Load time')}
            folder{@('left selected files/bytes','left free space','right selected files/bytes','right free space')}
            sync{@('left selected files/bytes','left free space','right selected files/bytes','right free space')}
            home{@()}
        }
        $context=if($view -in @('text','folder','table')){Get-Content (Join-Path $EvidenceRoot "supplemental/$view-context.tree.json") -Raw | ConvertFrom-Json -AsHashtable}else{$null}
        $inventories[$view]=@{menus=$menus;toolbar=$toolbar;status=$status;contextMenu=$context;source='真实弹出菜单截图 + MSAA；Explorer 子菜单委托 Windows Shell，按动态外部入口登记，不执行系统动作。'}
        $chm=@(Get-ChmCommands 'commandsbc.html')+@(Get-ChmCommands $commandPage[$view])
        $observed=@(foreach($menu in $menus){Flatten-Menu $menu.items})
        $normalize={param($s) ($s -replace '\.\.\.$','' -replace '\s*\*\s*$','').Trim()}
        $normalizedObserved=@($observed | ForEach-Object { & $normalize $_ })
        $normalizedChm=@($chm | ForEach-Object { & $normalize $_ })
        $crosscheck[$view]=@{page=$commandPage[$view];chmCommandCount=$chm.Count;observedMenuEntries=$observed.Count;chmOnly=@($chm | Where-Object {(& $normalize $_) -notin $normalizedObserved});uiOnly=@($observed | Where-Object {(& $normalize $_) -notin $normalizedChm});explanation='CHM 是命令目录而非默认可见菜单树：包括隐藏命令(*)、平台/版本/许可差异、合并的斜杠命令和 Source Control 条件项；实际可见树以逐项 MSAA/PNG 为准。所有未精确对应名称在两列显式列出，未虚构快捷键。'}
    }
    $inventories['findbar']=@{source='text-find-bar';controls=@('search icon','search combo','history dropdown','regular expression toggle','case-sensitive toggle','whole word toggle','wrap toggle','Prev','Next','expand/replace dropdown')}
    $textRegions=[ordered]@{menubar=@(0,0,1384,27);toolbar=@(0,27,1384,43);'pathbar.left'=@(52,70,666,26);'pathbar.right'=@(722,70,662,26);'info.left'=@(52,96,666,27);'info.right'=@(722,96,662,27);thumbnail=@(0,70,52,817);'gutter.left'=@(52,123,40,745);'gutter.right'=@(722,123,40,745);'pane.left'=@(92,123,608,745);'pane.right'=@(762,123,604,745);splitter=@(718,123,4,764);'paneStatus.left'=@(52,868,666,19);'paneStatus.right'=@(722,868,662,19);details=@(52,887,1332,49);statusbar=@(0,936,1384,25)}
    $folderRegions=[ordered]@{menubar=@(0,0,1384,27);toolbar=@(0,27,1384,43);filterbar=@(0,70,1384,45);'pathbar.left'=@(0,115,702,26);'pathbar.right'=@(702,115,682,26);'headers.left'=@(0,141,665,24);'headers.right'=@(702,141,664,24);'tree.left'=@(0,165,665,665);'tree.right'=@(702,165,664,665);splitter=@(665,141,37,689);log=@(0,830,1384,106);statusbar=@(0,936,1384,25)}
    $tableRegions=[ordered]@{menubar=@(0,0,1384,27);toolbar=@(0,27,1384,43);'pathbar.left'=@(52,70,666,26);'pathbar.right'=@(722,70,662,26);'info.left'=@(52,96,666,27);'info.right'=@(722,96,662,27);'headers.left'=@(52,123,181,28);'headers.right'=@(722,123,181,28);'grid.left'=@(52,151,648,685);'grid.right'=@(722,151,644,685);'paneStatus.left'=@(52,836,666,19);'paneStatus.right'=@(722,836,662,19);details=@(52,855,1332,81);statusbar=@(0,936,1384,25)}
    $scenes=[ordered]@{}
    foreach($entry in $manifest) {
        $name=$entry.scene
        $regions=[ordered]@{}
        $template=if($name -like 'folder-*'){$folderRegions}elseif($name -like 'table-*'){$tableRegions}else{$textRegions}
        foreach($key in $template.Keys){$regions[$key]=@($template[$key])}
        if($name -eq 'text-long-scrolled') {
            foreach($side in @('left','right')) {
                $regions["gutter.$side"][2]+=11
                $regions["pane.$side"][0]+=11
                $regions["pane.$side"][2]-=11
            }
        }
        if($name -eq 'home'){$regions=[ordered]@{menubar=@(0,0,1384,27);sessionTree=@(0,27,276,802);sessionToolbar=@(0,786,276,43);iconGrid=@(276,27,1108,802);supportPanel=@(0,829,1384,132)}}
        if($name -eq 'folder-sync'){$regions=[ordered]@{menubar=@(0,0,1384,27);toolbar=@(0,27,1384,43);paths=@(0,70,1384,40);operations=@(0,110,1384,191);'headers.left'=@(0,301,625,24);'headers.right'=@(742,301,624,24);'tree.left'=@(0,325,625,505);'tree.right'=@(742,325,624,505);arrows=@(625,301,117,529);log=@(0,830,1384,106);statusbar=@(0,936,1384,25)}}
        if($name -eq 'text-find-bar'){
            foreach($key in @('pathbar.left','pathbar.right','info.left','info.right')){$regions[$key][1]+=38}
            foreach($key in @('gutter.left','gutter.right','pane.left','pane.right')){$regions[$key][1]+=38;$regions[$key][3]-=38}
            $regions.findbar=@(0,70,1384,38)
        }
        if($name -eq 'dlg-options'){$regions=[ordered]@{navigation=@(0,0,242,604);content=@(242,0,617,604);buttons=@(0,604,859,51)}}
        if($name -eq 'dlg-text-session-settings'){$regions=[ordered]@{tabs=@(8,8,567,28);content=@(8,36,567,469);buttons=@(0,505,583,58)}}
        if($name -like 'text-menu-*'){
            $menu=$name.Substring(10)
            $tree=Get-Content (Join-Path $EvidenceRoot "supplemental/text-menu-$menu.tree.json") -Raw | ConvertFrom-Json -AsHashtable
            $r=$tree.screenRect
            if($r){$regions.dropdown=@(($r[0]-28),($r[1]-51),$r[2],$r[3])}
        }
        $scenes[$name]=@{corpus=@($entry.corpus);arguments=@($entry.arguments);operations=@($entry.operations);client=@{width=$entry.width;height=$entry.height};regions=$regions;sha256=$entry.sha256;capture=$entry.capture;checks=$entry.checks}
    }
    $data=[ordered]@{version=2;reference=@{product='Beyond Compare';version='5.0.6';dpi=96;language='English';theme='Light';notes='Find bar 与菜单合成方案获协调者批准；没有提取 exe 图标/位图。'};colors=$colors;metrics=$metrics;fonts=$fonts;inventories=$inventories;chmCrosscheck=$crosscheck;scenes=$scenes}
    New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
    $data | ConvertTo-Json -Depth 60 | Set-Content (Join-Path $OutputDirectory 'tokens.json') -Encoding utf8
    Write-Output "TOKENS colors=$($colors.Count) metrics=$($metrics.Count) fonts=$($fonts.Count) inventories=$($inventories.Count) scenes=$($scenes.Count)"
    foreach($view in $crosscheck.Keys){Write-Output "$view CHM=$($crosscheck[$view].chmCommandCount) UI=$($crosscheck[$view].observedMenuEntries)"}
} finally {foreach($bitmap in $bitmaps.Values){$bitmap.Dispose()}}




