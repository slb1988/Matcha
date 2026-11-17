//! 语法元素的显示风格：基础风格 + 按语言（文件格式名）覆盖。
//!
//! 着色只认 [`ElementKind`]（参考的十类语法元素），语言差异通过 [`LANGUAGE_OVERRIDES`]
//! 表达：新增/调整某种语言的风格只需在表里加一行，不改渲染代码。将来接入用户自定义配色
//! （Options > Colors）时，在 [`SyntaxStyle::for_format`] 之后再叠一层即可。
use bc_formats::{ElementKind, FileFormat};
use eframe::egui::Color32;

/// 一种语法元素的显示风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementStyle {
    /// 文字颜色；`None` 为正文色。
    pub color: Option<Color32>,
    pub bold: bool,
    pub italic: bool,
    /// 在单侧差异行（整行用差异色着墨）上仍保留本风格。注释类元素保留，
    /// 一眼可辨"这是注释"；代码元素让位给差异色。
    pub keep_on_diff: bool,
}

impl ElementStyle {
    /// 正文：无颜色、无粗斜体。
    pub const PLAIN: Self = Self {
        color: None,
        bold: false,
        italic: false,
        keep_on_diff: false,
    };
    pub const fn color(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: Some(Color32::from_rgb(r, g, b)),
            ..Self::PLAIN
        }
    }
    pub const fn bold(self) -> Self {
        Self { bold: true, ..self }
    }
    pub const fn italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }
    pub const fn keep_on_diff(self) -> Self {
        Self {
            keep_on_diff: true,
            ..self
        }
    }
}

/// 全部十类元素的风格表（按 [`ElementKind::ALL`] 顺序存放）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxStyle {
    styles: [ElementStyle; ElementKind::ALL.len()],
}

/// 一种语言对基础风格的覆盖：`(格式名, [(元素, 风格)])`。格式名即
/// [`FileFormat::name`]（内置库：CPP、JAVA、HTML、PYTHON …），大小写不敏感。
pub type LanguageOverride = (&'static str, &'static [(ElementKind, ElementStyle)]);

/// 按语言的风格覆盖。新增语言在此加一行即可。
pub const LANGUAGE_OVERRIDES: &[LanguageOverride] = &[
    // HTML 的 Keyword 是标签名：整页粗体太重，用深蓝常规体。
    (
        "HTML",
        &[(ElementKind::Keyword, ElementStyle::color(0, 0, 128))],
    ),
];

impl SyntaxStyle {
    /// 基础风格（对齐参考默认配色，取自参考截图）：注释灰、预处理指令栗色、
    /// 字符串深绿、关键字黑色粗体、环境变量紫色，其余为正文。
    pub const fn base() -> Self {
        let mut styles = [ElementStyle::PLAIN; ElementKind::ALL.len()];
        styles[index(ElementKind::Comment)] = ElementStyle::color(128, 128, 128).keep_on_diff();
        styles[index(ElementKind::Directive)] = ElementStyle::color(128, 0, 0);
        styles[index(ElementKind::String)] = ElementStyle::color(0, 128, 64);
        styles[index(ElementKind::Keyword)] = ElementStyle::PLAIN.bold();
        styles[index(ElementKind::EnvironmentVar)] = ElementStyle::color(128, 0, 128);
        Self { styles }
    }

    /// 某文件格式（语言）的风格：基础风格叠加 [`LANGUAGE_OVERRIDES`] 中该语言的覆盖。
    /// 无格式（纯文本）用基础风格（纯文本没有语法元素，实际全是正文）。
    pub fn for_format(format: Option<&FileFormat>) -> Self {
        let mut style = Self::base();
        if let Some(format) = format {
            for (name, overrides) in LANGUAGE_OVERRIDES {
                if name.eq_ignore_ascii_case(&format.name) {
                    for (kind, element) in *overrides {
                        style = style.with(*kind, *element);
                    }
                }
            }
        }
        style
    }

    pub fn get(&self, kind: ElementKind) -> ElementStyle {
        self.styles[index(kind)]
    }

    pub fn with(mut self, kind: ElementKind, style: ElementStyle) -> Self {
        self.styles[index(kind)] = style;
        self
    }
}

impl Default for SyntaxStyle {
    fn default() -> Self {
        Self::base()
    }
}

const fn index(kind: ElementKind) -> usize {
    match kind {
        ElementKind::Comment => 0,
        ElementKind::Directive => 1,
        ElementKind::EnvironmentVar => 2,
        ElementKind::Identifier => 3,
        ElementKind::Indent => 4,
        ElementKind::Keyword => 5,
        ElementKind::LineNum => 6,
        ElementKind::Number => 7,
        ElementKind::Operator => 8,
        ElementKind::String => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_display_order() {
        for (n, kind) in ElementKind::ALL.into_iter().enumerate() {
            assert_eq!(index(kind), n);
        }
    }

    #[test]
    fn base_matches_reference_defaults() {
        let s = SyntaxStyle::base();
        let comment = s.get(ElementKind::Comment);
        assert_eq!(comment.color, Some(Color32::from_rgb(128, 128, 128)));
        assert!(
            comment.keep_on_diff,
            "comments stay recognisable on diff rows"
        );
        assert!(s.get(ElementKind::Keyword).bold);
        assert!(!s.get(ElementKind::Keyword).keep_on_diff);
        assert_eq!(s.get(ElementKind::Identifier), ElementStyle::PLAIN);
    }

    #[test]
    fn language_overrides_apply_by_format_name() {
        let mut html = bc_formats::builtin_formats()
            .iter()
            .find(|f| f.name == "HTML" && f.kind == bc_formats::FormatKind::Text)
            .unwrap()
            .clone();
        let keyword = SyntaxStyle::for_format(Some(&html)).get(ElementKind::Keyword);
        assert_eq!(keyword.color, Some(Color32::from_rgb(0, 0, 128)));
        assert!(!keyword.bold);
        // 大小写不敏感；其它语言不受影响。
        html.name = "html".into();
        assert_eq!(
            SyntaxStyle::for_format(Some(&html)).get(ElementKind::Keyword),
            keyword
        );
        let cpp = bc_formats::builtin_formats()
            .iter()
            .find(|f| f.name == "CPP")
            .unwrap();
        assert_eq!(SyntaxStyle::for_format(Some(cpp)), SyntaxStyle::base());
        assert_eq!(SyntaxStyle::for_format(None), SyntaxStyle::base());
    }
}
