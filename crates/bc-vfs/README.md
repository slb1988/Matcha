# bc-vfs

Virtual file-system abstraction. Milestone 1 ships the `Vfs` trait, `VfsPath`,
`VfsEntry`, `VfsManager` and a streaming `LocalFs`; remote protocols are deliberately
absent (spec non-goal) and will be further `Vfs` implementations.

## Responsibilities

- `Vfs` trait: `list`, `metadata`, `open_read`, `get`, `put`, `delete`, `rename`,
  `make_dir`, `crc32`, `set_modified`, `set_attrs`, `watch` – each maps to one `TVfs*Job`.
- `VfsPath` – scheme + path; local paths stay `std::path`, remote ones mirror `vfs://` URLs.
- `VfsEntry`, `EntryKind`, `FileAttrs` (DOS attributes + unix mode), `ChangeEvent`, `Watcher`.
- `LocalFs` – enumeration, per-entry errors, DOS metadata, streaming atomic copies,
  cross-volume moves, recursive deletion, CRC32 and directory notifications.
- `CopyOptions` controls overwrite (default false), timestamps/attributes (default true),
  creation time (default false). `get_cancellable/put_cancellable/rename_cancellable/
  crc32_cancellable` accept `bc_core::cancel::CancelCheck`.
- Copies publish one complete file at a time; cancellation leaves already completed
  files but removes the current temporary file. Directory links are not traversed
  unless requested; copying an unfollowed link returns Unsupported.
- `VfsPath::parse` returns Result and preserves explicit URL spelling, including file://.
  `VfsEntry::error` records child errors without losing readable siblings.
- `VfsManager` – provider registry (`local_only()` for M1), implemented.

## Dependency boundary

Depends on bc-core for CancelCheck/Crc32. Windows-only windows-sys 0.61 enables
Foundation, Storage_FileSystem, UI_Shell, System_IO and Globalization (all Win32_).
The coordinator approved the last two to use generated I/O and NFC bindings rather
than handwritten FFI. Unsafe code is confined to the documented Windows boundary.
No jobs dependency or async runtime.

Windows watch uses overlapped ReadDirectoryChangesW with cancellation/drain on drop;
Unix uses snapshot polling with device/inode identity for rename events. NFC is exposed
by LocalFs::normalize_nfc; enabling NFC on non-Windows returns Unsupported.
Windows recycle uses the shell and rejects UNC/non-fixed drives; non-Windows returns
Unsupported. FOF_WANTNUKEWARNING explicitly requests a shell confirmation if local
policy/quota prevents recycling; this exceptional path may display system UI.
Cancellation returns Unsupported, and callers MUST ask a separate confirmation before
any permanent-delete fallback. It never silently falls back to permanent deletion.
See [recycle policy decision](../../docs/decisions/102-recycle-policy.md).

Tests: `cargo test -p bc-vfs`; the ignored `recycle_bin_manual` test additionally
checks Shell.Application's recycle namespace (run explicitly with --ignored).
All fixtures live in the system temporary directory.

## Planned-operation primitives

Vfs exposes single-file copy_file/move_file/exchange_files with CancelCheck and byte-delta
progress callbacks. Default provider implementations return Unsupported; LocalFs implements
them without recursive traversal. Filtering remains the planner's responsibility.
copy_metadata supplies the planner's post-order folder metadata step; remove_empty_dir
never recursively deletes hidden or newly appeared children.
Exchange writes both temporary copies before publication, restores backups on failure and
retains recovery files if restoration itself fails. No whole-directory transaction is claimed.

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

- Pairing case/separator semantics remain in bc-engine-dir, not VfsPath.
- Replace the shell warning path with IFileOperation in a future change if a fully
  headless, strict recycle-only API is required; do not suppress the warning first.
