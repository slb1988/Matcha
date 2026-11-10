//! Programmatic, original icons used by the shared toolbar and status chrome.
//!
//! No reference-tool bitmap is embedded here: every icon is assembled from simple
//! vector primitives at paint time so it remains crisp at 12-16 logical pixels.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Home,
    Sessions,
    All,
    Diffs,
    Same,
    Context,
    Minor,
    Rules,
    Format,
    Copy,
    Next,
    Prev,
    Swap,
    Reload,
    Expand,
    Collapse,
    Select,
    Files,
    Refresh,
    Stop,
    Filters,
    Sync,
    CopyLeft,
    CopyRight,
    Edit,
    Structure,
    Folder,
    File,
    ArrowLeft,
    ArrowRight,
    Status,
    History,
    Browse,
    Save,
    Peek,
    UpLevel,
    FolderUp,
    FolderUpBoth,
    BackCircle,
    ForwardCircle,
}

impl Icon {
    pub const ALL: [Self; 40] = [
        Self::Home,
        Self::Sessions,
        Self::All,
        Self::Diffs,
        Self::Same,
        Self::Context,
        Self::Minor,
        Self::Rules,
        Self::Format,
        Self::Copy,
        Self::Next,
        Self::Prev,
        Self::Swap,
        Self::Reload,
        Self::Expand,
        Self::Collapse,
        Self::Select,
        Self::Files,
        Self::Refresh,
        Self::Stop,
        Self::Filters,
        Self::Sync,
        Self::CopyLeft,
        Self::CopyRight,
        Self::Edit,
        Self::Structure,
        Self::Folder,
        Self::File,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::Status,
        Self::History,
        Self::Browse,
        Self::Save,
        Self::Peek,
        Self::UpLevel,
        Self::FolderUp,
        Self::FolderUpBoth,
        Self::BackCircle,
        Self::ForwardCircle,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Sessions => "Sessions",
            Self::All => "All",
            Self::Diffs => "Diffs",
            Self::Same => "Same",
            Self::Context => "Context",
            Self::Minor => "Minor",
            Self::Rules => "Rules",
            Self::Format => "Format",
            Self::Copy => "Copy",
            Self::Next => "Next Section",
            Self::Prev => "Prev Section",
            Self::Swap => "Swap",
            Self::Reload => "Reload",
            Self::Expand => "Expand",
            Self::Collapse => "Collapse",
            Self::Select => "Select",
            Self::Files => "Files",
            Self::Refresh => "Refresh",
            Self::Stop => "Stop",
            Self::Filters => "Filters",
            Self::Sync => "Sync",
            Self::CopyLeft => "Copy Left",
            Self::CopyRight => "Copy Right",
            Self::Edit => "Edit",
            Self::Structure => "Structure",
            Self::Folder => "Folder",
            Self::File => "File",
            Self::ArrowLeft => "Arrow Left",
            Self::ArrowRight => "Arrow Right",
            Self::Status => "Difference Status",
            Self::History => "History",
            Self::Browse => "Browse",
            Self::Save => "Save",
            Self::Peek => "Peek",
            Self::UpLevel => "Up One Level",
            Self::FolderUp => "Up Folder",
            Self::FolderUpBoth => "Up Both Sides",
            Self::BackCircle => "Back",
            Self::ForwardCircle => "Forward",
        }
    }
}

const BLUE: Color32 = Color32::from_rgb(45, 91, 145);
const PALE_BLUE: Color32 = Color32::from_rgb(111, 161, 207);
const RED: Color32 = Color32::from_rgb(190, 45, 45);
const YELLOW: Color32 = Color32::from_rgb(240, 196, 70);
const GREY: Color32 = Color32::from_rgb(105, 105, 105);
const DARK: Color32 = Color32::from_rgb(45, 45, 45);

