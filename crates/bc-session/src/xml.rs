//! `BCSessions.xml` compatibility layer (spec §5, file-formats note).
//!
//! ```xml
//! <?xml version="1.0" encoding="UTF-8"?>
//! <!-- Produced by … -->
//! <BCSessions Version="2" MinVersion="2">
//!   <TTextCompareSession Value="name">
//!     <LastModified Value="2026-10-30 20:47:37"/>
//!     <Specs><Left Value="…"/><Right Value="…"/></Specs>
//!     <Rules Value="TTextRules"><AlignKind Value="alignQuick"/>…</Rules>
//!     <State><ShowContext Value="True"/></State>
//!   </TTextCompareSession>
//!   <TSessionFolder>
//!     <Items>
//!       <TDirCompareSession Value="A &lt;--> B">…</TDirCompareSession>
//!     </Items>
//!   </TSessionFolder>
//! </BCSessions>
//! ```
//!
//! Observed in real files: the root holds sessions and folders directly; a folder's
//! entries sit in `<Items>` as class elements (not `_N` slots); sessions without a
//! `Value` are the per-kind defaults. Read compatibility: unknown classes and elements
//! are kept and written back in place, so a file produced by the reference tool
//! round-trips byte for byte apart from the banner comment and line endings.

use crate::folder::{FolderItem, SessionFolder};
use crate::session::{Session, SessionKind, SessionSpecs, Stamp};
use crate::state::SessionState;
use crate::SessionError;
use bc_core::align::AlignStrategy;
use bc_core::persist::{Node, XmlHeader};
use bc_engine_text::rules::AlignmentRules;
use bc_vfs::VfsPath;

/// Root element name.
pub const ROOT_ELEMENT: &str = "BCSessions";
const FOLDER: &str = "TSessionFolder";
const ITEMS: &str = "Items";
const LAST_MODIFIED: &str = "LastModified";
const SPECS: &str = "Specs";
const RULES: &str = "Rules";
const STATE: &str = "State";
const SIDES: [&str; 4] = ["Left", "Right", "Center", "Output"];
const ALIGN_KIND: &str = "AlignKind";
const NEVER_ALIGN: &str = "NeverAlignDiffs";
const SKEW: &str = "SkewTolerance";
const CLOSENESS: &str = "UseCloseness";

/// Header written on our files.
pub fn header() -> XmlHeader {
    XmlHeader {
        version: 2,
        min_version: 2,
        comment: Some("Produced by Matcha".into()),
    }
}

/// Parse a complete `BCSessions.xml` document.
pub fn read_sessions(xml: &str) -> Result<SessionFolder, SessionError> {
    let root = bc_core::persist::parse_xml(xml)?;
    if root.name != ROOT_ELEMENT {
        return Err(SessionError::UnknownClass(root.name));
    }
    Ok(SessionFolder {
        name: None,
        items: read_items(&root.children),
    })
}

fn read_items(nodes: &[Node]) -> Vec<FolderItem> {
    nodes
        .iter()
        .map(|node| {
            if node.name == FOLDER {
                FolderItem::Folder(folder_from_node(node))
            } else {
                match session_from_node(node) {
                    Ok(session) => FolderItem::Session(Box::new(session)),
                    Err(_) => FolderItem::Raw(node.clone()),
                }
            }
        })
        .collect()
}

fn folder_from_node(node: &Node) -> SessionFolder {
    SessionFolder {
        name: node.value.clone(),
        items: node.child(ITEMS).map(|i| read_items(&i.children)).unwrap_or_default(),
    }
}

/// Convert one `<T…Session Value="…">` node.
pub fn session_from_node(node: &Node) -> Result<Session, SessionError> {
    let kind = SessionKind::from_bc_class(&node.name)
        .ok_or_else(|| SessionError::UnknownClass(node.name.clone()))?;
    let mut session = Session::new(kind);
    session.name = node.value.clone();
    for child in &node.children {
        match child.name.as_str() {
            LAST_MODIFIED if session.last_modified.is_none() => {
                match child.value_str().and_then(parse_last_modified) {
                    Some(stamp) if child.children.is_empty() => session.last_modified = Some(stamp),
                    _ => session.unknown.push(child.clone()),
                }
            }
            SPECS => session.specs = specs_from_node(child),
            RULES if session.rules_node.is_none() => {
                if let (Some(alignment), Some(target)) =
                    (alignment_from_node(child), session.rules.alignment_mut())
                {
                    *target = alignment;
                }
                session.rules_node = Some(child.clone());
            }
            STATE => session.state = SessionState { values: child.children.clone() },
            _ => session.unknown.push(child.clone()),
        }
    }
    Ok(session)
}

fn specs_from_node(node: &Node) -> SessionSpecs {
    let mut specs = SessionSpecs::default();
    for child in &node.children {
        let slot = match SIDES.iter().position(|s| *s == child.name) {
            Some(0) => &mut specs.left,
            Some(1) => &mut specs.right,
            Some(2) => &mut specs.center,
            Some(3) => &mut specs.output,
            _ => {
                specs.extra.push(child.clone());
                continue;
            }
        };
        match child.value_str().map(VfsPath::parse) {
            Some(Ok(path)) if slot.is_none() && !child.stx && child.children.is_empty() => {
                *slot = Some(path)
            }
            _ => specs.extra.push(child.clone()),
        }
    }
    specs
}

