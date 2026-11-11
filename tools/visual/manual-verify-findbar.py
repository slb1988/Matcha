"""Element-level boxes of the docked find bar (reference vs Matcha)."""
import sys
from PIL import Image
ref = Image.open('docs/visual-parity/reference/text-find-bar.png').convert('RGB')
act = Image.open(sys.argv[1] if len(sys.argv) > 1 else 'target/visual/matcha/text-find-bar.png').convert('RGB')
def bbox(im, x0, y0, x1, y1, f):
    pts = [(x, y) for x in range(x0, x1) for y in range(y0, y1) if f(im.getpixel((x, y)))]
    if not pts:
        return None
    xs = [q[0] for q in pts]; ys = [q[1] for q in pts]
    return (min(xs), min(ys), max(xs), max(ys))
dark = lambda c: sum(c) < 350
frame = lambda c: max(c) < 0xE0 and abs(c[0] - c[2]) < 30  # gray button frames
blue = lambda c: c[2] > 180 and c[0] < 120  # focus / default borders, selection
checks = [
    ('search glyph', (0, 72, 40, 106), dark),
    ('combo border', (36, 74, 700, 106), blue),
    ('selection', (42, 80, 200, 101), lambda c: c == (0, 120, 215)),
    ('combo arrow', (675, 80, 694, 100), dark),
    ('history button', (700, 74, 734, 106), frame),
    ('toggle .*', (740, 74, 771, 106), frame),
    ('toggle Aa', (770, 74, 801, 106), frame),
    ('toggle word', (800, 74, 831, 106), frame),
    ('toggle wrap', (830, 74, 861, 106), frame),
    ('Prev button', (868, 74, 917, 106), frame),
    ('Next button', (917, 74, 968, 106), blue),
    ('Prev label', (874, 80, 914, 101), dark),
    ('Next label', (920, 80, 962, 101), dark),
    ('expand button', (1345, 74, 1384, 106), frame),
    ('path border below', (300, 104, 301, 112), lambda c: c == (141, 141, 141)),
]
worst = 0
for name, box, f in checks:
    a, b = bbox(ref, *box, f), bbox(act, *box, f)
    d = None if not a or not b else tuple(y - x for x, y in zip(a, b))
    if d:
        worst = max(worst, max(abs(v) for v in d))
    print(f'|{name}|{a}|{b}|{d}|')
print('worst |dev| =', worst)
