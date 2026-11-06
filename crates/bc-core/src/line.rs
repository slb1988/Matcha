//! Byte-oriented line model with per-line terminators and shared backing blocks.
//!
//! Empty input has zero lines; a final terminator does not create a phantom empty line.
//! Spans are relative to a block's backing buffer, never to a materialised whole file.

use std::{any::Any, cell::RefCell, ops::Range, path::Path, rc::Rc, sync::Arc};

use crate::edit::EditItem;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

fn revision() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Deduplicates mapping copies across current content and undo/redo snapshots.
/// Drop after conversion to release its retained original mapping handles.
#[derive(Debug, Default)]
pub struct MaterializeContext {
    buffers: HashMap<usize, (Arc<Buffer>, Arc<Buffer>)>,
}

impl MaterializeContext {
    pub fn copied_buffers(&self) -> usize {
        self.buffers.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineEnding {
    #[default]
    CrLf,
    Lf,
    Cr,
    None,
}

impl LineEnding {
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::CrLf => b"\r\n",
            Self::Lf => b"\n",
            Self::Cr => b"\r",
            Self::None => b"",
        }
    }
}

/// `Mixed` reports multiple styles; use [`LineList::dominant_ending`] for new lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndingStyle {
    #[default]
    Windows,
    Unix,
    Mac,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub span: Range<usize>,
    pub ending: LineEnding,
}

/// Actual storage selected when opening/editing a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageBackend {
    #[default]
    Owned,
    Mapped,
    Segmented,
}

#[derive(Debug)]
enum Buffer {
    Owned(Vec<u8>),
    #[cfg(windows)]
    Mapped {
        map: memmap2::Mmap,
        // Keep the no-write/no-delete sharing restriction for the full mapping lifetime.
        _file: std::fs::File,
    },
}

impl Buffer {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            #[cfg(windows)]
            Self::Mapped { map, .. } => map,
        }
    }
}

/// A slice of a shared line index and byte buffer. Splitting a piece copies neither.
#[derive(Debug, Clone)]
struct Piece {
    buffer: Arc<Buffer>,
    lines: Arc<Vec<Line>>,
    range: Range<usize>,
}

impl Piece {
    fn lines(&self) -> &[Line] {
        &self.lines[self.range.clone()]
    }
}

/// A line table with cheap immutable snapshots. Edits replace only affected pieces.
#[derive(Debug, Clone, Default)]
pub struct LineList {
    pieces: Vec<Piece>,
    /// Cumulative line counts for binary search.
    ends: Vec<usize>,
    /// CRLF / LF / CR / unterminated counts.
    ending_counts: [usize; 4],
    backend: StorageBackend,
    revision: u64,
}

