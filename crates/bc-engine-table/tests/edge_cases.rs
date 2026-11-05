use bc_core::{
    align::{AlignStrategy, AlignmentOverrides},
    cancel::NeverCancel,
    edit::EditStack,
    CoreError,
};
use bc_engine_table::{
    columns::{align_columns, ColumnChange, ColumnMapping, Side},
    compare::Importance,
    map::ShowRows,
    rules::{ColumnHandling, ColumnMode, KeySelection, Layers},
    source::{read_text, save_as, write_delimited, ReadOptions},
    summary::SummaryFormat,
    Cell, KeyAlign, Sheet, TableMap, TableRules,
};
use std::io::Cursor;

fn sheet(text: &str) -> Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
#[test]
fn random_small_alignment_partitions_sources_and_never_pairs_different_keys() {
    let mut random = 1234567u64;
    for _ in 0..50 {
        let mut sheets = [Sheet::default(), Sheet::default()];
        for sheet in &mut sheets {
            for _ in 0..20 {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                sheet.push_row(vec![Cell::number((random % 9) as f64)]);
            }
        }
        for strategy in [
            AlignStrategy::Quick,
            AlignStrategy::Smart,
            AlignStrategy::Patience,
            AlignStrategy::None,
        ] {
            let rules = TableRules {
                columns: ColumnMapping::by_number(1, 1),
                keys: KeySelection::AllColumns,
                strategy,
                ..Default::default()
            };
            let a = KeyAlign
                .align(
                    &sheets[0],
                    &sheets[1],
                    &rules,
                    &AlignmentOverrides::default(),
                    &NeverCancel,
                )
                .unwrap();
            assert_eq!(
                a.map
                    .rows()
                    .iter()
                    .filter_map(|r| r.left)
                    .collect::<Vec<_>>(),
                (0..20).collect::<Vec<_>>()
            );
            assert_eq!(
                a.map
                    .rows()
                    .iter()
                    .filter_map(|r| r.right)
                    .collect::<Vec<_>>(),
                (0..20).collect::<Vec<_>>()
            );
            for row in a.map.rows() {
                if let (Some(l), Some(r)) = (row.left, row.right) {
                    if strategy != AlignStrategy::None {
                        assert_eq!(sheets[0].cell(l, 0), sheets[1].cell(r, 0));
                    }
                }
            }
        }
    }
}
#[test]
fn all_column_and_by_number_selections_remain_explicit_and_auto_is_labelled() {
    let l = sheet("id,n\n1,a\n2,b\n");
    let r = sheet("id,n\n1,x\n2,b\n");
    let auto = TableMap::new(l.clone(), r.clone(), TableRules::default(), &NeverCancel).unwrap();
    assert_eq!(auto.rules().keys.label(), "自动");
    assert_eq!(auto.row_count(), 2);
    for selection in [KeySelection::AllColumns, KeySelection::ByColumnNumber] {
        let mut rules = TableRules {
            columns: ColumnMapping::by_number(2, 2),
            keys: selection.clone(),
            ..Default::default()
        };
        let suggestion =
            bc_engine_table::inference::infer_keys(&l, &r, &rules, &NeverCancel).unwrap();
        assert!(!suggestion.apply(&mut rules));
        assert_eq!(rules.keys, selection);
    }
}
#[test]
fn renamed_columns_at_98_percent_agreement_and_structural_summary() {
    let mut l = Sheet::default();
    l.headers = Some(vec!["qty".into()]);
    let mut r = Sheet::default();
    r.headers = Some(vec!["quantity".into()]);
    for i in 0..100 {
        l.push_row(vec![Cell::number(i as f64)]);
        r.push_row(vec![Cell::number(if i < 98 {
            i as f64
        } else {
            (i + 100) as f64
        })]);
    }
    let mapping = align_columns(&l, &r, true, &NeverCancel).unwrap();
    assert_eq!(mapping.0[0].changes, vec![ColumnChange::Renamed]);
    let map = TableMap::new(
        l,
        r,
        TableRules {
            columns: mapping,
            strategy: AlignStrategy::None,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert!(map
        .export_summary(SummaryFormat::Text)
        .unwrap()
        .contains("COLUMN\t1\tRenamed\t1\t1\tqty\tquantity"));
    assert!(map
        .export_summary(SummaryFormat::Csv)
        .unwrap()
        .contains("column,,Renamed,1,structure,qty,quantity"));
}
#[test]
fn ignore_unimportant_projects_navigation_filters_and_sheet_colour() {
    let l = sheet("id,n\n1,a\n");
    let r = sheet("id,n\n1,b\n");
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(2, 2),
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    rules.columns.0[1].handling = ColumnHandling {
        unimportant: true,
        ..Default::default()
    };
    let m = TableMap::new(l, r, rules, &NeverCancel).unwrap();
    assert_eq!(m.statistics().importance(), Importance::Unimportant);
    assert_eq!(m.next_difference_cell(None), Some((0, 1)));
    m.rules_mut().ignore_unimportant = true;
    assert_eq!(m.statistics().importance(), Importance::Same);
    assert_eq!(m.statistics().same, 1);
    assert!(m.visible_rows(ShowRows::Differences).is_empty());
    assert_eq!(m.visible_rows(ShowRows::Same), vec![0]);
    assert_eq!(m.next_difference_row(None), None);
    assert_eq!(m.previous_difference_row(None), None);
    assert_eq!(m.cell_diff(0, 1).unwrap().value, Importance::Unimportant);
}
#[test]
fn summary_escapes_payload_and_includes_formula_format_layers() {
    let mut l = sheet("id,n\n1,a\n");
    let mut r = l.clone();
    let ls = l.intern_style(bc_engine_table::Style {
        number_format: Some("0.00".into()),
    });
    let rs = r.intern_style(bc_engine_table::Style {
        number_format: Some("0.000".into()),
    });
    let mut a = Cell::text("a|b\nc");
    a.formula = Some("=A1".into());
    a.style = Some(ls);
    let mut b = Cell::text("x,\"q\"");
    b.formula = Some("=B1".into());
    b.style = Some(rs);
    l.set_cell(0, 1, a).unwrap();
    r.set_cell(0, 1, b).unwrap();
    let m = TableMap::new(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            layers: Layers::All,
            format_important: true,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(m.cell_diff(0, 1).unwrap().format, Importance::Important);
    let text = m.export_summary(SummaryFormat::Text).unwrap();
    assert!(text.contains("a\\|b\\nc"));
    assert!(text.contains("n (formula)"));
    assert!(text.contains("n (format)"));
    assert!(m
        .export_summary(SummaryFormat::Csv)
        .unwrap()
        .contains("\"x,\"\"q\"\"\""));
}
#[test]
fn copying_to_empty_side_adds_mapped_columns_and_undo_is_complete() {
    let l = sheet("id,n\n1,a\n");
    let r = Sheet::default();
    let m = TableMap::new(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    let mut history = EditStack::new();
    history.push(m.copy_row(0, Side::Left).unwrap());
    assert_eq!(m.statistics().same, 1);
    assert_eq!(m.right.borrow().column_count(), 2);
    history.undo();
    assert_eq!(m.right.borrow().column_count(), 0);
    assert_eq!(m.statistics().left_only, 1);
    assert!(!m.is_stale());
    history.redo();
    assert_eq!(m.statistics().same, 1);
    assert!(!m.is_stale());
}
#[test]
fn inserted_rows_shift_manual_anchors_and_can_be_undone() {
    let l = sheet("id,n\n1,a\n2,b\n3,c\n");
    let r = sheet("id,n\n1,a\n3,c\n");
    let m = TableMap::new(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    let mut anchors = AlignmentOverrides::default();
    anchors.insert(2, 1).unwrap();
    m.set_overrides(anchors.clone());
    m.recompare(&NeverCancel).unwrap();
    let mut history = EditStack::new();
    history.push(m.copy_row(1, Side::Left).unwrap());
    assert_eq!(m.overrides().pairs(), &[(2, 2)]);
    assert_eq!(m.statistics().same, 3);
    history.undo();
    assert_eq!(m.overrides(), anchors);
    assert!(!m.is_stale());
}
#[test]
fn active_cancel_and_invalid_anchor_return_no_partial_map() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut s = Sheet::default();
    for i in 0..10000 {
        s.push_row(vec![Cell::number(i as f64)]);
    }
    let rules = TableRules {
        columns: ColumnMapping::by_number(1, 1),
        ..Default::default()
    };
    let checks = AtomicUsize::new(0);
    let cancel = || checks.fetch_add(1, Ordering::Relaxed) > 8;
    assert!(matches!(
        KeyAlign.align(&s, &s, &rules, &AlignmentOverrides::default(), &cancel),
        Err(CoreError::Cancelled)
    ));
    let mut anchor = AlignmentOverrides::default();
    anchor.insert(99999, 0).unwrap();
    assert!(KeyAlign
        .align(&s, &s, &rules, &anchor, &NeverCancel)
        .is_err());
}
#[test]
fn atomic_save_failure_keeps_existing_target_and_utf8_boundary_detection() {
    let dir = std::env::temp_dir().join(format!("matcha-table-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("table.csv");
    std::fs::write(&path, b"original").unwrap();
    let mut s = read_text(
        Cursor::new("1"),
        &ReadOptions {
            format: bc_formats::TableOptions {
                qualifier: None,
                ..Default::default()
            },
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap()
    .sheet;
    s.set_cell(0, 0, Cell::text("a,b")).unwrap();
    assert!(save_as(&s, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    s.set_cell(0, 0, Cell::text("plain")).unwrap();
    save_as(&s, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"plain");
    std::fs::remove_dir_all(&dir).unwrap();
    let text = format!("{}猫,b\n", "a".repeat(8191));
    let parsed = read_text(Cursor::new(&text), &ReadOptions::default(), &NeverCancel).unwrap();
    let mut saved = Vec::new();
    write_delimited(&parsed.sheet, &mut saved).unwrap();
    assert_eq!(saved, text.as_bytes());
}
#[test]
fn automatic_column_mapping_is_used_without_overriding_positional_preset() {
    let l = sheet("id,name\n1,a\n");
    let r = sheet("name,id\na,1\n");
    let m = TableMap::new(
        l.clone(),
        r.clone(),
        TableRules {
            column_mode: ColumnMode::Automatic,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(m.rules().columns.0[0].right, Some(1));
    assert_eq!(m.statistics().same, 1);
    let m = TableMap::new(
        l,
        r,
        TableRules {
            column_mode: ColumnMode::Automatic,
            keys: KeySelection::ByColumnNumber,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(m.rules().columns.0[0].right, Some(0));
}
