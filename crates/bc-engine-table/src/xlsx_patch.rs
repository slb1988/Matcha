//! Style-preserving workbook output: the center workbook package is the template.
//!
//! Every part of the center package is copied as it is — styles, themes, column widths,
//! row heights, merged cells, freeze panes, conditional formats, data validation,
//! drawings, charts, comments, defined names, VBA — except:
//!
//! * each merged sheet's `<sheetData>` is regenerated from the merge output. A cell keeps
//!   the style index it had on the side it comes from; styles only the left or right
//!   workbook has are appended to the center's `styles.xml` (fonts, fills, borders and
//!   number formats deduplicated) and re-indexed. Row attributes (height, hidden,
//!   outline, row style) come from the row's base side. Text is written as inline
//!   strings, formulas without the shared/array grouping (calamine expands them);
//! * after rows moved, row references are re-pointed ([`crate::formula_refs`]): formulas,
//!   `ref`/`sqref` attributes of the sheet (merged cells, conditional formats, data
//!   validation, hyperlinks, auto filter, selection), the sheet's table parts, defined
//!   names and chart series;
//! * sheets only the left or right has are added (from that side's sheet part, without
//!   its drawing/table/comment relationships); dropped sheets are removed;
//! * `calcChain.xml` is removed and `fullCalcOnLoad` set, so Excel recalculates;
//! * a macro workbook written as `.xlsx` loses its VBA project (content type switched).
use crate::{
    formula_refs::{cell_name, parse_cell_name, shift_formula, shift_sqref, RowMap},
    invalid,
    merge::TableMerge,
    model::CellValue,
    Cell, Result, Sheet,
};
use bc_core::Side;
use quick_xml::events::Event;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Cursor, Read, Write},
    ops::Range,
};

type Archive<'a> = zip::ZipArchive<Cursor<&'a [u8]>>;

fn err(e: impl std::fmt::Display) -> bc_core::CoreError {
    invalid(format!("cannot keep the workbook layout: {e}"))
}

// ---------------------------------------------------------------- XML helpers

fn local(name: &[u8]) -> &[u8] {
    match name.iter().position(|b| *b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// Spans of every element named `name` (any prefix, any depth): whole element.
fn spans(xml: &str, name: &str) -> Vec<Range<usize>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new(); // (start, depth)
    let mut depth = 0usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local(e.name().as_ref()) == name.as_bytes() {
                    open.push((before, depth));
                }
                depth += 1;
            }
            Ok(Event::End(e)) => {
                depth = depth.saturating_sub(1);
                if local(e.name().as_ref()) == name.as_bytes()
                    && open.last().is_some_and(|(_, d)| *d == depth)
                {
                    let (start, _) = open.pop().unwrap();
                    out.push(start..reader.buffer_position() as usize);
                }
            }
            Ok(Event::Empty(e)) => {
                if local(e.name().as_ref()) == name.as_bytes() {
                    out.push(before..reader.buffer_position() as usize);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

fn first_span(xml: &str, name: &str) -> Option<Range<usize>> {
    spans(xml, name).into_iter().next()
}

/// Direct child elements of the element at `span`.
fn children(xml: &str, span: Range<usize>) -> Vec<Range<usize>> {
    let fragment = &xml[span.clone()];
    let mut reader = quick_xml::Reader::from_str(fragment);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(_)) => {
                if depth == 1 {
                    start = before;
                }
                depth += 1;
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    out.push(span.start + start..span.start + reader.buffer_position() as usize);
                }
            }
            Ok(Event::Empty(_)) if depth == 1 => {
                out.push(span.start + before..span.start + reader.buffer_position() as usize)
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Start tag of an element: name and raw (escaped) attributes, in order.
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    /// Text after the start tag (children and end tag), empty for `<x/>`.
    rest: String,
}

fn tag_end(raw: &str) -> usize {
    let mut quote = None;
    for (i, c) in raw.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return i,
            _ => {}
        }
    }
    raw.len()
}

fn parse_tag(raw: &str) -> Tag {
    let end = tag_end(raw);
    let head = raw[1..end].trim_end_matches('/');
    let mut chars = head.char_indices().peekable();
    let mut name_end = head.len();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            name_end = i;
            break;
        }
        chars.next();
    }
    let name = head[..name_end].to_string();
    let mut attrs = Vec::new();
    let mut rest = &head[name_end..];
    loop {
        rest = rest.trim_start();
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(close) = after[1..].find(q) else {
            break;
        };
        attrs.push((key, after[1..1 + close].to_string()));
        rest = &after[close + 2..];
    }
    let self_closing = raw[..end].ends_with('/');
    Tag {
        name,
        attrs,
        rest: if self_closing {
            String::new()
        } else {
            raw[end + 1..].to_string()
        },
    }
}

