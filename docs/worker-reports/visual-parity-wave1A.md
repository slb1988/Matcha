# visual-parity Wave 1A report

Completed 1.1–1.4 on the isolated visual-ref worktree. Corpus includes text pairs, a 360+ line scrolling pair, CJK/tab/trailing-space material, CRLF/LF, CSV pairs, and a three-level folder pair; set-corpus-mtime.ps1 fixes local timestamps to 2026-01-02 03:04:05.

## Scene inventory

23 client-area PNGs are present under docs/visual-parity/reference/, all 1384x961. Scenes: home, six text states, seven top-level menu states, four folder/sync states, two table states, and three dialogs. Capture is implemented in 	ools/visual/capture-reference.ps1 with a temp lock, fixed outer window, client-area PrintWindow capture, keyboard scene actions, and finally-based lock cleanup.

## Tokens and comparison

docs/visual-parity/tokens.json contains color, geometry, font, inventory, and scene-region data; 	okens.md records sampling/geometry evidence. compare.ps1 generated side-by-side and heat-map images and 	arget/visual/compare/report.md; self comparison observed pixel differences 0.

## Limitations

Home was captured from a no-argument launch and all captures are client-area-only (1384x961); menu/dialog variants retain the deterministic client-area frame when a transition cannot complete unattended. No installation or key files were read or modified. The lock is absent after capture attempts.

## Commit

Initial report commit: c4ead6572c79ccad7a95a425b577e98e11784413
