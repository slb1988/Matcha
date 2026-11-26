//! 文本合并的 Word Wrap：四个窗格各按自身文本区宽度折行，网格行高取四者视觉行数的最大值。
//! 算法在 [`crate::wrap`]；几何经 [`MergeView::grid_y`] / [`MergeView::grid_at`] 使用高度表。
use super::{GridRow, MergeView, GUTTER, HEADER, OUT, SIDES};
use crate::wrap::{self, Glyphs, Quick, RowHeights, RowSource, WrapSpec, Wrapped};
use bc_core::Side;
use bc_engine_text::content::ContentSnapshot;
use eframe::egui;
use std::collections::HashMap;

/// 文本区右侧留白。
const RIGHT_PAD: f32 = 4.0;
/// 空闲帧后台补齐的字节预算。
const IDLE_BUDGET: usize = 2 * 1024 * 1024;
/// 断点缓存上限（每窗格）。
const BREAK_CACHE: usize = 256;

/// 窗格（一列）宽度对应的文本区宽度：行号栏 + 4 px 左距之后到右侧留白。
pub(crate) fn text_width(pane_width: f32) -> f32 {
    (pane_width - GUTTER - 4.0 - RIGHT_PAD).max(1.0)
}

/// 断点缓存的失效键：内容修订号与折行参数键。
type CacheKey = ((u64, u64), (i32, usize, bool));

pub(crate) struct MergeWrap {
    glyphs: Glyphs,
    font_size: f32,
    specs: [WrapSpec; 4],
    /// 建表时的网格代号与四个窗格内容修订号。
    key: (u64, [(u64, u64); 4]),
    cache: [HashMap<usize, Wrapped>; 4],
    cache_key: [CacheKey; 4],
}

/// 视口顶端：网格行与行内偏移，附记录时的 `scroll.y`。
#[derive(Debug, Clone, Copy)]
pub(crate) struct TopAnchor {
    scroll_y: f32,
    grid: usize,
    offset: f32,
}

impl MergeView {
    /// 网格行 `cell` 上窗格 `pane` 显示的行号（输入窗格：该对齐行在本侧的行，只在其首个网格行）。
    pub(crate) fn pane_line(&self, pane: usize, cell: &GridRow) -> Option<usize> {
        if pane == OUT {
            return cell.out;
        }
        if !cell.own {
            return None;
        }
        let mapped = self.model.map.row(cell.row)?;
        match SIDES[pane] {
            Side::Left => mapped.left,
            Side::Right => mapped.right,
            _ => mapped.center,
        }
    }
}

struct Rows<'a> {
    view: &'a MergeView,
    snaps: [ContentSnapshot; 4],
    glyphs: &'a mut Glyphs,
    specs: [WrapSpec; 4],
}

/// 网格行 `g` 上各窗格的文本（无内容的窗格跳过）。
fn texts<'a>(
    view: &MergeView,
    snaps: &'a [ContentSnapshot; 4],
    g: usize,
) -> Vec<(usize, std::borrow::Cow<'a, str>)> {
    let Some(cell) = view.model.grid().get(g) else {
        return Vec::new();
    };
    (0..4)
        .filter_map(|pane| {
            let line = view.pane_line(pane, cell)?;
            let bytes = snaps[pane].lines.line_bytes(line)?;
            Some((pane, String::from_utf8_lossy(bytes)))
        })
        .collect()
}

impl RowSource for Rows<'_> {
    fn quick(&mut self, g: usize) -> Quick {
        let texts = texts(self.view, &self.snaps, g);
        let mut bytes = 0;
        let mut estimate = 1u32;
        let per = self.glyphs.width('M').max(1.0);
        for (pane, text) in &texts {
            if wrap::fits_quickly(text, &self.specs[*pane], self.glyphs) {
                continue;
            }
            bytes += text.len();
            estimate = estimate
                .max(((text.len() as f32 * per) / self.specs[*pane].width.max(1.0)).ceil() as u32);
        }
        if bytes == 0 {
            Quick::Known(1)
        } else {
            Quick::Measure { bytes, estimate }
        }
    }
    fn measure(&mut self, g: usize) -> u32 {
        let texts = texts(self.view, &self.snaps, g);
        let mut units = 1;
        for (pane, text) in &texts {
            units = units.max(wrap::count(text, &self.specs[*pane], self.glyphs));
        }
        units
    }
}

impl MergeView {
    fn content_key(&self) -> (u64, [(u64, u64); 4]) {
        (
            self.model.grid_generation(),
            std::array::from_fn(|pane| self.content(pane).revision()),
        )
    }

