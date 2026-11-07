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
