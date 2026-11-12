//! 可编辑路径组合框。调用者用 `push_id` 区分左右路径，负责历史持久化及文件操作。
//! 外观对齐经典浅色参考：25 px 白底编辑框（#8D8D8D 边）、右端内嵌历史下拉小箭头、
//! 右侧三个无边框 16 px 图标按钮（历史/浏览/保存），浏览按钮带静态 ▼ 字形。
//! 编辑框带经典右键菜单（Undo/Cut/Copy/Paste/Delete/Select All/Browse）。
use crate::chrome::icons::{self, Icon};
use crate::chrome::menubar::{MenuItem, MenuItemKind};
use eframe::egui::{self, Stroke};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathbarResponse {
    pub action: Option<String>,
    pub submitted: bool,
}
/// 无历史的简便入口；`show_with_history` 返回 Enter 提交和历史选择。
pub fn show(ui: &mut egui::Ui, path: &mut String) -> Option<String> {
    show_with_history(ui, path, &[], false).action
}
pub fn show_with_history(
    ui: &mut egui::Ui,
    path: &mut String,
    history: &[String],
    select_all: bool,
) -> PathbarResponse {
    let mut result = PathbarResponse::default();
    let edit_id = ui.make_persistent_id("pathbar-edit");
    clean_paste_events(ui.ctx(), edit_id);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.visuals_mut().widgets.inactive.bg_stroke =
            Stroke::new(1.0, super::theme::LINE_STRONG);
        ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.0, super::theme::TEXT_FAINT);
        ui.visuals_mut().extreme_bg_color = super::theme::SURFACE;
        let edit_width = (ui.available_width() - 82.0).max(40.0);
        // 与 add_sized 相同的「居中并填满」布局，但保留 TextEditOutput：
        // galley 供右键点击定位光标。
        let output = ui
            .allocate_ui_with_layout(
                egui::vec2(edit_width, 28.0),
                egui::Layout::centered_and_justified(ui.layout().main_dir()),
                |ui| {
                    egui::TextEdit::singleline(path)
                        .id(edit_id)
                        .margin(egui::vec2(8.0, 2.0))
                        .show(ui)
                },
            )
            .inner;
        let edit = output.response.response.clone();
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            result.submitted = true;
            result.action = Some("path-submit".into());
        }
        // 快照场景复刻参考「打开后全选路径」状态。
        if select_all {
            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit.id)
                .unwrap_or_default();
            let len = path.chars().count();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                )));
            state.store(ui.ctx(), edit.id);
            ui.memory_mut(|m| m.request_focus(edit.id));
        }
        // 右键编辑菜单（参考系统编辑控件）。
        if let Some(action) = edit_menu(ui, &edit, &output.galley, output.galley_pos, path) {
            result.action = Some(action);
        }
        // 编辑框持焦时 Alt+B 触发浏览（对应右键菜单上的快捷键标注）。
        // 从事件本身读修饰键（InputState.modifiers 只由 ModifiersChanged 维护）。
        let alt_b = ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: egui::Key::B, pressed: true, modifiers, .. } if modifiers.alt && !modifiers.command)
            })
        });
        if edit.has_focus() && alt_b {
            result.action = Some("browse".into());
        }
        // 编辑框右端内嵌历史下拉箭头。
        let chevron = egui::Rect::from_min_size(
            egui::pos2(edit.rect.right() - 22.0, edit.rect.top()),
            egui::vec2(20.0, edit.rect.height()),
        );
        let chev = ui.interact(
            chevron,
            ui.scope_id().with("path-history-chevron"),
            egui::Sense::click(),
        );
        let c = chevron.center();
        ui.painter().add(egui::Shape::line(
            vec![
                c + egui::vec2(-3.5, -1.5),
                c + egui::vec2(0.0, 2.0),
                c + egui::vec2(3.5, -1.5),
            ],
            Stroke::new(1.4, super::theme::TEXT_MUTED),
        ));
        egui::Popup::from_toggle_button_response(&chev)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if history.is_empty() {
                    ui.add_enabled(false, egui::Label::new("No recent paths"));
                }
                for entry in history {
                    if ui.selectable_label(*path == *entry, entry).clicked() {
                        *path = entry.clone();
                        result.submitted = true;
                        result.action = Some("history-select".into());
                        ui.close();
                    }
                }
            });
        chev.on_hover_text("Path history");
        // 右侧无边框图标按钮：历史 / 浏览(带 ▼) / 保存。
        ui.add_space(9.0);
        for (i, (icon, id, tip, extra)) in [
            (Icon::History, "history", "Path history", 17.0),
            (Icon::Browse, "browse", "Browse folder", 28.0),
            (Icon::Save, "save", "Save path", 17.0),
        ]
        .into_iter()
        .enumerate()
        {
            if i == 2 {
                ui.add_space(8.0);
            }
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(extra, 28.0), egui::Sense::click());
            if response.clicked() {
                result.action = Some(id.into());
            }
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect.shrink(1.0), super::theme::RADIUS_MD, super::theme::HOVER);
            }
            icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(
                    rect.left_center() + egui::vec2(8.0, 0.0),
                    egui::vec2(16.0, 16.0),
                ),
                icon,
            );
            if id == "browse" {
                let c = rect.left_center() + egui::vec2(23.0, 0.0);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-2.5, -1.0),
                        c + egui::vec2(2.5, -1.0),
                        c + egui::vec2(0.0, 2.0),
                    ],
                    super::theme::TEXT_MUTED,
                    Stroke::NONE,
                ));
            }
            response.on_hover_text(tip);
        }
    });
    result
}
/// 粘贴进路径框的文本：去掉首尾空白，以及成对包住整段的双引号
/// （资源管理器「复制为路径」、命令行引用的路径都带引号）。
pub fn clean_pasted_path(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text)
        .trim()
        .to_owned()
}
/// 路径编辑框持焦时，把本帧的 Paste 事件规范化为路径；须在 TextEdit 读取输入之前调用。
pub fn clean_paste_events(ctx: &egui::Context, id: egui::Id) {
    if ctx.memory(|m| m.has_focus(id)) {
        ctx.input_mut(|i| {
            for event in &mut i.events {
                if let egui::Event::Paste(text) = event {
                    *text = clean_pasted_path(text);
                }
            }
        });
    }
}
/// 系统剪贴板文本；空内容与读取失败都视为不可粘贴（对齐参考菜单的禁用态）。
#[cfg(not(test))]
fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .ok()
        .filter(|text| !text.is_empty())
}
/// 测试中完全不碰真实剪贴板：默认无内容，由 test_clipboard::set 注入。
/// （Windows 剪贴板 API 在多线程测试并行时会堆损坏。）
#[cfg(test)]
fn clipboard_text() -> Option<String> {
    test_clipboard::get().unwrap_or(None)
}
/// 测试中替代系统剪贴板，避免并行测试争抢真实剪贴板。
#[cfg(test)]
mod test_clipboard {
    use std::cell::RefCell;
    // 每个测试线程一份，避免并行测试互相覆盖。
    thread_local! {
        static MOCK: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
    }
    pub fn set(text: Option<String>) {
        MOCK.with(|m| *m.borrow_mut() = Some(text));
    }
    pub fn get() -> Option<Option<String>> {
        MOCK.with(|m| m.borrow().clone())
    }
}
/// 字符下标转字节下标（egui 光标按字符计数，越界取末尾）。
fn byte_of(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(byte, _)| byte)
}
/// 右键菜单项（对齐经典系统编辑控件；IME/Unicode 项按需求不实现）。
/// 启用态基于「上一帧末的选区」快照：点击菜单项会让 egui 判定「点了编辑框外面」
/// 而收走焦点，本帧 show() 随即把选区折叠成光标，实时状态不可信。
fn menu_items(
    ctx: &egui::Context,
    id: egui::Id,
    path: &str,
    snapshot: Option<egui::text::CCursorRange>,
) -> Vec<MenuItem> {
    fn item(label: &str, mnemonic: char, enabled: bool) -> MenuItem {
        MenuItem {
            label: label.into(),
            mnemonic: Some(mnemonic),
            shortcut: None,
            kind: MenuItemKind::Action,
            enabled,
            action_id: label.into(),
        }
    }
    fn separator() -> MenuItem {
        MenuItem {
            label: String::new(),
            mnemonic: None,
            shortcut: None,
            kind: MenuItemKind::Separator,
            enabled: false,
            action_id: String::new(),
        }
    }
    let selection = snapshot.is_some_and(|r| !r.is_empty());
    let state = egui::widgets::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let undo = state
        .undoer()
        .has_undo(&(snapshot.unwrap_or_default(), path.to_owned()));
    let paste = clipboard_text().is_some();
    let mut browse = item("Browse (Alt+B)", 'b', true);
    browse.action_id = "browse".into();
    vec![
        item("Undo", 'u', undo),
        separator(),
        item("Cut", 't', selection),
        item("Copy", 'c', selection),
        item("Paste", 'p', paste),
        item("Delete", 'd', selection),
        separator(),
        item("Select All", 'a', !path.is_empty()),
        separator(),
        browse,
    ]
}
/// 编辑框右键菜单：点在上一帧选区内时恢复选区（TextEdit 会把右键当普通点击而
/// 收拢选区）；编辑命令就地执行（TextEdit 每帧的 undoer feed 会自动记录这些改动），
/// 完成后焦点还给编辑框；"browse" 冒泡给调用者。
fn edit_menu(
    ui: &egui::Ui,
    edit: &egui::Response,
    galley: &egui::Galley,
    galley_pos: egui::Pos2,
    path: &mut String,
) -> Option<String> {
    use egui::text::{CCursor, CCursorRange};
    let ctx = ui.ctx().clone();
    let snapshot_id = edit.id.with("pathbar-selection");
    let snapshot: Option<CCursorRange> = ctx.data(|d| d.get_temp(snapshot_id)).unwrap_or(None);
    // 「选区内右键保留选区」：TextEdit 已把光标挪到右键落点，点在快照选区内时恢复。
    if edit.secondary_clicked() {
        if let Some(pos) = edit.interact_pointer_pos() {
            let ccursor =
                galley.cursor_from_pos(pos - galley_pos + egui::vec2(galley.rect.left(), 0.0));
            if let Some(sel) = snapshot.filter(|r| !r.is_empty()) {
                if sel.as_sorted_char_range().contains(&ccursor.index) {
                    let mut state = egui::widgets::text_edit::TextEditState::load(&ctx, edit.id)
                        .unwrap_or_default();
                    state.cursor.set_char_range(Some(sel));
                    state.store(&ctx, edit.id);
                }
            }
        }
    }
    let action = super::popup::context_menu(edit, || menu_items(&ctx, edit.id, path, snapshot));
    if let Some(action) = action {
        let mut state =
            egui::widgets::text_edit::TextEditState::load(&ctx, edit.id).unwrap_or_default();
        let len = path.chars().count();
        let (start, end) = snapshot
            .map(|r| r.as_sorted_char_range())
            .map_or((0, 0), |r| (r.start.0.min(len), r.end.0.min(len)));
        let selection = (start != end).then_some((start, end));
        match action.as_str() {
            "Undo" => {
                let mut current = (snapshot.unwrap_or_default(), path.clone());
                let mut undoer = state.undoer();
                // 撤销点按「光标+文本」判等，光标差异会让 undo() 只回退光标；
                // 系统编辑控件的撤销是文本级的，跳过文本未变的层级。
                let mut applied = None;
                for _ in 0..8 {
                    match undoer.undo(&current).cloned() {
                        Some(prev) if prev.1 != current.1 => {
                            applied = Some(prev);
                            break;
                        }
                        Some(prev) => current = prev,
                        None => break,
                    }
                }
                if let Some((ccursor, text)) = applied {
                    *path = text;
                    state.cursor.set_char_range(Some(ccursor));
                    state.set_undoer(undoer);
                }
            }
            "Cut" | "Copy" => {
                if let Some((a, b)) = selection {
                    ctx.copy_text(path[byte_of(path, a)..byte_of(path, b)].to_owned());
                    if action == "Cut" {
                        path.replace_range(byte_of(path, a)..byte_of(path, b), "");
                        state
                            .cursor
                            .set_char_range(Some(CCursorRange::one(CCursor::new(a))));
                    }
                }
            }
            "Paste" => {
                if let Some(text) = clipboard_text().map(|t| clean_pasted_path(&t)) {
                    path.replace_range(byte_of(path, start)..byte_of(path, end), &text);
                    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(
                        start + text.chars().count(),
                    ))));
                }
            }
            "Delete" => {
                if let Some((a, b)) = selection {
                    path.replace_range(byte_of(path, a)..byte_of(path, b), "");
                    state
                        .cursor
                        .set_char_range(Some(CCursorRange::one(CCursor::new(a))));
                }
            }
            "Select All" => {
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::select_all_str(path)));
            }
            _ => {}
        }
        // Copy/Browse 不改文本：恢复快照选区（本帧 show 已因失焦把它折叠了）。
        if matches!(action.as_str(), "Copy" | "browse") {
            if let Some(sel) = snapshot {
                state.cursor.set_char_range(Some(sel));
            }
        }
        let new_range = state.cursor.char_range();
        state.store(&ctx, edit.id);
        // 焦点被菜单点击收走了，拿回编辑框。
        ctx.memory_mut(|m| m.request_focus(edit.id));
        ctx.data_mut(|d| d.insert_temp(snapshot_id, new_range));
        return (action == "browse").then_some(action);
    }
    // 每帧末快照选区，供下一帧菜单与右键判断使用。
    let current = egui::widgets::text_edit::TextEditState::load(&ctx, edit.id)
        .and_then(|s| s.cursor.char_range());
    ctx.data_mut(|d| d.insert_temp(snapshot_id, current));
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_and_history_render_without_overflow_or_unrequested_changes() {
        let ctx = egui::Context::default();
        let mut path = "left.txt".to_string();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 100.0),
            )),
            ..Default::default()
        };
        let _ = crate::chrome::test_frame(&ctx, input, |ui| {
            let out = show_with_history(ui, &mut path, &["old.txt".into()], false);
            assert_eq!(out, PathbarResponse::default());
            assert!(ui.min_rect().right() <= 501.0);
        });
        assert_eq!(path, "left.txt");
    }
    fn menu_frame(
        ctx: &egui::Context,
        path: &mut String,
        events: Vec<egui::Event>,
        select_all: bool,
    ) -> egui::FullOutput {
        crate::chrome::test_frame(
            ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                show_with_history(ui, path, &[], select_all);
            },
        )
    }
    fn click(pos: egui::Pos2, button: egui::PointerButton) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }
    // 右键点 (30,12) 打开的菜单行中心（顶 5 px，26 px 行高 + 9 px 分隔）。
    // 注意：菜单首帧是 egui Area 的 sizing pass（不可见、不可交互），
    // 需要隔一帧再点击，与 popup::context_menu 自身测试的模式一致。
    const RIGHT: egui::Pos2 = egui::pos2(30.0, 12.0);
    const CUT: egui::Pos2 = egui::pos2(70.0, 65.0);
    const COPY: egui::Pos2 = egui::pos2(70.0, 91.0);
    const PASTE: egui::Pos2 = egui::pos2(70.0, 117.0);
    const DELETE: egui::Pos2 = egui::pos2(70.0, 143.0);
    const SELECT_ALL: egui::Pos2 = egui::pos2(70.0, 178.0);
    const BROWSE: egui::Pos2 = egui::pos2(70.0, 213.0);
    fn open_menu(ctx: &egui::Context, path: &mut String) {
        // 先空一帧：菜单关闭的那帧菜单仍参与命中测试，直接右键会被它吃掉。
        menu_frame(ctx, path, vec![], false);
        menu_frame(ctx, path, click(RIGHT, egui::PointerButton::Secondary), false);
        menu_frame(ctx, path, vec![], false);
    }
    #[test]
    fn right_click_copy_writes_selection_to_clipboard() {
        let ctx = egui::Context::default();
        let mut path = "C:/work/left.txt".to_string();
        // 全选 + 聚焦；右键点在选区内，选区保留。
        menu_frame(&ctx, &mut path, vec![], true);
        open_menu(&ctx, &mut path);
        let out = menu_frame(&ctx, &mut path, click(COPY, egui::PointerButton::Primary), false);
        assert!(out
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "C:/work/left.txt")));
        assert_eq!(path, "C:/work/left.txt");
    }
    #[test]
    fn right_click_cut_paste_delete_and_select_all() {
        let ctx = egui::Context::default();
        let mut path = "left.txt".to_string();
        menu_frame(&ctx, &mut path, vec![], true);
        open_menu(&ctx, &mut path);
        menu_frame(&ctx, &mut path, click(CUT, egui::PointerButton::Primary), false);
        assert_eq!(path, "");
        test_clipboard::set(Some("clip.txt".into()));
        open_menu(&ctx, &mut path);
        menu_frame(&ctx, &mut path, click(PASTE, egui::PointerButton::Primary), false);
        assert_eq!(path, "clip.txt");
        open_menu(&ctx, &mut path);
        menu_frame(
            &ctx,
            &mut path,
            click(SELECT_ALL, egui::PointerButton::Primary),
            false,
        );
        open_menu(&ctx, &mut path);
        menu_frame(&ctx, &mut path, click(DELETE, egui::PointerButton::Primary), false);
        assert_eq!(path, "");
    }
    #[test]
    fn pasted_path_drops_surrounding_quotes_and_whitespace() {
        assert_eq!(clean_pasted_path(r#""C:\a b\x.txt""#), r"C:\a b\x.txt");
        assert_eq!(clean_pasted_path("  \" C:/x.txt \" \r\n"), "C:/x.txt");
        assert_eq!(clean_pasted_path("\tC:/x.txt  "), "C:/x.txt");
        // 不成对或在中间的引号原样保留。
        assert_eq!(clean_pasted_path("\"C:/x.txt"), "\"C:/x.txt");
        assert_eq!(clean_pasted_path("a\"b\"c"), "a\"b\"c");
        assert_eq!(clean_pasted_path("\""), "\"");
    }
    #[test]
    fn keyboard_paste_into_focused_box_is_cleaned() {
        let ctx = egui::Context::default();
        let mut path = "old.txt".to_string();
        menu_frame(&ctx, &mut path, vec![], true); // 聚焦并全选
        menu_frame(
            &ctx,
            &mut path,
            vec![egui::Event::Paste(r#"  "C:\work\new file.txt"  "#.into())],
            false,
        );
        assert_eq!(path, r"C:\work\new file.txt");
    }
    #[test]
    fn menu_paste_is_cleaned() {
        let ctx = egui::Context::default();
        let mut path = "old.txt".to_string();
        menu_frame(&ctx, &mut path, vec![], true);
        open_menu(&ctx, &mut path);
        test_clipboard::set(Some(" \"D:/x y/b.txt\"\n".into()));
        menu_frame(&ctx, &mut path, click(PASTE, egui::PointerButton::Primary), false);
        assert_eq!(path, "D:/x y/b.txt");
    }
    #[test]
    fn right_click_undo_restores_previous_text() {
        let ctx = egui::Context::default();
        let mut path = "a".to_string();
        menu_frame(&ctx, &mut path, vec![], true); // 聚焦并全选 "a"
        menu_frame(&ctx, &mut path, vec![], false); // feed "a" 为首个撤销点
        path.push('b'); // 模拟键入
        menu_frame(&ctx, &mut path, vec![], false); // flux 开始
        // 时间越过 stable_time，"ab" 落定为新撤销点。
        crate::chrome::test_frame(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 300.0),
                )),
                time: Some(2.0),
                ..Default::default()
            },
            |ui| {
                show_with_history(ui, &mut path, &[], false);
            },
        );
        // 右键点在第一字符上（选区内），保留 (0..1) 选区，undo 的当前状态与撤销点一致。
        menu_frame(
            &ctx,
            &mut path,
            click(egui::pos2(7.0, 12.0), egui::PointerButton::Secondary),
            false,
        );
        menu_frame(&ctx, &mut path, vec![], false);
        menu_frame(
            &ctx,
            &mut path,
            click(egui::pos2(40.0, 24.0), egui::PointerButton::Primary),
            false,
        );
        assert_eq!(path, "a");
    }
    #[test]
    fn browse_action_from_menu_and_alt_b() {
        let ctx = egui::Context::default();
        let mut path = "left.txt".to_string();
        menu_frame(&ctx, &mut path, vec![], true);
        open_menu(&ctx, &mut path);
        let mut action = None;
        crate::chrome::test_frame(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 300.0),
                )),
                events: click(BROWSE, egui::PointerButton::Primary),
                ..Default::default()
            },
            |ui| action = show_with_history(ui, &mut path, &[], false).action,
        );
        assert_eq!(action.as_deref(), Some("browse"));
        // Alt+B 与菜单项快捷键标注一致。
        menu_frame(&ctx, &mut path, vec![], true);
        let mut action = None;
        crate::chrome::test_frame(
            &ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 300.0),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::B,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::ALT,
                }],
                ..Default::default()
            },
            |ui| action = show_with_history(ui, &mut path, &[], false).action,
        );
        assert_eq!(action.as_deref(), Some("browse"));
    }
}
