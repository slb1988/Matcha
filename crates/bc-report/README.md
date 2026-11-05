# bc-report

Report model and renderers (FR-R1, M2). One `ReportRequest` serves every `*-report`
script command and the report dialogs; option names are the script spellings so the
parser maps 1:1. Renderers and delivery are `todo!()`.

## Responsibilities

- `Layout` (`side-by-side`, `summary`, `interleaved`, `patch`, `statistics`, `xml`; deprecated
  `over-under`/`composite` accepted as `interleaved`).
- `Display` (`display-all|mismatches|matches|context`), `PatchFormat`, `ReportOptions`
  (incl. folder `display-*`, `column-*`, `include-file-links`).
- `Output` (`printer|clipboard|<file>`), `OutputOptions` (`print-color/mono`,
  `print-portrait/landscape`, `wrap-none/character/word`, `html-color/mono/custom`).
- `ReportRequest`, `Report`, `Renderer` trait, `TextReportRenderer`, `DirReportRenderer`,
  `deliver`.

## Dependency boundary

`bc-core`, `bc-session`, the four engine crates, `bc-jobs`. No GUI; HTML output is a
string the host opens externally.

## Layout availability (from the scripting reference)

| command | layouts |
|---|---|
| text-report | side-by-side, summary, interleaved, patch, statistics, xml |
| folder-report | side-by-side, summary, xml |
| table-report (`data-report`) | side-by-side, summary, interleaved |
| hex-report | side-by-side, summary, interleaved |
| file-report | side-by-side, summary (dispatches by file format) |
| picture-, media-, registry-, version-report | side-by-side, summary |

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `ReportRequest` | `TReportState`, `TBcReportState`, `TMixedReportState`, `TSessionReportState`, `TFileReportState`, `TDirReportState`, `TTextReportState`, `TTableReportState`, `THexReportState`, `TTagReportState`, `TPixReportState`, `TMediaReportState`, `TVerReportState`, `TRegReportState` |
| `Report`, `Renderer` | `TBcReport`, `TMetaReport`, `TMetaMultiReport`, `TFileLayouter`, `THexLayouter`, `THexSideBySideLayouter`, `THexInterleavedLayouter`, `THexSummaryLayouter` |
| `deliver` | `TReportJob`, `TReportSaveToFileJob`, `TReportCopyToClipboardJob`, `TReportPrintJob`, `TReportPreviewJob`, `TReportShowWebBrowserJob`, `TReportPrinter` |
| dialogs (not here) | `TReportDialog`, `TMixedReportDialog`, `TFileReportDialog`, `TDirReportDialog`, `TTextReportDialog`, `TTableReportDialog`, `THexReportDialog`, `TTagReportDialog`, `TPixReportDialog`, `TMediaReportDialog`, `TVerReportDialog`, `TRegReportDialog`, `TReportSetupDialog`, `TReportPreviewDialog` |

## Open questions

- Printing is a non-goal; `Output::Printer` is parsed but `deliver` should report it as
  unsupported until a print path exists.
- The XML report schema of the reference (`folder-report layout:xml`) must be captured
  from real output before `DirReportRenderer` can be made compatible; it doubles as the
  golden format for `bc-core/tests/golden/dir`.
