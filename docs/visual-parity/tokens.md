# Visual parity token ledger

`tokens.json` is the machine-readable source for the fixed 1384x961 client-area reference. Flat colors are sampled from the reference capture; geometry uses separator/background scans. The evidence scenes are in `reference/`, including the Options and session dialogs.

Geometry: menu 25px, toolbar 74px, path bars 29px, info headers 38px, panes 751px, details 22px, status bar 22px; text line height 16px and gutters 42px/18px. Fonts: Segoe UI 9pt UI, Consolas 10pt editor and 9pt line numbers.

Inventories cover text/folder/sync/table menus, toolbar entries, status fields, and context menus. Run `make-corpus.ps1`, `set-corpus-mtime.ps1`, and `capture-reference.ps1 -All` to reproduce evidence.
