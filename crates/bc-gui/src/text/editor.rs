//! UTF-8 字节光标与可撤销的局部编辑；从不把整份文件交给 TextEdit。
use bc_core::{
    line::{LineEdit, LineEnding, LineList},
    CoreError,
};
use bc_engine_text::TextContent;
use eframe::egui;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Caret {
    pub line: usize,
    pub byte: usize,
}
#[derive(Debug, Clone, Default)]
pub struct Editor {
    pub caret: Caret,
    pub anchor: Option<Caret>,
    pub preedit: String,
}
impl Editor {
    pub fn selection(&self) -> Option<(Caret, Caret)> {
        self.anchor
            .filter(|a| *a != self.caret)
            .map(|a| (a.min(self.caret), a.max(self.caret)))
    }
    pub fn clamp(&mut self, content: &TextContent) {
        let lines = content.lines.borrow();
        self.caret = clamp(&lines, self.caret);
        self.anchor = self.anchor.map(|p| clamp(&lines, p));
    }
    pub fn place(&mut self, caret: Caret, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = caret;
    }
    pub fn select_all(&mut self, content: &TextContent) {
        self.anchor = Some(Caret::default());
        self.caret = end(&content.lines.borrow());
    }
    pub fn selected_text(&self, content: &TextContent) -> String {
        let Some((start, end)) = self.selection() else {
            return String::new();
        };
        let lines = content.lines.borrow();
        let mut out = Vec::new();
        for i in start.line..=end.line.min(lines.len().saturating_sub(1)) {
            let Some(bytes) = lines.line_bytes(i) else {
                continue;
            };
            let a = if i == start.line {
                start.byte.min(bytes.len())
            } else {
                0
            };
            let b = if i == end.line {
                end.byte.min(bytes.len())
            } else {
                bytes.len()
            };
            out.extend_from_slice(&bytes[a..b]);
            if i < end.line {
                out.extend_from_slice(lines.ending(i).unwrap().as_bytes());
            }
        }
        String::from_utf8(out).expect("decoded UTF-8")
    }
    /// 一次输入、粘贴或选区替换对应一个 EditItem，只复制被改变的行。
    pub fn insert(&mut self, content: &mut TextContent, text: &str) -> Result<(), CoreError> {
        content.ensure_writable()?;
        self.clamp(content);
        let (start, finish) = self.selection().unwrap_or((self.caret, self.caret));
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines = content.lines.borrow();
        let prefix = lines
            .line_bytes(start.line)
            .map_or(&[][..], |b| &b[..start.byte]);
        let suffix = lines
            .line_bytes(finish.line)
            .map_or(&[][..], |b| &b[finish.byte..]);
        let ending = lines.ending(finish.line).unwrap_or(LineEnding::None);
        let dominant = lines.dominant_ending();
        let mut bytes = prefix.to_vec();
        for (i, part) in normalized.split('\n').enumerate() {
            if i > 0 {
                bytes.extend_from_slice(dominant.as_bytes());
            }
            bytes.extend_from_slice(part.as_bytes());
        }
        bytes.extend_from_slice(suffix);
        bytes.extend_from_slice(ending.as_bytes());
        let range = start.line..(finish.line + 1).min(lines.len());
        let count = normalized.bytes().filter(|b| *b == b'\n').count();
        let byte = if count == 0 {
            start.byte + normalized.len()
        } else {
            normalized.rsplit('\n').next().unwrap().len()
        };
        drop(lines);
        let replacement = LineList::from_bytes(bytes);
        content.edits.push(Box::new(LineEdit::replace_range(
            &content.lines,
            range,
            &replacement,
        )));
        self.caret = Caret {
            line: start.line + count,
            byte,
        };
        self.anchor = None;
        self.preedit.clear();
        self.clamp(content);
        Ok(())
    }
    pub fn delete(
        &mut self,
        content: &mut TextContent,
        backwards: bool,
        word: bool,
    ) -> Result<(), CoreError> {
        content.ensure_writable()?;
        self.clamp(content);
        if self.selection().is_none() {
            let old = self.caret;
            self.move_horizontal(content, !backwards, word, true);
            self.anchor = Some(old);
        }
        if self.selection().is_some() {
            self.insert(content, "")?;
        }
        Ok(())
    }
    pub fn move_horizontal(
        &mut self,
        content: &TextContent,
        forward: bool,
        word: bool,
        extend: bool,
    ) {
        self.clamp(content);
        if !extend {
            if let Some((a, b)) = self.selection() {
                self.place(if forward { b } else { a }, false);
                return;
            }
        }
        let lines = content.lines.borrow();
        let mut p = self.caret;
        let text = std::str::from_utf8(lines.line_bytes(p.line).unwrap_or_default()).unwrap();
        if forward {
            if p.byte < text.len() {
                let mut iter = text[p.byte..].char_indices();
                let (_, first) = iter.next().unwrap();
                p.byte += first.len_utf8();
                if word {
                    for (_, c) in iter {
                        if word_char(c) != word_char(first) {
                            break;
                        }
                        p.byte += c.len_utf8();
                    }
                }
            } else if p < end(&lines) {
                p = Caret {
                    line: p.line + 1,
                    byte: 0,
                };
            }
        } else if p.byte > 0 {
            let mut iter = text[..p.byte].chars().rev();
            let first = iter.next().unwrap();
            p.byte -= first.len_utf8();
            if word {
                for c in iter {
                    if word_char(c) != word_char(first) {
                        break;
                    }
                    p.byte -= c.len_utf8();
                }
            }
        } else if p.line > 0 {
            p.line -= 1;
            p.byte = lines.line_bytes(p.line).unwrap().len();
        }
        drop(lines);
        self.place(p, extend);
    }
    pub fn move_vertical(&mut self, content: &TextContent, delta: isize, extend: bool) {
        self.clamp(content);
        let lines = content.lines.borrow();
        let current =
            std::str::from_utf8(lines.line_bytes(self.caret.line).unwrap_or_default()).unwrap();
        let column = current[..self.caret.byte].chars().count();
        let line = self
            .caret
            .line
            .saturating_add_signed(delta)
            .min(end(&lines).line);
        let text = std::str::from_utf8(lines.line_bytes(line).unwrap_or_default()).unwrap();
        let byte = byte_at_char(text, column);
        drop(lines);
        self.place(Caret { line, byte }, extend);
    }
    /// 返回是否改变文本；egui 的复制/粘贴事件直接走系统剪贴板桥。
    pub fn event(
        &mut self,
        content: &mut TextContent,
        event: &egui::Event,
        ctx: &egui::Context,
    ) -> Result<bool, CoreError> {
        self.clamp(content);
        let revision = content.revision();
        match event {
            egui::Event::Copy => ctx.copy_text(self.selected_text(content)),
            egui::Event::Cut => {
                content.ensure_writable()?;
                ctx.copy_text(self.selected_text(content));
                if self.selection().is_some() {
                    self.insert(content, "")?;
                }
            }
            egui::Event::Paste(text) => self.insert(content, text)?,
            egui::Event::Text(text) if self.preedit.is_empty() => self.insert(content, text)?,
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                content.ensure_writable()?;
                self.preedit.clone_from(text);
            }
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                self.preedit.clear();
                if !text.is_empty() {
                    self.insert(content, text)?;
                }
            }
            egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            }) => {
                content.ensure_writable()?;
                for _ in 0..*before_chars {
                    self.move_horizontal(content, false, false, false);
                }
                self.anchor = Some(self.caret);
                for _ in 0..before_chars + after_chars {
                    self.move_horizontal(content, true, false, true);
                }
                self.insert(content, "")?;
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if self.preedit.is_empty() => {
                let command = modifiers.command;
                match key {
                    egui::Key::A if command => self.select_all(content),
                    egui::Key::Z if command => {
                        content.ensure_writable()?;
                        if modifiers.shift {
                            content.edits.redo();
                        } else {
                            content.edits.undo();
                        }
                        self.anchor = None;
                    }
                    egui::Key::Y if command => {
                        content.ensure_writable()?;
                        content.edits.redo();
                        self.anchor = None;
                    }
                    egui::Key::ArrowLeft => {
                        self.move_horizontal(content, false, command, modifiers.shift)
                    }
                    egui::Key::ArrowRight => {
                        self.move_horizontal(content, true, command, modifiers.shift)
                    }
                    egui::Key::ArrowUp => self.move_vertical(content, -1, modifiers.shift),
                    egui::Key::ArrowDown => self.move_vertical(content, 1, modifiers.shift),
                    egui::Key::PageUp => self.move_vertical(content, -30, modifiers.shift),
                    egui::Key::PageDown => self.move_vertical(content, 30, modifiers.shift),
                    egui::Key::Home => self.place(
                        Caret {
                            line: if command { 0 } else { self.caret.line },
                            byte: 0,
                        },
                        modifiers.shift,
                    ),
                    egui::Key::End => {
                        let p = if command {
                            end(&content.lines.borrow())
                        } else {
                            Caret {
                                line: self.caret.line,
                                byte: content
                                    .lines
                                    .borrow()
                                    .line_bytes(self.caret.line)
                                    .unwrap_or_default()
                                    .len(),
                            }
                        };
                        self.place(p, modifiers.shift);
                    }
                    egui::Key::Backspace => self.delete(content, true, command)?,
                    egui::Key::Delete => self.delete(content, false, command)?,
                    egui::Key::Enter => self.insert(content, "\n")?,
                    egui::Key::Tab => {
                        let tab = content
                            .format()
                            .and_then(|f| f.tab_size)
                            .unwrap_or(4)
                            .clamp(1, 32) as usize;
                        let insert = if content.format().is_some_and(|f| f.tab_as_spaces) {
                            let lines = content.lines.borrow();
                            let text = std::str::from_utf8(
                                lines.line_bytes(self.caret.line).unwrap_or_default(),
                            )
                            .unwrap();
                            let column = tab_column(&text[..self.caret.byte], tab);
                            " ".repeat(tab - column % tab)
                        } else {
                            "\t".into()
                        };
                        self.insert(content, &insert)?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        self.clamp(content);
        Ok(content.revision() != revision)
    }
}
pub(crate) fn tab_column(text: &str, tab: usize) -> usize {
    text.chars().fold(0, |col, c| {
        if c == '\t' {
            col + tab - col % tab
        } else {
            col + 1
        }
    })
}
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
pub fn byte_at_char(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(b, _)| b)
}
pub fn end(lines: &LineList) -> Caret {
    if lines.is_empty() || lines.ending(lines.len() - 1) != Some(LineEnding::None) {
        Caret {
            line: lines.len(),
            byte: 0,
        }
    } else {
        Caret {
            line: lines.len() - 1,
            byte: lines.line_bytes(lines.len() - 1).unwrap().len(),
        }
    }
}
fn clamp(lines: &LineList, p: Caret) -> Caret {
    let end = end(lines);
    let line = p.line.min(end.line);
    let text = std::str::from_utf8(lines.line_bytes(line).unwrap_or_default()).unwrap();
    let mut byte = p.byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    Caret { line, byte }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(text: &str) -> TextContent {
        TextContent::from_bytes(text.as_bytes().to_vec(), None).unwrap()
    }
    #[test]
    fn multiline_edit_preserves_endings_and_undo() {
        let mut c = content("one\r\ntwo\nthree");
        let mut e = Editor {
            caret: Caret { line: 1, byte: 2 },
            anchor: Some(Caret { line: 0, byte: 1 }),
            ..Default::default()
        };
        assert_eq!(e.selected_text(&c), "ne\r\ntw");
        e.insert(&mut c, "你\n好").unwrap();
        assert_eq!(c.to_bytes().unwrap(), "o你\r\n好o\nthree".as_bytes());
        c.edits.undo();
        assert_eq!(c.to_bytes().unwrap(), b"one\r\ntwo\nthree");
        c.edits.redo();
        assert_eq!(c.to_bytes().unwrap(), "o你\r\n好o\nthree".as_bytes());
    }
    #[test]
    fn empty_eof_and_unicode_backspace() {
        let mut c = content("");
        let mut e = Editor::default();
        e.insert(&mut c, "你\n").unwrap();
        assert_eq!(e.caret, Caret { line: 1, byte: 0 });
        e.insert(&mut c, "好").unwrap();
        e.delete(&mut c, true, false).unwrap();
        e.delete(&mut c, true, false).unwrap();
        assert_eq!(c.to_bytes().unwrap(), "你".as_bytes());
        e.delete(&mut c, true, false).unwrap();
        assert!(c.to_bytes().unwrap().is_empty());
    }
    #[test]
    fn ime_preedit_is_not_an_edit_commit_is_one_undo_step_and_readonly_rejects() {
        let mut c = content("x");
        let mut e = Editor::default();
        let ctx = egui::Context::default();
        e.event(
            &mut c,
            &egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "中".into(),
                active_range_chars: None,
            }),
            &ctx,
        )
        .unwrap();
        assert!(!c.is_dirty());
        e.event(
            &mut c,
            &egui::Event::Ime(egui::ImeEvent::Commit("中文".into())),
            &ctx,
        )
        .unwrap();
        assert_eq!(c.to_bytes().unwrap(), "中文x".as_bytes());
        c.edits.undo();
        assert_eq!(c.to_bytes().unwrap(), b"x");
        c.read_only = true;
        assert!(e.insert(&mut c, "bad").is_err());
        assert!(e
            .event(
                &mut c,
                &egui::Event::Ime(egui::ImeEvent::Commit("坏".into())),
                &ctx
            )
            .is_err());
    }
}
