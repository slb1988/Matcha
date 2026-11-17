//! System font installation for the shared Matcha chrome.
//!
//! Font files are deliberately loaded at runtime. This keeps the binary small and
//! lets the UI use the same Segoe UI/Consolas metrics as a stock Windows install.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::{Path, PathBuf};

const UI_FONT: &str = "matcha-system-message";
const UI_BOLD_FONT: &str = "matcha-system-message-bold";
const EDITOR_FONT: &str = "matcha-consolas";
const EDITOR_BOLD_FONT: &str = "matcha-consolas-bold";
const CJK_FONT: &str = "matcha-microsoft-yahei";
/// The CJK fallback as used inside the editor chain: same file as [`CJK_FONT`],
/// tweaked (see [`editor_cjk_tweak`]) to sit on the editor font's baseline.
const EDITOR_CJK_FONT: &str = "matcha-editor-cjk";
/// Named family for emphasized UI text (dialog titles, section headings).
pub const STRONG_FAMILY: &str = "matcha-strong";
/// Named family for bold editor text (syntax keywords): Consolas Bold, same advance as Consolas.
pub const EDITOR_BOLD_FAMILY: &str = "matcha-editor-bold";

/// The paths used by the loader, kept separate to make fallback behavior testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontPlan {
    pub ui: PathBuf,
    pub ui_bold: PathBuf,
    pub editor: PathBuf,
    /// TTC member of [`Self::editor`].
    pub editor_index: u32,
    /// Empty = no bold face (bold editor text uses the regular face).
    pub editor_bold: PathBuf,
    pub editor_bold_index: u32,
    /// The bold face is [`Self::editor_bold`] rendered at `wght` 700 (variable font).
    pub editor_bold_wght: bool,
    pub cjk: PathBuf,
}

impl FontPlan {
    pub fn from_dir(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        Self {
            ui: dir.join("segoeui.ttf"),
            ui_bold: dir.join("segoeuib.ttf"),
            editor: dir.join("consola.ttf"),
            editor_index: 0,
            editor_bold: dir.join("consolab.ttf"),
            editor_bold_index: 0,
            editor_bold_wght: false,
            cjk: dir.join("msyh.ttc"),
        }
    }

    /// Use the installed `family` for the editor (Options › Appearance). The
    /// default Consolas skips the font scan; an unknown family keeps the
    /// current editor faces.
    pub fn with_editor_family(mut self, family: &str) -> Self {
        let default = bc_session::prefs::EditorFont::DEFAULT_FAMILY;
        if family.eq_ignore_ascii_case(default) && self.editor.is_file() {
            return self;
        }
        if let Some(found) = super::font_catalog::find(family) {
            self.editor = found.regular.path.clone();
            self.editor_index = found.regular.index;
            let bold = found
                .bold
                .clone()
                .or_else(|| found.variable_weight.then(|| found.regular.clone()));
            self.editor_bold_wght = found.bold.is_none() && bold.is_some();
            (self.editor_bold, self.editor_bold_index) =
                bold.map_or((PathBuf::new(), 0), |b| (b.path, b.index));
        }
        self
    }

