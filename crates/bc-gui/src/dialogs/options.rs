//! Options dialog (Tools > Options): global preferences.
//!
//! Laid out like the reference (`dlg-options.png`): a #E3E7ED navigation
//! column with a search box and 37 px page rows (the selected row joins the
//! #F0F0F0 content like a tab), page content to the right and OK / Cancel /
//! Apply under a full-width rule. Only pages backed by real `Prefs` fields are
//! enabled (Appearance: difference colors and the editor font — family from the
//! installed monospace fonts, size in points; Text Editing: default tab size and
//! code page; Backups) plus Startup, which holds the Windows Explorer context
//! menu switch (`bc-shell`: registers or removes the HKCU verbs on OK/Apply);
//! the reference's other pages are listed disabled.
use crate::chrome::{
    dialog::{self, ButtonKind, Placement},
    theme,
};
use bc_session::prefs::Prefs;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// Client size of the reference dialog.
pub const CLIENT: Vec2 = Vec2::new(859.0, 655.0);
/// Reference page list; `true` = backed by Matcha preferences.
pub const PAGES: [(&str, bool); 11] = [
    ("Startup", true),
    ("Tabs", false),
    ("Appearance", true),
    ("Text Editing", true),
    ("Next Difference", true),
    ("Backups", true),
    ("File Operations", false),
    ("Archive Types", false),
    ("Commands", false),
    ("Open With", false),
    ("Tweaks", true),
];
const NAV_WIDTH: f32 = 241.0;
const ROW: f32 = 37.0;
const FIRST_ROW: f32 = 54.0;

#[derive(Default)]
pub struct OptionsDialog {
    pub open: bool,
    pub page: usize,
    placement: Option<Placement>,
    draft: Option<Prefs>,
    tab_size: String,
    code_page: String,
    suffix: String,
    /// Editor font size (points) as typed.
    font_size: String,
    search: String,
    error: Option<String>,
    /// Explorer context menu wanted (draft) / as last applied.
    shell_menu: bool,
    shell_menu_saved: bool,
    /// Applies the Explorer menu setting; `None` = the real registry (tests
    /// install a fake so they never touch HKCU).
    pub shell_apply: Option<fn(bool) -> Result<(), String>>,
}

/// Current Explorer menu setting (on unless turned off).
fn shell_menu_enabled() -> bool {
    bc_shell::Shell::from_env().map_or(true, |shell| shell.state.enabled())
}

