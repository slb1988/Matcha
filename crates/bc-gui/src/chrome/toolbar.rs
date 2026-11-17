//! 图标在上、文字在下的工具栏；分体按钮具有不重叠的主体与下拉命中区。
use crate::chrome::icons::{self, Icon};
use eframe::egui::{self, FontId, Rect, Sense, Stroke, StrokeKind};
pub type ActionId = String;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolItemKind {
    Normal,
    Toggle,
    Split,
    Dropdown,
}
/// `group` 同时定义分隔组和 Toggle 单选组；无组的 Toggle 独立切换。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolItem {
    pub label: String,
    pub action_id: ActionId,
    pub kind: ToolItemKind,
    pub enabled: bool,
    pub selected: bool,
    pub icon: Icon,
    pub group: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolbarResponse {
    pub clicked: Option<ActionId>,
    pub dropdown: Option<ActionId>,
    /// Rect of the item whose drop-down was requested (menu anchor).
    pub dropdown_rect: Option<Rect>,
}

/// Drop-down menu of a toolbar item in the shared popup style. Call every
/// frame with that frame's [`ToolbarResponse`]: a drop-down request opens
/// `items(action_id)` under the item (or closes it when it is already open);
/// returns the chosen menu action. `id` must be unique per toolbar.
pub fn dropdown_menu(
    ctx: &egui::Context,
    id: egui::Id,
    response: &ToolbarResponse,
    items: impl FnOnce(&str) -> Vec<super::menubar::MenuItem>,
) -> Option<ActionId> {
    let name_id = id.with("open-item");
    if let (Some(name), Some(rect)) = (&response.dropdown, response.dropdown_rect) {
        let open: Option<String> = ctx.data(|d| d.get_temp(name_id));
        if open.as_ref() == Some(name) && super::popup::is_menu_open(ctx, id) {
            super::popup::close_menu(ctx, id);
        } else {
            super::popup::open_menu(
                ctx,
                id,
                rect.left_bottom() + egui::vec2(0.0, 2.0),
                Some(rect),
            );
            ctx.data_mut(|d| d.insert_temp(name_id, name.clone()));
        }
    }
    if !super::popup::is_menu_open(ctx, id) {
        return None;
    }
    let name: String = ctx.data(|d| d.get_temp(name_id))?;
    super::popup::menu(ctx, id, 0.0, || items(&name))
}

/// Whether `id`'s drop-down menu is open (views block shortcuts meanwhile).
pub fn dropdown_open(ctx: &egui::Context, id: egui::Id) -> bool {
    super::popup::is_menu_open(ctx, id)
}