    fn entries(&self) -> [(&'static str, &Path, u32); 5] {
        [
            (UI_FONT, self.ui.as_path(), 0),
            (UI_BOLD_FONT, self.ui_bold.as_path(), 0),
            (EDITOR_FONT, self.editor.as_path(), self.editor_index),
            (
                EDITOR_BOLD_FONT,
                self.editor_bold.as_path(),
                self.editor_bold_index,
            ),
            (CJK_FONT, self.cjk.as_path(), 0),
        ]
    }
}

fn windows_font_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("windir"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts")
}

/// System font lookup and font-file reads, started off the UI thread by
/// [`prefetch`] so they overlap window creation (the SPI query spawns a
/// PowerShell helper that costs ~300 ms).
type Resolved = (Option<SystemFont>, FontDefinitions);
static PREFETCH: std::sync::Mutex<Option<std::thread::JoinHandle<Resolved>>> =
    std::sync::Mutex::new(None);

/// The system part of the plan (UI fonts) as resolved at start-up; editor
/// family changes re-plan from here without repeating the SPI query.
static BASE: std::sync::Mutex<Option<(FontPlan, Option<SystemFont>)>> = std::sync::Mutex::new(None);

/// Start resolving the system fonts (and the saved editor `family`) in the
/// background; [`install`] joins it.
pub fn prefetch(family: String) {
    if let Ok(handle) = std::thread::Builder::new()
        .name("matcha-fonts".into())
        .spawn(move || resolve(&family))
    {
        *PREFETCH.lock().unwrap() = Some(handle);
    }
}

fn resolve(family: &str) -> Resolved {
    let mut plan = FontPlan::from_dir(windows_font_dir());
    let t = std::time::Instant::now();
    let system = system_message_font();
    crate::perf::span("fonts:system-query", t);
    if let Some(font) = &system {
        plan.ui = font.path.clone();
        if font.family == "Microsoft YaHei UI" {
            plan.ui_bold = windows_font_dir().join("msyhbd.ttc");
        }
    }
    *BASE.lock().unwrap_or_else(|e| e.into_inner()) = Some((plan.clone(), system.clone()));
    let definitions = definitions(&plan.with_editor_family(family), system.as_ref());
    (system, definitions)
}

/// [`load`] plus the system-font face selection.
fn definitions(plan: &FontPlan, system: Option<&SystemFont>) -> FontDefinitions {
    let t = std::time::Instant::now();
    let mut definitions = load(plan);
    crate::perf::span("fonts:load-files", t);
    // 微软雅黑 TTC 索引 0 是 Microsoft YaHei，索引 1 是 UI 家族。
    if system.is_some_and(|font| font.family == "Microsoft YaHei UI") {
        for key in [UI_FONT, UI_BOLD_FONT] {
            if let Some(data) = definitions.font_data.get_mut(key) {
                std::sync::Arc::make_mut(data).index = 1;
            }
        }
    }
    definitions
}

/// Switch the editor font family at run time (Options › Appearance › Editor font).
pub fn set_editor_family(ctx: &egui::Context, family: &str) {
    let base = BASE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let (plan, system) = base.unwrap_or_else(|| (FontPlan::from_dir(windows_font_dir()), None));
    ctx.set_fonts(definitions(
        &plan.with_editor_family(family),
        system.as_ref(),
    ));
}

/// Install system fonts, preserving egui's bundled fonts as the final fallback.
pub fn install(ctx: &egui::Context, family: &str) {
    let t = std::time::Instant::now();
    let prefetched = PREFETCH.lock().unwrap().take();
    let (system, definitions) = prefetched
        .and_then(|handle| handle.join().ok())
        .unwrap_or_else(|| resolve(family));
    crate::perf::span("fonts:resolved", t);
    // 诊断只在计时运行（`MATCHA_PERF` / `MATCHA_TABLE_PERF`）输出：命令行用户的终端里不应出现。
    if crate::perf::enabled() {
        if let Some(font) = &system {
            eprintln!(
                "matcha.font message={:?} size_px={} path={} editor=Consolas size_pt=10",
                font.family,
                font.size_px,
                font.path.display()
            );
        } else {
            eprintln!("matcha.font: SPI message font unavailable; using Segoe UI/bundled fallback");
        }
    }
    ctx.set_fonts(definitions);
    set_sizes(
        ctx,
        system.as_ref().map_or(super::theme::UI_SIZE, |f| f.size_px),
    );
}

#[derive(Clone, Debug)]
struct SystemFont {
    family: String,
    path: PathBuf,
    size_px: f32,
}

#[cfg(not(windows))]
fn system_message_font() -> Option<SystemFont> {
    None
}

/// 保持 bc-gui 的 forbid(unsafe_code)：通过隐藏的系统 PowerShell 助手调用 SPI。
/// 只读取消息字体与 Windows 字体注册表；不依赖或启动参考工具。
#[cfg(windows)]
fn system_message_font() -> Option<SystemFont> {
    use std::os::windows::process::CommandExt;
    const QUERY: &str = r#"
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false)
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MatchaMessageFont {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Font {
  public int height,width,escapement,orientation,weight;
  public byte italic,underline,strikeout,charset,outprecision,clipprecision,quality,pitch;
  [MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)] public string family;
 }
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Metrics {
  public uint size; public int borderWidth,scrollWidth,scrollHeight,captionWidth,captionHeight;
  public Font caption; public int smallWidth,smallHeight; public Font small;
  public int menuWidth,menuHeight; public Font menu,status,message; public int paddedBorderWidth;
 }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern bool SystemParametersInfo(uint action,uint param,ref Metrics data,uint flags);
 public static Font Read() { var m=new Metrics(); m.size=(uint)Marshal.SizeOf(m); if(!SystemParametersInfo(0x29,m.size,ref m,0)) throw new Exception("SPI_GETNONCLIENTMETRICS failed"); return m.message; }
}
'@
$f=[MatchaMessageFont]::Read()
$fonts=Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts'
$entry=$fonts.PSObject.Properties | Where-Object { $_.Name -notmatch 'Bold|Italic' -and ($_.Name -replace ' \(TrueType\)$','').Split('&').Trim() -contains $f.family } | Select-Object -First 1
if(!$entry){throw 'Font file not registered'}
$path=[string]$entry.Value
if(![IO.Path]::IsPathRooted($path)){$path=Join-Path $env:WINDIR ('Fonts/'+$path)}
Write-Output $f.family
Write-Output ([Math]::Abs($f.height))
Write-Output $path
"#;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", QUERY])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_system_font(std::str::from_utf8(&output.stdout).ok()?)
}

