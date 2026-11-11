//! Per-user state files next to `recent-text.txt` (`%APPDATA%\Matcha`):
//!
//! | file | content |
//! |---|---|
//! | `shell-menu.txt` | `on` / `off` – Explorer menu setting (missing = on) |
//! | `shell-left.txt` | pending Left path (one line) |
//! | `shell-pair.txt` | pair slot: `path`, unix millis, owner token (three lines) |
//!
//! Several verb processes can run at once (Explorer starts one per selected
//! item), so every slot operation runs under an exclusive lock on
//! `shell.lock` (opened with share mode 0 on Windows). A rename-based "take" is
//! not enough: std's handle-based rename lets two takers both win.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SETTING: &str = "shell-menu.txt";
const LEFT: &str = "shell-left.txt";
const PAIR: &str = "shell-pair.txt";
const LOCK: &str = "shell.lock";

/// Give up waiting for the state lock after this long.
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Open `path` exclusively, retrying while another process holds it.
fn lock_file(path: &Path) -> io::Result<fs::File> {
    let deadline = Instant::now() + LOCK_TIMEOUT;
    loop {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
        match options.open(path) {
            // Off Windows (where no menu exists) this is not exclusive.
            Ok(file) => return Ok(file),
            // ERROR_SHARING_VIOLATION / ERROR_ACCESS_DENIED: held by someone else.
            Err(e)
                if Instant::now() < deadline && matches!(e.raw_os_error(), Some(32) | Some(5)) =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e),
        }
    }
}

/// A pair slot older than this is a leftover of a crashed invocation.
pub const PAIR_TTL: Duration = Duration::from_secs(10);

/// How long the first item of a multi-selection waits for its partner.
/// Measured on Windows 11 26100: Explorer starts the per-item processes one
/// after another, 350–500 ms apart (more under load).
pub const PAIR_WINDOW: Duration = Duration::from_millis(2000);

const POLL: Duration = Duration::from_millis(40);

/// What a `/comparepair` invocation should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pairing {
    /// Second item: compare `(first, this)`.
    Pair(PathBuf, PathBuf),
    /// First item, and the second one took the slot: nothing left to do.
    Partnered,
    /// No partner arrived (single item): behave like Compare to / Select Left.
    Alone(PathBuf),
}

/// The state directory.
#[derive(Debug, Clone)]
pub struct StateDir {
    dir: PathBuf,
}

fn unique_token() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Parsed pair slot.
struct Slot {
    path: PathBuf,
    millis: u128,
    owner: String,
}

impl Slot {
    fn encode(&self) -> String {
        format!("{}\n{}\n{}\n", self.path.display(), self.millis, self.owner)
    }
    fn decode(text: &str) -> Option<Slot> {
        let mut lines = text.lines();
        let path = PathBuf::from(lines.next().filter(|l| !l.is_empty())?);
        let millis = lines.next()?.parse().ok()?;
        let owner = lines.next()?.to_string();
        Some(Slot {
            path,
            millis,
            owner,
        })
    }
    fn fresh(&self) -> bool {
        now_millis().saturating_sub(self.millis) <= PAIR_TTL.as_millis()
    }
}

