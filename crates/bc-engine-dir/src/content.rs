//! Typed content job result; no tree references cross the worker boundary.
use crate::{jobs::ContentCompareJob, CompareStatus, ContentCompare};
use bc_core::{cancel::CancelCheck, hash::Crc32, CoreError};
use bc_engine_text::{jobs::Align2Job, rules::ComparisonRules, TextContent, TextMap, TextRules};
use bc_formats::{registry::FormatRegistry, FileFormat, FormatKind};
use bc_vfs::{VfsError, VfsResult};
use std::{io::Read, sync::OnceLock};

#[derive(Debug, Clone)]
pub struct ContentResult {
    pub node: crate::tree::NodeId,
    pub revision: u64,
    pub status: CompareStatus,
    pub bytes_read: [u64; 2],
    pub skipped: bool,
    pub versions_equal: Option<bool>,
}
impl ContentResult {
    pub fn apply(&self, tree: &mut crate::DirTree) -> bool {
        if self.skipped {
            tree.apply_skipped_content(self.node, self.revision, self.versions_equal)
        } else {
            tree.apply_content(self.node, self.revision, self.status, self.versions_equal)
        }
    }
}
fn check(cancel: &dyn CancelCheck) -> VfsResult<()> {
    if cancel.is_cancelled() {
        Err(VfsError::Cancelled)
    } else {
        Ok(())
    }
}
fn block(
    reader: &mut dyn Read,
    buffer: &mut [u8],
    cancel: &dyn CancelCheck,
    bytes: &mut u64,
) -> VfsResult<usize> {
    let mut count = 0;
    while count < buffer.len() {
        check(cancel)?;
        match reader.read(&mut buffer[count..]) {
            Ok(0) => break,
            Ok(n) => {
                count += n;
                *bytes += n as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(count)
}
fn read_all(reader: &mut dyn Read, cancel: &dyn CancelCheck, bytes: &mut u64) -> VfsResult<Vec<u8>> {
    let mut data = Vec::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let n = block(reader, &mut buffer, cancel, bytes)?;
        data.extend_from_slice(&buffer[..n]);
        if n < buffer.len() {
            return Ok(data);
        }
    }
}
/// Built-in plus user file formats, loaded once like the text view's registry.
/// An unreadable user format file falls back to the built-ins rather than failing scans.
fn formats() -> &'static FormatRegistry {
    static FORMATS: OnceLock<FormatRegistry> = OnceLock::new();
    FORMATS.get_or_init(|| {
        bc_formats::registry::user_formats_dir()
            .ok()
            .and_then(|dir| FormatRegistry::load_user_dir(&dir).ok())
            .unwrap_or_else(FormatRegistry::with_builtins)
    })
}
/// Text format chosen by file name, as the text view does when it opens the same file.
pub fn text_format(name: &str) -> Option<FileFormat> {
    formats()
        .resolve(std::path::Path::new(name), Some(FormatKind::Text))
        .cloned()
}
/// Rules-based comparison through the text engine: byte-identical files are Same, files
/// whose differences are all unimportant under `rules` (line endings by default; comments
/// or whitespace when configured, per the format grammar) are SimilarUnimportant, anything
/// important is Different. Input the text engine rejects (binary) falls back to the byte result.
pub fn compare_rules(
    left: Vec<u8>,
    right: Vec<u8>,
    formats: [Option<FileFormat>; 2],
    rules: &ComparisonRules,
    cancel: &dyn CancelCheck,
) -> VfsResult<CompareStatus> {
    check(cancel)?;
    if left == right {
        return Ok(CompareStatus::Same);
    }
    let [left_format, right_format] = formats;
    let (Ok(left), Ok(right)) = (
        TextContent::from_bytes(left, left_format),
        TextContent::from_bytes(right, right_format),
    ) else {
        return Ok(CompareStatus::Different);
    };
    let core = |e: CoreError| match e {
        CoreError::Cancelled => VfsError::Cancelled,
        e => VfsError::Other(e.to_string()),
    };
    let rules = TextRules {
        comparison: rules.clone(),
        ..TextRules::default()
    };
    let mut map = TextMap::new(left, right, None, rules);
    let output = Align2Job::from_map(&map).execute(cancel).map_err(core)?;
    map.set_alignment(output).map_err(core)?;
    Ok(if map.statistics().important > 0 {
        CompareStatus::Different
    } else {
        CompareStatus::SimilarUnimportant
    })
}
/// Fixed 64 KiB buffers, short-read safe and early exit at the first differing block.
/// Rules-based mode reads both inputs fully and compares them as plain text (no format) with
/// default rules; [`ContentCompareJob`] picks formats from the file names and uses its criteria.
pub fn compare_readers(
    left: &mut dyn Read,
    right: &mut dyn Read,
    mode: ContentCompare,
    cancel: &dyn CancelCheck,
) -> VfsResult<(CompareStatus, [u64; 2])> {
    check(cancel)?;
    if mode == ContentCompare::RulesBased {
        let mut bytes = [0; 2];
        let l = read_all(left, cancel, &mut bytes[0])?;
        let r = read_all(right, cancel, &mut bytes[1])?;
        let rules = ComparisonRules::default();
        return Ok((compare_rules(l, r, [None, None], &rules, cancel)?, bytes));
    }
    let mut bytes = [0; 2];
    let mut buffers = [vec![0; 64 * 1024], vec![0; 64 * 1024]];
    let mut hashes = [Crc32::new(), Crc32::new()];
    loop {
        let ln = block(left, &mut buffers[0], cancel, &mut bytes[0])?;
        let rn = block(right, &mut buffers[1], cancel, &mut bytes[1])?;
        if mode == ContentCompare::Binary && (ln != rn || buffers[0][..ln] != buffers[1][..rn]) {
            return Ok((CompareStatus::Different, bytes));
        }
        if mode == ContentCompare::Crc {
            hashes[0].update(&buffers[0][..ln]);
            hashes[1].update(&buffers[1][..rn]);
        }
        if ln == 0 && rn == 0 {
            break;
        }
    }
    check(cancel)?;
    let [left_crc, right_crc] = hashes;
    let same = bytes[0] == bytes[1]
        && (mode != ContentCompare::Crc || left_crc.finish() == right_crc.finish());
    Ok((
        if same {
            CompareStatus::Same
        } else {
            CompareStatus::Different
        },
        bytes,
    ))
}

impl ContentCompareJob {
    pub fn compare(&self, cancel: &dyn CancelCheck) -> VfsResult<ContentResult> {
        check(cancel)?;
        let mut result = ContentResult {
            node: self.node,
            revision: self.revision,
            status: self.quick_status,
            bytes_read: [0; 2],
            skipped: false,
            versions_equal: None,
        };
        // Versions are independent of the content-skip optimization.
        if self.criteria.version {
            let versions = [&self.left, &self.right].map(|p| {
                let local = p
                    .to_local_path()
                    .ok_or(VfsError::Unsupported("remote PE versions"))?;
                bc_engine_bin::ver::read_version_info(&local).map_err(VfsError::Io)
            });
            let [left, right] = versions;
            result.versions_equal = Some(left? == right?);
            check(cancel)?;
        }
        if !self.criteria.needs_content()
            || (self.criteria.skip_content_if_quick_same
                && self.quick_status == CompareStatus::Same)
        {
            result.skipped = true;
            return Ok(result);
        }
        let mut left = self.vfs.resolve(&self.left)?.open_read(&self.left)?;
        let mut right = self.vfs.resolve(&self.right)?.open_read(&self.right)?;
        if self.criteria.content == ContentCompare::RulesBased {
            let l = read_all(&mut left, cancel, &mut result.bytes_read[0])?;
            let r = read_all(&mut right, cancel, &mut result.bytes_read[1])?;
            let formats = [&self.left, &self.right].map(|p| p.file_name().and_then(text_format));
            result.status = compare_rules(l, r, formats, &self.criteria.rules, cancel)?;
            return Ok(result);
        }
        (result.status, result.bytes_read) =
            compare_readers(&mut left, &mut right, self.criteria.content, cancel)?;
        Ok(result)
    }
}
