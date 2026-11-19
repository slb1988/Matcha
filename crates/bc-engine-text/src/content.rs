//! Decoded text, reversible line edits, immutable worker snapshots and syntax caches.
use bc_core::{
    edit::EditStack,
    encoding::{self, BomPolicy, Detection, Encoding},
    line::{LineEdit, LineEnding, LineList, LineWriter, MaterializeContext, SharedLines},
    CoreError,
};
use bc_formats::{
    grammar::{LineState, Token, Tokenizer},
    FileFormat,
};
use bc_vfs::VfsPath;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::Write,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Debug, Default)]
struct TokenData {
    tokens: BTreeMap<usize, Arc<[Token]>>,
    states: BTreeMap<usize, LineState>,
    scanned: usize,
    frontier: Option<(usize, LineState)>,
}

/// One revision's cache; shared between that revision's workers and the UI.
#[derive(Debug)]
pub struct SyntaxCache {
    tokenizer: Option<Arc<Tokenizer<'static>>>,
    data: Mutex<TokenData>,
    empty: Arc<[Token]>,
}
impl SyntaxCache {
    fn new(tokenizer: Option<Arc<Tokenizer<'static>>>) -> Self {
        Self {
            tokenizer,
            data: Mutex::new(TokenData::default()),
            empty: Arc::from([]),
        }
    }
    pub fn tokens(&self, lines: &LineList, index: usize) -> Arc<[Token]> {
        self.tokens_checked(lines, index, &bc_core::cancel::NeverCancel, true)
            .expect("not cancelled")
    }
    pub fn tokens_checked(
        &self,
        lines: &LineList,
        index: usize,
        cancel: &dyn bc_core::cancel::CancelCheck,
        retain: bool,
    ) -> Result<Arc<[Token]>, CoreError> {
        cancel.check()?;
        let Some(tokenizer) = self
            .tokenizer
            .as_ref()
            .filter(|t| !t.format().grammar.is_empty())
        else {
            return Ok(self.empty.clone());
        };
        if index >= lines.len() {
            return Ok(self.empty.clone());
        }
        let mut cache = self.data.lock().expect("syntax cache poisoned");
        if let Some(tokens) = cache.tokens.get(&index) {
            return Ok(tokens.clone());
        }
        let (mut start, mut state) = cache
            .states
            .range(..=index)
            .next_back()
            .map_or((0, LineState::default()), |(&i, s)| (i, s.clone()));
        if let Some((frontier, previous)) = &cache.frontier {
            if *frontier <= index && *frontier > start {
                start = *frontier;
                state = previous.clone();
            }
        }
        let mut result = self.empty.clone();
        for i in start..=index {
            if i % 128 == 0 {
                cancel.check()?;
            }
            let text = std::str::from_utf8(lines.line_bytes(i).unwrap()).expect("decoded UTF-8");
            let tokens = tokenizer.tokenize_line(text, &mut state);
            cache.scanned += 1;
            if (i + 1) % 256 == 0 {
                cache.states.insert(i + 1, state.clone());
            }
            if i == index {
                result = tokens.into();
            }
        }
        cache.frontier = Some((index + 1, state));
        if retain {
            cache.tokens.insert(index, result.clone());
        }
        Ok(result)
    }
    pub fn scanned_lines(&self) -> usize {
        self.data.lock().expect("syntax cache poisoned").scanned
    }
    pub fn line_weight(&self, text: &str) -> bc_core::align::LineWeight {
        self.tokenizer
            .as_ref()
            .map_or(bc_core::align::LineWeight::default(), |t| {
                t.line_weight(text)
            })
    }
}

/// Send/Sync immutable snapshot. Never send a live Rc edit target to a worker.
#[derive(Debug, Clone)]
pub struct ContentSnapshot {
    pub lines: LineList,
    pub format: Option<Arc<FileFormat>>,
    pub syntax: Arc<SyntaxCache>,
    pub format_revision: u64,
}
impl ContentSnapshot {
    pub fn revision(&self) -> (u64, u64) {
        (self.lines.revision(), self.format_revision)
    }
    pub fn tokens(&self, line: usize) -> Arc<[Token]> {
        self.syntax.tokens(&self.lines, line)
    }
}

#[derive(Debug)]
pub struct TextContent {
    pub source: Option<VfsPath>,
    pub lines: SharedLines,
    pub encoding: Encoding,
    pub detection: Option<Detection>,
    pub bom_policy: BomPolicy,
    pub decoding_errors: usize,
    pub edits: EditStack,
    pub read_only: bool,
    format: Option<Arc<FileFormat>>,
    tokenizer: Option<Arc<Tokenizer<'static>>>,
    original: LineList,
    format_revision: u64,
    cache: RefCell<Option<(u64, Arc<SyntaxCache>)>>,
}