impl StateDir {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        StateDir { dir: dir.into() }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Run `f` while holding the per-user state lock (`shell.lock`, opened
    /// without sharing; the OS releases it when the handle closes, also when a
    /// process dies). Every slot operation goes through here, so a read-modify-
    /// write is atomic across the per-item processes Explorer starts.
    fn locked<T>(&self, f: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        fs::create_dir_all(&self.dir)?;
        let _lock = lock_file(&self.file(LOCK))?;
        f()
    }

    fn read(&self, name: &str) -> io::Result<Option<String>> {
        match fs::read_to_string(self.file(name)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Replace `name` with `text`.
    fn replace(&self, name: &str, text: &str) -> io::Result<()> {
        self.locked(|| fs::write(self.file(name), text))
    }

    /// Create `name` with `text`; `Ok(false)` when it already exists.
    fn create(&self, name: &str, text: &str) -> io::Result<bool> {
        self.locked(|| {
            if self.file(name).exists() {
                return Ok(false);
            }
            fs::write(self.file(name), text).map(|()| true)
        })
    }

    /// Remove `name` and return its content (`None` if absent); exactly one of
    /// several concurrent takers gets it.
    fn take(&self, name: &str) -> io::Result<Option<String>> {
        self.locked(|| {
            let text = self.read(name)?;
            if text.is_some() {
                fs::remove_file(self.file(name))?;
            }
            Ok(text)
        })
    }

    /// Current content of `name`, read under the lock.
    fn peek(&self, name: &str) -> Option<String> {
        if !self.file(name).exists() {
            return None;
        }
        self.locked(|| self.read(name)).ok().flatten()
    }

    // ---- Explorer menu setting -------------------------------------------------

    /// Explorer menu wanted? Missing or unreadable file = on (the default).
    pub fn enabled(&self) -> bool {
        self.peek(SETTING)
            .is_none_or(|text| !text.trim().eq_ignore_ascii_case("off"))
    }

    pub fn set_enabled(&self, on: bool) -> io::Result<()> {
        self.replace(SETTING, if on { "on\n" } else { "off\n" })
    }

    // ---- pending Left --------------------------------------------------------------

    /// The stored Left path, whether or not it still exists.
    pub fn left(&self) -> Option<PathBuf> {
        let text = self.peek(LEFT)?;
        let line = text.lines().next()?.trim_end();
        (!line.is_empty()).then(|| PathBuf::from(line))
    }

    /// Store (or replace) the Left.
    pub fn set_left(&self, path: &Path) -> io::Result<()> {
        self.replace(LEFT, &format!("{}\n", path.display()))
    }

    /// Consume the Left (exactly one concurrent caller gets it).
    pub fn take_left(&self) -> io::Result<Option<PathBuf>> {
        Ok(self.take(LEFT)?.and_then(|text| {
            let line = text.lines().next()?.trim_end().to_string();
            (!line.is_empty()).then(|| PathBuf::from(line))
        }))
    }

    pub fn clear_left(&self) -> io::Result<()> {
        self.take(LEFT).map(drop)
    }

    // ---- multi-selection pairing -----------------------------------------------------

    /// One `/comparepair` invocation. The first item parks itself in the pair slot
    /// and waits up to `window` for the next item to take it; the next item takes
    /// it and gets [`Pairing::Pair`]. Slots older than [`PAIR_TTL`] are discarded.
    pub fn pair(&self, this: &Path, window: Duration) -> io::Result<Pairing> {
        let owner = unique_token();
        let mine = Slot {
            path: this.to_path_buf(),
            millis: now_millis(),
            owner: owner.clone(),
        };
        // Take a waiting partner, or park; retry when another process raced us.
        let mut parked = false;
        for _ in 0..8 {
            if let Some(text) = self.take(PAIR)? {
                match Slot::decode(&text) {
                    Some(first) if first.fresh() && first.owner != owner => {
                        return Ok(Pairing::Pair(first.path, this.to_path_buf()));
                    }
                    _ => continue, // stale or garbage: discarded by the take
                }
            }
            if self.create(PAIR, &mine.encode())? {
                parked = true;
                break;
            }
        }
        if !parked {
            return Ok(Pairing::Alone(this.to_path_buf()));
        }
        let deadline = Instant::now() + window;
        while Instant::now() < deadline {
            std::thread::sleep(POLL);
            let still_mine = self
                .peek(PAIR)
                .and_then(|text| Slot::decode(&text))
                .is_some_and(|slot| slot.owner == owner);
            if !still_mine {
                return Ok(Pairing::Partnered);
            }
        }
        // Nobody came: reclaim the slot (unless the partner took it just now).
        match self.take(PAIR)? {
            Some(text) => match Slot::decode(&text) {
                Some(slot) if slot.owner == owner => Ok(Pairing::Alone(this.to_path_buf())),
                // A third process parked after ours was taken: put it back.
                Some(other) => {
                    let _ = self.create(PAIR, &other.encode());
                    Ok(Pairing::Partnered)
                }
                None => Ok(Pairing::Partnered),
            },
            None => Ok(Pairing::Partnered),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn temp_state(tag: &str) -> StateDir {
        let dir = std::env::temp_dir().join(format!("matcha-shell-{tag}-{}", unique_token()));
        StateDir::new(dir)
    }

    #[test]
    fn setting_defaults_to_on_and_round_trips() {
        let state = temp_state("setting");
        assert!(state.enabled());
        state.set_enabled(false).unwrap();
        assert!(!state.enabled());
        state.set_enabled(true).unwrap();
        assert!(state.enabled());
        let _ = fs::remove_dir_all(state.path());
    }

    #[test]
    fn left_is_replaced_then_consumed_once() {
        let state = temp_state("left");
        assert_eq!(state.left(), None);
        assert_eq!(state.take_left().unwrap(), None);
        state.set_left(Path::new(r"C:\a b\one.txt")).unwrap();
        state.set_left(Path::new(r"C:\a b\two.txt")).unwrap();
        assert_eq!(state.left(), Some(PathBuf::from(r"C:\a b\two.txt")));
        assert_eq!(
            state.take_left().unwrap(),
            Some(PathBuf::from(r"C:\a b\two.txt"))
        );
        assert_eq!(state.take_left().unwrap(), None);
        state.set_left(Path::new("x")).unwrap();
        state.clear_left().unwrap();
        assert_eq!(state.left(), None);
        let _ = fs::remove_dir_all(state.path());
    }

    #[test]
    fn concurrent_takers_get_the_left_exactly_once() {
        let state = Arc::new(temp_state("race"));
        state.set_left(Path::new("only")).unwrap();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let state = state.clone();
                std::thread::spawn(move || state.take_left().unwrap())
            })
            .collect();
        let got: Vec<_> = handles
            .into_iter()
            .filter_map(|h| h.join().unwrap())
            .collect();
        assert_eq!(got, [PathBuf::from("only")]);
        let _ = fs::remove_dir_all(state.path());
    }

    #[test]
    fn two_sequential_items_pair_up_and_a_single_item_is_alone() {
        let state = Arc::new(temp_state("pair"));
        let first = std::thread::spawn({
            let state = state.clone();
            move || state.pair(Path::new("A"), Duration::from_secs(5)).unwrap()
        });
        // Explorer starts the second item's process a few hundred ms later.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            state.pair(Path::new("B"), Duration::from_secs(5)).unwrap(),
            Pairing::Pair("A".into(), "B".into())
        );
        assert_eq!(first.join().unwrap(), Pairing::Partnered);

        let started = Instant::now();
        assert_eq!(
            state
                .pair(Path::new("C"), Duration::from_millis(200))
                .unwrap(),
            Pairing::Alone("C".into())
        );
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(!state.file(PAIR).exists(), "the lone item cleans up");
        let _ = fs::remove_dir_all(state.path());
    }

    #[test]
    fn stale_pair_slots_are_ignored() {
        let state = temp_state("stale");
        let old = Slot {
            path: "OLD".into(),
            millis: now_millis() - PAIR_TTL.as_millis() - 1000,
            owner: "dead".into(),
        };
        assert!(state.create(PAIR, &old.encode()).unwrap());
        assert_eq!(
            state
                .pair(Path::new("NEW"), Duration::from_millis(100))
                .unwrap(),
            Pairing::Alone("NEW".into())
        );
        let _ = fs::remove_dir_all(state.path());
    }
}