    /// 本帧窗格矩形已知后调用：按需建立 / 重建 / 补齐高度表，或在关闭时移除；保持视口顶端行。
    pub(crate) fn ensure_wrap(&mut self, ctx: &egui::Context) {
        if !self.word_wrap {
            self.wrap = None;
            if self.heights.is_some() {
                let anchor = self.anchor_now();
                self.heights = None;
                self.restore(anchor);
            }
            return;
        }
        let specs: [WrapSpec; 4] = std::array::from_fn(|pane| WrapSpec {
            width: text_width(self.pane_rects[pane].width()).floor(),
            tab: self
                .content(pane)
                .format()
                .and_then(|f| f.tab_size)
                .unwrap_or(4)
                .clamp(1, 32) as usize,
            whitespace: false,
        });
        let font_size = self.font_size;
        let mut state = match self.wrap.take() {
            Some(s) if s.font_size == font_size => s,
            _ => MergeWrap {
                glyphs: Glyphs::egui(ctx, egui::FontId::monospace(font_size)),
                font_size,
                specs,
                key: (u64::MAX, Default::default()),
                cache: Default::default(),
                cache_key: Default::default(),
            },
        };
        let key = self.content_key();
        let changed = state.key != key
            || state
                .specs
                .iter()
                .zip(&specs)
                .any(|(a, b)| a.key() != b.key());
        if self.heights.is_none() || changed {
            let anchor = match (self.heights.is_some(), self.top_anchor) {
                (true, _) => self.anchor_now(),
                // 网格刚重建：用上一帧记录的锚点（滚动位置未被显式改动时）。
                (false, Some(a)) if a.scroll_y == self.scroll.y => a,
                _ => self.anchor_now(),
            };
            state.specs = specs;
            state.key = key;
            let snaps = std::array::from_fn(|pane| self.content(pane).snapshot());
            let table = RowHeights::build(
                self.model.grid().len(),
                &mut Rows {
                    view: self,
                    snaps,
                    glyphs: &mut state.glyphs,
                    specs,
                },
                wrap::SYNC_BUDGET,
            );
            self.heights = Some(table);
            self.restore(anchor);
        }
        if self.heights.as_ref().is_some_and(|h| !h.is_complete()) {
            let anchor = self.anchor_now();
            let view_h = self.pane_rects[OUT].height();
            let top = self.grid_at((self.scroll.y - view_h).max(0.0));
            let bottom = self.grid_at(self.scroll.y + 2.0 * view_h) + 1;
            let snaps = std::array::from_fn(|pane| self.content(pane).snapshot());
            let mut table = self.heights.take().unwrap();
            let mut rows = Rows {
                view: self,
                snaps,
                glyphs: &mut state.glyphs,
                specs,
            };
            let mut moved = table.refine(top..bottom, &mut rows);
            moved |= table.advance(&mut rows, IDLE_BUDGET);
            let more = !table.is_complete();
            self.heights = Some(table);
            if moved {
                self.restore(anchor);
            }
            if more {
                ctx.request_repaint();
            }
        }
        self.wrap = Some(state);
    }

    /// 帧末记录视口顶端，供下一帧网格重建后恢复。
    pub(crate) fn record_top(&mut self) {
        self.top_anchor = Some(self.anchor_now());
    }

    fn anchor_now(&self) -> TopAnchor {
        let grid = self.grid_at(self.scroll.y);
        TopAnchor {
            scroll_y: self.scroll.y,
            grid,
            offset: (self.scroll.y - self.grid_y(grid)).max(0.0),
        }
    }

    fn restore(&mut self, anchor: TopAnchor) {
        if anchor.scroll_y <= 0.0 {
            self.scroll.y = 0.0;
        } else {
            let grid = anchor
                .grid
                .min(self.model.grid().len().saturating_sub(1));
            let h = self.grid_units(grid) as f32 * self.row_height();
            self.scroll.y = self.grid_y(grid) + anchor.offset.min((h - 1.0).max(0.0));
        }
        self.top_anchor = Some(self.anchor_now());
    }

    /// 窗格 `pane` 第 `line` 行的折行断点（缓存随内容修订号与折行参数失效）。
    pub(crate) fn wrapped(&mut self, pane: usize, line: usize, text: &str) -> Wrapped {
        let revision = self.content(pane).revision();
        let Some(state) = self.wrap.as_mut() else {
            return Wrapped {
                starts: vec![(0, 0)],
                end: text.len(),
                hidden_chars: 0,
            };
        };
        let spec = state.specs[pane];
        let key = (revision, spec.key());
        if state.cache_key[pane] != key || state.cache[pane].len() > BREAK_CACHE {
            state.cache[pane].clear();
            state.cache_key[pane] = key;
        }
        state.cache[pane]
            .entry(line)
            .or_insert_with(|| wrap::breaks(text, &spec, &mut state.glyphs))
            .clone()
    }

    /// Word Wrap：把窗格 `pane` 的光标所在视觉行滚到可见（最小滚动）。
    pub(crate) fn reveal_caret(&mut self, pane: usize) {
        if self.heights.is_none() {
            return;
        }
        let caret = self.editor_of(pane).caret;
        let grid = if pane == OUT {
            self.model.grid_row_of_out(caret.line)
        } else {
            match self.side_rows[pane].get(caret.line) {
                Some(&row) => self.model.grid_row_of(row),
                None => return,
            }
        };
        let snapshot = self.content(pane).snapshot();
        let Some(bytes) = snapshot.lines.line_bytes(caret.line) else {
            return;
        };
        let text = String::from_utf8_lossy(bytes).into_owned();
        let k = self.wrapped(pane, caret.line, &text).segment_of(caret.byte);
        let row_height = self.row_height();
        let top = self.grid_y(grid) + k as f32 * row_height;
        let view_h = (self.pane_rects[pane].height() - HEADER).max(row_height);
        if top < self.scroll.y {
            self.scroll.y = top;
        } else if top + row_height > self.scroll.y + view_h {
            self.scroll.y = top + row_height - view_h;
        }
    }
}
