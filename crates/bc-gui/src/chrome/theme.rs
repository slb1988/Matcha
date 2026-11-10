//! Shared classic-light theme and the initial visual-parity token values.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

/// Initial colors measured from the seed screenshot (seed; calibrate in group 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub editor_size: u16,
    pub editor_line_height: u16,
    pub icon_size: u16,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            panel_fill: Color32::from_rgb(240, 240, 240),
            window_fill: Color32::WHITE,
            border: Color32::from_rgb(160, 160, 160),
            text: Color32::from_rgb(0, 0, 0),
            selected_fill: Color32::from_rgb(204, 232, 255),
            selected_border: Color32::from_rgb(153, 209, 255),
            difference_fill: Color32::from_rgb(255, 227, 227),
            difference_text: Color32::from_rgb(255, 0, 0),
            menu_height: 27,
            toolbar_height: 43,
            status_height: 24,
            ui_size: 12,
            editor_size: 12,
            editor_line_height: 15,
            icon_size: 16,
        }
    }
}

pub const PANEL_BG: Color32 = Color32::from_rgb(240, 240, 240);
pub const BORDER: Color32 = Color32::from_rgb(160, 160, 160);
pub const CLASSIC_BLUE: Color32 = Color32::from_rgb(153, 209, 255);
pub const HOVER_BLUE: Color32 = Color32::from_rgb(204, 232, 255);
pub const UI_SIZE: f32 = 12.0;
pub const EDITOR_SIZE: f32 = 12.0;
pub const EDITOR_LINE_HEIGHT: f32 = 15.0;
pub const MENU_HEIGHT: f32 = 27.0;
pub const TOOLBAR_HEIGHT: f32 = 43.0;
pub const TOOL_ICON: f32 = 16.0;
pub const STATUS_HEIGHT: f32 = 24.0;

/// Apply the classic flat, one-pixel, zero-radius widget style.
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
            visuals.faint_bg_color = Color32::from_rgb(246, 246, 246);
            visuals.extreme_bg_color = Color32::WHITE;
            visuals.widgets.noninteractive = widget(theme.panel_fill, theme.border, theme.text);
            visuals.widgets.inactive =
                widget(Color32::from_rgb(248, 248, 248), theme.border, theme.text);
            visuals.widgets.hovered =
                widget(theme.selected_fill, theme.selected_border, theme.text);
            visuals.widgets.active = widget(
                Color32::from_rgb(194, 218, 244),
                theme.selected_border,
                theme.text,
            );
            visuals.widgets.open = widget(theme.selected_fill, theme.selected_border, theme.text);
            visuals.selection.bg_fill = Color32::from_rgb(0, 120, 215);
            visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
            visuals.window_corner_radius = CornerRadius::ZERO;
            visuals.menu_corner_radius = CornerRadius::ZERO;
            visuals.window_stroke = Stroke::new(1.0, theme.border);
            visuals.popup_shadow = egui::epaint::Shadow {
                offset: [2, 2],
                blur: 4,
                spread: 0,
                color: Color32::from_black_alpha(72),
            };
            style.visuals = visuals;
            for kind in [
                egui::TextStyle::Body,
                egui::TextStyle::Button,
                egui::TextStyle::Small,
            ] {
                style
                    .text_styles
                    .insert(kind, egui::FontId::proportional(theme.ui_size as f32));
            }
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::monospace(theme.editor_size as f32),
            );
            style.spacing.interact_size.y = 20.0;
            style.spacing.scroll = egui::style::ScrollStyle::solid();
            style.spacing.item_spacing = egui::vec2(4.0, 2.0);
            style.spacing.button_padding = egui::vec2(6.0, 3.0);
            style.spacing.menu_margin = egui::Margin::same(2);
            style.spacing.window_margin = egui::Margin::same(6);
        });
    }
}

fn widget(fill: Color32, border: Color32, text: Color32) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, border),
        fg_stroke: Stroke::new(1.0, text),
        corner_radius: CornerRadius::ZERO,
        expansion: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_tokens_are_stable() {
        let theme = Theme::default();
        assert_eq!(theme.ui_size, 12);
        assert_eq!(theme.editor_line_height, 15);
        assert_eq!(theme.selected_fill, HOVER_BLUE);
    }

    #[test]
    fn tokens_file_is_optional_but_checked_when_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/visual-parity/tokens.json");
        if !path.exists() {
            eprintln!("tokens.json 尚未合入；跳过真源断言");
            return;
        }
        let tokens: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let t = Theme::default();
        for (name, color) in [
            ("panel_fill", t.panel_fill),
            ("window_fill", t.window_fill),
            ("border", t.border),
            ("text", t.text),
            ("selected_fill", t.selected_fill),
            ("selected_border", t.selected_border),
            ("difference_fill", t.difference_fill),
            ("difference_text", t.difference_text),
        ] {
            let expected = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
            assert_eq!(
                tokens["colors"][name]
                    .as_str()
                    .map(str::to_ascii_uppercase)
                    .as_deref(),
                Some(expected.as_str()),
                "colors.{name}"
            );
        }
        for (name, value) in [
            ("menu_height", t.menu_height),
            ("toolbar_height", t.toolbar_height),
            ("status_height", t.status_height),
            ("editor_line_height", t.editor_line_height),
            ("icon_size", t.icon_size),
        ] {
            assert_eq!(
                tokens["metrics"][name].as_f64(),
                Some(value as f64),
                "metrics.{name}"
            );
        }
        for (name, value) in [("ui_size", t.ui_size), ("editor_size", t.editor_size)] {
            assert_eq!(
                tokens["fonts"][name].as_f64(),
                Some(value as f64),
                "fonts.{name}"
            );
        }
    }

    #[test]
    fn apply_sets_flat_classic_style() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.style_of(egui::Theme::Light);
        assert_eq!(
            style.visuals.widgets.active.corner_radius,
            CornerRadius::ZERO
        );
        assert_eq!(style.visuals.widgets.inactive.bg_stroke.width, 1.0);
        assert_eq!(style.visuals.panel_fill, PANEL_BG);
        assert_eq!(style.text_styles[&egui::TextStyle::Body].size, UI_SIZE);
    }
}
