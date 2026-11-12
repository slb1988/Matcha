//! `table-report layout:summary` / `/report=summary`：xlsx 与 CSV 的可读差异报告（格式契约见
//! `docs/features/reports.md` 的“表格差异报告”一节）。工作簿夹具由 XML + stored ZIP 现场生成。
use bc_core::hash::Crc32;
use bc_formats::registry::FormatRegistry;
use bc_script::parser::{shortcut_command, CommandError};
use bc_script::runner::{RunnerOptions, Streams};
use bc_script::{Command, ExitCode, Script, ScriptRunner};
use bc_vfs::VfsManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Minimal ZIP writer for stored (uncompressed) entries.
fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    fn u16le(out: &mut Vec<u8>, v: u16) {
        out.extend(v.to_le_bytes());
    }
    fn u32le(out: &mut Vec<u8>, v: u32) {
        out.extend(v.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let mut crc = Crc32::new();
        crc.update(data.as_bytes());
        let crc = crc.finish();
        u32le(&mut out, 0x04034b50);
        u16le(&mut out, 20);
        for _ in 0..4 {
            u16le(&mut out, 0);
        }
        u32le(&mut out, crc);
        u32le(&mut out, data.len() as u32);
        u32le(&mut out, data.len() as u32);
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(data.as_bytes());
        u32le(&mut central, 0x02014b50);
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        for _ in 0..4 {
            u16le(&mut central, 0);
        }
        u32le(&mut central, crc);
        u32le(&mut central, data.len() as u32);
        u32le(&mut central, data.len() as u32);
        u16le(&mut central, name.len() as u16);
        for _ in 0..4 {
            u16le(&mut central, 0);
        }
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend(name.as_bytes());
    }
    let offset = out.len();
    let size = central.len();
    out.extend(central);
    u32le(&mut out, 0x06054b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, entries.len() as u16);
    u16le(&mut out, entries.len() as u16);
    u32le(&mut out, size as u32);
    u32le(&mut out, offset as u32);
    u16le(&mut out, 0);
    out
}

