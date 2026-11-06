import contextlib
import io
import tempfile
import unittest
import sys
from pathlib import Path
sys.dont_write_bytecode = True
from check_table_parity import TOPICS, check

class CoverageTests(unittest.TestCase):
    def fixture(self, root):
        help_dir = root / "help"
        help_dir.mkdir()
        for name in TOPICS:
            (help_dir / name).write_text("<html></html>", encoding="utf-8")
        html = ('<meta charset="ISO-8859-1"><p>description\u00a0only</p>'
                '<p class="p_Command">Open With &gt;</p>'
                '<p class="p_Command"><span>Open With &gt;</span></p>')
        (help_dir / TOPICS[0]).write_bytes(html.encode("iso-8859-1"))
        text = ("| C01 | `Open With >` | group | note |\n"
                "| C02 | `Open With >` | group | note |\n"
                + "\n".join(f"`{name}`" for name in TOPICS))
        doc = root / "coverage.md"
        doc.write_text(text, encoding="utf-8")
        return help_dir, doc, text

    def test_declared_encoding_and_duplicate_paragraphs(self):
        with tempfile.TemporaryDirectory() as directory:
            help_dir, doc, _ = self.fixture(Path(directory))
            with contextlib.redirect_stdout(io.StringIO()) as out:
                check(help_dir, doc)
            self.assertIn("command_paragraphs=2 missing=0 topics=15", out.getvalue())

    def test_missing_duplicate_or_wrong_label_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            help_dir, doc, text = self.fixture(Path(directory))
            for malformed in (
                text.replace("| C02 | `Open With >` | group | note |\n", ""),
                text.replace("| C02", "| C03"),
                text.replace("| C02 | `Open With >`", "| C02 | `Open File`"),
                text.replace("`formattableregional.html`", ""),
            ):
                with self.subTest(document=malformed):
                    doc.write_text(malformed, encoding="utf-8")
                    with self.assertRaises(ValueError):
                        check(help_dir, doc)

    def test_missing_source_topic_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            help_dir, doc, _ = self.fixture(Path(directory))
            (help_dir / "sessiontablesheets.html").unlink()
            with self.assertRaises(ValueError):
                check(help_dir, doc)

if __name__ == "__main__":
    unittest.main()
