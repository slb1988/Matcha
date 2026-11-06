# Golden test corpus (bc-core)

Black-box fixtures for algorithm work that follows the scaffold. Each leaf directory
holds *input* files plus an *expected* output produced by the reference tool, so the
Rust implementation can be driven towards 1:1 behaviour without knowing BC internals.

| directory | inputs | expected artefact | drives |
|---|---|---|---|
| `text/2way/<case>/` | UTF-8 `left.txt`, `right.txt`, `case.properties` | `expected.myers.xml`, `expected.standard.xml` (raw reference reports) | `Aligner::align2`, `Differ` |
| `text/3way/<case>/` | `left.txt`, `center.txt`, `right.txt` | `expected.align` + `expected.merge` (auto-merge output, conflict markers) | `Aligner::align3`, bc-engine-text merge |
| `text/inline/<case>/` | `a.txt`, `b.txt` (single lines) | `expected.spans` (char ranges) | inline (character level) diff |
| `align/quick/<case>/` | reserved legacy location | active Myers oracle is beside `text/2way` inputs | `QuickAlign` |
| `align/smart/<case>/` | reserved legacy location | active Standard oracle is beside `text/2way` inputs | `SmartAlign` |
| `dir/criteria/<case>/` | two folder trees (zipped) + `criteria.txt` | `expected.tree` (BC folder-report `layout:xml`) | bc-engine-dir criteria pipeline |
| `table/key-align/<case>/` | `left.csv`, `right.csv`, `keys.txt` | `expected.align` | bc-engine-table `KeyAlign` |

Conventions

- Fixture names are `NNN-short-description`, zero padded, so ordering is stable.
- `expected.*` files are generated with the reference tool via its script engine
  (`text-report layout:xml`, `folder-report layout:xml`) and checked in verbatim; never
  hand-edit them.
- Keep inputs small (< 50 KB); large-file behaviour is covered by benchmarks, not goldens.
- No fixture may contain licensed/proprietary text; use synthetic or permissively
  licensed sources.

Wave 3 now includes 30 synthetic UTF-8 input-template pairs and case.properties.
The generator stages real encoded/line-ended working files; source templates remain
UTF-8. Expected reports are named expected.myers.xml / expected.standard.xml and must
be checked in verbatim. The complete 60-report oracle set is checked in: Myers matches 30/30 cases and
Standard+closeness 29/30 (one documented intentional difference). See
`tools/golden/README.md` and `KNOWN_DIFFS.md`. Absent oracle tests still announce a
skip for an empty corpus, but partial sets fail; the committed corpus does not skip.
Diagnostic reports outside this tree are not formal exceptions. Engine regression
checks all 30 cases, including raw importance; core also checks the 26
format-independent normalized alignment cases.