impl Tag {
    fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| {
                k == key || local(k.as_bytes()) == key.as_bytes() && !k.starts_with("xmlns")
            })
            .map(|(_, v)| v.as_str())
    }
    fn set(&mut self, key: &str, value: String) {
        match self.attrs.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.attrs.push((key.to_string(), value)),
        }
    }
    fn render(&self) -> String {
        let mut out = format!("<{}", self.name);
        for (k, v) in &self.attrs {
            out.push_str(&format!(" {k}=\"{v}\""));
        }
        if self.rest.is_empty() {
            out.push_str("/>");
        } else {
            out.push('>');
            out.push_str(&self.rest);
        }
        out
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => out.push_str(&format!("_x{:04X}_", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    quick_xml::escape::unescape(text)
        .map(|t| t.into_owned())
        .unwrap_or_else(|_| text.to_string())
}

/// Prefix (`x:` or empty) of the element at `span`.
fn prefix_of(xml: &str, span: &Range<usize>) -> String {
    let name = parse_tag(&xml[span.clone()]).name;
    name.find(':')
        .map(|i| name[..=i].to_string())
        .unwrap_or_default()
}

/// Replace spans (non-overlapping) in one pass.
fn splice(xml: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    for (range, text) in edits {
        if range.start < at {
            continue;
        }
        out.push_str(&xml[at..range.start]);
        out.push_str(&text);
        at = range.end;
    }
    out.push_str(&xml[at..]);
    out
}

// ---------------------------------------------------------------- package

struct BookSheet {
    name: String,
    part: String,
}

struct Book<'a> {
    zip: Archive<'a>,
    workbook_path: String,
    rels_path: String,
    sheets: Vec<BookSheet>,
    styles_path: Option<String>,
    sheet_xml: HashMap<String, String>,
    info: HashMap<String, SheetInfo>,
}

fn read_text(zip: &mut Archive<'_>, name: &str) -> Option<String> {
    let mut entry = zip.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

fn rels_path_of(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// Resolve a relationship target against the part that owns the relationships.
fn resolve(owner: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut parts: Vec<&str> = owner.split('/').collect();
    parts.pop();
    for piece in target.split('/') {
        match piece {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            p => parts.push(p),
        }
    }
    parts.join("/")
}

struct Rel {
    id: String,
    kind: String,
    target: String,
    span: Range<usize>,
}

fn relationships(xml: &str) -> Vec<Rel> {
    spans(xml, "Relationship")
        .into_iter()
        .map(|span| {
            let tag = parse_tag(&xml[span.clone()]);
            Rel {
                id: tag.get("Id").unwrap_or_default().to_string(),
                kind: tag.get("Type").unwrap_or_default().to_string(),
                target: unescape(tag.get("Target").unwrap_or_default()),
                span,
            }
        })
        .collect()
}

impl<'a> Book<'a> {
    fn open(bytes: &'a [u8]) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(err)?;
        let root = read_text(&mut zip, "_rels/.rels").ok_or_else(|| err("no _rels/.rels"))?;
        let workbook_path = relationships(&root)
            .into_iter()
            .find(|r| r.kind.ends_with("/officeDocument"))
            .map(|r| resolve("", &r.target))
            .ok_or_else(|| err("no workbook part"))?;
        let workbook = read_text(&mut zip, &workbook_path).ok_or_else(|| err("no workbook.xml"))?;
        let rels_path = rels_path_of(&workbook_path);
        let rels =
            read_text(&mut zip, &rels_path).ok_or_else(|| err("no workbook relationships"))?;
        let rels = relationships(&rels);
        let target = |id: &str| {
            rels.iter()
                .find(|r| r.id == id)
                .map(|r| resolve(&workbook_path, &r.target))
        };
        let mut sheets = Vec::new();
        for span in spans(&workbook, "sheet") {
            let tag = parse_tag(&workbook[span]);
            let id = tag
                .attrs
                .iter()
                .find(|(k, _)| k.ends_with(":id"))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            if let Some(part) = target(&id) {
                sheets.push(BookSheet {
                    name: unescape(tag.get("name").unwrap_or_default()),
                    part,
                });
            }
        }
        let styles_path = rels
            .iter()
            .find(|r| r.kind.ends_with("/styles"))
            .map(|r| resolve(&workbook_path, &r.target));
        Ok(Self {
            zip,
            workbook_path,
            rels_path,
            sheets,
            styles_path,
            sheet_xml: HashMap::new(),
            info: HashMap::new(),
        })
    }

    /// Sheet part for a merged sheet name (a single-sheet book matches any name).
    fn part_of(&self, name: &str) -> Option<String> {
        self.sheets
            .iter()
            .find(|s| s.name == name)
            .or_else(|| (self.sheets.len() == 1).then(|| &self.sheets[0]))
            .map(|s| s.part.clone())
    }

    fn xml(&mut self, part: &str) -> Result<String> {
        if let Some(xml) = self.sheet_xml.get(part) {
            return Ok(xml.clone());
        }
        let xml = read_text(&mut self.zip, part).ok_or_else(|| err(format!("no {part}")))?;
        self.sheet_xml.insert(part.to_string(), xml.clone());
        Ok(xml)
    }

    fn info(&mut self, part: &str) -> Result<&SheetInfo> {
        if !self.info.contains_key(part) {
            let xml = self.xml(part)?;
            self.info.insert(part.to_string(), SheetInfo::parse(&xml));
        }
        Ok(&self.info[part])
    }

    fn styles(&mut self) -> Option<String> {
        let path = self.styles_path.clone()?;
        read_text(&mut self.zip, &path)
    }
}

/// What the regenerated sheet data needs from an input sheet part.
#[derive(Default)]
struct SheetInfo {
    /// Row attributes other than `r` and `spans` (raw), by 0-based row.
    rows: HashMap<usize, Vec<(String, String)>>,
    /// Cell style index by (row, column).
    styles: HashMap<(usize, usize), u32>,
    /// Columns of styled cells per row (to keep formatted blanks).
    styled_columns: HashMap<usize, Vec<usize>>,
}

impl SheetInfo {
    fn parse(xml: &str) -> Self {
        let mut info = Self::default();
        let Some(data) = first_span(xml, "sheetData") else {
            return info;
        };
        let mut reader = quick_xml::Reader::from_str(&xml[data]);
        let mut row: Option<usize> = None;
        let mut column: Option<usize> = None;
        loop {
            let event = reader.read_event();
            let tag = match &event {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => e,
                Ok(Event::Eof) | Err(_) => break,
                _ => continue,
            };
            let raw: Vec<(String, String)> = tag
                .attributes()
                .with_checks(false)
                .filter_map(|a| a.ok())
                .map(|a| {
                    (
                        String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                        String::from_utf8_lossy(&a.value).into_owned(),
                    )
                })
                .collect();
            match local(tag.name().as_ref()) {
                b"row" => {
                    let r = raw
                        .iter()
                        .find(|(k, _)| k == "r")
                        .and_then(|(_, v)| v.parse::<usize>().ok())
                        .map(|r| r - 1)
                        .unwrap_or_else(|| row.map_or(0, |r| r + 1));
                    row = Some(r);
                    column = None;
                    let attrs: Vec<_> = raw
                        .into_iter()
                        .filter(|(k, _)| k != "r" && k != "spans")
                        .collect();
                    if !attrs.is_empty() {
                        info.rows.insert(r, attrs);
                    }
                }
                b"c" => {
                    let (r, c) = raw
                        .iter()
                        .find(|(k, _)| k == "r")
                        .and_then(|(_, v)| parse_cell_name(v))
                        .unwrap_or((row.unwrap_or(0), column.map_or(0, |c| c + 1)));
                    column = Some(c);
                    if let Some(s) = raw
                        .iter()
                        .find(|(k, _)| k == "s")
                        .and_then(|(_, v)| v.parse::<u32>().ok())
                    {
                        info.styles.insert((r, c), s);
                        info.styled_columns.entry(r).or_default().push(c);
                    }
                }
                _ => {}
            }
        }
        info
    }
}

// ---------------------------------------------------------------- styles

struct Styles {
    num_fmts: Vec<(u32, String)>,
    fonts: Vec<String>,
    fills: Vec<String>,
    borders: Vec<String>,
    xfs: Vec<String>,
    style_xfs: usize,
}

fn section(xml: &str, name: &str) -> Vec<String> {
    first_span(xml, name)
        .map(|span| {
            children(xml, span)
                .into_iter()
                .map(|r| xml[r].trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

impl Styles {
    fn parse(xml: &str) -> Self {
        Self {
            num_fmts: section(xml, "numFmts")
                .iter()
                .filter_map(|raw| {
                    let tag = parse_tag(raw);
                    Some((
                        tag.get("numFmtId")?.parse().ok()?,
                        tag.get("formatCode")?.to_string(),
                    ))
                })
                .collect(),
            fonts: section(xml, "fonts"),
            fills: section(xml, "fills"),
            borders: section(xml, "borders"),
            xfs: section(xml, "cellXfs"),
            style_xfs: section(xml, "cellStyleXfs").len(),
        }
    }
}

/// The center's styles plus what the merge appends.
struct CenterStyles {
    xml: String,
    base: Styles,
    added: BTreeMap<&'static str, Vec<String>>,
    lookup: HashMap<(&'static str, String), u32>,
    next_num_fmt: u32,
    prefix: String,
}

impl CenterStyles {
    fn new(xml: String) -> Self {
        let base = Styles::parse(&xml);
        let mut lookup = HashMap::new();
        for (kind, list) in [
            ("fonts", &base.fonts),
            ("fills", &base.fills),
            ("borders", &base.borders),
            ("cellXfs", &base.xfs),
        ] {
            for (i, raw) in list.iter().enumerate() {
                lookup.entry((kind, raw.clone())).or_insert(i as u32);
            }
        }
        for (id, code) in &base.num_fmts {
            lookup.entry(("numFmts", code.clone())).or_insert(*id);
        }
        let next_num_fmt = base
            .num_fmts
            .iter()
            .map(|(id, _)| id + 1)
            .max()
            .unwrap_or(164)
            .max(164);
        let prefix = first_span(&xml, "styleSheet")
            .map(|s| prefix_of(&xml, &s))
            .unwrap_or_default();
        Self {
            xml,
            base,
            added: BTreeMap::new(),
            lookup,
            next_num_fmt,
            prefix,
        }
    }

    fn len(&self, kind: &'static str) -> usize {
        let base = match kind {
            "fonts" => self.base.fonts.len(),
            "fills" => self.base.fills.len(),
            "borders" => self.base.borders.len(),
            _ => self.base.xfs.len(),
        };
        base + self.added.get(kind).map_or(0, Vec::len)
    }

    fn intern(&mut self, kind: &'static str, raw: String) -> u32 {
        if let Some(&i) = self.lookup.get(&(kind, raw.clone())) {
            return i;
        }
        let index = self.len(kind) as u32;
        self.added.entry(kind).or_default().push(raw.clone());
        self.lookup.insert((kind, raw), index);
        index
    }

    fn num_fmt(&mut self, code: &str) -> u32 {
        if let Some(&id) = self.lookup.get(&("numFmts", code.to_string())) {
            return id;
        }
        let id = self.next_num_fmt;
        self.next_num_fmt += 1;
        self.added.entry("numFmts").or_default().push(format!(
            "<{}numFmt numFmtId=\"{id}\" formatCode=\"{code}\"/>",
            self.prefix
        ));
        self.lookup.insert(("numFmts", code.to_string()), id);
        id
    }

    fn changed(&self) -> bool {
        !self.added.is_empty()
    }

    fn render(&self) -> String {
        let mut edits = Vec::new();
        for (kind, items) in &self.added {
            let total = match *kind {
                "numFmts" => self.base.num_fmts.len() + items.len(),
                k => self.len(k),
            };
            match first_span(&self.xml, kind) {
                Some(span) => {
                    let mut tag = parse_tag(&self.xml[span.clone()]);
                    tag.set("count", total.to_string());
                    let close = format!("</{}>", tag.name);
                    let inner = if tag.rest.is_empty() {
                        String::new()
                    } else {
                        tag.rest[..tag.rest.len() - close.len()].to_string()
                    };
                    tag.rest = format!("{inner}{}{close}", items.join(""));
                    edits.push((span, tag.render()));
                }
                None if *kind == "numFmts" => {
                    if let Some(root) = first_span(&self.xml, "styleSheet") {
                        let at = root.start + tag_end(&self.xml[root]) + 1;
                        edits.push((
                            at..at,
                            format!(
                                "<{p}numFmts count=\"{total}\">{}</{p}numFmts>",
                                items.join(""),
                                p = self.prefix
                            ),
                        ));
                    }
                }
                None => {}
            }
        }
        splice(&self.xml, edits)
    }
}

/// Style index translation of one side into the center's styles.
enum StyleMap {
    Identity,
    Mapped {
        styles: Styles,
        cache: HashMap<u32, u32>,
    },
}

impl StyleMap {
    fn map(&mut self, index: u32, center: &mut CenterStyles) -> u32 {
        let StyleMap::Mapped { styles, cache } = self else {
            return index;
        };
        if let Some(&mapped) = cache.get(&index) {
            return mapped;
        }
        let Some(raw) = styles.xfs.get(index as usize) else {
            return 0;
        };
        let mut tag = parse_tag(raw);
        let id = |tag: &Tag, key: &str| tag.get(key).and_then(|v| v.parse::<usize>().ok());
        if let Some(fmt) = id(&tag, "numFmtId") {
            if let Some((_, code)) = styles.num_fmts.iter().find(|(i, _)| *i as usize == fmt) {
                let mapped = center.num_fmt(code);
                tag.set("numFmtId", mapped.to_string());
            }
        }
        for (key, kind, list) in [
            ("fontId", "fonts", &styles.fonts),
            ("fillId", "fills", &styles.fills),
            ("borderId", "borders", &styles.borders),
        ] {
            if let Some(raw) = id(&tag, key).and_then(|i| list.get(i)) {
                let mapped = center.intern(kind, raw.clone());
                tag.set(key, mapped.to_string());
            }
        }
        if id(&tag, "xfId").is_some_and(|x| x >= center.base.style_xfs) {
            tag.set("xfId", "0".into());
        }
        let mapped = center.intern("cellXfs", tag.render());
        cache.insert(index, mapped);
        mapped
    }
}

// ---------------------------------------------------------------- cells

fn number(n: f64) -> Option<String> {
    n.is_finite().then(|| {
        let text = format!("{n}");
        if text == "-0" {
            "0".into()
        } else {
            text
        }
    })
}

const DAY_MS: f64 = 86_400_000.0;
const UNIX_EPOCH_SERIAL: f64 = 25569.0;

fn write_cell(out: &mut String, p: &str, at: &str, style: Option<u32>, cell: Option<&Cell>) {
    let style = style
        .filter(|s| *s != 0)
        .map(|s| format!(" s=\"{s}\""))
        .unwrap_or_default();
    let value = cell.map(|c| &c.value).unwrap_or(&CellValue::Empty);
    let formula = cell
        .and_then(|c| c.formula.as_deref())
        .map(|f| f.trim().trim_start_matches('='))
        .filter(|f| !f.is_empty());
    let text_node = |text: &str| {
        let space = if text.trim() != text {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        format!("<{p}t{space}>{}</{p}t>", escape(text))
    };
    if let Some(formula) = formula {
        let (kind, cached) = match value {
            CellValue::Empty => ("", None),
            CellValue::Number(n) => ("", number(*n)),
            CellValue::Date(ms) => ("", number(*ms as f64 / DAY_MS + UNIX_EPOCH_SERIAL)),
            CellValue::Text(t) => (" t=\"str\"", Some(escape(t))),
            CellValue::Bool(b) => (" t=\"b\"", Some(if *b { "1" } else { "0" }.into())),
            CellValue::Error(e) => (" t=\"e\"", Some(escape(e))),
        };
        out.push_str(&format!(
            "<{p}c r=\"{at}\"{style}{kind}><{p}f>{}</{p}f>",
            escape(formula)
        ));
        if let Some(v) = cached {
            out.push_str(&format!("<{p}v>{v}</{p}v>"));
        }
        out.push_str(&format!("</{p}c>"));
        return;
    }
    match value {
        CellValue::Empty => {
            if !style.is_empty() {
                out.push_str(&format!("<{p}c r=\"{at}\"{style}/>"));
            }
        }
        CellValue::Text(t) => out.push_str(&format!(
            "<{p}c r=\"{at}\"{style} t=\"inlineStr\"><{p}is>{}</{p}is></{p}c>",
            text_node(t)
        )),
        CellValue::Number(n) => {
            if let Some(v) = number(*n) {
                out.push_str(&format!("<{p}c r=\"{at}\"{style}><{p}v>{v}</{p}v></{p}c>"));
            }
        }
        CellValue::Date(ms) => {
            if let Some(v) = number(*ms as f64 / DAY_MS + UNIX_EPOCH_SERIAL) {
                out.push_str(&format!("<{p}c r=\"{at}\"{style}><{p}v>{v}</{p}v></{p}c>"));
            }
        }
        CellValue::Bool(b) => out.push_str(&format!(
            "<{p}c r=\"{at}\"{style} t=\"b\"><{p}v>{}</{p}v></{p}c>",
            u8::from(*b)
        )),
        CellValue::Error(e) => out.push_str(&format!(
            "<{p}c r=\"{at}\"{style} t=\"e\"><{p}v>{}</{p}v></{p}c>",
            escape(e)
        )),
    }
}

// ---------------------------------------------------------------- the patch

const SIDES: [Side; 3] = [Side::Left, Side::Center, Side::Right];

fn slot(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Right => 2,
        _ => 1,
    }
}

/// Elements of a sheet part that point at relationships of their own (or at the side's
/// differential formats, which are not merged); dropped when a sheet is copied from the
/// left or right workbook (its relationships are not copied).
const REL_ELEMENTS: [&str; 10] = [
    "conditionalFormatting",
    "drawing",
    "legacyDrawing",
    "legacyDrawingHF",
    "picture",
    "oleObjects",
    "controls",
    "tableParts",
    "hyperlinks",
    "extLst",
];

const WORKSHEET_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const WORKSHEET_CONTENT: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const MACRO_MAIN: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const PLAIN_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";

/// Write `sheets` (= `merge.output()`) into a copy of the center workbook package.
/// `keep_macros` = the output may stay a macro workbook (its name is not `.xlsx`).
/// Returns the package and remarks.
pub fn patch_workbook(
    merge: &TableMerge,
    sheets: &[Sheet],
    packages: [&[u8]; 3],
    keep_macros: bool,
) -> Result<(Vec<u8>, Vec<String>)> {
    let [left, center, right] = packages;
    let mut books = [Book::open(left)?, Book::open(center)?, Book::open(right)?];
    let mut warnings = Vec::new();
    let center_styles_xml = books[1].styles();
    let mut styles = CenterStyles::new(center_styles_xml.clone().unwrap_or_default());
    let mut style_maps: Vec<StyleMap> = Vec::new();
    for slot in 0..3 {
        let own = books[slot].styles();
        style_maps.push(
            if slot == 1 || own == center_styles_xml || center_styles_xml.is_none() {
                StyleMap::Identity
            } else {
                match own {
                    Some(xml) => StyleMap::Mapped {
                        styles: Styles::parse(&xml),
                        cache: HashMap::new(),
                    },
                    None => StyleMap::Identity,
                }
            },
        );
    }
    let maps: Vec<[RowMap; 3]> = merge.sheets.iter().map(|s| s.row_maps()).collect();
    let center_map = |name: &str| {
        merge
            .sheets
            .iter()
            .position(|s| s.name == name && !s.is_dropped() && s.present[1])
            .map(|i| &maps[i][1])
    };

    let mut replaced: HashMap<String, Vec<u8>> = HashMap::new();
    let mut removed: HashSet<String> = HashSet::new();
    let mut added: Vec<(String, Vec<u8>)> = Vec::new();
    let workbook_path = books[1].workbook_path.clone();
    let rels_path = books[1].rels_path.clone();
    let mut workbook = read_text(&mut books[1].zip, &workbook_path).unwrap_or_default();
    let mut rels = read_text(&mut books[1].zip, &rels_path).unwrap_or_default();
    let mut types = read_text(&mut books[1].zip, "[Content_Types].xml").unwrap_or_default();

    let mut out_sheets = sheets.iter();
    let mut new_sheets: Vec<(String, String)> = Vec::new(); // (name, part)
    let mut dropped_parts: Vec<String> = Vec::new();
    let existing: HashSet<String> = books[1]
        .zip
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.into_owned()))
        .collect();
    for (index, sheet_merge) in merge.sheets.iter().enumerate() {
        let center_part = sheet_merge.present[1]
            .then(|| books[1].part_of(&sheet_merge.name))
            .flatten();
        if sheet_merge.is_dropped() {
            if let Some(part) = center_part {
                dropped_parts.push(part);
            }
            continue;
        }
        let out = out_sheets
            .next()
            .ok_or_else(|| err("output sheets do not match the merge"))?;
        let template_side = if sheet_merge.present[1] {
            Side::Center
        } else if sheet_merge.present[0] {
            Side::Left
        } else {
            Side::Right
        };
        let ts = slot(template_side);
        let template_part = books[ts]
            .part_of(&sheet_merge.name)
            .ok_or_else(|| err(format!("sheet {:?} not found", sheet_merge.name)))?;
        let template = books[ts].xml(&template_part)?;
        // Side parts and infos needed by this sheet.
        let mut parts: [Option<String>; 3] = [None, None, None];
        for side in SIDES {
            let s = slot(side);
            if sheet_merge.present[s] {
                parts[s] = books[s].part_of(&sheet_merge.name);
                if let Some(part) = &parts[s] {
                    books[s].info(part)?;
                }
            }
        }
        let root = first_span(&template, "worksheet").ok_or_else(|| err("no worksheet"))?;
        let p = prefix_of(&template, &root);
        // sheetData
        let mut data = String::new();
        let output_rows = sheet_merge.output_rows();
        let mut last_column = 0;
        for (k, &row) in output_rows.iter().enumerate() {
            let Some(base) = sheet_merge.row_base(row) else {
                continue;
            };
            let bs = slot(base);
            let base_row = sheet_merge.rows[row].index(base);
            let info = |s: usize| parts[s].as_ref().and_then(|part| books[s].info.get(part));
            let mut attrs: Vec<(String, String)> = base_row
                .and_then(|r| info(bs).and_then(|i| i.rows.get(&r)).cloned())
                .unwrap_or_default();
            for (key, value) in &mut attrs {
                if key == "s" {
                    if let Ok(index) = value.parse::<u32>() {
                        *value = style_maps[bs].map(index, &mut styles).to_string();
                    }
                }
            }
            let mut columns: Vec<usize> = (0..out.column_count()).collect();
            if let Some(extra) =
                base_row.and_then(|r| info(bs).and_then(|i| i.styled_columns.get(&r)))
            {
                columns.extend(extra.iter().copied().filter(|c| *c >= out.column_count()));
            }
            let mut cells = String::new();
            for column in columns {
                let cell = out.cell(k, column);
                let side = if column < sheet_merge.columns {
                    sheet_merge.cell_side(row, column).unwrap_or(base)
                } else {
                    base
                };
                let s = slot(side);
                let style = sheet_merge.rows[row]
                    .index(side)
                    .and_then(|r| info(s).and_then(|i| i.styles.get(&(r, column))).copied())
                    .or_else(|| {
                        base_row.and_then(|r| {
                            info(bs).and_then(|i| i.styles.get(&(r, column))).copied()
                        })
                    });
                let style = style.map(|index| {
                    let from = if sheet_merge.rows[row].index(side).is_some() {
                        s
                    } else {
                        bs
                    };
                    style_maps[from].map(index, &mut styles)
                });
                let before = cells.len();
                write_cell(&mut cells, &p, &cell_name(k, column), style, cell);
                if cells.len() > before {
                    last_column = last_column.max(column);
                }
            }
            if cells.is_empty() && attrs.is_empty() {
                continue;
            }
            data.push_str(&format!("<{p}row r=\"{}\"", k + 1));
            for (key, value) in &attrs {
                data.push_str(&format!(" {key}=\"{value}\""));
            }
            if cells.is_empty() {
                data.push_str("/>");
            } else {
                data.push('>');
                data.push_str(&cells);
                data.push_str(&format!("</{p}row>"));
            }
        }
        let sheet_data = format!("<{p}sheetData>{data}</{p}sheetData>");
        let map = &maps[index][ts];
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let data_span = first_span(&template, "sheetData").ok_or_else(|| err("no sheetData"))?;
        edits.push((data_span.clone(), sheet_data));
        if let Some(span) = first_span(&template, "dimension") {
            let last = cell_name(output_rows.len().saturating_sub(1), last_column);
            edits.push((span, format!("<{p}dimension ref=\"A1:{last}\"/>")));
        }
        let outside =
            |span: &Range<usize>| span.end <= data_span.start || span.start >= data_span.end;
        if template_side != Side::Center {
            for name in REL_ELEMENTS {
                for span in spans(&template, name) {
                    if outside(&span) {
                        edits.push((span, String::new()));
                    }
                }
            }
            // Column default styles belong to that side's styles.
            for span in spans(&template, "col") {
                let mut tag = parse_tag(&template[span.clone()]);
                if let Some(index) = tag.get("style").and_then(|v| v.parse::<u32>().ok()) {
                    tag.set("style", style_maps[ts].map(index, &mut styles).to_string());
                    edits.push((span, tag.render()));
                }
            }
        }
        if !map.is_identity() {
            for span in spans(&template, "mergeCells") {
                let kept: Vec<String> = children(&template, span.clone())
                    .into_iter()
                    .filter_map(|c| {
                        let mut tag = parse_tag(&template[c]);
                        let shifted = shift_sqref(tag.get("ref")?, map)?;
                        tag.set("ref", shifted);
                        Some(tag.render())
                    })
                    .collect();
                edits.push((
                    span,
                    if kept.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "<{p}mergeCells count=\"{}\">{}</{p}mergeCells>",
                            kept.len(),
                            kept.join("")
                        )
                    },
                ));
            }
            for name in [
                "conditionalFormatting",
                "dataValidation",
                "hyperlink",
                "autoFilter",
                "selection",
                "protectedRange",
                "ignoredError",
            ] {
                for span in spans(&template, name) {
                    if !outside(&span) {
                        continue;
                    }
                    let raw = &template[span.clone()];
                    let end = tag_end(raw) + 1;
                    let mut tag = parse_tag(&raw[..end]);
                    let mut changed = false;
                    for key in ["sqref", "ref", "activeCell"] {
                        if let Some(value) = tag.get(key).map(str::to_string) {
                            if let Some(shifted) = shift_sqref(&value, map) {
                                tag.set(key, shifted);
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        let start = tag.render();
                        let start = if raw[..end].ends_with("/>") {
                            start
                        } else {
                            start.trim_end_matches("/>").to_string() + ">"
                        };
                        edits.push((span.start..span.start + end, start));
                    }
                }
            }
            // Formula text inside conditional formats and data validation.
            for name in ["formula", "formula1", "formula2"] {
                for span in spans(&template, name) {
                    if !outside(&span) {
                        continue;
                    }
                    let raw = &template[span.clone()];
                    let open = tag_end(raw) + 1;
                    let Some(close) = raw.rfind("</") else {
                        continue;
                    };
                    let text = unescape(&raw[open..close]);
                    let shifted = shift_formula(&text, Some(map), &|name| center_map(name));
                    if shifted != text {
                        edits.push((span.start + open..span.start + close, escape(&shifted)));
                    }
                }
            }
        }
        let xml = splice(&template, edits);
        if template_side == Side::Center {
            replaced.insert(template_part.clone(), xml.into_bytes());
            if !map.is_identity() {
                // Tables of this sheet.
                let sheet_rels = rels_path_of(&template_part);
                if let Some(text) = read_text(&mut books[1].zip, &sheet_rels) {
                    for rel in relationships(&text) {
                        if !rel.kind.ends_with("/table") {
                            continue;
                        }
                        let path = resolve(&template_part, &rel.target);
                        if let Some(table) = read_text(&mut books[1].zip, &path) {
                            let mut edits = Vec::new();
                            for name in ["table", "autoFilter"] {
                                for span in spans(&table, name) {
                                    let raw = &table[span.clone()];
                                    let end = tag_end(raw) + 1;
                                    let mut tag = parse_tag(&raw[..end]);
                                    if let Some(shifted) =
                                        tag.get("ref").and_then(|r| shift_sqref(r, map))
                                    {
                                        tag.set("ref", shifted);
                                        let start = tag.render();
                                        let start = if raw[..end].ends_with("/>") {
                                            start
                                        } else {
                                            start.trim_end_matches("/>").to_string() + ">"
                                        };
                                        edits.push((span.start..span.start + end, start));
                                    }
                                }
                            }
                            replaced.insert(path, splice(&table, edits).into_bytes());
                        }
                    }
                }
            }
        } else {
            let mut n = 1;
            let part = loop {
                let candidate = format!("xl/worksheets/sheet_merged{n}.xml");
                if !existing.contains(&candidate) && !added.iter().any(|(p, _)| *p == candidate) {
                    break candidate;
                }
                n += 1;
            };
            added.push((part.clone(), xml.into_bytes()));
            new_sheets.push((out.name.clone(), part));
        }
    }

    // Workbook: drop and add sheets, shift defined names, recalc on load.
    let mut wb_edits: Vec<(Range<usize>, String)> = Vec::new();
    let sheet_spans = spans(&workbook, "sheet");
    let rel_list = relationships(&rels);
    let mut rel_edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut removed_indices = Vec::new();
    for (i, span) in sheet_spans.iter().enumerate() {
        let tag = parse_tag(&workbook[span.clone()]);
        let id = tag
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(":id"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let Some(rel) = rel_list.iter().find(|r| r.id == id) else {
            continue;
        };
        let part = resolve(&workbook_path, &rel.target);
        if dropped_parts.contains(&part) {
            removed_indices.push(i);
            wb_edits.push((span.clone(), String::new()));
            rel_edits.push((rel.span.clone(), String::new()));
            removed.insert(part.clone());
            removed.insert(rels_path_of(&part));
        }
    }
    if sheet_spans.len() == removed_indices.len() && new_sheets.is_empty() {
        return Err(err("every sheet was removed"));
    }
    if !new_sheets.is_empty() {
        let last = sheet_spans.last().ok_or_else(|| err("no sheets"))?;
        let sample = parse_tag(&workbook[last.clone()]);
        let id_key = sample
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(":id"))
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| "r:id".into());
        let mut next_id = sheet_spans
            .iter()
            .filter_map(|s| {
                parse_tag(&workbook[s.clone()])
                    .get("sheetId")?
                    .parse::<u32>()
                    .ok()
            })
            .max()
            .unwrap_or(0)
            + 1;
        let mut text = String::new();
        let mut rel_text = String::new();
        let wb_dir = workbook_path.rsplit_once('/').map_or("", |(d, _)| d);
        for (n, (name, part)) in new_sheets.iter().enumerate() {
            let rid = format!("rIdMatcha{}", n + 1);
            text.push_str(&format!(
                "<{} name=\"{}\" sheetId=\"{next_id}\" {id_key}=\"{rid}\"/>",
                sample.name,
                escape(name)
            ));
            next_id += 1;
            let target = part
                .strip_prefix(&format!("{wb_dir}/"))
                .map(str::to_string)
                .unwrap_or_else(|| format!("/{part}"));
            rel_text.push_str(&format!(
                "<Relationship Id=\"{rid}\" Type=\"{WORKSHEET_TYPE}\" Target=\"{target}\"/>"
            ));
        }
        wb_edits.push((last.end..last.end, text));
        if let Some(close) = rels.rfind("</Relationships>") {
            rel_edits.push((close..close, rel_text));
        }
    }
    // Defined names: local sheet indices and references.
    for span in spans(&workbook, "definedName") {
        let raw = &workbook[span.clone()];
        let open = tag_end(raw) + 1;
        let mut tag = parse_tag(&raw[..open]);
        if let Some(local_id) = tag
            .get("localSheetId")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if removed_indices.contains(&local_id) {
                wb_edits.push((span, String::new()));
                continue;
            }
            let shift = removed_indices.iter().filter(|i| **i < local_id).count();
            if shift > 0 {
                tag.set("localSheetId", (local_id - shift).to_string());
            }
        }
        let Some(close) = raw.rfind("</") else {
            continue;
        };
        let text = unescape(&raw[open..close]);
        let shifted = shift_formula(&text, None, &|name| center_map(name));
        let mut head = tag.render();
        if head.ends_with("/>") {
            head = head.trim_end_matches("/>").to_string() + ">";
        }
        wb_edits.push((span, format!("{head}{}{}", escape(&shifted), &raw[close..])));
    }
    if let Some(span) = first_span(&workbook, "workbookView") {
        let mut tag = parse_tag(&workbook[span.clone()]);
        let count = sheet_spans.len() - removed_indices.len() + new_sheets.len();
        let mut changed = false;
        for key in ["activeTab", "firstSheet"] {
            if tag
                .get(key)
                .and_then(|v| v.parse::<usize>().ok())
                .is_some_and(|v| v >= count)
            {
                tag.set(key, "0".into());
                changed = true;
            }
        }
        if changed {
            wb_edits.push((span, tag.render()));
        }
    }
    match first_span(&workbook, "calcPr") {
        Some(span) => {
            let raw = &workbook[span.clone()];
            let end = tag_end(raw) + 1;
            let mut tag = parse_tag(&raw[..end]);
            tag.set("fullCalcOnLoad", "1".into());
            let mut head = tag.render();
            if !raw[..end].ends_with("/>") {
                head = head.trim_end_matches("/>").to_string() + ">";
            }
            wb_edits.push((span.start..span.start + end, head));
        }
        None => {
            let prefix = first_span(&workbook, "workbook")
                .map(|s| prefix_of(&workbook, &s))
                .unwrap_or_default();
            let after = [
                "definedNames",
                "externalReferences",
                "functionGroups",
                "sheets",
            ]
            .iter()
            .find_map(|name| first_span(&workbook, name));
            if let Some(after) = after {
                wb_edits.push((
                    after.end..after.end,
                    format!("<{prefix}calcPr fullCalcOnLoad=\"1\"/>"),
                ));
            }
        }
    }
    workbook = splice(&workbook, wb_edits);

    // Calculation chain: stale after any cell change.
    let mut type_edits: Vec<(Range<usize>, String)> = Vec::new();
    for rel in relationships(&rels) {
        if rel.kind.ends_with("/calcChain") || (!keep_macros && rel.kind.ends_with("/vbaProject")) {
            rel_edits.push((rel.span.clone(), String::new()));
            removed.insert(resolve(&workbook_path, &rel.target));
        }
    }
    rels = splice(&rels, rel_edits);
    for span in spans(&types, "Override") {
        let tag = parse_tag(&types[span.clone()]);
        let part = tag
            .get("PartName")
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();
        if removed.contains(&part) {
            type_edits.push((span, String::new()));
        }
    }
    if let Some(close) = types.rfind("</Types>") {
        let mut text = String::new();
        for (part, _) in &added {
            text.push_str(&format!(
                "<Override PartName=\"/{part}\" ContentType=\"{WORKSHEET_CONTENT}\"/>"
            ));
        }
        type_edits.push((close..close, text));
    }
    types = splice(&types, type_edits);
    if !keep_macros && types.contains(MACRO_MAIN) {
        types = types.replace(MACRO_MAIN, PLAIN_MAIN);
        warnings.push("macros not kept: the output is .xlsx".to_string());
    }

    replaced.insert(workbook_path.clone(), workbook.into_bytes());
    replaced.insert(rels_path, rels.into_bytes());
    replaced.insert("[Content_Types].xml".into(), types.into_bytes());
    if styles.changed() {
        if let Some(path) = books[1].styles_path.clone() {
            replaced.insert(path, styles.render().into_bytes());
        }
    }
    // Chart series and other ranges in charts.
    let chart_names: Vec<String> = books[1]
        .zip
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.into_owned()))
        .filter(|n| n.starts_with("xl/charts/chart") && n.ends_with(".xml"))
        .collect();
    if maps.iter().any(|m| !m[1].is_identity()) {
        for name in chart_names {
            let Some(chart) = read_text(&mut books[1].zip, &name) else {
                continue;
            };
            let mut edits = Vec::new();
            for span in spans(&chart, "f") {
                let raw = &chart[span.clone()];
                let open = tag_end(raw) + 1;
                let Some(close) = raw.rfind("</") else {
                    continue;
                };
                let text = unescape(&raw[open..close]);
                let shifted = shift_formula(&text, None, &|name| center_map(name));
                if shifted != text {
                    edits.push((span.start + open..span.start + close, escape(&shifted)));
                }
            }
            if !edits.is_empty() {
                replaced.insert(name, splice(&chart, edits).into_bytes());
            }
        }
    }

    // Write the package.
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let zip = &mut books[1].zip;
    for i in 0..zip.len() {
        let name = zip
            .by_index_raw(i)
            .map_err(err)?
            .name()
            .map_err(err)?
            .into_owned();
        if removed.contains(&name) {
            continue;
        }
        if let Some(bytes) = replaced.remove(&name) {
            writer.start_file(name, options).map_err(err)?;
            writer.write_all(&bytes)?;
        } else {
            let entry = zip.by_index_raw(i).map_err(err)?;
            writer.raw_copy_file(entry).map_err(err)?;
        }
    }
    for (name, bytes) in added.into_iter().chain(replaced) {
        writer.start_file(name, options).map_err(err)?;
        writer.write_all(&bytes)?;
    }
    let bytes = writer.finish().map_err(err)?.into_inner();
    Ok((bytes, warnings))
}

/// Formatting of one cell as written in a package (for tests and diagnostics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellFormat {
    pub bold: bool,
    pub italic: bool,
    /// Pattern fill colour (`RRGGBB`), if any.
    pub fill: Option<String>,
    /// Number format code (custom) or `builtin:N`.
    pub num_fmt: Option<String>,
}

/// Layout of one sheet as written in a package (for tests and diagnostics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetLayout {
    pub merged: Vec<String>,
    /// Column letters with an explicit width.
    pub widths: Vec<String>,
    pub frozen: bool,
    pub has_drawing: bool,
}

