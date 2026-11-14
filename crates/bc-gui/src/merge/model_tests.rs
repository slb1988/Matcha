use super::*;
use bc_engine_text::rules::TextMergeRules;

pub(crate) fn base(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("line{i:02}")).collect()
}
pub(crate) fn edit(base: &[String], changes: &[(usize, &str)]) -> Vec<String> {
    let mut out = base.to_vec();
    for (line, text) in changes {
        out[line - 1] = text.to_string();
    }
    out
}
pub(crate) fn insert(base: &[String], after: usize, lines: &[&str]) -> Vec<String> {
    let mut out = base.to_vec();
    for (i, line) in lines.iter().enumerate() {
        out.insert(after + i, line.to_string());
    }
    out
}
pub(crate) fn bytes(lines: &[String]) -> Vec<u8> {
    lines.iter().flat_map(|l| format!("{l}\n").into_bytes()).collect()
}
pub(crate) fn model_of(left: Vec<u8>, right: Vec<u8>, center: Vec<u8>) -> MergeModel {
    let content = |b: Vec<u8>| TextContent::from_bytes(b, None).unwrap();
    MergeModel::from_contents(
        content(left),
        content(right),
        content(center),
        TextMergeRules::default(),
    )
    .unwrap()
}
pub(crate) fn model(left: &[String], right: &[String], center: &[String]) -> MergeModel {
    model_of(bytes(left), bytes(right), bytes(center))
}
fn text(model: &MergeModel) -> String {
    String::from_utf8(model.output.lines.borrow().to_bytes()).unwrap()
}
fn joined(lines: &[String]) -> String {
    String::from_utf8(bytes(lines)).unwrap()
}
/// 不变量：元数据与输出行一一对应，对齐行号不减，网格覆盖每个对齐行与每个输出行。
fn check(model: &MergeModel) {
    let lines = model.output.lines.borrow().len();
    let meta: Vec<OutLine> = (0..lines).map(|i| model.out_line(i).unwrap()).collect();
    assert!(model.out_line(lines).is_none());
    assert!(meta.windows(2).all(|w| w[0].row <= w[1].row));
    let outs: Vec<usize> = model.grid().iter().filter_map(|g| g.out).collect();
    assert_eq!(outs, (0..lines).collect::<Vec<_>>());
    for row in 0..model.map.row_count() {
        assert_eq!(model.grid()[model.grid_row_of(row)].row, row);
    }
    for line in 0..lines {
        assert_eq!(model.grid()[model.grid_row_of_out(line)].out, Some(line));
    }
}
fn type_at(model: &mut MergeModel, editor: &mut Editor, line: usize, byte: usize, text: &str) {
    editor.place(Caret { line, byte }, false);
    assert!(model
        .edit(editor, |editor, content| {
            editor.insert(content, text)?;
            Ok(true)
        })
        .unwrap());
    check(model);
}

#[test]
fn opens_with_the_default_merge_and_origins() {
    let b = base(12);
    let left = edit(&b, &[(5, "L5")]);
    let right = insert(&b, 10, &["r new 1", "r new 2"]);
    let m = model(&left, &right, &b);
    check(&m);
    assert_eq!(text(&m), joined(&insert(&left, 10, &["r new 1", "r new 2"])));
    assert_eq!(m.out_line(4).unwrap().origin, OutOrigin::Left);
    assert_eq!(m.out_line(10).unwrap().origin, OutOrigin::Right);
    assert_eq!(m.out_line(0).unwrap().origin, OutOrigin::Center);
    assert_eq!(m.unresolved(), 0);
    assert_eq!(m.grid().len(), m.map.row_count());
    // 没有输出目标：未改动就不算未保存；结果码为"未写出"。
    assert!(!m.is_dirty());
    assert_eq!(m.result_code(), 101);
}

#[test]
fn take_replaces_the_section_and_can_be_undone() {
    let b = base(12);
    let left = insert(&b, 5, &["left 1", "left 2", "left 3"]);
    let right = insert(&b, 5, &["right 1"]);
    let mut m = model(&left, &right, &b);
    let mut editor = Editor::default();
    assert_eq!(m.unresolved(), 1);
    let before = text(&m);
    let rows = m.section_at(5).unwrap();
    assert_eq!(rows, 5..8);
    m.take(rows.clone(), Side::Right, &mut editor);
    check(&m);
    assert_eq!(text(&m), joined(&right));
    assert_eq!(m.unresolved(), 0);
    assert_eq!(m.out_line(5).unwrap().origin, OutOrigin::Right);
    // 输出少于对齐行：其余两行在输出里是空隙。
    assert_eq!(m.grid().iter().filter(|g| g.out.is_none()).count(), 2);
    assert_eq!(editor.caret.line, 5);
    assert!(m.is_dirty());
    assert!(m.undo(&mut editor));
    check(&m);
    assert_eq!(text(&m), before);
    assert_eq!(m.unresolved(), 1);
    assert!(m.redo(&mut editor));
    assert_eq!(text(&m), joined(&right));
    assert_eq!(m.unresolved(), 0);
    // Take Center 回到 base，Take Left 再换成左侧。
    m.take(rows.clone(), Side::Center, &mut editor);
    assert_eq!(text(&m), joined(&b));
    m.take(rows, Side::Left, &mut editor);
    assert_eq!(text(&m), joined(&left));
    check(&m);
}

