use bc_core::{cancel::NeverCancel, line::LineEdit, Side};
use bc_engine_text::{
    importance::Importance,
    jobs::{Align2Job, AlignOutput, Realign2Job},
    rules::TextRules,
    TextContent, TextMap,
};
use bc_jobs::{Job, JobContext, JobId, Progress, ProgressSink};
use std::sync::Arc;
fn content(text: &str) -> TextContent {
    TextContent::from_bytes(text.as_bytes().to_vec(), None).unwrap()
}
#[test]
fn sections_copy_undo_and_ignore_projection() {
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "CPP")
        .unwrap()
        .clone();
    let mut rules = TextRules::default();
    rules
        .comparison
        .importance
        .unimportant
        .push(bc_formats::ElementKind::Comment);
    let mut map = TextMap::compare(
        TextContent::from_bytes(
            b"// old\nanchor\nx = old\nend".to_vec(),
            Some(format.clone()),
        )
        .unwrap(),
        TextContent::from_bytes(b"// new\nanchor\nx = new\nend".to_vec(), Some(format)).unwrap(),
        rules,
    )
    .unwrap();
    assert_eq!(map.difference_counts(), (1, 1));
    let changed = map.sections()[1].rows.start;
    assert_eq!(
        map.next_difference(0, Importance::Unimportant),
        Some(changed)
    );
    map.rules.ignore_unimportant = true;
    assert_eq!(map.difference_counts(), (1, 0));
    let before = map.right.to_bytes().unwrap();
    let edit = map.copy_section(changed, Side::Left).unwrap();
    map.right.edits.push(edit);
    assert_eq!(
        map.text_at(Side::Right, changed).as_deref(),
        Some(&b"x = old"[..])
    );
    assert_eq!(map.difference_counts(), (0, 0));
    map.right.edits.undo();
    assert_eq!(map.right.to_bytes().unwrap(), before);
    assert_eq!(map.difference_counts(), (1, 0));
    map.right.edits.redo();
    assert_eq!(map.difference_counts(), (0, 0));
}
#[test]
fn copy_insert_delete_and_unterminated_joins() {
    for (left, right) in [("a\nnew\nz", "a\nz"), ("a\nz", "a\nold\nz"), ("a\nb", "a")] {
        let mut map = TextMap::compare(content(left), content(right), Default::default()).unwrap();
        let section = map.sections()[0].rows.start;
        let edit = map.copy_section(section, Side::Left).unwrap();
        map.right.edits.push(edit);
        assert_eq!(map.left.to_bytes().unwrap(), map.right.to_bytes().unwrap());
        map.right.edits.undo();
        assert_eq!(map.right.to_bytes().unwrap(), right.as_bytes());
    }
}
#[test]
fn reclassifying_masks_reuses_cached_tokens() {
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "CPP")
        .unwrap()
        .clone();
    let mut map = TextMap::compare(
        TextContent::from_bytes(b"// old".to_vec(), Some(format.clone())).unwrap(),
        TextContent::from_bytes(b"// new".to_vec(), Some(format)).unwrap(),
        Default::default(),
    )
    .unwrap();
    let cache = map.left.snapshot().syntax;
    let scanned = cache.scanned_lines();
    let mut rules = map.rules.comparison.clone();
    rules
        .importance
        .unimportant
        .push(bc_formats::ElementKind::Comment);
    map.reclassify(rules).unwrap();
    assert_eq!(cache.scanned_lines(), scanned);
    assert_eq!(map.difference_counts(), (0, 1));
}
struct Sink;
impl ProgressSink for Sink {
    fn report(&self, _: JobId, _: &Progress) {}
}
#[test]
fn jobs_run_on_std_threads_and_reject_stale_results() {
    let mut map = TextMap::new(
        content("a\nx\nz"),
        content("a\ny\nz"),
        None,
        Default::default(),
    );
    let job = Align2Job::from_map(&map);
    let output = std::thread::spawn(move || {
        let ctx = JobContext {
            id: JobId(1),
            cancel: Default::default(),
            progress: Arc::new(Sink),
            interaction: None,
        };
        *Box::new(job)
            .run(&ctx)
            .unwrap()
            .downcast::<AlignOutput>()
            .unwrap()
    })
    .join()
    .unwrap();
    map.set_alignment(output.clone()).unwrap();
    map.right.replace_line(1, b"x").unwrap();
    assert!(map.set_alignment(output).is_err());
    map.recompare().unwrap();
    assert_eq!(map.difference_counts(), (0, 0));
}
#[test]
fn windowed_realign_matches_full_outside_dirty_region_with_insertions() {
    let original = (0..100)
        .map(|i| format!("unique line {i}\n"))
        .collect::<String>();
    let mut map =
        TextMap::compare(content(&original), content(&original), Default::default()).unwrap();
    map.right.replace_line(50, b"changed line fifty").unwrap();
    let local = Realign2Job::from_map(&map, 50..51)
        .execute(&NeverCancel)
        .unwrap();
    assert!(local.recomputed_rows.len() < 10);
    map.set_alignment(local).unwrap();
    let localrows = map.rows().collect::<Vec<_>>();
    map.recompare().unwrap();
    assert_eq!(map.rows().collect::<Vec<_>>(), localrows);
    let edit = LineEdit::insert(
        &map.right.lines,
        51,
        b"new inserted row",
        bc_core::line::LineEnding::Lf,
    );
    map.right.edits.push(Box::new(edit));
    let local = Realign2Job::from_map(&map, 50..52)
        .execute(&NeverCancel)
        .unwrap();
    map.set_alignment(local).unwrap();
    let localrows = map.rows().collect::<Vec<_>>();
    map.recompare().unwrap();
    assert_eq!(map.rows().collect::<Vec<_>>(), localrows);
}