/// Persist the setting and register / unregister the verbs.
fn apply_shell_menu(on: bool) -> Result<(), String> {
    let mut registry = bc_shell::system_registry()
        .ok_or("The Explorer context menu is only available on Windows")?;
    let shell = bc_shell::Shell::from_env().map_err(|e| e.to_string())?;
    bc_shell::set_enabled(&shell, &mut *registry, on)
        .map(drop)
        .map_err(|e| format!("Explorer context menu: {e}"))
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
        self.font_size = prefs.editor_font.size_pt.to_string();
        crate::chrome::font_catalog::warm();
        self.draft = Some(prefs.clone());
        self.placement = Some(placement);
        if self.shell_apply.is_none() {
            self.shell_menu_saved = shell_menu_enabled();
        }
        self.shell_menu = self.shell_menu_saved;
        self.page = PAGES
            .iter()
            .position(|(name, _)| *name == "Appearance")
            .unwrap_or(0);
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
        let range = bc_session::prefs::EditorFont::SIZE_RANGE;
        prefs.editor_font.size_pt = self
            .font_size
            .trim()
            .parse()
            .ok()
            .filter(|n| range.contains(n))
            .ok_or_else(|| format!("Font size must be {}..{} pt", range.start(), range.end()))?;
        Ok(prefs)
    }
    /// Draw the dialog; OK / Apply write the draft into `prefs`.
    pub fn ui(&mut self, ctx: &egui::Context, prefs: &mut Prefs) {
        if !self.open || self.draft.is_none() {
            return;
        }
        let placement = self.placement.unwrap_or(Placement::Window);
        let changed =
            self.folded().is_ok_and(|p| p != *prefs) || self.shell_menu != self.shell_menu_saved;
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
        if closed
            || cancel
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.open = false;
            self.draft = None;
            return;
        }
        if (ok || apply) && self.shell_menu != self.shell_menu_saved {
            match (self.shell_apply.unwrap_or(apply_shell_menu))(self.shell_menu) {
                Ok(()) => self.shell_menu_saved = self.shell_menu,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
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
        let p = theme::palette(ui.ctx());
        painter.rect_filled(span(o, 0.0, 0.0, NAV_WIDTH, 604.0), 0.0, p.surface_subtle);
        painter.rect_filled(
            span(o, NAV_WIDTH, 0.0, NAV_WIDTH + 1.0, 604.0),
            0.0,
            dialog::RULE,
        );
        // Page filter box: rounded field, accent outline while focused.
        let search = span(o, 10.0, 10.0, 231.0, 35.0);
        let focused = ui.memory(|m| m.has_focus(egui::Id::unique("options-search")));
        painter.rect_filled(search, theme::RADIUS_MD, p.surface);
        painter.rect_stroke(
            search,
            theme::RADIUS_MD,
            Stroke::new(1.0, if focused { p.accent } else { p.line_strong }),
            StrokeKind::Inside,
        );
        let ring = at(o, 21.0, 20.0);
        painter.circle_stroke(ring, 3.5, Stroke::new(1.4, p.text_faint));
        painter.line_segment(
            [ring + Vec2::new(2.5, 2.5), ring + Vec2::new(5.0, 5.0)],
            Stroke::new(1.8, p.text_faint),
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
            let pill = span(o, 8.0, top + 3.0, NAV_WIDTH - 8.0, top + ROW - 3.0);
            if index == self.page {
                // Selected page: accent pill (same as list rows elsewhere).
                painter.rect_filled(pill, theme::RADIUS_MD, p.accent_soft);
            } else if enabled && ui.rect_contains_pointer(rect) {
                painter.rect_filled(pill, theme::RADIUS_MD, p.hover);
            }
            painter.text(
                at(o, 45.0, top + ROW / 2.0),
                egui::Align2::LEFT_CENTER,
                name,
                font.clone(),
                if !enabled {
                    p.text_faint
                } else if index == self.page {
                    p.accent_pressed
                } else {
                    p.text
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
    /// Appearance › Editor font: installed monospace family + size (points).
    fn editor_font(&mut self, ui: &mut egui::Ui, o: Pos2) {
        let painter = ui.painter().clone();
        let draft = self.draft.as_mut().unwrap();
        dialog::label(&painter, at(o, 258.0, 176.0), "Editor font:", true);
        let mut families: Vec<String> = match crate::chrome::font_catalog::ready() {
            Some(list) => list.iter().map(|f| f.name.clone()).collect(),
            None => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
                Vec::new()
            }
        };
        if !families
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&draft.editor_font.family))
        {
            families.insert(0, draft.editor_font.family.clone());
        }
        if let Some(i) = dialog::combo(
            ui,
            span(o, 274.0, 200.0, 534.0, 225.0),
            egui::Id::unique("options-editor-font"),
            &draft.editor_font.family,
            &families,
            true,
        ) {
            draft.editor_font.family = families[i].clone();
        }
        dialog::label(&painter, at(o, 550.0, 204.0), "Size:", true);
        dialog::text_field(
            ui,
            span(o, 590.0, 200.0, 640.0, 225.0),
            egui::Id::unique("options-editor-font-size"),
            &mut self.font_size,
            true,
        );
        dialog::label(&painter, at(o, 648.0, 204.0), "pt", true);
        dialog::label(
            &painter,
            at(o, 274.0, 236.0),
            "Used by Text Compare and Text Merge; saved for later sessions.",
            false,
        );
    }
    fn content(&mut self, ui: &mut egui::Ui, o: Pos2) {
        let painter = ui.painter().clone();
        let (name, _) = PAGES[self.page];
        dialog::heading(&painter, at(o, 257.0, 12.0), name);
        let draft = self.draft.as_mut().unwrap();
        match name {
            "Startup" => {
                let available = cfg!(windows);
                dialog::checkbox(
                    ui,
                    at(o, 258.0, 49.0),
                    "Add Matcha to the Windows Explorer context menu",
                    &mut self.shell_menu,
                    available,
                );
                for (n, line) in [
                    "Right-click a file or folder for \"Matcha: Select Left ... for Compare\",",
                    "then \"Matcha: Compare to\" on the other one; select two items for",
                    "\"Matcha: Compare\". Windows 11 lists them under \"Show more options\".",
                ]
                .into_iter()
                .enumerate()
                {
                    dialog::label(
                        &painter,
                        at(o, 278.0, 78.0 + 20.0 * n as f32),
                        line,
                        available && self.shell_menu,
                    );
                }
            }
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
                self.editor_font(ui, o);
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
            "Next Difference" => {
                dialog::checkbox(
                    ui,
                    at(o, 258.0, 49.0),
                    "Wrap around to first difference",
                    &mut draft.next_diff.wrap_around,
                    true,
                );
                dialog::checkbox(
                    ui,
                    at(o, 258.0, 75.0),
                    "Show message panel",
                    &mut draft.next_diff.message_panel,
                    true,
                );
                for (n, line) in [
                    "Wrap: past the last difference, continue from the first one (and back).",
                    "Message panel: when unmarked, suppress the message panel (and beep)",
                    "when a difference is not found.",
                ]
                .into_iter()
                .enumerate()
                {
                    dialog::label(&painter, at(o, 258.0, 108.0 + 20.0 * n as f32), line, true);
                }
            }
            "Tweaks" => {
                dialog::label(&painter, at(o, 258.0, 46.0), "File views:", true);
                dialog::checkbox(
                    ui,
                    at(o, 274.0, 72.0),
                    "Check for files changed on disk",
                    &mut draft.disk_changes.check,
                    true,
                );
                let check = draft.disk_changes.check;
                dialog::checkbox(
                    ui,
                    at(o, 294.0, 98.0),
                    "Automatically reload unless changes will be discarded",
                    &mut draft.disk_changes.auto_reload,
                    check,
                );
                for (n, line) in [
                    "Checked when Matcha regains focus or a file view's tab is selected:",
                    "\"File changed on disk, reload?\" Text, table and hex compare views.",
                ]
                .into_iter()
                .enumerate()
                {
                    dialog::label(&painter, at(o, 274.0, 128.0 + 20.0 * n as f32), line, check);
                }
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
        let mut d = OptionsDialog {
            shell_apply: Some(|_| panic!("explorer menu unchanged")),
            ..Default::default()
        };
        d.show(&prefs, Placement::ClientOnly);
        assert_eq!(PAGES[d.page].0, "Appearance");
        // A new egui area takes input from its second frame on.
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        // Disabled pages cannot be selected (Tabs); Backups can.
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + ROW + 18.0)),
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
        // Editor font size is validated and written on OK.
        d.show(&prefs, Placement::ClientOnly);
        d.font_size = "99".into();
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(d.open && d.error.as_deref().is_some_and(|e| e.contains("Font size")));
        d.font_size = "12".into();
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(!d.open);
        assert_eq!(prefs.editor_font.size_pt, 12);
    }

    #[cfg(windows)]
    #[test]
    fn startup_page_toggles_the_explorer_menu_on_apply_only() {
        use std::sync::atomic::{AtomicU8, Ordering};
        // 0 = not called, 1 = off, 2 = on
        static CALLED: AtomicU8 = AtomicU8::new(0);
        let ctx = egui::Context::default();
        let mut prefs = Prefs::default();
        let mut d = OptionsDialog {
            shell_apply: Some(|on| {
                CALLED.store(if on { 2 } else { 1 }, Ordering::SeqCst);
                Ok(())
            }),
            shell_menu_saved: true,
            ..Default::default()
        };
        d.show(&prefs, Placement::ClientOnly);
        assert!(d.shell_menu);
        // A new egui area takes input from its second frame on.
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + 18.0)),
        );
        assert_eq!(PAGES[d.page].0, "Startup");
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        assert!(!d.shell_menu);
        assert_eq!(CALLED.load(Ordering::SeqCst), 0, "nothing until Apply");
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(800.0, 629.0)));
        assert_eq!(CALLED.load(Ordering::SeqCst), 1);
        assert!(d.open && !d.shell_menu_saved);
        // Cancel after toggling back on discards it.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(695.0, 629.0)));
        assert_eq!(CALLED.load(Ordering::SeqCst), 1);
        // Reopen shows the applied state; OK after toggling applies "on".
        d.show(&prefs, Placement::ClientOnly);
        assert!(!d.shell_menu);
        d.page = 0;
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert_eq!(CALLED.load(Ordering::SeqCst), 2);
        assert!(!d.open);
    }

    #[test]
    fn next_difference_page_edits_navigation_prefs() {
        let ctx = egui::Context::default();
        let mut prefs = Prefs::default();
        let mut d = OptionsDialog {
            shell_apply: Some(|_| panic!("explorer menu unchanged")),
            ..Default::default()
        };
        d.show(&prefs, Placement::ClientOnly);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        let page = PAGES
            .iter()
            .position(|(n, _)| *n == "Next Difference")
            .unwrap();
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + page as f32 * ROW + 18.0)),
        );
        assert_eq!(PAGES[d.page].0, "Next Difference");
        frame(&ctx, &mut d, &mut prefs, vec![]);
        let draft = d.draft.as_ref().unwrap().next_diff;
        assert!(draft.message_panel && !draft.wrap_around);
        // Wrap on, OK writes it and closes.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 55.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(!d.open);
        assert!(prefs.next_diff.wrap_around && prefs.next_diff.message_panel);
        // Panel off, then Cancel: nothing changes.
        d.show(&prefs, Placement::ClientOnly);
        d.page = page;
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(264.0, 81.0)));
        assert!(!d.draft.as_ref().unwrap().next_diff.message_panel);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(696.0, 629.0)));
        assert!(!d.open);
        assert!(prefs.next_diff.message_panel);
    }

    #[test]
    fn tweaks_page_edits_disk_change_prefs() {
        let ctx = egui::Context::default();
        let mut prefs = Prefs::default();
        let mut d = OptionsDialog {
            shell_apply: Some(|_| panic!("explorer menu unchanged")),
            ..Default::default()
        };
        d.show(&prefs, Placement::ClientOnly);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        frame(&ctx, &mut d, &mut prefs, vec![]);
        let tweaks = PAGES.iter().position(|(n, _)| *n == "Tweaks").unwrap();
        frame(
            &ctx,
            &mut d,
            &mut prefs,
            click(Pos2::new(100.0, FIRST_ROW + tweaks as f32 * ROW + 18.0)),
        );
        assert_eq!(PAGES[d.page].0, "Tweaks");
        frame(&ctx, &mut d, &mut prefs, vec![]);
        // Defaults: check on, auto-reload off.
        let draft = d.draft.as_ref().unwrap().disk_changes;
        assert!(draft.check && !draft.auto_reload);
        // Turn auto-reload on, Apply writes it.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(300.0, 104.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(800.0, 629.0)));
        assert!(prefs.disk_changes.auto_reload && prefs.disk_changes.check);
        // Turning the check off greys auto-reload: clicking it changes nothing.
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(280.0, 78.0)));
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(300.0, 104.0)));
        let draft = d.draft.as_ref().unwrap().disk_changes;
        assert!(!draft.check && draft.auto_reload);
        frame(&ctx, &mut d, &mut prefs, click(Pos2::new(590.0, 629.0)));
        assert!(!d.open);
        assert!(!prefs.disk_changes.check);
    }
}
