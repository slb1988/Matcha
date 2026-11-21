//! 底部消息面板：比较视图里「找不到差异」之类的提示条（浅黄底 + 信息图标 + 一行文字），
//! 位于详情栏与状态栏之间。对齐参考 Options › Next Difference › Show message panel：
//! 出现时伴随系统提示音，下一次按键或鼠标按下即消失。

use eframe::egui::{self, Color32, Rect};

/// 面板高度（逻辑像素）；视图在面板可见时把主体区域上移这么多。
pub const HEIGHT: f32 = 48.0;

const FILL: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xE1);
const BORDER: Color32 = Color32::from_rgb(0xA0, 0xA0, 0xA0);

/// 一条待显示的面板消息；`beep` 在首次绘制时消费（只响一次）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagePanel {
    pub text: String,
    beep: bool,
}

impl MessagePanel {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            beep: true,
        }
    }

    /// 本帧有按键或鼠标按下（用户开始下一次操作）时应关闭面板。
    pub fn dismissed_by(ctx: &egui::Context) -> bool {
        ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::PointerButton { pressed: true, .. }
                        | egui::Event::Text(_)
                )
            })
        })
    }

    /// 在 `rect` 内绘制面板；首次绘制时播放提示音。
    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect) {
        if std::mem::take(&mut self.beep) && !cfg!(test) {
            bc_script::platform::beep();
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, FILL);
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, BORDER),
            egui::StrokeKind::Inside,
        );
        let center = egui::pos2(rect.left() + 26.0, rect.center().y);
        let radius = (rect.height() / 2.0 - 8.0).clamp(6.0, 14.0);
        painter.circle_filled(center, radius, super::theme::INFO);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "i",
            egui::FontId::proportional(radius * 1.4),
            Color32::WHITE,
        );
        painter.text(
            egui::pos2(center.x + radius + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &self.text,
            egui::FontId::proportional(super::theme::UI_SIZE + 2.0),
            super::theme::TEXT,
        );
    }
}

/// 差异导航找不到目标时的统一文案。
pub const NOT_FOUND: &str = "Difference not found.";

/// 视图持有的「找不到差异」面板状态：显示、关闭时机与占位高度只在这里实现一份。
#[derive(Debug, Default)]
pub struct NotFoundPanel(Option<MessagePanel>);

impl NotFoundPanel {
    /// 每帧在处理键盘与命令之前调用：有按键、文字输入或鼠标按下时关闭面板。
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        if self.0.is_some() && MessagePanel::dismissed_by(ctx) {
            self.0 = None;
        }
    }

    /// 导航找不到目标：偏好允许时显示面板（每次调用都会再响一次）。
    pub fn not_found(&mut self, prefs: bc_session::prefs::NextDiffPrefs) {
        self.0 = prefs.message_panel.then(|| MessagePanel::new(NOT_FOUND));
    }

    /// 面板占用的高度（未显示时为 0）。
    pub fn height(&self) -> f32 {
        if self.0.is_some() {
            HEIGHT
        } else {
            0.0
        }
    }

    pub fn is_shown(&self) -> bool {
        self.0.is_some()
    }

    /// 在 `rect` 内绘制（未显示时什么也不做）。
    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect) {
        if let Some(panel) = &mut self.0 {
            panel.paint(ui, rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bc_session::prefs::NextDiffPrefs;

    fn frame(ctx: &egui::Context, panel: &mut NotFoundPanel, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |_| panel.begin_frame(ctx));
        out.textures_delta.clear();
    }

    #[test]
    fn hidden_when_the_preference_is_off() {
        let mut panel = NotFoundPanel::default();
        panel.not_found(NextDiffPrefs {
            message_panel: false,
            wrap_around: false,
        });
        assert!(!panel.is_shown());
        assert_eq!(panel.height(), 0.0);
        panel.not_found(NextDiffPrefs::default());
        assert!(panel.is_shown());
        assert_eq!(panel.height(), HEIGHT);
    }

    #[test]
    fn closed_by_keys_text_and_presses_but_not_by_moves_or_wheel() {
        let ctx = egui::Context::default();
        let pos = egui::pos2(10.0, 10.0);
        let key = egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        let press = egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        for closing in [key, egui::Event::Text("x".into()), press] {
            let mut panel = NotFoundPanel::default();
            panel.not_found(NextDiffPrefs::default());
            frame(&ctx, &mut panel, vec![]);
            assert!(panel.is_shown());
            frame(&ctx, &mut panel, vec![closing]);
            assert!(!panel.is_shown());
        }
        let mut panel = NotFoundPanel::default();
        panel.not_found(NextDiffPrefs::default());
        frame(
            &ctx,
            &mut panel,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, -1.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                },
            ],
        );
        assert!(panel.is_shown());
    }
}
