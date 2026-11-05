//! 固定数量的缩略桶；滚动帧不遍历全文。
use bc_core::DiffKind;
use bc_engine_text::{importance::Importance, TextMap};
use bc_session::prefs::DiffColors;
use eframe::egui;

#[derive(Debug, Clone, Copy, Default)]
pub struct Mark {
    importance: Importance,
    orphan: bool,
}
#[derive(Default)]
pub struct NavBar {
    bins: Vec<Mark>,
    total: usize,
}
pub fn color(
    colors: &DiffColors,
    importance: Importance,
    orphan: bool,
    alpha: u8,
) -> egui::Color32 {
    let [r, g, b] = if orphan {
        colors.inserted
    } else if importance == Importance::Unimportant {
        colors.unimportant
    } else {
        colors.important
    };
    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
}
impl NavBar {
    pub fn rebuild(&mut self, map: &TextMap) {
        self.total = map.row_count();
        self.bins = vec![Mark::default(); 512];
        for (i, row) in map.rows().enumerate() {
            let importance = row
                .importance
                .with_ignore_unimportant(map.rules.ignore_unimportant);
            if importance == Importance::Same {
                continue;
            }
            let bin = &mut self.bins[i * 512 / self.total.max(1)];
            if importance >= bin.importance {
                *bin = Mark {
                    importance,
                    orphan: matches!(row.kind, DiffKind::LeftOnly | DiffKind::RightOnly),
                };
            }
        }
    }
    pub fn target(fraction: f32, total: usize) -> usize {
        ((fraction.clamp(0.0, 1.0) * total as f32) as usize).min(total.saturating_sub(1))
    }
    pub fn ui(
        &self,
        ui: &mut egui::Ui,
        colors: &DiffColors,
        visible: std::ops::Range<usize>,
        current: usize,
    ) -> Option<usize> {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(22.0, ui.available_height()),
            egui::Sense::click_and_drag(),
        );
        ui.painter()
            .rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
        for (i, bin) in self
            .bins
            .iter()
            .enumerate()
            .filter(|(_, b)| b.importance != Importance::Same)
        {
            let y = rect.top() + i as f32 / 512.0 * rect.height();
            let band = egui::Rect::from_min_size(
                egui::pos2(rect.left(), y),
                egui::vec2(rect.width(), (rect.height() / 512.0).max(1.0)),
            );
            ui.painter()
                .rect_filled(band, 0.0, color(colors, bin.importance, bin.orphan, 220));
        }
        let y = |r: usize| rect.top() + r as f32 / self.total.max(1) as f32 * rect.height();
        let viewport = egui::Rect::from_min_max(
            egui::pos2(rect.left(), y(visible.start)),
            egui::pos2(rect.right(), y(visible.end).max(y(visible.start) + 3.0)),
        );
        ui.painter().rect_stroke(
            viewport,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), y(current)),
                egui::pos2(rect.right(), y(current)),
            ],
            egui::Stroke::new(2.0, egui::Color32::YELLOW),
        );
        if response.clicked() || response.dragged() {
            response
                .interact_pointer_pos()
                .map(|p| Self::target((p.y - rect.top()) / rect.height(), self.total))
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn clicking_seventy_percent_maps_to_seventy_percent() {
        assert_eq!(super::NavBar::target(0.7, 1000), 700);
        assert_eq!(super::NavBar::target(1.0, 1000), 999);
        assert_eq!(super::NavBar::target(0.7, 0), 0);
    }
}
