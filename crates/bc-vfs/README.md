# bc-vfs

Virtual file-system abstraction. Milestone 1 ships the `Vfs` trait, `VfsPath`,
`VfsEntry`, `VfsManager` and a `LocalFs` skeleton; remote protocols are deliberately
absent (spec non-goal) and will be further `Vfs` implementations.

## Responsibilities

- `Vfs` trait: `list`, `metadata`, `open_read`, `get`, `put`, `delete`, `rename`,
  `make_dir`, `crc32`, `set_modified`, `set_attrs`, `watch` – each maps to one `TVfs*Job`.
- `VfsPath` – scheme + path; local paths stay `std::path`, remote ones mirror `vfs://` URLs.
- `VfsEntry`, `EntryKind`, `FileAttrs` (DOS attributes + unix mode), `ChangeEvent`, `Watcher`.
- `LocalFs` – all methods `todo!()`.
- `VfsManager` – provider registry (`local_only()` for M1), implemented.

## Dependency boundary

No dependencies at all. Synchronous API: concurrency and cancellation belong to
`bc-jobs` (engines wrap VFS calls in jobs). Used by every engine, the session layer and
the script runner.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `VfsManager` | `TVfsManager`, `VfsManagerImpl`, `TVfsLocalResourceFactory` |
| `Vfs` (provider) | `TVfsResource` (`TJobResource`), `TVfsLocalResource`, `VfsLocalResourceImpl`, `TVfsRemoteServerResource`, `TVfsArchiveResource`, `TVfsRarResource`, `TVfsTotalCmdrPackerResource`, `TVfsClearCaseDDiffResource` |
| `VfsEntry` | `TVfsEntry`, `TVfsEntryList`, `TVfsEntryExList`, `TVfsDeletedEntryList`, `TVfsEntryPlaceholder`, `TVfsEnumEntries` |
| `Vfs::list` | `TVfsLoadDirJob`, `TVfsEnumChildrenJob`, `TVfsRefreshJob`, `TVfsFreshenJob` |
| `Vfs::metadata` | `TVfsGetEntryJob`, `TVfsResourceGetEntryJob` |
| `Vfs::open_read` | `TVfsGetReadFileStreamJob`, `TVfsGetCachedFileJob`, `TVfsGetCachedStreamJob`, `TVfsFileCache`, `TVfsCacheReadStream` |
| `Vfs::get` / `put` | `TVfsGetFileJob`, `TVfsPutFileJob`, `TVfsPutLocalFileJob`, `TVfsReplaceLocalFileJob`, `TVfsCopyToJob`, `TVfsGetWriteStreamJob`, `TVfsGetWriteFileJob`, `TVfsWriteStream` |
| `Vfs::delete` | `TVfsDeleteFileJob`, `TVfsRemoveDirJob` |
| `Vfs::rename` | `TVfsRenameJob`, `TVfsMoveToJob` |
| `Vfs::make_dir` | `TVfsMakeDirJob`, `TVfsForceDirsJob`, `TVfsForceDirsPlaceholder` |
| `Vfs::crc32` | `TVfsGetCRC32Job` |
| `Vfs::set_modified` / `set_attrs` | `TVfsSetLastModifiedJob`, `TVfsSetDOSAttrJob`, `TVfsSetUnixOwnerAndModeJob` |
| `Vfs::watch` / `Watcher` | `TVfsChangeNotifier`, `TVfsChangeNotifierFiberBlocker` |
| `VfsEntry::version` | `TVfsGetExeVersionJob` |
| later providers | `TVfsFtp*` (Indy), SFTP (SecureBlackbox), WebDAV, S3, Dropbox, OneDrive, SVN, snapshots (`.bcss`), MTP, `TVfsPasswordCache`, `TVfsArchivePasswordDialog` |

## Open questions

- Recycle-bin delete and DOS attributes on Windows need Win32 calls; decide between a
  tiny `windows-sys` dependency and manual FFI (constraint: keep dependencies minimal).
- Junction/symlink semantics for `follow_symlinks` on Windows (reparse points).
- Whether `VfsPath` should normalise case/separators for local paths when pairing
  folder-compare nodes (currently left to `bc-engine-dir`).
