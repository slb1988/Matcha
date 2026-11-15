//! Session Settings 对话框（任务 6.1）：Specs / Format / Comparison 三页。
//!
//! 作用域下拉初版仅 "Use for this view only"（全局默认随会话持久化后置，H02 备注）。
//! Format 页为格式名下拉（初版固定 "Detected (Everything Else)" + Same as left 勾选；
//! 格式注册表接入随 hex 格式落地后置）。

use crate::chrome::{
    dialog::{self, ButtonKind, Placement},
    widgets,
};
use bc_core::align::ByteAlignStrategy;
use eframe::egui;

use super::view::HexCompareView;

/// 对话框编辑草稿（确认时才应用到会话）。
#[derive(Debug, Clone)]
pub struct SessionDraft {
    pub left_path: String,
    pub right_path: String,
    pub disable_editing: [bool; 2],
    pub description: String,
    pub format_left: String,
    pub format_right: String,
    pub same_as_left: bool,
    pub alignment: ByteAlignStrategy,
    pub lock_threshold_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Page {
    #[default]
    Specs,
    Format,
    Comparison,
}

impl HexCompareView {
    /// 由当前会话生成草稿。
    pub fn session_draft(&self) -> SessionDraft {
        let path = |c: &bc_engine_bin::hex::HexContent| {
            c.source()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        };
        SessionDraft {
            left_path: path(&self.map.left),
            right_path: path(&self.map.right),
            disable_editing: [self.map.left.read_only(), self.map.right.read_only()],
            description: String::new(),
            format_left: "Detected (Everything Else)".into(),
            format_right: "Detected (Everything Else)".into(),
            same_as_left: false,
            alignment: self.map.rules.alignment,
            lock_threshold_mb: self.map.rules.lock_threshold / (1024 * 1024),
        }
    }

    /// 应用草稿：禁编辑开关即时生效；对齐策略/锁阈值变化触发重对齐；路径变化换文件。
    pub fn apply_session_draft(&mut self, draft: SessionDraft) {
        self.map.left.set_read_only(draft.disable_editing[0]);
        self.map.right.set_read_only(draft.disable_editing[1]);
        let threshold = draft.lock_threshold_mb.max(1) * 1024 * 1024;
        let realign = self.map.rules.alignment != draft.alignment
            || self.map.rules.lock_threshold != threshold;
        self.map.rules.alignment = draft.alignment;
        self.map.rules.lock_threshold = threshold;
        // 路径换侧（H12 单侧换文件；空串保持现状）。
        for (side, text) in [(0, &draft.left_path), (1, &draft.right_path)] {
            let current = self.content_source(side).unwrap_or_default();
            if !text.trim().is_empty() && text.trim() != current {
                if let Err(e) = self.open_path(side, std::path::Path::new(text.trim())) {
                    self.message = e.to_string();
                }
            }
        }
        if realign {
            self.recompare();
        }
        self.settings_open = false;
    }

    fn content_source(&self, side: usize) -> Option<String> {
        (if side == 0 {
            &self.map.left
        } else {
            &self.map.right
        })
        .source()
        .map(|p| p.display().to_string())
    }

    /// Session Settings 对话框（三页）：与文本 / 表格 / 文件夹同一框架——下划线页签、页面卡片、
    /// OK / Cancel；编辑草稿，OK 才应用。
    pub(crate) fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        if self.settings_draft.is_none() {
            self.settings_draft = Some(self.session_draft());
        }
        const PAGES: [(Page, &str); 3] = [
            (Page::Specs, "Specs"),
            (Page::Format, "Format"),
            (Page::Comparison, "Comparison"),
        ];
        let (mut ok, mut cancel) = (false, false);
        let (closed, ()) = dialog::window(
            ctx,
            egui::Id::unique("hex-session-settings"),
            "Session Settings - Hex Compare",
            egui::vec2(583.0, 400.0),
            Placement::Window,
            |ui, client| {
                let o = client.min;
                let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
                    egui::Rect::from_min_max(o + egui::vec2(x0, y0), o + egui::vec2(x1, y1))
                };
                let page = at(8.0, 32.0, 575.0, 348.0);
                let labels = PAGES.map(|(_, label)| label);
                let current = PAGES
                    .iter()
                    .position(|(p, _)| *p == self.settings_page)
                    .unwrap_or(0);
                if let Some(i) = dialog::tabs(ui, page, &labels, current) {
                    self.settings_page = PAGES[i].0;
                }
                let draft = self.settings_draft.as_mut().unwrap();
                let page_id = self.settings_page;
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(page.shrink2(egui::vec2(14.0, 12.0))),
                    |ui| match page_id {
                        Page::Specs => {
                            let width = ui.available_width();
                            ui.label("Left file");
                            widgets::text_field_width(ui, &mut draft.left_path, width);
                            widgets::checkbox(ui, &mut draft.disable_editing[0], "Disable editing");
                            ui.add_space(4.0);
                            ui.label("Right file");
                            widgets::text_field_width(ui, &mut draft.right_path, width);
                            widgets::checkbox(ui, &mut draft.disable_editing[1], "Disable editing");
                            ui.add_space(4.0);
                            ui.label("Description");
                            widgets::text_field_width(ui, &mut draft.description, width);
                        }
                        Page::Format => {
                            ui.horizontal(|ui| {
                                ui.label("Left format:");
                                ui.label(&draft.format_left);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Right format:");
                                ui.label(&draft.format_right);
                            });
                            widgets::checkbox(
                                ui,
                                &mut draft.same_as_left,
                                "Same as left file format",
                            );
                        }
                        Page::Comparison => {
                            widgets::section(ui, "Alignment");
                            widgets::radio_value(
                                ui,
                                &mut draft.alignment,
                                ByteAlignStrategy::Complete,
                                "Complete",
                            );
                            widgets::radio_value(
                                ui,
                                &mut draft.alignment,
                                ByteAlignStrategy::Fast,
                                "Fast",
                            );
                            widgets::radio_value(
                                ui,
                                &mut draft.alignment,
                                ByteAlignStrategy::None,
                                "None",
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    "Files up to this size are loaded without file locks (MB):",
                                );
                                ui.add(
                                    egui::DragValue::new(&mut draft.lock_threshold_mb)
                                        .range(1..=4096),
                                );
                            });
                        }
                    },
                );
                dialog::combo(
                    ui,
                    at(10.0, 362.0, 197.0, 387.0),
                    egui::Id::unique("hex-session-scope"),
                    "Use for this view only",
                    &["Use for this view only".to_string()],
                    true,
                );
                ok = dialog::button(
                    ui,
                    at(373.0, 360.0, 467.0, 389.0),
                    "OK",
                    ButtonKind::Default,
                    true,
                );
                cancel = dialog::button(
                    ui,
                    at(478.0, 360.0, 572.0, 389.0),
                    "Cancel",
                    ButtonKind::Normal,
                    true,
                );
            },
        );
        if ok {
            let draft = self.settings_draft.take().unwrap();
            self.apply_session_draft(draft);
        } else if cancel || closed || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_draft = None;
            self.settings_open = false;
        }
    }
}
