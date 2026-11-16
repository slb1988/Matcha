//! Popup menu shared by menu-bar dropdowns, combo lists and right-click menus.
//!
//! Modern card style: white surface, hairline border, 10 px radius and a soft
//! shadow; 26 px rows with 6 px rounded hover pills inset 5 px, 9 px
//! separators, a check slot on the left, muted right-aligned shortcuts and a
//! chevron for submenus.
use super::menubar::{ActionId, MenuItem, MenuItemKind};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

pub const ITEM_HEIGHT: f32 = 26.0;
pub const SEPARATOR_HEIGHT: f32 = 9.0;
const PAD_TOP: f32 = 4.0;
const PAD_BOTTOM: f32 = 4.0;
const INSET: f32 = 5.0;
const LABEL_X: f32 = 32.0;
const SHORTCUT_RIGHT: f32 = 22.0;
/// Space between an item's label and its shortcut.
const SHORTCUT_GAP: f32 = 40.0;
const MIN_WIDTH: f32 = 160.0;
pub const BACKGROUND: Color32 = super::theme::SURFACE;
/// Highlighted item index meaning "none" (fresh right-click menus).
pub const NONE: usize = usize::MAX;

/// One painted popup level.
pub struct Level {
    pub rect: Rect,
    /// Enabled leaf clicked this frame.
    pub action: Option<ActionId>,
    /// Top-right of the highlighted submenu item (where its child opens).
    pub child_anchor: Option<Pos2>,
}

pub(crate) fn selectable(item: &MenuItem) -> bool {
    item.enabled && !matches!(item.kind, MenuItemKind::Separator)
}

fn font() -> FontId {
    FontId::proportional(super::theme::UI_SIZE)
}

/// Popup width for `items`: the widest item (label, plus gap and shortcut
/// when it has one) between the 27 px label inset and the 24 px right margin
/// — per item, like the reference (a long shortcut-less label does not add
/// the shortcut column).
fn width(ctx: &egui::Context, items: &[MenuItem], min_width: f32) -> f32 {
    let mut widest = 0.0f32;
    ctx.fonts_mut(|f| {
        for item in items {
            if matches!(item.kind, MenuItemKind::Separator) {
                continue;
            }
            let mut text = |s: &str| {
                f.layout_no_wrap(s.replace('&', ""), font(), Color32::BLACK)
                    .size()
                    .x
            };
            let mut w = text(&item.label);
            if let Some(shortcut) = &item.shortcut {
                w += SHORTCUT_GAP + text(shortcut);
            }
            widest = widest.max(w);
        }
    });
    (LABEL_X + widest + SHORTCUT_RIGHT + 1.0)
        .max(min_width)
        .round()
}

