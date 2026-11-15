//! Sheet tabs of a workbook comparison: one tab per sheet pair, with a status dot
//! (gray = not compared yet, text color = same, difference color = differs).
use crate::chrome::sheet_tabs::{self, SheetTab};
use bc_engine_table::{compare::Importance, sheets::SheetPairs};
use bc_session::prefs::DiffColors;
use eframe::egui;

pub fn tabs(
    names: &[Vec<String>; 2],
    pairs: &SheetPairs,
    states: &[Option<Importance>],
    colors: &DiffColors,
) -> Vec<SheetTab> {
    pairs
        .0
        .iter()
        .enumerate()
        .map(|(i, pair)| {
            let left = pair
                .left
                .and_then(|n| names[0].get(n))
                .map_or("—", String::as_str);
            let right = pair
                .right
                .and_then(|n| names[1].get(n))
                .map_or("—", String::as_str);
            let label = if left == right {
                left.into()
            } else {
                format!("{left} ↔ {right}")
            };
            let dot = match states.get(i).copied().flatten() {
                None => crate::chrome::theme::TEXT_FAINT,
                Some(Importance::Same) => crate::chrome::theme::TEXT_MUTED,
                Some(s) => super::grid::color(colors, s, false, 255),
            };
            SheetTab {
                dot: Some(dot),
                ..SheetTab::new(label)
            }
        })
        .collect()
}

pub fn ui(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    names: &[Vec<String>; 2],
    pairs: &SheetPairs,
    states: &[Option<Importance>],
    active: usize,
    colors: &DiffColors,
) -> Option<usize> {
    sheet_tabs::show(ui, rect, &tabs(names, pairs, states, colors), active)
}
