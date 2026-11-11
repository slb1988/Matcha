//! Options dialog (Tools > Options): global preferences.
//!
//! Laid out like the reference (`dlg-options.png`): a #E3E7ED navigation
//! column with a search box and 37 px page rows (the selected row joins the
//! #F0F0F0 content like a tab), page content to the right and OK / Cancel /
//! Apply under a full-width rule. Only pages backed by real `Prefs` fields are
//! enabled (Appearance: difference colors; Text Editing: default tab size and
//! code page; Backups); the reference's other pages are listed disabled.
use crate::chrome::dialog::{self, ButtonKind, Placement};
use bc_session::prefs::Prefs;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// Client size of the reference dialog.
pub const CLIENT: Vec2 = Vec2::new(859.0, 655.0);
/// Reference page list; `true` = backed by Matcha preferences.
pub const PAGES: [(&str, bool); 11] = [
    ("Startup", false),
    ("Tabs", false),
    ("Appearance", true),
    ("Text Editing", true),
    ("Next Difference", false),
    ("Backups", true),
    ("File Operations", false),
    ("Archive Types", false),
    ("Commands", false),
    ("Open With", false),
    ("Tweaks", false),
];
const NAV: Color32 = Color32::from_rgb(0xE3, 0xE7, 0xED);
const NAV_WIDTH: f32 = 241.0;
const ROW: f32 = 37.0;
const FIRST_ROW: f32 = 54.0;
const FOCUS: Color32 = Color32::from_rgb(0x00, 0x67, 0xC0);

#[derive(Default)]
pub struct OptionsDialog {
    pub open: bool,
    pub page: usize,
    placement: Option<Placement>,
    draft: Option<Prefs>,
    tab_size: String,
    code_page: String,
    suffix: String,
    search: String,
    error: Option<String>,
}

fn at(o: Pos2, x: f32, y: f32) -> Pos2 {
    o + Vec2::new(x, y)
}
fn span(o: Pos2, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(at(o, x0, y0), at(o, x1, y1))
}

