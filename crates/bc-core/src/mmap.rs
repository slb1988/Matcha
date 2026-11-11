//! Safe read-only file mapping wrapper. `bc-engine-table` forbids unsafe code;
//! the single unavoidable `memmap2::Mmap::map` call lives here, mirroring the
//! existing `line::open_mapped` precedent (read-only map, no mutable alias).
//!
//! Callers mapping a private snapshot own the backing file's lifetime; callers
//! mapping a user file must treat external mutation as a correctness hazard
//! (a read-only map reflects page-cache changes).

/// Map an open file read-only. The mapping never outlives the file contents:
/// the caller keeps the file/snapshot alive for the map's lifetime.
pub fn map_readonly(file: &std::fs::File) -> std::io::Result<memmap2::Mmap> {
    // SAFETY: read-only mapping of a file the caller keeps alive; no mutable
    // aliasing is exposed. Snapshot-private files are never written while mapped.
    #[allow(unsafe_code)]
    unsafe {
        memmap2::Mmap::map(file)
    }
}

/// 持有文件句柄的只读映射（句柄随映射存续，等价于参考工具大文件的加锁/流式路径）。
#[derive(Debug)]
pub struct ReadonlyMap {
    // 文件句柄必须活着，映射才有效（Windows 上句柄存续同时提供共享读语义）。
    _file: std::fs::File,
    map: memmap2::Mmap,
}

impl ReadonlyMap {
    /// 打开并只读映射整个文件。
    pub fn open(path: &std::path::Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let map = map_readonly(&file)?;
        Ok(Self { _file: file, map })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
