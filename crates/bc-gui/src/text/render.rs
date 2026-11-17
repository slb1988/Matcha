//! O(可见行)绘制；UTF-8 行内区间、光标和鼠标命中使用同一 galley。
use super::{
    editor::{byte_at_char, Caret},
    gutter,
    projection::{DisplayFilter, DisplayRow, RowGeometry},
    syntax_style::{ElementStyle, SyntaxStyle},
    view::{editor_id, index},
    TextCompareView,
};
use bc_core::{line::LineEnding, DiffKind};
use bc_engine_text::{
    importance::{Importance, LineView},
    inline::{inline_diff_classified, InlineOptions},
};
use bc_formats::grammar::Token;
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
            .rect_filled(rect, 0.0, crate::chrome::theme::SURFACE);
        // 窗格只有左边框与顶边框（发丝线）。
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
        let geometry = RowGeometry::for_row_height(row_height);
        // Clamp before either pane renders, including programmatic jumps in short files.
        self.scroll.y = self
            .scroll
            .y
            .min((self.projection.height(geometry) - remaining + 16.0).max(0.0));
        let scroll = self.scroll;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().scroll.bar_width = 16.0;
        ui.spacing_mut().scroll.bar_inner_margin = 1.0;
        ui.spacing_mut().scroll.bar_outer_margin = 1.0;
        ui.add_space(1.0);
        // 参考在内容放得下时不显示水平滚动条。
        let need_hscroll = self.content_width > rect.width() - 20.0;
        let need_vscroll_pre = self.projection.height(geometry)
            > remaining - 1.0 - if need_hscroll { 18.0 } else { 0.0 };
        // 行响应盖在窗格响应之上：行上的右键合并进窗格响应，统一打开右键菜单。
        let mut menu_target = response.clone();
        // 选区箭头所覆盖的本侧整行：每帧算一次。
        let selected = self.selected_line_range(s);
        let bold = crate::chrome::theme::editor_bold(ui.ctx(), self.font_size);
        // 按本侧文件格式（语言）取语法风格：每帧每窗格一次。
        let syntax = SyntaxStyle::for_format(snapshot.format.as_deref());
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
            // `ScrollArea::show_rows` with per-row heights from the projection
            // (gap folds are thin rows); the arithmetic is egui's own when
            // every row has the same height.
            .show_viewport(ui, |ui, viewport| {
                let total = self.projection.len();
                ui.set_height(self.projection.height(geometry).max(0.0));
                // Canvas below the last line (white "sheet" stops at EOF).
                let end = ui.max_rect().top() + self.projection.height(geometry);
                let clip = ui.clip_rect();
                if end < clip.bottom() {
                    let below = egui::Rect::from_min_max(
                        egui::pos2(clip.left(), end.max(clip.top())),
                        clip.max,
                    );
                    crate::chrome::theme::paint_canvas(&ui.painter().with_clip_rect(below), below);
                }
                let mut min_row = self.projection.display_at(viewport.min.y, geometry);
                let mut max_row = if self.projection.has_thin_folds() {
                    self.projection.display_at(viewport.max.y, geometry) + 2
                } else {
                    (viewport.max.y / row_height).ceil() as usize + 1
                };
                if max_row > total {
                    let diff = max_row.saturating_sub(min_row);
                    max_row = total;
                    min_row = total.saturating_sub(diff);
                }
                let y_min = ui.max_rect().top() + self.projection.y_of(min_row, geometry);
                let y_max = ui.max_rect().top() + self.projection.y_of(max_row, geometry);
                let rows = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(rows), |ui| {
                    ui.skip_ahead_auto_ids(min_row);
                    let range = min_row..max_row;
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let first = self.projection.row(range.start);
                    // The row range over-scans past the viewport; the thumbnail
                    // frame covers only rows that are (partly) shown.
                    let shown_end = if self.projection.has_thin_folds() {
                        range
                            .end
                            .min(self.projection.display_at(viewport.max.y - 0.5, geometry) + 1)
                    } else {
                        let shown = (ui.clip_rect().height() / row_height).ceil() as usize;
                        range.end.min(range.start + shown.max(1))
                    };
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
                                egui::vec2(
                                    ui.available_width().max(200.0),
                                    self.projection.row_height(display, geometry),
                                ),
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
                                    format!(
                                        "··· {} hidden rows — click to expand ···",
                                        hidden.len()
                                    ),
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
                        // 与参考一致按行着色：区块内只有注释等不重要差异的行显示为不重要（淡紫底蓝字），
                        // 同一区块的重要行仍为重要；行本身无分级时退回区块的重要性。
                        let mut importance = mapped.importance;
                        if importance == Importance::Same {
                            if let Some(section) = mapped.section.and_then(|i| self.sections.get(i))
                            {
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
                        if response.secondary_clicked() {
                            menu_target |= response.clone();
                        }
                        let viewport_rect = egui::Rect::from_min_max(
                            egui::pos2(ui.clip_rect().left(), rect.top()),
                            egui::pos2(ui.clip_rect().right(), rect.bottom()),
                        );
                        // 行号栏箭头：命中区注册在行响应之上，点击不落到文本。
                        let arrow = if self.show_numbers {
                            self.gutter_arrow(s, row, selected.as_ref())
                        } else {
                            None
                        };
                        // 只有首行的箭头可点；括号线只是标记。
                        let arrow_hit = arrow
                            .as_ref()
                            .filter(|a| a.mark == gutter::GutterMark::Arrow)
                            .map(|_| gutter::arrow_rect(viewport_rect));
                        let mut arrow_hovered = false;
                        if let (Some(arrow), Some(hit)) = (&arrow, arrow_hit) {
                            let enabled = arrow.disabled.is_none();
                            let hit_response = ui
                                .interact(
                                    hit,
                                    egui::Id::unique(("gutter_arrow", s, row)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text(arrow.tooltip(s));
                            arrow_hovered = hit_response.hovered();
                            if enabled && arrow_hovered {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            // 禁用时也受理点击：执行时在状态栏说明原因（目标只读等）。
                            if hit_response.clicked() {
                                self.pending_gutter_copy = Some((s, row, arrow.kind));
                            }
                        }
                        // 文本区（行号栏右侧）悬停显示 I 形插入光标。
                        let text_left = viewport_rect.left()
                            + if self.show_numbers {
                                gutter::WIDTH
                            } else {
                                0.0
                            };
                        if !arrow_hovered
                            && response.hovered()
                            && response.hover_pos().is_some_and(|p| p.x >= text_left)
                        {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                        }
                        let orphan =
                            matches!(mapped.kind, DiffKind::LeftOnly | DiffKind::RightOnly);
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
                        // 单侧差异行整行用差异色着墨；`keep_on_diff` 的元素（注释）保留自身风格。
                        let orphan_ink = orphan && importance != Importance::Same;
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
                            &LineInk {
                                normal: EDITOR_TEXT,
                                diff: orphan_ink.then(|| diff_ink(&prefs.colors, importance)),
                                syntax: &syntax,
                                bold: &bold,
                                unimportant: &self.map.rules.comparison.importance.unimportant,
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
                                    crate::chrome::theme::palette(ui.ctx()).text_selection,
                                );
                            }
                        }
                        ui.painter().galley(origin, galley.clone(), EDITOR_TEXT);
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
                                arrow.as_ref(),
                                arrow_hovered,
                                s,
                            );
                        }
                        let pressed = ui.input(|i| i.pointer.primary_pressed());
                        let held = ui.input(|i| i.pointer.primary_down());
                        let secondary = ui.input(|i| i.pointer.secondary_pressed());
                        let pointer = ui.input(|i| i.pointer.interact_pos());
                        // 弹出菜单（右键菜单/下拉）盖在窗格上时，按下属于菜单而非文本。
                        let covered = pointer.is_some_and(|p| {
                            (pressed || secondary)
                                && ui.ctx().layer_id_at(p).is_some_and(|l| l != ui.layer_id())
                        });
                        if let Some(p) = pointer.filter(|p| {
                            viewport_rect.contains(*p)
                                && self.pane_rects[s].contains(*p)
                                && !covered
                                // 在箭头上按下属于箭头（拖选经过箭头区仍照常扩展）。
                                && !((pressed || secondary)
                                    && arrow_hit.is_some_and(|h| h.contains(*p)))
                        }) {
                            // 右键：选区外先移动光标（选区内保留选区），供右键菜单作用。
                            if ui.is_enabled() && secondary {
                                if let Some(line) = line {
                                    self.active = s;
                                    self.current_row = row;
                                    ui.memory_mut(|m| m.request_focus(id));
                                    let cursor = galley.cursor_from_pos(p - origin);
                                    let byte = bytes.get(cursor.index.0).copied().unwrap_or(finish);
                                    let caret = Caret { line, byte };
                                    let inside = self.editors[s]
                                        .selection()
                                        .is_some_and(|(a, b)| a <= caret && caret <= b);
                                    if !inside {
                                        self.editors[s].place(caret, false);
                                        if let Some(line) = index(mapped, 1 - s) {
                                            self.editors[1 - s]
                                                .place(Caret { line, byte: 0 }, false);
                                        }
                                    }
                                } else {
                                    self.place_in_gap(s, row);
                                    ui.memory_mut(|m| m.request_focus(id));
                                }
                            }
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
                                } else if pressed {
                                    // 本侧空隙行：同样切换焦点；dragging 让松开时恢复焦点。
                                    self.place_in_gap(s, row);
                                    ui.memory_mut(|m| m.request_focus(id));
                                    self.dragging = Some(s);
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
                })
                .inner
            });
        // Row interactions sit above the pane's focus registration. Restore the
        // editor owner on mouse release after egui's click-elsewhere processing.
        // 拖选途中指针可能离开起始行（甚至窗格），光标保持 I 形。
        if self.dragging == Some(s) && ui.input(|i| i.pointer.primary_down()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if self.dragging == Some(s) && ui.input(|i| i.pointer.primary_released()) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        // 行号栏箭头点击：本窗格行循环结束后才改内容与对齐。
        if let Some((side, row, kind)) = self.pending_gutter_copy.take() {
            self.apply_gutter_copy(side, row, kind);
            ui.ctx().request_repaint();
        }
        // 右键菜单（参考 text-context）：作用于被点击的窗格。
        if menu_target.secondary_clicked() {
            self.active = s;
        }
        if let Some(action) =
            crate::chrome::popup::context_menu(&menu_target, || self.context_items(s))
        {
            self.context_command(s, &action, ui.ctx(), prefs);
        }
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
        );
        // 内容放得下时，egui 的竖向滚动条把手铺满轨道；此时直接盖成空白内容区。
        if !need_vscroll_pre {
            let bar_left = rect.right() - 18.0;
            let end = (rect.top() + 2.0 + self.projection.height(geometry) - scroll.y)
                .clamp(rect.top() + 1.0, rect.bottom());
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(bar_left, rect.top() + 1.0),
                    egui::pos2(rect.right(), end),
                ),
                0.0,
                crate::chrome::theme::SURFACE,
            );
            let below = egui::Rect::from_min_max(egui::pos2(bar_left, end), rect.right_bottom());
            if below.is_positive() {
                ui.painter()
                    .rect_filled(below, 0.0, crate::chrome::theme::CANVAS);
            }
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
    pub(super) fn ensure_inline(&mut self, row: usize) {
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
/// 编辑区正文色：参考为纯黑（界面其余文字用主题的近黑色）。
pub(crate) const EDITOR_TEXT: egui::Color32 = egui::Color32::BLACK;

/// 一行文字的着墨参数（[`layout`]）。
pub(crate) struct LineInk<'a> {
    /// 正文色（无语法元素、或元素无颜色时）。
    pub normal: egui::Color32,
    /// 单侧差异行的整行差异色；`None` 为普通行。
    pub diff: Option<egui::Color32>,
    /// 本侧语言的语法风格。
    pub syntax: &'a SyntaxStyle,
    /// 粗体字体（[`crate::chrome::theme::editor_bold`]）。
    pub bold: &'a egui::FontId,
    /// 差异不重要的语法元素（比较规则；默认是注释）。
    pub unimportant: &'a [bc_formats::ElementKind],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn layout(
    text: &str,
    start: usize,
    tokens: &[Token],
    spans: &[bc_engine_text::inline::InlineSpan],
    colors: &DiffColors,
    font: f32,
    ignore: bool,
    whitespace: bool,
    tab: usize,
    ink: &LineInk<'_>,
) -> (LayoutJob, Vec<usize>) {
    let mut job = LayoutJob::default();
    let mut mapping = Vec::new();
    let mut column = 0;
    for (offset, c) in text.char_indices() {
        let byte = start + offset;
        let token = tokens.iter().find(|t| t.span.contains(&byte));
        let style = token.map_or(ElementStyle::PLAIN, |t| ink.syntax.get(t.element));
        // 单侧差异行：不重要元素（默认是注释）用不重要差异色；`keep_on_diff` 的元素保留
        // 自身风格（注释被设为重要时仍可辨认）；其余用整行差异色。
        let color = match ink.diff {
            Some(_) if !ignore && token.is_some_and(|t| ink.unimportant.contains(&t.element)) => {
                diff_ink(colors, Importance::Unimportant)
            }
            Some(diff) if !style.keep_on_diff => diff,
            _ => style.color.unwrap_or(ink.normal),
        };
        let span = spans.iter().find(|s| {
            s.range.contains(&byte)
                && s.importance.with_ignore_unimportant(ignore) != Importance::Same
        });
        // 粗体用 Consolas Bold（与常规体等宽，列位置不变）。
        let font_id = if style.bold {
            ink.bold.clone()
        } else {
            egui::FontId::monospace(font)
        };
        let format = egui::TextFormat {
            font_id,
            color: span.map_or(color, |s| diff_ink(colors, s.importance)),
            italics: style.italic,
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

pub(crate) fn hatch(ui: &egui::Ui, rect: egui::Rect, cross: bool) {
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

pub(super) fn row_fill(colors: &DiffColors, importance: Importance, orphan: bool) -> egui::Color32 {
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
pub(super) fn diff_ink(colors: &DiffColors, importance: Importance) -> egui::Color32 {
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
