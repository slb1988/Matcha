"""Read only the named table settings RTTI; never scans other files/configuration.
Usage: python inspect_table_rtti.py <explicit executable>
No execution, patching, authorization inspection, or install-directory writes.
"""
import pathlib, struct, sys
data = pathlib.Path(sys.argv[1]).read_bytes()
pe = struct.unpack_from("<I", data, 0x3C)[0]
count = struct.unpack_from("<H", data, pe + 6)[0]
opt_size = struct.unpack_from("<H", data, pe + 20)[0]
base = struct.unpack_from("<Q", data, pe + 24 + 24)[0]
sections = []
for i in range(count):
    pos = pe + 24 + opt_size + i * 40
    vs, va, size, raw = struct.unpack_from("<IIII", data, pos + 8)
    sections.append((va, max(vs, size), raw))
def offset(va):
    rva = va - base
    for start, length, raw in sections:
        if start <= rva < start + length:
            return raw + rva - start
    raise ValueError(f"unmapped {va:#x}")
def u64(pos): return struct.unpack_from("<Q", data, pos)[0]
def short(pos):
    size = data[pos]
    return data[pos + 1:pos + 1 + size].decode("ascii", "replace"), pos + 1 + size
def typename(pointer):
    try:
        pos = offset(u64(offset(pointer)))
        return short(pos + 1)[0]
    except (ValueError,struct.error,IndexError): return "?"
if "--separator" in sys.argv[2:]:
    for address in [0x8d0c30,0x8d0b74]:
        p=offset(address); n=struct.unpack_from("<i",data,p-4)[0]
        print(hex(address),repr(data[p:p+2*n].decode("utf-16-le")))
    raise SystemExit(0)
if "--enums" in sys.argv[2:]:
    import re
    for name in [b"TAlignKind",b"TBcAlignKind",b"TLineAlignKind",b"TTableAlignKind"]:
        needle=bytes([3,len(name)])+name
        start=0
        while True:
            hit=data.find(needle,start)
            if hit<0:break
            start=hit+len(needle); p=start
            lo,hi=struct.unpack_from("<ii",data,p+1); print("ENUM_DATA", data[p:p+80].hex());p+=17
            labels=[]
            if 0<=hi-lo<30:
                for _ in range(hi-lo+1):
                    label,p=short(p);labels.append(label)
            print("ENUM",name.decode(),hex(hit),lo,hi,labels)
    raise SystemExit(0)
if "--serializer" in sys.argv[2:]:
    import hashlib, re, subprocess
    if hashlib.sha256(data).hexdigest() != "75ea69772796c909a6659f23037f779de3399eccaa2332969f6dcc28c7bf9826":
        raise SystemExit("review fixed address scope for this binary revision first")
    for start, end in [(0x1af2670,0x1af3518),(0x1af3b00,0x1af4378),(0x1280730,0x1280d00)]:
        assembly = subprocess.check_output(["C:/Program Files/LLVM/bin/llvm-objdump.exe","-d","--no-show-raw-insn",f"--start-address={start:#x}",f"--stop-address={end:#x}",sys.argv[1]],text=True)
        for line in assembly.splitlines():
            match = re.search(r"# (0x[0-9a-f]+)",line)
            if not match: continue
            address = int(match[1],16)
            try:
                pos=offset(address); length=struct.unpack_from("<i",data,pos-4)[0]
                if not 1<=length<=100: continue
                value=data[pos:pos+length*2].decode("utf-16-le")
                if all(c.isprintable() for c in value):
                    print("LITERAL",hex(address),"file",hex(pos),repr(value),"REFERENCED",line.strip().split(":")[0])
            except (ValueError,struct.error,UnicodeError): pass
    raise SystemExit(0)
if "--table-types" in sys.argv[2:]:
    import re
    for m in re.finditer(rb"TTable[A-Za-z0-9_]*(?:Rules|Col|Handling|Definition)[A-Za-z0-9_]*", data):
        start=m.start()
        if start>=2 and data[start-2]==7 and data[start-1]==len(m.group()):
            print("TABLE_SETTINGS_TYPE",m.group().decode("ascii"))
    raise SystemExit(0)
names = sys.argv[2:] or ["TTableRules"]
allowed = {"TTableRules", "TTableColHandling", "TTableColumn", "TTableColList", "TTableColRules", "TTableColHandlingList", "TTableColumns", "TTableRulesColumn", "TTableDefaultColHandling"}
for name in names:
    if name not in allowed and name != "TLineRules" and not (name.startswith("TTable") and any(s in name for s in ("Rules","Col","Handling","Definition"))): raise SystemExit("name outside table-settings allowlist")
    needle = bytes([7,len(name)]) + name.encode("ascii")
    found = []; start = 0
    while True:
        hit = data.find(needle,start)
        if hit < 0: break
        found.append(hit); start = hit + len(needle)
    print("CLASS",name,"RTTI offsets", [hex(x) for x in found])
    for hit in found:
        pos = hit + len(needle)
        vmt, parent = struct.unpack_from("<QQ",data,pos)
        count = struct.unpack_from("<H",data,pos+16)[0]
        unit, p = short(pos+18)
        direct = struct.unpack_from("<H",data,p)[0]; p += 2
        print("vmt",hex(vmt),"parent-info",hex(parent),typename(parent),"total",count,"unit",unit,"direct",direct)
        if direct > 100: raise SystemExit("unexpected property layout")
        print("VMT slots",[(hex(i),hex(u64(offset(vmt)+i))) for i in range(0,0x100,8)])
        for _ in range(direct):
            typ, get, set_, stored, index, default, nameindex = struct.unpack_from("<QQQQiiH",data,p)
            prop, end = short(p+42)
            print("PROPERTY",prop,"type",typename(typ),"typeptr",hex(typ),"get",hex(get),"set",hex(set_),"default",default)
            p = end