/// 由调用者在处理返回动作时调用。组件不会隐式改写视图的业务状态。
pub fn apply_toggle(items: &mut [ToolItem], action_id: &str) {
    let Some(index) = items
        .iter()
        .position(|i| i.action_id == action_id && i.enabled && i.kind == ToolItemKind::Toggle)
    else {
        return;
    };
    if let Some(group) = items[index].group.clone() {
        for item in items
            .iter_mut()
            .filter(|i| i.kind == ToolItemKind::Toggle && i.group.as_ref() == Some(&group))
        {
            item.selected = item.action_id == action_id;
        }
    } else {
        items[index].selected = !items[index].selected;
    }
}
/// 普通和 Toggle 返回 `clicked`；Dropdown 及 Split 箭头返回 `dropdown`。
pub fn show(ui: &mut egui::Ui, items: &[ToolItem]) -> ToolbarResponse {
    let mut output = ToolbarResponse::default();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (index, item) in items.iter().enumerate() {
            if index > 0 && item.group != items[index - 1].group {
                // 组间：细竖分隔线。
                let (gap, _) = ui.allocate_exact_size(
                    egui::vec2(9.0, super::theme::TOOLBAR_HEIGHT),
                    Sense::hover(),
                );
                ui.painter().vline(
                    gap.center().x,
                    gap.top() + 12.0..=gap.bottom() - 12.0,
                    Stroke::new(1.0, super::theme::LINE),
                );
            }
            let font = FontId::proportional(super::theme::UI_SIZE - 1.0);
            let text_color = if !item.enabled {
                super::theme::TEXT_FAINT
            } else if item.selected {
                super::theme::ACCENT_PRESSED
            } else {
                super::theme::TEXT_MUTED
            };
            let galley = ui
                .painter()
                .layout_no_wrap(item.label.clone(), font, text_color);
            let arrow_width = if matches!(item.kind, ToolItemKind::Split | ToolItemKind::Dropdown) {
                14.0
            } else {
                0.0
            };
            let width = (galley.size().x + 16.0).max(56.0) + arrow_width;
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(width, super::theme::TOOLBAR_HEIGHT),
                Sense::hover(),
            );
            let id = ui.scope_id().with(("tool", &item.action_id));
            let body_rect = Rect::from_min_max(
                rect.min,
                egui::pos2(
                    rect.right()
                        - if item.kind == ToolItemKind::Split {
                            arrow_width
                        } else {
                            0.0
                        },
                    rect.bottom(),
                ),
            );
            let drop_rect =
                Rect::from_min_max(egui::pos2(rect.right() - arrow_width, rect.top()), rect.max);
            let sense = if item.enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let body = ui.interact(body_rect, id, sense);
            let drop = (item.kind == ToolItemKind::Split)
                .then(|| ui.interact(drop_rect, id.with("drop"), sense));
            let hovered =
                item.enabled && (body.hovered() || drop.as_ref().is_some_and(|r| r.hovered()));
            let pressed = item.enabled
                && (body.is_pointer_button_down_on()
                    || drop.as_ref().is_some_and(|r| r.is_pointer_button_down_on()));
            if item.selected || hovered || pressed {
                // 圆角胶囊：选中=强调色浅底+描边，悬停/按下=中性灰。
                let fill = rect.shrink2(egui::vec2(0.0, 2.0));
                let radius = super::theme::RADIUS_LG - 2;
                let (bg, line) = match (item.selected, pressed, hovered) {
                    (_, true, _) => (super::theme::PRESSED, None),
                    (true, _, _) => (
                        super::theme::ACCENT_SOFT,
                        Some(super::theme::ACCENT_SOFT_LINE),
                    ),
                    _ => (super::theme::HOVER, None),
                };
                ui.painter().rect_filled(fill, radius, bg);
                if let Some(line) = line {
                    ui.painter().rect_stroke(
                        fill,
                        radius,
                        Stroke::new(1.0, line),
                        StrokeKind::Inside,
                    );
                }
            }
            if arrow_width > 0.0 {
                if item.kind == ToolItemKind::Split && hovered {
                    ui.painter().line_segment(
                        [
                            drop_rect.left_top() + egui::vec2(0.0, 8.0),
                            drop_rect.left_bottom() - egui::vec2(0.0, 8.0),
                        ],
                        Stroke::new(1.0, super::theme::LINE_STRONG),
                    );
                }
                let c = drop_rect.center();
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-3.0, -1.0),
                        c + egui::vec2(3.0, -1.0),
                        c + egui::vec2(0.0, 2.0),
                    ],
                    text_color,
                    Stroke::NONE,
                ));
            }
            let x = rect.center().x - arrow_width / 2.0;
            let icon =
                Rect::from_center_size(egui::pos2(x, rect.top() + 15.0), egui::vec2(16.0, 16.0));
            let mut painter = ui.painter().clone();
            if !item.enabled {
                painter.set_opacity(0.35);
            }
            icons::paint(&painter, icon, item.icon);
            ui.painter().galley(
                egui::pos2(x - galley.size().x / 2.0, rect.top() + 26.0),
                galley,
                text_color,
            );
            if item.enabled && body.clicked() {
                if item.kind == ToolItemKind::Dropdown {
                    output.dropdown = Some(item.action_id.clone());
                    output.dropdown_rect = Some(rect);
                } else {
                    output.clicked = Some(item.action_id.clone());
                }
            }
            if item.enabled && drop.is_some_and(|r| r.clicked()) {
                output.dropdown = Some(item.action_id.clone());
                output.dropdown_rect = Some(rect);
            }
        }
    });
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, kind: ToolItemKind) -> ToolItem {
        ToolItem {
            label: "X".into(),
            action_id: id.into(),
            kind,
            enabled: true,
            selected: id == "all",
            icon: Icon::All,
            group: Some("filter".into()),
        }
    }
    fn click(items: &[ToolItem], x: f32) -> ToolbarResponse {
        let ctx = egui::Context::default();
        let _ = crate::chrome::test_frame(&ctx, Default::default(), |ui| {
            show(ui, items);
        });
        let p = egui::pos2(x, 15.0);
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let mut out = ToolbarResponse::default();
        let _ = crate::chrome::test_frame(&ctx, input, |ui| {
            out = show(ui, items);
        });
        out
    }
    #[test]
    fn injected_split_clicks_are_disjoint() {
        let items = [item("split", ToolItemKind::Split)];
        assert_eq!(
            click(&items, 20.0),
            ToolbarResponse {
                clicked: Some("split".into()),
                ..Default::default()
            }
        );
        let drop = click(&items, 65.0);
        assert_eq!(drop.clicked, None);
        assert_eq!(drop.dropdown.as_deref(), Some("split"));
        assert!(drop
            .dropdown_rect
            .is_some_and(|r| r.contains(egui::pos2(65.0, 15.0))));
    }
    #[test]
    fn injected_toggle_click_excludes_its_group_only() {
        let mut items = vec![
            item("all", ToolItemKind::Toggle),
            item("diffs", ToolItemKind::Toggle),
            item("context", ToolItemKind::Toggle),
        ];
        items[2].group = None;
        items[2].selected = true;
        let response = click(&items, 65.0);
        apply_toggle(&mut items, response.clicked.as_deref().unwrap());
        assert!(!items[0].selected && items[1].selected && items[2].selected);
    }
    #[test]
    fn disabled_and_dropdown_semantics() {
        let mut items = [item("disabled", ToolItemKind::Normal)];
        items[0].enabled = false;
        assert_eq!(click(&items, 20.0), ToolbarResponse::default());
        assert_eq!(
            click(&[item("drop", ToolItemKind::Dropdown)], 20.0)
                .dropdown
                .as_deref(),
            Some("drop")
        );
    }
    #[test]
    fn dropdown_menu_opens_under_item_dispatches_and_toggles() {
        use crate::chrome::popup::{self, item};
        let ctx = egui::Context::default();
        let items = [item("drop", ToolItemKind::Dropdown)];
        let id = egui::Id::unique("test-toolbar-menu");
        let run = |events: Vec<egui::Event>| {
            let mut chosen = None;
            let _ = crate::chrome::test_frame(
                &ctx,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = show(ui, &items);
                    chosen = dropdown_menu(ui.ctx(), id, &response, |name| {
                        assert_eq!(name, "drop");
                        vec![
                            item::action("Home", "Home", true),
                            item::separator(),
                            item::action("Info", "Info", true),
                        ]
                    });
                },
            );
            chosen
        };
        let click = |p: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ]
        };
        run(vec![]);
        run(click(egui::pos2(20.0, 15.0)));
        assert!(popup::is_menu_open(&ctx, id));
        // Clicking the item again closes it (toggle).
        run(click(egui::pos2(20.0, 15.0)));
        assert!(!popup::is_menu_open(&ctx, id));
        run(click(egui::pos2(20.0, 15.0)));
        run(vec![]);
        // Menu sits under the 46 px toolbar item: 2 px gap + border + 4 px pad,
        // rows 26 px, separator 9 px -> "Info" row center.
        let info = egui::pos2(40.0, 46.0 + 2.0 + 1.0 + 4.0 + 26.0 + 9.0 + 13.0);
        assert_eq!(run(click(info)).as_deref(), Some("Info"));
        assert!(!popup::is_menu_open(&ctx, id));
    }
}
