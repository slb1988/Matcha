//! 确定性快照场景注册表：真实语料、真实视图、预置操作和静止帧门控。
use crate::chrome::{
    icons::Icon,
    menubar::{MenuItem, MenuItemKind, MenuSpec},
    toolbar::{ToolItem, ToolItemKind},
};
use crate::LaunchOptions;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const SCENES: &[&str] = &[
    "hex-basic",
    "hex-insertion",
    "hex-diffs-filter",
    "icon-catalog",
    "chrome-demo",
    "widget-gallery",
    "text-basic",
    "folder-basic",
    "text-diffs-filter",
    "text-cursor-on-diff",
    "folder-expanded",
    "folder-diffs-filter",
    "text-long-scrolled",
    "home",
    "text-menu-edit",
    "text-find-bar",
    "dlg-text-session-settings",
    "dlg-options",
    "table-basic",
    "table-cursor-on-diff",
    "folder-sync",
    "text-unsaved-modal",
    "merge-basic",
    "merge-conflict",
];
pub const DEFAULT_WINDOW_SIZE: [f32; 2] = [1384.0, 961.0];
/// Wave 2 合入正式语料后只需替换这两个常量。
pub const TEXT_LEFT: &str = "tools/visual/corpus/text-basic-left.txt";
pub const TEXT_RIGHT: &str = "tools/visual/corpus/text-basic-right.txt";
/// Long pair for the scrolled-thumbnail scene.
pub const TEXT_LONG_LEFT: &str = "tools/visual/corpus/text-long-left.txt";
pub const TEXT_LONG_RIGHT: &str = "tools/visual/corpus/text-long-right.txt";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneView {
    Icons,
    Chrome,
    Text,
    Folder,
    Home,
    Table,
    Hex,
    Merge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    FirstRow,
    ExpandAll,
    Differences,
    CursorOnDifference,
    LongScrolled,
    None,
}
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    pub name: &'static str,
    pub view: SceneView,
    pub preset: Preset,
}
/// hex-compare 快照语料：与参考截图同一字节（docs 语料库，SHA-256 校验过）。
pub const HEX_LEFT: &str = "docs/reference-parity/hex-compare/corpus/left.bin";
pub const HEX_RIGHT: &str = "docs/reference-parity/hex-compare/corpus/right.bin";
/// text-merge 快照语料：base 与两侧各自的修改（单侧改动、同改、重叠冲突、近距离冲突各一处）。
pub const MERGE_LEFT: &str = "tools/visual/corpus/merge-left.txt";
pub const MERGE_RIGHT: &str = "tools/visual/corpus/merge-right.txt";
pub const MERGE_BASE: &str = "tools/visual/corpus/merge-base.txt";
pub const REGISTRY: [Scene; 24] = [
    // text-merge：打开即定位到第一个冲突 / 解决第一个冲突之后。
    Scene {
        name: "merge-basic",
        view: SceneView::Merge,
        preset: Preset::FirstRow,
    },
    Scene {
        name: "merge-conflict",
        view: SceneView::Merge,
        preset: Preset::CursorOnDifference,
    },
    // hex-compare：主视图 / 插入区 / Diffs 过滤三场景（对照参考 01/06/22）。
    Scene {
        name: "hex-basic",
        view: SceneView::Hex,
        preset: Preset::FirstRow,
    },
    Scene {
        name: "hex-insertion",
        view: SceneView::Hex,
        preset: Preset::CursorOnDifference,
    },
    Scene {
        name: "hex-diffs-filter",
        view: SceneView::Hex,
        preset: Preset::Differences,
    },
    Scene {
        name: "icon-catalog",
        view: SceneView::Icons,
        preset: Preset::None,
    },
    Scene {
        name: "chrome-demo",
        view: SceneView::Chrome,
        preset: Preset::None,
    },
    // Design-system gallery: floating Window with every chrome::widgets
    // control plus an open drop-down menu (visual regression of the kit).
    Scene {
        name: "widget-gallery",
        view: SceneView::Chrome,
        preset: Preset::None,
    },
    Scene {
        name: "text-basic",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
    Scene {
        name: "folder-basic",
        view: SceneView::Folder,
        preset: Preset::None,
    },
    Scene {
        name: "text-diffs-filter",
        view: SceneView::Text,
        preset: Preset::Differences,
    },
    Scene {
        name: "text-cursor-on-diff",
        view: SceneView::Text,
        preset: Preset::CursorOnDifference,
    },
    Scene {
        name: "folder-expanded",
        view: SceneView::Folder,
        preset: Preset::ExpandAll,
    },
    Scene {
        name: "folder-diffs-filter",
        view: SceneView::Folder,
        preset: Preset::Differences,
    },
    Scene {
        name: "text-long-scrolled",
        view: SceneView::Text,
        preset: Preset::LongScrolled,
    },
    // Home defaults: New and Auto-saved expanded (Today open); New selected.
    Scene {
        name: "home",
        view: SceneView::Home,
        preset: Preset::None,
    },
    // Reference: "open top-level edit menu" (mouse-opened, nothing highlighted).
    Scene {
        name: "text-menu-edit",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
    // Reference: "Search > Find; docked bar" with "alpha" in the field.
    Scene {
        name: "text-find-bar",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
    // Dialog scenes: the dialog's client area fills a window of the
    // reference client size (PrintWindow capture of the dialog alone).
    Scene {
        name: "dlg-text-session-settings",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
    Scene {
        name: "dlg-options",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
    // Reference: table pair opened, no operation.
    Scene {
        name: "table-basic",
        view: SceneView::Table,
        preset: Preset::FirstRow,
    },
    // Reference: "left grid differing cell" clicked (C3).
    Scene {
        name: "table-cursor-on-diff",
        view: SceneView::Table,
        preset: Preset::CursorOnDifference,
    },
    // Reference: folder pair opened in Folder Sync, no operation.
    Scene {
        name: "folder-sync",
        view: SceneView::Folder,
        preset: Preset::FirstRow,
    },
    Scene {
        name: "text-unsaved-modal",
        view: SceneView::Text,
        preset: Preset::FirstRow,
    },
];
/// Client size for scenes that capture a dialog alone.
pub fn scene_window_size(scene: &str) -> Option<[f32; 2]> {
    match scene {
        "dlg-text-session-settings" => Some([583.0, 563.0]),
        "dlg-options" => Some([859.0, 655.0]),
        _ => None,
    }
}
pub fn is_scene(scene: Option<&str>) -> bool {
    scene.is_some_and(|s| REGISTRY.iter().any(|r| r.name == s))
}
pub fn is_demo(scene: &str) -> bool {
    matches!(scene, "icon-catalog" | "chrome-demo" | "widget-gallery")
}
/// 不忽略 I/O 错误；未知场景和缺失语料在打开窗口前失败。
pub fn prepare_options(mut options: LaunchOptions) -> std::io::Result<LaunchOptions> {
    let Some(scene) = options.snapshot.as_deref() else {
        return Ok(options);
    };
    let entry = REGISTRY.iter().find(|r| r.name == scene).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown snapshot scene: {scene}"),
        )
    })?;
    if options.bench.is_some() || options.bench_dir {
        return Err(std::io::Error::other(
            "snapshot and benchmark modes are mutually exclusive",
        ));
    }
    if let Some(size) = scene_window_size(scene) {
        options.window_size = Some(size);
    }
    options
        .snapshot_out
        .get_or_insert_with(|| PathBuf::from(format!("target/visual/matcha/{scene}.png")));
    let inputs = match entry.view {
        SceneView::Text if entry.preset == Preset::LongScrolled => Some((
            PathBuf::from(TEXT_LONG_LEFT),
            PathBuf::from(TEXT_LONG_RIGHT),
        )),
        SceneView::Text => Some((PathBuf::from(TEXT_LEFT), PathBuf::from(TEXT_RIGHT))),
        SceneView::Table => Some((
            PathBuf::from("tools/visual/corpus/table-left.csv"),
            PathBuf::from("tools/visual/corpus/table-right.csv"),
        )),
        SceneView::Folder => Some((
            PathBuf::from("tools/visual/corpus/folder-left"),
            PathBuf::from("tools/visual/corpus/folder-right"),
        )),
        SceneView::Hex => Some((PathBuf::from(HEX_LEFT), PathBuf::from(HEX_RIGHT))),
        _ => None,
    };
    options.left = None;
    options.right = None;
    options.merge = None;
    if entry.view == SceneView::Merge {
        for path in [MERGE_LEFT, MERGE_RIGHT, MERGE_BASE] {
            std::fs::metadata(path)?;
        }
        options.merge = Some(crate::merge::MergeLaunch::new(bc_script::cli::MergeSpec {
            left: PathBuf::from(MERGE_LEFT),
            right: PathBuf::from(MERGE_RIGHT),
            center: PathBuf::from(MERGE_BASE),
            output: Some(PathBuf::from("target/visual/matcha/merge-output.txt")),
        }));
    }
    if let Some((left, right)) = inputs {
        std::fs::metadata(&left)?;
        std::fs::metadata(&right)?;
        options.left = Some(left);
        options.right = Some(right);
    }
    Ok(options)
}
pub type CaptureResult = Arc<Mutex<Option<Result<(), String>>>>;
thread_local! { static HARNESS_WAKE:std::cell::Cell<bool>=const { std::cell::Cell::new(false) }; }
/// 两帧门控计数独立于总帧数；后台重新变忙或任意实际重绘请求都会归零。
#[derive(Default, Debug)]
struct QuietFrames(u8);
impl QuietFrames {
    fn observe(&mut self, ready: bool, repainted: bool) -> bool {
        self.0 = if ready && !repainted {
            self.0.saturating_add(1)
        } else {
            0
        };
        self.0 >= 2
    }
}
/// 只在快照模式安装的观察器；不替换 eframe 自己的重绘回调。
pub struct Capture {
    ready: AtomicBool,
    requested: AtomicBool,
    repaint_requests: AtomicU64,
    quiet: Mutex<QuietFrames>,
    started: Instant,
    path: PathBuf,
    result: CaptureResult,
    expected: [usize; 2],
}
impl Capture {
    pub fn install(
        ctx: &egui::Context,
        path: PathBuf,
        size: [f32; 2],
        result: CaptureResult,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            ready: AtomicBool::new(false),
            requested: AtomicBool::new(false),
            repaint_requests: AtomicU64::new(0),
            quiet: Mutex::new(QuietFrames::default()),
            started: Instant::now(),
            path,
            result,
            // 逻辑点；截图回调按整数倍缩放校验（install 时 zoom_factor 恒为 1）。
            expected: [size[0] as usize, size[1] as usize],
        });
        let observer = state.clone();
        ctx.set_repaint_observer(move |_| {
            if !HARNESS_WAKE.get() {
                observer.repaint_requests.fetch_add(1, Ordering::SeqCst);
            }
        });
        let end = state.clone();
        ctx.on_end_pass(
            "snapshot-quiet-gate",
            Arc::new(move |ui| end.end_pass(ui.ctx())),
        );
        state
    }
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }
    fn end_pass(&self, ctx: &egui::Context) {
        if self.requested.load(Ordering::SeqCst) {
            return;
        }
        if self.started.elapsed() > Duration::from_secs(60) {
            *self.result.lock().unwrap() =
                Some(Err("snapshot timeout waiting for loaded, quiet view".into()));
            self.requested.store(true, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let requests = self.repaint_requests.swap(0, Ordering::SeqCst);
        let repainted = requests > 0;
        if std::env::var_os("MATCHA_SNAPSHOT_TRACE").is_some() {
            eprintln!(
                "snapshot.gate ready={} requests={requests} causes={:?}",
                self.ready.load(Ordering::SeqCst),
                ctx.repaint_causes()
            );
        }
        let ready = self.ready.load(Ordering::SeqCst);
        if self.quiet.lock().unwrap().observe(ready, repainted) {
            self.requested.store(true, Ordering::SeqCst);
            let path = self.path.clone();
            let result = self.result.clone();
            let ctx = ctx.clone();
            let close = ctx.clone();
            let expected = self.expected;
            ctx.request_screenshot(move |image| {
                let saved = if image.size == expected {
                    crate::bench::save_png(&path, &image).map_err(|e| e.to_string())
                } else {
                    // HiDPI（Retina 2x 等）：物理像素 = 逻辑 × 整数倍也接受。
                    let (w, h) = (image.width(), image.height());
                    let scaled = (2..=3).any(|s| w == expected[0] * s && h == expected[1] * s);
                    if scaled {
                        crate::bench::save_png(&path, &image).map_err(|e| e.to_string())
                    } else {
                        Err(format!(
                            "snapshot size {:?} != requested {expected:?}",
                            image.size
                        ))
                    }
                };
                if saved.is_ok() {
                    println!(
                        "snapshot.saved={} size={}x{} ready=true quiet_frames=2",
                        path.display(),
                        image.width(),
                        image.height()
                    );
                }
                *result.lock().unwrap() = Some(saved);
                close.send_viewport_cmd(egui::ViewportCommand::Close);
            });
        } else {
            // 主动采样下一帧不是视图的重绘请求；线程局部屏蔽不隐藏后台线程通知。
            HARNESS_WAKE.set(true);
            ctx.request_repaint_after(Duration::from_millis(30));
            HARNESS_WAKE.set(false);
        }
    }
}

fn menu_item(label: &str, mnemonic: char, shortcut: &str, id: &str) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic: Some(mnemonic),
        shortcut: (!shortcut.is_empty()).then(|| shortcut.into()),
        kind: MenuItemKind::Action,
        enabled: true,
        action_id: id.into(),
    }
}
fn separator() -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Separator,
        enabled: false,
        ..MenuItem::action("", "")
    }
}
fn submenu(label: &str, mnemonic: char, items: Vec<MenuItem>) -> MenuItem {
    MenuItem {
        mnemonic: Some(mnemonic),
        kind: MenuItemKind::Submenu(items),
        ..MenuItem::action(label, label)
    }
}
/// 完整的文本审阅示例菜单；未实现的产品命令必须由视图设为 disabled。
pub fn demo_menus() -> Vec<MenuSpec> {
    let item = menu_item;
    let mut print = item("Print", 'P', "Ctrl+P", "print");
    print.enabled = false;
    let menu = |title: &str, mnemonic, items| MenuSpec {
        title: title.into(),
        mnemonic: Some(mnemonic),
        items,
    };
    vec![
        menu(
            "Session",
            'S',
            vec![
                submenu(
                    "New Session",
                    'N',
                    vec![
                        item("Text Compare", 'T', "", "new-text"),
                        item("Folder Compare", 'F', "", "new-folder"),
                        item("Table Compare", 'A', "", "new-table"),
                    ],
                ),
                item("Open Session", 'O', "Ctrl+O", "open-session"),
                item("Save Session", 'S', "Ctrl+S", "save-session"),
                item("Save Session As", 'A', "", "save-session-as"),
                separator(),
                item("Session Settings", 'E', "", "settings"),
                item("Session Manager", 'M', "", "sessions"),
                separator(),
                item("Close Tab", 'C', "Ctrl+W", "close"),
                item("Exit", 'X', "Alt+F4", "exit"),
            ],
        ),
        menu(
            "File",
            'F',
            vec![
                item("Open File", 'O', "", "open-file"),
                submenu(
                    "Open With",
                    'W',
                    vec![item("Associated Application", 'A', "", "open-with")],
                ),
                item("Save", 'S', "Ctrl+S", "save"),
                item("Save As", 'A', "", "save-as"),
                separator(),
                item("Reload", 'R', "Ctrl+R", "reload"),
                item("Swap Sides", 'W', "Ctrl+U", "swap"),
                separator(),
                print,
                item("File Information", 'I', "", "info"),
            ],
        ),
        menu(
            "Edit",
            'E',
            vec![
                item("Undo", 'U', "Ctrl+Z", "undo"),
                item("Redo", 'R', "Ctrl+Y", "redo"),
                separator(),
                item("Cut", 'T', "Ctrl+X", "cut"),
                item("Copy", 'C', "Ctrl+C", "copy"),
                item("Paste", 'P', "Ctrl+V", "paste"),
                item("Delete", 'D', "Del", "delete"),
                item("Select All", 'A', "Ctrl+A", "select-all"),
                separator(),
                item("Copy to Left", 'L', "Ctrl+Shift+R", "copy-left"),
                item("Copy to Right", 'I', "Ctrl+R", "copy-right"),
            ],
        ),
        menu(
            "Search",
            'A',
            vec![
                item("Find", 'F', "Ctrl+F", "find"),
                item("Find Next", 'N', "F3", "find-next"),
                item("Find Previous", 'P', "Shift+F3", "find-prev"),
                item("Replace", 'R', "Ctrl+H", "replace"),
                separator(),
                item("Next Difference", 'D', "Ctrl+N", "next"),
                item("Previous Difference", 'V', "Ctrl+P", "prev"),
                item("Go to Line", 'G', "Ctrl+G", "goto"),
            ],
        ),
        menu(
            "View",
            'V',
            vec![
                MenuItem {
                    kind: MenuItemKind::Radio { selected: true },
                    ..item("All", 'A', "", "all")
                },
                MenuItem {
                    kind: MenuItemKind::Radio { selected: false },
                    ..item("Differences", 'D', "", "diffs")
                },
                MenuItem {
                    kind: MenuItemKind::Radio { selected: false },
                    ..item("Same", 'S', "", "same")
                },
                separator(),
                MenuItem {
                    kind: MenuItemKind::Check { checked: true },
                    ..item("Context", 'C', "", "context")
                },
                MenuItem {
                    kind: MenuItemKind::Check { checked: false },
                    ..item("Ignore Unimportant Differences", 'I', "", "minor")
                },
                separator(),
                submenu(
                    "Layout",
                    'L',
                    vec![
                        item("Side by Side", 'S', "", "side-by-side"),
                        item("Over Under", 'O', "", "over-under"),
                    ],
                ),
                MenuItem {
                    kind: MenuItemKind::Check { checked: true },
                    ..item("Line Numbers", 'N', "", "numbers")
                },
                MenuItem {
                    kind: MenuItemKind::Check { checked: true },
                    ..item("Line Details", 'T', "", "details")
                },
            ],
        ),
        menu(
            "Tools",
            'T',
            vec![
                item("Options", 'O', "", "options"),
                item("File Formats", 'F', "", "formats"),
                item("Text Replacements", 'R', "", "replacements"),
                separator(),
                item("Import Settings", 'I', "", "import"),
                item("Export Settings", 'E', "", "export"),
            ],
        ),
        menu(
            "Help",
            'H',
            vec![
                item("Contents", 'C', "F1", "help"),
                item("Keyboard Shortcuts", 'K', "", "keys"),
                separator(),
                item("About Matcha", 'A', "", "about"),
            ],
        ),
    ]
}
pub fn demo_tools() -> Vec<ToolItem> {
    use ToolItemKind::*;
    [
        ("Home", "home", Normal, Icon::Home, Some("home")),
        (
            "Sessions",
            "sessions",
            Split,
            Icon::Sessions,
            Some("sessions"),
        ),
        ("All", "all", Toggle, Icon::All, Some("filter")),
        ("Diffs", "diffs", Toggle, Icon::Diffs, Some("filter")),
        ("Same", "same", Toggle, Icon::Same, Some("filter")),
        ("Context", "context", Toggle, Icon::Context, None),
        ("Minor", "minor", Toggle, Icon::Minor, None),
        ("Rules", "rules", Normal, Icon::Rules, Some("format")),
        ("Format", "format", Split, Icon::Format, Some("format")),
        ("Copy", "copy", Normal, Icon::Copy, Some("copy")),
        ("Next Section", "next", Normal, Icon::Next, Some("section")),
        ("Prev Section", "prev", Normal, Icon::Prev, Some("section")),
        ("Swap", "swap", Normal, Icon::Swap, Some("reload")),
        ("Reload", "reload", Normal, Icon::Reload, Some("reload")),
    ]
    .into_iter()
    .map(|(label, id, kind, icon, group)| ToolItem {
        label: label.into(),
        action_id: id.into(),
        kind,
        enabled: true,
        selected: matches!(id, "all" | "context"),
        icon,
        group: group.map(str::to_owned),
    })
    .collect()
}
/// 演示页也保持状态，不在每一帧重置输入、过滤器或动作结果。
pub struct ChromeDemo {
    paths: [String; 2],
    tools: Vec<ToolItem>,
    last_action: String,
    gallery: Gallery,
}
/// State of the `widget-gallery` scene.
struct Gallery {
    check: bool,
    radio: u8,
    combo: u8,
    text: String,
    row: usize,
    menu_opened: bool,
}
impl Default for Gallery {
    fn default() -> Self {
        Self {
            check: true,
            radio: 1,
            combo: 0,
            text: "C:\\work\\left.txt".into(),
            row: 1,
            menu_opened: false,
        }
    }
}
impl Default for ChromeDemo {
    fn default() -> Self {
        Self {
            paths: [TEXT_LEFT.into(), TEXT_RIGHT.into()],
            tools: demo_tools(),
            last_action: String::new(),
            gallery: Gallery::default(),
        }
    }
}
impl ChromeDemo {
    fn gallery(&mut self, ui: &mut egui::Ui) {
        use crate::chrome::{controls::Role, popup, theme, widgets, window::Window};
        let g = &mut self.gallery;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::CANVAS))
            .show(ui, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    let menu = ui.add(widgets::Button::new("Sessions").dropdown(true));
                    let id = menu.id.with("menu-below");
                    if !g.menu_opened {
                        popup::open_menu(
                            ui.ctx(),
                            id,
                            menu.rect.left_bottom() + egui::vec2(0.0, 2.0),
                            Some(menu.rect),
                        );
                        g.menu_opened = true;
                    }
                    popup::menu_below(&menu, || {
                        vec![
                            popup::item::action("Home", "home", true),
                            popup::item::action("Session Settings...", "settings", true),
                            popup::item::check("Show Toolbar", "toolbar", true),
                            popup::item::separator(),
                            popup::item::radio("Compare Values", "values", true),
                            popup::item::radio("Compare Formulas", "formulas", false),
                            popup::item::separator(),
                            popup::item::action("Open Session", "open", false),
                        ]
                    });
                });
            });
        Window::new("Widget gallery")
            .default_width(460.0)
            .show(ui.ctx(), |ui| {
                widgets::section(ui, "Buttons");
                ui.horizontal(|ui| {
                    widgets::button(ui, "Secondary");
                    widgets::primary_button(ui, "Primary");
                    ui.add(widgets::Button::new("Delete").danger());
                    ui.add(widgets::Button::new("Toggled").selected(true));
                    ui.add_enabled(false, widgets::Button::new("Disabled"));
                });
                widgets::section(ui, "Choices");
                ui.horizontal(|ui| {
                    widgets::checkbox(ui, &mut g.check, "Checked");
                    let mut off = false;
                    widgets::checkbox(ui, &mut off, "Unchecked");
                    ui.add_enabled(false, widgets::Checkbox::new(&mut off, "Disabled"));
                });
                ui.horizontal(|ui| {
                    widgets::radio_value(ui, &mut g.radio, 0, "Complete");
                    widgets::radio_value(ui, &mut g.radio, 1, "Fast");
                    widgets::radio_value(ui, &mut g.radio, 2, "None");
                });
                widgets::section(ui, "Fields");
                ui.horizontal(|ui| {
                    widgets::text_field_width(ui, &mut g.text, 220.0);
                    widgets::Combo::new("gallery-combo", ["UTF-8", "UTF-16 LE", "GBK (936)"][g.combo as usize])
                        .width(140.0)
                        .show(ui, |list| {
                            list.selectable_value(&mut g.combo, 0, "UTF-8");
                            list.selectable_value(&mut g.combo, 1, "UTF-16 LE");
                            list.selectable_value(&mut g.combo, 2, "GBK (936)");
                        });
                });
                widgets::section(ui, "List");
                for (i, row) in ["C/C++", "Rust", "XML"].iter().enumerate() {
                    widgets::list_value(ui, &mut g.row, i, *row);
                }
                widgets::hint(ui, "Hint text explains a setting in a muted color and wraps.");
                widgets::dialog_buttons(ui, &[("Cancel", Role::Secondary), ("OK", Role::Primary)]);
            });
    }
    pub fn show(&mut self, ui: &mut egui::Ui, scene: &str) {
        use crate::chrome::{
            menubar, pathbar,
            statusbar::{self, StatusField},
            toolbar,
        };
        if scene == "widget-gallery" {
            self.gallery(ui);
            return;
        }
        if scene == "icon-catalog" {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Matcha — original vector icon catalog");
                ui.label("Toolbar 16 px · Small 12 px · Disabled · Original Painter geometry");
                ui.add_space(16.0);
                egui::Grid::new("icon-catalog")
                    .num_columns(6)
                    .spacing(egui::vec2(16.0, 12.0))
                    .show(ui, |ui| {
                        for (index, icon) in Icon::ALL.iter().enumerate() {
                            ui.allocate_ui(egui::vec2(170.0, 66.0), |ui| {
                                ui.horizontal(|ui| {
                                    for (size, disabled) in
                                        [(16.0, false), (12.0, false), (16.0, true)]
                                    {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(32.0, 28.0),
                                            egui::Sense::hover(),
                                        );
                                        let mut painter = ui.painter().clone();
                                        if disabled {
                                            painter.set_opacity(0.35);
                                        }
                                        crate::chrome::icons::paint(
                                            &painter,
                                            egui::Rect::from_center_size(
                                                rect.center(),
                                                egui::Vec2::splat(size),
                                            ),
                                            *icon,
                                        );
                                    }
                                });
                                ui.label(icon.label());
                            });
                            if index % 6 == 5 {
                                ui.end_row();
                            }
                        }
                    });
            });
            return;
        }
        egui::Panel::top("demo-menu")
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if let Some(action) = menubar::show(ui, &demo_menus()) {
                    self.last_action = action;
                }
            });
        egui::Panel::top("demo-toolbar")
            .frame(
                egui::Frame::new()
                    .fill(crate::chrome::theme::PANEL_BG)
                    .inner_margin(2),
            )
            .show(ui, |ui| {
                let response = toolbar::show(ui, &self.tools);
                if let Some(id) = response.clicked {
                    toolbar::apply_toggle(&mut self.tools, &id);
                    self.last_action = id;
                }
                if let Some(id) = response.dropdown {
                    self.last_action = format!("{id} dropdown");
                }
            });
        egui::Panel::bottom("demo-status")
            .frame(
                egui::Frame::new()
                    .fill(crate::chrome::theme::PANEL_BG)
                    .inner_margin(2),
            )
            .show(ui, |ui| {
                statusbar::show(
                    ui,
                    &[
                        StatusField {
                            icon: Some(Icon::Status),
                            ..StatusField::fixed("3 difference section(s)", 290.0)
                        },
                        StatusField::flexible("Same"),
                        StatusField::fixed("Insert", 90.0),
                        StatusField::flexible("Load time: 0.00 seconds"),
                    ],
                );
            });
        egui::Panel::bottom("demo-details")
            .exact_size(44.0)
            .show(ui, |ui| {
                ui.monospace("→ alpha↵");
                ui.separator();
                ui.monospace("← alpha↵");
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(crate::chrome::theme::PANEL_BG)
                    .inner_margin(4),
            )
            .show(ui, |ui| {
                let available = ui.available_height();
                ui.columns(2, |columns| {
                    for (side, column) in columns.iter_mut().enumerate() {
                        column.push_id(side, |ui| {
                            let response = pathbar::show_with_history(
                                ui,
                                &mut self.paths[side],
                                &[TEXT_LEFT.into(), TEXT_RIGHT.into()],
                                false,
                            );
                            if let Some(id) = response.action {
                                self.last_action = id;
                            }
                            ui.horizontal(|ui| {
                                ui.label("2026-01-02 03:04:05");
                                ui.label(if side == 0 { "41 bytes" } else { "50 bytes" });
                                dropdown_label(ui, "Everything Else");
                                dropdown_label(ui, "UTF-8");
                                ui.label("UNIX");
                            });
                            let (pane, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), (available - 80.0).max(200.0)),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_stroke(
                                pane,
                                0.0,
                                egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
                                egui::StrokeKind::Inside,
                            );
                            let lines = if side == 0 {
                                [
                                    "alpha",
                                    "beta",
                                    "gamma line",
                                    "",
                                    "delta",
                                    "epsilon",
                                    "zeta",
                                ]
                            } else {
                                [
                                    "alpha",
                                    "beta changed",
                                    "gamma line",
                                    "new line",
                                    "delta",
                                    "",
                                    "zeta",
                                ]
                            };
                            for (row, text) in lines.iter().enumerate() {
                                let rect = egui::Rect::from_min_size(
                                    pane.min + egui::vec2(36.0, row as f32 * 15.0 + 1.0),
                                    egui::vec2(pane.width() - 38.0, 15.0),
                                );
                                let changed = matches!(row, 1 | 3 | 5);
                                ui.painter().rect_filled(
                                    rect,
                                    0.0,
                                    if changed {
                                        crate::chrome::theme::Theme::default().difference_fill
                                    } else {
                                        egui::Color32::WHITE
                                    },
                                );
                                ui.painter().text(
                                    rect.left_top() - egui::vec2(14.0, 0.0),
                                    egui::Align2::RIGHT_TOP,
                                    (row + 1).to_string(),
                                    egui::FontId::monospace(12.0),
                                    egui::Color32::GRAY,
                                );
                                ui.painter().text(
                                    rect.min,
                                    egui::Align2::LEFT_TOP,
                                    text,
                                    egui::FontId::monospace(12.0),
                                    if changed {
                                        egui::Color32::RED
                                    } else {
                                        egui::Color32::BLACK
                                    },
                                );
                            }
                            statusbar::show(
                                ui,
                                &[
                                    StatusField::fixed("1: 1", 96.0),
                                    StatusField::flexible("Default text"),
                                ],
                            );
                        });
                    }
                });
            });
    }
}

