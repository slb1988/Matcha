//! 4.4 多 sheet：标签色点与切换、手动配对、同文件两 sheet、左文件为源、
//! 剪贴板对 workbook 侧的分隔符探测（N7）、切换取消扫描后的恢复（N2）。
//! xlsx 夹具由代码生成的最小 zip 承载，不入库二进制。
use super::*;
use bc_core::hash::Crc32;
use bc_engine_table::compare::Importance;
use bc_engine_table::sheets::{SheetPair, SheetPairs};

// 最小 ZIP（stored 条目）：与 bc-engine-table tests/workbooks.rs 同手法。
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

fn column_letter(mut column: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (column % 26) as u8) as char);
        column /= 26;
        if column == 0 {
            return s;
        }
        column -= 1;
    }
}

/// sheets: (名称, "a,b\nc,d" 行文本) → 最小 xlsx；全部 inlineStr。
pub(super) fn xlsx(sheets: &[(&str, &str)]) -> Vec<u8> {
    let mut entries = vec![
        ("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.to_string()),
        ("_rels/.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_string()),
    ];
    let sheet_tags = sheets
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            format!(
                r#"<sheet name="{name}" sheetId="{}" r:id="rId{}"/>"#,
                i + 1,
                i + 1
            )
        })
        .collect::<String>();
    entries.push(("xl/workbook.xml", format!(r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{sheet_tags}</sheets></workbook>"#)));
    let rels = sheets
        .iter()
        .enumerate()
        .map(|(i, _)| {
            format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#, i + 1, i + 1)
        })
        .collect::<String>();
    entries.push(("xl/_rels/workbook.xml.rels", format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#)));
    let mut owned = Vec::new();
    for (i, (_, content)) in sheets.iter().enumerate() {
        let rows = content
            .lines()
            .enumerate()
            .map(|(r, line)| {
                let cells = line
                    .split(',')
                    .enumerate()
                    .map(|(c, text)| {
                        if !text.is_empty() && text.parse::<f64>().is_ok() {
                            format!(r#"<c r="{}{}"><v>{text}</v></c>"#, column_letter(c), r + 1)
                        } else {
                            format!(
                                r#"<c r="{}{}" t="inlineStr"><is><t>{text}</t></is></c>"#,
                                column_letter(c),
                                r + 1
                            )
                        }
                    })
                    .collect::<String>();
                format!(r#"<row r="{}">{cells}</row>"#, r + 1)
            })
            .collect::<String>();
        owned.push((
            format!("xl/worksheets/sheet{}.xml", i + 1).leak() as &str,
            format!(r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#),
        ));
    }
    let mut all = entries;
    all.extend(owned);
    zip(&all)
}

pub(super) fn temp_xlsx(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("matcha-wave7-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn wait_scanned(v: &mut TableCompareView) {
    let start = Instant::now();
    while v.states.iter().any(Option::is_none) {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn sheet_tabs_show_state_dots_and_switching_preserves_cached_edits() {
    let left = temp_xlsx(
        "tabs-left.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,total\n1,10")]),
    );
    let right = temp_xlsx(
        "tabs-right.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,total\n1,12")]),
    );
    let mut v = open_files(&left, &right);
    assert_eq!(v.pairs.0.len(), 2);
    wait_scanned(&mut v);
    assert_eq!(v.states[0], Some(Importance::Same));
    assert_eq!(v.states[1], Some(Importance::Important));
    // 标签带状态色点（相同 = 弱化文字色，有差异 = 差异色），Summary 标签可点击切换。
    let tabs = super::super::sheet_tabs::tabs(
        &v.names,
        &v.pairs,
        &v.states,
        &bc_session::prefs::Prefs::default().colors,
    );
    assert_eq!(tabs[0].dot, Some(crate::chrome::theme::TEXT_MUTED));
    assert_ne!(tabs[1].dot, tabs[0].dot);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Summary"));
    ready(&mut v);
    assert_eq!(v.active_pair, 1);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "10"
    );
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    // 在 Summary 编辑，切回 Data 再切回：编辑与历史随缓存 sheet 保留。
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("10".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Data"));
    ready(&mut v);
    assert_eq!(v.active_pair, 0);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Summary"));
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "10");
    assert!(
        p.history[1].can_undo(),
        "cached sheet pair keeps its history"
    );
    assert_eq!(p.cache.statistics.same, 2);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn same_file_two_sheets_via_manual_pairing_and_left_source_reordering() {
    let book = temp_xlsx(
        "same-file.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,total\n1,10")]),
    );
    let mut v = open_files(&book, &book.clone());
    assert_eq!(v.pairs.0.len(), 2);
    // 表头行保留为数据：同名对全同 = 表头 + 数据两行。
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 2);
    // Sheets 手动配对：把右侧 Summary 移到第一对 → 同文件两 sheet 交叉比较。
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft
        .pairs
        .move_to(1, 0, bc_engine_table::columns::Side::Right)
        .unwrap();
    assert_eq!(
        draft.pairs.0,
        vec![
            SheetPair {
                left: Some(0),
                right: Some(1)
            },
            SheetPair {
                left: Some(1),
                right: Some(0)
            }
        ]
    );
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    // 手动配对必须在设置重载后保留，而不是被按名推导覆盖。
    assert_eq!(
        v.pairs.0,
        vec![
            SheetPair {
                left: Some(0),
                right: Some(1)
            },
            SheetPair {
                left: Some(1),
                right: Some(0)
            }
        ]
    );
    // Data ↔ Summary：键 7 vs 1 不配对 → 左右孤儿。
    let s = v.pair.as_ref().unwrap().cache.statistics;
    assert_eq!(s.left_only, 1);
    assert_eq!(s.right_only, 1);
    // 手动配对后切换标签：第二对必须是 Summary ↔ Data，而不是按名重推导的同名对。
    v.select_sheet(1);
    ready(&mut v);
    assert_eq!(v.active_pair, 1);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "10");
    assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "apple");
    let _ = std::fs::remove_file(book);
}

