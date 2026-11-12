//! OS file drops onto two-sided views: which side did the drop land on?
//!
//! winit reports a drop without coordinates, and on Windows the window receives
//! no mouse moves during an Explorer drag, so `pointer.hover_pos()` is stale or
//! `None` when the drop arrives (it used to fall back to the active side, so a
//! file dropped on the right pane replaced the left). The drop point is taken
//! from the OS cursor instead, mapped into egui points through the viewport's
//! inner rect; egui's pointer is only the fallback (other platforms, tests).
//!
//! The side is the pane whose rect is nearest to the drop point, so a drop on a
//! pane's path bar, column header or status strip still picks that pane.
use eframe::egui;
use std::path::PathBuf;

/// Drop point in egui points, if known.
pub fn drop_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    os_cursor(ctx).or_else(|| ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos())))
}

fn os_cursor(ctx: &egui::Context) -> Option<egui::Pos2> {
    let (x, y) = bc_shell::cursor_screen_pos()?;
    // Desktop coordinates of the client area, in points (egui-winit divides the
    // physical inner position by the same pixels-per-point as `ctx`).
    let inner = ctx.input(|i| i.viewport().inner_rect)?;
    let ppp = ctx.pixels_per_point();
    let pos = egui::pos2(x as f32 / ppp, y as f32 / ppp) - inner.min.to_vec2();
    pos.is_finite().then_some(pos)
}

/// Side of the drop target nearest to `pos` (`targets` pairs a side with one
/// of its areas: pane body, path bar, ...); `fallback` when the position or
/// every target rect is unknown.
pub fn side_at(
    pos: Option<egui::Pos2>,
    targets: impl IntoIterator<Item = (usize, egui::Rect)>,
    fallback: usize,
) -> usize {
    let Some(pos) = pos else {
        return fallback;
    };
    targets
        .into_iter()
        .filter(|(_, r)| r.is_finite() && r.is_positive())
        .min_by(|(_, a), (_, b)| {
            a.distance_sq_to_pos(pos)
                .total_cmp(&b.distance_sq_to_pos(pos))
        })
        .map_or(fallback, |(side, _)| side)
}

/// `(side, rect)` targets of one rect per side, in side order.
pub fn panes(rects: &[egui::Rect]) -> impl Iterator<Item = (usize, egui::Rect)> + '_ {
    rects.iter().copied().enumerate()
}

/// This frame's dropped paths (empty when nothing was dropped).
pub fn dropped_paths(ctx: &egui::Context) -> Vec<PathBuf> {
    ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .collect()
    })
}

/// First file dropped this frame and the side it landed on.
pub fn dropped_on(
    ctx: &egui::Context,
    targets: impl IntoIterator<Item = (usize, egui::Rect)>,
    fallback: usize,
) -> Option<(usize, PathBuf)> {
    let path = dropped_paths(ctx).into_iter().next()?;
    Some((side_at(drop_pos(ctx), targets, fallback), path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_pane_wins_and_unknown_falls_back() {
        let left = egui::Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(500.0, 800.0));
        let right = egui::Rect::from_min_max(egui::pos2(505.0, 100.0), egui::pos2(1000.0, 800.0));
        let two = [left, right];
        assert_eq!(side_at(Some(egui::pos2(700.0, 400.0)), panes(&two), 0), 1);
        assert_eq!(side_at(Some(egui::pos2(100.0, 400.0)), panes(&two), 1), 0);
        // Right path bar (above the grid) and right status strip.
        assert_eq!(side_at(Some(egui::pos2(800.0, 60.0)), panes(&two), 0), 1);
        assert_eq!(side_at(Some(egui::pos2(900.0, 850.0)), panes(&two), 0), 1);
        // Stacked (over/under) layout.
        let top = egui::Rect::from_min_max(egui::pos2(0.0, 50.0), egui::pos2(1000.0, 400.0));
        let bottom = egui::Rect::from_min_max(egui::pos2(0.0, 430.0), egui::pos2(1000.0, 800.0));
        assert_eq!(
            side_at(Some(egui::pos2(300.0, 420.0)), panes(&[top, bottom]), 0),
            1
        );
        // Over/under with both path bars stacked above the top pane: the
        // second bar is an explicit right-side target.
        let bar1 = egui::Rect::from_min_max(egui::pos2(0.0, 25.0), egui::pos2(1000.0, 49.0));
        let stacked = [top, bottom];
        let targets = panes(&stacked).chain([(1, bar1)]);
        assert_eq!(side_at(Some(egui::pos2(300.0, 30.0)), targets, 0), 1);
        assert_eq!(side_at(None, panes(&two), 1), 1);
        assert_eq!(
            side_at(
                Some(egui::pos2(1.0, 1.0)),
                panes(&[egui::Rect::NOTHING; 2]),
                1
            ),
            1
        );
    }
}
