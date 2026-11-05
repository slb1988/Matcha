use bc_core::{
    edit::EditItem,
    line::{EndingStyle, LineEdit, LineEnding, LineList},
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn edits_and_ending_conversion_are_reversible() {
    let target = Rc::new(RefCell::new(LineList::from_bytes(
        b"one\r\ntwo\nlast".to_vec(),
    )));
    let original = target.borrow().to_bytes();
    let mut edits: Vec<Box<dyn EditItem>> = vec![
        Box::new(LineEdit::insert(&target, 1, b"new", LineEnding::Cr)),
        Box::new(LineEdit::replace(&target, 2, b"TWO")),
        Box::new(LineEdit::delete(&target, 0)),
        Box::new(LineEdit::set_ending(&target, 0, LineEnding::Lf)),
        Box::new(LineEdit::set_all_endings(&target, LineEnding::Lf)),
    ];
    assert_eq!(target.borrow().to_bytes(), b"new\nTWO\nlast\n");
    let modified = target.borrow().to_bytes();
    for e in edits.iter_mut().rev() {
        e.undo();
    }
    assert_eq!(target.borrow().to_bytes(), original);
    for e in &mut edits {
        e.redo();
    }
    assert_eq!(target.borrow().to_bytes(), modified);
}

#[test]
fn default_insert_and_append_preserve_line_boundaries() {
    let mut lines = LineList::from_bytes(b"a\nb".to_vec());
    lines.insert_default(2, b"c");
    assert_eq!(lines.to_bytes(), b"a\nb\nc\n");
    assert_eq!(lines.ending_style(), EndingStyle::Unix);
    lines.delete(2);
    lines.delete(1);
    lines.delete(0);
    assert!(lines.is_empty());
    lines.insert(0, b"z", LineEnding::None);
    assert_eq!(lines.to_bytes(), b"z");
}

#[test]
fn random_operation_sequences_undo_to_exact_original() {
    let original = b"a\r\nb\nc\rlast";
    let target = Rc::new(RefCell::new(LineList::from_bytes(original.to_vec())));
    let mut seed = 0x1234_5678_u64;
    let mut edits = Vec::new();
    let mut states = vec![original.to_vec()];
    for n in 0..1_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = target.borrow().len();
        let i = (seed >> 16) as usize % len.max(1);
        let text = format!("line {n}");
        let e = match seed % 5 {
            0 if len > 0 => LineEdit::delete(&target, i),
            1 if len > 0 => LineEdit::replace(&target, i, text.as_bytes()),
            2 if len > 0 => LineEdit::set_ending(&target, i, LineEnding::Cr),
            3 => LineEdit::set_all_endings(&target, LineEnding::Lf),
            _ => LineEdit::insert(&target, i, text.as_bytes(), LineEnding::CrLf),
        };
        edits.push(e);
        states.push(target.borrow().to_bytes());
    }
    for (i, e) in edits.iter_mut().enumerate().rev() {
        e.undo();
        assert_eq!(target.borrow().to_bytes(), states[i]);
    }
    for (i, e) in edits.iter_mut().enumerate() {
        e.redo();
        assert_eq!(target.borrow().to_bytes(), states[i + 1]);
    }
}

#[test]
fn snapshots_and_unaffected_lines_stay_shared() {
    let mut list = LineList::from_bytes(b"one\ntwo\nthree".to_vec());
    let snapshot = list.clone();
    let pointer = list.line_bytes(2).unwrap().as_ptr();
    list.replace(1, b"replacement");
    assert_eq!(list.line_bytes(2).unwrap().as_ptr(), pointer);
    assert_eq!(snapshot.to_bytes(), b"one\ntwo\nthree");
    assert_eq!(list.to_bytes(), b"one\nreplacement\nthree");
}
