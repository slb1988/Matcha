//! Session › Save Session As…：给当前标签页的会话起名，写入会话库
//! （`docs/features/saved-sessions.md`）。
use crate::chrome::{controls::Role, theme, widgets, window::Window};
use crate::egui;

#[derive(Default)]
pub struct SaveSessionDialog {
    pub open: bool,
    pub name: String,
    /// 写盘失败的原因（对话框保持打开）。
    pub error: Option<String>,
    /// 打开时已存在的会话名（同名保存会替换，给出提示）。
    existing: Vec<String>,
    focus: bool,
}

impl SaveSessionDialog {
    /// 以 `name` 为初始名称打开。
    pub fn show(&mut self, name: impl Into<String>, existing: Vec<String>) {
        self.open = true;
        self.name = name.into();
        self.error = None;
        self.existing = existing;
        self.focus = true;
    }

    /// 绘制；用户确认时返回去掉首尾空白的名称（非空）。
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<String> {
        let mut open = self.open;
        let mut chosen = None;
        Window::new("Save Session As")
            .open(&mut open)
            .min_width(420.0)
            .show(ctx, |ui| {
                ui.label("Session name");
                let response = ui.add(
                    widgets::field(&mut self.name)
                        .id(egui::Id::unique("save-session-name"))
                        .desired_width(396.0),
                );
                if std::mem::take(&mut self.focus) {
                    response.request_focus();
                }
                let name = self.name.trim().to_string();
                if self.existing.contains(&name) {
                    widgets::hint(ui, "A saved session with this name will be replaced.");
                }
                if let Some(error) = &self.error {
                    ui.colored_label(theme::palette(ui.ctx()).danger, error);
                }
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                match widgets::dialog_buttons(
                    ui,
                    &[("Save", Role::Primary), ("Cancel", Role::Secondary)],
                ) {
                    Some(1) => self.open = false,
                    Some(0) if !name.is_empty() => chosen = Some(name),
                    _ if enter && !name.is_empty() => chosen = Some(name),
                    _ => {}
                }
            });
        self.open &= open;
        chosen
    }
}
