use bc_core::{
    edit::EditStack,
    line::{LineEdit, LineEnding, LineList, SharedLines},
};
use std::{cell::RefCell, rc::Rc};

fn target() -> SharedLines {
    Rc::new(RefCell::new(LineList::from_bytes(b"a".to_vec())))
}
fn replace(stack: &mut EditStack, t: &SharedLines, text: &[u8]) {
    stack.push(Box::new(LineEdit::replace(t, 0, text)));
}
fn insert(stack: &mut EditStack, t: &SharedLines, text: &[u8]) {
    stack.push(Box::new(LineEdit::insert(t, 0, text, LineEnding::Lf)));
}

#[test]
fn coalescing_preserves_save_boundaries() {
    let t = target();
    let mut s = EditStack::new();
    assert!(!s.is_dirty());
    replace(&mut s, &t, b"ab");
    replace(&mut s, &t, b"abc");
    s.mark_saved();
    replace(&mut s, &t, b"abcd");
    assert!(s.is_dirty());
    assert_eq!(s.undo().as_deref(), Some("Replace line"));
    assert_eq!(t.borrow().to_bytes(), b"abc");
    assert!(!s.is_dirty());
    s.undo();
    assert_eq!(t.borrow().to_bytes(), b"a");
    assert!(!s.can_undo());
    assert!(s.is_dirty());
    s.redo();
    assert!(!s.is_dirty());
    s.redo();
    assert_eq!(t.borrow().to_bytes(), b"abcd");
    assert!(s.redo().is_none());
}

#[test]
fn groups_nest_undo_reverse_and_redo_forward() {
    let t = target();
    let mut s = EditStack::new();
    s.begin_group("outer");
    insert(&mut s, &t, b"b");
    s.begin_group("inner");
    insert(&mut s, &t, b"c");
    insert(&mut s, &t, b"d");
    assert!(s.is_dirty());
    assert!(s.undo().is_none());
    s.end_group();
    insert(&mut s, &t, b"e");
    s.end_group();
    assert_eq!(t.borrow().to_bytes(), b"e\nd\nc\nb\na");
    assert_eq!(s.undo().as_deref(), Some("outer"));
    assert_eq!(t.borrow().to_bytes(), b"a");
    assert!(!s.is_dirty());
    assert_eq!(s.redo().as_deref(), Some("outer"));
    assert_eq!(t.borrow().to_bytes(), b"e\nd\nc\nb\na");
}

#[test]
fn branch_and_empty_groups_do_not_resurrect_saved_state() {
    let t = target();
    let mut s = EditStack::new();
    insert(&mut s, &t, b"b");
    s.mark_saved();
    s.undo();
    s.begin_group("empty");
    s.end_group();
    assert!(s.can_redo());
    insert(&mut s, &t, b"c");
    assert!(
        s.is_dirty(),
        "same depth on a different branch is not the save point"
    );
    assert!(!s.can_redo());
    s.undo();
    s.redo();
    assert!(s.is_dirty());
}

#[test]
fn limit_truncation_rebases_or_invalidates_save_point() {
    let t = target();
    let mut s = EditStack::new();
    s.set_limit(2);
    insert(&mut s, &t, b"b");
    s.mark_saved();
    insert(&mut s, &t, b"c");
    insert(&mut s, &t, b"d");
    s.undo();
    s.undo();
    assert!(!s.is_dirty());
    assert!(!s.can_undo());
    assert_eq!(t.borrow().to_bytes(), b"b\na");
    s.redo();
    s.redo();
    insert(&mut s, &t, b"e");
    s.undo();
    s.undo();
    assert!(s.is_dirty(), "save point was evicted");
    s.clear();
    assert!(s.is_dirty());
    assert!(!s.can_redo());
    assert!(!s.can_undo());
    s.mark_saved();
    assert!(!s.is_dirty());
}

#[test]
fn lowering_limit_discards_distant_redo_and_saved_future() {
    let t = target();
    let mut s = EditStack::new();
    for value in [b"b", b"c", b"d"] {
        insert(&mut s, &t, value);
    }
    s.mark_saved();
    for _ in 0..3 {
        s.undo();
    }
    s.set_limit(1);
    assert!(s.redo().is_some());
    assert!(s.redo().is_none());
    assert!(s.is_dirty());
}
