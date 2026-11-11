"""Element edges of the two dialog scenes, reference vs Matcha.

Each check scans one row or column for runs of an exact edge color and
reports the edge coordinates in both images, e.g. the tab-page frame
(#E5E5E5), field borders (#8D8D8D), checkbox boxes (#626262), button
frames (#D0D0D0 / #0078D4) and the Options rules (#A0A0A0)."""
import sys
from PIL import Image


def edges(im, axis, fixed, lo, hi, colors):
    out = []
    for v in range(lo, hi):
        p = (v, fixed) if axis == 'x' else (fixed, v)
        if im.getpixel(p) in colors:
            out.append(v)
    return out


def runs(vals):
    """Collapse consecutive coordinates to their first value."""
    out = []
    for v in vals:
        if not out or v != out[-1][1] + 1:
            out.append([v, v])
        else:
            out[-1][1] = v
    return [a for a, _ in out]


E5 = {(229, 229, 229)}
F8D = {(141, 141, 141)}
CHK = {(98, 98, 98)}
BTN = {(208, 208, 208)}
DEF = {(0, 120, 212)}
RULE = {(160, 160, 160)}
COMBO = {(210, 210, 210)}

SESSION = [
    ('tab page + tabs, row y20 (#E5)', 'x', 20, 0, 583, E5),
    ('page frame, column x300 (#E5)', 'y', 300, 0, 563, E5),
    ('unselected tab, column x80 (#E5)', 'y', 80, 0, 40, E5),
    ('path fields, column x30 (#8D)', 'y', 30, 60, 180, F8D),
    ('path field, row y80 (#8D)', 'x', 80, 0, 583, F8D),
    ('checkboxes, column x28 (#62)', 'y', 28, 100, 205, CHK),
    ('checkbox, row y112 (#62)', 'x', 112, 0, 60, CHK),
    ('small buttons, row y80 (#D0)', 'x', 80, 500, 583, BTN),
    ('middle buttons, row y115 (#D0)', 'x', 115, 240, 340, BTN),
    ('OK button, row y537 (#0078D4)', 'x', 537, 300, 583, DEF),
    ('Cancel button, row y537 (#D0)', 'x', 537, 470, 583, BTN),
    ('scope combo, row y537 (#D2)', 'x', 537, 0, 300, COMBO),
    ('OK button, column x400 (#0078D4)', 'y', 400, 500, 563, DEF),
]
OPTIONS = [
    ('nav divider, row y300 (#A0)', 'x', 300, 0, 300, RULE),
    ('bottom rule, column x400 (#A0)', 'y', 400, 590, 655, RULE),
    ('buttons, row y630 (#0078D4)', 'x', 630, 500, 859, DEF),
    ('Cancel, row y630 (#D0)', 'x', 630, 640, 750, BTN),
    ('search box focus line, column x120', 'y', 120, 0, 60, {(0, 103, 192)}),
    ('OK button, column x590 (#0078D4)', 'y', 590, 600, 655, DEF),
]


def main():
    sets = {
        'dlg-text-session-settings': SESSION,
        'dlg-options': OPTIONS,
    }
    for scene, checks in sets.items():
        ref = Image.open(f'docs/visual-parity/reference/{scene}.png').convert('RGB')
        act = Image.open(f'target/visual/matcha/{scene}.png').convert('RGB')
        print(f'## {scene}')
        print('|check|reference|matcha|max abs dev|')
        print('|---|---|---|---|')
        for name, axis, fixed, lo, hi, colors in checks:
            a = runs(edges(ref, axis, fixed, lo, hi, colors))
            b = runs(edges(act, axis, fixed, lo, hi, colors))
            dev = max((abs(x - y) for x, y in zip(a, b)), default=None) if len(a) == len(b) else 'count differs'
            print(f'|{name}|{a}|{b}|{dev}|')


if __name__ == '__main__':
    sys.exit(main())
