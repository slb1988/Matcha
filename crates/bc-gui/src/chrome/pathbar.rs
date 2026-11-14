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
                        // egui 0.37 起 TextEdit 文本默认 LEFT_TOP；撑高的框内要垂直居中。
                        .align(egui::Align2::LEFT_CENTER)
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
        // 失焦/切换路径也保住水平滚动位置（末尾的文件名），详见函数注释。
        preserve_view_offset(
            ui,
            &edit,
            &output,
            ui.visuals().text_edit_bg_color(),
            super::theme::TEXT,
            output.text_clip_rect.expand(1.0),
        );
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
/// egui 的单行 TextEdit 一失焦就把水平滚动归零（其 `text_offset` 是 crate 私有字段，
/// 无法从外面保住），长路径一失焦/切换就跳回开头，看不见末尾的文件名。这里在帧末
/// 补一层覆盖绘制：持焦帧记录 egui 的实际偏移，失焦帧按记录值抹掉重画文本。
/// 选区与光标不在保留之列（失焦时本就不绘制）；点击重新聚焦的当帧 egui 已把内部
/// 偏移归零，落点映射与显示一起回到开头，之后恢复正常的光标跟随滚动。
/// `clip` 是擦除/剪裁范围：可编辑框有边框内边距可四向外扩；只读框紧贴标题条
/// 分隔线，只能横向外扩且要避开底线。
fn preserve_view_offset(
    ui: &egui::Ui,
    edit: &egui::Response,
    output: &egui::widgets::text_edit::TextEditOutput,
    bg: egui::Color32,
    text_color: egui::Color32,
    clip: egui::Rect,
) {
    let ctx = ui.ctx().clone();
    let inner = output.text_clip_rect;
    let max_offset = (output.galley.size().x - inner.width()).max(0.0);
    // 从 egui 绘制位置反推实时偏移：galley_pos.x = inner.left - offset
    // （egui 自己的落点公式里另有 galley.rect.left() 项，在此差值中抵消）。
    let live = (inner.left() - output.galley_pos.x).max(0.0);
    let offset_id = edit.id.with("view-offset");
    if edit.has_focus() {
        ctx.data_mut(|d| d.insert_temp(offset_id, live.min(max_offset)));
        return;
    }
    let saved: f32 = ctx.data(|d| d.get_temp(offset_id)).unwrap_or(0.0);
    let offset = saved.clamp(0.0, max_offset);
    if offset <= 0.5 || !ui.is_rect_visible(inner) {
        return;
    }
    let painter = ui.painter().with_clip_rect(clip);
    painter.rect_filled(clip, 0.0, bg);
    painter.add(egui::Shape::galley(
        output.galley_pos - egui::vec2(offset + output.galley.rect.left(), 0.0),
        output.galley.clone(),
        text_color,
    ));
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
fn action_item(label: &str, mnemonic: char, enabled: bool) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic: Some(mnemonic),
        shortcut: None,
        kind: MenuItemKind::Action,
        enabled,
        action_id: label.into(),
    }
}
fn separator_item() -> MenuItem {
    MenuItem {
        label: String::new(),
        mnemonic: None,
        shortcut: None,
        kind: MenuItemKind::Separator,
        enabled: false,
        action_id: String::new(),
    }
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
    let selection = snapshot.is_some_and(|r| !r.is_empty());
    let state = egui::widgets::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let undo = state
        .undoer()
        .has_undo(&(snapshot.unwrap_or_default(), path.to_owned()));
    let paste = clipboard_text().is_some();
    let mut browse = action_item("Browse (Alt+B)", 'b', true);
    browse.action_id = "browse".into();
    vec![
        action_item("Undo", 'u', undo),
        separator_item(),
        action_item("Cut", 't', selection),
        action_item("Copy", 'c', selection),
        action_item("Paste", 'p', paste),
        action_item("Delete", 'd', selection),
        separator_item(),
        action_item("Select All", 'a', !path.is_empty()),
        separator_item(),
        browse,
    ]
}
/// 「选区内右键保留选区」：egui 的 secondary_clicked 在右键「释放」帧才触发，
/// 而按下帧 TextEdit 已把选区折叠成落点光标（pointer_interaction 对任意指针键
/// 按下都收拢选区）；若等释放帧才恢复，帧末快照早被折叠后的光标覆盖，选区就找
/// 不回了。因此按下帧（本函数在 TextEdit::show 之后运行）就从事件落点恢复选区，
/// 释放帧的恢复只是幂等的重复。
fn preserve_selection_on_right_click(
    ui: &egui::Ui,
    edit: &egui::Response,
    galley: &egui::Galley,
    galley_pos: egui::Pos2,
    snapshot: Option<egui::text::CCursorRange>,
) {
    let right_pressed_at = ui.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                button: egui::PointerButton::Secondary,
                pressed: true,
                pos,
                ..
            } if edit.rect.contains(*pos) => Some(*pos),
            _ => None,
        })
    });
    let right_click_at = right_pressed_at.or_else(|| {
        edit.secondary_clicked()
            .then(|| edit.interact_pointer_pos())
            .flatten()
    });
    if let Some(pos) = right_click_at {
        let ccursor =
            galley.cursor_from_pos(pos - galley_pos + egui::vec2(galley.rect.left(), 0.0));
        if let Some(sel) = snapshot.filter(|r| !r.is_empty()) {
            if sel.as_sorted_char_range().contains(&ccursor.index) {
                let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit.id)
                    .unwrap_or_default();
                state.cursor.set_char_range(Some(sel));
                state.store(ui.ctx(), edit.id);
            }
        }
    }
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
    preserve_selection_on_right_click(ui, edit, galley, galley_pos, snapshot);
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
/// 只读路径框（合并视图标题条）：文本可选中、Ctrl+C 复制，右键菜单 Copy / Select All。
/// 无边框透明底（标题条自己画背景与边框），交互对齐系统只读编辑控件。
/// `rect` 是标题条里留给路径文本的区域；返回内部编辑框的响应（调用方可查焦点）。
pub fn show_readonly(ui: &mut egui::Ui, rect: egui::Rect, text: &str) -> egui::Response {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let edit_id = ui.make_persistent_id("pathbar-readonly");
        // &str 实现 TextBuffer 且 is_mutable()==false：egui 只读编辑框，可选中/复制。
        let mut buffer = text;
        let output = ui
            .allocate_ui_with_layout(
                rect.size(),
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    egui::TextEdit::singleline(&mut buffer)
                        .id(edit_id)
                        .frame(egui::Frame::NONE)
                        .margin(egui::vec2(0.0, 0.0))
                        .font(egui::FontId::proportional(super::theme::UI_SIZE - 1.0))
                        .text_color(super::theme::TEXT)
                        .align(egui::Align2::LEFT_CENTER)
                        .show(ui)
                },
            )
            .inner;
        let edit = output.response.response.clone();
        // 失焦也保住水平滚动位置；标题条底色擦除，纵向收缩以避开底部分隔线。
        let mut clip = output.text_clip_rect.expand2(egui::vec2(1.0, 0.0));
        clip.max.y -= 1.0;
        preserve_view_offset(
            ui,
            &edit,
            &output,
            super::theme::HEADER_FILL,
            super::theme::TEXT,
            clip,
        );
        readonly_menu(ui, &edit, &output.galley, output.galley_pos, text);
        edit
    })
    .inner
}
/// 只读路径框的右键菜单项：Copy 需有选区，Select All 需非空。
fn readonly_menu_items(selection: bool, empty: bool) -> Vec<MenuItem> {
    vec![
        action_item("Copy", 'c', selection),
        separator_item(),
        action_item("Select All", 'a', !empty),
    ]
}
/// 只读路径框的右键菜单：与 edit_menu 相同的选区快照机制，只是不改文本。
fn readonly_menu(
    ui: &egui::Ui,
    edit: &egui::Response,
    galley: &egui::Galley,
    galley_pos: egui::Pos2,
    text: &str,
) {
    use egui::text::CCursorRange;
    let ctx = ui.ctx().clone();
    let snapshot_id = edit.id.with("pathbar-readonly-selection");
    let snapshot: Option<CCursorRange> = ctx.data(|d| d.get_temp(snapshot_id)).unwrap_or(None);
    preserve_selection_on_right_click(ui, edit, galley, galley_pos, snapshot);
    let action = super::popup::context_menu(edit, || {
        readonly_menu_items(snapshot.is_some_and(|r| !r.is_empty()), text.is_empty())
    });
    if let Some(action) = action {
        let mut state =
            egui::widgets::text_edit::TextEditState::load(&ctx, edit.id).unwrap_or_default();
        let len = text.chars().count();
        let (start, end) = snapshot
            .map(|r| r.as_sorted_char_range())
            .map_or((0, 0), |r| (r.start.0.min(len), r.end.0.min(len)));
        match action.as_str() {
            "Copy" => {
                if start != end {
                    ctx.copy_text(text[byte_of(text, start)..byte_of(text, end)].to_owned());
                }
                // Copy 不改文本：恢复快照选区（本帧 show 已因失焦把它折叠了）。
                if let Some(sel) = snapshot {
                    state.cursor.set_char_range(Some(sel));
                }
            }
            "Select All" => {
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::select_all_str(text)));
            }
            _ => {}
        }
        let new_range = state.cursor.char_range();
        state.store(&ctx, edit.id);
        // 焦点被菜单点击收走了，拿回编辑框。
        ctx.memory_mut(|m| m.request_focus(edit.id));
        ctx.data_mut(|d| d.insert_temp(snapshot_id, new_range));
        return;
    }
    // 每帧末快照选区，供下一帧菜单与右键判断使用。
    let current = egui::widgets::text_edit::TextEditState::load(&ctx, edit.id)
        .and_then(|s| s.cursor.char_range());
    ctx.data_mut(|d| d.insert_temp(snapshot_id, current));
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
    fn right_press_and_release_in_separate_frames_keeps_selection() {
        // 真实输入中右键按下与释放落在不同帧：按下帧 TextEdit 会把选区折叠成落点
        // 光标，若选区快照在帧末被折叠后的光标覆盖，释放帧打开的菜单里 Cut/Copy
        // 会全灰（选区看似被右键清掉）。修复后按下帧即恢复快照选区。
        let ctx = egui::Context::default();
        let mut path = "C:/work/left.txt".to_string();
        menu_frame(&ctx, &mut path, vec![], true); // 聚焦并全选
        menu_frame(&ctx, &mut path, vec![], false); // 帧末快照写入选区
        let press = |pressed| egui::Event::PointerButton {
            pos: RIGHT,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        };
        menu_frame(&ctx, &mut path, vec![egui::Event::PointerMoved(RIGHT)], false);
        menu_frame(&ctx, &mut path, vec![press(true)], false); // 按下帧：折叠后应立即恢复
        menu_frame(&ctx, &mut path, vec![press(false)], false); // 释放帧：菜单打开
        menu_frame(&ctx, &mut path, vec![], false); // Area sizing pass
        let out = menu_frame(&ctx, &mut path, click(COPY, egui::PointerButton::Primary), false);
        assert!(out
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "C:/work/left.txt")));
        assert_eq!(path, "C:/work/left.txt");
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
    /// 从帧输出里找指定文本的最后一个 TextShape（覆盖绘制在后，是最上层的显示）。
    fn painted_text_x(out: &egui::FullOutput, text: &str) -> Option<f32> {
        out.shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos.x),
                _ => None,
            })
            .next_back()
    }
    const LONG_PATH: &str = "C:/very/long/path/that/keeps/going/through/many/folders/and/subfolders/until/the/final/file_name.txt";
    #[test]
    fn view_offset_survives_focus_loss_and_path_switch() {
        let ctx = egui::Context::default();
        let mut path = LONG_PATH.to_string();
        // 首帧偏移为 0（egui 原生行为：文本从开头显示）。
        let start = menu_frame(&ctx, &mut path, vec![], true);
        let start_x = painted_text_x(&start, LONG_PATH).unwrap();
        // 全选后主光标在末尾，egui 滚动到末尾，偏移被记下。
        let focused = menu_frame(&ctx, &mut path, vec![], false);
        let focused_x = painted_text_x(&focused, LONG_PATH).unwrap();
        assert!(focused_x < start_x - 40.0, "长路径应滚出开头: {focused_x} vs {start_x}");
        // 点击空白处失焦，再等一帧：显示不能跳回开头。
        menu_frame(
            &ctx,
            &mut path,
            click(egui::pos2(250.0, 200.0), egui::PointerButton::Primary),
            false,
        );
        let out = menu_frame(&ctx, &mut path, vec![], false);
        let unfocused_x = painted_text_x(&out, LONG_PATH).unwrap();
        assert!(
            (unfocused_x - focused_x).abs() < 2.0,
            "失焦后应保持滚动位置: {unfocused_x} vs {focused_x}（开头在 {start_x}）"
        );
        // 切换路径（同样足够长）：可视偏移保留，而不是回开头。
        path = "D:/another/equally/long/path/going/through/quite/a/few/folders/down/into/the/depths/other_target.txt".into();
        let out = menu_frame(&ctx, &mut path, vec![], false);
        let switched_x = painted_text_x(&out, &path).unwrap();
        assert!(
            (switched_x - unfocused_x).abs() < 2.0,
            "切换路径后应保持可视位置: {switched_x} vs {unfocused_x}"
        );
        // 短路径：偏移被钳到 0，从头显示。
        path = "x.txt".into();
        let out = menu_frame(&ctx, &mut path, vec![], false);
        let short_x = painted_text_x(&out, "x.txt").unwrap();
        assert!(
            (short_x - start_x).abs() < 1.0,
            "短路径应从头显示: {short_x} vs {start_x}"
        );
    }
    #[test]
    fn readonly_view_offset_survives_focus_loss() {
        let ctx = egui::Context::default();
        let text = LONG_PATH;
        let start = readonly_frame(&ctx, text, vec![]);
        let start_x = painted_text_x(&start, text).unwrap();
        readonly_select_all(&ctx, text);
        let focused = readonly_frame(&ctx, text, vec![]);
        let focused_x = painted_text_x(&focused, text).unwrap();
        assert!(focused_x < start_x - 40.0);
        readonly_frame(
            &ctx,
            text,
            click(egui::pos2(250.0, 200.0), egui::PointerButton::Primary),
        );
        let out = readonly_frame(&ctx, text, vec![]);
        let unfocused_x = painted_text_x(&out, text).unwrap();
        assert!(
            (unfocused_x - focused_x).abs() < 2.0,
            "只读框失焦后应保持滚动位置: {unfocused_x} vs {focused_x}（开头在 {start_x}）"
        );
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
    // ---- 只读路径框（合并视图标题条）----
    fn readonly_frame(
        ctx: &egui::Context,
        text: &str,
        events: Vec<egui::Event>,
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
                show_readonly(
                    ui,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 24.0)),
                    text,
                );
            },
        )
    }
    // 只读菜单（Copy / Select All）的行中心：右键点 (30,12)，顶 5 px，26 px 行高 + 9 px 分隔。
    const RO_COPY: egui::Pos2 = egui::pos2(70.0, 30.0);
    const RO_SELECT_ALL: egui::Pos2 = egui::pos2(70.0, 65.0);
    fn readonly_select_all(ctx: &egui::Context, text: &str) {
        // 先空一帧：egui 命中测试用上一帧注册的 widget，首帧的点击不生效。
        readonly_frame(ctx, text, vec![]);
        // 点击聚焦后 Ctrl+A 全选。
        readonly_frame(
            ctx,
            text,
            click(egui::pos2(30.0, 12.0), egui::PointerButton::Primary),
        );
        readonly_frame(
            ctx,
            text,
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
    }
    fn open_readonly_menu(ctx: &egui::Context, text: &str) {
        readonly_frame(ctx, text, vec![]);
        readonly_frame(ctx, text, click(RIGHT, egui::PointerButton::Secondary));
        readonly_frame(ctx, text, vec![]);
    }
    fn copied(out: &egui::FullOutput) -> Option<String> {
        out.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
    }
    #[test]
    fn readonly_right_click_copy_writes_selection_to_clipboard() {
        let ctx = egui::Context::default();
        let text = "C:/work/left.txt";
        readonly_select_all(&ctx, text);
        readonly_frame(&ctx, text, vec![]); // 帧末快照写入选区
        open_readonly_menu(&ctx, text);
        let out = readonly_frame(&ctx, text, click(RO_COPY, egui::PointerButton::Primary));
        assert_eq!(copied(&out).as_deref(), Some(text));
    }
    #[test]
    fn readonly_copy_disabled_without_selection() {
        let ctx = egui::Context::default();
        let text = "C:/work/left.txt";
        open_readonly_menu(&ctx, text);
        let out = readonly_frame(&ctx, text, click(RO_COPY, egui::PointerButton::Primary));
        assert_eq!(copied(&out), None);
    }
    #[test]
    fn readonly_select_all_then_copy() {
        let ctx = egui::Context::default();
        let text = "C:/work/left.txt";
        open_readonly_menu(&ctx, text);
        readonly_frame(&ctx, text, click(RO_SELECT_ALL, egui::PointerButton::Primary));
        // 全选后再右键（点在刚选中的文本上）：选区保留，Copy 可用。
        open_readonly_menu(&ctx, text);
        let out = readonly_frame(&ctx, text, click(RO_COPY, egui::PointerButton::Primary));
        assert_eq!(copied(&out).as_deref(), Some(text));
    }
    #[test]
    fn readonly_never_mutates_on_paste_or_typing() {
        let ctx = egui::Context::default();
        let text = "C:/work/left.txt";
        readonly_select_all(&ctx, text);
        // &str 缓冲不可变：文本事件与粘贴都不生效，也不 panic。
        readonly_frame(
            &ctx,
            text,
            vec![
                egui::Event::Text("typed".into()),
                egui::Event::Paste("pasted".into()),
            ],
        );
        readonly_select_all(&ctx, text);
        readonly_frame(&ctx, text, vec![]);
        open_readonly_menu(&ctx, text);
        let out = readonly_frame(&ctx, text, click(RO_COPY, egui::PointerButton::Primary));
        assert_eq!(copied(&out).as_deref(), Some(text));
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
