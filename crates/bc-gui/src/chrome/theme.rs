//! Matcha design system: semantic color, radius, spacing and type tokens plus
//! the global egui style built from them.
//!
//! The chrome left pixel parity with the classic Win32 reference in favour of
//! a modern, calm look (see `docs/decisions/401-modern-design-system.md`):
//! cool neutral surfaces, hairline borders, rounded controls, soft shadows and
//! one brand accent ("matcha" green). Diff semantics (red/blue difference
//! colors) are unchanged — they are content, not chrome.
//!
//! Always reach for a token here instead of a literal color so views stay in
//! one visual system.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

// ---------------------------------------------------------------- palette --

/// Window chrome surface (menu bar, toolbars, side panels, status bars).
pub const SURFACE_CHROME: Color32 = Color32::from_rgb(0xF4, 0xF5, 0xF7);
/// Raised content surface (cards, dialogs, editors, popups).
pub const SURFACE: Color32 = Color32::WHITE;
/// Slightly sunken surface (inputs on chrome, section wells, table stripes).
pub const SURFACE_SUBTLE: Color32 = Color32::from_rgb(0xF9, 0xFA, 0xFB);
/// Hairline borders and separators.
pub const LINE: Color32 = Color32::from_rgb(0xE1, 0xE4, 0xE8);
/// Control outlines (buttons, fields) — one step stronger than [`LINE`].
pub const LINE_STRONG: Color32 = Color32::from_rgb(0xCB, 0xD0, 0xD6);

pub const TEXT: Color32 = Color32::from_rgb(0x1C, 0x21, 0x26);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x5B, 0x63, 0x6D);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x98, 0x9F, 0xA8);

/// Neutral hover / pressed fills for flat (ghost) controls.
pub const HOVER: Color32 = Color32::from_rgb(0xEB, 0xED, 0xF0);
pub const PRESSED: Color32 = Color32::from_rgb(0xDF, 0xE3, 0xE7);

/// Brand accent — a muted matcha green.
pub const ACCENT: Color32 = Color32::from_rgb(0x3B, 0x7A, 0x50);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x33, 0x6B, 0x46);
pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(0x2B, 0x5B, 0x3B);
/// Accent tint for selected / toggled-on states and its outline.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xE3, 0xF0, 0xE6);
pub const ACCENT_SOFT_LINE: Color32 = Color32::from_rgb(0xB5, 0xD4, 0xBE);
/// Text selection highlight.
pub const SELECTION: Color32 = Color32::from_rgb(0xC9, 0xE2, 0xD0);

pub const DANGER: Color32 = Color32::from_rgb(0xC4, 0x3A, 0x34);
pub const DANGER_HOVER: Color32 = Color32::from_rgb(0xAD, 0x2F, 0x2A);
pub const DANGER_SOFT: Color32 = Color32::from_rgb(0xFC, 0xEC, 0xEB);
/// Pressed fill of destructive (red ghost) buttons.
pub const DANGER_PRESSED: Color32 = Color32::from_rgb(0xF8, 0xD9, 0xD7);
pub const WARNING: Color32 = Color32::from_rgb(0xB4, 0x6F, 0x0E);
pub const WARNING_SOFT: Color32 = Color32::from_rgb(0xFD, 0xF3, 0xE1);
pub const INFO: Color32 = Color32::from_rgb(0x2D, 0x6C, 0xD6);
pub const INFO_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF0, 0xFD);

/// Content well behind documents (empty area beyond a table's columns and
/// rows, below a file's last line): one step darker than the chrome so white
/// content reads as a sheet lying on it.
pub const CANVAS: Color32 = Color32::from_rgb(0xE9, 0xEC, 0xEF);
/// 合并视图的四个色位：左侧改动、右侧改动、冲突、手工编辑（各一深一浅）。
pub const MERGE_LEFT: Color32 = Color32::from_rgb(0x12, 0x8A, 0x8A);
pub const MERGE_LEFT_SOFT: Color32 = Color32::from_rgb(0xDC, 0xF3, 0xF1);
pub const MERGE_RIGHT: Color32 = Color32::from_rgb(0xA8, 0x3A, 0xB0);
pub const MERGE_RIGHT_SOFT: Color32 = Color32::from_rgb(0xF6, 0xE4, 0xF8);
pub const MERGE_CONFLICT: Color32 = DANGER;
pub const MERGE_CONFLICT_SOFT: Color32 = DANGER_SOFT;
pub const MERGE_MANUAL: Color32 = Color32::from_rgb(0xA8, 0x80, 0x00);
pub const MERGE_MANUAL_SOFT: Color32 = Color32::from_rgb(0xFF, 0xF5, 0xC7);
/// Faint dot texture on [`CANVAS`].
pub const CANVAS_DOT: Color32 = Color32::from_rgb(0xC9, 0xCF, 0xD6);
/// Row/column header fill of grids (row numbers, column letters).
pub const HEADER_FILL: Color32 = Color32::from_rgb(0xF3, 0xF5, 0xF7);
/// Dot spacing of the canvas texture.
pub const CANVAS_DOT_STEP: f32 = 14.0;