#[cfg(windows)]
fn parse_system_font(text: &str) -> Option<SystemFont> {
    let mut lines = text.trim().lines();
    let family = lines.next()?.trim().to_owned();
    let size_px = lines.next()?.trim().parse::<f32>().ok()?;
    let path = PathBuf::from(lines.next()?.trim());
    if family.is_empty() || !size_px.is_finite() || !(6.0..=96.0).contains(&size_px) {
        return None;
    }
    Some(SystemFont {
        family,
        path,
        size_px,
    })
}

/// Install a supplied plan. This is public for deterministic integration tests.
pub fn load(plan: &FontPlan) -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    let mut missing = Vec::new();

    for (name, path, index) in plan.entries() {
        if path.as_os_str().is_empty() {
            continue;
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                let mut data = FontData::from_owned(bytes);
                data.index = index;
                if name == EDITOR_BOLD_FONT && plan.editor_bold_wght {
                    data.tweak.coords =
                        egui::epaint::text::VariationCoords::new([(b"wght", 700.0)]);
                }
                definitions.font_data.insert(name.to_owned(), data.into());
            }
            Err(_) => missing.push(path.display().to_string()),
        }
    }
    // 编辑器链里的 CJK 回退：同一字体文件（共享字节），按编辑器字体缩放并对齐基线。
    if let Some(cjk) = definitions.font_data.get(CJK_FONT) {
        let mut data = FontData::clone(cjk);
        if let Some(tweak) = definitions
            .font_data
            .get(EDITOR_FONT)
            .and_then(|editor| editor_cjk_tweak(editor, &data))
        {
            data.tweak = tweak;
        }
        definitions
            .font_data
            .insert(EDITOR_CJK_FONT.to_owned(), data.into());
    }

    let mut proportional = Vec::new();
    if definitions.font_data.contains_key(UI_FONT) {
        proportional.push(UI_FONT.to_owned());
    }
    if definitions.font_data.contains_key(UI_BOLD_FONT) {
        proportional.push(UI_BOLD_FONT.to_owned());
    }
    if definitions.font_data.contains_key(CJK_FONT) {
        proportional.push(CJK_FONT.to_owned());
    }
    proportional.extend(
        definitions
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );

    let mut monospace = Vec::new();
    if definitions.font_data.contains_key(EDITOR_FONT) {
        monospace.push(EDITOR_FONT.to_owned());
    }
    if definitions.font_data.contains_key(EDITOR_CJK_FONT) {
        monospace.push(EDITOR_CJK_FONT.to_owned());
    }
    monospace.extend(
        definitions
            .families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
    // Named "strong" family for titles: bold face first, then the regular chain.
    let mut strong = Vec::new();
    if definitions.font_data.contains_key(UI_BOLD_FONT) {
        strong.push(UI_BOLD_FONT.to_owned());
    }
    strong.extend(proportional.iter().cloned());
    definitions
        .families
        .insert(FontFamily::Name(STRONG_FAMILY.into()), strong);
    // Bold editor family: Consolas Bold first, then the regular monospace chain.
    let mut editor_bold = Vec::new();
    if definitions.font_data.contains_key(EDITOR_BOLD_FONT) {
        editor_bold.push(EDITOR_BOLD_FONT.to_owned());
    }
    editor_bold.extend(monospace.iter().cloned());
    definitions
        .families
        .insert(FontFamily::Name(EDITOR_BOLD_FAMILY.into()), editor_bold);
    definitions
        .families
        .insert(FontFamily::Proportional, proportional);
    definitions
        .families
        .insert(FontFamily::Monospace, monospace);

    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !missing.is_empty() && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!(
            "matcha: system font fallback for {} missing file(s): {}",
            missing.len(),
            missing.join(", ")
        );
    }
    definitions
}

/// Vertical metrics in em units: (ascent, descent as a positive depth, line gap),
/// read the same way epaint reads them.
fn em_metrics(data: &FontData) -> Option<(f32, f32, f32)> {
    use skrifa::MetadataProvider;
    let font = skrifa::FontRef::from_index((*data.font).as_ref(), data.index).ok()?;
    let m = font.metrics(
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::default(),
    );
    let upm = f32::from(m.units_per_em);
    (upm > 0.0).then(|| (m.ascent / upm, -m.descent / upm, m.leading / upm))
}

