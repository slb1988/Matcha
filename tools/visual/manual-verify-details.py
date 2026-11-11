"""Element-level ink boxes of the text details strip (reference vs Matcha)."""
import sys
from PIL import Image
scene = sys.argv[1] if len(sys.argv) > 1 else 'text-basic'
ref = Image.open(f'docs/visual-parity/reference/{scene}.png').convert('RGB')
act = Image.open(sys.argv[2] if len(sys.argv) > 2 else f'target/visual/matcha/{scene}.png').convert('RGB')
def bbox(im, x0, y0, x1, y1, f):
    pts = [(x, y) for x in range(x0, x1) for y in range(y0, y1) if f(im.getpixel((x, y)))]
    if not pts:
        return None
    xs = [q[0] for q in pts]; ys = [q[1] for q in pts]
    return (min(xs), min(ys), max(xs), max(ys))
dark = lambda c: sum(c) < 300
checks = [
    ('frame top rule', (52, 884, 1384, 887), lambda c: c == (160, 160, 160)),
    ('row divider', (300, 900, 301, 905), lambda c: c == (160, 160, 160)),
    ('bottom highlight', (300, 930, 301, 937), lambda c: c == (255, 255, 255)),
    ('text row 1', (68, 887, 105, 902), dark),
    ('text row 2', (68, 903, 105, 918), dark),
    ('line-end marker 1', (105, 887, 125, 902), lambda c: 100 < sum(c) / 3 < 215),
    ('line-end marker 2', (105, 903, 125, 918), lambda c: 100 < sum(c) / 3 < 215),
    ('text box 1 (white)', (70, 889, 130, 890), lambda c: c == (255, 255, 255)),
    ('arrow 1', (53, 888, 70, 901), lambda c: sum(c) < 400),
    ('arrow 2', (53, 904, 70, 917), lambda c: sum(c) < 400),
    ('placeholder dash', (55, 920, 140, 934), lambda c: c[0] < 200),
]
worst = 0
for name, box, f in checks:
    a, b = bbox(ref, *box, f), bbox(act, *box, f)
    d = None if not a or not b else tuple(y - x for x, y in zip(a, b))
    if d:
        worst = max(worst, max(abs(v) for v in d))
    print(f'|{name}|{a}|{b}|{d}|')
print('worst |dev| =', worst)