/// Scrollbar handle ramp (rest / hover / drag).
pub const SCROLL_THUMB: Color32 = Color32::from_rgb(0xCB, 0xD0, 0xD6);
pub const SCROLL_THUMB_HOVER: Color32 = Color32::from_rgb(0xAE, 0xB5, 0xBD);
pub const SCROLL_THUMB_ACTIVE: Color32 = Color32::from_rgb(0x96, 0x9E, 0xA8);

/// Modal backdrop scrim.
pub const SCRIM: Color32 = Color32::from_black_alpha(64);

// ------------------------------------------------------------- palette --

/// 可替换的外框配色（换肤预留）。上面的 `pub const` token 是默认浅色方案
/// [`Palette::LIGHT`] 的取值；`chrome::widgets` / `chrome::window` 等组件一律经
/// [`palette`] 从 egui 上下文读取当前配色，而不是直接引用常量——将来换肤只需
/// [`Theme::with_palette`] 换一套 `Palette` 再 `apply`，存量直接引用常量的组件
/// 按需逐步迁到 `palette(ctx)`。差异内容色（红/蓝）不属于外框配色，不在此列。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub surface_chrome: Color32,
    pub surface: Color32,
    pub surface_subtle: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_soft: Color32,
    pub accent_soft_line: Color32,
    pub selection: Color32,
    pub danger: Color32,
    pub danger_hover: Color32,
    pub danger_soft: Color32,
    pub danger_pressed: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub info: Color32,
    pub info_soft: Color32,
    pub scrim: Color32,
}

impl Palette {
    /// 默认浅色方案（即本模块的 `pub const` token）。
    pub const LIGHT: Palette = Palette {
        surface_chrome: SURFACE_CHROME,
        surface: SURFACE,
        surface_subtle: SURFACE_SUBTLE,
        line: LINE,
        line_strong: LINE_STRONG,
        text: TEXT,
        text_muted: TEXT_MUTED,
        text_faint: TEXT_FAINT,
        hover: HOVER,
        pressed: PRESSED,
        accent: ACCENT,
        accent_hover: ACCENT_HOVER,
        accent_pressed: ACCENT_PRESSED,
        accent_soft: ACCENT_SOFT,
        accent_soft_line: ACCENT_SOFT_LINE,
        selection: SELECTION,
        danger: DANGER,
        danger_hover: DANGER_HOVER,
        danger_soft: DANGER_SOFT,
        danger_pressed: DANGER_PRESSED,
        warning: WARNING,
        warning_soft: WARNING_SOFT,
        info: INFO,
        info_soft: INFO_SOFT,
        scrim: SCRIM,
    };
}

impl Default for Palette {
    fn default() -> Self {
        Self::LIGHT
    }
}

fn palette_id() -> egui::Id {
    egui::Id::unique("matcha-theme-palette")
}

/// 当前外框配色：[`Theme::apply`] 安装的那套；未安装（无头测试）时为默认浅色。
pub fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id()))
        .unwrap_or(Palette::LIGHT)
}

// ---------------------------------------------- legacy names (remapped) --
// Older call sites still use these names; they now resolve to the modern
// tokens so the whole app moves together.

pub const PANEL_BG: Color32 = SURFACE_CHROME;
pub const BORDER: Color32 = LINE;
/// Selected-state outline (formerly the classic #99D1FF).
pub const CLASSIC_BLUE: Color32 = ACCENT_SOFT_LINE;
/// Selected / highlighted fill (formerly the classic #CCE8FF).
pub const HOVER_BLUE: Color32 = ACCENT_SOFT;

// ------------------------------------------------------------ metrics --

pub const RADIUS_SM: u8 = 4;
pub const RADIUS_MD: u8 = 6;
pub const RADIUS_LG: u8 = 10;
pub const RADIUS_XL: u8 = 14;

