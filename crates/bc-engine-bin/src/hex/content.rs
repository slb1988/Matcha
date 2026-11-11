//! 单侧字节内容 – `THexContent`。
//!
//! 加载分层（设计 D3）：小于 `lock_threshold` 整载入内存、不加文件锁、可编辑；
//! 达到阈值经 `bc-core::mmap` 只读映射（初版禁编辑与保存，已知限制）。
//! 保存走"暂存 + 改名落盘"，禁编辑侧（Specs 页 Disable editing）拒绝一切写入路径。

use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bc_core::CoreError;

/// 存储后端。
#[derive(Debug)]
pub(crate) enum HexStorage {
    /// 小文件整载（可编辑）。
    Whole(Vec<u8>),
    /// 大文件只读映射（初版禁编辑/保存）。
    Mapped(Arc<bc_core::mmap::ReadonlyMap>),
}

/// Byte buffer of one side – `THexContent`.
#[derive(Debug)]
pub struct HexContent {
    storage: HexStorage,
    /// Specs 页的 Disable editing（禁编辑侧拒绝编辑与保存）。
    read_only: bool,
    /// 编辑代次：每次写操作 +1，后台对齐结果按它识别过期。
    revision: u64,
    /// 来源路径；None 表示剪贴板/内存内容（保存须走另存）。
    source: Option<PathBuf>,
}

impl Default for HexContent {
    fn default() -> Self {
        Self {
            storage: HexStorage::Whole(Vec::new()),
            read_only: false,
            revision: 0,
            source: None,
        }
    }
}

impl HexContent {
    /// 从文件加载：按阈值选择整载或只读映射（设计 D3）。
    pub fn load(path: &Path, lock_threshold: u64) -> io::Result<Self> {
        let len = std::fs::metadata(path)?.len();
        let storage = if len < lock_threshold {
            HexStorage::Whole(std::fs::read(path)?)
        } else {
            HexStorage::Mapped(Arc::new(bc_core::mmap::ReadonlyMap::open(path)?))
        };
        Ok(Self {
            storage,
            read_only: false,
            revision: 0,
            source: Some(path.to_path_buf()),
        })
    }

    /// 内存/剪贴板内容（Open Clipboard）：可编辑，无来源路径。
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            storage: HexStorage::Whole(bytes),
            ..Default::default()
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match &self.storage {
            HexStorage::Whole(bytes) => bytes,
            HexStorage::Mapped(map) => map.bytes(),
        }
    }

    pub(crate) fn storage(&self) -> &HexStorage {
        &self.storage
    }

    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// 大文件（mmap 只读映射）路径。
    pub fn is_large(&self) -> bool {
        matches!(self.storage, HexStorage::Mapped(_))
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Specs 页 Disable editing 开关。
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// 写入门禁：禁编辑侧与大文件映射侧拒绝一切写操作。
    pub fn ensure_writable(&self) -> Result<(), CoreError> {
        if self.read_only {
            return Err(CoreError::Invalid("editing disabled for this side".into()));
        }
        if self.is_large() {
            return Err(CoreError::Invalid("large file is mapped read-only".into()));
        }
        Ok(())
    }

    /// 替换一段字节（Insert/Overwrite 编辑命令的底层写入口；撤销由视图侧记录逆操作）。
    pub fn replace(&mut self, range: Range<usize>, with: &[u8]) -> Result<(), CoreError> {
        self.ensure_writable()?;
        let HexStorage::Whole(bytes) = &mut self.storage else {
            unreachable!("writability excludes mapped storage");
        };
        if range.start > range.end || range.end > bytes.len() {
            return Err(CoreError::Invalid("replace range out of bounds".into()));
        }
        bytes.splice(range, with.iter().copied());
        self.revision += 1;
        Ok(())
    }

    /// 保存到来源路径（暂存 + 改名落盘）。无来源（剪贴板）须走 `save_as`。
    pub fn save(&self) -> io::Result<()> {
        match &self.source {
            Some(path) => self.save_atomic(path.clone()),
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "clipboard content has no source file",
            )),
        }
        // 调用方须先过 ensure_writable；save 本身不重复拒绝 mapped（mapped 不可编辑故无脏内容）。
    }

    /// 另存为：暂存 + 改名落盘，成功后把来源指向新路径。
    pub fn save_as(&mut self, path: &Path) -> io::Result<()> {
        self.save_atomic(path.to_path_buf())?;
        self.source = Some(path.to_path_buf());
        Ok(())
    }

    /// 暂存 + 改名落盘：同目录临时文件写完后 rename，避免半截文件。
    fn save_atomic(&self, target: PathBuf) -> io::Result<()> {
        if self.read_only {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "editing disabled for this side",
            ));
        }
        if self.is_large() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "large file is mapped read-only",
            ));
        }
        let dir = target.parent().unwrap_or_else(|| Path::new("."));
        let name = target
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
        let tmp = dir.join(format!(
            ".{}.matcha-tmp-{}",
            name.to_string_lossy(),
            std::process::id()
        ));
        let result = std::fs::write(&tmp, self.bytes()).and_then(|()| std::fs::rename(&tmp, &target));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}
