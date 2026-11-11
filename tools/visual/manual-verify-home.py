"""Element-level ink boxes on Home (reference vs Matcha); prints deltas."""
import sys
from PIL import Image
ref = Image.open(sys.argv[1] if len(sys.argv) > 1 else 'docs/visual-parity/reference/home.png').convert('RGB')
act = Image.open(sys.argv[2] if len(sys.argv) > 2 else 'target/visual/matcha/home.png').convert('RGB')
def bbox(im, x0, y0, x1, y1, dark):
    pts = [(x, y) for x in range(x0, x1) for y in range(y0, y1) if dark(im.getpixel((x, y)))]
    if not pts:
        return None
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    return (min(xs), min(ys), max(xs), max(ys))
ink = lambda c: sum(c) < 400
not_canvas = lambda c: max(abs(v - 249) for v in c) > 3  # renderer dithers ±1
checks = [
    ('Sessions title', (0, 30, 270, 59), ink),
    ('New selection', (15, 61, 120, 81), lambda c: c == (0, 120, 212)),
    ('New label', (40, 62, 72, 80), lambda c: sum(c) < 250),
    ('Auto-saved label', (40, 81, 140, 100), ink),
    ('plus button', (0, 787, 37, 824), lambda c: c[0] < 0xE0),
    ('search box', (70, 787, 274, 824), lambda c: c[0] > 0xF8 or c[0] < 0x90),
    ('text line 1', (600, 290, 1100, 316), ink),
    ('text line 2', (600, 318, 1100, 340), ink),
    ('icon 1', (616, 355, 736, 437), not_canvas),
    ('icon 2', (774, 355, 894, 437), not_canvas),
    ('icon 3', (932, 355, 1052, 437), not_canvas),
    ('label 1', (606, 438, 746, 460), ink),
    ('label 2', (764, 438, 904, 460), ink),
    ('label 3', (922, 438, 1062, 460), ink),
    ('chevron', (1250, 350, 1350, 450), not_canvas),
    ('panel close', (1350, 832, 1384, 855), None),
]
worst = 0
for name, box, f in checks:
    if f is None:  # panel close: white on brand red vs dark on neutral gray
        f = (lambda c: c[0] > 200 and c[1] > 200 and c[2] > 200)
        a = bbox(ref, *box, f)
        b = bbox(act, *box, lambda c: sum(c) < 400)
        d = tuple(bb - aa for aa, bb in zip(a, b))
        worst = max(worst, max(abs(v) for v in d))
        print(f'|{name}|{a}|{b}|{d}|')
        continue
    a, b = bbox(ref, *box, f), bbox(act, *box, f)
    d = None if not a or not b else tuple(bb - aa for aa, bb in zip(a, b))
    if d:
        worst = max(worst, max(abs(v) for v in d))
    print(f'|{name}|{a}|{b}|{d}|')
print('worst |dev| =', worst)
