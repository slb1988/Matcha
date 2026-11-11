# bc-report

Report model and renderers (FR-R1, M2). One `ReportRequest` serves every `*-report`
script command and the report dialogs; option names are the script spellings so the
parser maps 1:1.

## Implemented (change `text-patch-reports`)

- **Text `patch` layout** (`patch` module, `TextReportRenderer`): `patch-normal`,
  `patch-context`, `patch-unified`, `ignore-unimportant`. Byte format follows the reference
  samples – GNU diff layouts with 6 context lines, hunks merged across up to 12 unchanged
  lines and extended to EOF when at most 12 remain, CRLF, UTF-8 with a BOM when an input
  had one, headers only for identical inputs, and the normal-format quirk `2d0`/`0a2` when
  the line before is 1. Protocol and evidence: `tools/golden/README.md` ("patch 报告协议");
  regression: `tests/golden_patch.rs` (255 reference samples, byte for byte apart from
  header timestamps and the collection folder).
- `title:` is accepted and, like the reference, not printed in plain-text patches.
- Header paths and timestamps come from the caller (`ReportFile`): local time needs the
  OS, and this crate stays platform-neutral.
- **Delivery** (`deliver`): file targets via a temporary file renamed over the target (no
  partial file on failure); `Output::Stdout` (`output-to:-`, a Matcha extension) writes the
  same bytes to a caller-supplied writer.
- `ReportRequest::comparison` carries the `<comparison>` argument (`Files` / `Session`).

Not yet: other text layouts, HTML, folder/table/hex reports, printer and clipboard
(`deliver` reports them as not supported; the script parser rejects them up front).

## Responsibilities

- `Layout` (`side-by-side`, `summary`, `interleaved`, `patch`, `statistics`, `xml`; deprecated
  `over-under`/`composite` accepted as `interleaved`).
- `Display` (`display-all|mismatches|matches|context`), `PatchFormat`, `ReportOptions`
  (incl. folder `display-*`, `column-*`, `include-file-links`).
- `Output` (`printer|clipboard|<file>|-`), `OutputOptions` (`print-color/mono`,
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
