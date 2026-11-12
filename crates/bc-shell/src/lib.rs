//! # bc-shell
//!
//! Windows Explorer context menu for Matcha, BC4-style but without a COM shell
//! extension: plain static verbs under `HKEY_CURRENT_USER\Software\Classes`
//! (no admin rights), plus a little per-user state so the menu can say
//! `Compare to "<left>"`.
//!
//! ## Registry keys (all created, repaired and removed by this crate)
//!
//! For `<class>` = `*` (every file) and `Directory` (folders):
//!
//! | key under `HKCU\Software\Classes\<class>\shell\` | `MUIVerb` (visible text) | `MultiSelectModel` | command |
//! |---|---|---|---|
//! | `Matcha.SelectLeft` | `Matcha: Select Left File for Compare` / `… Folder …` | `Single` | `"<dir>\matcha-comp.exe" /selectleft "%1"` |
//! | `Matcha.CompareTo` | `Matcha: Compare to "<left name>"` | `Single` | `"<dir>\matcha-comp.exe" /compareto "%1"` |
//! | `Matcha.Compare` | `Matcha: Compare` | `Player` | `"<dir>\matcha-comp.exe" /comparepair "%1"` |
//!
//! Each verb key also holds `Icon` = `<dir>\matcha.exe,0`; the command lives in
//! the verb's `command` subkey (default value). `Matcha.CompareTo` carries
//! `ProgrammaticAccessOnly` (hidden from the menu) unless a Left of the same
//! kind is pending. The `Matcha.` key prefix and the `Matcha: ` text prefix keep
//! the entries apart from a co-installed Beyond Compare. `<dir>` is the folder of
//! the running `matcha.exe` (`matcha.exe` itself when `matcha-comp.exe` is
//! missing), so starting a different build repairs the paths.
//!
//! Unregistering deletes the six verb keys (with their `command` subkeys) and
//! then `<class>\shell` and `<class>` only if they are left empty.
//!
//! On Windows 11 these static verbs appear under "Show more options" (the
//! classic menu, Shift+F10 / Shift+right-click); the compact menu only lists
//! packaged (`IExplorerCommand`) handlers.
//!
//! ## Pending Left (`%APPDATA%\Matcha\shell-left.txt`)
//!
//! * **Set** by Select Left (replaces any previous Left), and by Compare to /
//!   Compare when there is nothing to compare with. The Compare-to label is
//!   rewritten to the new name and shown for the Left's kind (file/folder).
//! * **Consumed** by Compare to: the Left is taken atomically, the comparison
//!   opens, and the label goes back to hidden.
//! * **Stale** (the Left was deleted or renamed): Compare to treats the click as
//!   Select Left; start-up registration clears it and hides the label.
//! * **Other kind** (Left is a folder, target a file or vice versa – only
//!   reachable from the command line or the single-item "Compare" fallback):
//!   treated as Select Left, replacing the old Left.
//! * **Cleared** when the menu is turned off.
//!
//! ## Multi-selection (`Matcha: Compare`)
//!
//! Measured on Windows 11 26100 with a real `IContextMenu`: `Single` verbs are
//! hidden for 2+ items; `Player` and `Document` verbs show for 1..n items and
//! *both* start one process per item (`%1` = that item, `%*` empty), one after
//! another 350–500 ms apart. A pure-registry verb therefore cannot see the
//! selection, so `/comparepair` pairs consecutive invocations through
//! `shell-pair.txt` ([`state::StateDir::pair`]): the first item parks and waits
//! up to [`state::PAIR_WINDOW`]; the second takes it and compares the two. A
//! pending Left is not touched by this. When no partner arrives (the verb was
//! used on a single item), it acts like Compare to (or Select Left). A third
//! item of a 3+ selection is likewise left alone.
//!
//! ## Environment overrides (tests only)
//!
//! * `MATCHA_SHELL_ROOT` – registry root relative to HKCU instead of
//!   `Software\Classes` (tests use a private `Software\Matcha-test-*` key).
//! * `MATCHA_SHELL_STATE_DIR` – state directory instead of `%APPDATA%\Matcha`.
#[cfg(any(windows, target_os = "macos"))]
mod cursor;
pub mod layout;
pub mod registry;
pub mod state;
#[cfg(windows)]
mod windows;