pub const UI_SIZE: f32 = 13.0;
pub const TITLE_SIZE: f32 = 17.0;
pub const EDITOR_SIZE: f32 = 40.0 / 3.0;
pub const EDITOR_LINE_HEIGHT: f32 = 15.0;
pub const MENU_HEIGHT: f32 = 30.0;
pub const TOOLBAR_HEIGHT: f32 = 46.0;
pub const TOOL_ICON: f32 = 16.0;
pub const STATUS_HEIGHT: f32 = 26.0;

/// Soft elevation used by dialogs.
pub fn shadow_dialog() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 12],
        blur: 36,
        spread: 0,
        color: Color32::from_black_alpha(46),
    }
}
/// Lighter elevation used by popups and menus.
pub fn shadow_popup() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(30),
    }
}

/// Paint the empty-content canvas: [`CANVAS`] with a faint dot grid anchored
/// to `rect`'s top-left (so it does not shimmer while content scrolls).
pub fn paint_canvas(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 0.0, CANVAS);
    let clip = painter.clip_rect().intersect(rect);
    if !clip.is_positive() {
        return;
    }
    let step = CANVAS_DOT_STEP;
    let mut mesh = egui::Mesh::default();
    let first = |lo: f32, origin: f32| origin + ((lo - origin) / step).ceil().max(0.0) * step;
    let mut y = first(clip.top(), rect.top() + step / 2.0);
    while y < clip.bottom() {
        let mut x = first(clip.left(), rect.left() + step / 2.0);
        while x < clip.right() {
            mesh.add_colored_rect(
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(1.5, 1.5)),
                CANVAS_DOT,
            );
            x += step;
        }
        y += step;
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Emphasized (bold) UI font; falls back to the regular face until the
/// system fonts are installed (and in headless tests).
pub fn strong(ctx: &egui::Context, size: f32) -> egui::FontId {
    let family = egui::FontFamily::Name(super::fonts::STRONG_FAMILY.into());
    if ctx.fonts(|f| f.families().contains(&family)) {
        egui::FontId::new(size, family)
    } else {
        egui::FontId::proportional(size)
    }
}

/// Semantic tokens bundled for code that wants a value object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub panel_fill: Color32,
    pub window_fill: Color32,
    pub border: Color32,
    pub text: Color32,
    pub selected_fill: Color32,
    pub selected_border: Color32,
    pub difference_fill: Color32,
    pub difference_text: Color32,
    pub menu_height: u16,
    pub toolbar_height: u16,
    pub status_height: u16,
    pub ui_size: u16,
    pub editor_size: f32,
    pub editor_line_height: u16,
    pub icon_size: u16,
    /// 外框配色（换肤入口）；`panel_fill` 等旧字段保留为兼容别名。
    pub palette: Palette,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            panel_fill: SURFACE_CHROME,
            window_fill: SURFACE,
            border: LINE,
            text: TEXT,
            selected_fill: ACCENT_SOFT,
            selected_border: ACCENT_SOFT_LINE,
            // Diff colors are content semantics and stay as they were.
            difference_fill: Color32::from_rgb(252, 230, 230),
            difference_text: Color32::from_rgb(252, 48, 48),
            menu_height: MENU_HEIGHT as u16,
            toolbar_height: TOOLBAR_HEIGHT as u16,
            status_height: STATUS_HEIGHT as u16,
            ui_size: UI_SIZE as u16,
            editor_size: EDITOR_SIZE,
            editor_line_height: EDITOR_LINE_HEIGHT as u16,
            icon_size: TOOL_ICON as u16,
            palette: Palette::LIGHT,
        }
    }
}

/// Apply the Matcha design system to every egui style.
pub fn apply(ctx: &egui::Context) {
    Theme::default().apply(ctx);
}

