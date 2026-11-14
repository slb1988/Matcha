//! Table merge golden corpus: `tests/golden/table_merge/<case>/`.
//!
//! Each case has a `case.json`:
//!
//! * text-table cases ship `left.*`, `base.*`, `right.*` (csv/tsv) and, when the merge
//!   succeeds, `expected.*` compared byte for byte;
//! * workbook cases describe the three workbooks in `case.json` (`left` / `base` /
//!   `right`, see [`build_workbook`]); the runner builds them with `rust_xlsxwriter`
//!   (no binaries in the repository). Values are compared with `expected.txt`
//!   (`## <sheet>` then one `a | b | c` line per row), and `expect` lists formulas,
//!   cell formats and sheet layout that must survive.
//!
//! Common keys: `keys` (1-based, `/keys=`), `iu`, `favor` (`left`/`right`), `output`
//! (output file name), `exit` (0 or 101), `conflicts`, `key_label`.
//!
//! `MATCHA_BLESS=1 cargo test -p bc-script --test table_merge_golden` rewrites the
//! expected files from the current output (review the diff before committing).
//! `MATCHA_GOLDEN_KEEP=<dir>` copies every case's inputs and output there (to open them in
//! Excel by hand).
use bc_core::cancel::NeverCancel;
use bc_engine_table::{workbook::Workbook as Reader, xlsx_patch};
use bc_script::{
    cli::{Favor, MergeSpec},
    table_merge::{auto_merge_tables, options},
    ExitCode,
};
use rust_xlsxwriter::{Color, Format, Workbook};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/table_merge")
}

fn bless() -> bool {
    std::env::var_os("MATCHA_BLESS").is_some_and(|v| v == "1")
}

fn format_of(style: &Value) -> Format {
    let mut format = Format::new();
    if style["bold"].as_bool() == Some(true) {
        format = format.set_bold();
    }
    if style["italic"].as_bool() == Some(true) {
        format = format.set_italic();
    }
    if let Some(fill) = style["fill"].as_str() {
        format = format.set_background_color(Color::RGB(u32::from_str_radix(fill, 16).unwrap()));
    }
    if let Some(code) = style["num_fmt"].as_str() {
        format = format.set_num_format(code);
    }
    format
}

fn column_of(name: &str) -> u16 {
    name.bytes().fold(0u16, |n, b| {
        n * 26 + u16::from(b.to_ascii_uppercase() - b'A' + 1)
    }) - 1
}

fn cell_of(name: &str) -> (u32, u16) {
    let split = name.find(|c: char| c.is_ascii_digit()).unwrap();
    (
        name[split..].parse::<u32>().unwrap() - 1,
        column_of(&name[..split]),
    )
}

/// Days since 1899-12-30 for `YYYY-MM-DD`.
fn serial(date: &str) -> f64 {
    let parts: Vec<i64> = date.split('-').map(|p| p.parse().unwrap()).collect();
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    // Days from civil (Howard Hinnant), relative to 1970-01-01.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as f64 + 25569.0
}