/// Paint one original icon into `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon) {
    let s = rect.width().min(rect.height()).max(8.0);
    let c = rect.center();
    let rect = Rect::from_center_size(c, Vec2::splat(s));
    let stroke = Stroke::new((s / 9.0).clamp(1.2, 2.0), BLUE);
    let thin = Stroke::new(1.25, DARK);
    let inset = rect.shrink(s * 0.08);
    match icon {
        Icon::Home => {
            let roof = [
                Pos2::new(c.x, inset.top()),
                Pos2::new(inset.left(), c.y - s * 0.02),
                Pos2::new(inset.left(), inset.bottom()),
                Pos2::new(inset.right(), inset.bottom()),
                Pos2::new(inset.right(), c.y - s * 0.02),
            ];
            painter.add(egui::Shape::closed_line(roof.to_vec(), stroke));
            painter.line_segment(
                [
                    Pos2::new(c.x, inset.bottom()),
                    Pos2::new(c.x, inset.bottom() - s * 0.25),
                ],
                stroke,
            );
        }
        Icon::Sessions => {
            painter.rect_filled(inset, 1.0, Color32::from_rgb(220, 230, 239));
            painter.rect_stroke(inset, 1.0, stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(inset.left(), inset.top() + s * 0.2),
                    Pos2::new(inset.right(), inset.top() + s * 0.2),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - s * 0.16, inset.top()),
                    Pos2::new(c.x - s * 0.16, inset.top() - s * 0.1),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(c.x + s * 0.16, inset.top()),
                    Pos2::new(c.x + s * 0.16, inset.top() - s * 0.1),
                ],
                stroke,
            );
        }
        Icon::All => {
            for i in 0..4 {
                let v = Vec2::angled(i as f32 * std::f32::consts::PI / 4.0) * s * 0.36;
                painter.line_segment([c - v, c + v], stroke);
            }
        }
        Icon::Diffs => {
            not_equal(painter, c, s, RED);
        }
        Icon::Same => {
            painter.line_segment(
                [
                    Pos2::new(inset.left(), c.y - s * 0.12),
                    Pos2::new(inset.right(), c.y - s * 0.12),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left(), c.y + s * 0.12),
                    Pos2::new(inset.right(), c.y + s * 0.12),
                ],
                stroke,
            );
        }
        Icon::Context => {
            painter.rect_filled(inset, 1.0, Color32::WHITE);
            painter.rect_stroke(inset, 1.0, stroke, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(inset.left(), c.y - s * 0.08),
                    Pos2::new(inset.left() + s * 0.32, c.y + s * 0.18),
                ),
                0.0,
                RED,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.42, c.y - s * 0.2),
                    Pos2::new(inset.right() - s * 0.08, c.y - s * 0.2),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.42, c.y + s * 0.12),
                    Pos2::new(inset.right() - s * 0.08, c.y + s * 0.12),
                ],
                thin,
            );
        }
        Icon::Minor => {
            for y in [-0.14, 0.14] {
                let points = (0..=12)
                    .map(|i| {
                        let x = i as f32 / 12.0;
                        c + Vec2::new(
                            (x - 0.5) * s * 0.8,
                            (y + (x * std::f32::consts::TAU).sin() * 0.07) * s,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(points, Stroke::new(1.3, PALE_BLUE)));
            }
        }
        Icon::Rules => {
            painter.rect_filled(inset, 1.0, Color32::from_rgb(245, 245, 238));
            painter.rect_stroke(inset, 1.0, stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.16, inset.top() + s * 0.2),
                    Pos2::new(inset.right() - s * 0.12, inset.top() + s * 0.2),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.16, c.y),
                    Pos2::new(inset.right() - s * 0.12, c.y),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.16, inset.bottom() - s * 0.2),
                    Pos2::new(inset.right() - s * 0.12, inset.bottom() - s * 0.2),
                ],
                thin,
            );
            painter.circle_filled(
                Pos2::new(inset.left() + s * 0.1, inset.top() + s * 0.2),
                1.3,
                RED,
            );
        }
        Icon::Format => {
            painter.rect_filled(inset, 1.0, Color32::WHITE);
            painter.rect_stroke(inset, 1.0, stroke, StrokeKind::Inside);
            for y in [0.28, 0.48, 0.68] {
                painter.line_segment(
                    [
                        Pos2::new(inset.left() + s * 0.12, rect.top() + s * y),
                        Pos2::new(inset.right() - s * 0.2, rect.top() + s * y),
                    ],
                    thin,
                );
            }
            gear(
                painter,
                Pos2::new(inset.right(), inset.bottom()),
                s * 0.2,
                GREY,
            );
        }
        Icon::Copy => {
            painter.rect_filled(inset, 0.0, Color32::WHITE);
            painter.rect_stroke(inset, 0.0, thin, StrokeKind::Inside);
            arrow(painter, c, 1.0, YELLOW, s);
        }
        Icon::CopyLeft | Icon::CopyRight => {
            let direction = if icon == Icon::CopyLeft { -1.0 } else { 1.0 };
            painter.rect_stroke(
                inset.translate(Vec2::new(-direction * s * 0.08, -s * 0.1)),
                0.0,
                thin,
                StrokeKind::Inside,
            );
            arrow(painter, c + Vec2::new(0.0, s * 0.12), direction, BLUE, s);
        }
        Icon::Next | Icon::Prev => {
            let direction = if icon == Icon::Next { 1.0 } else { -1.0 };
            let tip = c + Vec2::new(0.0, s * 0.36 * direction);
            painter.line_segment(
                [c - Vec2::new(0.0, s * 0.34 * direction), tip],
                Stroke::new(2.0, YELLOW),
            );
            arrow_head(
                painter,
                tip,
                direction * std::f32::consts::FRAC_PI_2,
                YELLOW,
            );
            painter.line_segment(
                [tip + Vec2::new(-s * 0.2, -s * 0.25 * direction), tip],
                thin,
            );
        }
        Icon::ArrowLeft | Icon::ArrowRight => {
            let d = if icon == Icon::ArrowLeft { -1.0 } else { 1.0 };
            let points = [
                (-0.34, -0.12),
                (0.02, -0.12),
                (0.02, -0.30),
                (0.38, 0.0),
                (0.02, 0.30),
                (0.02, 0.12),
                (-0.34, 0.12),
            ]
            .map(|(x, y)| c + Vec2::new(x * s * d, y * s));
            // 凹多边形先三角化，不把凹路径交给 convex_polygon。
            painter.add(egui::Shape::convex_polygon(
                vec![points[2], points[3], points[4]],
                YELLOW,
                Stroke::NONE,
            ));
            painter.rect_filled(Rect::from_two_pos(points[0], points[5]), 0.0, YELLOW);
            painter.add(egui::Shape::closed_line(points.to_vec(), thin));
        }
        Icon::Swap => {
            arc(painter, c, s * 0.29, -2.8, -0.1, stroke);
            arc(painter, c, s * 0.29, 0.3, 3.0, Stroke::new(1.3, PALE_BLUE));
            arrow_head(
                painter,
                c + Vec2::new(s * 0.29, 0.0),
                std::f32::consts::FRAC_PI_2,
                BLUE,
            );
            arrow_head(
                painter,
                c - Vec2::new(s * 0.29, 0.0),
                -std::f32::consts::FRAC_PI_2,
                PALE_BLUE,
            );
        }
        Icon::Reload => {
            arc(painter, c, s * 0.3, -2.5, 2.1, stroke);
            arrow_head(painter, c + Vec2::new(-s * 0.15, s * 0.25), 3.6, BLUE);
        }
        Icon::Refresh => {
            arc(painter, c, s * 0.3, -2.7, -0.15, stroke);
            arc(painter, c, s * 0.3, 0.45, 3.0, Stroke::new(1.4, PALE_BLUE));
            arrow_head(painter, c + Vec2::new(s * 0.3, -s * 0.04), 1.2, BLUE);
            painter.circle_filled(c, s * 0.09, YELLOW);
        }
        Icon::Expand | Icon::Collapse => {
            painter.rect_filled(inset, 0.0, Color32::WHITE);
            painter.rect_stroke(inset, 0.0, thin, StrokeKind::Inside);
            painter.line_segment(
                [c - Vec2::new(s * 0.2, 0.0), c + Vec2::new(s * 0.2, 0.0)],
                stroke,
            );
            if icon == Icon::Expand {
                painter.line_segment(
                    [c - Vec2::new(0.0, s * 0.2), c + Vec2::new(0.0, s * 0.2)],
                    stroke,
                );
            }
        }
        Icon::Select => {
            painter.rect_stroke(inset, 0.0, stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.15, c.y),
                    Pos2::new(c.x - s * 0.03, inset.bottom() - s * 0.15),
                ],
                Stroke::new(1.7, BLUE),
            );
            painter.line_segment(
                [
                    Pos2::new(c.x - s * 0.03, inset.bottom() - s * 0.15),
                    Pos2::new(inset.right() - s * 0.12, inset.top() + s * 0.15),
                ],
                Stroke::new(1.7, BLUE),
            );
        }
        Icon::Files => {
            painter.rect_stroke(
                inset.translate(Vec2::new(-s * 0.1, 0.0)),
                0.0,
                thin,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                inset.translate(Vec2::new(s * 0.1, 0.0)),
                0.0,
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Stop => {
            painter.rect_filled(inset.shrink(s * 0.05), 1.0, RED);
        }
        Icon::Filters => {
            painter.line_segment(
                [
                    Pos2::new(inset.left(), inset.top()),
                    Pos2::new(inset.right(), inset.top()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.15, c.y),
                    Pos2::new(inset.right() - s * 0.15, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.3, inset.bottom()),
                    Pos2::new(inset.right() - s * 0.3, inset.bottom()),
                ],
                stroke,
            );
        }
        Icon::Sync => {
            arrow_line(
                painter,
                c.y - s * 0.15,
                inset.left(),
                inset.right(),
                1.0,
                BLUE,
            );
            arrow_line(
                painter,
                c.y + s * 0.15,
                inset.right(),
                inset.left(),
                -1.0,
                PALE_BLUE,
            );
        }
        Icon::Edit => {
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.2, inset.bottom() - s * 0.1),
                    Pos2::new(inset.right() - s * 0.1, inset.top() + s * 0.1),
                ],
                Stroke::new(2.4, BLUE),
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.14, inset.bottom() - s * 0.05),
                    Pos2::new(inset.left() + s * 0.2, inset.bottom() - s * 0.2),
                ],
                thin,
            );
        }
        Icon::Structure => {
            painter.circle_filled(Pos2::new(c.x, inset.top()), s * 0.1, BLUE);
            painter.circle_filled(
                Pos2::new(inset.left() + s * 0.1, inset.bottom()),
                s * 0.1,
                BLUE,
            );
            painter.circle_filled(
                Pos2::new(inset.right() - s * 0.1, inset.bottom()),
                s * 0.1,
                BLUE,
            );
            painter.line_segment(
                [Pos2::new(c.x, inset.top() + s * 0.1), Pos2::new(c.x, c.y)],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.1, c.y),
                    Pos2::new(inset.right() - s * 0.1, c.y),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.1, c.y),
                    Pos2::new(inset.left() + s * 0.1, inset.bottom() - s * 0.1),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.right() - s * 0.1, c.y),
                    Pos2::new(inset.right() - s * 0.1, inset.bottom() - s * 0.1),
                ],
                thin,
            );
        }
        Icon::Folder => {
            let body = Rect::from_min_max(
                Pos2::new(inset.left(), c.y - s * 0.12),
                Pos2::new(inset.right(), inset.bottom()),
            );
            painter.rect_filled(body, 1.0, YELLOW);
            painter.rect_stroke(body, 1.0, thin, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(inset.left_top(), Pos2::new(c.x - s * 0.02, c.y - s * 0.12)),
                1.0,
                YELLOW,
            );
        }
        Icon::File => {
            painter.rect_filled(inset, 0.0, Color32::WHITE);
            painter.rect_stroke(inset, 0.0, thin, StrokeKind::Inside);
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.15, c.y - s * 0.1),
                    Pos2::new(inset.right() - s * 0.15, c.y - s * 0.1),
                ],
                thin,
            );
            painter.line_segment(
                [
                    Pos2::new(inset.left() + s * 0.15, c.y + s * 0.13),
                    Pos2::new(inset.right() - s * 0.15, c.y + s * 0.13),
                ],
                thin,
            );
        }
        Icon::Status => {
            not_equal(painter, c, s * 0.8, RED);
            painter.circle_filled(c + Vec2::new(s * 0.34, -s * 0.34), s * 0.07, YELLOW);
        }
        Icon::History => {
            arc(painter, c, s * 0.3, -2.8, 2.4, stroke);
            arrow_head(
                painter,
                Pos2::new(inset.left() + s * 0.08, c.y - s * 0.2),
                2.8,
                BLUE,
            );
            painter.line_segment([c, Pos2::new(c.x, c.y - s * 0.18)], thin);
            painter.line_segment([c, Pos2::new(c.x + s * 0.16, c.y + s * 0.08)], thin);
        }
        Icon::Browse => {
            let body = Rect::from_min_max(
                inset.left_top() + Vec2::new(0.0, s * 0.18),
                inset.right_bottom(),
            );
            painter.rect_filled(body, 0.0, YELLOW);
            painter.rect_stroke(body, 0.0, thin, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_size(inset.left_top(), Vec2::new(s * 0.28, s * 0.2)),
                0.0,
                YELLOW,
            );
            painter.circle_stroke(c + Vec2::new(s * 0.16, 0.0), s * 0.15, stroke);
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.25, s * 0.1),
                    c + Vec2::new(s * 0.38, s * 0.28),
                ],
                stroke,
            );
        }
        Icon::Save => {
            painter.rect_filled(inset, 0.0, Color32::from_rgb(224, 234, 242));
            painter.rect_stroke(inset, 0.0, stroke, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(inset.left() + s * 0.18, inset.top() + s * 0.12),
                    Pos2::new(inset.right() - s * 0.18, c.y - s * 0.02),
                ),
                0.0,
                Color32::WHITE,
            );
            painter.rect_stroke(
                Rect::from_min_max(
                    Pos2::new(inset.left() + s * 0.22, c.y + s * 0.04),
                    Pos2::new(inset.right() - s * 0.22, inset.bottom() - s * 0.1),
                ),
                0.0,
                thin,
                StrokeKind::Inside,
            );
        }
        Icon::Peek => {
            // 漏斗（同 Filters）+ 右下角眼睛：参考 Peek 为漏斗加眼睛的组合图标。
            let f = Rect::from_min_max(
                Pos2::new(inset.left(), inset.top()),
                Pos2::new(inset.right() - s * 0.3, c.y),
            );
            painter.line_segment([f.left_top(), f.right_top()], stroke);
            painter.line_segment(
                [
                    Pos2::new(f.left() + s * 0.1, f.top()),
                    Pos2::new(f.center().x, f.bottom()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(f.right() - s * 0.1, f.top()),
                    Pos2::new(f.center().x, f.bottom()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(f.center().x, f.bottom()),
                    Pos2::new(f.center().x, f.bottom() + s * 0.14),
                ],
                stroke,
            );
            let eye = c + Vec2::new(s * 0.18, s * 0.2);
            painter.circle_stroke(eye, s * 0.2, thin);
            painter.circle_filled(eye, s * 0.07, DARK);
        }
        Icon::UpLevel => {
            // 向左上回转的灰箭头（参考路径栏第一个按钮）。
            let gray = Stroke::new((s / 8.0).clamp(1.2, 1.8), GREY);
            let points: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.0;
                    Pos2::new(
                        inset.right() - t * s * 0.62,
                        inset.bottom()
                            - s * 0.12
                            - (t * std::f32::consts::FRAC_PI_2).sin() * s * 0.5,
                    )
                })
                .collect();
            painter.add(egui::Shape::line(points, gray));
            arrow_head(
                painter,
                Pos2::new(inset.left() + s * 0.16, inset.bottom() - s * 0.62),
                -2.2,
                GREY,
            );
        }
        Icon::FolderUp | Icon::FolderUpBoth => {
            // 黄色文件夹 + 向上的蓝色箭头（UpBoth 为两个箭头）。
            let body = Rect::from_min_max(
                Pos2::new(inset.left(), c.y - s * 0.1),
                Pos2::new(inset.right(), inset.bottom()),
            );
            painter.rect_filled(body, 1.0, YELLOW);
            painter.rect_stroke(body, 1.0, thin, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(inset.left_top(), Pos2::new(c.x - s * 0.02, c.y - s * 0.1)),
                1.0,
                YELLOW,
            );
            let xs: &[(f32, f32)] = if icon == Icon::FolderUp {
                &[(0.0, 0.0)]
            } else {
                &[(-s * 0.14, 0.0), (s * 0.14, 0.0)]
            };
            for (dx, _) in xs {
                let tip = Pos2::new(c.x + dx, body.top() + s * 0.1);
                painter.line_segment(
                    [tip + Vec2::new(0.0, s * 0.42), tip],
                    Stroke::new(1.5, BLUE),
                );
                arrow_head(painter, tip, -std::f32::consts::FRAC_PI_2, BLUE);
            }
        }
        Icon::BackCircle | Icon::ForwardCircle => {
            let direction = if icon == Icon::BackCircle { -1.0 } else { 1.0 };
            painter.circle_stroke(c, s * 0.36, Stroke::new(1.4, GREY));
            let y = c.y;
            painter.line_segment(
                [
                    Pos2::new(c.x - direction * s * 0.18, y),
                    Pos2::new(c.x + direction * s * 0.16, y),
                ],
                Stroke::new(1.5, GREY),
            );
            arrow_head(
                painter,
                Pos2::new(c.x + direction * s * 0.16, y),
                if direction > 0.0 {
                    0.0
                } else {
                    std::f32::consts::PI
                },
                GREY,
            );
        }
    }
}