#[test]
fn left_file_is_source_decides_the_pairing_order() {
    let left = temp_xlsx(
        "src-left.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,total\n1,10")]),
    );
    let right = temp_xlsx(
        "src-right.xlsx",
        &xlsx(&[("Summary", "id,total\n1,10"), ("Data", "id,name\n7,apple")]),
    );
    let mut v = open_files(&left, &right);
    // 左文件为源：标签顺序按左文件，Data 配右侧第 2 个 sheet。
    assert_eq!(
        v.pairs.0,
        vec![
            SheetPair {
                left: Some(0),
                right: Some(1)
            },
            SheetPair {
                left: Some(1),
                right: Some(0)
            }
        ]
    );
    // 关掉开关：配对顺序跟随右文件。
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft.left_source = false;
    draft.rules.left_is_source = false;
    draft.pairs = SheetPairs::by_name(&v.names[0], &v.names[1], false);
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    assert_eq!(
        v.pairs.0,
        vec![
            SheetPair {
                left: Some(1),
                right: Some(0)
            },
            SheetPair {
                left: Some(0),
                right: Some(1)
            }
        ],
        "right-source pairing follows the right file's sheet order"
    );
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn clipboard_on_a_workbook_side_detects_the_actual_delimiter() {
    let left = temp_xlsx("clip-book.xlsx", &xlsx(&[("Data", "id,name\n7,apple")]));
    let right = temp_xlsx("clip-book-r.xlsx", &xlsx(&[("Data", "id,name\n7,apple")]));
    let mut v = open_files(&left, &right);
    // 该侧是 workbook：显式逗号配置不得把 TSV 剪贴板挤成一列。
    v.options[0].format.detect_delimiter = false;
    v.options[0].format.delimiters = vec![','];
    v.request(Pending::Clipboard(
        0,
        "id\tname\n7\tapple\n8\tpear\n".into(),
    ));
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().column_count(),
        2,
        "clipboard text is delimiter-detected for workbook sides"
    );
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.right_only, 0);
    // 文本侧的显式配置仍被尊重（TSV 剪贴板进逗号配置的文本侧按配置解析）。
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn cancelled_sheet_scan_is_restarted_so_tabs_do_not_stay_pending() {
    let left = temp_xlsx(
        "scan-left.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,2")]),
    );
    let right = temp_xlsx(
        "scan-right.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,2")]),
    );
    let mut v = open_files(&left, &right);
    wait_scanned(&mut v);
    // 模拟切换/显示层操作取消扫描后仍有未访问 sheet：恢复机制必须补齐。
    drop(v.scan.take());
    v.states[1] = None;
    v.poll();
    let start = Instant::now();
    while v.states[1].is_none() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(v.states[1], Some(Importance::Same));
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn open_file_preserves_the_other_sides_cached_sheet_edits_across_repairing() {
    let l0 = temp_xlsx(
        "multi-l0.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let r = temp_xlsx(
        "multi-r.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,y")]),
    );
    let mut v = open_files(&l0, &r);
    // 访问 Summary 并编辑右侧，回到 Data：Summary 对进入缓存且右侧脏。
    v.select_sheet(1);
    ready(&mut v);
    v.begin_edit(1, Cursor { row: 0, column: 1 }, Some("zed".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.select_sheet(0);
    ready(&mut v);
    // 用 [Data, Other] 的新左文件替换左侧：不确认、不丢任何右侧 sheet。
    let l1 = temp_xlsx(
        "multi-l1.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple\n8,pear"), ("Other", "id,q\n3,z")]),
    );
    v.request(Pending::Open(0, l1.clone()));
    assert!(
        v.pending.is_none(),
        "clean target side needs no confirmation"
    );
    ready(&mut v);
    assert_eq!(
        v.pairs.0.len(),
        3,
        "Data↔Data, Other 左孤儿, Summary 右孤儿"
    );
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(2, 1)
            .unwrap(),
        "pear",
        "active pair compares the new left Data sheet"
    );
    // Summary 标签保留右侧编辑与历史，左侧为诚实空位占位。
    v.select_sheet(2);
    ready(&mut v);
    {
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.right.borrow().display_at(0, 1).unwrap(), "zed");
        assert!(
            p.history[1].can_undo(),
            "cached sheet edit history preserved"
        );
        assert_eq!(p.map.left.borrow().row_count(), 0);
    }
    // Other 左孤儿标签：按需从磁盘加载新文件对应 sheet。
    v.select_sheet(1);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "z"
    );
    // 回到 Data：手术安装的对内容不变。
    v.select_sheet(0);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(2, 1)
            .unwrap(),
        "pear"
    );
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r);
    let _ = std::fs::remove_file(l1);
}

#[test]
fn swap_mirrors_cached_sheet_summaries_left_right_counts() {
    // 左 Data 多一行、右 Summary 多一行：Swap 后每 sheet 的 left/right-only 对调。
    let left = temp_xlsx(
        "mir-left.xlsx",
        &xlsx(&[
            ("Data", "id,name\n7,apple\n8,pear"),
            ("Summary", "id,t\n1,x"),
        ]),
    );
    let right = temp_xlsx(
        "mir-right.xlsx",
        &xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x\n2,y")]),
    );
    let mut v = open_files(&left, &right);
    wait_scanned(&mut v);
    let before: Vec<_> = v
        .summaries
        .iter()
        .map(|s| s.map(|s| (s[0].left_only, s[0].right_only)))
        .collect();
    assert_eq!(before, vec![Some((1, 0)), Some((0, 1))]);
    v.swap();
    ready(&mut v);
    let after: Vec<_> = v
        .summaries
        .iter()
        .map(|s| s.map(|s| (s[0].left_only, s[0].right_only)))
        .collect();
    assert_eq!(
        after,
        vec![Some((0, 1)), Some((1, 0))],
        "summaries mirrored without rescan"
    );
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
