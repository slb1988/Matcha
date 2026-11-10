//! O(可见行)绘制；UTF-8 行内区间、光标和鼠标命中使用同一 galley。
use super::{
    editor::{byte_at_char, Caret},
    gutter,
    projection::{DisplayFilter, DisplayRow},
    view::{editor_id, index},
    TextCompareView,
};
use bc_core::{line::LineEnding, DiffKind};
use bc_engine_text::{
    importance::{Importance, LineView},
    inline::{inline_diff_classified, InlineOptions},
};
use bc_formats::{grammar::Token, ElementKind};
use bc_session::prefs::{DiffColors, Prefs};
use eframe::egui::{
    self,
    text::{ByteIndex, CCursor, LayoutJob},
};

impl TextCompareView {
    pub(crate) fn pane(&mut self, ui: &mut egui::Ui, s: usize, height: f32, prefs: &Prefs) {
        let top = ui.cursor().top();
        self.header(ui, s, prefs);
        let remaining = (height - (ui.cursor().top() - top) - 19.0).max(40.0);
        let rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), remaining));
        self.pane_rects[s] = rect;
        ui.painter()
            .rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
        // 参考窗格只有左边框与顶边框；右侧/底部不画深色边。
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
        );
        let id = editor_id(s);
        let response = ui.interact(rect, id, egui::Sense::click());
        if response.clicked() {
            self.active = s;
            response.request_focus();
        }
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let snapshot = self.content(s).snapshot();
        let row_height = self.row_height();
        // Clamp before either pane renders, including programmatic jumps in short files.
        self.scroll.y = self
            .scroll
            .y
            .min((self.projection.len() as f32 * row_height - remaining + 16.0).max(0.0));
        let scroll = self.scroll;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().scroll.bar_width = 16.0;
        ui.spacing_mut().scroll.bar_inner_margin = 1.0;
        ui.spacing_mut().scroll.bar_outer_margin = 1.0;
        ui.add_space(1.0);
        // 参考在内容放得下时不显示水平滚动条。
        let need_hscroll = self.content_width > rect.width() - 20.0;
        let need_vscroll_pre = self.projection.len() as f32 * row_height
            > remaining - 1.0 - if need_hscroll { 18.0 } else { 0.0 };
        let output = egui::ScrollArea::new([need_hscroll, true])
            .id_salt(("text_scroll", s))
            // Wheel input is applied raw by `TextCompareView::ui` before
            // either pane renders (no smoothing glide, panes in lockstep).
            .scroll_source(crate::chrome::wheel::scroll_source())
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .max_height(remaining - 1.0)
            .vertical_scroll_offset(scroll.y)
            .horizontal_scroll_offset(scroll.x)
            .show_rows(ui, row_height, self.projection.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let first = self.projection.row(range.start);
                // `show_rows` over-scans one row past the viewport; the
                // thumbnail frame covers only rows that are (partly) shown.
                let shown = (ui.clip_rect().height() / row_height).ceil() as usize;
                let shown_end = range.end.min(range.start + shown.max(1));
                let last = self.projection.row(shown_end.saturating_sub(1));
                // Shown source rows (the thumbnail viewport): a fold at either
                // edge contributes none of its hidden rows.
                let start = match first {
                    Some(DisplayRow::Text(n)) => n,
                    Some(DisplayRow::Fold(r)) => r.end,
                    None => 0,
                };
                let end = match last {
                    Some(DisplayRow::Text(n)) => n + 1,
                    Some(DisplayRow::Fold(r)) => r.start,
                    None => 0,
                };
                self.visible = start..end.max(start);
                for display in range {
                    let Some(projected) = self.projection.row(display) else {
                        continue;
                    };
                    if let DisplayRow::Fold(hidden) = projected {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width().max(200.0), row_height),
                            egui::Sense::click(),
                        );
                        let viewport_rect = egui::Rect::from_min_max(
                            egui::pos2(ui.clip_rect().left(), rect.top()),
                            egui::pos2(ui.clip_rect().right(), rect.bottom()),
                        );
                        if self.filter == DisplayFilter::Context {
                            ui.painter().rect_filled(
                                viewport_rect,
                                0.0,
                                ui.visuals().faint_bg_color,
                            );
                            ui.painter().text(
                                viewport_rect.left_center() + egui::vec2(8.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("··· {} hidden rows — click to expand ···", hidden.len()),
                                egui::FontId::monospace(self.font_size),
                                ui.visuals().weak_text_color(),
                            );
                        } else {
                            // 参考：灰底、横向细分线、居中灰色小字「N FILTERED LINES」。
                            ui.painter().rect_filled(
                                viewport_rect,
                                0.0,
                                crate::chrome::theme::PANEL_BG,
                            );
                            let ink = egui::Color32::from_rgb(105, 105, 105);
                            let cy = viewport_rect.center().y;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(viewport_rect.left(), cy),
                                    egui::pos2(viewport_rect.right(), cy),
                                ],
                                egui::Stroke::new(1.0, ink),
                            );
                            let galley = ui.painter().layout_no_wrap(
                                format!("{} FILTERED LINES", hidden.len()),
                                egui::FontId::proportional(8.666667),
                                ink,
                            );
                            let text_rect = egui::Rect::from_center_size(
                                viewport_rect.center(),
                                galley.size() + egui::vec2(12.0, 0.0),
                            );
                            ui.painter().rect_filled(
                                text_rect,
                                0.0,
                                crate::chrome::theme::PANEL_BG,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    text_rect.center().x - galley.size().x / 2.0,
                                    cy - galley.size().y / 2.0,
                                ),
                                galley,
                                ink,
                            );
                        }
                        if response.clicked() {
                            self.expand(hidden);
                        }
                        continue;
                    }
                    let DisplayRow::Text(row) = projected else {
                        unreachable!()
                    };
                    let mapped = self.row(row);
                    let line = index(mapped, s);
                    let text = line
                        .and_then(|i| snapshot.lines.line_bytes(i))
                        .map(|b| std::str::from_utf8(b).unwrap())
                        .unwrap_or_default();
                    let ending = line.and_then(|i| snapshot.lines.ending(i));
                    let mut importance = mapped.importance;
                    // A mixed difference section is important, while each inline span retains its own importance.
                    if let Some(section) = mapped.section {
                        if let Some(section) = self.sections.get(section) {
                            importance = section.importance;
                        }
                    }
                    importance =
                        importance.with_ignore_unimportant(self.map.rules.ignore_unimportant);

                    let glyph = self.font_size * 0.62;
                    self.content_width = self
                        .content_width
                        .max((text.len() as f32 * glyph + 100.0).min(20_000_000.0));
                    let width = ui.available_width().max(self.content_width);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(width, row_height),
                        egui::Sense::click_and_drag(),
                    );
                    let viewport_rect = egui::Rect::from_min_max(
                        egui::pos2(ui.clip_rect().left(), rect.top()),
                        egui::pos2(ui.clip_rect().right(), rect.bottom()),
                    );
                    let orphan = matches!(mapped.kind, DiffKind::LeftOnly | DiffKind::RightOnly);
                    let fill = row_fill(&prefs.colors, importance, orphan);

                    ui.painter().rect_filled(viewport_rect, 0.0, fill);
                    if line.is_none() || row >= self.map.row_count() {
                        hatch(ui, viewport_rect, row >= self.map.row_count());
                    }
                    if row == self.current_row {
                        ui.painter().line_segment(
                            [viewport_rect.left_bottom(), viewport_rect.right_bottom()],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(190)),
                        );
                    }
                    self.ensure_inline(row);
                    let inline = self
                        .inline
                        .get(&row)
                        .map(|d| if s == 0 { &d.left[..] } else { &d.right[..] })
                        .unwrap_or_default();
                    let tokens = if self.show_syntax && !self.map.is_stale() {
                        line.filter(|i| *i < snapshot.lines.len())
                            .map(|i| snapshot.tokens(i))
                            .unwrap_or_default()
                    } else {
                        Default::default()
                    };
                    // Long lines use a horizontal window; never allocate a galley for the entire line.
                    let start_char = if text.len() > 4096 {
                        ((scroll.x / glyph).max(0.0) as usize).saturating_sub(32)
                    } else {
                        0
                    };
                    let start = byte_at_char(text, start_char);
                    let mut finish = (start + 4096).min(text.len());
                    while !text.is_char_boundary(finish) {
                        finish -= 1;
                    }
                    let window = &text[start..finish];
                    let (job, bytes) = layout(
                        window,
                        start,
                        &tokens,
                        inline,
                        &prefs.colors,
                        self.font_size,
                        self.map.rules.ignore_unimportant,
                        self.show_whitespace,
                        snapshot
                            .format
                            .as_ref()
                            .and_then(|f| f.tab_size)
                            .unwrap_or(4)
                            .clamp(1, 32) as usize,
                        if orphan && importance != Importance::Same {
                            diff_ink(&prefs.colors, importance)
                        } else {
                            ui.visuals().text_color()
                        },
                    );
                    let galley = ui.painter().layout_job(job);
                    let origin = rect.min
                        + egui::vec2(
                            if self.show_numbers {
                                gutter::WIDTH + 1.0
                            } else {
                                8.0
                            },
                            2.0,
                        )
                        + egui::vec2(start_char as f32 * glyph, 0.0);
                    let char_for_byte = |byte: usize| {
                        bytes
                            .partition_point(|b| *b < byte)
                            .min(galley.text().chars().count())
                    };
                    if let (Some(line), Some((a, b))) = (line, self.editors[s].selection()) {
                        if (a.line..=b.line).contains(&line) {
                            let a = if line == a.line { a.byte } else { 0 };
                            let b = if line == b.line { b.byte } else { text.len() };
                            let x1 = galley
                                .pos_from_cursor(CCursor::new(char_for_byte(a)))
                                .left();
                            let x2 = galley
                                .pos_from_cursor(CCursor::new(char_for_byte(b)))
                                .left();
                            let selection = egui::Rect::from_min_max(
                                origin + egui::vec2(x1, 0.0),
                                origin + egui::vec2(x2.max(x1 + 2.0), row_height),
                            );
                            ui.painter().rect_filled(
                                selection,
                                0.0,
                                ui.visuals().selection.bg_fill.gamma_multiply(0.55),
                            );
                        }
                    }
                    ui.painter()
                        .galley(origin, galley.clone(), ui.visuals().text_color());
                    if self.show_whitespace || ending == Some(LineEnding::None) {
                        let marker = match ending {
                            Some(LineEnding::Lf) => " LF",
                            Some(LineEnding::CrLf) => " CRLF",
                            Some(LineEnding::Cr) => " CR",
                            Some(LineEnding::None) => " ∅",
                            None => "",
                        };
                        if finish == text.len() {
                            ui.painter().text(
                                origin + egui::vec2(galley.size().x, 0.0),
                                egui::Align2::LEFT_TOP,
                                marker,
                                egui::FontId::monospace(self.font_size),
                                ui.visuals().weak_text_color(),
                            );
                        }
                    }
                    // 与参考一致：窗格即使未聚焦也画 caret。
                    if line == Some(self.editors[s].caret.line) {
                        let cursor = galley
                            .pos_from_cursor(CCursor::new(char_for_byte(
                                self.editors[s].caret.byte,
                            )))
                            .translate(origin.to_vec2());
                        ui.painter().line_segment(
                            [cursor.left_top(), cursor.left_bottom()],
                            egui::Stroke::new(1.5, ui.visuals().text_color()),
                        );
                        if !self.editors[s].preedit.is_empty() {
                            ui.painter().text(
                                cursor.left_top(),
                                egui::Align2::LEFT_TOP,
                                &self.editors[s].preedit,
                                egui::FontId::monospace(self.font_size),
                                egui::Color32::YELLOW,
                            );
                        }
                        if !self.content(s).read_only && ui.memory(|m| m.owns_ime_events(id)) {
                            ui.output_mut(|o| {
                                o.ime = Some(egui::output::IMEOutput {
                                    purpose: egui::IMEPurpose::Normal,
                                    rect: self.pane_rects[s],
                                    cursor_rect: cursor,
                                    should_interrupt_composition: false,
                                });
                                o.mutable_text_under_cursor = true;
                            });
                        }
                    }
                    // Gutter is fixed horizontally, after text painting so it cannot be overdrawn.
                    if self.show_numbers {
                        gutter::paint(
                            ui,
                            viewport_rect,
                            line.filter(|i| *i < snapshot.lines.len()),
                            row == self.current_row,
                            importance != Importance::Same,
                            s,
                        );
                    }
                    let pressed = ui.input(|i| i.pointer.primary_pressed());
                    let held = ui.input(|i| i.pointer.primary_down());
                    let pointer = ui.input(|i| i.pointer.interact_pos());
                    if let Some(p) = pointer
                        .filter(|p| viewport_rect.contains(*p) && self.pane_rects[s].contains(*p))
                    {
                        if ui.is_enabled() && (pressed || (held && self.dragging == Some(s))) {
                            if let Some(line) = line {
                                self.active = s;
                                self.current_row = row;
                                ui.memory_mut(|m| m.request_focus(id));
                                let cursor = galley.cursor_from_pos(p - origin);
                                let byte = bytes.get(cursor.index.0).copied().unwrap_or(finish);
                                let extend = !pressed || ui.input(|i| i.modifiers.shift);
                                self.editors[s].place(Caret { line, byte }, extend);
                                if let Some(line) = index(mapped, 1 - s) {
                                    self.editors[1 - s].place(Caret { line, byte: 0 }, false);
                                }
                                self.dragging = Some(s);
                                if pressed {
                                    if let Some((other, source_line)) = self.align_with.take() {
                                        if other != s {
                                            let (left, right) = if s == 0 {
                                                (line, source_line)
                                            } else {
                                                (source_line, line)
                                            };
                                            match self.map.overrides.insert(left, right) {
                                                Ok(()) => self.recompare(),
                                                Err(e) => self.message = e.to_string(),
                                            }
                                        } else {
                                            self.align_with = Some((other, source_line));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if response.double_clicked() {
                        if let Some(line) = line {
                            let mut a = self.editors[s].caret.byte.min(text.len());
                            let mut b = a;
                            while a > 0 {
                                let c = text[..a].chars().next_back().unwrap();
                                if !c.is_alphanumeric() && c != '_' {
                                    break;
                                }
                                a -= c.len_utf8();
                            }
                            while b < text.len() {
                                let c = text[b..].chars().next().unwrap();
                                if !c.is_alphanumeric() && c != '_' {
                                    break;
                                }
                                b += c.len_utf8();
                            }
                            self.editors[s].anchor = Some(Caret { line, byte: a });
                            self.editors[s].caret = Caret { line, byte: b };
                        }
                    }
                    self.last_stats.rows_drawn += 1;
                }
            });
        // Row interactions sit above the pane's focus registration. Restore the
        // editor owner on mouse release after egui's click-elsewhere processing.
        if self.dragging == Some(s) && ui.input(|i| i.pointer.primary_released()) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
        );
        // 内容放得下时，egui 的竖向滚动条把手铺满轨道；参考此时为平坦灰轨加两侧白线。
        if !need_vscroll_pre {
            let bar_left = rect.right() - 18.0;
            let top = rect.top() + 1.0;
            let bottom = rect.bottom();
            let p = ui.painter();
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(bar_left + 1.0, top),
                    egui::pos2(rect.right() - 1.0, bottom),
                ),
                0.0,
                crate::chrome::theme::PANEL_BG,
            );
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(bar_left, top),
                    egui::pos2(bar_left + 1.0, bottom),
                ),
                0.0,
                egui::Color32::WHITE,
            );
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.right() - 1.0, top),
                    egui::pos2(rect.right(), bottom),
                ),
                0.0,
                egui::Color32::WHITE,
            );
        }
        let status = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom()),
            egui::vec2(rect.width(), 19.0),
        );
        super::status_bar::pane(ui, status, self.content(s), &self.editors[s]);
        let offset = output.state.clamped_offset();
        if (offset - scroll).length() > 0.5 {
            self.scroll = offset;
            ui.ctx().request_repaint();
        }
    }
    fn ensure_inline(&mut self, row: usize) {
        if self.map.is_stale() || self.inline.contains_key(&row) {
            return;
        }
        let Some(r) = self
            .map
            .row(row)
            .filter(|r| r.importance != Importance::Same)
        else {
            return;
        };
        let (Some(l), Some(rr)) = (r.left, r.right) else {
            return;
        };
        if self.inline.len() > 512 {
            self.inline.clear();
        }
        let left = self.map.left.snapshot();
        let right = self.map.right.snapshot();
        let ltext = std::str::from_utf8(left.lines.line_bytes(l).unwrap()).unwrap();
        let rtext = std::str::from_utf8(right.lines.line_bytes(rr).unwrap()).unwrap();
        let lt = left.tokens(l);
        let rt = right.tokens(rr);
        let diff = inline_diff_classified(
            LineView {
                text: ltext,
                tokens: &lt,
                format: left.format.as_deref(),
                ending: left.lines.ending(l).unwrap(),
            },
            LineView {
                text: rtext,
                tokens: &rt,
                format: right.format.as_deref(),
                ending: right.lines.ending(rr).unwrap(),
            },
            &self.comparison,
            &InlineOptions::default(),
        );
        self.inline.insert(row, diff);
    }
}
#[allow(clippy::too_many_arguments)]
fn layout(
    text: &str,
    start: usize,
    tokens: &[Token],
    spans: &[bc_engine_text::inline::InlineSpan],
    colors: &DiffColors,
    font: f32,
    ignore: bool,
    whitespace: bool,
    tab: usize,
    normal: egui::Color32,
) -> (LayoutJob, Vec<usize>) {
    let mut job = LayoutJob::default();
    let mut mapping = Vec::new();
    let mut column = 0;
    for (offset, c) in text.char_indices() {
        let byte = start + offset;
        let token = tokens.iter().find(|t| t.span.contains(&byte));
        let color = token.map_or(normal, |t| syntax_color(t.element, normal));
        let span = spans.iter().find(|s| {
            s.range.contains(&byte)
                && s.importance.with_ignore_unimportant(ignore) != Importance::Same
        });
        let format = egui::TextFormat {
            font_id: egui::FontId::monospace(font),
            color: span.map_or(color, |s| diff_ink(colors, s.importance)),
            ..Default::default()
        };
        let value = if c == '\t' {
            let width = tab - column % tab;
            if whitespace {
                format!("→{}", " ".repeat(width - 1))
            } else {
                " ".repeat(width)
            }
        } else if c == ' ' && whitespace {
            "·".into()
        } else {
            c.to_string()
        };
        for _ in value.chars() {
            mapping.push(byte);
            column += 1;
        }
        let begin = job.text.len();
        job.text.push_str(&value);
        if let Some(last) = job
            .sections
            .last_mut()
            .filter(|section| section.format == format)
        {
            last.byte_range.end = ByteIndex(job.text.len());
        } else {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: ByteIndex(begin)..ByteIndex(job.text.len()),
                format,
            });
        }
    }
    mapping.push(start + text.len());
    (job, mapping)
}
fn syntax_color(kind: ElementKind, normal: egui::Color32) -> egui::Color32 {
    match kind {
        ElementKind::Comment => egui::Color32::from_rgb(115, 155, 115),
        ElementKind::Keyword | ElementKind::Directive => egui::Color32::from_rgb(180, 150, 230),
        ElementKind::String => egui::Color32::from_rgb(205, 165, 115),
        ElementKind::Number => egui::Color32::from_rgb(125, 195, 190),
        _ => normal,
    }
}

