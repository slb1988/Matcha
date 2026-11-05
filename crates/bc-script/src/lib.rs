//! # bc-script
//!
//! Scripting engine, command-line parsing and exit codes (FR-S1, FR-S2). The script
//! engine and the GUI share the same internal command API
//! (load → filter/criteria → select → op/report → exit code) so behaviour is identical.
//!
//! | module | source of truth |
//! |---|---|
//! | [`exit_code`] | command_line_reference "exit codes" table (`wBcExitCode`) |
//! | [`command`] | scripting_reference: the command list (29 entries; notes said 28) with argument grammars |
//! | [`parser`] | script file syntax (`#` comments, `&` continuation, quoting, `@file`) |
//! | [`cli`] | command_line_reference: switches, positional inference (`SsCmdLine`) |
//! | [`runner`] | sequential interpreter over engines/vfs/jobs (`TDIRSCRIPTDIALOG` backend) |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

pub mod cli;
pub mod command;
pub mod exit_code;
pub mod parser;
pub mod runner;

pub use cli::{CliArgs, Invocation, Switch};
pub use command::{Command, CommandKind};
pub use exit_code::ExitCode;
pub use parser::{parse_script, ScriptError, ScriptLine};
pub use runner::{Script, ScriptRunner};