impl LineList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::from_buffer(Buffer::Owned(bytes), StorageBackend::Owned)
    }

    /// Read-only mapping on Windows, held under FILE_SHARE_READ (no write/delete).
    /// A sharing conflict falls back to owned bytes; non-Windows uses owned bytes.
    /// Release all mapped snapshots before overwriting the source, or save elsewhere.
    pub fn open_mapped(path: &Path) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(path);
            match file {
                Ok(file) if file.metadata()?.len() > 0 => {
                    // SAFETY: the handle denies write/delete sharing, including existing
                    // conflicting writers, so normal file APIs cannot mutate/truncate
                    // the backing file. The read-only mapping exposes no mutable alias.
                    // The handle is retained with the map until its final Arc is dropped.
                    #[allow(unsafe_code)]
                    let map = unsafe { memmap2::Mmap::map(&file)? };
                    return Ok(Self::from_buffer(
                        Buffer::Mapped { map, _file: file },
                        StorageBackend::Mapped,
                    ));
                }
                Ok(_) => return Ok(Self::new()),
                Err(error) if error.raw_os_error() == Some(32) => {} // sharing violation
                Err(error) => return Err(error),
            }
        }
        std::fs::read(path).map(Self::from_bytes)
    }

    pub fn backend(&self) -> StorageBackend {
        self.backend
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Change backing storage only; content revisions and every line ending stay unchanged.
    pub fn materialize_owned(&mut self, context: &mut MaterializeContext) {
        for piece in &mut self.pieces {
            if !matches!(&*piece.buffer, Buffer::Owned(_)) {
                let key = Arc::as_ptr(&piece.buffer) as usize;
                let (_, owned) = context.buffers.entry(key).or_insert_with(|| {
                    (
                        piece.buffer.clone(),
                        Arc::new(Buffer::Owned(piece.buffer.bytes().to_vec())),
                    )
                });
                piece.buffer = owned.clone();
            }
        }
        self.backend = if self.pieces.len() <= 1 {
            StorageBackend::Owned
        } else {
            StorageBackend::Segmented
        };
    }

    fn from_buffer(buffer: Buffer, backend: StorageBackend) -> Self {
        let (lines, _) = LineParser::default().parse(buffer.bytes());
        let mut result = Self {
            backend,
            revision: revision(),
            ..Self::new()
        };
        for line in &lines {
            result.ending_counts[line.ending as usize] += 1;
        }
        if !lines.is_empty() {
            let len = lines.len();
            result.pieces.push(Piece {
                buffer: Arc::new(buffer),
                lines: Arc::new(lines),
                range: 0..len,
            });
            result.ends.push(len);
        }
        result
    }

    pub fn len(&self) -> usize {
        self.ends.last().copied().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn locate(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.len() {
            return None;
        }
        let piece = self.ends.partition_point(|&end| end <= index);
        let start = if piece == 0 { 0 } else { self.ends[piece - 1] };
        Some((piece, self.pieces[piece].range.start + index - start))
    }

    pub fn line_bytes(&self, index: usize) -> Option<&[u8]> {
        let (p, i) = self.locate(index)?;
        let piece = &self.pieces[p];
        Some(&piece.buffer.bytes()[piece.lines[i].span.clone()])
    }

    pub fn ending(&self, index: usize) -> Option<LineEnding> {
        let (p, i) = self.locate(index)?;
        Some(self.pieces[p].lines[i].ending)
    }

    /// Sequential access without repeated binary searches.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], LineEnding)> {
        self.pieces.iter().flat_map(|p| {
            p.lines()
                .iter()
                .map(|line| (&p.buffer.bytes()[line.span.clone()], line.ending))
        })
    }

    pub fn ending_style(&self) -> EndingStyle {
        style(&self.ending_counts)
    }

    /// Most frequent terminator, ignoring an unterminated last line.
    /// Ties (and files without any terminators) prefer CRLF, then LF, then CR.
    pub fn dominant_ending(&self) -> LineEnding {
        let mut winner = 0;
        for i in 1..3 {
            if self.ending_counts[i] > self.ending_counts[winner] {
                winner = i;
            }
        }
        [LineEnding::CrLf, LineEnding::Lf, LineEnding::Cr][winner]
    }

    /// Insert one line. Text must not contain terminators; only the last line may
    /// have no ending. Appending after an unterminated line terminates that line
    /// using the dominant style. Invalid indices/embedded terminators panic.
    pub fn insert(&mut self, index: usize, text: &[u8], ending: LineEnding) {
        assert!(index <= self.len(), "line index out of bounds");
        assert!(
            ending != LineEnding::None || index == self.len(),
            "only the last line may lack an ending"
        );
        validate_text(text);
        if index > 0 && index == self.len() && self.ending(index - 1) == Some(LineEnding::None) {
            self.set_ending(index - 1, self.dominant_ending());
        }
        self.splice(index, false, Some(owned_line(text, ending)));
    }

    pub fn insert_default(&mut self, index: usize, text: &[u8]) {
        self.insert(index, text, self.dominant_ending());
    }

    pub fn delete(&mut self, index: usize) {
        assert!(index < self.len(), "line index out of bounds");
        self.splice(index, true, None);
    }

    pub fn replace(&mut self, index: usize, text: &[u8]) {
        validate_text(text);
        let ending = self.ending(index).expect("line index out of bounds");
        self.splice(index, true, Some(owned_line(text, ending)));
    }

    pub fn set_ending(&mut self, index: usize, ending: LineEnding) {
        let (p, i) = self.locate(index).expect("line index out of bounds");
        assert!(
            ending != LineEnding::None || index + 1 == self.len(),
            "only the last line may lack an ending"
        );
        let piece = &self.pieces[p];
        let line = Line {
            span: piece.lines[i].span.clone(),
            ending,
        };
        let replacement = Piece {
            buffer: piece.buffer.clone(),
            lines: Arc::new(vec![line]),
            range: 0..1,
        };
        self.splice(index, true, Some(replacement));
    }

    /// Explicit whole-file conversion, including the unterminated last line.
    pub fn set_all_endings(&mut self, ending: LineEnding) {
        assert!(
            ending != LineEnding::None || self.len() <= 1,
            "only the last line may lack an ending"
        );
        self.backend = StorageBackend::Segmented;
        self.revision = revision();
        for piece in &mut self.pieces {
            let mut lines = piece.lines().to_vec();
            for line in &mut lines {
                line.ending = ending;
            }
            piece.range = 0..lines.len();
            piece.lines = Arc::new(lines);
        }
        self.ending_counts = [0; 4];
        self.ending_counts[ending as usize] = self.len();
    }

    fn splice(&mut self, index: usize, remove: bool, replacement: Option<Piece>) {
        self.revision = revision();
        self.backend = StorageBackend::Segmented;
        if remove {
            self.ending_counts[self.ending(index).unwrap() as usize] -= 1;
        }
        if let Some(ref piece) = replacement {
            self.ending_counts[piece.lines()[0].ending as usize] += 1;
        }
        if let Some((p, i)) = self.locate(index) {
            let old = self.pieces[p].clone();
            let mut pieces = Vec::with_capacity(3);
            if old.range.start < i {
                pieces.push(Piece {
                    range: old.range.start..i,
                    ..old.clone()
                });
            }
            pieces.extend(replacement);
            let after = i + usize::from(remove);
            if after < old.range.end {
                pieces.push(Piece {
                    range: after..old.range.end,
                    ..old
                });
            }
            self.pieces.splice(p..=p, pieces);
        } else {
            self.pieces.extend(replacement);
        }
        self.ends.clear();
        let mut end = 0;
        for piece in &self.pieces {
            end += piece.range.len();
            self.ends.push(end);
        }
    }

    /// Shared line-range snapshot (no text-byte copy).
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(
            range.start <= range.end && range.end <= self.len(),
            "invalid line range"
        );
        if range == (0..self.len()) {
            return self.clone();
        }
        let mut result = Self {
            backend: self.backend,
            revision: revision(),
            ..Self::new()
        };
        let mut start = 0;
        for piece in &self.pieces {
            let end = start + piece.range.len();
            let lo = start.max(range.start);
            let hi = end.min(range.end);
            if lo < hi {
                let mut part = piece.clone();
                part.range = piece.range.start + lo - start..piece.range.start + hi - start;
                for line in part.lines() {
                    result.ending_counts[line.ending as usize] += 1;
                }
                let total = result.len() + hi - lo;
                result.pieces.push(part);
                result.ends.push(total);
            }
            start = end;
        }
        result
    }

    /// Replace a line range by shared pieces; repair only unterminated join boundaries.
    pub fn replace_range(&mut self, range: Range<usize>, replacement: &LineList) {
        assert!(
            range.start <= range.end && range.end <= self.len(),
            "invalid line range"
        );
        let ending = self.dominant_ending();
        let before = self.slice(0..range.start);
        let after = self.slice(range.end..self.len());
        let mut result = Self {
            backend: StorageBackend::Segmented,
            revision: revision(),
            ..Self::new()
        };
        for source in [&before, replacement, &after] {
            for piece in &source.pieces {
                let end = result.len() + piece.range.len();
                result.pieces.push(piece.clone());
                result.ends.push(end);
            }
            for (count, add) in result.ending_counts.iter_mut().zip(source.ending_counts) {
                *count += add;
            }
        }
        for boundary in [before.len(), before.len() + replacement.len()] {
            if boundary > 0
                && boundary < result.len()
                && result.ending(boundary - 1) == Some(LineEnding::None)
            {
                result.set_ending(boundary - 1, ending);
            }
        }
        *self = result;
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        LineWriter::default()
            .write(self, &mut out)
            .expect("writing to Vec cannot fail");
        out
    }
}