fn align_kind_name(strategy: AlignStrategy) -> &'static str {
    match strategy {
        AlignStrategy::Quick => "alignQuick",
        AlignStrategy::Smart => "alignSmart",
        AlignStrategy::Patience => "alignPatience",
        AlignStrategy::None => "alignNone",
    }
}

/// Alignment keys of a `<Rules>` element over the defaults; `None` when a key is
/// present but unreadable (the element is then kept untouched).
fn alignment_from_node(node: &Node) -> Option<AlignmentRules> {
    let mut rules = AlignmentRules::default();
    if let Some(kind) = node.child_value(ALIGN_KIND) {
        rules.strategy = [
            AlignStrategy::Quick,
            AlignStrategy::Smart,
            AlignStrategy::Patience,
            AlignStrategy::None,
        ]
        .into_iter()
        .find(|s| align_kind_name(*s) == kind)?;
    }
    if let Some(v) = node.child_bool(NEVER_ALIGN).ok()? {
        rules.never_align_differences = v;
    }
    if let Some(v) = node.child_int(SKEW).ok()? {
        rules.skew_tolerance = u32::try_from(v).ok()?;
    }
    if let Some(v) = node.child_bool(CLOSENESS).ok()? {
        rules.use_closeness = v;
    }
    Some(rules)
}

fn value_node(name: &str, value: impl Into<String>) -> Node {
    Node {
        value: Some(value.into()),
        ..Node::new(name)
    }
}

fn delphi_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// `<Rules>` to write: the element as read when the modelled keys are unchanged,
/// otherwise that element with the four alignment keys set.
fn rules_to_node(session: &Session) -> Option<Node> {
    let Some(alignment) = session.rules.alignment() else {
        return session.rules_node.clone();
    };
    // Unreadable keys were left at the defaults on read; unchanged means untouched.
    let current = session
        .rules_node
        .as_ref()
        .and_then(alignment_from_node)
        .unwrap_or_default();
    if current == *alignment {
        return session.rules_node.clone();
    }
    let mut node = session
        .rules_node
        .clone()
        .unwrap_or_else(|| value_node(RULES, session.rules.bc_class()));
    let keys = [
        (ALIGN_KIND, align_kind_name(alignment.strategy).to_string()),
        (NEVER_ALIGN, delphi_bool(alignment.never_align_differences).to_string()),
        (SKEW, alignment.skew_tolerance.to_string()),
        (CLOSENESS, delphi_bool(alignment.use_closeness).to_string()),
    ];
    for (name, value) in keys {
        match node.children.iter_mut().find(|c| c.name == name) {
            Some(child) => {
                child.value = Some(value);
                child.stx = false;
                child.children.clear();
            }
            None => node.children.push(value_node(name, value)),
        }
    }
    Some(node)
}

/// Build the node tree for a session.
pub fn session_to_node(session: &Session) -> Node {
    let mut node = Node::new(session.kind.bc_class());
    node.value = session.name.clone();
    if let Some(stamp) = session.last_modified {
        node.children.push(value_node(LAST_MODIFIED, stamp.to_string()));
    }
    let specs = &session.specs;
    let sides = [&specs.left, &specs.right, &specs.center, &specs.output];
    let mut spec_node = Node::new(SPECS);
    for (name, path) in SIDES.iter().zip(sides) {
        if let Some(path) = path {
            spec_node.children.push(value_node(name, path.to_string()));
        }
    }
    spec_node.children.extend(specs.extra.iter().cloned());
    if !spec_node.children.is_empty() {
        node.children.push(spec_node);
    }
    node.children.extend(rules_to_node(session));
    if !session.state.is_empty() {
        node.children.push(Node {
            children: session.state.values.clone(),
            ..Node::new(STATE)
        });
    }
    node.children.extend(session.unknown.iter().cloned());
    node
}

fn items_to_nodes(items: &[FolderItem]) -> Vec<Node> {
    items
        .iter()
        .map(|item| match item {
            FolderItem::Session(s) => session_to_node(s),
            FolderItem::Raw(n) => n.clone(),
            FolderItem::Folder(f) => Node {
                value: f.name.clone(),
                children: vec![Node {
                    children: items_to_nodes(&f.items),
                    ..Node::new(ITEMS)
                }],
                ..Node::new(FOLDER)
            },
        })
        .collect()
}

/// Serialise the tree back to BC-compatible XML.
pub fn write_sessions(root: &SessionFolder) -> String {
    let node = Node {
        children: items_to_nodes(&root.items),
        ..Node::new(ROOT_ELEMENT)
    };
    bc_core::persist::write_xml(&node, Some(&header()))
}

/// Parse `LastModified` (`YYYY-MM-DD HH:MM:SS`, local time).
pub fn parse_last_modified(value: &str) -> Option<Stamp> {
    Stamp::parse(value)
}