#[test]
fn accept_clears_the_conflict_without_touching_the_output() {
    let b = base(12);
    let mut m = model(&edit(&b, &[(5, "L5")]), &edit(&b, &[(7, "R7")]), &b);
    let mut editor = Editor::default();
    let before = text(&m);
    assert_eq!(before, joined(&edit(&b, &[(5, "L5"), (7, "R7")])));
    let rows = m.section_at(4).unwrap();
    assert_eq!(rows, 4..7);
    m.accept(&rows, &editor);
    assert_eq!(m.unresolved(), 0);
    assert_eq!(text(&m), before);
    assert!(m.undo(&mut editor));
    assert_eq!(m.unresolved(), 1);
}

#[test]
fn conflict_navigation_skips_resolved_sections() {
    let b = base(30);
    let left = edit(&b, &[(3, "L"), (12, "L"), (25, "L")]);
    let right = edit(&b, &[(3, "R"), (12, "R"), (25, "R")]);
    let mut m = model(&left, &right, &b);
    assert_eq!(m.next_conflict(0, true), Some(2));
    assert_eq!(m.next_conflict(2, true), Some(11));
    assert_eq!(m.next_conflict(24, true), None);
    assert_eq!(m.next_conflict(24, false), Some(11));
    assert_eq!(m.next_conflict(2, false), None);
    let middle = m.section_at(11).unwrap();
    m.accept(&middle, &Editor::default());
    assert_eq!(m.next_conflict(2, true), Some(24));
    assert_eq!(m.next_conflict(24, false), Some(2));
}

#[test]
fn manual_edits_mark_lines_and_resolve_their_conflict() {
    let b = base(12);
    let mut m = model(&edit(&b, &[(5, "L5")]), &edit(&b, &[(5, "R5")]), &b);
    let mut editor = Editor::default();
    assert_eq!(m.unresolved(), 1);
    type_at(&mut m, &mut editor, 4, 0, "merged ");
    assert_eq!(m.out_line(4).unwrap().origin, OutOrigin::Manual);
    assert_eq!(m.out_line(3).unwrap().origin, OutOrigin::Center);
    assert_eq!(m.out_line(5).unwrap().origin, OutOrigin::Center);
    assert_eq!(m.unresolved(), 0);
    assert!(text(&m).contains("merged line05\n"));
    // Take 覆盖手工内容。
    let rows = m.section_at(4).unwrap();
    m.take(rows, Side::Left, &mut editor);
    assert!(text(&m).contains("L5\n") && !text(&m).contains("merged"));
    assert!((0..12).all(|i| m.out_line(i).unwrap().origin != OutOrigin::Manual));
    // 撤销两步回到最初：冲突重新计数。
    assert!(m.undo(&mut editor) && m.undo(&mut editor));
    assert_eq!(m.unresolved(), 1);
    assert_eq!(text(&m), joined(&b));
    check(&m);
}

#[test]
fn inserted_and_joined_lines_keep_the_grid_consistent() {
    let b = base(8);
    let mut m = model(&b, &b, &b);
    let mut editor = Editor::default();
    // 行尾回车：新行附着在第 3 个对齐行上，输入窗格在那里多一个空隙行。
    type_at(&mut m, &mut editor, 2, 6, "\nextra");
    assert_eq!(m.output.lines.borrow().len(), 9);
    assert_eq!(m.out_line(2).unwrap(), OutLine { origin: OutOrigin::Center, row: 2 });
    assert_eq!(m.out_line(3).unwrap(), OutLine { origin: OutOrigin::Manual, row: 2 });
    assert_eq!(m.out_line(4).unwrap(), OutLine { origin: OutOrigin::Center, row: 3 });
    assert_eq!(m.grid().len(), 9);
    assert_eq!(m.grid()[2], GridRow { row: 2, own: true, out: Some(2) });
    assert_eq!(m.grid()[3], GridRow { row: 2, own: false, out: Some(3) });
    // 在行首插入一行：新行挂在上一行所属的对齐行后面，输入窗格在那里是空隙。
    type_at(&mut m, &mut editor, 6, 0, "above\n");
    let at = m.grid_row_of_out(6);
    assert_eq!(m.out_line(6).unwrap().origin, OutOrigin::Manual);
    assert!(!m.grid()[at].own && m.grid()[at - 1].own && m.grid()[at + 1].own);
    assert_eq!(m.grid()[at].row, m.grid()[at - 1].row);
    assert!(m.undo(&mut editor));
    // 插在文件最前面：没有上一行可挂，输入窗格仍与原来的第一行对齐。
    type_at(&mut m, &mut editor, 0, 0, "top\n");
    assert_eq!(m.grid()[0], GridRow { row: 0, own: false, out: Some(0) });
    assert_eq!(m.grid()[1], GridRow { row: 0, own: true, out: Some(1) });
    assert!(m.undo(&mut editor));
    // 行首退格：与上一行合并。
    editor.place(Caret { line: 6, byte: 0 }, false);
    assert!(m
        .edit(&mut editor, |editor, content| {
            editor.delete(content, true, false)?;
            Ok(true)
        })
        .unwrap());
    check(&m);
    assert_eq!(m.output.lines.borrow().len(), 8);
    assert!(text(&m).contains("line05line06\n"));
    assert_eq!(m.out_line(5).unwrap().origin, OutOrigin::Manual);
    // 跨行选区删除、粘贴多行。
    editor.place(Caret { line: 0, byte: 2 }, false);
    editor.place(Caret { line: 2, byte: 3 }, true);
    assert!(m
        .edit(&mut editor, |editor, content| {
            editor.insert(content, "A\nB\nC\nD")?;
            Ok(true)
        })
        .unwrap());
    check(&m);
    assert!(text(&m).starts_with("liA\nB\nC\nDe03\n"));
    // 一路撤销到底。
    while m.undo(&mut editor) {
        check(&m);
    }
    assert_eq!(text(&m), joined(&b));
    assert!(m.can_redo() && !m.can_undo());
}

