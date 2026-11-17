//! 文件对该用哪种比较打开：唯一的判定入口（GUI 命令行/Home/拖放/文件夹比较打开、
//! 脚本 `/report` 与合并分派都调这里，不各自判扩展名）。
//!
//! 顺序固定为 **表格 → 十六进制 → 文本**：
//!
//! 1. 表格：原生可读的工作簿（`*.xlsx`/`*.xlsm`，或扩展名对不上但内容是 xlsx 包），
//!    或格式库里无外部转换的表格格式（内置 `CSV` 掩码 `*.csv`、用户自定义表格格式），
//!    格式库没匹配时 `*.tsv`/`*.tab` 也算。工作簿是 ZIP 包，必须先于二进制嗅探判定，
//!    否则会落进十六进制比较。
//! 2. 十六进制：`*.bin` 掩码，或前 8 KiB 内容嗅探为二进制（与文本引擎加载同一规则）。
//! 3. 其余为文本。
//!
//! 需要外部转换器的表格格式（`*.xls` 等）当前读不了，不算表格，按内容继续往下判。
use crate::{registry::FormatRegistry, FormatKind};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// 文件（对）的比较类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileView {
    Text,
    Table,
    Hex,
}

fn extension_is(path: &Path, names: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| names.iter().any(|n| e.eq_ignore_ascii_case(n)))
}

/// 原生读取的工作簿：扩展名 `xlsx`/`xlsm`；扩展名对不上时（版本控制工具的临时文件名常带
/// `#版本`、`.r123` 之类后缀）看内容是否为含 `xl/workbook.xml` 的 ZIP 包。
/// 不存在或读不了的文件只看扩展名。
pub fn is_workbook(path: &Path) -> bool {
    extension_is(path, &["xlsx", "xlsm"]) || sniffs_workbook(path)
}

/// ZIP 本地头魔数 + 尾部中央目录（最多 64 KiB）里有 `xl/workbook.xml` 条目名。
/// docx/odt/jar 等其他 ZIP 没有这个条目。
fn sniffs_workbook(path: &Path) -> bool {
    const TAIL: u64 = 64 * 1024;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || magic != *b"PK\x03\x04" {
        return false;
    }
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    let mut tail = Vec::new();
    if file
        .seek(SeekFrom::Start(len.saturating_sub(TAIL)))
        .is_err()
        || file.take(TAIL).read_to_end(&mut tail).is_err()
    {
        return false;
    }
    const NAME: &[u8] = b"xl/workbook.xml";
    tail.windows(NAME.len()).any(|w| w == NAME)
}

/// 表格比较能直接读取的文件（见模块说明第 1 条）。
pub fn is_table(path: &Path, registry: &FormatRegistry) -> bool {
    if is_workbook(path) {
        return true;
    }
    match registry.resolve(path, None) {
        Some(format) => format.kind == FormatKind::Table && format.convert.is_none(),
        None => extension_is(path, &["csv", "tsv", "tab"]),
    }
}

/// 十六进制路由：`*.bin` 掩码，或内容嗅探为二进制。读不了的文件不算二进制——目标会话会
/// 自己报 IO 错误。
pub fn is_binary(path: &Path) -> bool {
    extension_is(path, &["bin"]) || sniffs_binary(path)
}

/// 读前 8 KiB 经 `encoding::detect` 判二进制。
fn sniffs_binary(path: &Path) -> bool {
    let mut prefix = Vec::new();
    if std::fs::File::open(path)
        .and_then(|f| f.take(8192).read_to_end(&mut prefix))
        .is_err()
    {
        return false;
    }
    bc_core::encoding::detect(&prefix, 1252).encoding == bc_core::encoding::Encoding::Binary
}

/// 单个文件的比较类型。
pub fn file_view(path: &Path, registry: &FormatRegistry) -> FileView {
    pair_view(path, path, registry)
}

/// 文件对的比较类型：任一侧是表格 → 表格；否则任一侧是二进制 → 十六进制；否则文本。
pub fn pair_view(left: &Path, right: &Path, registry: &FormatRegistry) -> FileView {
    let sides = [left, right];
    if sides.iter().any(|p| is_table(p, registry)) {
        FileView::Table
    } else if sides.iter().any(|p| is_binary(p)) {
        FileView::Hex
    } else {
        FileView::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("matcha-route-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 最小的“xlsx 包”：ZIP 魔数开头，尾部中央目录带 `xl/workbook.xml` 名字。
    fn fake_xlsx() -> Vec<u8> {
        let mut bytes = b"PK\x03\x04\x14\x00\x00\x00\x00\x00".to_vec();
        bytes.extend([0u8; 64]);
        bytes.extend(b"PK\x01\x02");
        bytes.extend(b"[Content_Types].xml");
        bytes.extend(b"PK\x01\x02");
        bytes.extend(b"xl/workbook.xml");
        bytes.extend(b"PK\x05\x06");
        bytes
    }

    #[test]
    fn workbooks_route_to_table_before_the_binary_sniff() {
        let dir = Dir::new("xlsx");
        let registry = FormatRegistry::with_builtins();
        let book = fake_xlsx();
        assert!(
            is_binary(&dir.file("probe.zip", &book)),
            "xlsx bytes sniff as binary"
        );
        for name in ["a.xlsx", "b.XLSM", "c.xlsx#r12", "d.tmp"] {
            let path = dir.file(name, &book);
            assert_eq!(file_view(&path, &registry), FileView::Table, "{name}");
        }
        // 其他 ZIP（docx 等）不是工作簿。
        let docx = dir.file(
            "e.docx",
            b"PK\x03\x04\x00\x00\x00\x00PK\x01\x02word/document.xml",
        );
        assert!(!is_workbook(&docx));
        assert_eq!(file_view(&docx, &registry), FileView::Hex);
        // 不存在的文件只看扩展名。
        assert!(is_workbook(Path::new("missing.xlsx")));
        assert!(!is_workbook(Path::new("missing.zip")));
    }

    #[test]
    fn text_tables_route_to_table_and_the_rest_by_content() {
        let dir = Dir::new("mix");
        let registry = FormatRegistry::with_builtins();
        for name in ["a.csv", "b.CSV", "c.tsv", "d.tab"] {
            let path = dir.file(name, b"a,b\n1,2\n");
            assert_eq!(file_view(&path, &registry), FileView::Table, "{name}");
        }
        // 表格任一侧即表格。
        let csv = dir.file("x.csv", b"a\n");
        let txt = dir.file("y.txt", b"a\n");
        assert_eq!(pair_view(&csv, &txt, &registry), FileView::Table);
        assert_eq!(pair_view(&txt, &txt, &registry), FileView::Text);
        // 需要外部转换器的 xls 读不了：按内容落到十六进制。
        let xls = dir.file("old.xls", b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1\x00\x00");
        assert!(!is_table(&xls, &registry));
        assert_eq!(file_view(&xls, &registry), FileView::Hex);
        // *.bin 与二进制内容走十六进制；格式库里的文本格式仍是文本。
        assert_eq!(
            file_view(Path::new("missing.bin"), &registry),
            FileView::Hex
        );
        let exe = dir.file("a.exe", b"MZ\x90\x00\x03\x00\x00\x00");
        assert_eq!(file_view(&exe, &registry), FileView::Hex);
        let cpp = dir.file("main.cpp", b"int main() {}\n");
        assert_eq!(file_view(&cpp, &registry), FileView::Text);
    }
}
