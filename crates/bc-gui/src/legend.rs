//! Folder colors and their explanation; no engine depends on egui.
use crate::egui;
use bc_engine_dir::CompareStatus;
use bc_session::prefs::FolderColors;
pub fn color(rgb: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
pub fn side_color(
    palette: FolderColors,
    status: CompareStatus,
    side: usize,
    teal: bool,
) -> egui::Color32 {
    use CompareStatus::*;
    color(if teal {
        palette.filtered
    } else {
        match status {
            Same => palette.same,
            LeftOrphan | RightOrphan => palette.orphan,
            LeftNewer if side == 1 => palette.unknown,
            RightNewer if side == 0 => palette.unknown,
            Different | LeftNewer | RightNewer | SimilarUnimportant | Error => palette.different,
            Unknown => palette.unknown,
        }
    })
}
pub fn show(ctx: &egui::Context, open: &mut bool, palette: FolderColors) {
    crate::chrome::window::Window::new("Folder legend")
        .open(open)
        .show(ctx, |ui| {
            for (rgb, label) in [
                (palette.unknown, "Unknown / older"),
                (palette.same, "Same"),
                (palette.orphan, "Orphan"),
                (palette.different, "Newer / different"),
                (palette.filtered, "Filtered but shown"),
                (palette.unscanned, "Unscanned; automatic scan disabled"),
            ] {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, crate::chrome::theme::RADIUS_SM, color(rgb));
                    ui.label(label);
                });
            }
            ui.label("Hollow folder: scan pending. Red + purple markers: mixed descendants.");
            ui.label("Center: . not content-tested; = same, != different, ~ unimportant, ! error.");
        });
}
#[cfg(test)]
mod tests {
    #[test]
    fn older_and_filtered_are_not_newer() {
        let p = bc_session::prefs::FolderColors::default();
        assert_eq!(
            super::side_color(p, bc_engine_dir::CompareStatus::LeftNewer, 1, false),
            super::color(p.unknown)
        );
        assert_eq!(
            super::side_color(p, bc_engine_dir::CompareStatus::Same, 0, true),
            super::color(p.filtered)
        );
    }
}