pub use layout::{Class, Left, Targets, Verb};

/// Desktop cursor position in physical pixels; `None` on platforms without an
/// OS query (where the GUI's own pointer position is used instead). The GUI
/// needs it to locate an OS file drop, whose coordinates winit does not
/// report. `pixels_per_point` is the window's scale factor: macOS reads the
/// cursor in CoreGraphics points and converts with it; Windows returns
/// physical pixels already and ignores it.
pub fn cursor_screen_pos(pixels_per_point: f64) -> Option<(i32, i32)> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        cursor::screen_pos(pixels_per_point)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = pixels_per_point;
        None
    }
}
pub use registry::{MemRegistry, Registry};
pub use state::{Pairing, StateDir};

use std::io;
use std::path::{Path, PathBuf};

pub const ROOT_ENV: &str = "MATCHA_SHELL_ROOT";
pub const STATE_ENV: &str = "MATCHA_SHELL_STATE_DIR";

/// Launcher looked up next to `matcha.exe`.
pub const LAUNCHER: &str = "matcha-comp.exe";

/// Where the menu lives and what it points at.
#[derive(Debug, Clone)]
pub struct Shell {
    /// Registry root relative to HKCU (normally [`layout::ROOT`]).
    pub root: String,
    pub state: StateDir,
    pub targets: Targets,
}

/// Verbs point at `matcha-comp.exe` beside `exe`, or at `exe` itself.
pub fn targets_for(exe: &Path) -> Targets {
    let launcher = exe
        .parent()
        .map(|dir| dir.join(LAUNCHER))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| exe.to_path_buf());
    Targets {
        launcher,
        icon: exe.to_path_buf(),
    }
}

/// `%APPDATA%\Matcha` (the folder holding `recent-text.txt`).
pub fn default_state_dir() -> Option<PathBuf> {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|formats| formats.parent().map(Path::to_path_buf))
}

impl Shell {
    /// Configuration for the running `matcha.exe` (honours the test overrides).
    pub fn from_env() -> io::Result<Shell> {
        let root = std::env::var(ROOT_ENV)
            .ok()
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| layout::ROOT.to_string());
        let state = std::env::var_os(STATE_ENV)
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(default_state_dir)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no user settings folder"))?;
        let exe = std::env::current_exe()?;
        Ok(Shell {
            root,
            state: StateDir::new(state),
            targets: targets_for(&exe),
        })
    }

    /// The pending Left if it still exists; a stale one is cleared.
    pub fn current_left(&self) -> Option<Left> {
        let path = self.state.left()?;
        let left = Left::read(path);
        if left.is_none() {
            let _ = self.state.clear_left();
        }
        left
    }
}