fn sheet_part_xml(package: &[u8], sheet: &str) -> Result<String> {
    let mut book = Book::open(package)?;
    let part = book
        .sheets
        .iter()
        .find(|s| s.name == sheet)
        .map(|s| s.part.clone())
        .ok_or_else(|| err(format!("no sheet {sheet:?}")))?;
    book.xml(&part)
}

/// Formatting of cell `at` (`B3`) of `sheet`.
pub fn cell_format(package: &[u8], sheet: &str, at: &str) -> Result<CellFormat> {
    let xml = sheet_part_xml(package, sheet)?;
    let index = spans(&xml, "c")
        .into_iter()
        .map(|s| parse_tag(&xml[s]))
        .find(|t| t.get("r") == Some(at))
        .and_then(|t| t.get("s").and_then(|s| s.parse::<usize>().ok()))
        .unwrap_or(0);
    let mut book = Book::open(package)?;
    let styles = Styles::parse(&book.styles().unwrap_or_default());
    let Some(xf) = styles.xfs.get(index).map(|x| parse_tag(x)) else {
        return Ok(CellFormat::default());
    };
    let get = |key: &str| {
        xf.get(key)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let font = styles.fonts.get(get("fontId")).cloned().unwrap_or_default();
    let fill = styles.fills.get(get("fillId")).cloned().unwrap_or_default();
    let fill = spans(&fill, "fgColor")
        .into_iter()
        .find_map(|s| parse_tag(&fill[s]).get("rgb").map(str::to_string))
        .map(|rgb| rgb[rgb.len().saturating_sub(6)..].to_string());
    let fmt = get("numFmtId");
    let num_fmt = (fmt != 0).then(|| {
        styles
            .num_fmts
            .iter()
            .find(|(id, _)| *id as usize == fmt)
            .map(|(_, code)| unescape(code))
            .unwrap_or_else(|| format!("builtin:{fmt}"))
    });
    Ok(CellFormat {
        bold: !spans(&font, "b").is_empty(),
        italic: !spans(&font, "i").is_empty(),
        fill,
        num_fmt,
    })
}

/// Merged ranges, sized columns, frozen pane and drawing of `sheet`.
pub fn sheet_layout(package: &[u8], sheet: &str) -> Result<SheetLayout> {
    let xml = sheet_part_xml(package, sheet)?;
    let merged = spans(&xml, "mergeCell")
        .into_iter()
        .filter_map(|s| parse_tag(&xml[s]).get("ref").map(str::to_string))
        .collect();
    let mut widths = Vec::new();
    for span in spans(&xml, "col") {
        let tag = parse_tag(&xml[span]);
        if tag.get("width").is_some() {
            let min: usize = tag.get("min").and_then(|v| v.parse().ok()).unwrap_or(1);
            let max: usize = tag.get("max").and_then(|v| v.parse().ok()).unwrap_or(min);
            for c in min..=max.min(min + 64) {
                let name = cell_name(0, c - 1);
                widths.push(name.trim_end_matches('1').to_string());
            }
        }
    }
    Ok(SheetLayout {
        merged,
        widths,
        frozen: spans(&xml, "pane").into_iter().any(|s| {
            parse_tag(&xml[s])
                .get("state")
                .is_some_and(|v| v.starts_with("frozen"))
        }),
        has_drawing: !spans(&xml, "drawing").is_empty(),
    })
}

#[cfg(test)]
#[path = "xlsx_patch_tests.rs"]
mod tests;
