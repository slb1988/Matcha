# bc-session

Session / session-folder / workspace model and the compatibility layer for the reference
tool's `BCSessions.xml` and `BCPreferences.xml` (spec §5, FR-X1). All persistence bodies
are `todo!()`; the data model and class-name tables are complete.

## Responsibilities

- `session` – `SessionKind` (13 kinds, with XML class names and `/fv=` spellings),
  `SessionSpecs` (left/right/center/output + titles + VCS paths + read-only flags),
  `SessionRules` (per-family rules enum), `Session` (keeps unknown XML nodes for
  write-through).
- `state` – `TextViewState`, `DirViewState`, `GenericState` behind `SessionState`.
- `folder` – `SessionFolder` tree and `SessionManager` (load/save/upsert).
- `workspace` – `Workspace`, `WorkspaceItem`, `WindowPlacement`.
- `xml` – `read_sessions` / `write_sessions` / node conversion on `bc_core::persist`.
- `prefs` – `Prefs` subset of `TBcPrefs` (colours, backups, default code page, tab size).

## Dependency boundary

`bc-core`, `bc-formats`, `bc-vfs`, all four engine crates (for their rule types). No
GUI, no jobs. Consumers: `bc-report`, `bc-script`, `bc-gui`, `apps/bcompare`.

## XML shape handled

```xml
<BCSessions Version="2" MinVersion="2">
  <TSessionFolder><Items>
    <TTextCompareSession Value="name">
      <LastModified Value="2026-10-30 20:47:37"/>
      <Specs><Left Value="…"/><Right Value="…"/></Specs>
      <State><ShowContext Value="True"/></State>
    </TTextCompareSession>
    <TDirCompareSession Value="A &lt;--> B">
      <State><DirHandling Value="dirCompareFilesOnly"/></State>
    </TDirCompareSession>
  </Items></TSessionFolder>
</BCSessions>
```

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `Session`, `SessionKind` | `TSession`, `TFileSession`, `TLineSession`, `TTextSession`, `TTextCompareSession`, `TTextMergeSession`, `TTextEditSession`, `TTextPatchSession`, `TTableSession`, `TTableCompareSession`, `THexSession`, `THexCompareSession`, `TTagSession`, `TPixSession`, `TPixCompareSession`, `TMediaSession`, `TMediaCompareSession`, `TVerSession`, `TVerCompareSession`, `TRegSession`, `TRegCompareSession`, `TDirBaseSession`, `TDirSession`, `TDirCompareSession`, `TDirMergeSession`, `TDirSyncSession`, `TExternalSession`, `TSessionClassList` |
| `SessionRules` | `TSessionRules`, `TBasicSessionRules`, `TLineRules`, `TTextRules`, `TTextMergeRules`, `TTableRules`, `THexRules`, `TTagRules`, `TPixRules`, `TVerRules`, `TMediaRules`, `TRegRules`, `TDirRules`, `TDirMergeRules`, `TDirSyncRules` |
| `SessionState` | `TSessionState`, `TFileSessionState`, `TTextSessionState`, `TTextCompareSessionState`, `TTextMergeSessionState`, `TTableSessionState`, `THexSessionState`, `TTagSessionState`, `TPixSessionState`, `TVerSessionState`, `TMediaSessionState`, `TRegSessionState`, `TDirSessionState`, `TDirCompareSessionState`, `TDirMergeSessionState`, `TDirSyncSessionState` |
| `SessionFolder`, `SessionManager` | `TSessionFolder`, `TSessionManager` (`TBcSettingsManager`), `TDirChildSessionList` |
| `Workspace`, `WorkspaceItem` | `TWorkspaceItem`, `TWorkspaceList`, `TWorkspaceManagerDialog`, `TWorkspaceSaveAsDialog` |
| `Prefs` | `TBcPrefs`, `TPrefs`, `TProfile`, `TFormPosList` |
| UI (not here) | `TSessionDialog`, `TSessionFrame*`, `TSessionSaveAsDialog`, `TSessionView`, `TSessionViewState` |

## Open questions

- Which `<State>` keys each session class writes (only `ShowContext` and `DirHandling`
  were observed); needs a wider sample of real session files.
- `LastModified` is local time without zone – confirm before writing.
- Settings directory naming for this project (must not reuse the reference vendor path);
  `SessionManager::default_settings_dir` is where that decision lands.
- Import of the reference's `BCState.xml` (window state) is out of scope; `WindowPlacement`
  is kept in the workspace for now.