fn dropdown_label(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 12.0), egui::Sense::hover());
        crate::chrome::icons::dropdown_triangle(ui.painter(), rect.center(), egui::Color32::BLACK);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_requires_two_quiet_ready_frames_and_resets_on_work() {
        let mut gate = QuietFrames::default();
        assert!(!gate.observe(false, false));
        assert!(!gate.observe(true, false));
        assert!(!gate.observe(true, true));
        assert!(!gate.observe(true, false));
        assert!(gate.observe(true, false));
        assert!(!gate.observe(false, false));
    }
    #[test]
    fn registry_maps_compare_scenes_to_real_views() {
        assert_eq!(REGISTRY.len(), SCENES.len());
        // Every listed scene resolves, and lookups are by name (order-free).
        assert!(SCENES.iter().all(|s| is_scene(Some(s))));
        assert_eq!(
            REGISTRY
                .iter()
                .find(|r| r.name == "text-basic")
                .unwrap()
                .view,
            SceneView::Text
        );
        assert_eq!(
            REGISTRY
                .iter()
                .find(|r| r.name == "folder-basic")
                .unwrap()
                .view,
            SceneView::Folder
        );
        assert!(!is_scene(Some("unknown")));
    }
    #[test]
    fn png_codec_roundtrip_and_repeat_are_byte_identical() {
        let dir = std::env::temp_dir().join(format!("matcha-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = egui::ColorImage::new(
            [2, 2],
            vec![
                egui::Color32::RED,
                egui::Color32::BLUE,
                egui::Color32::GREEN,
                egui::Color32::WHITE,
            ],
        );
        for name in ["a.png", "b.png"] {
            crate::bench::save_png(&dir.join(name), &image).unwrap();
        }
        let bytes = std::fs::read(dir.join("a.png")).unwrap();
        assert_eq!(bytes, std::fs::read(dir.join("b.png")).unwrap());
        let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (2, 2));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
