//! System font installation for the shared Matcha chrome.
//!
//! Font files are deliberately loaded at runtime. This keeps the binary small and
//! lets the UI use the same Segoe UI/Consolas metrics as a stock Windows install.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::{Path, PathBuf};

const UI_FONT: &str = "matcha-segoe-ui";
const UI_BOLD_FONT: &str = "matcha-segoe-ui-bold";
const EDITOR_FONT: &str = "matcha-consolas";
const CJK_FONT: &str = "matcha-microsoft-yahei";

/// The paths used by the loader, kept separate to make fallback behavior testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontPlan {
    pub ui: PathBuf,
    pub ui_bold: PathBuf,
    pub editor: PathBuf,
    pub cjk: PathBuf,
}

impl FontPlan {
    pub fn from_dir(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        Self {
            ui: dir.join("segoeui.ttf"),
            ui_bold: dir.join("segoeuib.ttf"),
            editor: dir.join("consola.ttf"),
            cjk: dir.join("msyh.ttc"),
        }
    }

    fn entries(&self) -> [(&'static str, &Path); 4] {
        [
            (UI_FONT, self.ui.as_path()),
            (UI_BOLD_FONT, self.ui_bold.as_path()),
            (EDITOR_FONT, self.editor.as_path()),
            (CJK_FONT, self.cjk.as_path()),
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

/// Install system fonts, preserving egui's bundled fonts as the final fallback.
pub fn install(ctx: &egui::Context) {
    install_from_plan(ctx, &FontPlan::from_dir(windows_font_dir()));
}

/// Install a supplied plan. This is public for deterministic integration tests.
pub fn load(plan: &FontPlan) -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    let mut missing = Vec::new();

    for (name, path) in plan.entries() {
        match std::fs::read(path) {
            Ok(bytes) => {
                definitions
                    .font_data
                    .insert(name.to_owned(), FontData::from_owned(bytes).into());
            }
            Err(_) => missing.push(path.display().to_string()),
        }
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
    if definitions.font_data.contains_key(CJK_FONT) {
        monospace.push(CJK_FONT.to_owned());
    }
    monospace.extend(
        definitions
            .families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
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

/// 按同一字号 token 安装字体；DPI 默认由 eframe 管理。
pub fn install_from_plan(ctx: &egui::Context, plan: &FontPlan) {
    ctx.set_fonts(load(plan));
    ctx.all_styles_mut(|style| {
        for text_style in [TextStyle::Body, TextStyle::Button, TextStyle::Small] {
            style
                .text_styles
                .insert(text_style, FontId::proportional(super::theme::UI_SIZE));
        }
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
        for (_, path) in plan.entries() {
            std::fs::write(path, bytes.as_ref()).unwrap();
        }
        let fonts = load(&plan);
        assert_eq!(
            &fonts.families[&FontFamily::Proportional][..3],
            &[UI_FONT, UI_BOLD_FONT, CJK_FONT]
        );
        assert_eq!(
            &fonts.families[&FontFamily::Monospace][..2],
            &[EDITOR_FONT, CJK_FONT]
        );
        assert!(fonts.families[&FontFamily::Monospace].len() > 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plan_names_windows_fonts() {
        let plan = FontPlan::from_dir(r"C:\Windows\Fonts");
        assert_eq!(plan.ui.file_name().unwrap(), "segoeui.ttf");
        assert_eq!(plan.ui_bold.file_name().unwrap(), "segoeuib.ttf");
        assert_eq!(plan.editor.file_name().unwrap(), "consola.ttf");
        assert_eq!(plan.cjk.file_name().unwrap(), "msyh.ttc");
    }
}