/// The real per-user registry (`None` off Windows).
pub fn system_registry() -> Option<Box<dyn Registry>> {
    #[cfg(windows)]
    {
        Some(Box::new(registry::Hkcu))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Is any Matcha verb present?
pub fn is_registered(reg: &dyn Registry, root: &str) -> io::Result<bool> {
    for key in layout::all_verb_keys(root) {
        if reg.exists(&key)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Start-up step: register (idempotent; repairs labels and exe paths) when the
/// setting is on, otherwise make sure nothing is left behind. Returns the number
/// of registry changes.
pub fn sync(shell: &Shell, reg: &mut dyn Registry) -> io::Result<usize> {
    if shell.state.enabled() {
        let left = shell.current_left();
        let entries = layout::entries(&shell.root, &shell.targets, left.as_ref());
        registry::apply(reg, &entries)
    } else {
        unregister(reg, &shell.root)
    }
}

/// Remove every key Matcha created; returns how many verb keys existed.
pub fn unregister(reg: &mut dyn Registry, root: &str) -> io::Result<usize> {
    let mut removed = 0;
    for key in layout::all_verb_keys(root) {
        if reg.exists(&key)? {
            reg.remove_tree(&key)?;
            removed += 1;
        }
    }
    for class in Class::ALL {
        let class_key = format!(r"{root}\{}", class.key());
        reg.remove_if_empty(&format!(r"{class_key}\shell"))?;
        reg.remove_if_empty(&class_key)?;
    }
    Ok(removed)
}

/// Tools > Options / `/shellmenu=`: persist the setting, then register or
/// unregister. Turning the menu off also forgets the pending Left.
pub fn set_enabled(shell: &Shell, reg: &mut dyn Registry, on: bool) -> io::Result<usize> {
    shell.state.set_enabled(on)?;
    if !on {
        shell.state.clear_left()?;
    }
    sync(shell, reg)
}

/// Rewrite the Compare-to label/visibility from the current Left. Only touches
/// keys that exist, so it never re-creates a menu that is turned off.
pub fn refresh_label(shell: &Shell, reg: &mut dyn Registry) -> io::Result<()> {
    if !shell.state.enabled() {
        return Ok(());
    }
    let left = shell.current_left();
    for entry in layout::compare_to_entries(&shell.root, left.as_ref()) {
        if reg.exists(&entry.key)? {
            registry::apply(reg, std::slice::from_ref(&entry))?;
        }
    }
    Ok(())
}

/// What the caller should do after a verb ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to open (Left stored, or the partner process opens the pair).
    Stored(String),
    /// Open this comparison.
    Compare(PathBuf, PathBuf),
}

/// Select Left: store `path`, show `Compare to "<name>"`.
pub fn select_left(shell: &Shell, reg: &mut dyn Registry, path: &Path) -> io::Result<Outcome> {
    shell.state.set_left(path)?;
    refresh_label(shell, reg)?;
    Ok(Outcome::Stored(format!(
        "Left set to {}; choose \"Compare to\" on the other item",
        path.display()
    )))
}

/// Compare to: consume the Left and compare it with `path`. Without a live Left
/// of the same kind (a file cannot be compared with a folder) this acts as
/// Select Left, replacing the old one.
pub fn compare_to(shell: &Shell, reg: &mut dyn Registry, path: &Path) -> io::Result<Outcome> {
    let class = Class::of(path);
    let matches = shell.current_left().is_some_and(|left| left.class == class);
    if !matches {
        return select_left(shell, reg, path);
    }
    let left = shell.state.take_left()?.filter(|left| left.exists());
    refresh_label(shell, reg)?;
    match left {
        Some(left) => Ok(Outcome::Compare(left, path.to_path_buf())),
        None => select_left(shell, reg, path),
    }
}

/// Run one verb invocation (`/selectleft`, `/compareto`, `/comparepair`).
pub fn invoke(
    shell: &Shell,
    reg: &mut dyn Registry,
    verb: Verb,
    path: &Path,
) -> io::Result<Outcome> {
    match verb {
        Verb::SelectLeft => select_left(shell, reg, path),
        Verb::CompareTo => compare_to(shell, reg, path),
        Verb::Compare => match shell.state.pair(path, state::PAIR_WINDOW)? {
            Pairing::Pair(first, second) => Ok(Outcome::Compare(first, second)),
            Pairing::Partnered => Ok(Outcome::Stored("paired with the next selected item".into())),
            Pairing::Alone(path) => compare_to(shell, reg, &path),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layout::ROOT;

    struct Fixture {
        shell: Shell,
        reg: MemRegistry,
        work: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.work);
        }
    }

    fn fixture(tag: &str) -> Fixture {
        let work = std::env::temp_dir().join(format!(
            "matcha-shell-lib-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(work.join("dir")).unwrap();
        std::fs::write(work.join("a.txt"), "a").unwrap();
        std::fs::write(work.join("b.txt"), "b").unwrap();
        let shell = Shell {
            root: ROOT.to_string(),
            state: StateDir::new(work.join("state")),
            targets: Targets {
                launcher: PathBuf::from(r"C:\build\matcha-comp.exe"),
                icon: PathBuf::from(r"C:\build\matcha.exe"),
            },
        };
        Fixture {
            shell,
            reg: MemRegistry::default(),
            work,
        }
    }

    fn label(f: &Fixture, class: Class) -> (Option<String>, bool) {
        let key = layout::verb_key(ROOT, class, Verb::CompareTo);
        (
            f.reg.get(&key, "MUIVerb").unwrap(),
            f.reg.get(&key, "ProgrammaticAccessOnly").unwrap().is_none(),
        )
    }

    #[test]
    fn registration_is_idempotent_and_repairs_moved_executables() {
        let mut f = fixture("idem");
        assert!(sync(&f.shell, &mut f.reg).unwrap() > 0);
        assert!(is_registered(&f.reg, ROOT).unwrap());
        let writes = f.reg.writes;
        assert_eq!(sync(&f.shell, &mut f.reg).unwrap(), 0);
        assert_eq!(f.reg.writes, writes, "second run writes nothing");

        // A release build starts: every command and icon follows it.
        f.shell.targets = Targets {
            launcher: PathBuf::from(r"D:\rel\matcha-comp.exe"),
            icon: PathBuf::from(r"D:\rel\matcha.exe"),
        };
        assert_eq!(sync(&f.shell, &mut f.reg).unwrap(), 12);
        let command = f
            .reg
            .get(&layout::command_key(ROOT, Class::Folder, Verb::Compare), "")
            .unwrap();
        assert_eq!(
            command.as_deref(),
            Some(r#""D:\rel\matcha-comp.exe" /comparepair "%1""#)
        );
    }

    #[test]
    fn unregister_removes_everything_it_created() {
        let mut f = fixture("unreg");
        // Someone else's verb under the same class survives.
        f.reg
            .set(r"Software\Classes\*\shell\Other\command", "", "x")
            .unwrap();
        sync(&f.shell, &mut f.reg).unwrap();
        assert_eq!(set_enabled(&f.shell, &mut f.reg, false).unwrap(), 6);
        assert!(!is_registered(&f.reg, ROOT).unwrap());
        assert!(f.reg.exists(r"Software\Classes\*\shell\Other").unwrap());
        assert!(f.reg.keys_under(r"Software\Classes\Directory").is_empty());
        // Off stays off across restarts; on restores it.
        assert_eq!(sync(&f.shell, &mut f.reg).unwrap(), 0);
        assert!(!is_registered(&f.reg, ROOT).unwrap());
        set_enabled(&f.shell, &mut f.reg, true).unwrap();
        assert!(is_registered(&f.reg, ROOT).unwrap());
        assert_eq!(f.reg.keys_under(r"Software\Classes\Directory").len(), 8);
    }

    #[test]
    fn left_lifecycle_drives_the_compare_to_label() {
        let mut f = fixture("life");
        sync(&f.shell, &mut f.reg).unwrap();
        assert!(!label(&f, Class::File).1, "hidden at first");

        let a = f.work.join("a.txt");
        let b = f.work.join("b.txt");
        assert!(matches!(
            invoke(&f.shell, &mut f.reg, Verb::SelectLeft, &a).unwrap(),
            Outcome::Stored(_)
        ));
        assert_eq!(
            label(&f, Class::File),
            (Some("Matcha: Compare to \"a.txt\"".into()), true)
        );
        assert!(!label(&f, Class::Folder).1, "files only");

        assert_eq!(
            invoke(&f.shell, &mut f.reg, Verb::CompareTo, &b).unwrap(),
            Outcome::Compare(a.clone(), b.clone())
        );
        assert_eq!(
            label(&f, Class::File),
            (Some("Matcha: Compare to".into()), false)
        );
        assert_eq!(f.shell.state.left(), None, "consumed");

        // Compare to without a Left = Select Left.
        invoke(&f.shell, &mut f.reg, Verb::CompareTo, &b).unwrap();
        assert_eq!(f.shell.state.left(), Some(b.clone()));
        // A folder Left switches the label to folders.
        let dir = f.work.join("dir");
        invoke(&f.shell, &mut f.reg, Verb::SelectLeft, &dir).unwrap();
        // ...and a file cannot be compared with it: that file becomes the Left.
        let a2 = f.work.join("a.txt");
        assert!(matches!(
            compare_to(&f.shell, &mut f.reg, &a2).unwrap(),
            Outcome::Stored(_)
        ));
        assert_eq!(f.shell.state.left(), Some(a2));
        invoke(&f.shell, &mut f.reg, Verb::SelectLeft, &dir).unwrap();
        assert!(!label(&f, Class::File).1);
        assert_eq!(
            label(&f, Class::Folder),
            (Some("Matcha: Compare to \"dir\"".into()), true)
        );
    }

    #[test]
    fn a_deleted_left_is_stale() {
        let mut f = fixture("stale");
        sync(&f.shell, &mut f.reg).unwrap();
        let gone = f.work.join("gone.txt");
        std::fs::write(&gone, "x").unwrap();
        select_left(&f.shell, &mut f.reg, &gone).unwrap();
        std::fs::remove_file(&gone).unwrap();
        // Restart: label hidden, state cleared.
        sync(&f.shell, &mut f.reg).unwrap();
        assert!(!label(&f, Class::File).1);
        assert_eq!(f.shell.state.left(), None);
        // Compare to against a stale Left becomes Select Left.
        select_left(&f.shell, &mut f.reg, &gone).unwrap();
        let b = f.work.join("b.txt");
        assert!(matches!(
            compare_to(&f.shell, &mut f.reg, &b).unwrap(),
            Outcome::Stored(_)
        ));
        assert_eq!(f.shell.state.left(), Some(b));
    }

    #[test]
    fn labels_are_not_written_while_the_menu_is_off() {
        let mut f = fixture("off");
        set_enabled(&f.shell, &mut f.reg, false).unwrap();
        select_left(&f.shell, &mut f.reg, &f.work.join("a.txt")).unwrap();
        assert!(f.reg.keys_under("Software").is_empty());
    }

    #[test]
    fn launcher_falls_back_to_the_main_executable() {
        let f = fixture("targets");
        let exe = f.work.join("matcha.exe");
        assert_eq!(targets_for(&exe).launcher, exe);
        std::fs::write(f.work.join(LAUNCHER), "").unwrap();
        assert_eq!(targets_for(&exe).launcher, f.work.join(LAUNCHER));
        assert_eq!(targets_for(&exe).icon, exe);
    }

    #[cfg(windows)]
    #[test]
    fn real_hkcu_round_trip_under_a_private_test_key() {
        // Never Software\Classes: a throwaway key Explorer does not read.
        let root = format!(r"Software\Matcha-test-shell-{}", std::process::id());
        let mut f = fixture("hkcu");
        f.shell.root = root.clone();
        let mut reg = registry::Hkcu;
        assert!(sync(&f.shell, &mut reg).unwrap() > 0);
        assert_eq!(sync(&f.shell, &mut reg).unwrap(), 0, "idempotent");
        let a = f.work.join("a.txt");
        select_left(&f.shell, &mut reg, &a).unwrap();
        let key = layout::verb_key(&root, Class::File, Verb::CompareTo);
        assert_eq!(
            reg.get(&key, "MUIVerb").unwrap().as_deref(),
            Some("Matcha: Compare to \"a.txt\"")
        );
        assert_eq!(reg.get(&key, "ProgrammaticAccessOnly").unwrap(), None);
        compare_to(&f.shell, &mut reg, &f.work.join("b.txt")).unwrap();
        assert_eq!(
            reg.get(&key, "ProgrammaticAccessOnly").unwrap().as_deref(),
            Some("")
        );
        assert_eq!(unregister(&mut reg, &root).unwrap(), 6);
        assert!(!reg.exists(&format!(r"{root}\*")).unwrap());
        reg.remove_if_empty(&root).unwrap();
        assert!(!reg.exists(&root).unwrap());
    }
}