/// Paint one level at `position` (outer top-left). `path[depth]` is the
/// highlighted row (or [`NONE`]); hovering moves it, and hovering a submenu
/// opens it by pushing its first row onto `path`.
#[allow(clippy::too_many_arguments)]
pub fn level(
    ctx: &egui::Context,
    id: egui::Id,
    position: Pos2,
    items: &[MenuItem],
    path: &mut Vec<usize>,
    depth: usize,
    alt: bool,
    moved: bool,
    min_width: f32,
) -> Level {
    let palette = super::theme::palette(ctx);
    let width = width(ctx, items, min_width);
    let height = 2.0
        + PAD_TOP
        + PAD_BOTTOM
        + items
            .iter()
            .map(|i| {
                if matches!(i.kind, MenuItemKind::Separator) {
                    SEPARATOR_HEIGHT
                } else {
                    ITEM_HEIGHT
                }
            })
            .sum::<f32>();
    let mut action = None;
    let mut child_anchor = None;
    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .movable(false)
        // Menus appear at once (no egui fade-in).
        .fade_in(false)
        .show(ctx, |ui| {
            let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
            let painter = ui.painter();
            let radius = super::theme::RADIUS_LG;
            painter.add(super::theme::shadow_popup().as_shape(outer, radius));
            painter.rect_filled(outer, radius, palette.surface);
            painter.rect_stroke(
                outer,
                radius,
                Stroke::new(1.0, palette.line),
                egui::StrokeKind::Inside,
            );
            let mut y = outer.top() + 1.0 + PAD_TOP;
            for (row, item) in items.iter().enumerate() {
                if matches!(item.kind, MenuItemKind::Separator) {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(outer.left() + INSET + 4.0, y + 4.0),
                            Pos2::new(outer.right() - INSET - 4.0, y + 5.0),
                        ),
                        0.0,
                        palette.line,
                    );
                    y += SEPARATOR_HEIGHT;
                    continue;
                }
                let rect = Rect::from_min_size(
                    Pos2::new(outer.left() + 1.0, y),
                    Vec2::new(width - 2.0, ITEM_HEIGHT),
                );
                y += ITEM_HEIGHT;
                let response = ui.interact(rect, id.with(row), Sense::click());
                if selectable(item) && response.hovered() && (moved || response.clicked()) {
                    if path[depth] != row {
                        path.truncate(depth + 1);
                    }
                    path[depth] = row;
                    if let MenuItemKind::Submenu(children) = &item.kind {
                        if path.len() == depth + 1 {
                            path.push(first(children));
                        }
                    }
                }
                let highlighted = path[depth] == row;
                if highlighted && selectable(item) {
                    painter.rect_filled(
                        rect.shrink2(Vec2::new(INSET - 1.0, 1.0)),
                        super::theme::RADIUS_MD,
                        palette.hover,
                    );
                }
                if highlighted && matches!(item.kind, MenuItemKind::Submenu(_)) {
                    child_anchor = Some(Pos2::new(outer.right() - 2.0, rect.top() - 1.0 - PAD_TOP));
                }
                let color = if item.enabled {
                    palette.text
                } else {
                    palette.text_faint
                };
                let galley = ui.fonts_mut(|f| {
                    f.layout_job(super::menubar::label_job(
                        &item.label,
                        item.mnemonic,
                        alt,
                        color,
                    ))
                });
                painter.galley(
                    Pos2::new(
                        outer.left() + LABEL_X,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    color,
                );
                let slot = Pos2::new(outer.left() + 17.0, rect.center().y);
                match item.kind {
                    MenuItemKind::Check { checked: true } => {
                        painter.add(egui::Shape::line(
                            vec![
                                slot + Vec2::new(-3.5, 0.0),
                                slot + Vec2::new(-1.0, 2.5),
                                slot + Vec2::new(3.5, -2.5),
                            ],
                            Stroke::new(1.5, color),
                        ));
                    }
                    MenuItemKind::Radio { selected: true } => {
                        painter.circle_filled(slot, 3.0, color);
                    }
                    _ => {}
                }
                if matches!(item.kind, MenuItemKind::Submenu(_)) {
                    // Chevron, 14 px from the right edge.
                    let tip = Pos2::new(outer.right() - 14.0, rect.center().y);
                    painter.add(egui::Shape::line(
                        vec![tip + Vec2::new(-3.5, -4.0), tip, tip + Vec2::new(-3.5, 4.0)],
                        Stroke::new(1.4, palette.text_muted),
                    ));
                } else if let Some(shortcut) = &item.shortcut {
                    painter.text(
                        Pos2::new(outer.right() - SHORTCUT_RIGHT, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        shortcut,
                        FontId::proportional(super::theme::UI_SIZE - 1.0),
                        palette.text_faint,
                    );
                }
                if response.clicked()
                    && selectable(item)
                    && !matches!(item.kind, MenuItemKind::Submenu(_))
                {
                    action = Some(item.action_id.clone());
                }
            }
        });
    Level {
        rect: area.response.rect,
        action,
        child_anchor,
    }
}

pub(crate) fn first(items: &[MenuItem]) -> usize {
    items.iter().position(selectable).unwrap_or(0)
}

/// Items of the level at `depth` under `path` (submenu chain from `root`).
pub(crate) fn items_at<'a>(root: &'a [MenuItem], path: &[usize], depth: usize) -> &'a [MenuItem] {
    let mut items = root;
    for index in path.iter().take(depth) {
        match items.get(*index) {
            Some(MenuItem {
                kind: MenuItemKind::Submenu(children),
                ..
            }) => items = children,
            _ => return &[],
        }
    }
    items
}

/// Paint a whole cascade from `position`; returns the clicked action and
/// every level's rect (for click-away detection).
pub fn cascade(
    ctx: &egui::Context,
    id: egui::Id,
    position: Pos2,
    root: &[MenuItem],
    path: &mut Vec<usize>,
    alt: bool,
) -> (Option<ActionId>, Vec<Rect>) {
    cascade_min_width(ctx, id, position, root, path, alt, MIN_WIDTH)
}

