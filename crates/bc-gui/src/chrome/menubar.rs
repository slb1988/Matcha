//! 数据驱动的经典菜单。视图拥有动作语义；组件只保存打开层级与键盘焦点。
//! Alt+助记符打开顶层菜单，方向键遍历，Enter 执行，Esc 逐层退出。
use eframe::egui::{self, Color32, FontId, Key, Rect, Sense, Stroke};

pub type ActionId = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItemKind {
    Action,
    Check { checked: bool },
    Radio { selected: bool },
    Submenu(Vec<MenuItem>),
    Separator,
}

/// `shortcut` 是右对齐的展示文本；命令的全局快捷键由视图处理。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    pub mnemonic: Option<char>,
    pub shortcut: Option<String>,
    pub kind: MenuItemKind,
    pub enabled: bool,
    pub action_id: ActionId,
}
impl MenuItem {
    pub fn action(label: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            mnemonic: None,
            shortcut: None,
            kind: MenuItemKind::Action,
            enabled: true,
            action_id: action_id.into(),
        }
    }
    fn selectable(&self) -> bool {
        self.enabled && !matches!(self.kind, MenuItemKind::Separator)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuSpec {
    pub title: String,
    pub mnemonic: Option<char>,
    pub items: Vec<MenuItem>,
}
#[derive(Clone, Debug, Default)]
struct MenuState {
    open: Option<usize>,
    // 每层当前项；多出一层表示其父子菜单已展开。
    path: Vec<usize>,
}
fn first(items: &[MenuItem]) -> usize {
    items.iter().position(MenuItem::selectable).unwrap_or(0)
}
fn items_at<'a>(menus: &'a [MenuSpec], state: &MenuState, depth: usize) -> &'a [MenuItem] {
    let mut items = &menus[state.open.unwrap()].items[..];
    for index in state.path.iter().take(depth) {
        if let Some(MenuItem {
            kind: MenuItemKind::Submenu(children),
            ..
        }) = items.get(*index)
        {
            items = children;
        }
    }
    items
}
fn open(state: &mut MenuState, menus: &[MenuSpec], index: usize) {
    state.open = Some(index);
    state.path = vec![first(&menus[index].items)];
}
fn activate(state: &mut MenuState, menus: &[MenuSpec]) -> Option<ActionId> {
    let depth = state.path.len().checked_sub(1)?;
    let item = items_at(menus, state, depth).get(state.path[depth])?;
    if !item.selectable() {
        return None;
    }
    if let MenuItemKind::Submenu(children) = &item.kind {
        state.path.push(first(children));
        None
    } else {
        let action = item.action_id.clone();
        state.open = None;
        Some(action)
    }
}
fn pressed(ctx: &egui::Context, mnemonic: char) -> bool {
    Key::from_name(&mnemonic.to_ascii_uppercase().to_string())
        .is_some_and(|key| ctx.input(|i| i.key_pressed(key)))
}
fn keyboard(ctx: &egui::Context, menus: &[MenuSpec], state: &mut MenuState) -> Option<ActionId> {
    if menus.is_empty() {
        state.open = None;
        return None;
    }
    if ctx.input(|i| i.modifiers.alt) {
        if let Some(index) = menus
            .iter()
            .position(|m| m.mnemonic.is_some_and(|c| pressed(ctx, c)))
        {
            open(state, menus, index);
            return None;
        }
    }
    let index = state.open?;
    if index >= menus.len() {
        state.open = None;
        return None;
    }
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        if state.path.len() > 1 {
            state.path.pop();
        } else {
            state.open = None;
        }
        return None;
    }
    let depth = state.path.len().saturating_sub(1);
    let items = items_at(menus, state, depth);
    if items.is_empty() {
        return None;
    }
    if let Some(selected) = items
        .iter()
        .position(|item| item.selectable() && item.mnemonic.is_some_and(|c| pressed(ctx, c)))
    {
        state.path[depth] = selected;
        return activate(state, menus);
    }
    let down = ctx.input(|i| i.key_pressed(Key::ArrowDown));
    let up = ctx.input(|i| i.key_pressed(Key::ArrowUp));
    if down || up {
        let current = state.path[depth].min(items.len() - 1);
        for step in 1..=items.len() {
            let next = if down {
                (current + step) % items.len()
            } else {
                (current + items.len() - step) % items.len()
            };
            if items[next].selectable() {
                state.path[depth] = next;
                break;
            }
        }
    }
    if ctx.input(|i| i.key_pressed(Key::Enter)) {
        return activate(state, menus);
    }
    if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
        let items = items_at(menus, state, depth);
        if matches!(
            items.get(state.path[depth]).map(|i| &i.kind),
            Some(MenuItemKind::Submenu(_))
        ) {
            return activate(state, menus);
        } else if depth == 0 {
            open(state, menus, (index + 1) % menus.len());
        }
    }
    if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
        if depth > 0 {
            state.path.pop();
        } else {
            open(state, menus, (index + menus.len() - 1) % menus.len());
        }
    }
    None
}
/// 仅在指定助记字符下画线，保留 UTF-8 字符边界。
fn label_job(
    text: &str,
    mnemonic: Option<char>,
    alt: bool,
    color: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut marked = false;
    for ch in text.chars().filter(|c| *c != '&') {
        let underline = alt && !marked && mnemonic.is_some_and(|m| m.eq_ignore_ascii_case(&ch));
        if underline {
            marked = true;
        }
        job.append(
            &ch.to_string(),
            0.0,
            egui::TextFormat {
                font_id: FontId::proportional(super::theme::UI_SIZE),
                color,
                underline: if underline {
                    Stroke::new(1.0, color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        );
    }
    job
}
/// 返回一个已启用叶子项的动作；可在 `ui.push_id` 中放置多个独立菜单栏。
pub fn show(ui: &mut egui::Ui, menus: &[MenuSpec]) -> Option<ActionId> {
    let ctx = ui.ctx().clone();
    let id = ui.scope_id().with("classic-menubar");
    let mut state = ctx.data(|d| d.get_temp::<MenuState>(id).unwrap_or_default());
    let mut action = keyboard(&ctx, menus, &mut state);
    let alt = ctx.input(|i| i.modifiers.alt);
    let moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let mut anchors = Vec::new();
    let bar = egui::Frame::new()
        .fill(Color32::WHITE)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (index, menu) in menus.iter().enumerate() {
                    let galley = ui.fonts_mut(|f| {
                        f.layout_job(label_job(&menu.title, menu.mnemonic, alt, Color32::BLACK))
                    });
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(galley.size().x + 14.0, super::theme::MENU_HEIGHT),
                        Sense::click(),
                    );
                    anchors.push(rect);
                    if response.clicked() {
                        if state.open == Some(index) {
                            state.open = None;
                        } else {
                            open(&mut state, menus, index);
                        }
                    } else if response.hovered()
                        && moved
                        && state.open.is_some()
                        && state.open != Some(index)
                    {
                        open(&mut state, menus, index);
                    }
                    if response.hovered() || state.open == Some(index) {
                        ui.painter()
                            .rect_filled(rect, 0.0, super::theme::HOVER_BLUE);
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, super::theme::CLASSIC_BLUE),
                            egui::StrokeKind::Inside,
                        );
                    }
                    ui.painter().galley(
                        rect.left_center() + egui::vec2(7.0, -galley.size().y / 2.0),
                        galley,
                        Color32::BLACK,
                    );
                }
            });
        })
        .response
        .rect;
    let mut popup_rects = Vec::new();
    if let Some(index) = state.open.filter(|i| *i < anchors.len()) {
        let mut position = anchors[index].left_bottom();
        let mut depth = 0;
        while depth < state.path.len() && state.open.is_some() {
            let items = items_at(menus, &state, depth).to_vec();
            let mut child_anchor = None;
            let popup = egui::Area::new(id.with(("popup", depth)))
                .order(egui::Order::Foreground)
                .fixed_pos(position)
                .movable(false)
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(Color32::WHITE)
                        .stroke(Stroke::new(1.0, super::theme::BORDER))
                        .corner_radius(0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let width = items
                                .iter()
                                .map(|item| {
                                    let label = ui
                                        .painter()
                                        .layout_no_wrap(
                                            item.label.replace('&', ""),
                                            FontId::proportional(12.0),
                                            Color32::BLACK,
                                        )
                                        .size()
                                        .x;
                                    let shortcut = ui
                                        .painter()
                                        .layout_no_wrap(
                                            item.shortcut.clone().unwrap_or_default(),
                                            FontId::proportional(12.0),
                                            Color32::BLACK,
                                        )
                                        .size()
                                        .x;
                                    label + shortcut + 76.0
                                })
                                .fold(220.0, f32::max);
                            for (row, item) in items.iter().enumerate() {
                                if matches!(item.kind, MenuItemKind::Separator) {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(width, 7.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().line_segment(
                                        [
                                            rect.left_center() + egui::vec2(24.0, 0.0),
                                            rect.right_center(),
                                        ],
                                        Stroke::new(1.0, super::theme::BORDER),
                                    );
                                    continue;
                                }
                                let (rect, response) =
                                    ui.allocate_exact_size(egui::vec2(width, 22.0), Sense::click());
                                if item.selectable()
                                    && response.hovered()
                                    && (moved || response.clicked())
                                {
                                    if state.path[depth] != row {
                                        state.path.truncate(depth + 1);
                                    }
                                    state.path[depth] = row;
                                    if let MenuItemKind::Submenu(children) = &item.kind {
                                        if state.path.len() == depth + 1 {
                                            state.path.push(first(children));
                                        }
                                    }
                                }
                                if state.path[depth] == row {
                                    if item.selectable() {
                                        ui.painter().rect_filled(
                                            rect,
                                            0.0,
                                            super::theme::HOVER_BLUE,
                                        );
                                    }
                                    child_anchor = Some(rect.right_top());
                                }
                                let color = if item.enabled {
                                    Color32::BLACK
                                } else {
                                    Color32::GRAY
                                };
                                let galley = ui.fonts_mut(|f| {
                                    f.layout_job(label_job(&item.label, item.mnemonic, alt, color))
                                });
                                ui.painter().galley(
                                    rect.left_center() + egui::vec2(26.0, -galley.size().y / 2.0),
                                    galley,
                                    color,
                                );
                                let marker = match item.kind {
                                    MenuItemKind::Check { checked: true } => "✓",
                                    MenuItemKind::Radio { selected: true } => "●",
                                    _ => "",
                                };
                                ui.painter().text(
                                    rect.left_center() + egui::vec2(12.0, 0.0),
                                    egui::Align2::CENTER_CENTER,
                                    marker,
                                    FontId::proportional(12.0),
                                    color,
                                );
                                let right = if matches!(item.kind, MenuItemKind::Submenu(_)) {
                                    "▸"
                                } else {
                                    item.shortcut.as_deref().unwrap_or_default()
                                };
                                ui.painter().text(
                                    rect.right_center() - egui::vec2(10.0, 0.0),
                                    egui::Align2::RIGHT_CENTER,
                                    right,
                                    FontId::proportional(12.0),
                                    color,
                                );
                                if response.clicked()
                                    && item.selectable()
                                    && !matches!(item.kind, MenuItemKind::Submenu(_))
                                {
                                    action = Some(item.action_id.clone());
                                    state.open = None;
                                }
                            }
                        });
                });
            popup_rects.push(popup.response.rect);
            if let Some(anchor) = child_anchor {
                position = anchor;
            } else {
                break;
            }
            depth += 1;
        }
    }
    if ctx.input(|i| {
        i.pointer.any_click()
            && i.pointer.interact_pos().is_some_and(|p| {
                !bar.contains(p) && !popup_rects.iter().any(|r: &Rect| r.contains(p))
            })
    }) {
        state.open = None;
    }
    ctx.data_mut(|d| d.insert_temp(id, state));
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    fn menus() -> Vec<MenuSpec> {
        let mut disabled = MenuItem::action("Disabled", "disabled");
        disabled.enabled = false;
        disabled.mnemonic = Some('D');
        let mut open = MenuItem::action("Open", "open");
        open.shortcut = Some("Ctrl+O".into());
        open.mnemonic = Some('O');
        let mut sub = MenuItem::action("Recent", "recent");
        sub.kind = MenuItemKind::Submenu(vec![MenuItem::action("One", "one")]);
        vec![MenuSpec {
            title: "File".into(),
            mnemonic: Some('F'),
            items: vec![open, disabled, sub],
        }]
    }
    fn key(key: Key, alt: bool) -> egui::RawInput {
        let modifiers = egui::Modifiers {
            alt,
            ..Default::default()
        };
        egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        }
    }
    fn frame(ctx: &egui::Context, input: egui::RawInput) -> (Option<String>, egui::FullOutput) {
        let mut action = None;
        let output = crate::chrome::test_frame(&ctx, input, |ui| {
            action = show(ui, &menus());
        });
        (action, output)
    }
    #[test]
    fn alt_opens_shortcut_is_painted_and_enter_activates() {
        let ctx = egui::Context::default();
        frame(&ctx, egui::RawInput::default());
        frame(&ctx, key(Key::F, true));
        let (_, output) = frame(&ctx, egui::RawInput::default());
        assert!(output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Ctrl+O")));
        assert_eq!(
            frame(&ctx, key(Key::Enter, false)).0.as_deref(),
            Some("open")
        );
    }
    #[test]
    fn keyboard_skips_disabled_and_opens_submenu_without_dispatching_parent() {
        let ctx = egui::Context::default();
        frame(&ctx, key(Key::F, true));
        assert_eq!(frame(&ctx, key(Key::D, false)).0, None);
        frame(&ctx, key(Key::ArrowDown, false));
        assert_eq!(frame(&ctx, key(Key::ArrowRight, false)).0, None);
        assert_eq!(
            frame(&ctx, key(Key::Enter, false)).0.as_deref(),
            Some("one")
        );
    }
    #[test]
    fn click_opens_and_outside_closes() {
        let ctx = egui::Context::default();
        frame(&ctx, egui::RawInput::default());
        let click = |p| egui::RawInput {
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
        frame(&ctx, click(egui::pos2(15.0, 12.0)));
        assert_eq!(
            frame(&ctx, key(Key::Enter, false)).0.as_deref(),
            Some("open")
        );
        frame(&ctx, key(Key::F, true));
        frame(&ctx, click(egui::pos2(700.0, 500.0)));
        assert_eq!(frame(&ctx, key(Key::Enter, false)).0, None);
    }
    #[test]
    fn underline_only_the_mnemonic() {
        let job = label_job("Session", Some('e'), true, Color32::BLACK);
        let underlined: Vec<_> = job
            .sections
            .iter()
            .filter(|s| s.format.underline.width > 0.0)
            .collect();
        assert_eq!(underlined.len(), 1);
        assert_eq!(
            &job.text[underlined[0].byte_range.start.0..underlined[0].byte_range.end.0],
            "e"
        );
    }
}
