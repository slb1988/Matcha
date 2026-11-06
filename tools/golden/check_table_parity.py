"""Check command/option documentation coverage against explicitly extracted help.
Read-only; no executable invocation, configuration access, or extra dependency.
"""
import argparse
import re
from html.parser import HTMLParser
from pathlib import Path

TOPICS = (
    "commandstable.html", "sessiontable.html", "sessiontablealignment.html",
    "sessiontablecolumns.html", "sessiontablesheets.html", "sessiontableformats.html",
    "sessiontablespecs.html", "dlgtablecolhandling.html", "formattable.html",
    "formattablegeneral.html", "formattableconversion.html", "formattabletype.html",
    "formattableregional.html", "table_working_with_multiple.html", "viewtable.html",
)

class Commands(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.active = False
        self.parts = []
        self.commands = []

    def handle_starttag(self, tag, attrs):
        if tag == "p":
            self.active = "p_Command" in dict(attrs).get("class", "").split()
            self.parts = []
        elif tag == "br" and self.active:
            self.parts.append(" ")

    def handle_data(self, data):
        if self.active:
            self.parts.append(data)

    def handle_endtag(self, tag):
        if tag == "p" and self.active:
            self.commands.append(" ".join("".join(self.parts).split()))
            self.active = False

def check(help_dir, document):
    for name in TOPICS:
        if not (help_dir / name).is_file():
            raise ValueError(f"missing extracted topic: {name}")
    parser = Commands()
    raw = (help_dir / TOPICS[0]).read_bytes()
    declaration = re.search(br"""charset\s*=\s*["']?([A-Za-z0-9_-]+)""", raw[:4096], re.IGNORECASE)
    encoding = declaration[1].decode("ascii").lower() if declaration else "utf-8-sig"
    if raw.startswith(b"\xef\xbb\xbf"):
        encoding = "utf-8-sig"
    if encoding not in ("utf-8", "utf-8-sig", "iso-8859-1", "windows-1252"):
        raise ValueError(f"unsupported declared help encoding: {encoding}")
    parser.feed(raw.decode(encoding))
    if not parser.commands:
        raise ValueError("no p_Command entries; wrong help revision or parser contract")
    text = document.read_text(encoding="utf-8")
    rows = re.findall(r"^\| C(\d+) \| `([^`]+)` \|", text, re.MULTILINE)
    ids = [int(index) for index, _ in rows]
    if ids != list(range(1, len(parser.commands) + 1)):
        raise ValueError("command row IDs must cover every paragraph exactly once, in order")
    actual = [label for _, label in rows]
    for index, (expected, observed) in enumerate(zip(parser.commands, actual), 1):
        if expected != observed:
            raise ValueError(f"C{index:02}: help={expected!r}, documentation={observed!r}")
    for name in TOPICS:
        if f"`{name}`" not in text:
            raise ValueError(f"topic not documented: {name}")
    print(f"table.parity.command_paragraphs={len(actual)} missing=0 topics={len(TOPICS)}")
    print("Coverage only: this does not assert that all documented commands are implemented.")

if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--help-dir", type=Path, required=True)
    p.add_argument("--document", type=Path, default=Path(__file__).resolve().parents[2] / "docs/reference-parity/table-compare.md")
    args = p.parse_args()
    try:
        check(args.help_dir, args.document)
    except (OSError, ValueError) as error:
        p.exit(1, f"table parity check failed: {error}\n")
