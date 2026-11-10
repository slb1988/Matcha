//! 状态文字保持 O(1)，统计仅在比较结果/投影变化时重建。
use super::editor::Editor;
use bc_core::line::LineEnding;
use bc_engine_text::TextContent;
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
pub fn pane(ui: &mut egui::Ui, rect: egui::Rect, content: &TextContent, editor: &Editor) {
    let lines = content.lines.borrow();
    let text =
        std::str::from_utf8(lines.line_bytes(editor.caret.line).unwrap_or_default()).unwrap();
    let byte = editor.caret.byte.min(text.len());
    let column = text.char_indices().take_while(|(b, _)| *b < byte).count() + 1;
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    );
    for x in [98.0, 228.0, 360.0] {
        painter.line_segment(
            [
                rect.min + egui::vec2(x, 1.0),
                rect.min + egui::vec2(x, 18.0),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(214)),
        );
    }
    for (offset, label) in [
        (2.0, format!("{}: {}", editor.caret.line + 1, column)),
        (102.0, "Default text".into()),
    ] {
        painter.text(
            rect.left_center() + egui::vec2(offset, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            egui::Color32::BLACK,
        );
    }
}
pub fn details(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    contents: [&TextContent; 2],
    editors: &[Editor; 2],
) {
    use crate::chrome::icons::{self, Icon};
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
    for s in 0..2 {
        let top = rect.top() + s as f32 * 16.0;
        painter.line_segment(
            [egui::pos2(rect.left(), top), egui::pos2(rect.right(), top)],
            egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
        );
        icons::paint(
            &painter,
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 2.0, top + 2.0),
                egui::vec2(12.0, 12.0),
            ),
            if s == 0 {
                Icon::ArrowRight
            } else {
                Icon::ArrowLeft
            },
        );
        let lines = contents[s].lines.borrow();
        let line = editors[s].caret.line;
        let text = std::str::from_utf8(lines.line_bytes(line).unwrap_or_default()).unwrap();
        let end = match lines.ending(line) {
            Some(LineEnding::CrLf) => "¶",
            Some(LineEnding::Lf) => "↵",
            Some(LineEnding::Cr) => "←",
            _ => "",
        };
        let mut limit = text.len().min(4096);
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        painter.text(
            egui::pos2(rect.left() + 18.0, top + 1.0),
            egui::Align2::LEFT_TOP,
            format!("{}{end}", &text[..limit]),
            egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE),
            egui::Color32::BLACK,
        );
    }
}

pub fn encoding_label(content: &TextContent) -> String {
    use bc_core::encoding::Encoding;
    let name = match content.encoding {
        Encoding::Utf8 => "UTF-8".into(),
        Encoding::Utf16Le => "UTF-16 LE".into(),
        Encoding::Utf16Be => "UTF-16 BE".into(),
        Encoding::CodePage(n) => format!("CP {n}"),
        Encoding::Binary => "Binary".into(),
    };
    format!(
        "{name}{}",
        if content.detection.as_ref().is_some_and(|d| d.has_bom) {
            " BOM"
        } else {
            ""
        }
    )
}
pub fn ending_label(content: &TextContent) -> &'static str {
    match content.lines.borrow().dominant_ending() {
        LineEnding::CrLf => "PC",
        LineEnding::Lf => "Unix",
        LineEnding::Cr => "Mac",
        LineEnding::None => "",
    }
}
/// 在加载时缓存系统本地时间；没有平台 FFI、依赖或逐帧 I/O；失败时明确回退 UTC。
pub fn modified_label(time: Option<std::time::SystemTime>) -> String {
    let Some(seconds) = time
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
    else {
        return String::new();
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let command=format!("[DateTimeOffset]::FromUnixTimeSeconds({seconds}).LocalDateTime.ToString('M/d/yyyy h:mm:ss tt',[Globalization.CultureInfo]::InvariantCulture)");
        if let Ok(output) = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .creation_flags(0x0800_0000)
            .output()
        {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_owned();
            }
        }
    }
    let days = seconds / 86400 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    let h = seconds / 3600 % 24;
    let m = seconds / 60 % 60;
    let sec = seconds % 60;
    format!("{month}/{day}/{year} {h}:{m:02}:{sec:02} UTC")
}
