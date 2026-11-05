//! Delphi-style XML persistence model – BC `TSsPersistent` family.
//!
//! Every BC settings file (`BCSessions.xml`, `BCPreferences.xml`, `BCFileFormats.xml`,
//! the embedded `BCFILEFORMATS` resource, `.bcpkg` payloads) uses the same shape:
//!
//! * element name = class or property name (`<TTextCompareSession>`, `<Mask>`),
//! * scalar values in a `Value` attribute (`<TabSize Value="4"/>`),
//! * lists as `<Items>`/`<Grammar>`/… containers with `<_0/> <_1/> …` children,
//! * string values prefixed with `&#2;` (STX) – a Delphi serialisation quirk
//!   that marks *string* values (see `bc-formats` README for the open question of
//!   exactly which strings carry it),
//! * a `Version`/`MinVersion` header on the root element for user files.
//!
//! The reader is implemented (the `bc-formats` build script depends on it); the writer
//! is a scaffold until write-compatibility is validated against real BC output.

use quick_xml::events::Event;
use quick_xml::Reader;

/// Error raised while reading or writing a persistence tree.
#[derive(Debug)]
pub enum PersistError {
    /// Underlying XML syntax/encoding error.
    Xml(String),
    /// A required element or attribute was missing.
    Missing(String),
    /// A value could not be interpreted (`expected` describes the type).
    BadValue {
        element: String,
        value: String,
        expected: &'static str,
    },
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Xml(m) => write!(f, "xml: {m}"),
            PersistError::Missing(m) => write!(f, "missing: {m}"),
            PersistError::BadValue {
                element,
                value,
                expected,
            } => {
                write!(
                    f,
                    "bad value for <{element}>: {value:?} (expected {expected})"
                )
            }
        }
    }
}

impl std::error::Error for PersistError {}

/// One element of a Delphi persistence tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// Element name (class name, property name or `_N` list slot).
    pub name: String,
    /// Contents of the `Value` attribute, STX prefix removed.
    pub value: Option<String>,
    /// `true` when the `Value` carried the `&#2;` string marker.
    pub stx: bool,
    /// Child elements in document order.
    pub children: Vec<Node>,
}

impl Node {
    /// Create a node with a name only.
    pub fn new(name: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            ..Default::default()
        }
    }

    /// First child with `name`.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All children with `name`.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The `Value` attribute as a string slice.
    pub fn value_str(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// `Value` of the first child named `name`.
    pub fn child_value(&self, name: &str) -> Option<&str> {
        self.child(name).and_then(Node::value_str)
    }

    /// `Value` of child `name` parsed as Delphi boolean (`True`/`False`).
    pub fn child_bool(&self, name: &str) -> Result<Option<bool>, PersistError> {
        match self.child_value(name) {
            None => Ok(None),
            Some(v) if v.eq_ignore_ascii_case("true") => Ok(Some(true)),
            Some(v) if v.eq_ignore_ascii_case("false") => Ok(Some(false)),
            Some(v) => Err(PersistError::BadValue {
                element: name.to_string(),
                value: v.to_string(),
                expected: "True/False",
            }),
        }
    }

    /// `Value` of child `name` parsed as integer.
    pub fn child_int(&self, name: &str) -> Result<Option<i64>, PersistError> {
        match self.child_value(name) {
            None => Ok(None),
            Some(v) => v
                .trim()
                .parse::<i64>()
                .map(Some)
                .map_err(|_| PersistError::BadValue {
                    element: name.to_string(),
                    value: v.to_string(),
                    expected: "integer",
                }),
        }
    }

    /// `true` for `_0`, `_1`, … list-slot elements.
    pub fn is_item(&self) -> bool {
        self.name
            .strip_prefix('_')
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    }

    /// List-slot children (`_N`) ordered by their index.
    pub fn items(&self) -> Vec<&Node> {
        let mut items: Vec<(usize, &Node)> = self
            .children
            .iter()
            .filter(|c| c.is_item())
            .map(|c| (c.name[1..].parse::<usize>().unwrap_or(usize::MAX), c))
            .collect();
        items.sort_by_key(|(i, _)| *i);
        items.into_iter().map(|(_, n)| n).collect()
    }

    /// `Value`s of the list-slot children (string lists such as `<Description>` / `<Items>`).
    pub fn item_values(&self) -> Vec<String> {
        self.items()
            .into_iter()
            .filter_map(|n| n.value.clone())
            .collect()
    }
}

/// Parse a BC-style XML document into its root [`Node`].
///
/// Unknown elements are preserved verbatim (read-compatibility rule: ignore what you do
/// not understand, but keep it so a later writer can round-trip it).
pub fn parse_xml(xml: &str) -> Result<Node, PersistError> {
    // `&#2;` is not a legal XML 1.0 character reference; normalise it to a raw STX byte
    // before parsing and strip/flag it afterwards.
    let normalised = xml.replace("&#2;", "\u{2}");
    let mut reader = Reader::from_str(&normalised);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| PersistError::Xml(e.to_string()))?;
        match event {
            Event::Start(e) => {
                let node = node_from_start(&e)?;
                stack.push(node);
            }
            Event::Empty(e) => {
                let node = node_from_start(&e)?;
                attach(&mut stack, &mut root, node);
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| PersistError::Xml("unbalanced end tag".into()))?;
                attach(&mut stack, &mut root, node);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !stack.is_empty() {
        return Err(PersistError::Xml("unterminated element".into()));
    }
    root.ok_or_else(|| PersistError::Missing("root element".into()))
}

fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_none() {
        *root = Some(node);
    }
}

fn node_from_start(e: &quick_xml::events::BytesStart<'_>) -> Result<Node, PersistError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut node = Node::new(name);
    for attr in e.attributes() {
        let attr = attr.map_err(|e| PersistError::Xml(e.to_string()))?;
        if attr.key.as_ref() == b"Value" {
            // Plain entity unescaping, deliberately *without* attribute-value whitespace
            // normalisation: grammar texts may contain literal tabs that must survive.
            let escaped =
                std::str::from_utf8(&attr.value).map_err(|e| PersistError::Xml(e.to_string()))?;
            let raw = quick_xml::escape::unescape(escaped)
                .map_err(|e| PersistError::Xml(e.to_string()))?
                .into_owned();
            match raw.strip_prefix('\u{2}') {
                Some(rest) => {
                    node.stx = true;
                    node.value = Some(rest.to_string());
                }
                None => node.value = Some(raw),
            }
        }
    }
    Ok(node)
}

/// Header written on the root element of user files (`<BCSessions Version="2" MinVersion="2">`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlHeader {
    pub version: u32,
    pub min_version: u32,
    /// Leading comment line, e.g. the "Produced by …" banner.
    pub comment: Option<String>,
}

/// Serialise a tree back to BC-compatible XML (tabs, `Value` attributes, `&#2;` string
/// markers, `Version`/`MinVersion` header).
#[allow(unused_variables)] // remaining scaffold entry point
pub fn write_xml(root: &Node, header: Option<&XmlHeader>) -> String {
    todo!("bc-core::persist: write-compatible serialiser (spec §5 write target)")
}
