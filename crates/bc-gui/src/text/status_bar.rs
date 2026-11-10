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
        [rect.left_top(), rect.left_bottom()],
        egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
    );
    // 参考：右缘 1 px 白线（分隔右侧窗格或窗口边）。
    painter.rect_filled(
        egui::Rect::from_min_max(rect.right_top() - egui::vec2(1.0, 0.0), rect.right_bottom()),
        0.0,
        egui::Color32::WHITE,
    );
    // 参考：顶部 215 细分隔线，底部白色高光和 160 凹槽线。
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, egui::Color32::from_gray(215)),
    );
    painter.line_segment(
        [
            rect.left_bottom() - egui::vec2(0.0, 2.0),
            rect.right_bottom() - egui::vec2(0.0, 2.0),
        ],
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    );
    painter.line_segment(
        [
            rect.left_bottom() - egui::vec2(0.0, 1.0),
            rect.right_bottom() - egui::vec2(0.0, 1.0),
        ],
        egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
    );
    for x in [98.0, 229.0, 360.0] {
        painter.line_segment(
            [
                rect.min + egui::vec2(x, 1.0),
                rect.min + egui::vec2(x, 18.0),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_gray(214)),
        );
    }
    for (offset, label) in [
        (4.0, format!("{}: {}", editor.caret.line + 1, column)),
        (104.0, "Default text".into()),
        (378.0, "\u{2014}".into()),
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
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
    // 参考行详情：左侧 160 边、两条内容行（白底/灰白底文本框）、底部白色高光。
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
    );
    painter.line_segment(
        [
            rect.left_bottom() - egui::vec2(0.0, 1.0),
            rect.right_bottom() - egui::vec2(0.0, 1.0),
        ],
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    );
    for s in 0..2 {
        let top = rect.top() + s as f32 * 16.0;
        if s == 1 {
            painter.line_segment(
                [
                    egui::pos2(rect.left(), top - 1.0),
                    egui::pos2(rect.right(), top - 1.0),
                ],
                egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
            );
            painter.line_segment(
                [
                    egui::pos2(rect.left(), top + 15.0),
                    egui::pos2(rect.right(), top + 15.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }
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
        let galley = painter.layout_no_wrap(
            format!("{}{end}", &text[..limit]),
            egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE),
            egui::Color32::BLACK,
        );
        // 收缩包裹的文本框：第一行纯白，第二行 #FCFCFC。
        let text_x = rect.left() + 20.0;
        let box_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 19.0, top),
            egui::pos2(
                text_x + galley.size().x + 3.0,
                (top + 15.0).min(rect.bottom()),
            ),
        );
        painter.rect_filled(
            box_rect,
            0.0,
            if s == 0 {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(252, 252, 252)
            },
        );
        painter.line_segment(
            [box_rect.left_top(), box_rect.left_bottom()],
            egui::Stroke::new(1.0, egui::Color32::from_gray(192)),
        );
        // 参考箭头：灰描边浅蓝填充。
        let c = egui::pos2(rect.left() + 8.0, top + 7.5);
        let dir = if s == 0 { 1.0 } else { -1.0 };
        let points = [
            (-5.0, -2.0),
            (1.0, -2.0),
            (1.0, -4.0),
            (6.0, 0.0),
            (1.0, 4.0),
            (1.0, 2.0),
            (-5.0, 2.0),
        ]
        .into_iter()
        .map(|(x, y)| c + egui::vec2(x * dir, y))
        .collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            egui::Color32::from_rgb(215, 233, 249),
            egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
        ));
        painter.galley(egui::pos2(text_x, top + 1.0), galley, egui::Color32::BLACK);
    }
    // 参考第三行是静态占位「—」。
    painter.text(
        rect.left_center() + egui::vec2(21.0, 8.0),
        egui::Align2::LEFT_CENTER,
        "\u{2014}",
        egui::FontId::proportional(12.0),
        egui::Color32::BLACK,
    );
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
/// File modification label resolved off the UI thread: the local-time host
/// query costs ~150 ms per file, which used to stall every text open.
#[derive(Default)]
pub struct ModifiedLabel {
    text: String,
    pending: Option<std::sync::mpsc::Receiver<String>>,
}
impl ModifiedLabel {
    pub fn resolve(time: Option<std::time::SystemTime>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = time.is_some()
            && std::thread::Builder::new()
                .name("matcha-modified-label".into())
                .spawn(move || {
                    let t = std::time::Instant::now();
                    let _ = tx.send(modified_label(time));
                    crate::perf::span("text:modified-label", t);
                })
                .is_ok();
        Self {
            text: if spawned || time.is_none() {
                String::new()
            } else {
                modified_label(time)
            },
            pending: spawned.then_some(rx),
        }
    }
    /// Pick up a finished lookup; true while one is still running.
    pub fn poll(&mut self) -> bool {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(text) => {
                    self.text = text;
                    self.pending = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        self.pending.is_some()
    }
    pub fn text(&self) -> &str {
        &self.text
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