fn hatch(ui: &egui::Ui, rect: egui::Rect, cross: bool) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    // 参考为 8 px 周期平铺菱格位图（非几何线），颜色 #E6E6E6，相位锚定客户区原点。
    let _ = cross;
    const TILE: [u8; 8] = [
        0b0100_0010,
        0b0010_0100,
        0b0001_1000,
        0b0001_1000,
        0b0010_0100,
        0b0100_0010,
        0b1000_0001,
        0b1000_0001,
    ];
    let color = egui::Color32::from_rgb(230, 230, 230);
    let mut mesh = egui::Mesh {
        texture_id: egui::TextureId::Managed(0),
        ..Default::default()
    };
    let left = rect.left().floor() as i32;
    let right = rect.right().ceil() as i32;
    for py in rect.top().floor() as i32..rect.bottom().ceil() as i32 {
        let bits = TILE[(py as usize) % 8];
        let mut px = left;
        while px < right {
            if bits >> ((px as usize) % 8) & 1 == 1 {
                // 汇聚同一行里的连续 ON 像素为一条横段。
                let mut end = px + 1;
                while end < right && bits >> ((end as usize) % 8) & 1 == 1 {
                    end += 1;
                }
                mesh.add_rect_with_uv(
                    egui::Rect::from_min_max(
                        egui::pos2(px as f32, py as f32),
                        egui::pos2(end as f32, py as f32 + 1.0),
                    ),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(0.0, 0.0)),
                    color,
                );
                px = end;
            } else {
                px += 1;
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn row_fill(colors: &DiffColors, importance: Importance, orphan: bool) -> egui::Color32 {
    let defaults = DiffColors::default();
    let custom = if orphan {
        colors.inserted != defaults.inserted
    } else if importance == Importance::Unimportant {
        colors.unimportant != defaults.unimportant
    } else {
        colors.important != defaults.important
    };
    if importance == Importance::Same {
        egui::Color32::WHITE
    } else if custom {
        super::nav_bar::color(colors, importance, orphan, 38)
    } else if importance == Importance::Unimportant {
        egui::Color32::from_rgb(238, 238, 255)
    } else {
        crate::chrome::theme::Theme::default().difference_fill
    }
}
fn diff_ink(colors: &DiffColors, importance: Importance) -> egui::Color32 {
    let defaults = DiffColors::default();
    if importance == Importance::Unimportant {
        if colors.unimportant == defaults.unimportant {
            egui::Color32::BLUE
        } else {
            super::nav_bar::color(colors, importance, false, 255)
        }
    } else if colors.important == defaults.important {
        crate::chrome::theme::Theme::default().difference_text
    } else {
        super::nav_bar::color(colors, importance, false, 255)
    }
}
