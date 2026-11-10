# 仅由 capture-reference.ps1 在互斥锁内调用；不访问参考程序设置或资源。
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
if (-not ('ReferenceNative' -as [type])) {
    Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class ReferenceNative {
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr p);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint msg,IntPtr w,IntPtr l);
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct LOGFONT {
        public int height,width,escapement,orientation,weight;
        public byte italic,underline,strikeout,charset,outprecision,clipprecision,quality,pitch;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)] public string family;
    }
    [DllImport("gdi32.dll", CharSet=CharSet.Unicode)] public static extern int GetObject(IntPtr h,int size,out LOGFONT font);
    public sealed class FontEvidence { public string windowClass; public string family; public int heightPx; public int weight; }
    public static FontEvidence[] Fonts(IntPtr parent) {
        var fonts=new List<FontEvidence>();
        EnumChildWindows(parent,(h,p)=>{
            IntPtr hf=SendMessage(h,0x31,IntPtr.Zero,IntPtr.Zero); LOGFONT f;
            if(hf!=IntPtr.Zero && GetObject(hf,Marshal.SizeOf(typeof(LOGFONT)),out f)!=0) {
                var c=new StringBuilder(256);GetClassName(h,c,256);
                fonts.Add(new FontEvidence{windowClass=c.ToString(),family=f.family,heightPx=f.height,weight=f.weight});
            }
            return true;
        },IntPtr.Zero);
        return fonts.ToArray();
    }
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder name, int count);
    public static IntPtr[] Windows(uint process) {
        var list = new List<IntPtr>();
        EnumWindows((h,p) => { uint id; GetWindowThreadProcessId(h,out id); if(id==process && IsWindowVisible(h)) list.Add(h); return true; }, IntPtr.Zero);
        return list.ToArray();
    }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
}
'@
}

function Get-ReferenceWindows([uint32]$ProcessId) {
    foreach ($handle in [ReferenceNative]::Windows($ProcessId)) {
        $class=[Text.StringBuilder]::new(256)
        [void][ReferenceNative]::GetClassName($handle,$class,256)
        $rect=[ReferenceNative+RECT]::new()
        [void][ReferenceNative]::GetWindowRect($handle,[ref]$rect)
        [pscustomobject]@{handle=$handle;class=$class.ToString();title=(Get-ReferenceTitle $handle);rect=$rect}
    }
}

function Get-ReferenceMenuBitmap([IntPtr]$Handle, [uint32]$ProcessId) {
    # 主窗口和每个真实弹出 hwnd 分别 PrintWindow；不混入其他应用悬浮层。
    $bitmap=Get-ReferenceBitmap $Handle
    $origin=[ReferenceNative+POINT]::new()
    [void][ReferenceNative]::ClientToScreen($Handle,[ref]$origin)
    $popups=@(Get-ReferenceWindows $ProcessId | Where-Object { $_.class -match '32768|Popup' })
    if(-not $popups.Count){$bitmap.Dispose(); throw '未找到可合成的弹出菜单 hwnd。'}
    $g=[Drawing.Graphics]::FromImage($bitmap)
    try {
        foreach($popup in $popups) {
            $rect=$popup.rect
            $menu=[Drawing.Bitmap]::new(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
            $mg=[Drawing.Graphics]::FromImage($menu)
            $dc=$mg.GetHdc()
            try { if(-not [ReferenceNative]::PrintWindow($popup.handle,$dc,2)){throw '菜单 PrintWindow 失败'} }
            finally { $mg.ReleaseHdc($dc);$mg.Dispose() }
            try {
                $g.DrawImageUnscaled($menu,($rect.Left-$origin.X),($rect.Top-$origin.Y))
            } finally { $menu.Dispose() }
        }
    } finally {$g.Dispose()}
    return ,$bitmap
}

function Get-ReferenceTitle([IntPtr]$Handle) {
    $text = [Text.StringBuilder]::new(1024)
    [void][ReferenceNative]::GetWindowText($Handle, $text, $text.Capacity)
    $text.ToString()
}

function Get-ReferenceBitmap([IntPtr]$Handle, [switch]$Screen) {
    $client = [ReferenceNative+RECT]::new()
    $window = [ReferenceNative+RECT]::new()
    $origin = [ReferenceNative+POINT]::new()
    [void][ReferenceNative]::GetClientRect($Handle, [ref]$client)
    [void][ReferenceNative]::GetWindowRect($Handle, [ref]$window)
    [void][ReferenceNative]::ClientToScreen($Handle, [ref]$origin)
    $bitmap = [Drawing.Bitmap]::new($client.Right, $client.Bottom)
    if ($Screen) {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($origin.X, $origin.Y, 0, 0, $bitmap.Size) }
        finally { $graphics.Dispose() }
    } else {
        $full = [Drawing.Bitmap]::new(($window.Right-$window.Left), ($window.Bottom-$window.Top))
        $graphics = [Drawing.Graphics]::FromImage($full)
        $dc = $graphics.GetHdc()
        try {
            if (-not [ReferenceNative]::PrintWindow($Handle, $dc, 2)) { throw 'PrintWindow failed' }
        } finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $crop = [Drawing.Rectangle]::new(($origin.X-$window.Left), ($origin.Y-$window.Top), $client.Right, $client.Bottom)
            $graphics.DrawImage($full, [Drawing.Rectangle]::new(0,0,$client.Right,$client.Bottom), $crop, [Drawing.GraphicsUnit]::Pixel)
        } finally { $graphics.Dispose(); $full.Dispose() }
    }
    return ,$bitmap
}

function Send-ReferenceKeys([IntPtr]$Handle, [string]$Keys) {
    [void][ReferenceNative]::SetForegroundWindow($Handle)
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds 450
}

function Click-ReferenceClient([IntPtr]$Handle, [int]$X, [int]$Y) {
    $origin = [ReferenceNative+POINT]::new()
    [void][ReferenceNative]::ClientToScreen($Handle, [ref]$origin)
    [void][ReferenceNative]::SetForegroundWindow($Handle)
    [void][ReferenceNative]::SetCursorPos(($origin.X+$X), ($origin.Y+$Y))
    [ReferenceNative]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
    [ReferenceNative]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

function Get-ChangedSamples($Before, $After, [int[]]$Rect) {
    $changed = 0
    for ($y=$Rect[1]; $y -lt ($Rect[1]+$Rect[3]); $y+=3) {
        for ($x=$Rect[0]; $x -lt ($Rect[0]+$Rect[2]); $x+=3) {
            if ($Before.GetPixel($x,$y).ToArgb() -ne $After.GetPixel($x,$y).ToArgb()) { $changed++ }
        }
    }
    return $changed
}
