//! 文本比较的 Word Wrap 状态：每侧折行参数、字宽缓存、断点缓存，以及按显示行建高度表
//! （对齐行取两侧视觉行数的最大值）与滚动锚定。算法在 [`crate::wrap`]。
use super::{
    gutter,
    projection::{DisplayRow, RowGeometry},
    view::index,
    TextCompareView,
};
use crate::wrap::{self, Glyphs, Quick, RowHeights, RowSource, WrapSpec, Wrapped};
use bc_engine_text::content::ContentSnapshot;
use eframe::egui;
use std::collections::HashMap;

/// 纵向滚动条占用的宽度（`bar_width` 16 + 内外边距各 1）。
const VBAR: f32 = 18.0;
/// 文本区右侧留白。
const RIGHT_PAD: f32 = 4.0;
/// 空闲帧后台补齐的字节预算。
const IDLE_BUDGET: usize = 2 * 1024 * 1024;
/// 断点缓存上限（每侧）。
const BREAK_CACHE: usize = 256;

/// 窗格（含行号栏与纵向滚动条）宽度对应的文本区宽度。
pub(crate) fn text_width(pane_width: f32, numbers: bool) -> f32 {
    let left = if numbers { gutter::WIDTH + 1.0 } else { 8.0 };
    (pane_width - VBAR - left - RIGHT_PAD).max(1.0)
}

/// 一侧已算过的断点：随内容修订号与折行参数失效。
#[derive(Default)]
struct BreakCache {
    revision: (u64, u64),
    key: (i32, usize, bool),
    lines: HashMap<usize, Wrapped>,
}

pub(crate) struct WrapState {
    glyphs: Glyphs,
    font_size: f32,
    pub(crate) specs: [WrapSpec; 2],
    cache: [BreakCache; 2],
}

/// 视口顶端：显示行与行内偏移（像素），附记录时的 `scroll.y`。
#[derive(Debug, Clone, Copy)]
pub(crate) struct TopAnchor {
    scroll_y: f32,
    display: usize,
    offset: f32,
}

/// 建表用的行来源：对齐行取两侧视觉行数的最大值。
struct Rows<'a> {
    view: &'a TextCompareView,
    snaps: [ContentSnapshot; 2],
    glyphs: &'a mut Glyphs,
    specs: [WrapSpec; 2],
}

/// 对齐行 `row` 在 `s` 侧的文本（该侧无此行时 `None`）。
fn line_text<'a>(
    view: &TextCompareView,
    snaps: &'a [ContentSnapshot; 2],
    s: usize,
    row: usize,
) -> Option<&'a str> {
    let line = index(view.row(row), s)?;
    let bytes = snaps[s].lines.line_bytes(line)?;
    Some(std::str::from_utf8(bytes).unwrap_or_default())
}

impl RowSource for Rows<'_> {
    fn quick(&mut self, display: usize) -> Quick {
        let row = match self.view.projection.row(display) {
            Some(DisplayRow::Text(row)) => row,
            Some(DisplayRow::Fold(_)) => return Quick::Known(self.view.projection.fold_units()),
            None => return Quick::Known(1),
        };
        let mut known = 1;
        let mut bytes = 0;
        let mut estimate = 1;
        for s in 0..2 {
            let Some(text) = line_text(self.view, &self.snaps, s, row) else {
                continue;
            };
            if wrap::fits_quickly(text, &self.specs[s], self.glyphs) {
                continue;
            }
            bytes += text.len();
            // 估算：按 ASCII 字宽，至少 1。
            let per = self.glyphs.width('M').max(1.0);
            estimate = estimate.max(
                ((text.len() as f32 * per) / self.specs[s].width.max(1.0)).ceil() as u32,
            );
            known = 0;
        }
        if known == 1 {
            Quick::Known(1)
        } else {
            Quick::Measure { bytes, estimate }
        }
    }
    fn measure(&mut self, display: usize) -> u32 {
        let Some(DisplayRow::Text(row)) = self.view.projection.row(display) else {
            return match self.view.projection.row(display) {
                Some(DisplayRow::Fold(_)) => self.view.projection.fold_units(),
                _ => 1,
            };
        };
        let mut units = 1;
        for s in 0..2 {
            if let Some(text) = line_text(self.view, &self.snaps, s, row) {
                units = units.max(wrap::count(text, &self.specs[s], self.glyphs));
            }
        }
        units
    }
}

impl TextCompareView {
    /// 每侧的折行参数（由本帧的窗格矩形决定）。
    fn wrap_specs(&self, panes: [egui::Rect; 2]) -> [WrapSpec; 2] {
        std::array::from_fn(|s| WrapSpec {
            width: text_width(panes[s].width(), self.show_numbers).floor(),
            tab: self
                .content(s)
                .format()
                .and_then(|f| f.tab_size)
                .unwrap_or(4)
                .clamp(1, 32) as usize,
            whitespace: self.show_whitespace,
        })
    }

