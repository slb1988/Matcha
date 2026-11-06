param(
    [ValidateSet('Start','Click','Key','Capture','Close')][string]$Action='Start',
    [int]$ProcessId=0,[int]$X=0,[int]$Y=0,
    [string]$Name='folder-waveC-smoke',
    [ValidateSet('Tab','Enter','Escape','Down','Up','Space')][string]$Key='Tab',
    [int]$Repeat=1
)
$ErrorActionPreference='Stop'
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$exe=Join-Path $repo 'target\release\matcha.exe'
$statePath=Join-Path $repo 'target\folder-waveC-smoke.json'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName Microsoft.VisualBasic
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MatchaSmoke {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int mode);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
}
'@
if($Action -eq 'Start'){
    $root=Join-Path ([IO.Path]::GetTempPath()) ('matcha-gui-smoke-'+[Guid]::NewGuid().ToString('N'))
    $utf8=New-Object Text.UTF8Encoding($false)
    foreach($side in @('left','right')){[IO.Directory]::CreateDirectory((Join-Path $root "$side\nested"))|Out-Null}
    foreach($side in @('left','right')){
        [IO.File]::WriteAllText((Join-Path $root "$side\same.txt"),"same line`r`n",$utf8)
        [IO.File]::WriteAllText((Join-Path $root "$side\newer.txt"),"$side content`r`n",$utf8)
        [IO.File]::WriteAllText((Join-Path $root "$side\nested\child.txt"),"child`r`n",$utf8)
        [IO.File]::SetLastWriteTimeUtc((Join-Path $root "$side\same.txt"),[DateTime]::Parse('2026-01-01T00:00:00Z').ToUniversalTime())
        [IO.File]::SetLastWriteTimeUtc((Join-Path $root "$side\newer.txt"),[DateTime]::Parse('2026-01-01T00:00:00Z').ToUniversalTime().AddSeconds($(if($side -eq 'left'){10}else{0})))
    }
    [IO.File]::WriteAllText((Join-Path $root 'left\left-only.txt'),"left`r`n",$utf8)
    [IO.File]::WriteAllText((Join-Path $root 'right\right-only.txt'),"right`r`n",$utf8)
    $p=Start-Process -FilePath $exe -ArgumentList @(('"'+(Join-Path $root 'left')+'"'),('"'+(Join-Path $root 'right')+'"')) -PassThru -RedirectStandardError (Join-Path $repo 'target\folder-waveC-smoke.err')
    @{pid=$p.Id;root=$root;executable=$exe}|ConvertTo-Json|Set-Content -LiteralPath $statePath -Encoding UTF8
    Write-Output "smoke.pid=$($p.Id)";Write-Output "smoke.fixture=$root"
    Start-Sleep -Seconds 3
    $ProcessId=$p.Id
}else{
    $state=[IO.File]::ReadAllText($statePath)|ConvertFrom-Json
    if($ProcessId -eq 0){$ProcessId=$state.pid}
}
$p=[Diagnostics.Process]::GetProcessById($ProcessId)
if(![string]::Equals($p.MainModule.FileName,$exe,[StringComparison]::OrdinalIgnoreCase)){throw 'Refusing to control a process outside this worktree'}
$p.Refresh();$h=$p.MainWindowHandle
if($h -eq [IntPtr]::Zero){throw 'Owned application has no window'}
if($Action -eq 'Close'){
    if(!$p.CloseMainWindow()){throw 'Owned window did not accept close'}
    if(!$p.WaitForExit(15000)){throw 'Owned process still running; fixture retained'}
    if($null -eq $state){$state=[IO.File]::ReadAllText($statePath)|ConvertFrom-Json}
    $temp=[IO.Path]::GetTempPath()
    if(!$state.root.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase)){throw 'Fixture is not in system temporary directory'}
    [IO.Directory]::Delete($state.root,$true)
    Write-Output 'smoke.closed=true';exit 0
}
[MatchaSmoke]::ShowWindow($h,9)|Out-Null
[Microsoft.VisualBasic.Interaction]::AppActivate($ProcessId)
[MatchaSmoke]::SetForegroundWindow($h)|Out-Null
Start-Sleep -Milliseconds 300
if($Action -eq 'Click'){
    if([MatchaSmoke]::GetForegroundWindow() -ne $h){throw 'Physical desktop input unavailable; no mouse action claimed'}
    $rect=New-Object MatchaSmoke+Rect;[MatchaSmoke]::GetClientRect($h,[ref]$rect)|Out-Null
    if($X -lt 0 -or $Y -lt 0 -or $X -ge $rect.Right -or $Y -ge $rect.Bottom){throw 'Click is outside owned client rectangle'}
    $point=[IntPtr]($X -bor ($Y -shl 16))
    [MatchaSmoke]::PostMessage($h,0x0200,[IntPtr]::Zero,$point)|Out-Null
    [MatchaSmoke]::PostMessage($h,0x0201,[IntPtr]1,$point)|Out-Null
    Start-Sleep -Milliseconds 80
    [MatchaSmoke]::PostMessage($h,0x0202,[IntPtr]::Zero,$point)|Out-Null
    Start-Sleep -Milliseconds 500
}
if($Action -eq 'Key'){
    $keys=@{Tab=@(0x09,0x0F);Enter=@(0x0D,0x1C);Escape=@(0x1B,0x01);Down=@(0x28,0x50);Up=@(0x26,0x48);Space=@(0x20,0x39)}
    $code=$keys[$Key];$down=1 -bor ($code[1] -shl 16);$up=[int64]$down -bor 0xC0000000L
    [MatchaSmoke]::PostMessage($h,0x0007,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
    for($i=0;$i -lt $Repeat;$i++){
        [MatchaSmoke]::PostMessage($h,0x0100,[IntPtr]$code[0],[IntPtr]$down)|Out-Null
        Start-Sleep -Milliseconds 60
        [MatchaSmoke]::PostMessage($h,0x0101,[IntPtr]$code[0],[IntPtr]$up)|Out-Null
        Start-Sleep -Milliseconds 80
    }
}
# Native framebuffer capture is target-scoped and works without stealing foreground.
$bmp=Join-Path ([IO.Path]::GetTempPath()) ("matcha-folder-view-"+$ProcessId+".bmp")
if(Test-Path -LiteralPath $bmp){[IO.File]::Delete($bmp)}
[MatchaSmoke]::PostMessage($h,0x0100,[IntPtr]0x7B,[IntPtr]0x00580001)|Out-Null
Start-Sleep -Milliseconds 80
[MatchaSmoke]::PostMessage($h,0x0101,[IntPtr]0x7B,[IntPtr]0xC0580001)|Out-Null
$deadline=[DateTime]::UtcNow.AddSeconds(8)
while(!(Test-Path -LiteralPath $bmp)){if([DateTime]::UtcNow -ge $deadline){throw 'Owned viewport did not return a diagnostic framebuffer'};Start-Sleep -Milliseconds 50}
$rect=New-Object MatchaSmoke+Rect;$origin=New-Object MatchaSmoke+Point
[MatchaSmoke]::GetClientRect($h,[ref]$rect)|Out-Null;[MatchaSmoke]::ClientToScreen($h,[ref]$origin)|Out-Null
$bitmap=[Drawing.Image]::FromFile($bmp)
$path=Join-Path $repo ("docs\worker-reports\assets\"+$Name+".png")
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path))|Out-Null
try{$bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose();[IO.File]::Delete($bmp)}
Write-Output "smoke.screenshot=$path"
