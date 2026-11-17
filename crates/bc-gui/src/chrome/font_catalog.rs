//! Installed editor-font catalog: the monospace families the Options dialog offers.
//!
//! Scans the system and per-user font folders and reads only the few sfnt tables
//! needed (`name`, `OS/2`, `post`, `fvar` presence) with seeks, so the full
//! files (CJK fonts are 10–30 MB each) are never loaded. The scan runs once per
//! process ([`warm`] starts it in the background; [`get`] / [`find`] join it).
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One face inside a font file (`index` = TTC member).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceRef {
    pub path: PathBuf,
    pub index: u32,
}

/// A selectable editor family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Family {
    /// English family name (stable across UI languages; stored in settings).
    pub name: String,
    pub regular: FaceRef,
    /// Real bold face, if installed.
    pub bold: Option<FaceRef>,
    /// The regular face is a variable font with a `wght` axis (bold = wght 700).
    pub variable_weight: bool,
}

/// Facts read from one face.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Face {
    family: String,
    weight: u16,
    italic: bool,
    mono: bool,
    wght_axis: bool,
    face: FaceRef,
}

static CATALOG: OnceLock<Vec<Family>> = OnceLock::new();
static PENDING: std::sync::Mutex<Option<std::thread::JoinHandle<Vec<Family>>>> =
    std::sync::Mutex::new(None);

/// Start scanning in the background (idempotent).
pub fn warm() {
    if CATALOG.get().is_some() {
        return;
    }
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.is_none() {
        *pending = std::thread::Builder::new()
            .name("matcha-font-catalog".into())
            .spawn(|| scan(&font_dirs()))
            .ok();
    }
}

/// The catalog, if the scan has finished (never blocks).
pub fn ready() -> Option<&'static [Family]> {
    if let Some(list) = CATALOG.get() {
        return Some(list);
    }
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.as_ref().is_some_and(|h| h.is_finished()) {
        let list = pending.take().unwrap().join().unwrap_or_default();
        return Some(CATALOG.get_or_init(|| list));
    }
    None
}

/// The catalog, scanning (or joining the background scan) if needed.
pub fn get() -> &'static [Family] {
    CATALOG.get_or_init(|| {
        let handle = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
        handle
            .and_then(|h| h.join().ok())
            .unwrap_or_else(|| scan(&font_dirs()))
    })
}

/// Look up a family by name (case-insensitive).
pub fn find(name: &str) -> Option<&'static Family> {
    get().iter().find(|f| f.name.eq_ignore_ascii_case(name))
}

/// `%WINDIR%\Fonts` and the per-user font folder.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR").or_else(|| std::env::var_os("windir")) {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join(r"Microsoft\Windows\Fonts"));
    }
    dirs
}

/// Scan `dirs` for monospace families (sorted by name, first directory wins on duplicates).
pub fn scan(dirs: &[PathBuf]) -> Vec<Family> {
    let mut faces = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    ["ttf", "otf", "ttc"]
                        .iter()
                        .any(|x| e.eq_ignore_ascii_case(x))
                })
            })
            .collect();
        paths.sort();
        for path in paths {
            faces.extend(read_faces(&path).unwrap_or_default());
        }
    }
    families(faces)
}

/// Group faces into monospace families with a regular and (optional) bold face.
fn families(faces: Vec<Face>) -> Vec<Family> {
    let mut names: Vec<String> = Vec::new();
    for face in &faces {
        if face.mono && !names.iter().any(|n| n.eq_ignore_ascii_case(&face.family)) {
            names.push(face.family.clone());
        }
    }
    let mut out = Vec::new();
    for name in names {
        let upright: Vec<&Face> = faces
            .iter()
            .filter(|f| f.family.eq_ignore_ascii_case(&name) && !f.italic)
            .collect();
        let Some(regular) = upright
            .iter()
            .min_by_key(|f| (f.weight as i32 - 400).abs())
            .filter(|f| (300..=500).contains(&f.weight))
        else {
            continue;
        };
        let bold = upright
            .iter()
            .filter(|f| f.weight >= 600)
            .min_by_key(|f| (f.weight as i32 - 700).abs())
            .map(|f| f.face.clone());
        out.push(Family {
            name,
            regular: regular.face.clone(),
            bold,
            variable_weight: regular.wght_axis,
        });
    }
    out.sort_by_key(|f| f.name.to_lowercase());
    out
}

fn read_faces(path: &Path) -> std::io::Result<Vec<Face>> {
    let mut file = File::open(path)?;
    let tag = read_bytes(&mut file, 0, 4)?;
    let offsets = if tag == b"ttcf" {
        let count = u32_at(&read_bytes(&mut file, 8, 4)?, 0).min(64);
        let table = read_bytes(&mut file, 12, 4 * count as usize)?;
        (0..count as usize).map(|i| u32_at(&table, 4 * i)).collect()
    } else {
        vec![0]
    };
    let mut faces = Vec::new();
    for (index, offset) in offsets.into_iter().enumerate() {
        if let Ok(Some(face)) = read_face(&mut file, offset as u64, path, index as u32) {
            faces.push(face);
        }
    }
    Ok(faces)
}

