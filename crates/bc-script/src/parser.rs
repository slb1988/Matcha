//! Script text → [`Command`] list.
//!
//! Syntax (scripting_reference / "Scripts" help topics):
//! * one command per line, keywords case-insensitive;
//! * `#` starts a comment line; `comment` keyword lines are ignored too;
//! * trailing `&` continues the command on the next line;
//! * double quotes group arguments containing spaces; `%1`…`%9` substitute CLI arguments;
//! * `lt`/`rt` are synonyms of `left`/`right`.

use crate::command::{Command, CommandKind};

/// Parse failure → exit code 106 (`ScriptSyntaxError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// 1-based line of the (logical, continuation-joined) command.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ScriptError {}

/// One parsed logical line.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptLine {
    pub line: usize,
    pub raw: String,
    pub command: Command,
}

/// Parse a whole script. `args` are the `%1`…`%9` substitutions from the command line.
pub fn parse_script(source: &str, args: &[String]) -> Result<Vec<ScriptLine>, ScriptError> {
    todo!("bc-script::parser: comments, continuation, substitution, per-line parse_command")
}

/// Split one logical line into shell-like tokens (quotes, `&` already resolved).
pub fn tokenize(line: &str) -> Vec<String> {
    todo!("bc-script::parser: tokenizer")
}

/// Parse one command from its tokens.
pub fn parse_command(tokens: &[String]) -> Result<Command, String> {
    let keyword = tokens.first().ok_or("empty command")?;
    let kind = CommandKind::parse(keyword).ok_or_else(|| format!("unknown command {keyword:?}"))?;
    parse_arguments(kind, &tokens[1..])
}

/// Argument grammar per command (see `command.rs` doc comments).
pub fn parse_arguments(kind: CommandKind, args: &[String]) -> Result<Command, String> {
    match kind {
        CommandKind::Beep if args.is_empty() => Ok(Command::Beep),
        _ => todo!("bc-script::parser: argument grammar for {:?}", kind),
    }
}

/// Parse the shared report arguments
/// (`layout:` `options:` `title:` `output-to:` `output-options:` `[<comparison>]`).
pub fn parse_report_arguments(
    kind: bc_session::SessionKind,
    args: &[String],
) -> Result<bc_report::ReportRequest, String> {
    todo!("bc-script::parser: common report arguments")
}