    /// 本帧窗格矩形已知后调用：按需建立 / 重建 / 补齐高度表，或在关闭时移除，并保持视口顶端行。
    pub(crate) fn ensure_wrap(&mut self, ctx: &egui::Context, panes: [egui::Rect; 2], view_h: f32) {
        let g = RowGeometry::for_row_height(self.row_height());
        if !self.word_wrap {
            self.wrap = None;
            if self.projection.heights().is_some() {
                let anchor = self.anchor_now(g);
                self.projection.set_heights(None);
                self.restore(anchor, g);
            }
            return;
        }
        let specs = self.wrap_specs(panes);
        let font_size = self.font_size;
        let mut state = match self.wrap.take() {
            Some(s) if s.font_size == font_size => s,
            _ => WrapState {
                glyphs: Glyphs::egui(ctx, egui::FontId::monospace(font_size)),
                font_size,
                specs,
                cache: Default::default(),
            },
        };
        let has = self.projection.heights().is_some();
        let changed = state.specs.iter().zip(&specs).any(|(a, b)| a.key() != b.key());
        if !has || changed {
            // 投影刚重建时，用上一帧记录的锚点（滚动位置未被显式改动时）。
            let anchor = if has {
                self.anchor_now(g)
            } else {
                match self.top_anchor {
                    Some(a) if a.scroll_y == self.scroll.y => a,
                    _ => self.anchor_now(g),
                }
            };
            state.specs = specs;
            let snaps = [self.map.left.snapshot(), self.map.right.snapshot()];
            let table = RowHeights::build(
                self.projection.len(),
                &mut Rows {
                    view: self,
                    snaps,
                    glyphs: &mut state.glyphs,
                    specs,
                },
                wrap::SYNC_BUDGET,
            );
            self.projection.set_heights(Some(table));
            self.restore(anchor, g);
        }
        if self.projection.heights().is_some_and(|h| !h.is_complete()) {
            // 视口及上下各一屏精确，其余在空闲帧分批补齐。
            let anchor = self.anchor_now(g);
            let top = self.projection.display_at((self.scroll.y - view_h).max(0.0), g);
            let bottom = self.projection.display_at(self.scroll.y + 2.0 * view_h, g) + 1;
            let snaps = [self.map.left.snapshot(), self.map.right.snapshot()];
            let mut table = self.projection.heights_mut().map(std::mem::take).unwrap();
            let mut rows = Rows {
                view: self,
                snaps,
                glyphs: &mut state.glyphs,
                specs,
            };
            let mut moved = table.refine(top..bottom, &mut rows);
            moved |= table.advance(&mut rows, IDLE_BUDGET);
            let more = !table.is_complete();
            self.projection.set_heights(Some(table));
            if moved {
                self.restore(anchor, g);
            }
            if more {
                ctx.request_repaint();
            }
        }
        self.wrap = Some(state);
    }

    /// 帧末记录视口顶端，供下一帧投影重建后恢复。
    pub(crate) fn record_top(&mut self) {
        let g = RowGeometry::for_row_height(self.row_height());
        self.top_anchor = Some(self.anchor_now(g));
    }

    fn anchor_now(&self, g: RowGeometry) -> TopAnchor {
        let display = self.projection.display_at(self.scroll.y, g);
        TopAnchor {
            scroll_y: self.scroll.y,
            display,
            offset: (self.scroll.y - self.projection.y_of(display, g)).max(0.0),
        }
    }

    fn restore(&mut self, anchor: TopAnchor, g: RowGeometry) {
        if anchor.scroll_y <= 0.0 {
            self.scroll.y = 0.0;
        } else {
            let display = anchor.display.min(self.projection.len().saturating_sub(1));
            let h = self.projection.row_height(display, g);
            self.scroll.y = self.projection.y_of(display, g) + anchor.offset.min((h - 1.0).max(0.0));
        }
        self.top_anchor = Some(self.anchor_now(g));
    }

    /// 某侧某行的折行断点（绘制、命中、光标可见性共用；缓存随修订号失效）。
    pub(crate) fn wrapped(&mut self, s: usize, line: usize, text: &str) -> Wrapped {
        let revision = self.content(s).revision();
        let Some(state) = self.wrap.as_mut() else {
            return Wrapped {
                starts: vec![(0, 0)],
                end: text.len(),
                hidden_chars: 0,
            };
        };
        let spec = state.specs[s];
        let cache = &mut state.cache[s];
        if cache.revision != revision || cache.key != spec.key() || cache.lines.len() > BREAK_CACHE
        {
            cache.lines.clear();
            cache.revision = revision;
            cache.key = spec.key();
        }
        cache
            .lines
            .entry(line)
            .or_insert_with(|| wrap::breaks(text, &spec, &mut state.glyphs))
            .clone()
    }
}

impl TextCompareView {
    /// Word Wrap：把 `s` 侧光标所在的视觉行滚到可见（最小滚动）。`pane_h` 为窗格总高
    /// （含 53 px 标题区与 19 px 状态条）。
    pub(crate) fn reveal_caret(&mut self, s: usize, pane_h: f32) {
        if self.projection.heights().is_none() {
            return;
        }
        let g = RowGeometry::for_row_height(self.row_height());
        let caret = self.editors[s].caret;
        let snapshot = self.content(s).snapshot();
        let Some(bytes) = snapshot.lines.line_bytes(caret.line) else {
            return;
        };
        let text = std::str::from_utf8(bytes).unwrap_or_default();
        let k = self.wrapped(s, caret.line, text).segment_of(caret.byte);
        let display = self
            .projection
            .display_for(self.row_for_line(s, caret.line));
        let top = self.projection.y_of(display, g) + k as f32 * g.row;
        let view_h = (pane_h - 53.0 - 19.0 - 2.0).max(g.row);
        if top < self.scroll.y {
            self.scroll.y = top;
        } else if top + g.row > self.scroll.y + view_h {
            self.scroll.y = top + g.row - view_h;
        }
    }
}