impl TextContent {
    pub fn empty() -> Self {
        Self {
            source: None,
            lines: Rc::new(RefCell::new(LineList::new())),
            encoding: Encoding::Utf8,
            detection: None,
            bom_policy: BomPolicy::Preserve,
            decoding_errors: 0,
            edits: EditStack::new(),
            read_only: false,
            format: None,
            tokenizer: None,
            original: LineList::new(),
            format_revision: 0,
            cache: RefCell::new(None),
        }
    }
    pub fn from_bytes(bytes: Vec<u8>, format: Option<FileFormat>) -> Result<Self, CoreError> {
        Self::from_bytes_with_encoding(bytes, format, 1252, None)
    }
    pub fn from_bytes_with_encoding(
        bytes: Vec<u8>,
        format: Option<FileFormat>,
        default_code_page: u16,
        forced: Option<Encoding>,
    ) -> Result<Self, CoreError> {
        Self::from_original(
            LineList::from_bytes(bytes),
            format,
            default_code_page,
            forced,
        )
    }
    /// UTF-8 files keep their mmap backing; conversions allocate decoded UTF-8 once.
    /// Files at least this large are memory-mapped (on Windows the mapping holds the file
    /// against outside writes); smaller ones are read into memory and keep no handle, so
    /// other programs can save them while they are compared.
    pub const MAP_THRESHOLD: u64 = 64 * 1024 * 1024;

