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
    painter.rect_filled(rect, 0.0, crate::chrome::theme::SURFACE_SUBTLE);
    for line in [
        [rect.left_top(), rect.left_bottom()],
        [rect.left_top(), rect.right_top()],
        [
            rect.left_bottom() - egui::vec2(0.0, 0.5),
            rect.right_bottom() - egui::vec2(0.0, 0.5),
        ],
    ] {
        painter.line_segment(line, egui::Stroke::new(1.0, crate::chrome::theme::LINE));
    }
    for x in [98.0, 229.0, 360.0] {
        painter.line_segment(
            [
                rect.min + egui::vec2(x, 4.0),
                rect.min + egui::vec2(x, 15.0),
            ],
            egui::Stroke::new(1.0, crate::chrome::theme::LINE),
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
            egui::FontId::proportional(11.5),
            crate::chrome::theme::TEXT_MUTED,
        );
    }
}
/// A line index and its character-level difference segments (byte range, ink).
pub type DetailSegments = (usize, Vec<(std::ops::Range<usize>, egui::Color32)>);
// As in the panes, cap a single text layout at 4 KiB (on a UTF-8 boundary).
const DETAIL_PREVIEW_BYTES: usize = 4096;

pub(super) fn details(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    sides: [(&TextContent, &Editor); 2],
    style: Option<(egui::Color32, egui::Color32)>,
    inline: [Option<DetailSegments>; 2],
    active: usize,
    scroll_x: &mut f32,
) -> Option<egui::Response> {
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
    const GUTTER: f32 = 19.0;
    let left = rect.left() + GUTTER;
    let view = (rect.right() - left).max(0.0);
    let font = egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE);
    let ink = style.map_or(egui::Color32::BLACK, |(_, ink)| ink);
    // Use the reference's integer cell advance in the text layout itself so the
    // scrollbar extent, caret and ending agree with the painted glyphs.
    let advance = ui.fonts_mut(|fonts| fonts.glyph_width(&font, '0'));
    let spacing = advance.round() - advance;
    // Measure what is actually painted (including tabs, fallback glyphs and the line ending),
    // not the byte length or the scroll extent of the panes above.
    let rows = [0, 1].map(|s| {
        let (content, editor) = sides[s];
        let lines = content.lines.borrow();
        let line = editor.caret.line;
        let text = std::str::from_utf8(lines.line_bytes(line).unwrap_or_default()).unwrap();
        let mut limit = text.len().min(DETAIL_PREVIEW_BYTES);
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        let truncated = limit < text.len();
        let ending = if truncated { None } else { lines.ending(line) };
        let end = match ending {
            Some(LineEnding::Lf) => "\u{21b5}",
            Some(LineEnding::Cr) => "\u{2190}",
            _ if truncated => "…",
            _ => "",
        };
        let marker = painter.layout_no_wrap(end.to_owned(), font.clone(), MARKER);
        let marker_w = if ending == Some(LineEnding::CrLf) {
            7.0
        } else {
            marker.size().x
        };
        let segments = inline[s]
            .as_ref()
            .filter(|(at, _)| *at == line)
            .map(|(_, seg)| seg.as_slice());
        let text = &text[..limit];
        let galley = painter.layout_job(detail_job(text, &font, spacing, ink, segments));
        let caret = (s == active && editor.caret.byte <= limit).then(|| {
            text.char_indices()
                .take_while(|(b, _)| *b < editor.caret.byte)
                .count()
        });
        (galley, marker, marker_w, ending, caret)
    });
    let width = rows
        .iter()
        .map(|(g, _, w, _, _)| g.size().x + w)
        .fold(0.0, f32::max);
    let scrollbar = if width > view {
        if ui.is_enabled() && ui.rect_contains_pointer(rect) {
            let delta = crate::chrome::wheel::delta(ui.ctx(), &[rect]);
            *scroll_x -= delta.x + delta.y;
        }
        let band = egui::Rect::from_min_size(
            rect.min + egui::vec2(0.0, 32.0),
            egui::vec2(rect.width(), 16.0),
        );
        Some(crate::chrome::scrollbar::show_horizontal_px(
            ui,
            band,
            ui.make_persistent_id("text_details_scroll"),
            width,
            view,
            scroll_x,
        ))
    } else {
        *scroll_x = 0.0;
        // When both rows fit, retain the reference's inactive band.
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min + egui::vec2(18.0, 39.0), egui::vec2(17.0, 2.0)),
            0.0,
            DASH,
        );
        None
    };
    let text_x = left - *scroll_x;
    for (s, (galley, marker, marker_w, ending, caret)) in rows.into_iter().enumerate() {
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
        // The gutter arrows and the row separators stay fixed; everything from the
        // text box through its end marker moves together and is clipped at the gutter.
        let row_painter = painter.with_clip_rect(egui::Rect::from_min_max(
            egui::pos2(left, top),
            egui::pos2(rect.right(), top + 16.0),
        ));
        let marker_x = text_x + galley.size().x;
        // 收缩包裹的文本框：第一行纯白，第二行 #FCFCFC；右缘紧贴行尾符。
        let box_rect = egui::Rect::from_min_max(
            egui::pos2(text_x, top),
            egui::pos2(marker_x + marker_w, (top + 15.0).min(rect.bottom())),
        );
        // Equal rows: white / #FCFCFC with a #C0C0C0 left edge; difference
        // rows take the pane's row fill (second row: channel-wise min with
        // #FCFCFC, reference #EEEEFC) and no edge.
        let base = if s == 0 {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(252, 252, 252)
        };
        let fill = style.map_or(base, |(fill, _)| {
            egui::Color32::from_rgb(
                fill.r().min(base.r()),
                fill.g().min(base.g()),
                fill.b().min(base.b()),
            )
        });
        row_painter.rect_filled(box_rect, 0.0, fill);
        if style.is_none() {
            row_painter.line_segment(
                [box_rect.left_top(), box_rect.left_bottom()],
                egui::Stroke::new(1.0, CARET),
            );
        }
        // Side arrow (original vector): shaft 6 px, head 5 px and 10 rows,
        // spanning x+4..+15 around the row's center line.
        let c = egui::pos2(rect.left() + 10.0, top + 8.0);
        let dir = if s == 0 { 1.0 } else { -1.0 };
        let points = [
            (-5.5, -1.5),
            (0.5, -1.5),
            (0.5, -4.5),
            (5.5, 0.0),
            (0.5, 4.5),
            (0.5, 1.5),
            (-5.5, 1.5),
        ]
        .into_iter()
        .map(|(x, y)| c + egui::vec2(x * dir, y))
        .collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            egui::Color32::from_rgb(215, 233, 249),
            egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
        ));
        if let Some(caret) = caret {
            // Caret: a 2 px #C0C0C0 bar left of the caret column, clipped to the box.
            let prefix = galley
                .pos_from_cursor(egui::text::CCursor::new(caret))
                .left();
            let x = (text_x + prefix - 1.0).round();
            let bar = egui::Rect::from_min_max(
                egui::pos2(x.max(box_rect.left()), top),
                egui::pos2(x + 2.0, (top + 15.0).min(rect.bottom())),
            );
            row_painter.rect_filled(bar, 0.0, CARET);
        }
        row_painter.galley(egui::pos2(text_x, top + 2.0), galley, ink);
        if ending == Some(LineEnding::CrLf) {
            crlf_marker(&row_painter, egui::pos2(marker_x, top + 4.0));
        } else {
            row_painter.galley(egui::pos2(marker_x, top + 2.0), marker, MARKER);
        }
    }
    scrollbar
}
/// Details caret bar and equal-row box edge.
const CARET: egui::Color32 = egui::Color32::from_rgb(0xC0, 0xC0, 0xC0);
/// Visible line-end ink (reference gray).
const MARKER: egui::Color32 = egui::Color32::from_rgb(0xA0, 0xA0, 0xA0);
const DASH: egui::Color32 = egui::Color32::from_rgb(0x85, 0x85, 0x85);
/// CRLF marker in a 7x7 cell at `cell` (top-left): a 5x5 square with
/// corner ticks, drawn as unfeathered pixels.
fn crlf_marker(painter: &egui::Painter, cell: egui::Pos2) {
    let mut mesh = egui::Mesh::default();
    let mut px = |x: f32, y: f32, w: f32, h: f32| {
        mesh.add_colored_rect(
            egui::Rect::from_min_size(cell + egui::vec2(x, y), egui::vec2(w, h)),
            MARKER,
        );
    };
    px(1.0, 0.0, 1.0, 7.0);
    px(5.0, 0.0, 1.0, 7.0);
    px(2.0, 1.0, 3.0, 1.0);
    px(2.0, 5.0, 3.0, 1.0);
    painter.add(egui::Shape::mesh(mesh));
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
/// File modification label (`M/d/yyyy h:mm:ss AM`, local time), resolved
/// in-process when the file is opened.
#[derive(Default)]
pub struct ModifiedLabel {
    text: String,
}
impl ModifiedLabel {
    pub fn resolve(time: Option<std::time::SystemTime>) -> Self {
        Self {
            text: time.map(modified_label).unwrap_or_default(),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
/// Local time in the reference's invariant `M/d/yyyy h:mm:ss tt` form
/// (`bc_script::platform::local`: Windows time-zone rules, including the DST
/// offset in force at that moment). Other platforms have no local offset yet
/// and say `UTC`.
pub fn modified_label(time: std::time::SystemTime) -> String {
    let zone = if cfg!(windows) { "" } else { " UTC" };
    format!("{}{zone}", clock_label(&bc_script::platform::local(time)))
}
fn clock_label(t: &bc_script::platform::LocalTime) -> String {
    let hour = match t.hour % 12 {
        0 => 12,
        h => h,
    };
    let half = if t.hour < 12 { "AM" } else { "PM" };
    format!(
        "{}/{}/{} {hour}:{:02}:{:02} {half}",
        t.month, t.day, t.year, t.minute, t.second
    )
}

/// Line-detail text: uniform `ink`, or black with the inline difference segments inked.
fn detail_job(
    text: &str,
    font: &egui::FontId,
    spacing: f32,
    ink: egui::Color32,
    segments: Option<&[(std::ops::Range<usize>, egui::Color32)]>,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob {
        keep_trailing_whitespace: true,
        ..Default::default()
    };
    let format = |color| egui::TextFormat {
        font_id: font.clone(),
        extra_letter_spacing: spacing,
        color,
        ..Default::default()
    };
    let Some(segments) = segments else {
        job.append(text, 0.0, format(ink));
        return job;
    };
    let mut at = 0;
    for (range, color) in segments {
        let (start, end) = (range.start.min(text.len()), range.end.min(text.len()));
        if start < at || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            continue;
        }
        job.append(&text[at..start], 0.0, format(egui::Color32::BLACK));
        job.append(&text[start..end], 0.0, format(*color));
        at = end;
    }
    job.append(&text[at..], 0.0, format(egui::Color32::BLACK));
    job
}

#[cfg(test)]
mod clock_label_tests {
    use bc_script::platform::local_from_utc;

    #[test]
    fn uses_the_invariant_twelve_hour_form() {
        // 2026-11-11 07:41:49 UTC
        let label = |offset| super::clock_label(&local_from_utc(1_794_382_909, offset));
        assert_eq!(label(8 * 60), "11/11/2026 3:41:49 PM");
        assert_eq!(label(-(7 * 60 + 41)), "11/11/2026 12:00:49 AM");
        assert_eq!(label(4 * 60 + 18), "11/11/2026 11:59:49 AM");
        assert_eq!(label(4 * 60 + 19), "11/11/2026 12:00:49 PM");
    }
}