fn arc(painter: &Painter, center: Pos2, radius: f32, start: f32, end: f32, stroke: Stroke) {
    let steps = 12;
    let points = (0..=steps)
        .map(|i| {
            let t = start + (end - start) * i as f32 / steps as f32;
            center + Vec2::angled(t) * radius
        })
        .collect();
    painter.add(egui::Shape::line(points, stroke));
}
fn not_equal(painter: &Painter, c: Pos2, s: f32, color: Color32) {
    for y in [-0.13, 0.13] {
        painter.line_segment(
            [
                c + Vec2::new(-s * 0.3, s * y),
                c + Vec2::new(s * 0.3, s * y),
            ],
            Stroke::new(1.5, color),
        );
    }
    painter.line_segment(
        [
            c + Vec2::new(-s * 0.16, s * 0.35),
            c + Vec2::new(s * 0.16, -s * 0.35),
        ],
        Stroke::new(1.5, color),
    );
}

fn arrow(painter: &Painter, center: Pos2, direction: f32, color: Color32, size: f32) {
    let start = Pos2::new(center.x - 0.38 * direction * size, center.y);
    let end = Pos2::new(center.x + 0.38 * direction * size, center.y);
    painter.line_segment([start, end], Stroke::new(1.7, color));
    arrow_head(
        painter,
        end,
        if direction > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        },
        color,
    );
}

