Add-Type -AssemblyName Accessibility
if (-not ('ReferenceAccessibility' -as [type])) {
    Add-Type -IgnoreWarnings -WarningAction SilentlyContinue -ReferencedAssemblies Accessibility @"
using System;
using System.Runtime.InteropServices;
public static class ReferenceAccessibility {
    [DllImport("oleacc.dll")] public static extern int AccessibleObjectFromWindow(IntPtr h,uint objectId,ref Guid id,[MarshalAs(UnmanagedType.Interface)] out Accessibility.IAccessible result);
    public sealed class Item {public string name;public string shortcut;public int role;public int state;public int[] rect;public int depth;}
    static Item[] items;
    static void ReadItem(Accessibility.IAccessible acc,object child,int depth) {
        var item=new Item {depth=depth};
        try{item.name=acc.get_accName(child);}catch{}
        try{item.shortcut=acc.get_accKeyboardShortcut(child);}catch{}
        try{item.role=Convert.ToInt32(acc.get_accRole(child));}catch{}
        try{item.state=Convert.ToInt32(acc.get_accState(child));}catch{}
        try{int x,y,w,h;acc.accLocation(out x,out y,out w,out h,child);item.rect=new[]{x,y,w,h};}catch{}
        Array.Resize(ref items,items.Length+1);items[items.Length-1]=item;
    }
    static void Visit(Accessibility.IAccessible acc,int depth) {
        if(depth>5)return;
        ReadItem(acc,0,depth);
        for(int i=1;i<=acc.accChildCount;i++) {
            Accessibility.IAccessible nested=null;
            try{nested=acc.get_accChild(i) as Accessibility.IAccessible;}catch{}
            if(nested!=null)Visit(nested,depth+1);else ReadItem(acc,i,depth+1);
        }
    }
    public static Item[] Read(IntPtr handle) {
        var id=new Guid("618736e0-3c3d-11cf-810c-00aa00389b71");Accessibility.IAccessible acc;
        int code=AccessibleObjectFromWindow(handle,0xfffffffc,ref id,out acc);items=new Item[0];
        if(code==0 && acc!=null)Visit(acc,0);return items;
    }
}
"@
}
function Get-ReferenceAccessibleItems([IntPtr]$Handle) { return [ReferenceAccessibility]::Read($Handle) }

function Save-ReferenceMenuTree([IntPtr]$Handle, [uint32]$ProcessId, [string]$Directory, [string]$Prefix, [int]$Depth = 0) {
    $raw=@(Get-ReferenceAccessibleItems $Handle)
    if(-not $raw.Count){throw "MSAA 菜单读取失败: $Prefix"}
    $bitmap=Get-ReferenceBitmap $Handle
    $bitmap.Save((Join-Path $Directory "$Prefix.png"),[Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $items=[Collections.Generic.List[object]]::new()
    $index=0
    foreach($item in $raw | Where-Object depth -eq 1) {
        if(-not [ReferenceNative]::IsWindowVisible($Handle)){throw "父菜单意外关闭: $Prefix"}
        $parts=@($item.name -split "`t",2)
        $entry=[ordered]@{
            name=$parts[0]; mnemonic=$item.shortcut
            shortcut=$(if($parts.Count -gt 1){$parts[1]}else{$null})
            separator=($item.role -eq 21); enabled=(($item.state -band 1) -eq 0)
            checked=(($item.state -band 16) -ne 0)
            hasSubmenu=(($item.state -band 1073741824) -ne 0)
            msaaState=$item.state; source="$Prefix.png"; children=@()
            dynamicExternal=($parts[0] -eq 'Explorer')
        }
        if($entry.hasSubmenu -and $entry.enabled -and -not $entry.dynamicExternal -and $Depth -lt 4) {
            # 只展开带 HASPOPUP 状态的菜单，不执行叶子命令。
            $existing=@([ReferenceNative]::Windows($ProcessId))
            $r=$item.rect
            [void][ReferenceNative]::SetCursorPos(($r[0]+[int]($r[2]/2)),($r[1]+[int]($r[3]/2)))
            Start-Sleep -Milliseconds 600
            $child=Get-ReferenceWindows $ProcessId | Where-Object { $_.class -match 'Popup|32768' -and $_.handle -notin $existing } | Select-Object -First 1
            if(-not $child) {
                [ReferenceNative]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
                [ReferenceNative]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
                Start-Sleep -Milliseconds 500
                $child=Get-ReferenceWindows $ProcessId | Where-Object { $_.class -match 'Popup|32768' -and $_.handle -notin $existing } | Select-Object -First 1
            }
            if(-not $child){throw "子菜单未出现: $Prefix / $($entry.name)"}
            $tree=Save-ReferenceMenuTree $child.handle $ProcessId $Directory "$Prefix-$index" ($Depth+1)
            $entry.children=@($tree.items)
            [Windows.Forms.SendKeys]::SendWait('{ESC}')
            Start-Sleep -Milliseconds 150
        }
        $items.Add($entry)
        $index++
    }
    $tree=[ordered]@{name=$raw[0].name;source="$Prefix.png";screenRect=$raw[0].rect;items=@($items)}
    $tree | ConvertTo-Json -Depth 40 | Set-Content (Join-Path $Directory "$Prefix.tree.json") -Encoding utf8
    return $tree
}
