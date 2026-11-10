//! Filter controls and presets use the script-facing engine's parser.
//! `ui` 是文件夹同步视图沿用的旧行式布局；`chrome` 是文件夹比较视图的参考布局：
//! `Filters:` 标签 + 可编辑组合框 + Filters/Peek 两个大图标按钮，高 45 px。
use crate::egui;
use bc_engine_dir::{
    filter::{DisplayFilter, FolderDisplay},
    filter_script::FilterPresets,
    view::ViewOptions,
};
#[derive(Debug, Default)]
pub struct FiltersBar {
    pub names: String,
    pub other: String,
    pub options: ViewOptions,
    pub presets: FilterPresets,
    pub preset_name: String,
    pub error: Option<String>,
    pub show_none_option: bool,
    pub content: String,
    pub exclude_containing: bool,
}
#[derive(Debug, Default)]
pub struct FilterChange {
    pub apply: bool,
    pub save_presets: bool,
}
impl FiltersBar {
    /// 规格文本映射：参考的 `*.*` 等同于不过滤；其余文本按引擎 glob 解析。
    pub fn effective_names(&self) -> String {
        if self.names.trim() == "*.*" {
            String::new()
        } else {
            self.names.clone()
        }
    }
    /// 参考布局：标签 + 组合框（下拉列出预设）+ Filters / Peek 大图标按钮。
    /// 只在 45 px 高的条带内绘制；点击 Filters 或回车应用，Peek 切换 suppress_filters。
    pub fn chrome(&mut self, ui: &mut egui::Ui, bar: egui::Rect) -> FilterChange {
        use crate::chrome::{icons, icons::Icon, theme};
        let mut change = FilterChange::default();
        let painter = ui.painter().clone();
        painter.rect_filled(bar, 0.0, theme::PANEL_BG);
        // 顶部分隔线与高光（参考 y70 #848484 / y71 #F5F5F5）。
        painter.hline(
            bar.left()..=bar.right(),
            bar.top(),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(132, 132, 132)),
        );
        painter.hline(
            bar.left()..=bar.right(),
            bar.top() + 1.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(245, 245, 245)),
        );
        // 标签。
        painter.text(
            egui::pos2(bar.left() + 3.0, bar.center().y),
            egui::Align2::LEFT_CENTER,
            "Filters:",
            egui::FontId::proportional(theme::UI_SIZE),
            egui::Color32::BLACK,
        );
        // 组合框：x43 起、293 宽、25 高，白底 #8D8D8D 边。
        let combo = egui::Rect::from_min_size(
            egui::pos2(bar.left() + 43.0, bar.top() + 11.0),
            egui::vec2(293.0, 25.0),
        );
        painter.rect_filled(combo, 0.0, egui::Color32::WHITE);
        painter.rect_stroke(
            combo,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(141, 141, 141)),
            egui::StrokeKind::Inside,
        );
        let mut popup_choice = None;
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(combo)
                .id_salt("filter_combo"),
            |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.visuals_mut().widgets.inactive.bg_stroke =
                    egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
                let edit_rect = egui::Rect::from_min_size(
                    combo.min + egui::vec2(1.0, 1.0),
                    egui::vec2(combo.width() - 19.0, combo.height() - 2.0),
                );
                let edit = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.names)
                                .frame(egui::Frame::NONE)
                                .margin(egui::vec2(2.0, 4.0)),
                        )
                    })
                    .inner;
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    change.apply = true;
                }
                // 内嵌下拉箭头：列出预设。
                let chevron = egui::Rect::from_min_size(
                    egui::pos2(combo.right() - 16.0, combo.top()),
                    egui::vec2(16.0, combo.height()),
                );
                let chev = ui.interact(
                    chevron,
                    ui.scope_id().with("filter-preset-chevron"),
                    egui::Sense::click(),
                );
                icons::dropdown_triangle(
                    ui.painter(),
                    chevron.center(),
                    egui::Color32::from_rgb(110, 110, 110),
                );
                egui::Popup::from_toggle_button_response(&chev)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        if self.presets.items.is_empty() {
                            ui.add_enabled(false, egui::Label::new("No saved presets"));
                        }
                        for preset in &self.presets.items {
                            if ui
                                .selectable_label(self.names == preset.masks, &preset.name)
                                .clicked()
                            {
                                popup_choice = Some(preset.masks.clone());
                                ui.close();
                            }
                        }
                    });
                chev.on_hover_text("Filter presets");
            },
        );
        if let Some(masks) = popup_choice {
            self.names = masks;
            change.apply = true;
        }
        // Filters / Peek 大图标按钮（图标在上、文字在下，同工具栏风格）。
        let buttons = [
            (Icon::Filters, "Filters", false, "apply"),
            (Icon::Peek, "Peek", self.options.suppress_filters, "peek"),
        ];
        let mut x = combo.right() + 13.0;
        for (icon, label, selected, id) in buttons {
            let font = egui::FontId::proportional(theme::UI_SIZE);
            let galley = painter.layout_no_wrap(label.to_string(), font, egui::Color32::BLACK);
            let width = (galley.size().x + 14.0).max(46.0);
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, bar.top() + 1.0),
                egui::vec2(width, theme::TOOLBAR_HEIGHT),
            );
            let response = ui.interact(
                rect,
                ui.scope_id().with(("filter-bar", id)),
                egui::Sense::click(),
            );
            if response.hovered() || selected {
                let fill = egui::Rect::from_min_max(
                    rect.min + egui::vec2(0.0, 2.0),
                    rect.max - egui::vec2(0.0, 3.0),
                );
                painter.rect_filled(fill, 0.0, theme::HOVER_BLUE);
                painter.rect_stroke(
                    fill,
                    0.0,
                    egui::Stroke::new(1.0, theme::CLASSIC_BLUE),
                    egui::StrokeKind::Inside,
                );
            }
            icons::paint(
                &painter,
                egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 12.0),
                    egui::vec2(16.0, 16.0),
                ),
                icon,
            );
            painter.galley(
                egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 23.0),
                galley,
                egui::Color32::BLACK,
            );
            if response.clicked() {
                match id {
                    "apply" => change.apply = true,
                    "peek" => {
                        self.options.suppress_filters = !self.options.suppress_filters;
                        change.apply = true;
                    }
                    _ => {}
                }
            }
            x += width + 4.0;
        }
        change
    }
    /// 文件夹同步视图沿用的旧行式布局（Wave 3 不换肤同步视图）。
    pub fn ui_legacy(&mut self, ui: &mut egui::Ui, allow_flatten: bool) -> FilterChange {
        let mut change = FilterChange::default();
        ui.horizontal(|ui| {
            ui.label("Filters");
            let edited = ui.add(
                egui::TextEdit::singleline(&mut self.names)
                    .desired_width(280.0)
                    .hint_text("*.rs;-*.bak;-Backup\\"),
            );
            if edited.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                change.apply = true;
            }
            if ui.button("Apply filters").clicked() {
                change.apply = true;
            }
            egui::ComboBox::from_id_salt("name_presets")
                .selected_text("Presets")
                .show_ui(ui, |ui| {
                    for preset in &self.presets.items {
                        if ui.button(&preset.name).clicked() {
                            self.names = preset.masks.clone();
                            change.apply = true;
                            ui.close();
                        }
                    }
                });
            ui.menu_button("Add preset", |ui| {
                ui.text_edit_singleline(&mut self.preset_name);
                if ui.button("Save named preset").clicked() {
                    match self.presets.upsert(&self.preset_name, &self.names) {
                        Ok(()) => {
                            self.error = None;
                            change.save_presets = true;
                            ui.close();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("display_filter")
                .selected_text(display_label(self.options.display))
                .show_ui(ui, |ui| {
                    for value in DisplayFilter::ALL {
                        if value == DisplayFilter::None && !self.show_none_option {
                            continue;
                        }
                        change.apply |= ui
                            .selectable_value(
                                &mut self.options.display,
                                value,
                                display_label(value),
                            )
                            .changed();
                    }
                });
            egui::ComboBox::from_id_salt("folder_filter")
                .selected_text(folder_label(self.options.folders))
                .show_ui(ui, |ui| {
                    for value in [
                        FolderDisplay::AlwaysShow,
                        FolderDisplay::FilesAndStructure,
                        FolderDisplay::FilesOnly,
                        FolderDisplay::Flatten,
                    ] {
                        if value == FolderDisplay::Flatten && !allow_flatten {
                            continue;
                        }
                        change.apply |= ui
                            .selectable_value(&mut self.options.folders, value, folder_label(value))
                            .changed();
                    }
                });
            change.apply |= ui
                .checkbox(&mut self.options.suppress_filters, "Suppress filters")
                .changed();
            change.apply |= ui
                .checkbox(&mut self.options.ignore_unimportant, "Ignore unimportant")
                .changed();
            ui.menu_button("Other filters", |ui| {
                ui.label("Script fields: cutoff:<7days size:>1MB attrib:-sh");
                ui.add(egui::TextEdit::singleline(&mut self.other).desired_width(400.0));
                ui.label("UTF-8 content text (empty disables)");
                ui.text_edit_singleline(&mut self.content);
                ui.checkbox(
                    &mut self.exclude_containing,
                    "Exclude containing (otherwise exclude not containing)",
                );
                if ui.button("Apply other filters").clicked() {
                    change.apply = true;
                    ui.close();
                }
            });
        });
        if !self.other.trim().is_empty() || !self.content.is_empty() {
            ui.colored_label(
                egui::Color32::LIGHT_BLUE,
                "Other filters active; filtered partners remain visible in teal.",
            );
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        }
        change
    }
}
pub fn display_label(value: DisplayFilter) -> &'static str {
    use DisplayFilter::*;
    match value {
        All => "Show all",
        Mismatches => "Show differences",
        Matches => "Show same",
        NoOrphans => "No orphans",
        MismatchesNoOrphans => "Differences, no orphans",
        Orphans => "Orphans",
        LeftNewer => "Left newer",
        RightNewer => "Right newer",
        LeftNewerOrphans => "Left newer + orphans",
        RightNewerOrphans => "Right newer + orphans",
        LeftOrphans => "Left orphans",
        RightOrphans => "Right orphans",
        None => "Show none",
    }
}
pub fn folder_label(value: FolderDisplay) -> &'static str {
    match value {
        FolderDisplay::AlwaysShow => "Always show folders",
        FolderDisplay::FilesAndStructure => "Files and folder structure",
        FolderDisplay::FilesOnly => "Only compare files",
        FolderDisplay::Flatten => "Ignore folder structure",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn star_dot_star_is_the_match_all_alias() {
        let mut bar = super::FiltersBar::default();
        assert_eq!(bar.effective_names(), "");
        bar.names = "*.rs".into();
        assert_eq!(bar.effective_names(), "*.rs");
        bar.names = " *.* ".into();
        assert_eq!(bar.effective_names(), "");
    }
}
