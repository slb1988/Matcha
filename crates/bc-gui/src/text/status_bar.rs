//! 状态文字保持 O(1)，统计仅在比较结果/投影变化时重建。
use super::editor::Editor;
use bc_core::line::LineEnding;
use bc_engine_text::{map::DifferenceCounts, TextContent};
use eframe::egui;
pub fn side_text(content: &TextContent, editor: &Editor) -> String {
    let lines = content.lines.borrow();
    let text =
        std::str::from_utf8(lines.line_bytes(editor.caret.line).unwrap_or_default()).unwrap();
    let mut byte = editor.caret.byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    let col = text[..byte].chars().count() + 1;
    let eof = if lines.is_empty() || lines.ending(lines.len() - 1) != Some(LineEnding::None) {
        ""
    } else {
        " · no final newline"
    };
    format!(
        "{}:{} · {:?} · {:?} · {} · {}{}{} · decoding errors {}",
        editor.caret.line + 1,
        col,
        content.encoding,
        lines.ending_style(),
        content.format().map_or("Plain text", |f| f.name.as_str()),
        if content.read_only {
            "read-only"
        } else {
            "editable"
        },
        if content.is_dirty() {
            " · modified"
        } else {
            ""
        },
        eof,
        content.decoding_errors
    )
}
pub fn ui(
    ui: &mut egui::Ui,
    left: &TextContent,
    right: &TextContent,
    editors: &[Editor; 2],
    counts: DifferenceCounts,
    message: &str,
) {
    egui::Panel::bottom("text_status").show(ui, |ui| {
        ui.label(format!("Left  {}", side_text(left, &editors[0])));
        ui.label(format!("Right {}", side_text(right, &editors[1])));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} important · {} unimportant · {} left-only · {} right-only",
                counts.important, counts.unimportant, counts.left_only, counts.right_only
            ));
            if left.lines.borrow().ending_style() != right.lines.borrow().ending_style() {
                ui.label("Line ending styles differ");
            }
            let final_ending = |c: &TextContent| {
                let l = c.lines.borrow();
                l.len().checked_sub(1).and_then(|i| l.ending(i))
            };
            if final_ending(left) != final_ending(right) {
                ui.label("Final line endings differ");
            }
            if !message.is_empty() {
                ui.label(message);
            }
        });
    });
}