/// Workbook from a description:
/// `{"sheets": [{"name", "rows": [[cell…]…], "widths": {"B": 30}, "freeze": [1, 0],
///   "merge": [{"range": "A7:B7", "value": "Total", "style": "total"}]}],
///   "names": {"Qty": "=Data!$C$2:$C$6"}}`; a cell is `null`, a number, a bool, a
/// string (`=…` formula, `date:YYYY-MM-DD`) or `{"v": <cell>, "s": "<style>"}`.
fn build_workbook(desc: &Value, styles: &Value, path: &Path) {
    let mut book = Workbook::new();
    let style = |name: Option<&str>| name.map(|n| format_of(&styles[n]));
    let date = Format::new().set_num_format("yyyy-mm-dd");
    for sheet_desc in desc["sheets"].as_array().unwrap() {
        let sheet = book.add_worksheet();
        sheet
            .set_name(sheet_desc["name"].as_str().unwrap())
            .unwrap();
        if let Some(widths) = sheet_desc["widths"].as_object() {
            for (column, width) in widths {
                sheet
                    .set_column_width(column_of(column), width.as_f64().unwrap())
                    .unwrap();
            }
        }
        if let Some(freeze) = sheet_desc["freeze"].as_array() {
            sheet
                .set_freeze_panes(
                    freeze[0].as_u64().unwrap() as u32,
                    freeze[1].as_u64().unwrap() as u16,
                )
                .unwrap();
        }
        for (r, row) in sheet_desc["rows"].as_array().unwrap().iter().enumerate() {
            for (c, cell) in row.as_array().unwrap().iter().enumerate() {
                let (value, format) = match cell {
                    Value::Object(o) => (o["v"].clone(), style(o.get("s").and_then(Value::as_str))),
                    other => (other.clone(), None),
                };
                let (r, c) = (r as u32, c as u16);
                match value {
                    Value::Null => {
                        if let Some(f) = &format {
                            sheet.write_blank(r, c, f).unwrap();
                        }
                    }
                    Value::Bool(b) => {
                        match &format {
                            Some(f) => sheet.write_boolean_with_format(r, c, b, f),
                            None => sheet.write_boolean(r, c, b),
                        }
                        .unwrap();
                    }
                    Value::Number(n) => {
                        let n = n.as_f64().unwrap();
                        match &format {
                            Some(f) => sheet.write_number_with_format(r, c, n, f),
                            None => sheet.write_number(r, c, n),
                        }
                        .unwrap();
                    }
                    Value::String(s) if s.starts_with('=') => {
                        match &format {
                            Some(f) => sheet.write_formula_with_format(r, c, s.as_str(), f),
                            None => sheet.write_formula(r, c, s.as_str()),
                        }
                        .unwrap();
                    }
                    Value::String(s) if s.starts_with("date:") => {
                        sheet
                            .write_number_with_format(
                                r,
                                c,
                                serial(&s[5..]),
                                format.as_ref().unwrap_or(&date),
                            )
                            .unwrap();
                    }
                    Value::String(s) => {
                        match &format {
                            Some(f) => sheet.write_string_with_format(r, c, s.as_str(), f),
                            None => sheet.write_string(r, c, s.as_str()),
                        }
                        .unwrap();
                    }
                    other => panic!("unsupported cell {other}"),
                }
            }
        }
        for merge in sheet_desc["merge"].as_array().into_iter().flatten() {
            let (a, b) = merge["range"].as_str().unwrap().split_once(':').unwrap();
            let ((r1, c1), (r2, c2)) = (cell_of(a), cell_of(b));
            let format = style(merge["style"].as_str()).unwrap_or_default();
            sheet
                .merge_range(
                    r1,
                    c1,
                    r2,
                    c2,
                    merge["value"].as_str().unwrap_or(""),
                    &format,
                )
                .unwrap();
        }
    }
    for (name, formula) in desc["names"].as_object().into_iter().flatten() {
        book.define_name(name, formula.as_str().unwrap()).unwrap();
    }
    book.save(path).unwrap();
}

fn workbook_text(path: &Path) -> String {
    let mut book = Reader::open(path, Arc::new(NeverCancel)).unwrap();
    let mut out = String::new();
    for name in book.sheet_names().to_vec() {
        let sheet = book.load_sheet(&name).unwrap();
        out.push_str(&format!("## {name}\n"));
        for r in 0..sheet.row_count() {
            let row: Vec<String> = (0..sheet.column_count())
                .map(|c| {
                    sheet
                        .cell(r, c)
                        .map(|cell| cell.display())
                        .unwrap_or_default()
                })
                .collect();
            out.push_str(row.join(" | ").trim_end_matches(" |").trim_end());
            out.push('\n');
        }
    }
    out
}

fn formula_at(path: &Path, at: &str) -> Option<String> {
    let (sheet, cell) = at.split_once('!').unwrap();
    let mut book = Reader::open(path, Arc::new(NeverCancel)).unwrap();
    let sheet = book.load_sheet(sheet).unwrap();
    let (r, c) = cell_of(cell);
    sheet
        .cell(r as usize, c as usize)?
        .formula
        .as_deref()
        .map(str::to_string)
}

struct Outcome {
    failures: Vec<String>,
}

impl Outcome {
    fn check(&mut self, case: &str, ok: bool, what: impl FnOnce() -> String) {
        if !ok {
            self.failures.push(format!("{case}: {}", what()));
        }
    }
}

