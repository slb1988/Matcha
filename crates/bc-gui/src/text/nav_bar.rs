//! 左侧缩略图：两列（左/右文件）每行 1 px，超出可用高度时按像素分桶；
//! 视口框、当前行三角与参考一致。差异按行程存储，像素桶按轨道高度缓存，
//! 滚动帧不遍历全文。
use bc_core::DiffKind;
use bc_engine_text::{importance::Importance, TextMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
use std::ops::Range;

/// Difference mark of one side of a row (`Same` = unmarked).
type Mark = Importance;

/// Strip geometry measured on the reference (offsets from the strip's
/// top-left; the strip is 52 px wide and starts at the path-bar row).
const TRACK_LEFT: f32 = 11.0;
const TRACK_TOP: f32 = 12.0;
const CONTENT_LEFT: f32 = 16.0;
const COLUMN: f32 = 12.0;
const TRACK_RIGHT: f32 = 40.0;
const FRAME_LEFT: f32 = 7.0;
const FRAME_RIGHT: f32 = 44.0;
const POINTER: egui::Color32 = egui::Color32::from_rgb(0x00, 0x78, 0xD7);
const FRAME: egui::Color32 = egui::Color32::from_rgb(0x80, 0x80, 0x80);
const TRACK_EDGE: egui::Color32 = egui::Color32::from_rgb(0xA0, 0xA0, 0xA0);

#[derive(Default)]
pub struct NavBar {
    /// Runs of consecutive rows with identical non-same marks.
    runs: Vec<(Range<usize>, [Mark; 2])>,
    /// Map rows plus the virtual EOF row.
    total: usize,
    /// Per-pixel marks for the last track height (`(height, marks)`).
    pixels: Option<(usize, Vec<[Mark; 2]>)>,
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
fn lerp(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}
/// Thumbnail ink: the importance color at ~75% over white (reference
/// #FF4141 / #4141FF for the default red / blue); orphans use the
/// important color like the reference.
fn ink(colors: &DiffColors, mark: Mark) -> egui::Color32 {
    let [r, g, b] = if mark == Importance::Unimportant {
        colors.unimportant
    } else {
        colors.important
    };
    lerp(
        egui::Color32::from_rgb(r, g, b),
        egui::Color32::WHITE,
        0.255,
    )
}
/// Rows outside the viewport are veiled 20% toward the panel gray.
fn veil(color: egui::Color32) -> egui::Color32 {
    lerp(color, crate::chrome::theme::PANEL_BG, 0.2)
}
impl NavBar {
    pub fn rebuild(&mut self, map: &TextMap) {
        self.total = map.row_count() + 1;
        self.runs.clear();
        self.pixels = None;
        for (i, row) in map.rows().enumerate() {
            let importance = row
                .importance
                .with_ignore_unimportant(map.rules.ignore_unimportant);
            if importance == Importance::Same || row.kind == DiffKind::Same {
                continue;
            }
            let side = |present: bool| {
                if present {
                    importance
                } else {
                    Importance::Same
                }
            };
            let marks = [side(row.left.is_some()), side(row.right.is_some())];
            match self.runs.last_mut() {
                Some((rows, last)) if rows.end == i && *last == marks => rows.end = i + 1,
                _ => self.runs.push((i..i + 1, marks)),
            }
        }
    }
    pub fn target(fraction: f32, total: usize) -> usize {
        ((fraction.clamp(0.0, 1.0) * total as f32) as usize).min(total.saturating_sub(1))
    }
    /// Track height in pixels: one per row while they fit.
    fn track_height(&self, available: f32) -> usize {
        self.total.min(available.max(1.0) as usize).max(1)
    }
    /// Most important mark per side for each pixel row of the track.
    fn pixels(&mut self, height: usize) -> &[[Mark; 2]] {
        if self.pixels.as_ref().map(|(h, _)| *h) != Some(height) {
            let mut pixels = vec![[Importance::Same; 2]; height];
            let total = self.total.max(1);
            for (rows, marks) in &self.runs {
                let first = rows.start * height / total;
                let last = ((rows.end - 1) * height / total).min(height - 1);
                for pixel in &mut pixels[first..=last] {
                    for side in 0..2 {
                        pixel[side] = pixel[side].max(marks[side]);
                    }
                }
            }
            self.pixels = Some((height, pixels));
        }
        &self.pixels.as_ref().unwrap().1
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        colors: &DiffColors,
        visible: std::ops::Range<usize>,
        current: usize,
    ) -> Option<usize> {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(52.0, ui.available_height()),
            egui::Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
        // The path-bar row's white top highlight runs across the strip too.
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 1.0)),
            0.0,
            egui::Color32::WHITE,
        );
        let left = rect.left();
        let content_top = rect.top() + TRACK_TOP + 1.0;
        let height = self.track_height(rect.bottom() - content_top - 2.0);
        let total = self.total.max(1);
        let y = |row: usize| content_top + (row.min(total) * height / total) as f32;
        let x = |dx: f32| left + dx;
        let span = |x0: f32, x1: f32, y0: f32, y1: f32| {
            egui::Rect::from_min_max(egui::pos2(x(x0), y0), egui::pos2(x(x1), y1))
        };
        let bottom = content_top + height as f32;
        // Sunken track: gray top/left edge, white bottom/right edge.
        painter.rect_filled(
            span(TRACK_LEFT, TRACK_RIGHT, content_top - 1.0, content_top),
            0.0,
            TRACK_EDGE,
        );
        painter.rect_filled(
            span(TRACK_LEFT, TRACK_LEFT + 1.0, content_top, bottom),
            0.0,
            TRACK_EDGE,
        );
        painter.rect_filled(
            span(
                TRACK_RIGHT,
                TRACK_RIGHT + 1.0,
                content_top - 1.0,
                bottom + 1.0,
            ),
            0.0,
            egui::Color32::WHITE,
        );
        painter.rect_filled(
            span(TRACK_LEFT, TRACK_RIGHT, bottom, bottom + 1.0),
            0.0,
            egui::Color32::WHITE,
        );
        // Content: veiled outside the viewport, plain inside it.
        let view_top = y(visible.start);
        let view_bottom = y(visible.end.max(visible.start + 1)).max(view_top + 1.0);
        let content = span(CONTENT_LEFT, TRACK_RIGHT, content_top, bottom);
        painter.rect_filled(content, 0.0, veil(egui::Color32::WHITE));
        painter.rect_filled(
            span(CONTENT_LEFT, TRACK_RIGHT, view_top, view_bottom.min(bottom)),
            0.0,
            egui::Color32::WHITE,
        );
        let pixels = self.pixels(height).to_vec();
        for side in 0..2 {
            let x0 = CONTENT_LEFT + side as f32 * COLUMN;
            let mut start = 0;
            // Merge equal consecutive pixels into one rect per run.
            for p in 1..=pixels.len() {
                if p < pixels.len() && pixels[p][side] == pixels[start][side] {
                    continue;
                }
                let mark = pixels[start][side];
                if mark != Importance::Same {
                    let (y0, y1) = (content_top + start as f32, content_top + p as f32);
                    let full = ink(colors, mark);
                    // Split at the viewport edges so both veils are exact.
                    for (a, b, c) in [
                        (y0, y1.min(view_top), veil(full)),
                        (y0.max(view_top), y1.min(view_bottom), full),
                        (y0.max(view_bottom), y1, veil(full)),
                    ] {
                        if b > a {
                            painter.rect_filled(span(x0, x0 + COLUMN, a, b), 0.0, c);
                        }
                    }
                }
                start = p;
            }
        }
        // Viewport frame.
        painter.rect_stroke(
            span(
                FRAME_LEFT,
                FRAME_RIGHT + 1.0,
                view_top - 1.0,
                view_bottom.min(bottom) + 1.0,
            ),
            0.0,
            egui::Stroke::new(1.0, FRAME),
            egui::StrokeKind::Inside,
        );
        // Current-row pointer: 9-row triangle at x1..5 pointing right.
        // One unfeathered mesh: egui feathers 1x1 rects away at the tips.
        let cy = y(current);
        let mut mesh = egui::Mesh::default();
        for d in -4i32..=4 {
            let w = 5 - d.abs();
            mesh.add_colored_rect(
                span(1.0, 1.0 + w as f32, cy + d as f32, cy + d as f32 + 1.0),
                POINTER,
            );
        }
        painter.add(egui::Shape::mesh(mesh));
        if response.clicked() || response.dragged() {
            response
                .interact_pointer_pos()
                .map(|p| Self::target((p.y - content_top) / height as f32, self.total))
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_pointer_click_maps_through_the_scaled_track() {
        let ctx = eframe::egui::Context::default();
        let mut nav = super::NavBar {
            total: 1000,
            ..Default::default()
        };
        // 900 px strip: content starts 13 px down and is 885 px tall, so
        // each pixel stands for 1000/885 rows.
        let pos = eframe::egui::pos2(20.0, 13.0 + 0.7 * 885.0);
        let mut target = None;
        for pressed in [None, Some(true), Some(false)] {
            let mut input = eframe::egui::RawInput {
                screen_rect: Some(eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    eframe::egui::vec2(52.0, 900.0),
                )),
                ..Default::default()
            };
            if let Some(pressed) = pressed {
                input.events = vec![
                    eframe::egui::Event::PointerMoved(pos),
                    eframe::egui::Event::PointerButton {
                        pos,
                        button: eframe::egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ];
            }
            let mut out = ctx.run_ui(input, |ui| {
                ui.spacing_mut().item_spacing = eframe::egui::Vec2::ZERO;
                if let Some(row) = nav.ui(ui, &Default::default(), 0..40, 0) {
                    target = Some(row);
                }
            });
            out.textures_delta.clear();
        }
        assert_eq!(target, Some(700));
    }
    #[test]
    fn clicking_seventy_percent_maps_to_seventy_percent() {
        assert_eq!(super::NavBar::target(0.7, 1000), 700);
        assert_eq!(super::NavBar::target(1.0, 1000), 999);
        assert_eq!(super::NavBar::target(0.7, 0), 0);
    }
    #[test]
    fn pixel_bins_keep_the_most_important_mark_per_side() {
        let mut nav = NavBar {
            total: 10,
            runs: vec![
                (1..2, [Importance::Unimportant, Importance::Unimportant]),
                (2..3, [Importance::Same, Importance::Important]),
            ],
            pixels: None,
        };
        // One pixel per row while rows fit.
        let one = nav.pixels(10).to_vec();
        assert_eq!(one[1], [Importance::Unimportant; 2]);
        assert_eq!(one[2], [Importance::Same, Importance::Important]);
        // Two rows per pixel: row 1 lands in pixel 0, row 2 in pixel 1.
        let half = nav.pixels(5).to_vec();
        assert_eq!(half[0], [Importance::Unimportant; 2]);
        assert_eq!(half[1], [Importance::Same, Importance::Important]);
        assert_eq!(
            ink(&DiffColors::default(), Importance::Important),
            egui::Color32::from_rgb(255, 65, 65)
        );
        assert_eq!(
            veil(ink(&DiffColors::default(), Importance::Important)),
            egui::Color32::from_rgb(252, 100, 100)
        );
    }
}