/// One cell: numbers as `<v>`, everything else as an inline string.
fn cell(reference: &str, value: &str) -> String {
    if value.parse::<f64>().is_ok() {
        format!(r#"<c r="{reference}"><v>{value}</v></c>"#)
    } else {
        format!(
            r#"<c r="{reference}" t="inlineStr"><is><t xml:space="preserve">{value}</t></is></c>"#
        )
    }
}

fn worksheet(rows: &[&[&str]]) -> String {
    let mut xml = String::from(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for (r, row) in rows.iter().enumerate() {
        xml.push_str(&format!(r#"<row r="{}">"#, r + 1));
        for (c, value) in row.iter().enumerate() {
            let reference = format!("{}{}", (b'A' + c as u8) as char, r + 1);
            xml.push_str(&cell(&reference, value));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    xml
}

/// A workbook with the given (name, rows) sheets.
fn workbook(sheets: &[(&str, &[&[&str]])]) -> Vec<u8> {
    let mut entries: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.into()),
        ("_rels/.rels".into(), r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
    ];
    let mut list = String::new();
    let mut rels = String::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        let n = i + 1;
        list.push_str(&format!(
            r#"<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>"#
        ));
        rels.push_str(&format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#));
        entries.push((format!("xl/worksheets/sheet{n}.xml"), worksheet(rows)));
    }
    entries.push(("xl/workbook.xml".into(), format!(r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{list}</sheets></workbook>"#)));
    entries.push(("xl/_rels/workbook.xml.rels".into(), format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#)));
    let borrowed: Vec<(&str, String)> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.clone()))
        .collect();
    zip(&borrowed)
}

fn folder(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("matcha-table-report-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Run {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(dir: &Path, command: Command) -> Run {
    let script = Script::single(command, "/report");
    let mut runner = ScriptRunner::new(
        RunnerOptions::default(),
        Arc::new(VfsManager::local_only()),
        Arc::new(FormatRegistry::with_builtins()),
        dir.to_path_buf(),
    );
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = runner.run(
        &script,
        &mut Streams {
            stdout: &mut stdout,
            stderr: &mut stderr,
        },
    );
    Run {
        code,
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

fn report(dir: &Path, layout: &str, options: Option<&str>, left: &str, right: &str) -> Run {
    let left = dir.join(left).display().to_string();
    let right = dir.join(right).display().to_string();
    run(
        dir,
        shortcut_command(layout, options, None, &left, &right).unwrap(),
    )
}

#[test]
fn xlsx_report_lists_sheets_rows_and_cells() {
    let dir = folder("xlsx");
    // Enough shared ids for the automatic key inference to pick `id`.
    let left: &[&[&str]] = &[
        &["id", "name", "price"],
        &["1", "apple", "1"],
        &["2", "pear", "2"],
        &["3", "plum", "3"],
        &["5", "kiwi", "5"],
        &["6", "lime", "6"],
        &["7", "date", "7"],
        &["8", "sloe", "8"],
        &["9", "yuzu", "9"],
    ];
    let right: &[&[&str]] = &[
        &["id", "name", "price"],
        &["1", "apple", "1.5"],
        &["3", "plum", "3"],
        &["4", "fig|tree", "4"],
        &["5", "kiwi", "5"],
        &["6", "lime", "6"],
        &["7", "date", "7"],
        &["8", "sloe", "8"],
        &["9", "yuzu", "9"],
    ];
    let same: &[&[&str]] = &[&["k", "v"], &["a", "1"]];
    std::fs::write(
        dir.join("left.xlsx"),
        workbook(&[("Data", left), ("Notes", same), ("Old", same)]),
    )
    .unwrap();
    std::fs::write(
        dir.join("right.xlsx"),
        workbook(&[("Data", right), ("Notes", same), ("New", same)]),
    )
    .unwrap();
    // `/report=patch-unified` on workbooks is the table report, as `/report=summary`.
    let patch = report(&dir, "patch-unified", None, "left.xlsx", "right.xlsx");
    let summary = report(&dir, "summary", None, "left.xlsx", "right.xlsx");
    assert_eq!(patch.code, ExitCode::Success, "{}", patch.stderr);
    assert_eq!(patch.stdout, summary.stdout);
    let expected = format!(
        "MATCHA-TABLE-REPORT 1\n\
         left:   {}\n\
         right:  {}\n\
         result: different\n\
         sheets: 2 compared, 1 different, 1 same, 1 left-only, 1 right-only\n\
         {}\n\
         \n\
         === sheet \"Data\": different\n\
         key: id\n\
         rows: 1 changed, 0 unimportant, 1 left-only, 1 right-only, 6 same\n\
         ~ id=1  (left row 2, right row 2)\n    price (C2): 1 -> 1.5\n\
         - id=2  (left row 3)\n    id=2 | name=pear | price=2\n\
         + id=4  (right row 4)\n    id=4 | name=fig\\|tree | price=4\n\
         \n\
         === sheet \"Notes\": same\n\
         key: k\n\
         rows: 0 changed, 0 unimportant, 0 left-only, 0 right-only, 1 same\n\
         \n\
         === sheet \"Old\": left-only (1 rows x 2 columns)\n\
         \n\
         === sheet \"New\": right-only (1 rows x 2 columns)\n",
        dir.join("left.xlsx").display(),
        dir.join("right.xlsx").display(),
        bc_report::table::LEGEND,
    );
    assert_eq!(summary.stdout, expected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn identical_workbooks_report_same() {
    let dir = folder("same");
    let rows: &[&[&str]] = &[&["id", "v"], &["1", "x"]];
    std::fs::write(dir.join("a.xlsx"), workbook(&[("S", rows)])).unwrap();
    std::fs::write(dir.join("b.xlsx"), workbook(&[("S", rows)])).unwrap();
    let run = report(&dir, "patch", None, "a.xlsx", "b.xlsx");
    assert_eq!(run.code, ExitCode::Success, "{}", run.stderr);
    assert!(run.stdout.contains("\nresult: same\n"), "{}", run.stdout);
    assert!(run.stdout.ends_with("=== sheet \"S\": same\nkey: id\nrows: 0 changed, 0 unimportant, 0 left-only, 0 right-only, 1 same\n"), "{}", run.stdout);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn csv_summary_reports_column_changes_without_cell_refs() {
    let dir = folder("csv");
    std::fs::write(dir.join("a.csv"), "id,name\n1,old\n2,  same\n").unwrap();
    std::fs::write(dir.join("b.csv"), "id,name,note\n1,new,\n2,  same,x\n").unwrap();
    let run = report(&dir, "summary", None, "a.csv", "b.csv");
    assert_eq!(run.code, ExitCode::Success, "{}", run.stderr);
    let body = run.stdout.split_once("\n\n").unwrap().1;
    assert_eq!(
        body,
        "=== table: different\n\
         key: id\n\
         columns:\n  + \"note\" (right #3) inserted\n\
         rows: 2 changed, 0 unimportant, 0 left-only, 0 right-only, 0 same\n\
         ~ id=1  (left row 2, right row 2)\n    name: old -> new\n    note: (no column) -> (empty)\n\
         ~ id=2  (left row 3, right row 3)\n    note: (no column) -> x\n"
    );
    // CSV with a patch layout stays a text patch.
    let patch = report(&dir, "patch", None, "a.csv", "b.csv");
    assert!(patch.stdout.contains("\r\n1,3c1,3\r\n"), "{}", patch.stdout);
    assert!(!patch.stdout.starts_with("MATCHA-TABLE-REPORT"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shortcut_routing_and_errors() {
    let table = |layout: &str, l: &str, r: &str| {
        matches!(
            shortcut_command(layout, None, None, l, r).unwrap(),
            Command::TableReport(_)
        )
    };
    assert!(table("patch-unified", "a.xlsx", "b.xlsx"));
    assert!(table("PATCH", "a.XLSM", "b.txt"));
    assert!(table("summary", "a.csv", "b.csv"));
    assert!(!table("patch", "a.csv", "b.csv"));
    let e: CommandError =
        shortcut_command("side-by-side", None, None, "a.xlsx", "b.xlsx").unwrap_err();
    assert!(e.message.contains("side-by-side"), "{}", e.message);

    let dir = folder("errors");
    std::fs::write(dir.join("a.xlsx"), b"not a zip").unwrap();
    std::fs::write(dir.join("b.xlsx"), b"not a zip").unwrap();
    let run = report(&dir, "summary", None, "a.xlsx", "missing.xlsx");
    assert_eq!(
        run.code,
        ExitCode::ScriptLoadFileOrFolderFailed,
        "{}",
        run.stderr
    );
    let run = report(&dir, "summary", None, "a.xlsx", "b.xlsx");
    assert_eq!(
        run.code,
        ExitCode::ScriptLoadFileOrFolderFailed,
        "{}",
        run.stderr
    );
    assert!(run.stderr.starts_with("cannot read "), "{}", run.stderr);
    assert!(run.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn title_row_labels_lettered_columns() {
    let dir = folder("title-row");
    // All-text columns: the header is not detected, so columns are named by letter.
    let left: &[&[&str]] = &[
        &["ID", "Name"],
        &["x1", "WaterCrab"],
        &["x2", "Slime"],
        &["x3", "Bat"],
    ];
    let right: &[&[&str]] = &[
        &["ID", "Name"],
        &["x1", "Chameleon"],
        &["x2", "Slime"],
        &["x3", "Bat"],
    ];
    std::fs::write(dir.join("a.xlsx"), workbook(&[("S", left)])).unwrap();
    std::fs::write(dir.join("b.xlsx"), workbook(&[("S", right)])).unwrap();
    let plain = report(&dir, "summary", None, "a.xlsx", "b.xlsx");
    assert!(
        plain
            .stdout
            .contains("\n    B (B2): WaterCrab -> Chameleon\n"),
        "{}",
        plain.stdout
    );
    let titled = report(&dir, "summary", Some("title-row:1"), "a.xlsx", "b.xlsx");
    assert_eq!(titled.code, ExitCode::Success, "{}", titled.stderr);
    let body = titled.stdout.split_once("\n\n").unwrap().1;
    assert_eq!(
        body,
        "=== sheet \"S\": different\n\
         key: A[ID]\n\
         rows: 1 changed, 0 unimportant, 0 left-only, 0 right-only, 3 same\n\
         ~ A[ID]=x1  (left row 2, right row 2)\n    Name (B2): WaterCrab -> Chameleon\n"
    );

    // A detected header already names the columns: title-row:1 changes nothing.
    let header: &[&[&str]] = &[&["id", "v"], &["1", "a"], &["2", "b"], &["3", "c"]];
    let header2: &[&[&str]] = &[&["id", "v"], &["1", "a"], &["2", "B"], &["3", "c"]];
    std::fs::write(dir.join("h1.xlsx"), workbook(&[("S", header)])).unwrap();
    std::fs::write(dir.join("h2.xlsx"), workbook(&[("S", header2)])).unwrap();
    let plain = report(&dir, "summary", None, "h1.xlsx", "h2.xlsx");
    let titled = report(&dir, "summary", Some("title-row:1"), "h1.xlsx", "h2.xlsx");
    assert_eq!(plain.stdout, titled.stdout);
    // …while a later row is appended to the header name.
    let titled = report(&dir, "summary", Some("title-row:2"), "h1.xlsx", "h2.xlsx");
    assert!(
        titled
            .stdout
            .contains("\n~ id[1]=2  (left row 3, right row 3)\n    v[a] (B3): b -> B\n"),
        "{}",
        titled.stdout
    );

    let e = shortcut_command("summary", Some("title-row:0"), None, "a.xlsx", "b.xlsx").unwrap_err();
    assert!(e.message.contains("title-row"), "{}", e.message);
    let _ = std::fs::remove_dir_all(&dir);
}