fn run_case(dir: &Path, outcome: &mut Outcome) {
    let case = dir.file_name().unwrap().to_string_lossy().into_owned();
    let spec_text = std::fs::read_to_string(dir.join("case.json")).unwrap();
    let spec: Value =
        serde_json::from_str(&spec_text).unwrap_or_else(|e| panic!("{case}/case.json: {e}"));
    let work = std::env::temp_dir().join(format!("matcha-golden-tm-{case}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let workbook = spec.get("base").is_some_and(Value::is_object);
    let mut paths: HashMap<&str, PathBuf> = HashMap::new();
    for side in ["left", "base", "right"] {
        let path = if workbook {
            let path = work.join(format!("{side}.xlsx"));
            build_workbook(&spec[side], &spec["styles"], &path);
            path
        } else {
            let input = std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| p.file_stem().is_some_and(|s| s == side))
                .unwrap_or_else(|| panic!("{case}: no {side}.*"));
            let path = work.join(input.file_name().unwrap());
            std::fs::copy(&input, &path).unwrap();
            path
        };
        paths.insert(side, path);
    }
    let output =
        work.join(
            spec["output"]
                .as_str()
                .unwrap_or(if workbook { "out.xlsx" } else { "out.csv" }),
        );
    let merge = MergeSpec {
        left: paths["left"].clone(),
        right: paths["right"].clone(),
        center: paths["base"].clone(),
        output: Some(output.clone()),
    };
    let keys = spec["keys"]
        .as_array()
        .map(|k| k.iter().map(|n| n.as_u64().unwrap() as usize - 1).collect());
    let favor = match spec["favor"].as_str() {
        Some("left") => Favor::Left,
        Some("right") => Favor::Right,
        _ => Favor::None,
    };
    let report = match auto_merge_tables(
        &merge,
        &options(keys, spec["iu"].as_bool() == Some(true), favor),
    ) {
        Ok(report) => report,
        Err(e) => {
            outcome.check(&case, false, || format!("merge failed: {e}"));
            return;
        }
    };
    let exit = spec["exit"].as_i64().unwrap_or(0);
    outcome.check(&case, i64::from(report.code.code()) == exit, || {
        format!(
            "exit {} ({}), expected {exit}",
            report.code.code(),
            report.summary()
        )
    });
    if let Some(n) = spec["conflicts"].as_u64() {
        outcome.check(&case, report.conflicts as u64 == n, || {
            format!("{} conflict section(s), expected {n}", report.conflicts)
        });
    }
    if let Some(label) = spec["key_label"].as_str() {
        outcome.check(&case, report.keys.as_deref() == Some(label), || {
            format!("keys {:?}, expected {label:?}", report.keys)
        });
    }
    if let Some(keep) = std::env::var_os("MATCHA_GOLDEN_KEEP") {
        let target = Path::new(&keep).join(&case);
        std::fs::create_dir_all(&target).unwrap();
        for entry in std::fs::read_dir(&work).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, target.join(path.file_name().unwrap())).unwrap();
        }
    }
    if report.code != ExitCode::Success {
        outcome.check(&case, !output.exists(), || {
            "output written despite conflicts".into()
        });
        let _ = std::fs::remove_dir_all(&work);
        return;
    }
    if workbook {
        let actual = workbook_text(&output);
        let expected_path = dir.join("expected.txt");
        if bless() {
            std::fs::write(&expected_path, &actual).unwrap();
        }
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        outcome.check(&case, actual == expected, || {
            format!("values differ:\n--- expected\n{expected}--- actual\n{actual}")
        });
        let package = std::fs::read(&output).unwrap();
        let expect = &spec["expect"];
        for (at, formula) in expect["formulas"].as_object().into_iter().flatten() {
            let actual = formula_at(&output, at);
            outcome.check(&case, actual.as_deref() == formula.as_str(), || {
                format!("{at}: formula {actual:?}, expected {formula}")
            });
        }
        for (at, want) in expect["formats"].as_object().into_iter().flatten() {
            let (sheet, cell) = at.split_once('!').unwrap();
            let got = xlsx_patch::cell_format(&package, sheet, cell).unwrap();
            let ok = want["bold"].as_bool().is_none_or(|b| b == got.bold)
                && want["italic"].as_bool().is_none_or(|b| b == got.italic)
                && want["fill"].as_str().is_none_or(|f| {
                    got.fill
                        .as_deref()
                        .is_some_and(|g| g.eq_ignore_ascii_case(f))
                })
                && want["num_fmt"]
                    .as_str()
                    .is_none_or(|f| got.num_fmt.as_deref() == Some(f));
            outcome.check(&case, ok, || {
                format!("{at}: format {got:?}, expected {want}")
            });
        }
        for (sheet, want) in expect["layout"].as_object().into_iter().flatten() {
            let got = xlsx_patch::sheet_layout(&package, sheet).unwrap();
            let list = |v: &Value| -> Option<Vec<String>> {
                v.as_array()
                    .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
            };
            let ok = list(&want["merged"]).is_none_or(|m| m == got.merged)
                && list(&want["widths"]).is_none_or(|w| w.iter().all(|c| got.widths.contains(c)))
                && want["frozen"].as_bool().is_none_or(|f| f == got.frozen);
            outcome.check(&case, ok, || {
                format!("{sheet}: layout {got:?}, expected {want}")
            });
        }
    } else {
        let expected_path = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.file_stem().is_some_and(|s| s == "expected"))
            .unwrap_or_else(|| {
                dir.join(format!(
                    "expected.{}",
                    output.extension().unwrap().to_string_lossy()
                ))
            });
        let actual = std::fs::read(&output).unwrap();
        if bless() {
            std::fs::write(&expected_path, &actual).unwrap();
        }
        let expected = std::fs::read(&expected_path).unwrap_or_default();
        outcome.check(&case, actual == expected, || {
            format!(
                "output differs:\n--- expected\n{}--- actual\n{}",
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&actual)
            )
        });
    }
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn table_merge_golden_corpus() {
    let mut cases: Vec<PathBuf> = std::fs::read_dir(corpus())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("case.json").is_file())
        .collect();
    cases.sort();
    assert!(cases.len() >= 20, "corpus has {} cases", cases.len());
    let mut outcome = Outcome {
        failures: Vec::new(),
    };
    for case in &cases {
        run_case(case, &mut outcome);
    }
    assert!(
        outcome.failures.is_empty(),
        "{} of {} cases failed:\n{}",
        outcome.failures.len(),
        cases.len(),
        outcome.failures.join("\n")
    );
}
