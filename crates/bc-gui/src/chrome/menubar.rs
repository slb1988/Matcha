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
/// Mouse-opened menus start with no highlighted item (reference behavior);
/// keyboard-opened ones highlight the first selectable item.
fn open_by_mouse(state: &mut MenuState, index: usize) {
    state.open = Some(index);
    state.path = vec![super::popup::NONE];
}
fn request_id() -> egui::Id {
    egui::Id::unique("classic-menubar-open-request")
}
/// Open the menu titled `title` on the next `show` as if clicked
/// (deterministic snapshots of an open dropdown).
pub fn request_open(ctx: &egui::Context, title: &str) {
    ctx.data_mut(|d| d.insert_temp(request_id(), title.to_string()));
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
        let current = match state.path[depth] {
            super::popup::NONE if down => items.len() - 1,
            super::popup::NONE => 0,
            current => current.min(items.len() - 1),
        };
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
const OPEN_TITLE: Color32 = Color32::from_rgb(0xF9, 0xF9, 0xF9);
/// 仅在指定助记字符下画线，保留 UTF-8 字符边界。
pub(crate) fn label_job(
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
    if let Some(title) = ctx.data_mut(|d| d.remove_temp::<String>(request_id())) {
        if let Some(index) = menus.iter().position(|m| m.title == title) {
            open_by_mouse(&mut state, index);
        }
    }
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
                            open_by_mouse(&mut state, index);
                        }
                    } else if response.hovered()
                        && moved
                        && state.open.is_some()
                        && state.open != Some(index)
                    {
                        open_by_mouse(&mut state, index);
                    }
                    if state.open == Some(index) {
                        // Reference open title: #F9F9F9, rows 2..24, no border.
                        ui.painter().rect_filled(
                            Rect::from_min_max(
                                rect.min + egui::vec2(0.0, 2.0),
                                egui::pos2(rect.right(), rect.top() + 25.0),
                            ),
                            0.0,
                            OPEN_TITLE,
                        );
                    } else if response.hovered() {
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
        // Reference: the dropdown's border starts 25 px below the bar top,
        // left-aligned with its title.
        let position = egui::pos2(anchors[index].left(), bar.top() + 25.0);
        let (clicked, rects) = super::popup::cascade(
            &ctx,
            id,
            position,
            &menus[index].items,
            &mut state.path,
            alt,
        );
        popup_rects = rects;
        if clicked.is_some() {
            action = clicked;
            state.open = None;
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
        let output = crate::chrome::test_frame(ctx, input, |ui| {
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
        // A mouse-opened menu highlights nothing: Enter is inert until ↓.
        assert_eq!(frame(&ctx, key(Key::Enter, false)).0, None);
        frame(&ctx, key(Key::ArrowDown, false));
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