/// [`cascade`] whose first level is at least `min_width` wide (combo lists
/// match their box).
pub fn cascade_min_width(
    ctx: &egui::Context,
    id: egui::Id,
    position: Pos2,
    root: &[MenuItem],
    path: &mut Vec<usize>,
    alt: bool,
    min_width: f32,
) -> (Option<ActionId>, Vec<Rect>) {
    let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
    let mut rects = Vec::new();
    let mut action = None;
    let mut position = position;
    let mut depth = 0;
    while depth < path.len() {
        let items = items_at(root, path, depth);
        if items.is_empty() {
            break;
        }
        let level = level(
            ctx,
            id.with(("popup", depth)),
            position,
            items,
            path,
            depth,
            alt,
            moved,
            if depth == 0 { min_width } else { MIN_WIDTH },
        );
        rects.push(level.rect);
        action = action.or(level.action);
        match level.child_anchor {
            Some(anchor) if path.len() > depth + 1 => position = anchor,
            _ => break,
        }
        depth += 1;
    }
    (action, rects)
}

/// An open popup menu: where it sits, its highlighted path and the control
/// that owns it (clicks on the owner do not count as clicking away, so the
/// owner can toggle the menu).
#[derive(Clone, Debug)]
struct MenuState {
    position: Pos2,
    path: Vec<usize>,
    owner: Option<Rect>,
}

/// Open the menu `id` with its top-left at `position`.
pub fn open_menu(ctx: &egui::Context, id: egui::Id, position: Pos2, owner: Option<Rect>) {
    let state = MenuState {
        position,
        path: vec![NONE],
        owner,
    };
    ctx.data_mut(|d| d.insert_temp(id, state));
}

pub fn close_menu(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.remove::<MenuState>(id));
}

pub fn is_menu_open(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<MenuState>(id).is_some())
}

/// Toggle the menu `id` as a drop-down under `owner` (button, chevron,
/// toolbar item): opens 2 px below its bottom-left, or closes it when open.
pub fn toggle_menu_below(ctx: &egui::Context, id: egui::Id, owner: Rect) {
    if is_menu_open(ctx, id) {
        close_menu(ctx, id);
    } else {
        open_menu(
            ctx,
            id,
            owner.left_bottom() + Vec2::new(0.0, 2.0),
            Some(owner),
        );
    }
}

/// Paint the menu `id` if it is open (first level at least `min_width`
/// wide). `items` is only built while open. Returns the chosen action; a
/// choice, Esc or a click outside the menu and its owner closes it.
pub fn menu(
    ctx: &egui::Context,
    id: egui::Id,
    min_width: f32,
    items: impl FnOnce() -> Vec<MenuItem>,
) -> Option<ActionId> {
    show_menu(ctx, id, min_width, items, false)
}

fn show_menu(
    ctx: &egui::Context,
    id: egui::Id,
    min_width: f32,
    items: impl FnOnce() -> Vec<MenuItem>,
    opening: bool,
) -> Option<ActionId> {
    let mut open = ctx.data(|d| d.get_temp::<MenuState>(id))?;
    let root = items();
    if root.is_empty() {
        close_menu(ctx, id);
        return None;
    }
    let (action, mut rects) = cascade_min_width(
        ctx,
        id,
        open.position,
        &root,
        &mut open.path,
        false,
        min_width.max(MIN_WIDTH),
    );
    rects.extend(open.owner);
    let clicked_away = !opening
        && ctx.input(|i| {
            (i.pointer.any_click() || i.pointer.secondary_clicked())
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rects.iter().any(|r| r.contains(p)))
        });
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if action.is_some() || clicked_away || escape {
        close_menu(ctx, id);
    } else {
        ctx.data_mut(|d| d.insert_temp(id, open));
    }
    action
}

/// Drop-down menu owned by `response` (a button or chevron): a click toggles
/// it under the control. Returns the chosen action.
pub fn menu_below(
    response: &egui::Response,
    items: impl FnOnce() -> Vec<MenuItem>,
) -> Option<ActionId> {
    let id = response.id.with("menu-below");
    if response.clicked() {
        toggle_menu_below(&response.ctx, id, response.rect);
    }
    menu(&response.ctx, id, response.rect.width(), items)
}

/// Right-click menu on `response` in the shared popup style. `items` is only
/// built while the menu is open. Returns the chosen action; Esc, a chosen
/// item or a click elsewhere closes it.
pub fn context_menu(
    response: &egui::Response,
    items: impl FnOnce() -> Vec<MenuItem>,
) -> Option<ActionId> {
    let ctx = &response.ctx;
    let id = response.id.with("classic-context-menu");
    let opening = response.secondary_clicked();
    if opening {
        if let Some(position) = response.interact_pointer_pos() {
            open_menu(ctx, id, position, None);
        }
    }
    show_menu(ctx, id, MIN_WIDTH, items, opening)
}

