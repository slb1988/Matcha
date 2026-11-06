//! Internal operation/select command parsing. Not an application CLI or script runner.
use crate::{
    filter::FilterClock,
    filter_script::{parse_date, words},
    ops::*,
    view::ViewOptions,
    CompareStatus, DirTree,
};
use bc_core::Side;
use bc_vfs::FileAttrs;

fn invalid(message: impl Into<String>) -> PlanError {
    PlanError::Invalid(message.into())
}
pub fn parse_target(text: &str) -> Result<Target, PlanError> {
    match text.to_ascii_lowercase().as_str() {
        "left" => Ok(Target::Left),
        "right" => Ok(Target::Right),
        "all" => Ok(Target::All),
        _ => Err(invalid("expected left/right/all")),
    }
}
pub fn parse_direction(text: &str) -> Result<(Side, Side), PlanError> {
    match text.to_ascii_lowercase().as_str() {
        "left->right" => Ok((Side::Left, Side::Right)),
        "right->left" => Ok((Side::Right, Side::Left)),
        _ => Err(invalid("expected left->right/right->left")),
    }
}
impl DirOp {
    pub fn parse_script(input: &str, clock: FilterClock) -> Result<Self, PlanError> {
        let tokens = words(input).map_err(invalid)?;
        let command = tokens
            .first()
            .ok_or_else(|| invalid("empty command"))?
            .to_ascii_lowercase();
        let args = &tokens[1..];
        match command.as_str() {
            "copy" | "move" => {
                if args.len() != 1 {
                    return Err(invalid("transfer needs one direction"));
                }
                let (from, to) = parse_direction(&args[0])?;
                Ok(if command == "copy" {
                    Self::Copy { from, to }
                } else {
                    Self::Move { from, to }
                })
            }
            "copyto" | "moveto" => {
                let mut target = Target::All;
                let mut path_mode = PathMode::None;
                let mut dest = None;
                for arg in args {
                    match arg.to_ascii_lowercase().as_str() {
                        "left" | "right" | "all" => target = parse_target(arg)?,
                        "path:none" => path_mode = PathMode::None,
                        "path:base" => path_mode = PathMode::Base,
                        "path:relative" => path_mode = PathMode::Relative,
                        _ => {
                            if arg.starts_with("path:") || dest.replace(arg.clone()).is_some() {
                                return Err(invalid("invalid transfer target/options"));
                            }
                        }
                    }
                }
                let dest = dest.ok_or_else(|| invalid("destination path missing"))?;
                bc_vfs::VfsPath::parse(&dest).map_err(|e| invalid(e.to_string()))?;
                Ok(if command == "copyto" {
                    Self::CopyTo {
                        target,
                        path_mode,
                        dest,
                    }
                } else {
                    Self::MoveTo {
                        target,
                        path_mode,
                        dest,
                    }
                })
            }
            "delete" => {
                let mut target = Target::All;
                let mut recycle_bin = true;
                for arg in args {
                    match arg.to_ascii_lowercase().as_str() {
                        "left" | "right" | "all" => target = parse_target(arg)?,
                        "recyclebin=yes" => recycle_bin = true,
                        "recyclebin=no" => recycle_bin = false,
                        _ => return Err(invalid("invalid delete argument")),
                    }
                }
                Ok(Self::Delete {
                    target,
                    recycle_bin,
                })
            }
            "rename" => match args {
                [mask] => Ok(Self::Rename(RenameSpec::Dos {
                    new_mask: mask.clone(),
                })),
                [mode, old, new] if mode.eq_ignore_ascii_case("regexpr") => {
                    bc_core::pattern::Pattern::new(old, true)
                        .map_err(|e| invalid(e.to_string()))?;
                    Ok(Self::Rename(RenameSpec::Regex {
                        old_mask: old.clone(),
                        new_mask: new.clone(),
                    }))
                }
                _ => Err(invalid("rename [regexpr old] new")),
            },
            "touch" => {
                if args.len() != 1 {
                    return Err(invalid("touch requires one timestamp or direction"));
                }
                if let Ok((from, to)) = parse_direction(&args[0]) {
                    return Ok(Self::Touch(TouchSpec::Copy { from, to }));
                }
                let (target, time) = args[0]
                    .split_once(':')
                    .ok_or_else(|| invalid("touch side:timestamp"))?;
                let when = if time.eq_ignore_ascii_case("now") {
                    None
                } else {
                    Some(parse_date(time, clock.utc_offset_seconds).map_err(invalid)?)
                };
                Ok(Self::Touch(TouchSpec::Set {
                    target: parse_target(target)?,
                    when,
                }))
            }
            "attrib" => {
                if args.is_empty() {
                    return Err(invalid("attribute set missing"));
                }
                let mut set = FileAttrs::default();
                let mut clear = FileAttrs::default();
                for arg in args {
                    let (add, bits) = if let Some(s) = arg.strip_prefix('+') {
                        (true, s)
                    } else if let Some(s) = arg.strip_prefix('-') {
                        (false, s)
                    } else {
                        return Err(invalid("attribute sign missing"));
                    };
                    if bits.is_empty() {
                        return Err(invalid("empty attribute set"));
                    }
                    let attrs =
                        FileAttrs::parse_set(bits).ok_or_else(|| invalid("invalid attributes"))?;
                    for (enabled, a, b) in [
                        (attrs.readonly, &mut set.readonly, &mut clear.readonly),
                        (attrs.hidden, &mut set.hidden, &mut clear.hidden),
                        (attrs.system, &mut set.system, &mut clear.system),
                        (attrs.archive, &mut set.archive, &mut clear.archive),
                    ] {
                        if enabled {
                            *a = add;
                            *b = !add;
                        }
                    }
                }
                Ok(Self::Attrib { set, clear })
            }
            _ => Err(invalid(format!("unknown operation command: {command}"))),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectClass {
    All,
    Exact,
    Different,
    Newer,
    Older,
    Orphan,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectKind {
    All,
    Files,
    Folders,
    EmptyFolders,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectMask {
    pub target: Target,
    pub class: SelectClass,
    pub kind: SelectKind,
}
impl SelectMask {
    pub fn parse(mask: &str) -> Result<Self, PlanError> {
        let mask = mask.to_ascii_lowercase();
        let mut value = Self {
            target: Target::All,
            class: SelectClass::All,
            kind: SelectKind::All,
        };
        if mask == "empty.folders" {
            value.kind = SelectKind::EmptyFolders;
            return Ok(value);
        }
        let parts: Vec<_> = mask.split('.').collect();
        let mut at = 0;
        if let Ok(target) = parse_target(parts[at]) {
            value.target = target;
            at += 1;
        }
        if let Some(part) = parts.get(at) {
            let class = match *part {
                "all" => Some(SelectClass::All),
                "exact" => Some(SelectClass::Exact),
                "diff" => Some(SelectClass::Different),
                "newer" => Some(SelectClass::Newer),
                "older" => Some(SelectClass::Older),
                "orphan" => Some(SelectClass::Orphan),
                _ => None,
            };
            if let Some(class) = class {
                value.class = class;
                at += 1;
            }
        }
        if let Some(part) = parts.get(at) {
            value.kind = match *part {
                "all" => SelectKind::All,
                "files" => SelectKind::Files,
                "folders" => SelectKind::Folders,
                _ => return Err(invalid("invalid selection kind")),
            };
            at += 1;
        }
        if at != parts.len() {
            return Err(invalid("invalid selection mask"));
        }
        Ok(value)
    }
    fn status(self, status: CompareStatus, side: usize) -> bool {
        use CompareStatus::*;
        match self.class {
            SelectClass::All => true,
            SelectClass::Exact => status == Same,
            SelectClass::Different => matches!(status, Different | SimilarUnimportant),
            SelectClass::Newer => status == if side == 0 { LeftNewer } else { RightNewer },
            SelectClass::Older => status == if side == 0 { RightNewer } else { LeftNewer },
            SelectClass::Orphan => status == if side == 0 { LeftOrphan } else { RightOrphan },
        }
    }
}
impl Selection {
    /// Script selection ignores expansion but honors file/display filtering. This makes
    /// unattended 'select newer.files' independent of whether a GUI tree was expanded.
    pub fn parse_script(input: &str, tree: &DirTree, view: ViewOptions) -> Result<Self, PlanError> {
        let tokens = words(input).map_err(invalid)?;
        let start = usize::from(
            tokens
                .first()
                .is_some_and(|t| t.eq_ignore_ascii_case("select")),
        );
        if tokens.len() == start {
            return Err(invalid("selection masks missing"));
        }
        let masks = tokens[start..]
            .iter()
            .map(|m| SelectMask::parse(m))
            .collect::<Result<Vec<_>, _>>()?;
        let mut result = Self::default();
        for (id, node) in tree.nodes().filter(|(id, _)| *id != 0) {
            for side in 0..2 {
                if node.sides[side].is_none() {
                    continue;
                }
                if !view.suppress_filters
                    && (node.excluded
                        || node.filtered[side]
                        || !view
                            .display
                            .shows_with_importance(node.status, view.ignore_unimportant))
                {
                    continue;
                }
                for mask in &masks {
                    if !mask.target.contains(side) {
                        continue;
                    }
                    match mask.kind {
                        SelectKind::Files if node.is_dir => continue,
                        SelectKind::Folders | SelectKind::EmptyFolders if !node.is_dir => continue,
                        _ => {}
                    }
                    if mask.kind == SelectKind::EmptyFolders {
                        if node.loaded[side]
                            && !node
                                .children
                                .iter()
                                .filter_map(|id| tree.node(*id))
                                .any(|n| n.sides[side].is_some())
                        {
                            result.insert(id, side);
                        }
                        continue;
                    }
                    let effective = |status| {
                        if view.ignore_unimportant && status == CompareStatus::SimilarUnimportant {
                            CompareStatus::Same
                        } else {
                            status
                        }
                    };
                    let mut statuses = Vec::new();
                    let mut pending = node.children.clone();
                    while let Some(child) = pending.pop() {
                        let Some(n) = tree.node(child) else {
                            continue;
                        };
                        if n.sides[side].is_none() {
                            continue;
                        }
                        if n.is_dir {
                            pending.extend(n.children.iter().copied());
                        } else {
                            statuses.push(effective(n.status));
                        }
                    }
                    if !node.is_dir || statuses.is_empty() {
                        statuses.push(effective(node.status));
                    }
                    if statuses.into_iter().all(|s| mask.status(s, side)) {
                        result.insert(id, side);
                    }
                }
            }
        }
        Ok(result)
    }
}
