# bc-session

Session / session-folder / workspace model and the compatibility layer for the reference
tool's `BCSessions.xml` (spec §5, FR-X1). Files written by the reference tool round-trip
byte for byte apart from the banner comment and line endings
(`tests/sessions_xml.rs`).

## Responsibilities

- `session` – `SessionKind` (13 kinds, with XML class names, `/fv=` spellings and default
  rules), `SessionSpecs` (left/right/center/output + titles + VCS paths + read-only flags;
  unmodelled `<Specs>` children in `extra`), `SessionRules` (per-family rules enum),
  `Stamp` (`LastModified`, zone-less local time), `Session` (keeps the `<Rules>` element
  as read in `rules_node` and other unknown children in `unknown`).
- `state` – `SessionState`: the ordered `<State>` key/value children with typed accessors.
- `folder` – `SessionFolder` (ordered `FolderItem`s: folder / session / raw unknown class)
  and `SessionManager` (load/save/upsert/remove, `%APPDATA%\Matcha\BCSessions.xml`).
- `workspace` – `Workspace`, `WorkspaceItem`, `WindowPlacement`.
- `xml` – `read_sessions` / `write_sessions` / node conversion on `bc_core::persist`.
- `prefs` – `Prefs` subset of `TBcPrefs` (colours, backups, default code page, tab size,
  editor font family + point size); persisted by `bc-gui::settings` as `settings.txt`.

## Dependency boundary

`bc-core`, `bc-formats`, `bc-vfs`, all four engine crates (for their rule types). No
GUI, no jobs. Consumers: `bc-report`, `bc-script`, `bc-gui`, `apps/bcompare`.

## XML shape handled

```xml
<BCSessions Version="2" MinVersion="2">
  <TTextCompareSession Value="named at the root">
    <LastModified Value="2026-11-06 01:00:00"/>
    <Specs><Left Value="…"/><Right Value="…"/></Specs>
    <Rules Value="TTextRules"><AlignKind Value="alignQuick"/>…</Rules>
  </TTextCompareSession>
  <TSessionFolder><Items>
    <TTextCompareSession Value="name">
      <LastModified Value="2026-10-30 20:47:37"/>
      <Specs><Left Value="…"/><Right Value="…"/></Specs>
      <State><ShowContext Value="True"/></State>
    </TTextCompareSession>
    <TDirCompareSession>   <!-- no Value: per-kind default session -->
      <State><DirHandling Value="dirCompareFilesOnly"/></State>
    </TDirCompareSession>
  </Items></TSessionFolder>
</BCSessions>
```

Modelled `<Rules>` keys: `AlignKind` (`alignQuick`/`alignSmart`/`alignPatience`/`alignNone`),
`NeverAlignDiffs`, `SkewTolerance`, `UseCloseness`. The element is written back verbatim
unless those values changed; then only those four keys are updated.

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

- Which `<State>` / `<Rules>` keys each session class writes (observed in 34 real files:
  `ShowContext`, `DirHandling`, `SortSide`, `DisplayFilter`; rules only the four alignment
  keys); other session settings are not persisted until their key names are known.
- `LastModified` is kept as zone-less local time (`Stamp`); callers produce it from the
  local clock.
- Import of the reference's `BCState.xml` (window state) is out of scope; `WindowPlacement`
  is kept in the workspace for now.