/// Menu-item constructors for code that builds popup menus.
pub mod item {
    use super::super::menubar::{MenuItem, MenuItemKind};
    pub fn action(label: impl Into<String>, id: impl Into<String>, enabled: bool) -> MenuItem {
        MenuItem {
            enabled,
            ..MenuItem::action(label, id)
        }
    }
    pub fn check(label: impl Into<String>, id: impl Into<String>, checked: bool) -> MenuItem {
        MenuItem {
            kind: MenuItemKind::Check { checked },
            ..MenuItem::action(label, id)
        }
    }
    pub fn radio(label: impl Into<String>, id: impl Into<String>, selected: bool) -> MenuItem {
        MenuItem {
            kind: MenuItemKind::Radio { selected },
            ..MenuItem::action(label, id)
        }
    }
    pub fn submenu(label: impl Into<String>, children: Vec<MenuItem>) -> MenuItem {
        let label = label.into();
        MenuItem {
            kind: MenuItemKind::Submenu(children),
            ..MenuItem::action(label.clone(), label)
        }
    }
    pub fn separator() -> MenuItem {
        MenuItem {
            kind: MenuItemKind::Separator,
            enabled: false,
            ..MenuItem::action("", "")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn items() -> Vec<MenuItem> {
        let mut disabled = MenuItem::action("Disabled", "disabled");
        disabled.enabled = false;
        let separator = MenuItem {
            kind: MenuItemKind::Separator,
            ..MenuItem::action("", "")
        };
        vec![
            MenuItem::action("Select row", "select"),
            separator,
            disabled,
            MenuItem::action("Open file pair", "open"),
        ]
    }
    fn pointer(pos: Pos2, button: egui::PointerButton) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }
    fn frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> (Option<ActionId>, egui::FullOutput) {
        let mut action = None;
        let output = crate::chrome::test_frame(
            ctx,
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                let response = ui.interact(
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0)),
                    egui::Id::unique("target"),
                    Sense::click(),
                );
                action = context_menu(&response, items);
            },
        );
        (action, output)
    }
    #[test]
    fn right_click_opens_geometry_and_left_click_dispatches() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![]);
        frame(
            &ctx,
            pointer(Pos2::new(50.0, 60.0), egui::PointerButton::Secondary),
        );
        let (_, out) = frame(&ctx, vec![]);
        // Body rect: border + 4 px pad + 26 + 9 + 26 + 26 + 4 px pad.
        let body = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == BACKGROUND => Some(r.rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(body.min, Pos2::new(50.0, 60.0));
        assert_eq!(body.height(), 2.0 + 4.0 + 26.0 + 9.0 + 26.0 + 26.0 + 4.0);
        // Disabled row: click does nothing and keeps the menu open.
        let disabled = Pos2::new(80.0, 60.0 + 5.0 + 26.0 + 9.0 + 13.0);
        assert_eq!(
            frame(&ctx, pointer(disabled, egui::PointerButton::Primary)).0,
            None
        );
        let open_row = Pos2::new(80.0, 60.0 + 5.0 + 26.0 + 9.0 + 26.0 + 13.0);
        assert_eq!(
            frame(&ctx, pointer(open_row, egui::PointerButton::Primary))
                .0
                .as_deref(),
            Some("open")
        );
        // Closed after dispatch: clicking the same spot does nothing.
        assert_eq!(
            frame(&ctx, pointer(open_row, egui::PointerButton::Primary)).0,
            None
        );
    }
    #[test]
    fn escape_and_click_away_close() {
        let ctx = egui::Context::default();
        frame(
            &ctx,
            pointer(Pos2::new(50.0, 60.0), egui::PointerButton::Secondary),
        );
        frame(
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let row = Pos2::new(80.0, 60.0 + 2.0 + 10.0);
        assert_eq!(
            frame(&ctx, pointer(row, egui::PointerButton::Primary)).0,
            None
        );
        frame(
            &ctx,
            pointer(Pos2::new(50.0, 60.0), egui::PointerButton::Secondary),
        );
        frame(
            &ctx,
            pointer(Pos2::new(390.0, 390.0), egui::PointerButton::Primary),
        );
        assert_eq!(
            frame(&ctx, pointer(row, egui::PointerButton::Primary)).0,
            None
        );
    }
}
