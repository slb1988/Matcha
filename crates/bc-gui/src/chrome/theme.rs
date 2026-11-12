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
pub const WARNING: Color32 = Color32::from_rgb(0xB4, 0x6F, 0x0E);
pub const WARNING_SOFT: Color32 = Color32::from_rgb(0xFD, 0xF3, 0xE1);
pub const INFO: Color32 = Color32::from_rgb(0x2D, 0x6C, 0xD6);
pub const INFO_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF0, 0xFD);

/// Modal backdrop scrim.
pub const SCRIM: Color32 = Color32::from_black_alpha(64);

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
        }
    }
}

/// Apply the Matcha design system to every egui style.
pub fn apply(ctx: &egui::Context) {
    Theme::default().apply(ctx);
}

impl Theme {
    /// 将本实例的颜色、度量和字号应用到所有全局样式。
    pub fn apply(&self, ctx: &egui::Context) {
        let theme = *self;
        ctx.all_styles_mut(|style| {
            let mut visuals = Visuals::light();
            visuals.window_fill = theme.window_fill;
            visuals.panel_fill = theme.panel_fill;
            visuals.faint_bg_color = SURFACE_SUBTLE;
            visuals.extreme_bg_color = SURFACE;
            visuals.code_bg_color = SURFACE_SUBTLE;
            visuals.hyperlink_color = ACCENT;
            visuals.warn_fg_color = WARNING;
            visuals.error_fg_color = DANGER;
            let md = CornerRadius::same(RADIUS_MD);
            visuals.widgets.noninteractive = widget(theme.panel_fill, theme.border, theme.text, md);
            visuals.widgets.inactive = widget(SURFACE, LINE_STRONG, theme.text, md);
            visuals.widgets.hovered = widget(HOVER, LINE_STRONG, theme.text, md);
            visuals.widgets.active = widget(PRESSED, LINE_STRONG, theme.text, md);
            visuals.widgets.open = widget(HOVER, LINE_STRONG, theme.text, md);
            // Flat (frameless) buttons only get a fill on hover.
            visuals.widgets.inactive.weak_bg_fill = SURFACE;
            visuals.selection.bg_fill = SELECTION;
            visuals.selection.stroke = Stroke::new(1.0, ACCENT_PRESSED);
            visuals.text_cursor.stroke = Stroke::new(1.5, ACCENT);
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
    }
}
