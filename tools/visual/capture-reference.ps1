[CmdletBinding()]
param([ValidateSet('home','text-basic','text-diffs-filter','text-long-scrolled','text-cjk-tabs','text-crlf-lf','text-cursor-on-diff','text-menu-session','text-menu-file','text-menu-edit','text-menu-search','text-menu-view','text-menu-tools','text-menu-help','folder-basic','folder-expanded','folder-diffs-filter','folder-sync','table-basic','table-cursor-on-diff','dlg-text-session-settings','dlg-options','dlg-find')][string]$Scene, [switch]$All)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$lock=Join-Path $env:TEMP 'matcha-reference-tool.lock'; $root=(Resolve-Path (Join-Path $PSScriptRoot 'corpus')).Path; $out=(Resolve-Path (Join-Path $PSScriptRoot '..\..\docs\visual-parity\reference')).Path
if(-not $Scene -and -not $All){throw 'Specify -Scene or -All'}
$scenes=if($All){@('home','text-basic','text-diffs-filter','text-long-scrolled','text-cjk-tabs','text-crlf-lf','text-cursor-on-diff','text-menu-session','text-menu-file','text-menu-edit','text-menu-search','text-menu-view','text-menu-tools','text-menu-help','folder-basic','folder-expanded','folder-diffs-filter','folder-sync','table-basic','table-cursor-on-diff','dlg-text-session-settings','dlg-options','dlg-find')}else{@($Scene)}
$got=$false; for($i=0;$i -lt 60 -and -not $got;$i++){try{New-Item -ItemType File -Path $lock -ErrorAction Stop | Out-Null;$got=$true}catch{Start-Sleep -Seconds 10}}
if(-not $got){throw 'Timed out acquiring reference lock'}
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class RefWin { [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int hgt,bool r); [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r); [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out RECT r); [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref POINT p); [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h); public struct RECT{public int Left,Top,Right,Bottom;} public struct POINT{public int X,Y;} }
"@
try {
  if((Get-Process BCompare -ErrorAction SilentlyContinue).Count -gt 1){throw 'Another BCompare.exe process is running'}
  $exe='D:\Program Files\Beyond Compare 5\BCompare.exe'; $left=Join-Path $root 'text-basic-left.txt'; $right=Join-Path $root 'text-basic-right.txt'
  $p=Get-Process BCompare -ErrorAction SilentlyContinue | Select-Object -First 1
  if($scenes -contains 'home') { Get-Process BCompare -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 300; $p=Start-Process $exe -ArgumentList @('/solo') -PassThru } elseif(-not $p){$p=Start-Process $exe -ArgumentList @('/solo',$left,$right) -PassThru}
  $p.WaitForInputIdle(); Start-Sleep -Seconds 2; $p.Refresh(); $h=$p.MainWindowHandle
  if(-not [RefWin]::IsWindow($h)){throw 'Reference window not found'}
  [RefWin]::MoveWindow($h,20,20,1400,1000,$true)|Out-Null; Start-Sleep -Milliseconds 500
  foreach($s in $scenes){
    for($try=0;$try -lt 5;$try++){[RefWin]::MoveWindow($h,20,20,1400,1000,$true)|Out-Null; Start-Sleep -Milliseconds 150; $wr=New-Object RefWin+RECT; [RefWin]::GetWindowRect($h,[ref]$wr)|Out-Null; if(($wr.Right-$wr.Left)-eq 1400 -and ($wr.Bottom-$wr.Top)-eq 1000){break}}
    [RefWin]::SetForegroundWindow($h)|Out-Null
    switch -Wildcard ($s) { '*long-scrolled' {[System.Windows.Forms.SendKeys]::SendWait('{PGDN}{PGDN}{PGDN}')} '*cursor*' {[System.Windows.Forms.SendKeys]::SendWait('{DOWN}{DOWN}')} '*menu-session' {[System.Windows.Forms.SendKeys]::SendWait('%s')} '*menu-file' {[System.Windows.Forms.SendKeys]::SendWait('%f')} '*menu-edit' {[System.Windows.Forms.SendKeys]::SendWait('%e')} '*menu-search' {[System.Windows.Forms.SendKeys]::SendWait('%h')} '*menu-view' {[System.Windows.Forms.SendKeys]::SendWait('%v')} '*menu-tools' {[System.Windows.Forms.SendKeys]::SendWait('%t')} '*menu-help' {[System.Windows.Forms.SendKeys]::SendWait('%y')} }
    Start-Sleep -Milliseconds 350
    $rc=New-Object RefWin+RECT; [RefWin]::GetClientRect($h,[ref]$rc)|Out-Null; $pt=New-Object RefWin+POINT; [RefWin]::ClientToScreen($h,[ref]$pt)|Out-Null; $w=$rc.Right-$rc.Left; $hh=$rc.Bottom-$rc.Top
    $bmp=New-Object Drawing.Bitmap -ArgumentList ([int]$w),([int]$hh); $g=[Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($pt.X,$pt.Y,0,0,$bmp.Size); $g.Dispose(); $bmp.Save((Join-Path $out ($s+'.png')),[Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
  }
} finally {Remove-Item $lock -Force -ErrorAction SilentlyContinue}


