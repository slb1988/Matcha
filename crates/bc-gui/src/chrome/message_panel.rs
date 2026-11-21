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
