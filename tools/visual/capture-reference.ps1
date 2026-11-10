[CmdletBinding(DefaultParameterSetName='One')]
param(
    [Parameter(Mandatory, ParameterSetName='One')][string]$Scene,
    [Parameter(Mandatory, ParameterSetName='All')][switch]$All,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../../docs/visual-parity/reference'),
    [switch]$Supplemental
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'reference-native.ps1')
. (Join-Path $PSScriptRoot 'reference-accessibility.ps1')
$corpus = (Resolve-Path (Join-Path $PSScriptRoot 'corpus')).Path
$exe = 'D:\Program Files\Beyond Compare 5\BCompare.exe'
$names = @('home','text-basic','text-diffs-filter','text-long-scrolled','text-cjk-tabs','text-crlf-lf','text-cursor-on-diff','text-menu-session','text-menu-file','text-menu-edit','text-menu-search','text-menu-view','text-menu-tools','text-menu-help','folder-basic','folder-expanded','folder-diffs-filter','folder-sync','table-basic','table-cursor-on-diff','dlg-text-session-settings','dlg-options','text-find-bar')
if (-not $All -and $Scene -notin $names) { throw "未知场景 $Scene" }
$selected = if ($All) { $names } else { @($Scene) }
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$OutputDirectory = (Resolve-Path $OutputDirectory).Path
$lockPath = Join-Path $env:TEMP 'matcha-reference-tool.lock'
# 原子互斥；不删除别人持有的锁。不读取设置文件或安装资源。
$lock = [IO.File]::Open($lockPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
$process = $null
$results = [Collections.Generic.List[object]]::new()
try {
    if (Get-Process BCompare -ErrorAction SilentlyContinue) { throw '已有参考程序进程；请先协调使用权。' }
    foreach ($name in $selected) {
        $bitmap = $null
        $before = $null
        try {
            $kind = 'Text Compare'
            $files = @('text-basic-left.txt', 'text-basic-right.txt')
            if ($name -eq 'home') { $kind='Home'; $files=@() }
            elseif ($name -like 'folder-*') { $kind='Folder Compare'; $files=@('folder-left','folder-right') }
            elseif ($name -like 'table-*') { $kind='Table Compare'; $files=@('table-left.csv','table-right.csv') }
            elseif ($name -eq 'text-long-scrolled') { $files=@('text-long-left.txt','text-long-right.txt') }
            elseif ($name -eq 'text-cjk-tabs') { $files=@('text-cjk-tabs-left.txt','text-cjk-tabs-right.txt') }
            elseif ($name -eq 'text-crlf-lf') { $files=@('text-crlf-left.txt','text-crlf-right.txt') }
            if ($name -eq 'folder-sync') { $kind='Folder Sync' }
            $arguments = @('/solo')
            if ($kind -ne 'Home') { $arguments += '/fv="' + $kind + '"' }
            foreach ($file in $files) { $arguments += '"' + (Join-Path $corpus $file) + '"' }
            $process = Start-Process $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru
            [void]$process.WaitForInputIdle(15000)
            $handle = [IntPtr]::Zero
            for ($attempt=0; $attempt -lt 40; $attempt++) {
                $process.Refresh()
                $handle=$process.MainWindowHandle
                if ($handle -ne [IntPtr]::Zero) {
                    $startupTitle=Get-ReferenceTitle $handle
                    if ($startupTitle -like '*Beyond Compare*' -and ($kind -eq 'Home' -or $startupTitle -like "*$kind*")) { break }
                }
                Start-Sleep -Milliseconds 250
            }
            if ($handle -eq [IntPtr]::Zero) { throw "主窗口未出现: $name" }
            [void][ReferenceNative]::ShowWindow($handle, 9)
            for ($attempt=0; $attempt -lt 20; $attempt++) {
                [void][ReferenceNative]::MoveWindow($handle,20,20,1400,1000,$true)
                Start-Sleep -Milliseconds 200
                $rect=[ReferenceNative+RECT]::new()
                [void][ReferenceNative]::GetWindowRect($handle,[ref]$rect)
                if (($rect.Right-$rect.Left) -eq 1400 -and ($rect.Bottom-$rect.Top) -eq 1000) { break }
            }
            [void][ReferenceNative]::SetForegroundWindow($handle)
            Start-Sleep -Milliseconds 1300
            # Windows 前台锁可能拒绝 SetForegroundWindow；点击空白工具栏激活后再发键。
            Click-ReferenceClient $handle 1100 48
            $title=Get-ReferenceTitle $handle
            if ($kind -ne 'Home' -and $title -notlike "*$kind*") { throw "错误视图: expected=$kind title=$title" }
            if($kind -in @('Text Compare','Folder Compare','Table Compare')) {
                # 不继承上次临时会话的显示过滤；每个场景显式从 All 开始。
                Click-ReferenceClient $handle 174 48
            }
            if($kind -eq 'Text Compare') {
                Click-ReferenceClient $handle 193 12
                $popup=Get-ReferenceWindows $process.Id | Where-Object class -match 'Popup|32768' | Select-Object -First 1
                $context=Get-ReferenceAccessibleItems $popup.handle | Where-Object name -eq 'Show Context'
                if(-not $context){throw '无法确认 Context 初始状态'}
                if(($context.state -band 16) -eq 0) {
                    $origin=[ReferenceNative+POINT]::new()
                    [void][ReferenceNative]::ClientToScreen($handle,[ref]$origin)
                    Click-ReferenceClient $handle ($context.rect[0]-$origin.X+40) ($context.rect[1]-$origin.Y+10)
                } else { Send-ReferenceKeys $handle '{ESC}' }
                Click-ReferenceClient $handle 1100 48
                Click-ReferenceClient $handle 200 132
                Send-ReferenceKeys $handle '^{HOME}'
            }
            if($kind -eq 'Folder Compare') {
                Click-ReferenceClient $handle 129 12
                Click-ReferenceClient $handle 170 59
            }
            $before=Get-ReferenceBitmap $handle
            $operations=[Collections.Generic.List[string]]::new()
            $captureHandle=$handle
            $screen=$false
            switch ($name) {
                'text-diffs-filter' { Click-ReferenceClient $handle 233 48; Click-ReferenceClient $handle 358 48; $operations.Add('toolbar Diffs; Context off') }
                'text-long-scrolled' { Click-ReferenceClient $handle 250 300; Send-ReferenceKeys $handle '{PGDN}{PGDN}{PGDN}'; $operations.Add('left pane; PageDown x3') }
                'text-cursor-on-diff' { Click-ReferenceClient $handle 140 146; $operations.Add('left pane line 2') }
                'folder-expanded' { Click-ReferenceClient $handle 150 174; Send-ReferenceKeys $handle '^a'; Click-ReferenceClient $handle 129 12; Click-ReferenceClient $handle 170 38; $operations.Add('Ctrl+A; Edit > Expand All') }
                'folder-diffs-filter' { Click-ReferenceClient $handle 233 48; $operations.Add('toolbar Diffs') }
                'table-cursor-on-diff' { Click-ReferenceClient $handle 235 186; $operations.Add('left grid differing cell') }
                'home' { Click-ReferenceClient $handle 10 90; Click-ReferenceClient $handle 60 71; $operations.Add('collapse Auto-saved; select New (no session files edited)') }
                'text-find-bar' { Click-ReferenceClient $handle 147 12; Click-ReferenceClient $handle 185 184; $operations.Add('Search > Find; docked bar (coordinator approved)') }
                'dlg-text-session-settings' { Click-ReferenceClient $handle 28 12; Click-ReferenceClient $handle 100 226; $operations.Add('Session > Session Settings (observed menu row)') }
                'dlg-options' { Click-ReferenceClient $handle 234 12; Click-ReferenceClient $handle 280 38; $operations.Add('Tools > Options') }
            }
            if ($name -like 'text-menu-*') {
                $menuX=@{session=28;file=72;edit=105;search=147;view=193;tools=234;help=276}
                $menu=$name.Substring(10)
                Click-ReferenceClient $handle $menuX[$menu] 12
                $screen=$true
                $operations.Add("open top-level $menu menu")
            }
            if ($name -like 'dlg-*') {
                $expected=@{'dlg-find'='Find';'dlg-options'='Options';'dlg-text-session-settings'='Session Settings'}[$name]
                for($attempt=0;$attempt -lt 30;$attempt++) {
                    $dialog=Get-ReferenceWindows $process.Id | Where-Object { $_.handle -ne $handle -and $_.title -like "*$expected*" } | Select-Object -First 1
                    if($dialog){$captureHandle=$dialog.handle;break}
                    Start-Sleep -Milliseconds 200
                }
                [uint32]$owner=0
                [void][ReferenceNative]::GetWindowThreadProcessId($captureHandle,[ref]$owner)
                if ($captureHandle -eq $handle -or $owner -ne $process.Id) { throw "独立对话框未出现: $name" }
                $title=Get-ReferenceTitle $captureHandle
                if ($title -notlike "*$expected*") { throw "错误对话框: $title" }
            }
            $bitmap=if($screen){Get-ReferenceMenuBitmap $handle $process.Id}else{Get-ReferenceBitmap $captureHandle}
            $checks=[Collections.Generic.List[string]]::new()
            if ($name -notlike 'dlg-*') {
                if ($bitmap.Width -ne 1384 -or $bitmap.Height -ne 961) { throw "客户区尺寸错误: $($bitmap.Size)" }
                $ink=0
                for($y=4;$y -lt 20;$y++) { for($x=5;$x -lt 290;$x++) { if($bitmap.GetPixel($x,$y).GetBrightness() -lt .45){$ink++} } }
                if ($ink -lt 150) { throw '顶行无菜单文字；可能锁屏、遮挡或错误窗口。' }
                $checks.Add("client=1384x961; menuInk=$ink")
            } else { $checks.Add("dialogHwnd=$captureHandle != mainHwnd=$handle; title=$title") }
            if ($name -like 'text-menu-*') {
                $delta=Get-ChangedSamples $before $bitmap @(0,25,650,650)
                if ($delta -lt 500) { throw "下拉菜单未打开: changed samples=$delta" }
                $checks.Add("menu dropdown changed samples=$delta")
            }
            if ($name -eq 'text-find-bar') {
                $delta=Get-ChangedSamples $before $bitmap @(0,70,1384,40)
                if ($delta -lt 300) { throw 'Find bar 未打开。' }
                $checks.Add("findbar changed samples=$delta")
            }
            if ($name -in @('text-diffs-filter','text-long-scrolled','text-cursor-on-diff','folder-expanded','folder-diffs-filter','table-cursor-on-diff')) {
                $delta=Get-ChangedSamples $before $bitmap @(0,27,1384,908)
                if ($delta -lt 10) { throw "场景操作未改变内容: $name delta=$delta" }
                $checks.Add("operation changed samples=$delta")
            }
            if ($kind -in @('Folder Compare','Folder Sync','Table Compare')) {
                # 双侧列头/网格带垂直边缘计数，结合窗口类型检查，拒绝复用文本画面。
                $edgeCounts=@()
                foreach($start in @(55,745)) {
                    $edges=0
                    for($y=120;$y -lt 180;$y++) { for($x=$start;$x -lt ($start+580);$x+=2) {
                        if ([Math]::Abs($bitmap.GetPixel($x,$y).GetBrightness()-$bitmap.GetPixel(($x+1),$y).GetBrightness()) -gt .25) {$edges++}
                    } }
                    $edgeCounts+=$edges
                }
                if (($edgeCounts | Measure-Object -Minimum).Minimum -lt 40) { throw "双树列头/网格自检失败: $edgeCounts" }
                $checks.Add("$kind dual header/grid edges=$($edgeCounts -join ',')")
            }
            $destination=Join-Path $OutputDirectory "$name.png"
            $temporary=Join-Path $OutputDirectory "$name.pending.png"
            $bitmap.Save($temporary,[Drawing.Imaging.ImageFormat]::Png)
            $hash=(Get-FileHash $temporary -Algorithm SHA256).Hash
            if ($results.sha256 -contains $hash) { Remove-Item -LiteralPath $temporary; throw "批次重复图像: $name SHA256=$hash" }
            Move-Item -LiteralPath $temporary -Destination $destination -Force
            $results.Add([ordered]@{scene=$name;arguments=$arguments;corpus=$files;operations=@($operations);width=$bitmap.Width;height=$bitmap.Height;sha256=$hash;checks=@($checks);title=$title;capture= $(if($screen){'PrintWindow client + popup hwnd composite (coordinator approved)'}else{'PrintWindow client'})})
            Write-Output "PASS $name $($bitmap.Width)x$($bitmap.Height) $($hash.Substring(0,12)) $($checks -join '; ')"
            if ($Supplemental) {
                $extra=Join-Path $OutputDirectory '../supplemental'
                New-Item -ItemType Directory -Force $extra | Out-Null
                @([ReferenceNative]::Fonts($captureHandle)) | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $extra "$name-fonts.json") -Encoding utf8
                @(Get-ReferenceAccessibleItems $captureHandle) | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $extra "$name-accessibility.json") -Encoding utf8
                if($name -eq 'dlg-options') {
                    Click-ReferenceClient $captureHandle 100 147
                    Click-ReferenceClient $captureHandle 366 128
                    $fontPage=Get-ReferenceBitmap $captureHandle
                    $fontPage.Save((Join-Path $extra 'options-file-views.png'),[Drawing.Imaging.ImageFormat]::Png)
                    $fontPage.Dispose()
                    Click-ReferenceClient $captureHandle 310 443
                    Start-Sleep -Seconds 1
                    $fontHandle=[ReferenceNative]::GetForegroundWindow()
                    if($fontHandle -eq $captureHandle){throw 'Editor Font 对话框未打开'}
                    $fontPage=Get-ReferenceBitmap $fontHandle
                    $fontPage.Save((Join-Path $extra 'options-editor-font.png'),[Drawing.Imaging.ImageFormat]::Png)
                    $fontPage.Dispose()
                    Send-ReferenceKeys $fontHandle '{ESC}'
                }
                if($name -in @('home','text-basic','folder-basic','folder-sync','table-basic')) {
                    $view=@{'home'='home';'text-basic'='text';'folder-basic'='folder';'folder-sync'='sync';'table-basic'='table'}[$name]
                    $menuPoints=if($view -eq 'home'){@(@('Session',28),@('View',78),@('Tools',118),@('Help',160))}
                    elseif($view -in @('folder','sync')){@(@('Session',28),@('Actions',85),@('Edit',129),@('Search',172),@('View',216),@('Tools',258),@('Help',298))}
                    else{@(@('Session',28),@('File',72),@('Edit',105),@('Search',147),@('View',193),@('Tools',234),@('Help',276))}
                    foreach($menuPoint in $menuPoints) {
                        Click-ReferenceClient $handle $menuPoint[1] 12
                        $popup=Get-ReferenceWindows $process.Id | Where-Object class -match 'Popup|32768' | Select-Object -First 1
                        if(-not $popup){throw "菜单未打开: $view/$($menuPoint[0])"}
                        $null=Save-ReferenceMenuTree $popup.handle $process.Id $extra "$view-menu-$($menuPoint[0])"
                        Send-ReferenceKeys $handle '{ESC}'
                    }
                    if($view -in @('text','folder','table')) {
                        Click-ReferenceClient $handle 200 175
                        [ReferenceNative]::mouse_event(8,0,0,0,[UIntPtr]::Zero)
                        [ReferenceNative]::mouse_event(16,0,0,0,[UIntPtr]::Zero)
                        Start-Sleep -Milliseconds 500
                        $popup=Get-ReferenceWindows $process.Id | Where-Object class -match 'Popup|32768' | Select-Object -First 1
                        if(-not $popup){throw "右键菜单未打开: $view"}
                        $null=Save-ReferenceMenuTree $popup.handle $process.Id $extra "$view-context"
                        Send-ReferenceKeys $handle '{ESC}'
                    }
                }
            }
            if ($name -like 'dlg-*') { Send-ReferenceKeys $captureHandle '{ESC}' }
        } finally {
            if ($bitmap) { $bitmap.Dispose() }
            if ($before) { $before.Dispose() }
            if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force; $process.WaitForExit() }
            $process=$null
        }
    }
    if ($All) {
        $duplicates=Get-ChildItem -LiteralPath $OutputDirectory -Filter '*.png' | Get-FileHash | Group-Object Hash | Where-Object Count -gt 1
        if ($duplicates) { throw '全局 SHA256 去重失败。' }
    }
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    $lock.Dispose()
    Remove-Item -LiteralPath $lockPath -Force
    $manifest=if($All){'capture-manifest.json'}else{"$Scene.capture.json"}
    @($results) | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $OutputDirectory $manifest) -Encoding utf8
    Write-Output "CLEANUP lockExists=$(Test-Path $lockPath); BCompareProcesses=$(@(Get-Process BCompare -ErrorAction SilentlyContinue).Count)"
}