fn arrow_line(painter: &Painter, y: f32, from: f32, to: f32, direction: f32, color: Color32) {
    let end = Pos2::new(to, y);
    painter.line_segment([Pos2::new(from, y), end], Stroke::new(1.3, color));
    arrow_head(
        painter,
        end,
        if direction > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        },
        color,
    );
}

fn arrow_head(painter: &Painter, tip: Pos2, angle: f32, color: Color32) {
    let b = Vec2::angled(angle + 2.45) * 5.0;
    let c = Vec2::angled(angle - 2.45) * 5.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, tip + b, tip + c],
        color,
        Stroke::NONE,
    ));
}

fn gear(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    painter.circle_filled(center, radius, color);
    painter.circle_filled(center, radius * 0.38, Color32::WHITE);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let p = center + Vec2::angled(a) * radius * 1.35;
        painter.circle_filled(p, radius * 0.25, color);
    }
}

/// 使用几何三角，不依赖系统 UI 字体是否包含 U+25BE。
pub fn dropdown_triangle(painter: &Painter, center: Pos2, color: Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-3.0, -1.0),
            center + Vec2::new(3.0, -1.0),
            center + Vec2::new(0.0, 2.0),
        ],
        color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_icon_has_a_label() {
        assert_eq!(Icon::ALL.len(), 40);
        assert!(Icon::ALL.iter().all(|icon| !icon.label().is_empty()));
    }

    #[test]
    fn each_icon_emits_distinct_vector_geometry_without_text_placeholders() {
        let ctx = egui::Context::default();
        let mut seen = std::collections::HashSet::new();
        for icon in Icon::ALL {
            let output = crate::chrome::test_frame(&ctx, Default::default(), |ui| {
                paint(
                    ui.painter(),
                    Rect::from_min_size(Pos2::ZERO, Vec2::splat(16.0)),
                    icon,
                );
            });
            assert!(!output.shapes.is_empty(), "{icon:?}");
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| matches!(s.shape, egui::Shape::Text(_))),
                "{icon:?}"
            );
            assert!(
                seen.insert(format!("{:?}", output.shapes)),
                "重复图形: {icon:?}"
            );
        }
    }
}
