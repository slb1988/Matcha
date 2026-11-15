//! Session Settings dialog for the text view (Session > Session Settings).
//!
//! Classic tabbed dialog (Specs / Format / Importance / Alignment) laid out
//! like the reference (`dlg-text-session-settings.png`). It edits a draft
//! of the view's session state; OK validates and returns the draft for the
//! view to apply, Cancel / × / Esc discard it. Only settings Matcha actually
//! has are live; reference controls without a Matcha counterpart (spec copy
//! button, session description, session-default scopes) are disabled
//! placeholders.
use crate::chrome::{
    dialog::{self, ButtonKind, Placement},
    theme, widgets,
};
use bc_core::{
    align::AlignStrategy,
    encoding::{BomPolicy, Encoding},
};
use bc_engine_text::{importance::CompiledComparison, rules::TextRules};
use bc_formats::{
    text_rules::{Replacement, UnimportantText},
    ElementKind,
};
use eframe::egui::{self, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// Client size of the reference dialog.
pub const CLIENT: Vec2 = Vec2::new(583.0, 563.0);
pub const TABS: [&str; 4] = ["Specs", "Format", "Importance", "Alignment"];
pub const ENCODINGS: [(&str, Encoding); 5] = [
    ("UTF-8", Encoding::Utf8),
    ("UTF-16 LE", Encoding::Utf16Le),
    ("UTF-16 BE", Encoding::Utf16Be),
    ("GBK (936)", Encoding::CodePage(936)),
    ("Windows-1252", Encoding::CodePage(1252)),
];
const BOM: [(&str, BomPolicy); 3] = [
    ("Preserve", BomPolicy::Preserve),
    ("Always", BomPolicy::Always),
    ("Never", BomPolicy::Never),
];
const STRATEGIES: [(&str, AlignStrategy); 4] = [
    ("Standard", AlignStrategy::Smart),
    ("Myers O(ND)", AlignStrategy::Quick),
    ("Patience", AlignStrategy::Patience),
    ("Unaligned", AlignStrategy::None),
];

/// Per-side format selection.
#[derive(Clone, Debug, PartialEq)]
pub enum FormatChoice {
    /// Keep the current format.
    Keep,
    /// Re-resolve from the file mask.
    Detected,
    Plain,
    Named(String),
}

/// Everything the dialog edits; produced from and applied to the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub rules: TextRules,
    pub context: usize,
    pub paths: [String; 2],
    pub read_only: [bool; 2],
    pub format: [FormatChoice; 2],
    /// Name of the current format per side (display only).
    pub format_names: [String; 2],
    pub encoding: [Encoding; 2],
    pub bom: [BomPolicy; 2],
}

#[derive(Default)]
pub struct SessionDialog {
    pub open: bool,
    pub tab: usize,
    pub placement: Option<Placement>,
    draft: Option<Draft>,
    context_text: String,
    skew_text: String,
    error: Option<String>,
}

fn at(o: Pos2, x: f32, y: f32) -> Pos2 {
    o + Vec2::new(x, y)
}
fn span(o: Pos2, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(at(o, x0, y0), at(o, x1, y1))
}