fn validate_text(text: &[u8]) {
    assert!(
        !text.contains(&b'\r') && !text.contains(&b'\n'),
        "line text contains a terminator"
    );
}

fn owned_line(text: &[u8], ending: LineEnding) -> Piece {
    Piece {
        buffer: Arc::new(Buffer::Owned(text.to_vec())),
        lines: Arc::new(vec![Line {
            span: 0..text.len(),
            ending,
        }]),
        range: 0..1,
    }
}

/// Editor-owned target. Snapshots are Send/Sync; the live undo target is UI-thread local.
pub type SharedLines = Rc<RefCell<LineList>>;

/// Already-applied reversible line edit. Snapshots share all unaffected bytes/indices.
/// Do not mutate the target out of band while retaining its undo history.
#[derive(Debug)]
pub struct LineEdit {
    target: SharedLines,
    before: LineList,
    after: LineList,
    description: &'static str,
    replace_index: Option<usize>,
}

impl LineEdit {
    fn apply(
        target: &SharedLines,
        description: &'static str,
        replace_index: Option<usize>,
        edit: impl FnOnce(&mut LineList),
    ) -> Self {
        let mut lines = target.borrow_mut();
        let before = lines.clone();
        edit(&mut lines);
        let after = lines.clone();
        Self {
            target: target.clone(),
            before,
            after,
            description,
            replace_index,
        }
    }

