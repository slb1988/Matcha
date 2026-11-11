param(
  [Parameter(Mandatory=$true)][string]$Folder,
  [Parameter(Mandatory=$true)][string[]]$Names,
  [string]$Invoke = ""
)
$src = @"
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;

[ComImport, Guid("000214E6-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IShellFolder {
  [PreserveSig] int ParseDisplayName(IntPtr hwnd, IntPtr pbc, [MarshalAs(UnmanagedType.LPWStr)] string name, IntPtr pchEaten, out IntPtr ppidl, IntPtr pdwAttributes);
  [PreserveSig] int EnumObjects(IntPtr hwnd, int flags, out IntPtr ppenum);
  [PreserveSig] int BindToObject(IntPtr pidl, IntPtr pbc, [In] ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object ppv);
  [PreserveSig] int BindToStorage(IntPtr pidl, IntPtr pbc, [In] ref Guid riid, out IntPtr ppv);
  [PreserveSig] int CompareIDs(IntPtr lParam, IntPtr pidl1, IntPtr pidl2);
  [PreserveSig] int CreateViewObject(IntPtr hwnd, [In] ref Guid riid, out IntPtr ppv);
  [PreserveSig] int GetAttributesOf(uint cidl, IntPtr apidl, ref uint rgfInOut);
  [PreserveSig] int GetUIObjectOf(IntPtr hwnd, uint cidl, [MarshalAs(UnmanagedType.LPArray)] IntPtr[] apidl, [In] ref Guid riid, IntPtr rgfReserved, [MarshalAs(UnmanagedType.Interface)] out object ppv);
  [PreserveSig] int GetDisplayNameOf(IntPtr pidl, uint flags, IntPtr pName);
  [PreserveSig] int SetNameOf(IntPtr hwnd, IntPtr pidl, [MarshalAs(UnmanagedType.LPWStr)] string name, uint flags, out IntPtr ppidlOut);
}

[ComImport, Guid("000214E4-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IContextMenu {
  [PreserveSig] int QueryContextMenu(IntPtr hmenu, uint indexMenu, uint idCmdFirst, uint idCmdLast, uint uFlags);
  [PreserveSig] int InvokeCommand(ref CMINVOKECOMMANDINFOEX pici);
  [PreserveSig] int GetCommandString(UIntPtr idCmd, uint uType, IntPtr pReserved, StringBuilder pszName, uint cchMax);
}

[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
public struct CMINVOKECOMMANDINFOEX {
  public int cbSize; public uint fMask; public IntPtr hwnd;
  [MarshalAs(UnmanagedType.LPStr)] public string lpVerb;
  [MarshalAs(UnmanagedType.LPStr)] public string lpParameters;
  [MarshalAs(UnmanagedType.LPStr)] public string lpDirectory;
  public int nShow; public uint dwHotKey; public IntPtr hIcon;
  [MarshalAs(UnmanagedType.LPStr)] public string lpTitle;
  [MarshalAs(UnmanagedType.LPWStr)] public string lpVerbW;
  [MarshalAs(UnmanagedType.LPWStr)] public string lpParametersW;
  [MarshalAs(UnmanagedType.LPWStr)] public string lpDirectoryW;
  [MarshalAs(UnmanagedType.LPWStr)] public string lpTitleW;
  public int ptInvokeX; public int ptInvokeY;
}

public static class Probe {
  [DllImport("shell32.dll")] static extern int SHGetDesktopFolder(out IShellFolder ppshf);
  [DllImport("user32.dll")] static extern IntPtr CreatePopupMenu();
  [DllImport("user32.dll")] static extern bool DestroyMenu(IntPtr h);
  [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetMenuStringW(IntPtr h, uint id, StringBuilder s, int max, uint flags);
  [DllImport("user32.dll")] static extern uint GetMenuItemID(IntPtr h, int pos);

  public static List<string> Run(string folder, string[] names, string invokeVerb) {
    IShellFolder desktop; SHGetDesktopFolder(out desktop);
    IntPtr folderPidl; Marshal.ThrowExceptionForHR(desktop.ParseDisplayName(IntPtr.Zero, IntPtr.Zero, folder, IntPtr.Zero, out folderPidl, IntPtr.Zero));
    Guid iidSF = typeof(IShellFolder).GUID; object o;
    Marshal.ThrowExceptionForHR(desktop.BindToObject(folderPidl, IntPtr.Zero, ref iidSF, out o));
    IShellFolder sf = (IShellFolder)o;
    IntPtr[] pidls = new IntPtr[names.Length];
    for (int i = 0; i < names.Length; i++) {
      Marshal.ThrowExceptionForHR(sf.ParseDisplayName(IntPtr.Zero, IntPtr.Zero, names[i], IntPtr.Zero, out pidls[i], IntPtr.Zero));
    }
    Guid iidCM = typeof(IContextMenu).GUID;
    Marshal.ThrowExceptionForHR(sf.GetUIObjectOf(IntPtr.Zero, (uint)pidls.Length, pidls, ref iidCM, IntPtr.Zero, out o));
    IContextMenu cm = (IContextMenu)o;
    IntPtr menu = CreatePopupMenu();
    // CMF_NORMAL | CMF_EXTENDEDVERBS(0x100) off; plain right-click menu
    Marshal.ThrowExceptionForHR(cm.QueryContextMenu(menu, 0, 1, 0x7FFF, 0));
    var result = new List<string>();
    int n = GetMenuItemCount(menu);
    for (int i = 0; i < n; i++) {
      var sb = new StringBuilder(512);
      GetMenuStringW(menu, (uint)i, sb, 512, 0x400);
      uint id = GetMenuItemID(menu, i);
      var verb = new StringBuilder(256);
      if (id != 0xFFFFFFFF && id >= 1) {
        cm.GetCommandString((UIntPtr)(id - 1), 4 /*GCS_VERBW*/, IntPtr.Zero, verb, 256);
      }
      if (sb.Length > 0) result.Add(sb.ToString() + "  [verb=" + verb.ToString() + "]");
    }
    if (invokeVerb.Length > 0) {
      var ici = new CMINVOKECOMMANDINFOEX();
      ici.cbSize = Marshal.SizeOf(typeof(CMINVOKECOMMANDINFOEX));
      ici.fMask = 0x00004000; // CMIC_MASK_UNICODE
      ici.lpVerb = invokeVerb; ici.lpVerbW = invokeVerb; ici.nShow = 1;
      int hr = cm.InvokeCommand(ref ici);
      result.Add("invoke " + invokeVerb + " hr=0x" + hr.ToString("X8"));
    }
    DestroyMenu(menu);
    return result;
  }
}
"@
Add-Type -TypeDefinition $src -Language CSharp
[Probe]::Run($Folder, $Names, $Invoke) | ForEach-Object { $_ }
