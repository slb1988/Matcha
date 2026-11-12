//! Script text → [`Command`] list.
//!
//! Syntax ("Scripts" help topic):
//! * one command per line, keywords and option names case-insensitive;
//! * `#` starts a comment (outside double quotes); blank lines are ignored;
//! * a trailing `&` continues the command on the next line (the logical line keeps the
//!   number of its first physical line);
//! * double quotes group an argument containing spaces and are not part of its value;
//! * `%1`…`%9` are the command-line arguments after the script name that do not start
//!   with `/`; `%date%`, `%time%`, `%fn_time%` are filled in when the script is parsed;
//!   `%NAME%` is an environment variable (case-sensitive; left as written when undefined).
//!
//! The whole script is parsed before anything runs. Errors come in two kinds:
//! [`ErrorKind::Syntax`] (exit 106) and [`ErrorKind::Unsupported`] (a reference command or
//! option this build does not run yet; exit 100). A syntax error anywhere wins over an
//! unsupported command, so a broken script is always reported as broken.

use crate::command::{Command, CommandKind, LogLevel, ScriptOption};
use crate::exit_code::ExitCode;
use bc_report::{
    Comparison, Display, Layout, Output, OutputOptions, PatchFormat, ReportOptions, ReportRequest,
};
use bc_session::SessionKind;
use std::path::PathBuf;

/// Why a script was rejected before running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Unknown command or invalid arguments → exit 106.
    Syntax,
    /// Valid reference syntax this build does not execute yet → exit 100.
    Unsupported,
}

/// Rejection of one command (before a line number is attached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
}

impl CommandError {
    pub fn syntax(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Syntax,
            message: message.into(),
        }
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Unsupported,
            message: message.into(),
        }
    }
}

/// Parse failure of a whole script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// 1-based line of the (logical, continuation-joined) command.
    pub line: usize,
    pub kind: ErrorKind,
    pub message: String,
}

impl ScriptError {
    pub fn exit_code(&self) -> ExitCode {
        match self.kind {
            ErrorKind::Syntax => ExitCode::ScriptSyntaxError,
            ErrorKind::Unsupported => ExitCode::UnknownError,
        }
    }
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
    /// The command after substitution, as logged by `log verbose`.
    pub raw: String,
    pub command: Command,
}

/// Values substituted into script lines.
#[derive(Debug, Clone, Default)]
pub struct Variables {
    /// `%1`…`%9` (missing ones are empty).
    pub args: Vec<String>,
    pub date: String,
    pub time: String,
    pub fn_time: String,
    /// Environment lookup (`None` keeps `%NAME%` as written).
    pub env: Option<fn(&str) -> Option<String>>,
}

impl Variables {
    /// The real values: `args` as given after the script name (switches starting with `/`
    /// are dropped), the current local date/time and the process environment.
    pub fn current(args: &[String]) -> Self {
        let now = std::time::SystemTime::now();
        let local = crate::platform::local(now);
        Variables {
            args: args
                .iter()
                .filter(|a| !a.starts_with('/'))
                .cloned()
                .collect(),
            date: local.date(),
            time: crate::platform::regional_time(now),
            fn_time: local.file_name_time(),
            env: Some(|name| std::env::var(name).ok()),
        }
    }
}

/// Parse a whole script with the real variables. `args` are the command-line arguments
/// after the script name.
pub fn parse_script(source: &str, args: &[String]) -> Result<Vec<ScriptLine>, ScriptError> {
    parse_script_with(source, &Variables::current(args))
}

/// [`parse_script`] with explicit variables (tests).
pub fn parse_script_with(source: &str, vars: &Variables) -> Result<Vec<ScriptLine>, ScriptError> {
    let mut lines = Vec::new();
    let mut unsupported: Option<ScriptError> = None;
    for (line, text) in logical_lines(source) {
        let raw = substitute(&text, vars);
        let tokens = tokenize(&raw);
        if tokens.is_empty() {
            continue;
        }
        match parse_command(&tokens) {
            Ok(command) => lines.push(ScriptLine { line, raw, command }),
            Err(e) => {
                let error = ScriptError {
                    line,
                    kind: e.kind,
                    message: e.message,
                };
                if error.kind == ErrorKind::Syntax {
                    return Err(error);
                }
                unsupported.get_or_insert(error);
            }
        }
    }
    match unsupported {
        Some(error) => Err(error),
        None => Ok(lines),
    }
}

