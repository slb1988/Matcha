# Explorer context menu (Phase 3/3) – worker report

Pure-registry, BC4-style shell menu: static verbs under `HKCU\Software\Classes`
(no admin, no COM shell extension). Code: `crates/bc-shell` (module docs hold
the authoritative key list and lifecycle), `apps/bcompare/src/main.rs`,
`crates/bc-script/src/cli.rs`, `crates/bc-gui/src/dialogs/options.rs`.

## Registry layout

For `<class>` = `*` (files) and `Directory` (folders), under
`HKCU\Software\Classes\<class>\shell\`:

| key | `MUIVerb` | `MultiSelectModel` | `command` (default value) |
|---|---|---|---|
| `Matcha.SelectLeft` | `Matcha: Select Left File for Compare` / `Matcha: Select Left Folder for Compare` | `Single` | `"<dir>\matcha-comp.exe" /selectleft "%1"` |
| `Matcha.CompareTo` | `Matcha: Compare to "<left name>"` | `Single` | `"<dir>\matcha-comp.exe" /compareto "%1"` |
| `Matcha.Compare` | `Matcha: Compare` | `Player` | `"<dir>\matcha-comp.exe" /comparepair "%1"` |

Every verb key also has `Icon` = `<dir>\matcha.exe,0`. `Matcha.CompareTo` has
`ProgrammaticAccessOnly` (hidden) unless a Left of that class is pending.
`<dir>` = folder of the running `matcha.exe` (`current_exe()`); falls back to
`matcha.exe` itself when `matcha-comp.exe` is missing. The `Matcha.` key prefix and
`Matcha: ` text prefix keep the entries apart from Beyond Compare's.

Registration (every GUI start-up, background thread, default on) compares and
writes only differing values: a second start writes nothing, a build in another
folder rewrites the 12 path values. Unregistration deletes the six verb keys and
prunes `<class>\shell` / `<class>` only when empty. Windows 11 lists the verbs
under **Show more options** (static verbs are not shown in the compact menu).

## State files (`%APPDATA%\Matcha`, next to `recent-text.txt`)

| file | role |
|---|---|
| `shell-menu.txt` | `on`/`off` (missing = on) |
| `shell-left.txt` | pending Left path |
| `shell-pair.txt` | multi-select pair slot (path, millis, owner) – short-lived |
| `shell.lock` | exclusive lock (share mode 0) around every slot operation |

Left lifecycle: **set** by Select Left (replaces); **consumed** by Compare to
(label hidden again); **stale** (deleted) → Compare to acts as Select Left and
start-up clears it; **other kind** (file vs folder) → Select Left; **cleared**
when the menu is turned off.

## MultiSelectModel findings (Windows 11 Pro 26100, real `IContextMenu`)

Measured with temporary `MatchaProbe*` verbs (since removed) through
`IShellFolder::GetUIObjectOf` → `QueryContextMenu` / `InvokeCommand`
(`evidence/explorer-shell-menu/ctxmenu.ps1`):

* `Single`: shown for 1 item only. `Player` and `Document`: shown for 1, 2 and 3 items.
* `Player` and `Document` alike start **one process per item** (`%1` = that item,
  `%*` empty), sequentially, **350–500 ms apart**.
* Mixed file+folder selection: the first item's class supplies the verb and command for all.
* `ProgrammaticAccessOnly`: hidden from the menu, still invocable.

Hence `Matcha: Compare` (`Player`) pairs consecutive invocations through
`shell-pair.txt`: the first parks and waits up to 2 s, the second takes it and
opens `(first, second)`. A pending Left is not touched. With no partner (verb used
on one item) it falls back to Compare to / Select Left after the 2 s wait. A
plain "first stores Left, second compares" scheme was rejected: with a Left
already pending it would compare Left+A and leave B as the new Left.

## Acceptance evidence

* `cargo build --workspace`, `cargo test --workspace` (88 test binaries, 0 failures),
  `cargo clippy --workspace --all-targets` clean.
* First run of `target\debug\matcha.exe` → `explorer menu: updated 26 registry value(s)`;
  `reg query` shows the 6 verb keys with commands pointing at `target\debug\matcha-comp.exe`.
* Real context menu on one file: `Matcha: Compare`, `Matcha: Select Left File for Compare`;
  two files: only `Matcha: Compare`.
* `matcha-comp /selectleft …\a.txt` → `shell-left.txt` written, `*\…\Matcha.CompareTo`
  `MUIVerb = Matcha: Compare to "a.txt"`, `ProgrammaticAccessOnly` removed (folder one stays hidden).
* Real menu on `b.txt` listed `Matcha: Compare to "a.txt"`; invoking it opened
  `a.txt <--> b.txt` as a new tab in the running instance and hid the label again
  (`compare-to-new-tab.png`).
* Real `Matcha: Compare` on `c.txt`+`b.txt` with a folder Left pending → exactly one
  `c.txt <--> b.txt` tab; the Left stayed (`multiselect-compare-new-tab.png`).
* Folder Left `d1` + `/compareto d2` → Folder Compare tab `d1 <--> d2`.
* Tools > Options > Startup (now reachable from Home, `options-startup-page.png`):
  unchecking + Apply → `reg query … /f Matcha` finds 0 keys, setting `off`; checking + OK
  → 6 keys back. `/shellmenu=off|on` does the same from the command line.
* Copy of the binaries in another folder → `/shellmenu=on` repointed all commands
  there; the debug build repointed them back.
* `--snapshot=home` with a sandboxed root wrote no keys and no state;
  `/solo a.txt c.txt` opened its own window while the primary kept its tabs.

## Manual Explorer check

1. Run `matcha.exe` once (or `matcha /shellmenu=on`).
2. In Explorer right-click a file → **Show more options** → `Matcha: Select Left File for Compare`.
3. Right-click another file → Show more options → `Matcha: Compare to "<name>"` →
   a new tab opens in the running Matcha; the entry disappears again.
4. Repeat with two folders (`Select Left Folder for Compare`).
5. Select two files → Show more options → `Matcha: Compare` → one new tab.
6. Tools > Options > Startup → untick → OK: the entries are gone; tick → OK: back.

## Leftover issues (not fixed here)

* `matcha.exe` and `matcha-comp.exe` are console-subsystem binaries, so a verb
  briefly flashes a console window, and if no instance is running, the new GUI keeps
  that console open. A `windows_subsystem = "windows"` launcher (or detached spawn) is
  needed – Phase 2 territory.
* `matcha.exe` has no embedded icon resource; `Icon = matcha.exe,0` therefore shows
  the generic executable icon until one is added.
* `Matcha: Compare` is also offered for a single item (registry verbs cannot be
  "2+ only"); used there it waits 2 s and then acts as Compare to / Select Left.
  In a 3+ selection the third item becomes the Left.
* Git Bash rewrites `/switch` arguments into paths (`MSYS_NO_PATHCONV=1` needed when
  scripting `matcha /selectleft …` from it).
* Enumerating a *folder* context menu via `IContextMenu` hung > 60 s on this machine
  (a third-party folder extension); the folder verbs were exercised via the launcher.