impl SessionDialog {
    pub fn show(&mut self, draft: Draft, placement: Placement) {
        self.context_text = draft.context.to_string();
        self.skew_text = draft.rules.alignment.skew_tolerance.to_string();
        self.draft = Some(draft);
        self.placement = Some(placement);
        self.error = None;
        self.open = true;
    }
    pub fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }
    pub fn draft_mut(&mut self) -> Option<&mut Draft> {
        self.draft.as_mut()
    }
    fn close(&mut self) {
        self.open = false;
        self.draft = None;
    }
    /// Validate the draft for OK.
    fn validated(&mut self) -> Result<Draft, String> {
        let mut draft = self.draft.clone().ok_or("no draft")?;
        draft.context = self
            .context_text
            .trim()
            .parse()
            .map_err(|_| "Context lines must be a whole number".to_string())?;
        draft.rules.alignment.skew_tolerance = self
            .skew_text
            .trim()
            .parse()
            .map_err(|_| "Skew tolerance must be a whole number".to_string())?;
        CompiledComparison::new(&draft.rules.comparison).map_err(|e| e.to_string())?;
        Ok(draft)
    }
    /// Draw the dialog; returns the validated draft when OK is pressed.
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        format_names: &[String],
        recent: &[String],
    ) -> Option<Draft> {
        if !self.open || self.draft.is_none() {
            return None;
        }
        let placement = self.placement.unwrap_or(Placement::Window);
        let mut ok = false;
        let mut cancel = false;
        let (closed, ()) = dialog::window(
            ctx,
            egui::Id::unique("session-settings-dialog"),
            "Session Settings - Text Compare",
            CLIENT,
            placement,
            |ui, client| {
                let o = client.min;
                let page = span(o, 8.0, 32.0, 571.0, 511.0);
                if let Some(tab) = dialog::tabs(ui, page, &TABS, self.tab) {
                    self.tab = tab;
                }
                match self.tab {
                    0 => self.specs(ui, o, recent),
                    1 => self.format(ui, o, format_names),
                    2 => self.importance(ui, o, page),
                    _ => self.alignment(ui, o),
                }
                // Bottom row: scope combo, OK, Cancel, size grip.
                dialog::combo(
                    ui,
                    span(o, 10.0, 525.0, 197.0, 550.0),
                    egui::Id::unique("session-scope"),
                    "Use for this view only",
                    &["Use for this view only".to_string()],
                    true,
                );
                if let Some(error) = &self.error {
                    ui.painter().text(
                        at(o, 205.0, 537.0),
                        egui::Align2::LEFT_CENTER,
                        error,
                        dialog::font(),
                        theme::palette(ui.ctx()).danger,
                    );
                }
                ok = dialog::button(
                    ui,
                    span(o, 373.0, 523.0, 467.0, 552.0),
                    "OK",
                    ButtonKind::Default,
                    true,
                );
                cancel = dialog::button(
                    ui,
                    span(o, 478.0, 523.0, 572.0, 552.0),
                    "Cancel",
                    ButtonKind::Normal,
                    true,
                );
            },
        );
        if closed || cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return None;
        }
        if ok {
            match self.validated() {
                Ok(draft) => {
                    self.close();
                    return Some(draft);
                }
                Err(e) => self.error = Some(e),
            }
        }
        None
    }
    fn specs(&mut self, ui: &mut egui::Ui, o: Pos2, recent: &[String]) {
        let painter = ui.painter().clone();
        let draft = self.draft.as_mut().unwrap();
        for s in 0..2 {
            let dy = s as f32 * 82.0;
            dialog::label(
                &painter,
                at(o, 22.0, 46.0 + dy),
                if s == 0 { "Left file:" } else { "Right file:" },
                true,
            );
            let field = span(o, 22.0, 69.0 + dy, 498.0, 94.0 + dy);
            dialog::text_field(
                ui,
                Rect::from_min_max(field.min, field.max - Vec2::new(16.0, 0.0)),
                egui::Id::unique(("session-path", s)),
                &mut draft.paths[s],
                true,
            );
            painter.rect_stroke(
                field,
                theme::RADIUS_MD,
                Stroke::new(1.0, theme::palette(ui.ctx()).line_strong),
                StrokeKind::Inside,
            );
            dialog::combo_chevron(
                &painter,
                at(o, 482.0, 80.0 + dy),
                theme::palette(ui.ctx()).text_muted,
            );
            // "..." browse: Matcha's open prompt is the path field itself.
            dialog::button(
                ui,
                span(o, 509.0, 70.0 + dy, 537.0, 97.0 + dy),
                "...",
                ButtonKind::Normal,
                false,
            );
            // History of recently opened paths.
            let history = span(o, 539.0, 70.0 + dy, 558.0, 97.0 + dy);
            if let Some(i) = dialog::dropdown_button(
                ui,
                history,
                egui::Id::unique(("session-history", s)),
                recent,
                !recent.is_empty(),
            ) {
                draft.paths[s] = recent[i].clone();
            }
            dialog::checkbox(
                ui,
                at(o, 22.0, 106.0 + dy),
                "Disable editing",
                &mut draft.read_only[s],
                true,
            );
        }
        // Copy-spec (no Matcha counterpart) and swap sides.
        dialog::glyph_button(
            ui,
            span(o, 255.0, 102.0, 284.0, 130.0),
            dialog::Glyph::Down,
            false,
        );
        if dialog::glyph_button(
            ui,
            span(o, 296.0, 102.0, 326.0, 130.0),
            dialog::Glyph::Swap,
            true,
        ) {
            draft.paths.swap(0, 1);
            draft.read_only.swap(0, 1);
        }
        dialog::label(&painter, at(o, 22.0, 211.0), "Description:", true);
        // Session descriptions are not stored by Matcha: disabled box.
        let desc = span(o, 22.0, 233.0, 558.0, 471.0);
        let p = theme::palette(ui.ctx());
        painter.rect_filled(desc, theme::RADIUS_MD, p.surface_subtle);
        painter.rect_stroke(
            desc,
            theme::RADIUS_MD,
            Stroke::new(1.0, p.line),
            StrokeKind::Inside,
        );
    }
    fn format(&mut self, ui: &mut egui::Ui, o: Pos2, format_names: &[String]) {
        let painter = ui.painter().clone();
        let draft = self.draft.as_mut().unwrap();
        let mut choices = vec!["Detected".to_string(), "Plain text".to_string()];
        choices.extend(format_names.iter().cloned());
        for s in 0..2 {
            let dy = s as f32 * 150.0;
            dialog::label(
                &painter,
                at(o, 22.0, 46.0 + dy),
                if s == 0 { "Left file" } else { "Right file" },
                true,
            );
            dialog::rule(
                &painter,
                o.x + 90.0,
                o.x + 556.0,
                o.y + 54.0 + dy,
                theme::palette(ui.ctx()).line,
            );
            let shown = match &draft.format[s] {
                FormatChoice::Keep => draft.format_names[s].clone(),
                FormatChoice::Detected => "Detected".into(),
                FormatChoice::Plain => "Plain text".into(),
                FormatChoice::Named(n) => n.clone(),
            };
            let rows: [(&str, f32); 3] = [
                ("Format:", 72.0),
                ("Encoding:", 104.0),
                ("Save BOM:", 136.0),
            ];
            for (label, y) in rows {
                dialog::label(&painter, at(o, 36.0, y + dy + 5.0), label, true);
            }
            if let Some(i) = dialog::combo(
                ui,
                span(o, 130.0, 72.0 + dy, 400.0, 97.0 + dy),
                egui::Id::unique(("session-format", s)),
                &shown,
                &choices,
                true,
            ) {
                draft.format[s] = match i {
                    0 => FormatChoice::Detected,
                    1 => FormatChoice::Plain,
                    n => FormatChoice::Named(choices[n].clone()),
                };
            }
            let encoding = ENCODINGS
                .iter()
                .find(|(_, e)| *e == draft.encoding[s])
                .map_or_else(
                    || format!("{:?}", draft.encoding[s]),
                    |(n, _)| n.to_string(),
                );
            if let Some(i) = dialog::combo(
                ui,
                span(o, 130.0, 104.0 + dy, 400.0, 129.0 + dy),
                egui::Id::unique(("session-encoding", s)),
                &encoding,
                &ENCODINGS
                    .iter()
                    .map(|(n, _)| n.to_string())
                    .collect::<Vec<_>>(),
                true,
            ) {
                draft.encoding[s] = ENCODINGS[i].1;
            }
            let bom = BOM
                .iter()
                .find(|(_, b)| *b == draft.bom[s])
                .map_or("Preserve", |(n, _)| n);
            if let Some(i) = dialog::combo(
                ui,
                span(o, 130.0, 136.0 + dy, 400.0, 161.0 + dy),
                egui::Id::unique(("session-bom", s)),
                bom,
                &BOM.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
                true,
            ) {
                draft.bom[s] = BOM[i].1;
            }
        }
    }
    fn importance(&mut self, ui: &mut egui::Ui, o: Pos2, page: Rect) {
        let painter = ui.painter().clone();
        let draft = self.draft.as_mut().unwrap();
        let i = &mut draft.rules.comparison.importance;
        let checks: [(&str, &mut bool); 5] = [
            (
                "Leading whitespace is unimportant",
                &mut i.leading_whitespace_unimportant,
            ),
            (
                "Embedded whitespace is unimportant",
                &mut i.embedded_whitespace_unimportant,
            ),
            (
                "Trailing whitespace is unimportant",
                &mut i.trailing_whitespace_unimportant,
            ),
            ("Character case is unimportant", &mut i.case_unimportant),
            (
                "Line endings are unimportant",
                &mut i.line_endings_unimportant,
            ),
        ];
        for (n, (label, value)) in checks.into_iter().enumerate() {
            dialog::checkbox(ui, at(o, 22.0, 48.0 + 22.0 * n as f32), label, value, true);
        }
        dialog::label(
            &painter,
            at(o, 300.0, 46.0),
            "Grammar elements (checked = important):",
            true,
        );
        for (n, kind) in ElementKind::ALL.into_iter().enumerate() {
            let mut important = !i.unimportant.contains(&kind);
            let pos = at(
                o,
                300.0 + 130.0 * (n % 2) as f32,
                68.0 + 22.0 * (n / 2) as f32,
            );
            if dialog::checkbox(ui, pos, kind.as_str(), &mut important, true) {
                i.unimportant.retain(|e| *e != kind);
                if !important {
                    i.unimportant.push(kind);
                }
            }
        }
        dialog::checkbox(
            ui,
            at(o, 22.0, 166.0),
            "Ignore unimportant differences",
            &mut draft.rules.ignore_unimportant,
            true,
        );
        dialog::label(&painter, at(o, 22.0, 194.0), "Context lines:", true);
        dialog::text_field(
            ui,
            span(o, 110.0, 190.0, 170.0, 213.0),
            egui::Id::unique("session-context"),
            &mut self.context_text,
            true,
        );
        // Unimportant text and replacements: compact rows in a white list.
        let list = Rect::from_min_max(
            at(o, 22.0, 286.0),
            Pos2::new(page.right() - 14.0, page.bottom() - 14.0),
        );
        dialog::label(
            &painter,
            at(o, 22.0, 264.0),
            "Unimportant text and replacements:",
            true,
        );
        let p = theme::palette(ui.ctx());
        painter.rect_filled(list, theme::RADIUS_MD, p.surface);
        painter.rect_stroke(
            list,
            theme::RADIUS_MD,
            Stroke::new(1.0, p.line_strong),
            StrokeKind::Inside,
        );
        let comparison = &mut draft.rules.comparison;
        ui.scope_builder(egui::UiBuilder::new().max_rect(list.shrink(4.0)), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("session-importance-lists")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut remove = None;
                    for (n, r) in comparison.unimportant_text.iter_mut().enumerate() {
                        ui.push_id(("unimportant", n), |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Unimportant");
                                ui.add(widgets::field(&mut r.pattern).desired_width(150.0));
                                widgets::checkbox(ui, &mut r.is_regexp, "Regex");
                                widgets::checkbox(ui, &mut r.case_sensitive, "Case");
                                widgets::Combo::new(
                                    "element",
                                    r.only_in.map_or("Any element", |k| k.as_str()),
                                )
                                .width(110.0)
                                .show(ui, |list| {
                                    list.selectable_value(&mut r.only_in, None, "Any element");
                                    for kind in ElementKind::ALL {
                                        list.selectable_value(
                                            &mut r.only_in,
                                            Some(kind),
                                            kind.as_str(),
                                        );
                                    }
                                });
                                if ui.add(widgets::Button::new("Remove").danger()).clicked() {
                                    remove = Some(n);
                                }
                            });
                        });
                    }
                    if let Some(n) = remove {
                        comparison.unimportant_text.remove(n);
                    }
                    let mut remove = None;
                    for (n, r) in comparison.replacements.iter_mut().enumerate() {
                        ui.push_id(("replacement", n), |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Replace");
                                ui.add(widgets::field(&mut r.from).desired_width(90.0));
                                ui.label("with");
                                ui.add(widgets::field(&mut r.to).desired_width(90.0));
                                widgets::checkbox(ui, &mut r.is_regexp, "Regex");
                                widgets::checkbox(ui, &mut r.case_sensitive, "Case");
                                widgets::checkbox(ui, &mut r.whole_words, "Words");
                                widgets::checkbox(ui, &mut r.bidirectional, "Both");
                                if ui.add(widgets::Button::new("Remove").danger()).clicked() {
                                    remove = Some(n);
                                }
                            });
                        });
                    }
                    if let Some(n) = remove {
                        comparison.replacements.remove(n);
                    }
                });
        });
        if dialog::button(
            ui,
            span(o, 340.0, 258.0, 448.0, 282.0),
            "Add unimportant",
            ButtonKind::Normal,
            true,
        ) {
            comparison.unimportant_text.push(UnimportantText {
                pattern: String::new(),
                is_regexp: false,
                case_sensitive: true,
                only_in: None,
            });
        }
        if dialog::button(
            ui,
            span(o, 452.0, 258.0, 556.0, 282.0),
            "Add replacement",
            ButtonKind::Normal,
            true,
        ) {
            comparison.replacements.push(Replacement {
                from: String::new(),
                to: String::new(),
                is_regexp: false,
                case_sensitive: true,
                whole_words: false,
                bidirectional: false,
            });
        }
    }
    fn alignment(&mut self, ui: &mut egui::Ui, o: Pos2) {
        let painter = ui.painter().clone();
        let draft = self.draft.as_mut().unwrap();
        let a = &mut draft.rules.alignment;
        dialog::label(&painter, at(o, 22.0, 46.0), "Alignment method:", true);
        for (n, (label, strategy)) in STRATEGIES.into_iter().enumerate() {
            if dialog::radio(
                ui,
                at(o, 36.0, 70.0 + 24.0 * n as f32),
                label,
                a.strategy == strategy,
                true,
            ) {
                a.strategy = strategy;
            }
        }
        let smart = a.strategy == AlignStrategy::Smart;
        dialog::checkbox(
            ui,
            at(o, 22.0, 178.0),
            "Use closeness matching",
            &mut a.use_closeness,
            smart,
        );
        dialog::checkbox(
            ui,
            at(o, 22.0, 202.0),
            "Never align differences",
            &mut a.never_align_differences,
            true,
        );
        dialog::label(&painter, at(o, 22.0, 232.0), "Skew tolerance:", true);
        dialog::text_field(
            ui,
            span(o, 120.0, 228.0, 200.0, 251.0),
            egui::Id::unique("session-skew"),
            &mut self.skew_text,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn draft() -> Draft {
        Draft {
            rules: TextRules::default(),
            context: 3,
            paths: ["left.txt".into(), "right.txt".into()],
            read_only: [false, false],
            format: [FormatChoice::Keep, FormatChoice::Keep],
            format_names: ["Everything Else".into(), "Everything Else".into()],
            encoding: [Encoding::Utf8, Encoding::Utf8],
            bom: [BomPolicy::Preserve, BomPolicy::Preserve],
        }
    }
    fn frame(
        ctx: &egui::Context,
        d: &mut SessionDialog,
        events: Vec<egui::Event>,
    ) -> Option<Draft> {
        let mut out = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, CLIENT)),
                events,
                ..Default::default()
            },
            |ui| out = d.ui(ui.ctx(), &[], &[]),
        )
        .drop_without_applying_deltas();
        out
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
    fn ok_returns_the_edited_draft_and_cancel_discards_it() {
        let ctx = egui::Context::default();
        let mut d = SessionDialog::default();
        d.show(draft(), Placement::ClientOnly);
        frame(&ctx, &mut d, vec![]);
        frame(&ctx, &mut d, vec![]);
        // Specs: "Disable editing" on the left; swap sides.
        frame(&ctx, &mut d, click(Pos2::new(28.0, 112.0)));
        frame(&ctx, &mut d, click(Pos2::new(311.0, 116.0)));
        // Importance tab: case unimportant.
        frame(&ctx, &mut d, click(Pos2::new(150.0, 20.0)));
        frame(&ctx, &mut d, vec![]);
        frame(&ctx, &mut d, click(Pos2::new(28.0, 48.0 + 66.0 + 6.0)));
        // Alignment tab: Patience.
        frame(&ctx, &mut d, click(Pos2::new(222.0, 20.0)));
        frame(&ctx, &mut d, vec![]);
        frame(&ctx, &mut d, click(Pos2::new(42.0, 70.0 + 48.0 + 6.0)));
        let out =
            frame(&ctx, &mut d, click(Pos2::new(420.0, 537.0))).expect("OK returns the draft");
        assert_eq!(out.paths, ["right.txt", "left.txt"]);
        assert_eq!(out.read_only, [false, true]);
        assert!(out.rules.comparison.importance.case_unimportant);
        assert_eq!(out.rules.alignment.strategy, AlignStrategy::Patience);
        assert!(!d.open);
        // Cancel discards.
        d.show(draft(), Placement::ClientOnly);
        frame(&ctx, &mut d, vec![]);
        frame(&ctx, &mut d, click(Pos2::new(28.0, 112.0)));
        assert!(frame(&ctx, &mut d, click(Pos2::new(525.0, 537.0))).is_none());
        assert!(!d.open && d.draft().is_none());
    }
    #[test]
    fn invalid_numbers_keep_the_dialog_open_with_an_error() {
        let ctx = egui::Context::default();
        let mut d = SessionDialog::default();
        d.show(draft(), Placement::ClientOnly);
        d.context_text = "many".into();
        frame(&ctx, &mut d, vec![]);
        frame(&ctx, &mut d, vec![]);
        assert!(frame(&ctx, &mut d, click(Pos2::new(420.0, 537.0))).is_none());
        assert!(d.open);
        assert!(d.error.as_deref().unwrap().contains("Context lines"));
    }
}
