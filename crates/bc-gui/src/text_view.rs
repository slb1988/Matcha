//! Dual-pane text view prototype – validates virtual scrolling on very large files.
//!
//! The file is memory-mapped (never copied) and a line-start index is built once; each
//! frame only the visible rows are materialised. This is deliberately independent from
//! `bc_core::line::LineList` so the benchmark measures egui, not the (unimplemented)
//! engine; the production view will read rows from `bc_engine_text::TextMap`.

use eframe::egui;
use memmap2::Mmap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Longest line prefix rendered per row (keeps layout cost bounded on pathological files).
const MAX_RENDERED_BYTES: usize = 2048;

/// A memory-mapped text file with a line index.
pub struct LoadedText {
    pub path: PathBuf,
    map: Mmap,
    /// Byte offset of the start of each line; `line_starts[len]` is never stored,
    /// the end of the last line is `map.len()`.
    line_starts: Vec<u32>,
    pub load_time: Duration,
}

impl std::fmt::Debug for LoadedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedText")
            .field("path", &self.path)
            .field("bytes", &self.map.len())
            .field("lines", &self.line_starts.len())
            .finish()
    }
}

impl LoadedText {
    /// Map the file and index line starts.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let started = Instant::now();
        let file = File::open(path)?;
        // SAFETY: the mapping is read-only; concurrent modification of the file by other
        // processes is a documented limitation of the prototype (production code will copy
        // on edit or lock the file).
        #[allow(unsafe_code)]
        let map = unsafe { Mmap::map(&file)? };
        if map.len() > u32::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "prototype indexes files up to 4 GiB",
            ));
        }
        let line_starts = index_lines(&map);
        Ok(Self { path: path.to_path_buf(), map, line_starts, load_time: started.elapsed() })
    }

    pub fn byte_len(&self) -> usize {
        self.map.len()
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Bytes of line `index` without its terminator.
    pub fn line(&self, index: usize) -> &[u8] {
        let start = self.line_starts[index] as usize;
        let end = self
            .line_starts
            .get(index + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.map.len());
        let mut slice = &self.map[start..end];
        if let [rest @ .., b'\n'] = slice {
            slice = rest;
        }
        if let [rest @ .., b'\r'] = slice {
            slice = rest;
        }
        slice
    }

    /// Size of the line index in bytes (reported by the benchmark).
    pub fn index_bytes(&self) -> usize {
        self.line_starts.len() * std::mem::size_of::<u32>()
    }
}

/// Offsets of line starts (`0` plus every byte after `\n`).
fn index_lines(bytes: &[u8]) -> Vec<u32> {
    let mut starts = Vec::with_capacity(bytes.len() / 40 + 1);
    starts.push(0u32);
    let mut pos = 0usize;
    while let Some(rel) = bytes[pos..].iter().position(|&b| b == b'\n') {
        pos += rel + 1;
        if pos < bytes.len() {
            starts.push(pos as u32);
        }
    }
    starts
}

/// Per-frame render statistics for the benchmark.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneStats {
    pub rows_drawn: usize,
}

/// Side-by-side text comparison prototype.
pub struct TextCompareView {
    pub left: LoadedText,
    pub right: LoadedText,
    /// Shared vertical scroll offset (pixels) keeping both panes in lock-step.
    scroll_y: f32,
    /// Pending programmatic scroll (set by the benchmark / navigation).
    pending_scroll_row: Option<usize>,
    pub last_stats: PaneStats,
}

impl TextCompareView {
    pub fn open(left: &Path, right: &Path) -> std::io::Result<Self> {
        Ok(Self {
            left: LoadedText::open(left)?,
            right: LoadedText::open(right)?,
            scroll_y: 0.0,
            pending_scroll_row: None,
            last_stats: PaneStats::default(),
        })
    }

    /// Rows in the (unaligned) view: the longer of the two files.
    pub fn row_count(&self) -> usize {
        self.left.line_count().max(self.right.line_count())
    }

    /// Scroll so that `row` is at the top on the next frame.
    pub fn scroll_to_row(&mut self, row: usize) {
        self.pending_scroll_row = Some(row);
    }

    /// Current top row (approximate, from the scroll offset).
    pub fn top_row(&self, row_height: f32) -> usize {
        (self.scroll_y / row_height).floor().max(0.0) as usize
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let total_rows = self.row_count();

        if let Some(row) = self.pending_scroll_row.take() {
            self.scroll_y = row.min(total_rows.saturating_sub(1)) as f32 * row_height;
        }

        let mut drawn = 0usize;
        let mut new_offset: Option<f32> = None;
        let scroll_y = self.scroll_y;
        let (left, right) = (&self.left, &self.right);

        ui.columns(2, |cols| {
            for (index, (pane, text)) in cols.iter_mut().zip([left, right]).enumerate() {
                pane.label(
                    egui::RichText::new(format!(
                        "{}  ({} lines, {:.1} MB)",
                        text.path.display(),
                        text.line_count(),
                        text.byte_len() as f64 / 1e6
                    ))
                    .small(),
                );
                let output = egui::ScrollArea::vertical()
                    .id_salt(("pane", index))
                    .auto_shrink([false, false])
                    .vertical_scroll_offset(scroll_y)
                    .show_rows(pane, row_height, total_rows, |ui, range| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for row in range {
                            draw_row(ui, text, row);
                            drawn += 1;
                        }
                    });
                // Whichever pane the user scrolled wins; the other follows next frame.
                let offset_y = output.state.clamped_offset().y;
                if (offset_y - scroll_y).abs() > 0.5 {
                    new_offset = Some(offset_y);
                }
            }
        });

        if let Some(y) = new_offset {
            self.scroll_y = y;
        }
        self.last_stats = PaneStats { rows_drawn: drawn };
    }
}

fn draw_row(ui: &mut egui::Ui, text: &LoadedText, row: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let number = if row < text.line_count() { format!("{:>8}", row + 1) } else { "        ".to_string() };
        ui.add(egui::Label::new(egui::RichText::new(number).monospace().weak()).selectable(false));
        if row < text.line_count() {
            let bytes = text.line(row);
            let shown = &bytes[..bytes.len().min(MAX_RENDERED_BYTES)];
            let s = String::from_utf8_lossy(shown);
            ui.add(
                egui::Label::new(egui::RichText::new(s.as_ref()).monospace())
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .selectable(false),
            );
        }
    });
}
