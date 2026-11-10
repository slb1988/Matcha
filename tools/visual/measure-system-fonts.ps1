[CmdletBinding()]
param([string]$Output = (Join-Path $PSScriptRoot '../../docs/visual-parity/supplemental/system-fonts.json'))
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'reference-native.ps1')
if(-not ('ReferenceSystemFonts' -as [type])) {
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ReferenceSystemFonts {
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct LOGFONT {
        public int height,width,escapement,orientation,weight;
        public byte italic,underline,strikeout,charset,outprecision,clipprecision,quality,pitch;
        [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)]public string family;
    }
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]public struct METRICS {
        public uint size;public int borderWidth,scrollWidth,scrollHeight,captionWidth,captionHeight;
        public LOGFONT captionFont;public int smallCaptionWidth,smallCaptionHeight;public LOGFONT smallCaptionFont;
        public int menuWidth,menuHeight;public LOGFONT menuFont,statusFont,messageFont;public int paddedBorderWidth;
    }
    [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern bool SystemParametersInfo(uint action,uint parameter,ref METRICS data,uint update);
    [DllImport("gdi32.dll")]public static extern IntPtr GetStockObject(int index);
    [DllImport("gdi32.dll",CharSet=CharSet.Unicode)]public static extern int GetObject(IntPtr h,int size,out LOGFONT font);
    public static METRICS Read(){var m=new METRICS();m.size=(uint)Marshal.SizeOf(m);if(!SystemParametersInfo(0x29,m.size,ref m,0))throw new Exception("SPI_GETNONCLIENTMETRICS failed");return m;}
    public static LOGFONT DefaultGui(){LOGFONT f;GetObject(GetStockObject(17),Marshal.SizeOf(typeof(LOGFONT)),out f);return f;}
}
"@
}
$metrics=[ReferenceSystemFonts]::Read()
$stock=[ReferenceSystemFonts]::DefaultGui()
$result=[ordered]@{method='SystemParametersInfoW(SPI_GETNONCLIENTMETRICS=0x29) / GetStockObject(DEFAULT_GUI_FONT=17)';messageFont=$metrics.messageFont;menuFont=$metrics.menuFont;statusFont=$metrics.statusFont;defaultGuiFont=$stock}
New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
$result | ConvertTo-Json -Depth 6 | Set-Content $Output -Encoding utf8
Write-Output "SPI message=$($metrics.messageFont.family),$($metrics.messageFont.height); menu=$($metrics.menuFont.family),$($metrics.menuFont.height); DEFAULT_GUI_FONT=$($stock.family),$($stock.height)"