/// Strip comments and join `&` continuations: `(first physical line, logical text)`.
pub fn logical_lines(source: &str) -> Vec<(usize, String)> {
    let mut result = Vec::new();
    let mut pending: Option<(usize, String)> = None;
    for (index, physical) in source.lines().enumerate() {
        let text = strip_comment(physical).trim_end();
        let (text, continues) = match text.strip_suffix('&') {
            Some(head) => (head, true),
            None => (text, false),
        };
        let entry = pending.get_or_insert_with(|| (index + 1, String::new()));
        if !entry.1.is_empty() {
            entry.1.push(' ');
        }
        entry.1.push_str(text.trim_start());
        if !continues {
            let (line, text) = pending.take().unwrap();
            if !text.trim().is_empty() {
                result.push((line, text));
            }
        }
    }
    if let Some((line, text)) = pending {
        if !text.trim().is_empty() {
            result.push((line, text));
        }
    }
    result
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

/// Replace `%1`…`%9`, the dynamic variables and environment variables.
pub fn substitute(line: &str, vars: &Variables) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(digit @ '1'..='9') = after.chars().next() {
            let index = digit as usize - '1' as usize;
            out.push_str(vars.args.get(index).map_or("", String::as_str));
            rest = &after[1..];
            continue;
        }
        let Some(end) = after.find('%') else {
            out.push('%');
            rest = after;
            continue;
        };
        let name = &after[..end];
        let value = match name.to_ascii_lowercase().as_str() {
            "date" => Some(vars.date.clone()),
            "time" => Some(vars.time.clone()),
            "fn_time" => Some(vars.fn_time.clone()),
            _ if !name.is_empty() && !name.contains(char::is_whitespace) => {
                vars.env.and_then(|lookup| lookup(name))
            }
            _ => None,
        };
        match value {
            Some(value) => {
                out.push_str(&value);
                rest = &after[end + 1..];
            }
            None => {
                // Not a variable: keep the `%` and rescan from the next character, so the
                // closing `%` can still open a following variable.
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Split one logical line into arguments: whitespace separates, double quotes group and
/// are removed (`output-to:"My Report.txt"` → `output-to:My Report.txt`, `""` → empty).
pub fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

/// Parse one command from its tokens.
pub fn parse_command(tokens: &[String]) -> Result<Command, CommandError> {
    let keyword = tokens
        .first()
        .ok_or_else(|| CommandError::syntax("empty command"))?;
    let kind = CommandKind::parse(keyword)
        .ok_or_else(|| CommandError::syntax(format!("unknown command {keyword:?}")))?;
    parse_arguments(kind, &tokens[1..])
}

/// Commands this build executes; every other reference command is recognised and
/// rejected as unsupported.
pub const SUPPORTED: [CommandKind; 5] = [
    CommandKind::Beep,
    CommandKind::Log,
    CommandKind::Option,
    CommandKind::TableReport,
    CommandKind::TextReport,
];

/// Argument grammar per command.
pub fn parse_arguments(kind: CommandKind, args: &[String]) -> Result<Command, CommandError> {
    match kind {
        CommandKind::Beep => match args {
            [] => Ok(Command::Beep),
            _ => Err(CommandError::syntax("beep takes no arguments")),
        },
        CommandKind::Log => parse_log(args),
        CommandKind::Option => parse_option(args),
        CommandKind::TextReport => {
            parse_report_arguments(SessionKind::TextCompare, args).map(Command::TextReport)
        }
        CommandKind::TableReport => {
            parse_report_arguments(SessionKind::TableCompare, args).map(Command::TableReport)
        }
        other => Err(CommandError::unsupported(format!(
            "the {} command is not supported yet",
            other.keyword()
        ))),
    }
}

/// `log [none|normal|verbose] [[append:]<filename>]`.
fn parse_log(args: &[String]) -> Result<Command, CommandError> {
    let mut level = None;
    let mut file = None;
    let mut append = false;
    for arg in args {
        let lower = arg.to_ascii_lowercase();
        let parsed = match lower.as_str() {
            "none" => Some(LogLevel::None),
            "normal" => Some(LogLevel::Normal),
            "verbose" => Some(LogLevel::Verbose),
            _ => None,
        };
        match parsed {
            Some(parsed) if level.is_none() && file.is_none() => level = Some(parsed),
            _ if file.is_some() => {
                return Err(CommandError::syntax(format!(
                    "unexpected log argument {arg:?}"
                )))
            }
            _ => {
                let (path, appended) = match lower.strip_prefix("append:") {
                    Some(_) => (&arg["append:".len()..], true),
                    None => (arg.as_str(), false),
                };
                if path.is_empty() {
                    return Err(CommandError::syntax("log needs a file name after append:"));
                }
                file = Some(PathBuf::from(path));
                append = appended;
            }
        }
    }
    Ok(Command::Log {
        level,
        file,
        append,
    })
}

/// `option stop-on-error` | `option confirm:(prompt|yes-to-all|no-to-all)`.
fn parse_option(args: &[String]) -> Result<Command, CommandError> {
    let [arg] = args else {
        return Err(CommandError::syntax("option takes exactly one argument"));
    };
    let option = match arg.to_ascii_lowercase().as_str() {
        "stop-on-error" => ScriptOption::StopOnError,
        "confirm:prompt" => ScriptOption::ConfirmPrompt,
        "confirm:yes-to-all" => ScriptOption::Confirm(bc_jobs::Confirmation::YesToAll),
        "confirm:no-to-all" => ScriptOption::Confirm(bc_jobs::Confirmation::NoToAll),
        _ => return Err(CommandError::syntax(format!("unknown option {arg:?}"))),
    };
    Ok(Command::Option(option))
}

/// Split `key:value` for the report keys only (so `C:\file` stays a file name).
fn report_key(arg: &str) -> Option<(&'static str, &str)> {
    let (key, value) = arg.split_once(':')?;
    let key = ["layout", "options", "title", "output-to", "output-options"]
        .into_iter()
        .find(|k| k.eq_ignore_ascii_case(key))?;
    Some((key, value))
}

/// Parse the shared report arguments
/// (`layout:` `options:` `title:` `output-to:` `output-options:` `[<comparison>]`).
///
/// Syntax is checked completely first; only then is the first unsupported feature reported,
/// so a script with both kinds of problem is reported as a syntax error.
pub fn parse_report_arguments(
    kind: SessionKind,
    args: &[String],
) -> Result<ReportRequest, CommandError> {
    let mut layout = None;
    let mut options = ReportOptions::default();
    let mut title = None;
    let mut output = None;
    let mut output_options = OutputOptions::default();
    let mut names = Vec::new();
    let mut unsupported: Option<String> = None;
    let mut defer = |message: String| {
        unsupported.get_or_insert(message);
    };
    for arg in args {
        let Some((key, value)) = report_key(arg) else {
            names.push(arg.clone());
            continue;
        };
        match key {
            "layout" => {
                let parsed = Layout::parse(value)
                    .ok_or_else(|| CommandError::syntax(format!("unknown layout {value:?}")))?;
                // The one layout each report command renders so far.
                let supported = match kind {
                    SessionKind::TableCompare => Layout::Summary,
                    _ => Layout::Patch,
                };
                if parsed != supported {
                    defer(format!(
                        "the {} layout is not supported yet; use layout:{}",
                        parsed.script_name(),
                        supported.script_name()
                    ));
                }
                layout = Some(parsed);
            }
            "options" => {
                for option in value.split(',').map(str::trim).filter(|o| !o.is_empty()) {
                    match option.to_ascii_lowercase().as_str() {
                        "patch-normal" => options.patch_format = PatchFormat::Normal,
                        "patch-context" => options.patch_format = PatchFormat::Context,
                        "patch-unified" => options.patch_format = PatchFormat::Unified,
                        "ignore-unimportant" => options.ignore_unimportant = true,
                        "display-all" => options.display = Display::All,
                        "display-mismatches" => options.display = Display::Mismatches,
                        "display-matches" => options.display = Display::Matches,
                        "display-context" => options.display = Display::Context,
                        "line-numbers" => options.line_numbers = true,
                        "strikeout-left-diffs" => options.strikeout_left_diffs = true,
                        "strikeout-right-diffs" => options.strikeout_right_diffs = true,
                        // Deprecated; no effect on the patch layout.
                        "stats-descriptive" | "stats-tabular" => {}
                        _ => {
                            return Err(CommandError::syntax(format!(
                                "unknown report option {option:?}"
                            )))
                        }
                    }
                }
            }
            "title" => title = Some(value.to_string()),
            "output-to" => {
                output = Some(match value.to_ascii_lowercase().as_str() {
                    "" => return Err(CommandError::syntax("output-to needs a target")),
                    "-" => Output::Stdout,
                    "printer" => {
                        defer("printer output is not supported yet".into());
                        Output::Printer
                    }
                    "clipboard" => {
                        defer("clipboard output is not supported yet".into());
                        Output::Clipboard
                    }
                    _ => Output::File(PathBuf::from(value)),
                });
            }
            "output-options" => {
                for option in value.split(',').map(str::trim).filter(|o| !o.is_empty()) {
                    let lower = option.to_ascii_lowercase();
                    match lower.as_str() {
                        "print-color" => output_options.print_color = true,
                        "print-mono" => output_options.print_color = false,
                        "print-landscape" => output_options.landscape = true,
                        "print-portrait" => output_options.landscape = false,
                        "wrap-none" => output_options.wrap = bc_report::Wrap::None,
                        "wrap-character" => output_options.wrap = bc_report::Wrap::Character,
                        "wrap-word" => output_options.wrap = bc_report::Wrap::Word,
                        "html-color" | "html-mono" => {
                            defer("HTML output is not supported yet".into())
                        }
                        _ if lower.starts_with("html-custom:") => {
                            defer("HTML output is not supported yet".into())
                        }
                        _ => {
                            return Err(CommandError::syntax(format!(
                                "unknown output option {option:?}"
                            )))
                        }
                    }
                }
            }
            _ => unreachable!("report_key returns known keys only"),
        }
    }
    let layout = layout.ok_or_else(|| CommandError::syntax("missing layout:"))?;
    let output = output.ok_or_else(|| CommandError::syntax("missing output-to:"))?;
    let comparison = match names.as_slice() {
        [] => {
            defer("a report without a comparison needs load/select, which are not supported yet; give two file names".into());
            None
        }
        [session] => {
            defer(format!(
                "reports on a saved session ({session:?}) are not supported yet; give two file names"
            ));
            Some(Comparison::Session(session.clone()))
        }
        [left, right] => Some(Comparison::Files(left.into(), right.into())),
        _ => {
            return Err(CommandError::syntax(format!(
                "too many arguments: {}",
                names.join(" ")
            )))
        }
    };
    if let Some(message) = unsupported {
        return Err(CommandError::unsupported(message));
    }
    Ok(ReportRequest {
        kind,
        layout,
        options,
        title,
        output,
        output_options,
        comparison,
    })
}

/// The `/report` command-line shortcut as a script `text-report` would read it: same
/// validation, same request, hence the same bytes. `layout` is the `/report=` value
/// (`patch-unified` & co. expand to `layout:patch` plus the format option); `file: None`
/// writes to standard output.
pub fn shortcut_report(
    layout: &str,
    options: Option<&str>,
    file: Option<&std::path::Path>,
    left: &str,
    right: &str,
) -> Result<ReportRequest, CommandError> {
    let lower = layout.to_ascii_lowercase();
    let (layout, format) = match lower.as_str() {
        "patch-normal" | "patch-context" | "patch-unified" => ("patch", Some(lower.as_str())),
        _ => (layout, None),
    };
    let options: Vec<&str> = format.into_iter().chain(options).collect();
    let mut args = vec![format!("layout:{layout}")];
    if !options.is_empty() {
        args.push(format!("options:{}", options.join(",")));
    }
    args.push(match file {
        Some(file) => format!("output-to:{}", file.display()),
        None => "output-to:-".into(),
    });
    args.push(left.into());
    args.push(right.into());
    parse_report_arguments(SessionKind::TextCompare, &args)
}

/// The `/report` command line as the script command it stands for:
///
/// * `summary` → `table-report layout:summary` (always a table comparison);
/// * a `patch…` layout on an xlsx/xlsm file → the same table report, since a binary
///   workbook has no text to patch;
/// * anything else → `text-report`, as [`shortcut_report`].
pub fn shortcut_command(
    layout: &str,
    options: Option<&str>,
    file: Option<&std::path::Path>,
    left: &str,
    right: &str,
) -> Result<Command, CommandError> {
    let lower = layout.to_ascii_lowercase();
    let workbook = crate::table_report::is_workbook(std::path::Path::new(left))
        || crate::table_report::is_workbook(std::path::Path::new(right));
    let table = lower == "summary" || (workbook && lower.starts_with("patch"));
    if !table {
        return shortcut_report(layout, options, file, left, right).map(Command::TextReport);
    }
    let mut args = vec!["layout:summary".to_string()];
    if let Some(options) = options {
        args.push(format!("options:{options}"));
    }
    args.push(match file {
        Some(file) => format!("output-to:{}", file.display()),
        None => "output-to:-".into(),
    });
    args.push(left.into());
    args.push(right.into());
    parse_report_arguments(SessionKind::TableCompare, &args).map(Command::TableReport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_shortcut_equals_the_script_line() {
        let script = report(
            "text-report layout:patch options:patch-unified,ignore-unimportant output-to:- a b",
        );
        let shortcut =
            shortcut_report("patch-unified", Some("ignore-unimportant"), None, "a", "b").unwrap();
        assert_eq!(shortcut, script);
        let file = shortcut_report(
            "PATCH",
            None,
            Some(std::path::Path::new("out.patch")),
            "a",
            "b",
        )
        .unwrap();
        assert_eq!(
            file,
            report("text-report layout:patch output-to:out.patch a b")
        );
        let e = shortcut_report("side-by-side", None, None, "a", "b").unwrap_err();
        assert_eq!(e.kind, ErrorKind::Unsupported);
        let e = shortcut_report("fancy", None, None, "a", "b").unwrap_err();
        assert_eq!(e.kind, ErrorKind::Syntax);
        let e = shortcut_report("patch", Some("shiny"), None, "a", "b").unwrap_err();
        assert_eq!(e.kind, ErrorKind::Syntax);
    }

    fn vars(args: &[&str]) -> Variables {
        Variables {
            args: args.iter().map(|s| s.to_string()).collect(),
            date: "2026-11-11".into(),
            time: "15:41:49".into(),
            fn_time: "15-41-49".into(),
            env: Some(|name| (name == "TEMP").then(|| r"C:\Temp".to_string())),
        }
    }

    fn parse(source: &str) -> Result<Vec<ScriptLine>, ScriptError> {
        parse_script_with(source, &vars(&["left.txt", "right.txt"]))
    }

    fn report(source: &str) -> ReportRequest {
        match &parse(source).unwrap()[0].command {
            Command::TextReport(request) => request.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn error(source: &str) -> ScriptError {
        parse(source).unwrap_err()
    }

    #[test]
    fn continuation_quotes_and_comments() {
        let lines = parse(
            "# 生成补丁\n\nTEXT-REPORT layout:patch &\n  output-to:\"My Report.txt\" \"a b.txt\" c.txt  # tail\n",
        )
        .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line, 3);
        let Command::TextReport(request) = &lines[0].command else {
            panic!()
        };
        assert_eq!(request.output, Output::File("My Report.txt".into()));
        assert_eq!(
            request.comparison,
            Some(Comparison::Files("a b.txt".into(), "c.txt".into()))
        );
    }

    #[test]
    fn hash_inside_quotes_is_kept() {
        let request = report("text-report layout:patch output-to:\"a#b.patch\" x y");
        assert_eq!(request.output, Output::File("a#b.patch".into()));
    }

    #[test]
    fn tokenizer_keeps_empty_quoted_arguments() {
        assert_eq!(tokenize(r#"a "" "b c" d"e f"g"#), ["a", "", "b c", "de fg"]);
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn substitution() {
        let v = vars(&["one"]);
        assert_eq!(substitute("\"%1\" \"%2\"", &v), "\"one\" \"\"");
        assert_eq!(substitute("%TEMP%\\r.txt", &v), "C:\\Temp\\r.txt");
        assert_eq!(
            substitute("%temp%\\r.txt", &v),
            "%temp%\\r.txt",
            "env names are case-sensitive"
        );
        assert_eq!(substitute("%UNDEFINED%", &v), "%UNDEFINED%");
        assert_eq!(
            substitute("r-%date%_%fn_time%.patch", &v),
            "r-2026-11-11_15-41-49.patch"
        );
        assert_eq!(substitute("%TIME%", &v), "15:41:49");
        assert_eq!(substitute("100% done", &v), "100% done");
        assert_eq!(substitute("50% and %TEMP%", &v), "50% and C:\\Temp");
    }

    #[test]
    fn current_variables_drop_switches_from_arguments() {
        let v = Variables::current(&["/silent".into(), "left.txt".into(), "right.txt".into()]);
        assert_eq!(v.args, ["left.txt", "right.txt"]);
        assert_eq!(v.date.len(), 10);
        assert_eq!(&v.date[4..5], "-");
    }

    #[test]
    fn command_line_arguments_reach_the_comparison() {
        let request = report("text-report layout:patch output-to:- \"%1\" \"%2\"");
        assert_eq!(
            request.comparison,
            Some(Comparison::Files("left.txt".into(), "right.txt".into()))
        );
        assert_eq!(request.output, Output::Stdout);
    }

    #[test]
    fn misspelled_command_is_a_syntax_error_with_its_line() {
        let e = error("log normal\n\ntext-reprot layout:patch output-to:- a b");
        assert_eq!((e.line, e.kind), (3, ErrorKind::Syntax));
        assert_eq!(e.exit_code(), ExitCode::ScriptSyntaxError);
        assert!(e.to_string().starts_with("line 3: "), "{e}");
    }

    #[test]
    fn reference_commands_outside_this_build_are_unsupported() {
        let e = error("load \"C:\\left\" \"C:\\right\"\ntext-report layout:patch output-to:- a b");
        assert_eq!((e.line, e.kind), (1, ErrorKind::Unsupported));
        assert_eq!(e.exit_code(), ExitCode::UnknownError);
        assert!(e.message.contains("load"), "{}", e.message);
        let rejected: Vec<_> = CommandKind::ALL
            .into_iter()
            .filter(|kind| !SUPPORTED.contains(kind))
            .collect();
        assert_eq!(rejected.len(), 24);
        for kind in rejected {
            let e = parse_arguments(kind, &[]).unwrap_err();
            assert_eq!(e.kind, ErrorKind::Unsupported, "{kind:?}");
        }
        assert_eq!(
            parse_arguments(CommandKind::FileReport, &["layout:summary".into()])
                .unwrap_err()
                .kind,
            ErrorKind::Unsupported
        );
    }

    #[test]
    fn a_syntax_error_anywhere_wins_over_unsupported() {
        let e = error("load x\nbeep extra");
        assert_eq!((e.line, e.kind), (2, ErrorKind::Syntax));
    }

    #[test]
    fn report_layouts() {
        assert_eq!(
            error("text-report layout:patch a b").kind,
            ErrorKind::Syntax,
            "missing output-to"
        );
        assert_eq!(
            error("text-report output-to:- a b").kind,
            ErrorKind::Syntax,
            "missing layout"
        );
        assert_eq!(
            error("text-report layout:fancy output-to:- a b").kind,
            ErrorKind::Syntax
        );
        for layout in [
            "side-by-side",
            "summary",
            "interleaved",
            "statistics",
            "xml",
            "over-under",
            "composite",
        ] {
            let e = error(&format!(
                "text-report layout:{layout} output-to:r.txt a.txt b.txt"
            ));
            assert_eq!(e.kind, ErrorKind::Unsupported, "{layout}");
            assert!(e.message.contains("not supported yet"), "{}", e.message);
        }
    }

    #[test]
    fn report_options() {
        let request = report(
            "text-report layout:patch options:patch-unified,ignore-unimportant output-to:- a b",
        );
        assert_eq!(request.options.patch_format, PatchFormat::Unified);
        assert!(request.options.ignore_unimportant);
        assert_eq!(
            report("text-report layout:patch output-to:- a b")
                .options
                .patch_format,
            PatchFormat::Normal
        );
        let accepted = report(
            "text-report layout:patch options:Patch-Context,line-numbers,display-mismatches,strikeout-left-diffs,strikeout-right-diffs,stats-descriptive,stats-tabular output-to:- a b",
        );
        assert_eq!(accepted.options.patch_format, PatchFormat::Context);
        assert_eq!(
            error("text-report layout:patch options:patch-git output-to:- a b").kind,
            ErrorKind::Syntax
        );
        assert_eq!(
            report("text-report layout:patch title:\"My Title\" output-to:- a b")
                .title
                .as_deref(),
            Some("My Title")
        );
    }

    #[test]
    fn report_targets_and_output_options() {
        assert_eq!(
            report("text-report layout:patch output-to:- a b").output,
            Output::Stdout
        );
        assert_eq!(
            report("text-report layout:patch output-to:\"%TEMP%\\report.txt\" a b").output,
            Output::File(r"C:\Temp\report.txt".into())
        );
        for target in ["printer", "clipboard", "CLIPBOARD"] {
            let e = error(&format!(
                "text-report layout:patch output-to:{target} a.txt b.txt"
            ));
            assert_eq!(e.kind, ErrorKind::Unsupported, "{target}");
        }
        for html in ["html-color", "html-mono", "html-custom:style.css"] {
            let e = error(&format!(
                "text-report layout:patch output-to:r.html output-options:{html} a b"
            ));
            assert_eq!(e.kind, ErrorKind::Unsupported, "{html}");
            assert!(e.message.contains("HTML"));
        }
        let accepted = report(
            "text-report layout:patch output-to:r.txt output-options:wrap-word,print-color,print-landscape,print-portrait,print-mono,wrap-none,wrap-character a b",
        );
        assert_eq!(accepted.output, Output::File("r.txt".into()));
        assert_eq!(
            error("text-report layout:patch output-to:r.txt output-options:shiny a b").kind,
            ErrorKind::Syntax
        );
    }

    #[test]
    fn report_comparison_forms() {
        let e = error("text-report layout:patch output-to:- \"My Session\"");
        assert_eq!(e.kind, ErrorKind::Unsupported);
        assert!(e.message.contains("session"), "{}", e.message);
        assert_eq!(
            error("text-report layout:patch output-to:-").kind,
            ErrorKind::Unsupported
        );
        assert_eq!(
            error("text-report layout:patch output-to:- a b c").kind,
            ErrorKind::Syntax
        );
        // `C:\x` is a file name, not a `C:` key.
        assert_eq!(
            report(r"text-report layout:patch output-to:- C:\a.txt D:\b.txt").comparison,
            Some(Comparison::Files(r"C:\a.txt".into(), r"D:\b.txt".into()))
        );
    }

    #[test]
    fn log_option_and_beep() {
        let commands: Vec<_> = parse(
            "log normal\nlog verbose append:run.log\nlog none out.log\noption stop-on-error\noption confirm:yes-to-all\noption confirm:prompt\noption confirm:no-to-all\nbeep",
        )
        .unwrap()
        .into_iter()
        .map(|l| l.command)
        .collect();
        assert_eq!(
            commands,
            [
                Command::Log {
                    level: Some(LogLevel::Normal),
                    file: None,
                    append: false
                },
                Command::Log {
                    level: Some(LogLevel::Verbose),
                    file: Some("run.log".into()),
                    append: true
                },
                Command::Log {
                    level: Some(LogLevel::None),
                    file: Some("out.log".into()),
                    append: false
                },
                Command::Option(ScriptOption::StopOnError),
                Command::Option(ScriptOption::Confirm(bc_jobs::Confirmation::YesToAll)),
                Command::Option(ScriptOption::ConfirmPrompt),
                Command::Option(ScriptOption::Confirm(bc_jobs::Confirmation::NoToAll)),
                Command::Beep,
            ]
        );
        for bad in [
            "option confirm:maybe",
            "option",
            "option stop-on-error extra",
            "beep now",
            "log normal a.log b.log",
            "log append:",
        ] {
            assert_eq!(error(bad).kind, ErrorKind::Syntax, "{bad}");
        }
    }
}