#[test]
fn save_writes_the_buffer_and_tracks_the_result_code() {
    let dir = std::env::temp_dir().join(format!("matcha-merge-model-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let b = base(12);
    let left = edit(&b, &[(5, "L5")]);
    let right = edit(&b, &[(5, "R5")]);
    for (name, lines) in [("left.txt", &left), ("right.txt", &right), ("base.txt", &b)] {
        std::fs::write(dir.join(name), bytes(lines)).unwrap();
    }
    std::fs::write(dir.join("out.txt"), "old output that must be ignored\n").unwrap();
    let launch = MergeLaunch::new(MergeSpec {
        left: dir.join("left.txt"),
        right: dir.join("right.txt"),
        center: dir.join("base.txt"),
        output: Some(dir.join("out.txt")),
    });
    let mut m = MergeModel::load(&launch).unwrap();
    let mut editor = Editor::default();
    // 带输出目标：打开即未保存；既有输出内容不参与。
    assert!(m.is_dirty());
    assert_eq!(m.result_code(), 101);
    assert_eq!(text(&m), joined(&b));
    // 默认保存到 LEFT 的文件，输出文件（P4V `%r`）同步一份。
    let target = m.target.clone().unwrap();
    assert_eq!(target, dir.join("left.txt"));
    m.save(&target).unwrap();
    assert!(!m.is_dirty());
    assert_eq!(m.result_code(), 14, "saved with an unresolved conflict");
    assert_eq!(std::fs::read(&target).unwrap(), bytes(&b));
    assert_eq!(std::fs::read(dir.join("out.txt")).unwrap(), bytes(&b));
    let rows = m.section_at(4).unwrap();
    m.take(rows, Side::Right, &mut editor);
    assert!(m.is_dirty());
    assert_eq!(m.result_code(), 14, "the file on disk still has the conflict");
    m.save(&target).unwrap();
    assert_eq!(m.result_code(), 0);
    assert_eq!(std::fs::read(&target).unwrap(), bytes(&right));
    // 输出可以覆盖某个输入；目录里不出现备份或临时文件。
    m.save(&dir.join("right.txt")).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["base.txt", "left.txt", "out.txt", "right.txt"]);
    // 打不开的输入。
    let mut bad = launch.clone();
    bad.spec.left = dir.join("missing.txt");
    assert!(MergeModel::load(&bad).err().unwrap().contains("does not exist"));
    bad.spec.left = dir.clone();
    assert!(MergeModel::load(&bad).err().unwrap().contains("is a folder"));
    std::fs::write(dir.join("b.bin"), [0u8, 1, 2, 0, 0, 0, 3, 0, 0, 0, 0, 0]).unwrap();
    bad.spec.left = dir.join("b.bin");
    assert!(MergeModel::load(&bad).err().unwrap().contains("not a text file"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_saved_bytes_use_the_three_way_encoding() {
    let ansi = b"caf\xe9\nline02\n".to_vec();
    let mut utf8_bom = b"\xef\xbb\xbf".to_vec();
    utf8_bom.extend_from_slice("café\nline02\n".as_bytes());
    let mut right = ansi.clone();
    right.extend_from_slice(b"line03\n");
    let m = model_of(utf8_bom.clone(), right, ansi);
    let mut expected = utf8_bom;
    expected.extend_from_slice(b"line03\n");
    assert_eq!(m.output_bytes().unwrap(), expected);
    assert_eq!(text(&m), "café\nline02\nline03\n");
}

#[test]
fn empty_inputs_still_accept_typed_lines() {
    let mut m = model_of(Vec::new(), Vec::new(), Vec::new());
    let mut editor = Editor::default();
    check(&m);
    assert_eq!(m.grid().len(), 0);
    type_at(&mut m, &mut editor, 0, 0, "hello\nworld");
    // 空缓冲没有行尾可循，沿用行表的默认风格。
    assert_eq!(text(&m).replace("\r\n", "\n"), "hello\nworld");
    assert!(m.is_dirty());
}
