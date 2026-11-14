//! `/automerge` without a window: `runner::auto_merge_files` (spec `text-merge-cli`).
use bc_engine_text::{merge::MergeLabels, rules::Favor, TextMergeRules};
use bc_script::{
    runner::{auto_merge_files, AutoMergeRequest},
    ExitCode,
};
use std::path::{Path, PathBuf};

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "matcha-automerge-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const BASE: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";

fn request(left: &Path, right: &Path, center: &Path, output: &Path) -> AutoMergeRequest {
    AutoMergeRequest {
        left: left.into(),
        right: right.into(),
        center: center.into(),
        output: output.into(),
        rules: TextMergeRules::default(),
        labels: MergeLabels {
            left: "theirs".into(),
            right: "yours".into(),
        },
    }
}

#[test]
fn clean_merge_writes_the_output_and_returns_success() {
    let dir = Dir::new("clean");
    let left = dir.file("a.txt", &BASE.replace("two", "TWO"));
    let right = dir.file("b.txt", &BASE.replace("seven", "SEVEN"));
    let center = dir.file("base.txt", BASE);
    let out = dir.0.join("out.txt");
    let report = auto_merge_files(&request(&left, &right, &center, &out)).unwrap();
    assert_eq!(report.code, ExitCode::Success);
    assert!(report.written && report.conflicts == 0);
    assert_eq!(report.summary(), "Success");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        BASE.replace("two", "TWO").replace("seven", "SEVEN")
    );
    // Nothing but the inputs and the output: no backup, no temporary file left behind.
    assert_eq!(dir.names(), ["a.txt", "b.txt", "base.txt", "out.txt"]);
}

#[test]
fn conflicts_without_force_leave_an_existing_output_alone() {
    let dir = Dir::new("withheld");
    let left = dir.file("a.txt", &BASE.replace("four", "LEFT"));
    let right = dir.file("b.txt", &BASE.replace("four", "RIGHT"));
    let center = dir.file("base.txt", BASE);
    let out = dir.file("out.txt", "previous content\n");
    let modified = std::fs::metadata(&out).unwrap().modified().unwrap();
    let report = auto_merge_files(&request(&left, &right, &center, &out)).unwrap();
    assert_eq!(report.code, ExitCode::ConflictsMergeNotWritten);
    assert_eq!(report.code.code(), 101);
    assert!(!report.written);
    assert_eq!(report.conflicts, 1);
    assert_eq!(
        report.summary(),
        "Conflicts detected, merge output not written: 1 conflict section(s)"
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "previous content\n");
    assert_eq!(std::fs::metadata(&out).unwrap().modified().unwrap(), modified);
    // No output at all when there was none.
    std::fs::remove_file(&out).unwrap();
    auto_merge_files(&request(&left, &right, &center, &out)).unwrap();
    assert!(!out.exists());
}

#[test]
fn force_writes_markers_and_returns_14() {
    let dir = Dir::new("force");
    let left = dir.file("a.txt", &BASE.replace("four", "LEFT"));
    let right = dir.file("b.txt", &BASE.replace("four", "RIGHT"));
    let center = dir.file("base.txt", BASE);
    let out = dir.file("out.txt", "previous content\n");
    let mut request = request(&left, &right, &center, &out);
    request.rules.force_conflict_markers = true;
    let report = auto_merge_files(&request).unwrap();
    assert_eq!(report.code, ExitCode::ConflictsDetected);
    assert_eq!(report.code.code(), 14);
    assert!(report.written);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        BASE.replace("four\n", "<<<<<<< theirs\nLEFT\n=======\nRIGHT\n>>>>>>> yours\n")
    );
}

#[test]
fn the_output_may_replace_one_of_the_inputs() {
    let dir = Dir::new("inplace");
    let left = dir.file("a.txt", &BASE.replace("two", "TWO"));
    let right = dir.file("b.txt", &BASE.replace("seven", "SEVEN"));
    let center = dir.file("base.txt", BASE);
    let report = auto_merge_files(&request(&left, &right, &center, &right)).unwrap();
    assert_eq!(report.code, ExitCode::Success);
    assert_eq!(
        std::fs::read_to_string(&right).unwrap(),
        BASE.replace("two", "TWO").replace("seven", "SEVEN")
    );
    assert_eq!(dir.names(), ["a.txt", "b.txt", "base.txt"]);
}

#[test]
fn unusable_inputs_are_errors_and_write_nothing() {
    let dir = Dir::new("errors");
    let left = dir.file("a.txt", BASE);
    let right = dir.file("b.txt", BASE);
    let center = dir.file("base.txt", BASE);
    let out = dir.0.join("out.txt");
    let missing = dir.0.join("missing.txt");
    let error = auto_merge_files(&request(&missing, &right, &center, &out)).unwrap_err();
    assert!(error.contains("missing.txt") && error.contains("does not exist"), "{error}");
    let error = auto_merge_files(&request(&left, &dir.0, &center, &out)).unwrap_err();
    assert!(error.contains("is a folder"), "{error}");
    let binary = dir.0.join("book.xlsx");
    std::fs::write(&binary, [b'P', b'K', 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    let error = auto_merge_files(&request(&left, &right, &binary, &out)).unwrap_err();
    assert!(error.contains("book.xlsx") && error.contains("not a text file"), "{error}");
    let nowhere = dir.0.join("no-such-folder").join("out.txt");
    let error = auto_merge_files(&request(&left, &right, &center, &nowhere)).unwrap_err();
    assert!(error.contains("cannot write"), "{error}");
    assert!(!out.exists());
}

#[test]
fn csv_files_merge_as_text_and_switches_reach_the_engine() {
    let dir = Dir::new("csv");
    let base = "id,name\n1,ann\n2,bob\n3,cy\n4,dee\n5,eve\n6,fay\n";
    let left = dir.file("a.csv", &base.replace("1,ann", "1,ANN"));
    let right = dir.file("b.csv", &base.replace("6,fay", "6,FAY"));
    let center = dir.file("base.csv", base);
    let out = dir.0.join("out.csv");
    let report = auto_merge_files(&request(&left, &right, &center, &out)).unwrap();
    assert_eq!(report.code, ExitCode::Success);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        base.replace("1,ann", "1,ANN").replace("6,fay", "6,FAY")
    );
    // `/iu /favorright` on a conflict that is only a line-ending-insensitive match is a
    // no-op under the default rules: the switches are passed through, not interpreted.
    let left = dir.file("a.csv", &base.replace("3,cy", "3,LEFT"));
    let right = dir.file("b.csv", &base.replace("3,cy", "3,RIGHT"));
    let mut request = request(&left, &right, &center, &out);
    request.rules.ignore_unimportant = true;
    request.rules.favor = Favor::Right;
    let report = auto_merge_files(&request).unwrap();
    assert_eq!(report.code, ExitCode::ConflictsMergeNotWritten);
}
