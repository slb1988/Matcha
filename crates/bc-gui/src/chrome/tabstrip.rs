//! 会话标签条：经典浅色 Windows 选项卡外观——活动页白底与内容区相连，
//! 非活动页浅灰渐变；每页带会话图标、标题与关闭钮，末尾为 "+" 新建标签页。
//! 只负责绘制与命中，标签页集合与生命周期由应用持有。

use crate::chrome::{
    icons::{self, Icon},
    theme,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// 标签条总高（含底边线）。
pub const HEIGHT: f32 = 26.0;
const TAB_TOP: f32 = 3.0;
const MIN_TAB: f32 = 90.0;
const MAX_TAB: f32 = 220.0;
const CLOSE: f32 = 14.0;
const NEW_BUTTON: f32 = 22.0;
const ACTIVE_FILL: Color32 = Color32::WHITE;
const INACTIVE_TOP: Color32 = Color32::from_rgb(0xF4, 0xF4, 0xF4);
const INACTIVE_BOTTOM: Color32 = Color32::from_rgb(0xE2, 0xE2, 0xE2);
const HOVER_FILL: Color32 = Color32::from_rgb(0xE5, 0xF1, 0xFB);
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xE8, 0x11, 0x23);

/// 一个标签页的显示内容。
#[derive(Clone, Debug, PartialEq)]
pub struct TabSpec {
    pub title: String,
    pub icon: Option<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabEvent {
    Activate(usize),
    Close(usize),
    New,
}

/// 各标签页矩形：等分可用宽度，夹在 [MIN_TAB, MAX_TAB]；放不下的尾部被裁掉。
fn layout(rect: Rect, count: usize) -> Vec<Rect> {
    let avail = (rect.width() - NEW_BUTTON - 8.0).max(0.0);
    let width = (avail / count.max(1) as f32)
        .clamp(MIN_TAB, MAX_TAB)
        .floor();
    (0..count)
        .map(|i| {
            let x = rect.left() + 2.0 + i as f32 * width;
            Rect::from_min_max(
                Pos2::new(x, rect.top() + TAB_TOP),
                Pos2::new(x + width - 1.0, rect.bottom()),
            )
        })
        .collect()
}

fn close_rect(tab: Rect) -> Rect {
    Rect::from_center_size(
        Pos2::new(tab.right() - 4.0 - CLOSE / 2.0, tab.center().y),
        Vec2::splat(CLOSE),
    )
}

fn new_rect(rect: Rect, tabs: &[Rect]) -> Rect {
    let x = tabs.last().map_or(rect.left() + 2.0, |t| t.right() + 4.0);
    Rect::from_min_size(
        Pos2::new(x, rect.top() + TAB_TOP + 1.0),
        Vec2::new(NEW_BUTTON, rect.height() - TAB_TOP - 3.0),
    )
}

/// 绘制标签条并返回本帧的点击事件（关闭钮优先于激活）。
pub fn show(ui: &mut egui::Ui, rect: Rect, tabs: &[TabSpec], active: usize) -> Option<TabEvent> {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::PANEL_BG);
    let base = rect.bottom() - 0.5;
    let font = FontId::proportional(theme::UI_SIZE);
    let rects = layout(rect, tabs.len());
    let mut event = None;
    let mut active_rect = None;
    for (i, (tab, spec)) in rects.iter().zip(tabs).enumerate() {
        if tab.left() >= rect.right() {
            break;
        }
        let id = egui::Id::unique(("matcha-tab", i));
        let response = ui.interact(*tab, id, Sense::click());
        let close = close_rect(*tab);
        let close_response = ui.interact(close, id.with("close"), Sense::click());
        let is_active = i == active;
        // 活动页抬高 2 px 并压住底线；非活动页自上而下浅灰渐变。
        let body = if is_active {
            Rect::from_min_max(tab.min - Vec2::new(0.0, 2.0), tab.max)
        } else {
            *tab
        };
        if is_active {
            painter.rect_filled(body, 0.0, ACTIVE_FILL);
            active_rect = Some(body);
        } else if response.hovered() {
            painter.rect_filled(body, 0.0, HOVER_FILL);
        } else {
            let mut mesh = egui::Mesh::default();
            let top = body.top() as i32;
            let bottom = body.bottom() as i32;
            for y in top..bottom {
                let t = (y - top) as f32 / (bottom - top).max(1) as f32;
                mesh.add_colored_rect(
                    Rect::from_min_max(
                        Pos2::new(body.left(), y as f32),
                        Pos2::new(body.right(), y as f32 + 1.0),
                    ),
                    INACTIVE_TOP.lerp_to_gamma(INACTIVE_BOTTOM, t),
                );
            }
            painter.add(egui::Shape::mesh(mesh));
        }
        let edge = Stroke::new(1.0, theme::BORDER);
        let (l, r, t) = (body.left() + 0.5, body.right() - 0.5, body.top() + 0.5);
        painter.add(egui::Shape::line(
            vec![
                Pos2::new(l, body.bottom()),
                Pos2::new(l, t),
                Pos2::new(r, t),
                Pos2::new(r, body.bottom()),
            ],
            edge,
        ));
        let mut x = body.left() + 6.0;
        if let Some(icon) = spec.icon {
            icons::paint(
                &painter,
                Rect::from_center_size(Pos2::new(x + 8.0, body.center().y), Vec2::splat(16.0)),
                icon,
            );
            x += 21.0;
        }
        let text_clip = Rect::from_min_max(
            Pos2::new(x, body.top()),
            Pos2::new(close.left() - 2.0, body.bottom()),
        );
        let galley = painter.layout_no_wrap(spec.title.clone(), font.clone(), Color32::BLACK);
        painter.with_clip_rect(text_clip.intersect(rect)).galley(
            Pos2::new(x, body.center().y - galley.size().y / 2.0),
            galley,
            Color32::BLACK,
        );
        // 关闭钮：悬停时红底白叉，否则灰叉。
        let ink = if close_response.hovered() {
            painter.rect_filled(close, 2.0, CLOSE_HOVER);
            Color32::WHITE
        } else {
            Color32::from_gray(0x60)
        };
        let c = close.center();
        let cross = Stroke::new(1.3, ink);
        painter.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], cross);
        painter.line_segment([c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)], cross);
        if close_response.clicked() || response.middle_clicked() {
            event = Some(TabEvent::Close(i));
        } else if response.clicked() && !is_active {
            event = Some(TabEvent::Activate(i));
        }
        response.on_hover_text(&spec.title);
    }
    // 底边线在活动页下断开，使其与下方内容连成一体。
    let border = Stroke::new(1.0, theme::BORDER);
    match active_rect {
        Some(a) => {
            painter.line_segment(
                [Pos2::new(rect.left(), base), Pos2::new(a.left(), base)],
                border,
            );
            painter.line_segment(
                [Pos2::new(a.right(), base), Pos2::new(rect.right(), base)],
                border,
            );
        }
        None => {
            painter.line_segment(
                [Pos2::new(rect.left(), base), Pos2::new(rect.right(), base)],
                border,
            );
        }
    }
    let plus = new_rect(rect, &rects);
    let response = ui
        .interact(plus, egui::Id::unique("matcha-tab-new"), Sense::click())
        .on_hover_text("New Tab (Ctrl+T)");
    if response.hovered() {
        painter.rect_filled(plus, 2.0, theme::HOVER_BLUE);
        painter.rect_stroke(
            plus,
            2.0,
            Stroke::new(1.0, theme::CLASSIC_BLUE),
            StrokeKind::Inside,
        );
    }
    let c = plus.center();
    let ink = Color32::from_gray(0x4F);
    painter.rect_filled(Rect::from_center_size(c, Vec2::new(9.0, 1.6)), 0.0, ink);
    painter.rect_filled(Rect::from_center_size(c, Vec2::new(1.6, 9.0)), 0.0, ink);
    if response.clicked() {
        event = Some(TabEvent::New);
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_clamps_tab_widths_and_places_the_new_button_after_the_last_tab() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, HEIGHT));
        let tabs = layout(rect, 2);
        assert_eq!(tabs[0].width(), MAX_TAB - 1.0);
        assert_eq!(tabs[1].left(), tabs[0].right() + 1.0);
        let many = layout(rect, 40);
        assert_eq!(many[0].width(), MIN_TAB - 1.0);
        assert!(new_rect(rect, &tabs).left() > tabs[1].right());
        assert!(close_rect(tabs[0]).right() < tabs[0].right());
    }
}