impl OptionsDialog {
    pub fn show(&mut self, prefs: &Prefs, placement: Placement) {
        self.tab_size = prefs.default_tab_size.to_string();
        self.code_page = prefs.default_code_page.to_string();
        self.suffix = prefs.backups.extension.clone().unwrap_or_default();
        self.draft = Some(prefs.clone());
        self.placement = Some(placement);
        self.page = PAGES.iter().position(|(_, on)| *on).unwrap_or(0);
        self.search.clear();
        self.error = None;
        self.open = true;
    }
    /// Draft with the text fields folded in (numbers validated).
    fn folded(&self) -> Result<Prefs, String> {
        let mut prefs = self.draft.clone().ok_or("no draft")?;
        prefs.default_tab_size = self
            .tab_size
            .trim()
            .parse()
            .ok()
            .filter(|n| (0..=64).contains(n))
            .ok_or("Tab size must be 0..64 (0 = format default)")?;
        prefs.default_code_page = self
            .code_page
            .trim()
            .parse()
            .map_err(|_| "Code page must be a number".to_string())?;
        prefs.backups.extension = (!self.suffix.is_empty()).then(|| self.suffix.clone());
        Ok(prefs)
    }
    /// Draw the dialog; OK / Apply write the draft into `prefs`.
    pub fn ui(&mut self, ctx: &egui::Context, prefs: &mut Prefs) {
        if !self.open || self.draft.is_none() {
            return;
        }
        let placement = self.placement.unwrap_or(Placement::Window);
        let changed = self.folded().is_ok_and(|p| p != *prefs);
        let mut ok = false;
        let mut cancel = false;
        let mut apply = false;
        let (closed, ()) = dialog::window(
            ctx,
            egui::Id::unique("options-dialog"),
            "Options",
            CLIENT,
            placement,
            |ui, client| {
                let o = client.min;
                self.navigation(ui, o);
                self.content(ui, o);
                let painter = ui.painter().clone();
                dialog::rule(&painter, o.x, o.x + CLIENT.x, o.y + 604.0, dialog::RULE);
                if let Some(error) = &self.error {
                    painter.text(
                        at(o, 258.0, 629.0),
                        egui::Align2::LEFT_CENTER,
                        error,
                        dialog::font(),
                        Color32::from_rgb(0xC0, 0x00, 0x00),
                    );
                }
                ok = dialog::button(
                    ui,
                    span(o, 544.0, 615.0, 638.0, 644.0),
                    "OK",
                    ButtonKind::Default,
                    true,
                );
                cancel = dialog::button(
                    ui,
                    span(o, 649.0, 615.0, 743.0, 644.0),
                    "Cancel",
                    ButtonKind::Normal,
                    true,
                );
                apply = dialog::button(
                    ui,
                    span(o, 754.0, 615.0, 848.0, 644.0),
                    "Apply",
                    ButtonKind::Normal,
                    changed,
                );
            },
        );
        if closed || cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.open = false;
            self.draft = None;
            return;
        }
        if ok || apply {
            match self.folded() {
                Ok(next) => {
                    *prefs = next;
                    self.error = None;
                    if ok {
                        self.open = false;
                        self.draft = None;
                    }
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn navigation(&mut self, ui: &mut egui::Ui, o: Pos2) {
        let painter = ui.painter().clone();
        painter.rect_filled(span(o, 0.0, 0.0, NAV_WIDTH, 604.0), 0.0, NAV);
        painter.rect_filled(
            span(o, NAV_WIDTH, 0.0, NAV_WIDTH + 1.0, 604.0),
            0.0,
            dialog::RULE,
        );
        // Page filter box (focused look: 2 px blue underline).
        let search = span(o, 10.0, 10.0, 231.0, 35.0);
        painter.rect_filled(search, 0.0, Color32::WHITE);
        painter.rect_stroke(
            search,
            0.0,
            Stroke::new(1.0, Color32::from_gray(0xEC)),
            StrokeKind::Inside,
        );
        painter.rect_filled(span(o, 10.0, 33.0, 231.0, 35.0), 0.0, FOCUS);
        let ring = at(o, 21.0, 20.0);
        painter.circle_stroke(ring, 3.5, Stroke::new(1.4, Color32::from_gray(0x6E)));
        painter.line_segment(
            [ring + Vec2::new(-2.5, 2.5), ring + Vec2::new(-5.0, 5.0)],
            Stroke::new(1.8, Color32::from_gray(0x6E)),
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(span(o, 30.0, 12.0, 228.0, 32.0)),
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .id(egui::Id::unique("options-search"))
                        .frame(egui::Frame::NONE)
                        .font(dialog::font())
                        .desired_width(196.0),
                );
            },
        );
        let query = self.search.trim().to_lowercase();
        let font = FontId::proportional(16.0);
        let mut row = 0;
        for (index, (name, enabled)) in PAGES.into_iter().enumerate() {
            if !query.is_empty() && !name.to_lowercase().contains(&query) {
                continue;
            }
            let top = FIRST_ROW + ROW * row as f32;
            row += 1;
            let rect = span(o, 0.0, top, NAV_WIDTH, top + ROW);
            if index == self.page {
                // Selected row joins the content: #F0 across the divider,
                // #A0 rules above and below.
                painter.rect_filled(
                    span(o, 0.0, top, NAV_WIDTH + 1.0, top + ROW),
                    0.0,
                    dialog::FACE,
                );
                dialog::rule(&painter, o.x, o.x + NAV_WIDTH, o.y + top, dialog::RULE);
                dialog::rule(
                    &painter,
                    o.x,
                    o.x + NAV_WIDTH,
                    o.y + top + ROW - 1.0,
                    dialog::RULE,
                );
            }
            painter.text(
                at(o, 45.0, top + ROW / 2.0),
                egui::Align2::LEFT_CENTER,
                name,
                font.clone(),
                if enabled {
                    Color32::BLACK
                } else {
                    dialog::DISABLED_TEXT
                },
            );
            if enabled
                && ui
                    .interact(
                        rect,
                        egui::Id::unique(("options-page", index)),
                        egui::Sense::click(),
                    )
                    .clicked()
            {
                self.page = index;
            }
        }
    }
    fn content(&mut self, ui: &mut egui::Ui, o: Pos2) {
        let painter = ui.painter().clone();
        let (name, _) = PAGES[self.page];
        dialog::heading(&painter, at(o, 257.0, 12.0), name);
        let draft = self.draft.as_mut().unwrap();
        match name {
            "Appearance" => {
                dialog::label(&painter, at(o, 258.0, 46.0), "Difference colors:", true);
                for (n, (label, rgb)) in [
                    ("Important differences", &mut draft.colors.important),
                    ("Unimportant differences", &mut draft.colors.unimportant),
                    ("Inserted / deleted lines", &mut draft.colors.inserted),
                ]
                .into_iter()
                .enumerate()
                {
                    let y = 70.0 + 30.0 * n as f32;
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(span(o, 274.0, y, 310.0, y + 22.0)),
                        |ui| {
                            ui.color_edit_button_srgb(rgb);
                        },
                    );
                    dialog::label(&painter, at(o, 318.0, y + 3.0), label, true);
                }
            }
            "Text Editing" => {
                dialog::label(
                    &painter,
                    at(o, 258.0, 46.0),
                    "Default tab size (0 = use the file format's):",
                    true,
                );
                dialog::text_field(
                    ui,
                    span(o, 258.0, 67.0, 338.0, 92.0),
                    egui::Id::unique("options-tab-size"),
                    &mut self.tab_size,
                    true,
                );
                dialog::label(
                    &painter,
                    at(o, 258.0, 106.0),
                    "Default code page (used when detection is inconclusive):",
                    true,
                );
                dialog::text_field(
                    ui,
                    span(o, 258.0, 127.0, 338.0, 152.0),
                    egui::Id::unique("options-code-page"),
                    &mut self.code_page,
                    true,
                );
            }
            "Backups" => {
                dialog::checkbox(
                    ui,
                    at(o, 258.0, 49.0),
                    "Create a backup before saving",
                    &mut draft.backups.enabled,
                    true,
                );
                dialog::label(
                    &painter,
                    at(o, 258.0, 78.0),
                    "Backup suffix (empty = \"~\" prefix):",
                    draft.backups.enabled,
                );
                dialog::text_field(
                    ui,
                    span(o, 258.0, 99.0, 548.0, 124.0),
                    egui::Id::unique("options-suffix"),
                    &mut self.suffix,
                    draft.backups.enabled,
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        d: &mut OptionsDialog,
        prefs: &mut Prefs,
        events: Vec<egui::Event>,
    ) {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, CLIENT)),
                events,
                ..Default::default()
            },
            |ui| d.ui(ui.ctx(), prefs),
        )
        .drop_without_applying_deltas();
    }
    fn click(pos: Pos2) -> Vec<egui::Event> {
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![egui::Event::PointerMoved(pos), press(true), press(false)]
    }
    #[test]
    fn apply_ok_and_cancel_follow_classic_semantics() {
        let ctx = egui::Context::default();
        let mut prefs = Prefs::default();
        let mut d = OptionsDialog::default();
        d.show(&prefs, Placement::ClientOnly);
        assert_eq!(PAGES[d.page].0, "Appearance");
        frame(&ctx, &mut d, &mut prefs, vec![]);
        // Disabled pages cannot be selected; Backups can.
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + 18.0)),
        );
        assert_eq!(PAGES[d.page].0, "Appearance");
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + 5.0 * ROW + 18.0)),
        );
        assert_eq!(PAGES[d.page].0, "Backups");
        frame(&ctx, &mut d, &mut prefs, vec![]);
        // Toggle backups, Apply writes but keeps the dialog open.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(800.0, 629.0)));
        assert!(prefs.backups.enabled && d.open);
        // Cancel after another change discards it.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(695.0, 629.0)));
        assert!(prefs.backups.enabled && !d.open);
        // OK with an invalid tab size keeps the dialog open.
        d.show(&prefs, Placement::ClientOnly);
        d.tab_size = "65".into();
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(d.open && d.error.is_some());
        d.tab_size = "8".into();
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(!d.open);
        assert_eq!(prefs.default_tab_size, 8);
    }
}
