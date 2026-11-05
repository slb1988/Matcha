//! `BCSessions.xml` compatibility layer (spec §5, file-formats note).
//!
//! ```xml
//! <?xml version="1.0" encoding="UTF-8"?>
//! <!-- Produced by … -->
//! <BCSessions Version="2" MinVersion="2">
//!   <TSessionFolder>
//!     <Items>
//!       <TTextCompareSession Value="name">
//!         <LastModified Value="2026-10-30 20:47:37"/>
//!         <Specs><Left Value="…"/><Right Value="…"/></Specs>
//!         <State><ShowContext Value="True"/></State>
//!       </TTextCompareSession>
//!     </Items>
//!   </TSessionFolder>
//! </BCSessions>
//! ```
//!
//! Read compatibility: unknown elements are kept in [`crate::Session::unknown`].
//! Write compatibility target: `Version`/`MinVersion` header, `&#2;` string markers,
//! `<Items><_0/>…` list style (via `bc_core::persist::write_xml`).

use crate::folder::SessionFolder;
use crate::session::Session;
use crate::SessionError;
use bc_core::persist::{Node, XmlHeader};

/// Root element name.
pub const ROOT_ELEMENT: &str = "BCSessions";

/// Header the reference tool writes.
pub fn header() -> XmlHeader {
    XmlHeader { version: 2, min_version: 2, comment: None }
}

/// Parse a complete `BCSessions.xml` document.
pub fn read_sessions(xml: &str) -> Result<SessionFolder, SessionError> {
    todo!("bc-session::xml: parse_xml → folder tree (ignore unknown elements)")
}

/// Convert one `<T…Session Value="…">` node.
pub fn session_from_node(node: &Node) -> Result<Session, SessionError> {
    todo!("bc-session::xml: class → kind, Specs, State, LastModified, retain unknown")
}

/// Build the node tree for a session.
pub fn session_to_node(session: &Session) -> Node {
    todo!("bc-session::xml: inverse of session_from_node")
}

/// Serialise the tree back to BC-compatible XML.
pub fn write_sessions(root: &SessionFolder) -> Result<String, SessionError> {
    todo!("bc-session::xml: build nodes and call bc_core::persist::write_xml")
}

/// Parse `LastModified` (`YYYY-MM-DD HH:MM:SS`, local time).
pub fn parse_last_modified(value: &str) -> Option<std::time::SystemTime> {
    todo!("bc-session::xml: timestamp parse")
}