impl Theme {
    /// 换一套外框配色（换肤预留入口；目前只有 [`Palette::LIGHT`]）。
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self.panel_fill = palette.surface_chrome;
        self.window_fill = palette.surface;
        self.border = palette.line;
        self.text = palette.text;
        self.selected_fill = palette.accent_soft;
        self.selected_border = palette.accent_soft_line;
        self
    }

    /// 将本实例的颜色、度量和字号应用到所有全局样式，并安装配色供 [`palette`] 读取。
    pub fn apply(&self, ctx: &egui::Context) {
        let theme = *self;
        let p = theme.palette;
        ctx.data_mut(|d| d.insert_temp(palette_id(), p));
        ctx.all_styles_mut(|style| {
            let mut visuals = Visuals::light();
            visuals.window_fill = theme.window_fill;
            visuals.panel_fill = theme.panel_fill;
            visuals.faint_bg_color = p.surface_subtle;
            visuals.extreme_bg_color = p.surface;
            visuals.code_bg_color = p.surface_subtle;
            visuals.hyperlink_color = p.accent;
            visuals.warn_fg_color = p.warning;
            visuals.error_fg_color = p.danger;
            let md = CornerRadius::same(RADIUS_MD);
            visuals.widgets.noninteractive = widget(theme.panel_fill, theme.border, theme.text, md);
            visuals.widgets.inactive = widget(p.surface, p.line_strong, theme.text, md);
            visuals.widgets.hovered = widget(p.hover, p.line_strong, theme.text, md);
            visuals.widgets.active = widget(p.pressed, p.line_strong, theme.text, md);
            visuals.widgets.open = widget(p.hover, p.line_strong, theme.text, md);
            // Buttons paint `weak_bg_fill`; `bg_fill` is what scrollbar
            // handles and slider rails use — keep it a visible
            // gray ramp so thumbs read on white content.
            visuals.widgets.inactive.weak_bg_fill = p.surface;
            visuals.widgets.inactive.bg_fill = SCROLL_THUMB;
            visuals.widgets.hovered.bg_fill = SCROLL_THUMB_HOVER;
            visuals.widgets.active.bg_fill = SCROLL_THUMB_ACTIVE;
            visuals.selection.bg_fill = p.selection;
            visuals.selection.stroke = Stroke::new(1.0, p.accent_pressed);
            visuals.text_cursor.stroke = Stroke::new(1.5, p.accent);
            visuals.window_corner_radius = CornerRadius::same(RADIUS_XL);
            visuals.menu_corner_radius = CornerRadius::same(RADIUS_LG);
            visuals.window_stroke = Stroke::new(1.0, theme.border);
            visuals.window_shadow = shadow_dialog();
            visuals.popup_shadow = shadow_popup();
            visuals.striped = false;
            style.visuals = visuals;
            for kind in [egui::TextStyle::Body, egui::TextStyle::Button] {
                style
                    .text_styles
                    .insert(kind, egui::FontId::proportional(theme.ui_size as f32));
            }
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
            style.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::proportional(TITLE_SIZE),
            );
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::monospace(theme.editor_size),
            );
            style.spacing.interact_size.y = 22.0;
            style.spacing.scroll = egui::style::ScrollStyle::solid();
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.menu_margin = egui::Margin::same(6);
            style.spacing.window_margin = egui::Margin::same(16);
        });
    }
}

fn widget(
    fill: Color32,
    border: Color32,
    text: Color32,
    radius: CornerRadius,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, border),
        fg_stroke: Stroke::new(1.0, text),
        corner_radius: radius,
        expansion: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_names_follow_the_modern_tokens() {
        let theme = Theme::default();
        assert_eq!(theme.ui_size as f32, UI_SIZE);
        assert_eq!(theme.selected_fill, HOVER_BLUE);
        assert_eq!(PANEL_BG, SURFACE_CHROME);
        assert_eq!(BORDER, LINE);
    }

    #[test]
    fn apply_sets_rounded_modern_style() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.style_of(egui::Theme::Light);
        assert_eq!(
            style.visuals.widgets.active.corner_radius,
            CornerRadius::same(RADIUS_MD)
        );
        assert_eq!(style.visuals.widgets.inactive.bg_stroke.width, 1.0);
        assert_eq!(style.visuals.panel_fill, PANEL_BG);
        assert_eq!(style.text_styles[&egui::TextStyle::Body].size, UI_SIZE);
        assert_eq!(palette(&ctx), Palette::LIGHT);
    }

    #[test]
    fn swapped_palette_is_installed_and_drives_visuals() {
        let ctx = egui::Context::default();
        let dark = Palette {
            surface: Color32::from_gray(30),
            accent: Color32::from_rgb(1, 2, 3),
            ..Palette::LIGHT
        };
        Theme::default().with_palette(dark).apply(&ctx);
        assert_eq!(palette(&ctx).accent, Color32::from_rgb(1, 2, 3));
        let style = ctx.style_of(egui::Theme::Light);
        assert_eq!(style.visuals.extreme_bg_color, Color32::from_gray(30));
    }
}
