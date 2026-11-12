//! 会话标签条：现代浏览器式标签——活动页为白色圆角卡片并与下方内容相连，
//! 非活动页透明、悬停时浅灰胶囊；每页带会话图标、标题与关闭钮，末尾为 "+"。
//! 只负责绘制与命中，标签页集合与生命周期由应用持有。

use crate::chrome::{
    icons::{self, Icon},
    theme,
};
use eframe::egui::{self, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// 标签条总高（含底边线）。
pub const HEIGHT: f32 = 36.0;
const TAB_TOP: f32 = 6.0;
const MIN_TAB: f32 = 110.0;
const MAX_TAB: f32 = 240.0;
const CLOSE: f32 = 18.0;
const NEW_BUTTON: f32 = 26.0;

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
            let x = rect.left() + 6.0 + i as f32 * width;
            Rect::from_min_max(
                Pos2::new(x, rect.top() + TAB_TOP),
                Pos2::new(x + width - 1.0, rect.bottom()),
            )
        })
        .collect()
}

fn close_rect(tab: Rect) -> Rect {
    Rect::from_center_size(
        Pos2::new(tab.right() - 7.0 - CLOSE / 2.0, tab.center().y),
        Vec2::splat(CLOSE),
    )
}

fn new_rect(rect: Rect, tabs: &[Rect]) -> Rect {
    let x = tabs.last().map_or(rect.left() + 6.0, |t| t.right() + 4.0);
    Rect::from_center_size(
        Pos2::new(
            x + NEW_BUTTON / 2.0,
            rect.top() + TAB_TOP + (rect.height() - TAB_TOP) / 2.0,
        ),
        Vec2::splat(NEW_BUTTON),
    )
}

/// 绘制标签条并返回本帧的点击事件（关闭钮优先于激活）。
pub fn show(ui: &mut egui::Ui, rect: Rect, tabs: &[TabSpec], active: usize) -> Option<TabEvent> {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::SURFACE_CHROME);
    let base = rect.bottom() - 0.5;
    let font = FontId::proportional(theme::UI_SIZE);
    let rects = layout(rect, tabs.len());
    let mut event = None;
    let mut active_rect = None;
    let line = Stroke::new(1.0, theme::LINE);
    for (i, (tab, spec)) in rects.iter().zip(tabs).enumerate() {
        if tab.left() >= rect.right() {
            break;
        }
        let id = egui::Id::unique(("matcha-tab", i));
        let response = ui.interact(*tab, id, Sense::click());
        let close = close_rect(*tab);
        let close_response = ui.interact(close, id.with("close"), Sense::click());
        let is_active = i == active;
        let body = *tab;
        if is_active {
            let radius = CornerRadius {
                nw: theme::RADIUS_LG,
                ne: theme::RADIUS_LG,
                sw: 0,
                se: 0,
            };
            painter.rect_filled(body, radius, theme::SURFACE);
            painter.rect_stroke(body, radius, line, StrokeKind::Inside);
            active_rect = Some(body);
        } else if response.hovered() {
            painter.rect_filled(
                body.shrink2(Vec2::new(2.0, 3.0)),
                theme::RADIUS_MD,
                theme::HOVER,
            );
        } else if i + 1 != active && i + 1 < tabs.len() {
            // 相邻非活动页之间的细分隔线。
            painter.vline(
                body.right() + 0.5,
                body.top() + 8.0..=body.bottom() - 8.0,
                line,
            );
        }
        let mut x = body.left() + 10.0;
        if let Some(icon) = spec.icon {
            icons::paint(
                &painter,
                Rect::from_center_size(Pos2::new(x + 8.0, body.center().y), Vec2::splat(16.0)),
                icon,
            );
            x += 24.0;
        }
        let text_clip = Rect::from_min_max(
            Pos2::new(x, body.top()),
            Pos2::new(close.left() - 4.0, body.bottom()),
        );
        let ink = if is_active {
            theme::TEXT
        } else {
            theme::TEXT_MUTED
        };
        let galley = painter.layout_no_wrap(spec.title.clone(), font.clone(), ink);
        painter.with_clip_rect(text_clip.intersect(rect)).galley(
            Pos2::new(x, body.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
        // 关闭钮：活动页或悬停时显示，悬停加圆形灰底。
        if is_active || response.hovered() || close_response.hovered() {
            if close_response.hovered() {
                painter.rect_filled(close, theme::RADIUS_SM, theme::PRESSED);
            }
            let c = close.center();
            let cross = Stroke::new(1.3, theme::TEXT_MUTED);
            painter.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], cross);
            painter.line_segment([c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)], cross);
        }
        if close_response.clicked() || response.middle_clicked() {
            event = Some(TabEvent::Close(i));
        } else if response.clicked() && !is_active {
            event = Some(TabEvent::Activate(i));
        }
        response.on_hover_text(&spec.title);
    }
    // 底边线在活动页下断开，使其与下方内容连成一体。
    match active_rect {
        Some(a) => {
            painter.line_segment(
                [Pos2::new(rect.left(), base), Pos2::new(a.left(), base)],
                line,
            );
            painter.line_segment(
                [Pos2::new(a.right(), base), Pos2::new(rect.right(), base)],
                line,
            );
        }
        None => {
            painter.line_segment(
                [Pos2::new(rect.left(), base), Pos2::new(rect.right(), base)],
                line,
            );
        }
    }
    let plus = new_rect(rect, &rects);
    let response = ui
        .interact(plus, egui::Id::unique("matcha-tab-new"), Sense::click())
        .on_hover_text("New Tab (Ctrl+T)");
    if response.hovered() {
        painter.rect_filled(plus, theme::RADIUS_MD, theme::HOVER);
    }
    let c = plus.center();
    let ink = theme::TEXT_MUTED;
    painter.rect_filled(Rect::from_center_size(c, Vec2::new(10.0, 1.5)), 0.0, ink);
    painter.rect_filled(Rect::from_center_size(c, Vec2::new(1.5, 10.0)), 0.0, ink);
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
