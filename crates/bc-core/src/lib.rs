//! # bc-core
//!
//! Foundation layer of the comparison engine. Mirrors the Delphi `Ss*` infrastructure
//! units of the reference implementation: line model, diff/alignment, hashing,
//! encoding, regular-expression wrapper, undo stack and the Delphi-style XML
//! persistence model used by every `*.xml` settings file.
//!
//! Scaffold stage: every algorithmic entry point is `todo!()`. Only [`persist`] has a
//! working reader because the `bc-formats` build script needs it at build time.
//!
//! | module | BC classes |
//! |---|---|
//! | [`line`] | `TSsLineList`, `TSsLineParser`, `TSsLineWriter`, `TLineConLineList` |
//! | [`diff`] | (algorithm layer feeding `TBcAlign`) |
//! | [`align`] | `TBcAlign`, `TBcQuickAlign`, `TBcSmartAlign`, `TBcPatienceDiffAlign`, `TBcAlignLines`, `TBcByteAlign`, `TBcIntAlign` |
//! | [`edit`] | `TSsEditStack`, `TSsEditItem`, `TSsEditParentItem`, `TSsLineList*Item` |
//! | [`hash`] | xxHash line hashing / CRC helpers (`TVfsGetCRC32Job` consumer side) |
//! | [`encoding`] | code page detection & BOM handling (`UiCodePageEdit`, `TFILECODEPAGEERRORDIALOG` backend) |
//! | [`pattern`] | `TSsRegExpr`, `TREParser` state tables |
//! | [`persist`] | `TSsPersistent`, `TSsPersistentList`, `TSsPersistentObjList`, `TSsPersistentParent(Ex)` |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

pub mod align;
pub mod diff;
pub mod edit;
pub mod encoding;
pub mod hash;
pub mod line;
pub mod pattern;
pub mod persist;

/// Which side of a comparison a value belongs to.
///
/// BC uses `left`/`right`/`center`/`output` throughout its UI, scripting (`lt`/`rt`) and
/// session XML (`<Specs><Left/><Right/>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    /// Merge base (3-way) – BC `/center=`.
    Center,
    /// Merge result pane – BC `/mergeoutput=`.
    Output,
}

impl Side {
    /// All four sides in BC's pane order.
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Center, Side::Output];
}

/// Result of comparing two aligned items (one row of an [`align::AlignMap`]).
///
/// The *importance* refinement (important/unimportant) lives in `bc-engine-text`;
/// the core only knows structural outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiffKind {
    /// Both sides present and identical.
    #[default]
    Same,
    /// Both sides present and different.
    Changed,
    /// Only present on the left side.
    LeftOnly,
    /// Only present on the right side.
    RightOnly,
}

/// Error type shared by the core primitives.
#[derive(Debug)]
pub enum CoreError {
    /// Input is not valid for the requested operation.
    Invalid(String),
    /// Wraps a persistence (XML) error.
    Persist(persist::PersistError),
    /// I/O failure.
    Io(std::io::Error),
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::Invalid(m) => write!(f, "invalid input: {m}"),
            CoreError::Persist(e) => write!(f, "persistence error: {e}"),
            CoreError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e)
    }
}

impl From<persist::PersistError> for CoreError {
    fn from(e: persist::PersistError) -> Self {
        CoreError::Persist(e)
    }
}