fn read_face(file: &mut File, at: u64, path: &Path, index: u32) -> std::io::Result<Option<Face>> {
    let head = read_bytes(file, at, 12)?;
    let count = u16_at(&head, 4) as usize;
    let records = read_bytes(file, at + 12, 16 * count)?;
    let table = |tag: &[u8; 4]| {
        (0..count).find_map(|i| {
            let r = &records[16 * i..16 * i + 16];
            (&r[..4] == tag).then(|| (u32_at(r, 8) as u64, u32_at(r, 12) as usize))
        })
    };
    let Some((name_at, name_len)) = table(b"name") else {
        return Ok(None);
    };
    let Some(family) = family_name(&read_bytes(file, name_at, name_len.min(1 << 20))?) else {
        return Ok(None);
    };
    let (mut weight, mut italic, mut mono) = (400, false, false);
    if let Some((os2, len)) = table(b"OS/2") {
        let os2 = read_bytes(file, os2, len.min(64))?;
        if os2.len() >= 64 {
            weight = u16_at(&os2, 4);
            italic = u16_at(&os2, 62) & 1 != 0;
            // PANOSE: Latin text family (2) with proportion "monospaced" (9).
            mono = os2[32] == 2 && os2[35] == 9;
        }
    }
    if let Some((post, len)) = table(b"post") {
        if len >= 16 {
            mono |= u32_at(&read_bytes(file, post, 16)?, 12) != 0;
        }
    }
    Ok(Some(Face {
        family,
        weight,
        italic,
        mono,
        wght_axis: table(b"fvar").is_some(),
        face: FaceRef {
            path: path.to_path_buf(),
            index,
        },
    }))
}

/// English family name: typographic family (16) before legacy family (1);
/// Windows Unicode records, en-US first.
fn family_name(table: &[u8]) -> Option<String> {
    if table.len() < 6 {
        return None;
    }
    let count = u16_at(table, 2) as usize;
    let strings = u16_at(table, 4) as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let r = 6 + 12 * i;
        let Some(rec) = table.get(r..r + 12) else {
            break;
        };
        let (platform, encoding, language, id) = (
            u16_at(rec, 0),
            u16_at(rec, 2),
            u16_at(rec, 4),
            u16_at(rec, 6),
        );
        if platform != 3 || !matches!(encoding, 0 | 1 | 10) || !matches!(id, 1 | 16) {
            continue;
        }
        let start = strings + u16_at(rec, 10) as usize;
        let Some(bytes) = table.get(start..start + u16_at(rec, 8) as usize) else {
            continue;
        };
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        let Ok(text) = String::from_utf16(&units) else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        let rank = u8::from(id == 1) * 2 + u8::from(language != 0x0409);
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text.trim().to_owned()));
        }
    }
    best.map(|(_, name)| name)
}

fn read_bytes(file: &mut File, at: u64, len: usize) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(at))?;
    let mut buf = Vec::with_capacity(len);
    file.take(len as u64).read_to_end(&mut buf)?;
    if buf.len() < len {
        return Err(std::io::ErrorKind::UnexpectedEof.into());
    }
    Ok(buf)
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(family: &str, weight: u16, italic: bool, mono: bool, file: &str) -> Face {
        Face {
            family: family.into(),
            weight,
            italic,
            mono,
            wght_axis: false,
            face: FaceRef {
                path: PathBuf::from(file),
                index: 0,
            },
        }
    }

    #[test]
    fn families_keep_monospace_regular_and_pick_bold() {
        let list = families(vec![
            face("Consolas", 400, false, true, "consola.ttf"),
            face("Consolas", 700, false, true, "consolab.ttf"),
            face("Consolas", 400, true, true, "consolai.ttf"),
            face("Segoe UI", 400, false, false, "segoeui.ttf"),
            face("Thin Mono", 100, false, true, "thin.ttf"),
            face("courier new", 400, false, true, "cour.ttf"),
        ]);
        let names: Vec<&str> = list.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Consolas", "courier new"]);
        assert_eq!(list[0].regular.path, PathBuf::from("consola.ttf"));
        assert_eq!(
            list[0].bold.as_ref().unwrap().path,
            PathBuf::from("consolab.ttf")
        );
        assert_eq!(list[1].bold, None);
    }

    #[test]
    fn name_table_prefers_english_typographic_family() {
        // format 0, 3 records: (3,1,0x804,1)="宋", (3,1,0x409,1)="Legacy", (3,1,0x409,16)="Typo"
        let strings: Vec<(u16, u16, &str)> =
            vec![(0x804, 1, "宋"), (0x409, 1, "Legacy"), (0x409, 16, "Typo")];
        let mut data = Vec::new();
        let mut records = Vec::new();
        for (lang, id, text) in &strings {
            let utf16: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
            for v in [3u16, 1, *lang, *id, utf16.len() as u16, data.len() as u16] {
                records.extend(v.to_be_bytes());
            }
            data.extend(utf16);
        }
        let mut table = Vec::new();
        for v in [0u16, strings.len() as u16, 6 + records.len() as u16] {
            table.extend(v.to_be_bytes());
        }
        table.extend(records);
        table.extend(data);
        assert_eq!(family_name(&table).as_deref(), Some("Typo"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_catalog_lists_consolas_with_bold() {
        let list = scan(&font_dirs());
        if let Some(consolas) = list.iter().find(|f| f.name == "Consolas") {
            assert!(consolas.bold.is_some());
            assert!(!list.iter().any(|f| f.name == "Segoe UI"));
        }
    }
}
