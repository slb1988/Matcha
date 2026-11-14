"""Writes the three-way merge corpus inputs (left/right/center) byte-exactly.

    python -I tools/golden/gen_merge_cases.py [<corpus root>]

The inputs are checked in; this script only documents how they were built and lets them be
rebuilt. The corpus directory carries `* -text` in .gitattributes so line endings, BOMs and
ANSI bytes survive checkout. Expected outputs come from gen_merge_expected.ps1.
"""
import os
import sys

BASE = ["line%02d" % i for i in range(1, 13)]


def text(lines, eol="\n", final=True):
    body = eol.join(lines)
    return body + (eol if final and lines else "")


def edit(changes=None, insert=None, delete=()):
    """BASE with 1-based `changes` {n: text}, `insert` {after_n: [lines]}, `delete` (n, ...)."""
    changes = changes or {}
    insert = insert or {}
    out = list(insert.get(0, []))
    for n, line in enumerate(BASE, 1):
        if n not in delete:
            out.append(changes.get(n, line))
        out.extend(insert.get(n, []))
    return out


def case(left, right, center=None, eol=("\n", "\n", "\n"), final=(True, True, True), enc=("utf-8",) * 3):
    center = BASE if center is None else center
    sides = (left, right, center)
    return [text(s, e, f).encode(c) for s, e, f, c in zip(sides, eol, final, enc)]


CRLF = "\r\n"
CASES = {
    "001-left-only": case(edit({5: "line05 LEFT"}), BASE),
    "002-both-far": case(edit({3: "line03 LEFT"}), edit({10: "line10 RIGHT"})),
    "003-gap0": case(edit({5: "line05 LEFT"}), edit({6: "line06 RIGHT"})),
    "004-gap1": case(edit({5: "line05 LEFT"}), edit({7: "line07 RIGHT"})),
    "005-gap2": case(edit({5: "line05 LEFT"}), edit({8: "line08 RIGHT"})),
    "006-gap3": case(edit({5: "line05 LEFT"}), edit({9: "line09 RIGHT"})),
    "007-same-line": case(edit({5: "line05 LEFT"}), edit({5: "line05 RIGHT"})),
    "008-same-change": case(edit({5: "line05 BOTH"}), edit({5: "line05 BOTH"})),
    "009-insert-unequal": case(
        edit(insert={5: ["left new 1", "left new 2", "left new 3"]}),
        edit(insert={5: ["right new 1"]}),
    ),
    "010-delete-vs-modify": case(edit(delete=(5, 6)), edit({5: "line05 RIGHT"})),
    "011-insert-identical": case(
        edit(insert={5: ["both new 1", "both new 2"]}),
        edit(insert={5: ["both new 1", "both new 2"]}),
    ),
    "012-ws-left-only": case(edit({5: "    line05"}), BASE),
    "013-ws-conflict": case(edit({5: "    line05"}), edit({5: "\tline05"})),
    "014-ws-vs-real": case(edit({5: "    line05"}), edit({5: "line05 RIGHT"})),
    "015-eol-left-lf": case(edit({5: "line05 LEFT"}), BASE, eol=("\n", CRLF, CRLF)),
    "016-enc-left-utf8bom": case(
        edit({2: "café 02"}),
        edit({2: "café 02", 9: "line09 RIGHT"}),
        center=edit({2: "café 02"}),
        enc=("utf-8-sig", "cp1252", "cp1252"),
    ),
    "017-no-final-newline": case(
        edit({12: "line12 LEFT"}), edit({12: "line12 RIGHT"}), final=(False, False, False)
    ),
    "018-all-same": case(BASE, BASE),
    "019-case-conflict": case(edit({5: "LINE05"}), edit({5: "Line05"})),
    "020-far-and-conflict": case(
        edit({2: "line02 LEFT", 9: "line09 LEFT"}), edit({9: "line09 RIGHT", 12: "line12 RIGHT"})
    ),
}


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        here, "..", "..", "crates", "bc-engine-text", "tests", "golden", "merge"
    )
    os.makedirs(root, exist_ok=True)
    with open(os.path.join(root, ".gitattributes"), "wb") as f:
        f.write(b"* -text\n")
    for name, sides in sorted(CASES.items()):
        folder = os.path.join(root, name)
        os.makedirs(folder, exist_ok=True)
        for leaf, data in zip(("left.txt", "right.txt", "center.txt"), sides):
            with open(os.path.join(folder, leaf), "wb") as f:
                f.write(data)
    print("%d merge case(s) written to %s" % (len(CASES), os.path.normpath(root)))


if __name__ == "__main__":
    main()
