# bc-engine-bin

Placeholders for the non-text comparison engines (P2–P4): Hex, Picture, Version,
Registry and Media compare. Data models and entry points only; all bodies `todo!()`.

## Responsibilities

- `tag` – shared name/value tree used by Picture, Version and Media compare (`TagValue`,
  `TagRow`, `TagRules`, `align_tags`).
- `hex` – `HexContent`, `HexRules`, `HexMap` (byte alignment via `bc_core::align::ByteAlign`).
- `pix` – `PixRules` (tolerance, colour replacements), `Image`, `PixDiff`, `BlendMode`.
- `ver` – PE version-info extraction.
- `reg` – `.reg` files / live hive loading.
- `media` – audio/video tag reading.

## Dependency boundary

`bc-core`, `bc-formats`, `bc-jobs`, `bc-vfs`. No image/audio decoding dependency has
been chosen yet; those decisions are deferred to the milestone that implements each view.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `tag` | `TTagContent`, `TTagMapTree` (`TSsMapTree`), `TTagRules`, `TTagSession`, `TTagSessionState`, `TTagSessionFrame` |
| `hex` | `THexContent`, `THexMap`, `THexMapAlign`, `THexMapByteAlign`, `THexRules`, `THexFormat`, `THexFiltered`, `THexSession`, `THexCompareSession`, `THexSessionState`, `THexCompareSessionState`, `THexCon*Item`, `THexMap*Item`, `THexLayouter`, `THexInterleavedLayouter`, `THexSideBySideLayouter`, `THexSummaryLayouter` |
| `pix` | `TPixContent`, `TPixMapTree`, `TPixRules`, `TPixReplacementItem`, `TPixReplacementList`, `TPixColors`, `TPixProcessorThread`, `TPixFormat`, `TPixSession`, `TPixCompareSession`, `TPixSessionState`, `TPixCompareSessionState`, `TPixSessionReplacementsFrame` |
| `ver` | `TVerContent`, `TVerMapTree`, `TVerRules`, `TVerFormat`, `TVerSession`, `TVerCompareSession`, `TVerSessionState`, `TVerCompareSessionState`, `TVfsGetExeVersionJob` |
| `reg` | `TRegSession`, `TRegCompareSession`, `TRegRules`, `TRegMapTree`, `TRegistry`, `TRegGroup`, `TRegGroups`, `TRegSessionState`, `TRegCompareSessionState` |
| `media` | `TMediaContent`, `TMediaMapTree`, `TMediaRules`, `TMediaFormat`, `TMediaSession`, `TMediaCompareSession`, `TMediaSessionState`, `TMediaCompareSessionState` |
| views (bc-gui) | `THexView`, `THexCompareView`, `TUiHexEditor`, `TPixView`, `TPixCompareView` |

## Open questions

- Which image formats to support natively (PIX mask: bmp gif ico jpg png tif) and with
  which decoder crate.
- Registry live-hive access needs Win32; same dependency decision as in `bc-vfs`.