/// GDI-style font linking for the editor's CJK fallback: scale the fallback so
/// its line height (ascent + descent + gap) matches the editor font's
/// (Microsoft YaHei 1.32 em vs Consolas 1.17 em, so unscaled ideographs dwarf
/// Latin text and spill below the row), then shift it so both baselines
/// coincide. egui otherwise centres a taller fallback's line box on the
/// primary's, which drops ideographs ~2 px below the Latin baseline and clips
/// their bottoms on 15 px rows.
fn editor_cjk_tweak(editor: &FontData, cjk: &FontData) -> Option<egui::epaint::text::FontTweak> {
    let (ea, ed, eg) = em_metrics(editor)?;
    let (ca, cd, cg) = em_metrics(cjk)?;
    if ca + cd + cg <= 0.0 {
        return None;
    }
    let scale = ((ea + ed + eg) / (ca + cd + cg)).clamp(0.5, 1.0);
    // epaint's baseline for a fallback glyph, in units of the font size:
    //   fallback ascent + (primary height - fallback height) / 2
    let natural = ca * scale + 0.5 * ((ea + ed + eg) - (ca + cd + cg) * scale);
    Some(egui::epaint::text::FontTweak {
        scale,
        // `y_offset_factor` is multiplied by size × scale.
        y_offset_factor: (ea - natural) / scale,
        ..Default::default()
    })
}

/// 按同一字号 token 安装字体；DPI 默认由 eframe 管理。
pub fn install_from_plan(ctx: &egui::Context, plan: &FontPlan) {
    ctx.set_fonts(load(plan));
    set_sizes(ctx, super::theme::UI_SIZE);
}

fn set_sizes(ctx: &egui::Context, ui_size: f32) {
    ctx.all_styles_mut(|style| {
        for text_style in [TextStyle::Body, TextStyle::Button] {
            style
                .text_styles
                .insert(text_style, FontId::proportional(ui_size));
        }
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(ui_size - 1.5));
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::monospace(super::theme::EDITOR_SIZE),
        );
    });
}

/// 快照固定物理像素与逻辑像素一一对应；普通模式不覆盖系统 DPI。
pub fn snapshot_scale(ctx: &egui::Context) {
    ctx.set_pixels_per_point(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_font_paths_are_safe_and_order_is_stable() {
        let plan = FontPlan::from_dir(Path::new("target/matcha-font-test-does-not-exist"));
        let ctx = egui::Context::default();
        install_from_plan(&ctx, &plan);
        let fonts = load(&plan);
        let proportional = fonts.families.get(&FontFamily::Proportional).unwrap();
        let monospace = fonts.families.get(&FontFamily::Monospace).unwrap();
        assert!(proportional.iter().all(|name| name != UI_FONT));
        assert!(monospace.iter().all(|name| name != EDITOR_FONT));
        assert!(!proportional.is_empty());
        assert!(!monospace.is_empty());
    }

    #[test]
    fn installed_families_prefer_system_fonts_before_bundled_fallbacks() {
        // 使用 egui 自带的有效字体字节构造固定输入，测试不依赖宿主 Windows。
        let bundled = FontDefinitions::default();
        let bytes = bundled.font_data.values().next().unwrap().font.clone();
        let dir = std::env::temp_dir().join(format!("matcha-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plan = FontPlan::from_dir(&dir);
        for (_, path, _) in plan.entries() {
            std::fs::write(path, bytes.as_ref()).unwrap();
        }
        let fonts = load(&plan);
        assert_eq!(
            &fonts.families[&FontFamily::Proportional][..3],
            &[UI_FONT, UI_BOLD_FONT, CJK_FONT]
        );
        assert_eq!(
            &fonts.families[&FontFamily::Monospace][..2],
            &[EDITOR_FONT, EDITOR_CJK_FONT]
        );
        assert!(fonts.families[&FontFamily::Monospace].len() > 2);
        assert_eq!(
            &fonts.families[&FontFamily::Name(EDITOR_BOLD_FAMILY.into())][..2],
            &[EDITOR_BOLD_FONT, EDITOR_FONT]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plan_names_windows_fonts() {
        let plan = FontPlan::from_dir(r"C:\Windows\Fonts");
        assert_eq!(plan.ui.file_name().unwrap(), "segoeui.ttf");
        assert_eq!(plan.ui_bold.file_name().unwrap(), "segoeuib.ttf");
        assert_eq!(plan.editor.file_name().unwrap(), "consola.ttf");
        assert_eq!(plan.editor_bold.file_name().unwrap(), "consolab.ttf");
        assert_eq!(plan.cjk.file_name().unwrap(), "msyh.ttc");
    }
}
