"""Deterministic UTF-8 CSV corpus and temporary OOXML fixtures; stdlib only."""
import argparse,csv,io,json,pathlib,zipfile,xml.sax.saxutils as xml
ROOT=pathlib.Path(__file__).resolve().parents[2]/"crates/bc-core/tests/golden/table"
def xlsx(source,target,multiple=False):
    rows=list(csv.reader(pathlib.Path(source).read_text(encoding="utf-8").splitlines(keepends=True)))
    sheet=['<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>']
    for r,row in enumerate(rows,1):
        sheet.append(f'<row r="{r}">')
        for c,value in enumerate(row):
            name="";n=c+1
            while n:n,rem=divmod(n-1,26);name=chr(65+rem)+name
            ref=f'{name}{r}'
            try:
                number=float(value)
                if not value.strip(): raise ValueError()
                sheet.append(f'<c r="{ref}"><v>{number}</v></c>')
            except ValueError:
                if value in ("TRUE","FALSE"):sheet.append(f'<c r="{ref}" t="b"><v>{int(value=="TRUE")}</v></c>')
                elif value in ("#N/A","#VALUE!"):sheet.append(f'<c r="{ref}" t="e"><v>{xml.escape(value)}</v></c>')
                else:sheet.append(f'<c r="{ref}" t="inlineStr"><is><t xml:space="preserve">{xml.escape(value)}</t></is></c>')
        sheet.append('</row>')
    sheet.append('</sheetData></worksheet>')
    names=["Data","Audit"] if multiple else ["Data"]
    with zipfile.ZipFile(target,"w",zipfile.ZIP_STORED) as out:
        out.writestr("[Content_Types].xml",'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'+''.join(f'<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>' for i in range(1,len(names)+1))+'</Types>')
        out.writestr("_rels/.rels",'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>')
        out.writestr("xl/workbook.xml",'<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>'+''.join(f'<sheet name="{name}" sheetId="{i}" r:id="rId{i}"/>' for i,name in enumerate(names,1))+'</sheets></workbook>')
        out.writestr("xl/_rels/workbook.xml.rels",'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'+''.join(f'<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>' for i in range(1,len(names)+1))+'</Relationships>')
        for i in range(1,len(names)+1):out.writestr(f"xl/worksheets/sheet{i}.xml",''.join(sheet))
def corpus():
    cases=[]
    def add(name,left,right,**props):
        defaults=dict(keys=[0],sort=False,strategy="Quick",handling={},header="Data")
        defaults.update(props);cases.append((name,left,right,defaults))
    head="id,name,price\n"
    add("001-identical",head+"7,apple,1.0\n",head+"7,apple,1.0\n")
    add("002-key-value-change",head+"7,apple,1.0\n",head+"7,apple,1.2\n")
    add("003-insert-middle",head+"1,a,1\n3,c,3\n",head+"1,a,1\n2,b,2\n3,c,3\n")
    add("004-delete-middle",head+"1,a,1\n2,b,2\n3,c,3\n",head+"1,a,1\n3,c,3\n")
    add("005-empty-left","",head+"1,a,1\n")
    add("006-empty-right",head+"1,a,1\n","")
    add("007-sorted-keys",head+"3,c,3\n1,a,1\n2,b,2\n",head+"1,a,1\n2,b,2\n3,c,3\n",sort=True)
    add("008-duplicate-keys",head+"3,a,1\n3,b,2\n",head+"3,a,1\n")
    add("009-composite",head+"1,a,1\n1,b,2\n",head+"1,a,3\n1,b,2\n",keys=[0,1])
    add("010-all-columns",head+"1,a,1\n2,b,2\n",head+"1,a,3\n2,b,2\n",keys=[])
    add("011-case-ignored",head+"1,APPLE,1\n",head+"1,apple,1\n",handling={"1":{"AsText":True,"StrCaseInsig":True}})
    add("012-whitespace-ignored",head+'1,"a  b",1\n',head+'1,"a b",1\n',handling={"1":{"AsText":True,"StrWSInsig":True}})
    add("013-numeric-tolerance",head+"1,a,1.001\n",head+"1,a,1.005\n",handling={"2":{"NumTolerance":0.01}})
    add("014-unimportant",head+"1,old,1\n",head+"1,new,1\n",handling={"1":{"Insig":True}})
    add("015-text-type","id,value\n1,001\n","id,value\n1,1\n",handling={"1":{"AsText":True}})
    add("016-booleans","id,value\n1,TRUE\n2,FALSE\n","id,value\n1,TRUE\n2,TRUE\n")
    add("017-quoted-comma",head+'1,"a,b",1\n',head+'1,"a,c",1\n')
    add("018-unicode",head+"1,茶,1\n2,猫,2\n",head+"1,抹茶,1\n2,猫,2\n")
    add("019-many-changes",head+"1,a,1\n2,b,2\n3,c,3\n",head+"1,x,4\n2,y,5\n3,z,6\n")
    add("020-extra-column","id,name\n1,a\n","id,name,note\n1,a,new\n")
    add("021-case-significant",head+"1,Apple,1\n",head+"1,apple,1\n",handling={"1":{"AsText":True}})
    add("022-date-tolerance","id,date\n1,1/1/2026\n","id,date\n1,1/2/2026\n",handling={"1":{"DateTimeTolerance":86400}},date_order="Mdy",date_separator="/")
    add("023-sorted-duplicates",head+"2,b,2\n1,a,1\n1,c,3\n",head+"1,a,1\n1,c,3\n2,b,2\n",sort=True)
    add("024-unaligned",head+"1,a,1\n2,b,2\n",head+"2,b,2\n1,a,1\n",strategy="None")
    add("025-outside-tolerance",head+"1,a,1.00\n",head+"1,a,1.02\n",handling={"2":{"NumTolerance":0.01}})
    add("026-empty-fields",head+"1,,\n2,a,2\n",head+"1,,\n2,,2\n")
    add("027-quoted-newline",head+'1,"line1\nline2",1\n',head+'1,"line1\nline3",1\n')
    add("028-xlsx-values",head+"1,a,1\n2,b,2\n",head+"1,a,1.5\n2,b,2\n",xlsx=True)
    add("029-xlsx-booleans-errors","id,flag,error\n1,TRUE,#N/A\n","id,flag,error\n1,FALSE,#VALUE!\n",xlsx=True)
    add("030-xlsx-multiple",head+"1,a,1\n2,b,2\n",head+"1,a,2\n2,b,2\n",xlsx=True,multiple=True)
    for name,left,right,props in cases:
        path=ROOT/name;path.mkdir(parents=True,exist_ok=True)
        (path/"left.csv").write_text(left,encoding="utf-8",newline="")
        (path/"right.csv").write_text(right,encoding="utf-8",newline="")
        (path/"case.json").write_text(json.dumps(props,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"generated {len(cases)} input cases at {ROOT}")
if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--xlsx",nargs=2);parser.add_argument("--multiple",action="store_true");args=parser.parse_args()
    if args.xlsx:xlsx(*args.xlsx,args.multiple)
    else:corpus()