    pub fn insert(target: &SharedLines, index: usize, text: &[u8], ending: LineEnding) -> Self {
        Self::apply(target, "Insert line", None, |lines| {
            lines.insert(index, text, ending)
        })
    }

    pub fn delete(target: &SharedLines, index: usize) -> Self {
        Self::apply(target, "Delete line", None, |lines| lines.delete(index))
    }

    pub fn replace(target: &SharedLines, index: usize, text: &[u8]) -> Self {
        Self::apply(target, "Replace line", Some(index), |lines| {
            lines.replace(index, text)
        })
    }

    pub fn set_ending(target: &SharedLines, index: usize, ending: LineEnding) -> Self {
        Self::apply(target, "Change line ending", None, |lines| {
            lines.set_ending(index, ending)
        })
    }

    pub fn replace_range(
        target: &SharedLines,
        range: Range<usize>,
        replacement: &LineList,
    ) -> Self {
        Self::apply(target, "Replace lines", None, |lines| {
            lines.replace_range(range, replacement)
        })
    }

    pub fn set_all_endings(target: &SharedLines, ending: LineEnding) -> Self {
        Self::apply(target, "Convert line endings", None, |lines| {
            lines.set_all_endings(ending)
        })
    }
}

impl EditItem for LineEdit {
    fn description(&self) -> String {
        self.description.into()
    }

    fn undo(&mut self) {
        *self.target.borrow_mut() = self.before.clone();
    }

    fn redo(&mut self) {
        *self.target.borrow_mut() = self.after.clone();
    }

    fn try_merge(&mut self, next: &dyn EditItem) -> bool {
        let Some(next) = next.as_any().and_then(|n| n.downcast_ref::<Self>()) else {
            return false;
        };
        if self.replace_index.is_some()
            && self.replace_index == next.replace_index
            && Rc::ptr_eq(&self.target, &next.target)
        {
            self.after = next.after.clone();
            true
        } else {
            false
        }
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn materialize(&mut self, context: &mut MaterializeContext) {
        self.target.borrow_mut().materialize_owned(context);
        self.before.materialize_owned(context);
        self.after.materialize_owned(context);
    }
}

/// Options for splitting byte buffers. CRLF is always recognised.
#[derive(Debug, Clone)]
pub struct LineParser {
    pub accept_cr: bool,
    /// Byte limit, 0 = unlimited. A cut line is returned without a terminator.
    pub limit: usize,
}

impl Default for LineParser {
    fn default() -> Self {
        Self {
            accept_cr: true,
            limit: 0,
        }
    }
}

impl LineParser {
    pub fn parse(&self, bytes: &[u8]) -> (Vec<Line>, EndingStyle) {
        let bytes = &bytes[..if self.limit == 0 {
            bytes.len()
        } else {
            self.limit.min(bytes.len())
        }];
        let mut lines = Vec::with_capacity(bytes.len() / 64);
        let mut counts = [0; 4];
        let mut start = 0;
        let mut pos = 0;
        while pos < bytes.len() {
            let (ending, width) = match bytes[pos] {
                b'\r' if bytes.get(pos + 1) == Some(&b'\n') => (LineEnding::CrLf, 2),
                b'\r' if self.accept_cr => (LineEnding::Cr, 1),
                b'\n' => (LineEnding::Lf, 1),
                _ => {
                    pos += 1;
                    continue;
                }
            };
            lines.push(Line {
                span: start..pos,
                ending,
            });
            counts[ending as usize] += 1;
            pos += width;
            start = pos;
        }
        if start < bytes.len() {
            lines.push(Line {
                span: start..bytes.len(),
                ending: LineEnding::None,
            });
        }
        (lines, style(&counts))
    }
}

fn style(counts: &[usize; 4]) -> EndingStyle {
    if counts[..3].iter().filter(|&&n| n > 0).count() > 1 {
        EndingStyle::Mixed
    } else if counts[1] > 0 {
        EndingStyle::Unix
    } else if counts[2] > 0 {
        EndingStyle::Mac
    } else {
        EndingStyle::Windows
    }
}

#[derive(Debug, Clone, Default)]
pub struct LineWriter {
    /// Override every line's ending, including an unterminated last line.
    pub force_ending: Option<LineEnding>,
}

impl LineWriter {
    pub fn write(&self, lines: &LineList, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        for (text, ending) in lines.iter() {
            out.write_all(text)?;
            out.write_all(self.force_ending.unwrap_or(ending).as_bytes())?;
        }
        Ok(())
    }
}
