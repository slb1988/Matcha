//! Typed content job result; no tree references cross the worker boundary.
use crate::{jobs::ContentCompareJob, CompareStatus, ContentCompare};
use bc_core::{cancel::CancelCheck, hash::Crc32};
use bc_vfs::{VfsError, VfsResult};
use std::io::Read;

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
        tree.apply_content(self.node, self.revision, self.status, self.versions_equal)
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
/// Fixed 64 KiB buffers, short-read safe and early exit at the first differing block.
pub fn compare_readers(
    left: &mut dyn Read,
    right: &mut dyn Read,
    mode: ContentCompare,
    cancel: &dyn CancelCheck,
) -> VfsResult<(CompareStatus, [u64; 2])> {
    check(cancel)?;
    if mode == ContentCompare::RulesBased {
        return Err(VfsError::Unsupported(
            "rules comparison awaits text engine integration",
        ));
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
        if self.criteria.content == ContentCompare::RulesBased {
            return Err(VfsError::Unsupported(
                "rules comparison awaits text engine integration",
            ));
        }
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
        (result.status, result.bytes_read) =
            compare_readers(&mut left, &mut right, self.criteria.content, cancel)?;
        Ok(result)
    }
}