    pub fn open(
        path: &Path,
        format: Option<FileFormat>,
        default_code_page: u16,
    ) -> Result<Self, CoreError> {
        let lines = if std::fs::metadata(path)?.len() >= Self::MAP_THRESHOLD {
            LineList::open_mapped(path)?
        } else {
            LineList::from_bytes(std::fs::read(path)?)
        };
        let mut content = Self::from_original(lines, format, default_code_page, None)?;
        content.source = Some(VfsPath::local(path));
        Ok(content)
    }
    fn from_original(
        original: LineList,
        format: Option<FileFormat>,
        default_page: u16,
        forced: Option<Encoding>,
    ) -> Result<Self, CoreError> {
        let mut prefix = Vec::new();
        for (text, ending) in original.iter() {
            for bytes in [text, ending.as_bytes()] {
                prefix.extend_from_slice(&bytes[..bytes.len().min(8192 - prefix.len())]);
                if prefix.len() == 8192 {
                    break;
                }
            }
            if prefix.len() == 8192 {
                break;
            }
        }
        let mut detection = encoding::detect(&prefix, default_page);
        let valid_utf8 = original
            .iter()
            .all(|(line, _)| std::str::from_utf8(line).is_ok());
        if !detection.has_bom && detection.encoding != Encoding::Binary {
            detection.encoding = if valid_utf8 {
                Encoding::Utf8
            } else {
                Encoding::CodePage(default_page)
            };
        }
        let selected = forced.unwrap_or(detection.encoding);
        if selected == Encoding::Binary {
            return Err(CoreError::Invalid(
                "binary input: use Hex comparison".into(),
            ));
        }
        let (lines, errors) = if selected == Encoding::Utf8 && valid_utf8 {
            let mut lines = original.clone();
            if let Some(first) = lines
                .line_bytes(0)
                .and_then(|b| b.strip_prefix(b"\xef\xbb\xbf"))
            {
                let first = first.to_vec();
                if first.is_empty() && lines.len() == 1 && lines.ending(0) == Some(LineEnding::None)
                {
                    lines.delete(0);
                } else {
                    lines.replace(0, &first);
                }
            }
            (lines, 0)
        } else {
            let (text, errors) = encoding::decode_with_errors(&original.to_bytes(), selected)?;
            (LineList::from_bytes(text.into_bytes()), errors)
        };
        let mut content = Self::empty();
        content.lines = Rc::new(RefCell::new(lines));
        content.original = original;
        content.encoding = selected;
        content.detection = Some(detection);
        content.decoding_errors = errors;
        content.set_format(format)?;
        Ok(content)
    }
    pub fn format(&self) -> Option<&FileFormat> {
        self.format.as_deref()
    }
    pub fn set_format(&mut self, format: Option<FileFormat>) -> Result<(), CoreError> {
        let tokenizer = format
            .as_ref()
            .map(|f| Tokenizer::new(f).map(|t| Arc::new(t.into_owned())))
            .transpose()?;
        use std::sync::atomic::{AtomicU64, Ordering};
        static FORMAT_REVISION: AtomicU64 = AtomicU64::new(1);
        self.format_revision = FORMAT_REVISION.fetch_add(1, Ordering::Relaxed);
        self.format = format.map(Arc::new);
        self.tokenizer = tokenizer;
        *self.cache.borrow_mut() = None;
        Ok(())
    }
    pub fn revision(&self) -> (u64, u64) {
        (self.lines.borrow().revision(), self.format_revision)
    }
    pub fn snapshot(&self) -> ContentSnapshot {
        let lines = self.lines.borrow().clone();
        let mut cached = self.cache.borrow_mut();
        if cached.as_ref().map(|(revision, _)| *revision) != Some(lines.revision()) {
            *cached = Some((
                lines.revision(),
                Arc::new(SyntaxCache::new(self.tokenizer.clone())),
            ));
        }
        ContentSnapshot {
            lines,
            format: self.format.clone(),
            syntax: cached.as_ref().unwrap().1.clone(),
            format_revision: self.format_revision,
        }
    }
    /// Reload original saved bytes in another encoding, clearing incompatible edits.
    pub fn reload_encoding(&mut self, encoding: Encoding) -> Result<(), CoreError> {
        let loaded = Self::from_original(
            self.original.clone(),
            self.format().cloned(),
            1252,
            Some(encoding),
        )?;
        *self.lines.borrow_mut() = loaded.lines.borrow().clone();
        self.encoding = loaded.encoding;
        self.decoding_errors = loaded.decoding_errors;
        self.edits.clear();
        self.edits.mark_saved();
        *self.cache.borrow_mut() = None;
        Ok(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, CoreError> {
        let original_bom = self.detection.as_ref().is_some_and(|d| d.has_bom);
        if self.encoding == Encoding::Utf8 {
            let mut bytes = encoding::encode("", self.encoding, self.bom_policy, original_bom)?;
            LineWriter::default().write(&self.lines.borrow(), &mut bytes)?;
            Ok(bytes)
        } else {
            let text = self.lines.borrow().to_bytes();
            encoding::encode(
                std::str::from_utf8(&text).expect("decoded text"),
                self.encoding,
                self.bom_policy,
                original_bom,
            )
        }
    }
    pub fn replace_line(&mut self, index: usize, text: &[u8]) -> Result<(), CoreError> {
        self.ensure_writable()?;
        if index >= self.lines.borrow().len()
            || std::str::from_utf8(text).is_err()
            || text.contains(&b'\r')
            || text.contains(&b'\n')
        {
            return Err(CoreError::Invalid("invalid replacement line".into()));
        }
        self.edits
            .push(Box::new(LineEdit::replace(&self.lines, index, text)));
        Ok(())
    }
    pub fn convert_endings(&mut self, ending: LineEnding) -> Result<(), CoreError> {
        self.ensure_writable()?;
        if ending == LineEnding::None && self.lines.borrow().len() > 1 {
            return Err(CoreError::Invalid(
                "only final line may lack an ending".into(),
            ));
        }
        self.edits
            .push(Box::new(LineEdit::set_all_endings(&self.lines, ending)));
        Ok(())
    }
    pub fn ensure_writable(&self) -> Result<(), CoreError> {
        if self.read_only {
            Err(CoreError::Invalid("content is read-only".into()))
        } else {
            Ok(())
        }
    }
    pub fn is_dirty(&self) -> bool {
        self.edits.is_dirty()
    }

    /// Safe atomic save-as. Mapping locks may reject replacing the original path,
    /// leaving it intact; use materialize_and_save for same-path saving.
    pub fn save_as(&mut self, path: &Path) -> Result<(), CoreError> {
        self.ensure_writable()?;
        let bytes = self.to_bytes()?;
        atomic_save(path, &bytes)?;
        let has_bom = matches!(
            self.encoding,
            Encoding::Utf8
                | Encoding::Utf16Le
                | Encoding::Utf16Be
                | Encoding::CodePage(65001 | 1200 | 1201)
        ) && [
            b"\xef\xbb\xbf".as_slice(),
            b"\xff\xfe".as_slice(),
            b"\xfe\xff".as_slice(),
        ]
        .iter()
        .any(|bom| bytes.starts_with(bom));
        self.original = LineList::from_bytes(bytes);
        self.source = Some(VfsPath::local(path));
        self.detection = Some(Detection {
            encoding: self.encoding,
            has_bom,
            confidence: 100,
        });
        self.edits.mark_saved();
        Ok(())
    }
    /// Materialize current/original/history buffers once, preserving undo and redo.
    /// Callers MUST cancel/join workers and drop external mapped snapshots first;
    /// remaining external locks cause a safe error (no truncation), then retry.
    pub fn materialize_and_save(&mut self, path: &Path) -> Result<(), CoreError> {
        self.ensure_writable()?;
        {
            let mut context = MaterializeContext::default();
            self.materialize_owned(&mut context);
        }
        self.save_as(path)
    }
    /// Rebase this pane and its history. A view saving a source shared by copied
    /// sections must rebase ALL panes with one context, drop it, then save.
    /// Workers must already have been cancelled and joined.
    pub fn materialize_owned(&mut self, context: &mut MaterializeContext) {
        self.lines.borrow_mut().materialize_owned(context);
        self.original.materialize_owned(context);
        self.edits.materialize(context);
    }
}

fn atomic_save(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = path.with_file_name(format!(
        ".matcha-save-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| {
        CoreError::Invalid(format!(
            "save failed (release mapped worker snapshots or save elsewhere): {e}"
        ))
    })
}
