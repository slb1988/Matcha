//! Shared classic-light theme and the initial visual-parity token values.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

/// Initial colors measured from the seed screenshot (seed; calibrate in group 3).
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
            panel_fill: Color32::from_rgb(240, 240, 240),
            window_fill: Color32::WHITE,
            border: Color32::from_rgb(160, 160, 160),
            text: Color32::from_rgb(0, 0, 0),
            selected_fill: Color32::from_rgb(204, 232, 255),
            selected_border: Color32::from_rgb(153, 209, 255),
            difference_fill: Color32::from_rgb(252, 230, 230),
            difference_text: Color32::from_rgb(252, 48, 48),
            menu_height: 27,
            toolbar_height: 43,
            status_height: 25,
            ui_size: 12,
            editor_size: 40.0 / 3.0,
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
pub const EDITOR_SIZE: f32 = 40.0 / 3.0;
pub const EDITOR_LINE_HEIGHT: f32 = 15.0;
pub const MENU_HEIGHT: f32 = 27.0;
pub const TOOLBAR_HEIGHT: f32 = 43.0;
pub const TOOL_ICON: f32 = 16.0;
pub const STATUS_HEIGHT: f32 = 25.0;

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
                egui::FontId::monospace(theme.editor_size),
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
        let tokens: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(path)
                .unwrap()
                .trim_start_matches('\u{feff}'),
        )
        .unwrap();
        let t = Theme::default();
        for (name, color) in [
            ("panel", t.panel_fill),
            ("menubar", t.window_fill),
            ("separator", t.border),
            ("textForeground", t.text),
            ("toolbarToggle", t.selected_fill),
            ("toolbarToggleBorder", t.selected_border),
            ("diffRow", t.difference_fill),
            ("diffForeground", t.difference_text),
        ] {
            let expected = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
            assert_eq!(
                tokens["colors"][name]["hex"]
                    .as_str()
                    .map(str::to_ascii_uppercase)
                    .as_deref(),
                Some(expected.as_str()),
                "colors.{name}"
            );
        }
        for (name, value) in [
            ("menubarHeight", t.menu_height),
            ("toolbarHeight", t.toolbar_height),
            ("statusbarHeight", t.status_height),
            ("lineHeight", t.editor_line_height),
        ] {
            assert_eq!(
                tokens["metrics"][name]["value"].as_f64(),
                Some(value as f64),
                "metrics.{name}"
            );
        }
        assert_eq!(
            tokens["metrics"]["toolbarIconSize"]["value"],
            serde_json::json!([t.icon_size, t.icon_size])
        );
        assert_eq!(
            tokens["fonts"]["ui"]["sizePt"].as_f64().unwrap() * 4.0 / 3.0,
            t.ui_size as f64
        );
        assert!(
            (tokens["fonts"]["editor"]["sizePx"].as_f64().unwrap() - t.editor_size as f64).abs()
                < 0.001
        );
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
