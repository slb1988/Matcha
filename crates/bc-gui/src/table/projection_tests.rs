use super::*;
use bc_engine_table::{
    compare::Importance,
    sheets::{SheetPair, SheetPairs},
    Style,
};

fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
    ready(v);
}
fn formula_pair() -> (Sheet, Sheet) {
    let mut left = sheet("id,value\n1,6\n");
    let mut right = left.clone();
    let mut cell = Cell::number(6.0);
    cell.formula = Some("=A1+A2+A3".into());
    left.set_cell(0, 1, cell.clone()).unwrap();
    cell.formula = Some("=SUM(A1:A3)".into());
    right.set_cell(0, 1, cell).unwrap();
    (left, right)
}
fn format_pair() -> (Sheet, Sheet) {
    let mut left = sheet("id,value\n1,6\n");
    let mut right = left.clone();
    for (s, format) in [(&mut left, "0.0"), (&mut right, "0.00")] {
        let style = s.intern_style(Style {
            number_format: Some(format.into()),
        });
        let mut cell = Cell::number(6.0);
        cell.style = Some(style);
        s.set_cell(0, 1, cell).unwrap();
    }
    (left, right)
}
fn from_pair(pair: (Sheet, Sheet)) -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        pair.0,
        pair.1,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}

#[test]
fn raw_three_layer_controls_and_ignore_preserve_formulas_formats_and_details() {
    let (mut left, mut right) = formula_pair();
    let (fl, fr) = format_pair();
    for (s, source) in [(&mut left, &fl), (&mut right, &fr)] {
        let mut cell = source.cell(0, 1).unwrap().clone();
        cell.style = Some(s.intern_style(source.style(cell.style).unwrap().clone()));
        s.push_row(vec![Cell::number(2.0), cell]);
    }
    left.push_row(vec![Cell::number(3.0), Cell::number(1.0)]);
    right.push_row(vec![Cell::number(3.0), Cell::number(2.0)]);
    let mut v = from_pair((left, right));
    v.show_details = true;
    v.grid.cursor = Cursor { row: 0, column: 1 };
    let ctx = egui::Context::default();
    let out = frame(&ctx, &mut v, vec![]);
    for formula in ["=A1+A2+A3", "=SUM(A1:A3)"] {
        assert!(out
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == formula)));
    }
    let stats = |v: &TableCompareView| {
        let s = v.pair.as_ref().unwrap().cache.statistics;
        (s.same, s.unimportant, s.important)
    };
    assert_eq!(stats(&v), (2, 0, 1));
    for _ in 0..2 {
        choose(&ctx, &mut v, "+ Formulas");
        assert_eq!(stats(&v), (1, 0, 2));
        choose(&ctx, &mut v, "+ Number formats");
        assert_eq!(stats(&v), (0, 1, 2));
        choose(&ctx, &mut v, "Ignore Unimportant");
        assert_eq!(stats(&v), (1, 0, 2));
        assert_eq!(
            v.pair.as_ref().unwrap().map.cell_importance(1, 1),
            Some(Importance::Same)
        );
        choose(&ctx, &mut v, "Ignore Unimportant");
        assert_eq!(stats(&v), (0, 1, 2));
        choose(&ctx, &mut v, "Values");
        assert_eq!(stats(&v), (2, 0, 1));
    }
    let p = v.pair.as_ref().unwrap();
    assert_eq!(
        p.map.cell_diff(0, 1).unwrap().formula,
        Importance::Important
    );
    assert_eq!(
        p.map.cell_diff(1, 1).unwrap().format,
        Importance::Unimportant
    );
    assert!(p.history.iter().all(|h| !h.can_undo()));
}

#[test]
fn cached_sheet_switch_keeps_selected_layers_ignore_and_filter_consistent() {
    let mut v = from_pair(formula_pair());
    let mut other = from_pair(format_pair());
    v.cached.insert(1, other.pair.take().unwrap());
    v.names = [
        vec!["Data".into(), "Other".into()],
        vec!["Data".into(), "Other".into()],
    ];
    v.pairs = SheetPairs(vec![
        SheetPair {
            left: Some(0),
            right: Some(0),
        },
        SheetPair {
            left: Some(1),
            right: Some(1),
        },
    ]);
    v.states.push(other.states[0]);
    v.summaries.push(other.summaries[0]);
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "Differences");
    assert_eq!(v.row_count(), 0);
    choose(&ctx, &mut v, "+ Number formats");
    assert_eq!(v.row_count(), 1);
    assert_eq!(
        v.states,
        [Some(Importance::Important), Some(Importance::Unimportant)]
    );
    choose(&ctx, &mut v, "Ignore Unimportant");
    choose(&ctx, &mut v, "● Other");
    assert_eq!(v.active_pair, 1);
    assert_eq!(v.row_count(), 0);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
    assert_eq!(v.pair.as_ref().unwrap().map.rules().layers, Layers::All);
    choose(&ctx, &mut v, "Ignore Unimportant");
    assert_eq!(v.row_count(), 1);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.unimportant, 1);
    choose(&ctx, &mut v, "● Data");
    assert_eq!(v.active_pair, 0);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    choose(&ctx, &mut v, "Values");
    choose(&ctx, &mut v, "● Other");
    assert_eq!(v.row_count(), 0);
    assert_eq!(v.states, [Some(Importance::Same), Some(Importance::Same)]);
}

#[test]
fn layer_projection_does_not_cancel_an_inflight_all_layer_sheet_summary() {
    use super::super::worker::{Output, Worker};
    use bc_engine_table::map::TableStatistics;
    use bc_jobs::{Job, JobContext, JobError, JobResult};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    struct GatedSummary {
        release: Arc<AtomicBool>,
        summaries: Vec<[TableStatistics; 3]>,
    }
    impl Job for GatedSummary {
        fn name(&self) -> String {
            "Gated sheet summary".into()
        }
        fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
            while !self.release.load(Ordering::Acquire) {
                if ctx.cancel.is_cancelled() {
                    return Err(JobError::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(Box::new(Output::Scanned(self.summaries)))
        }
    }
    let mut v = from_pair(format_pair());
    let summaries = v.pair.as_ref().unwrap().cache.summaries;
    v.states.push(None);
    v.summaries.push(None);
    let release = Arc::new(AtomicBool::new(false));
    v.scan = Some(Worker::spawn(Box::new(GatedSummary {
        release: release.clone(),
        summaries: vec![summaries; 2],
    })));
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "+ Number formats");
    assert!(
        v.scan.is_some(),
        "a projection change must not strand pending sheet tabs"
    );
    release.store(true, Ordering::Release);
    let start = Instant::now();
    while v.scan.is_some() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(v.states[1], Some(Importance::Unimportant));
    assert!(v.summaries[1].is_some());
    choose(&ctx, &mut v, "Ignore Unimportant");
    assert_eq!(
        v.states[1]
            .unwrap()
            .projected(v.default_rules.ignore_unimportant),
        Importance::Same
    );
}
