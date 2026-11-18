//! Segmented status bar with fixed and flexible fields.

use crate::chrome::icons::{self, Icon};
use eframe::egui::{self, Stroke};

#[derive(Clone, Debug, PartialEq)]
pub struct StatusField {
    pub text: String,
    pub width: Option<f32>,
    pub icon: Option<Icon>,
    pub fixed: bool,
}

impl StatusField {
    pub fn fixed(text: impl Into<String>, width: f32) -> Self {
        Self {
            text: text.into(),
            width: Some(width),
            icon: None,
            fixed: true,
        }
    }

    pub fn flexible(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            width: None,
            icon: None,
            fixed: false,
        }
    }
}

/// Draw recessed fields. Fixed widths stay stable while `None` fields stretch.
///
/// The bar is at most [`STATUS_HEIGHT`](super::theme::STATUS_HEIGHT) tall and
/// never taller than the slot it is given (text/table/merge views reserve the
/// reference's 25 px): content that pokes past the slot makes a resizable
/// host window grow to fit it, a pixel more every frame.
pub fn show(ui: &mut egui::Ui, fields: &[StatusField]) {
    let widths = field_widths(fields, ui.available_width(), ui.spacing().item_spacing.x);
    let height = super::theme::STATUS_HEIGHT
        .min(ui.available_height())
        .max(0.0);
    ui.horizontal(|ui| {
        for (index, field) in fields.iter().enumerate() {
            let width = widths[index];
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(width, height),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 0.0, super::theme::SURFACE_CHROME);
            let mut x = rect.left() + 8.0;
            if let Some(icon) = field.icon {
                let icon_rect = egui::Rect::from_min_size(
                    egui::pos2(x, rect.center().y - 8.0),
                    egui::vec2(16.0, 16.0),
                );
                icons::paint(ui.painter(), icon_rect, icon);
                x += 20.0;
            }
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &field.text,
                egui::FontId::proportional(super::theme::UI_SIZE - 1.0),
                super::theme::TEXT_MUTED,
            );
            if index + 1 < fields.len() && field.fixed {
                ui.painter().vline(
                    rect.right() - 0.5,
                    rect.top() + 6.0..=rect.bottom() - 6.0,
                    Stroke::new(1.0, super::theme::LINE),
                );
            }
        }
    });
}

fn field_widths(fields: &[StatusField], available: f32, spacing: f32) -> Vec<f32> {
    let fixed: f32 = fields
        .iter()
        .filter(|f| f.fixed)
        .map(|f| f.width.unwrap_or(70.0))
        .sum();
    let count = fields.iter().filter(|f| !f.fixed).count();
    let flex = ((available - fixed - fields.len().saturating_sub(1) as f32 * spacing)
        / count.max(1) as f32)
        .max(0.0);
    fields
        .iter()
        .map(|f| {
            if f.fixed {
                f.width.unwrap_or(70.0)
            } else {
                flex
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_and_flexible_fields_are_distinguishable() {
        assert!(StatusField::fixed("x", 20.0).fixed);
        assert!(!StatusField::flexible("x").fixed);
        assert_eq!(
            field_widths(
                &[
                    StatusField::fixed("x", 100.0),
                    StatusField::flexible("y"),
                    StatusField::flexible("z")
                ],
                508.0,
                4.0
            ),
            vec![100.0, 200.0, 200.0]
        );
    }
}
