use super::*;
use bc_script::cli::MergeSpec;

/// 临时目录：三份 CSV 输入与输出路径，测试结束删除。
pub(crate) struct Fixture {
    pub dir: PathBuf,
    pub launch: MergeLaunch,
}

impl Fixture {
    pub(crate) fn new(tag: &str, left: &str, right: &str, center: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "matcha-gui-table-merge-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, text: &str| {
            let path = dir.join(name);
            std::fs::write(&path, text).unwrap();
            path
        };
        let mut launch = MergeLaunch::new(MergeSpec {
            left: file("left.csv", left),
            right: file("right.csv", right),
            center: file("base.csv", center),
            output: Some(dir.join("out.csv")),
        });
        launch.table = true;
        Self { dir, launch }
    }
    pub(crate) fn output(&self) -> PathBuf {
        self.dir.join("out.csv")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(crate) const BASE: &str = "id,name,qty\r\n1,apple,10\r\n2,pear,20\r\n3,plum,30\r\n";

/// left 改 1 号数量为 12、删 2 号；right 改 1 号数量为 15、改 2 号名称、新增 4 号。
pub(crate) fn conflicted(tag: &str) -> Fixture {
    Fixture::new(
        tag,
        "id,name,qty\r\n1,apple,12\r\n3,plum,30\r\n",
        "id,name,qty\r\n1,apple,15\r\n2,PEAR,20\r\n3,plum,30\r\n4,fig,40\r\n",
        BASE,
    )
}

#[test]
fn load_caches_conflicts_and_result_codes() {
    let f = conflicted("load");
    let mut model = TableMergeModel::load(&f.launch).unwrap();
    assert_eq!(model.sheets().len(), 1);
    // 1 号的异改格与 2 号的删改冲突相邻，聚成一个区段。
    assert_eq!(model.unresolved(), 1);
    assert_eq!(model.conflicts()[0].rows, 0..2);
    assert_eq!(model.section_at(1), Some(0..2));
    assert!(model.in_conflict(0));
    assert!(model.is_dirty());
    assert_eq!(model.result_code(), 101);
    model.save(&f.output()).unwrap();
    assert_eq!(model.result_code(), 14);
    assert!(!model.is_dirty());
    // 未解决：异改格保留 base，删改冲突保留 center 行；右侧新增照收。
    assert_eq!(
        std::fs::read_to_string(f.output()).unwrap(),
        "id,name,qty\r\n1,apple,10\r\n2,pear,20\r\n3,plum,30\r\n4,fig,40\r\n"
    );
}

#[test]
fn take_accept_undo_redo() {
    let f = conflicted("take");
    let mut model = TableMergeModel::load(&f.launch).unwrap();
    assert!(model.take(0..2, Side::Right));
    assert_eq!(model.unresolved(), 0);
    assert!(model.can_undo());
    assert_eq!(model.undo(), Some(0));
    assert_eq!(model.unresolved(), 1);
    assert_eq!(model.redo(), Some(0));
    assert_eq!(model.unresolved(), 0);
    model.save(&f.output()).unwrap();
    assert_eq!(model.result_code(), 0);
    assert_eq!(
        std::fs::read_to_string(f.output()).unwrap(),
        "id,name,qty\r\n1,apple,15\r\n2,PEAR,20\r\n3,plum,30\r\n4,fig,40\r\n"
    );
    // 撤销到冲突状态、接受当前内容同样算解决。
    model.undo();
    assert!(model.accept(0..2));
    assert_eq!(model.unresolved(), 0);
    assert!(model.is_dirty());
    // 没有变化的操作不进撤销栈。
    let depth = model.undo.len();
    assert!(!model.accept(0..2));
    assert_eq!(model.undo.len(), depth);
}

#[test]
fn editing_an_output_cell_resolves_it() {
    let f = Fixture::new(
        "edit",
        "id,name,qty\r\n1,apple,12\r\n2,pear,20\r\n3,plum,30\r\n",
        "id,name,qty\r\n1,apple,15\r\n2,pear,20\r\n3,plum,30\r\n",
        BASE,
    );
    let mut model = TableMergeModel::load(&f.launch).unwrap();
    assert_eq!(model.unresolved(), 1);
    assert!(model.edit(0, 2, "13"));
    assert_eq!(model.unresolved(), 0);
    let (cell, text, _) = model.current().output_cell(0, 2).unwrap();
    assert_eq!(cell.value, bc_engine_table::CellValue::Number(13.0));
    assert_eq!(text, "13");
    // 单格取侧。
    assert!(model.take_cell(0, 2, Side::Left));
    assert_eq!(model.current().output_cell(0, 2).unwrap().1, "12");
}

#[test]
fn missing_input_fails_to_load() {
    let f = conflicted("missing");
    std::fs::remove_file(&f.launch.spec.center).unwrap();
    let error = TableMergeModel::load(&f.launch).err().unwrap();
    assert!(error.contains("does not exist"), "{error}");
}

#[test]
fn next_change_walks_changes_even_without_conflicts() {
    // 只有自动合并改动（无冲突）：导航逐段走改动，打开落点是第一个改动。
    let f = Fixture::new(
        "changes",
        "id,name,qty\r\n1,apple,12\r\n2,pear,20\r\n3,plum,30\r\n",
        "id,name,qty\r\n1,apple,10\r\n2,pear,20\r\n3,plum,33\r\n",
        BASE,
    );
    let model = TableMergeModel::load(&f.launch).unwrap();
    assert_eq!(model.unresolved(), 0);
    assert!(model.has_changes());
    // 两处改动各自成段（1 号左改、3 号右改，2 号未动隔开）。
    assert_eq!(model.changes(), &[0..1, 2..3]);
    assert_eq!(model.first_conflict_or_change(), Some(0));
    assert_eq!(model.next_change(0, true), Some(2));
    assert_eq!(model.next_change(2, true), None);
    assert_eq!(model.next_change(2, false), Some(0));
    assert_eq!(model.next_change(0, false), None);
    assert_eq!(model.next_change_sheet(true), None);
}
