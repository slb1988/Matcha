"""Dropdown geometry of the text-menu-edit scene: border box, separator rows,
first label / shortcut ink and open-title fill, per image."""
import sys
from PIL import Image
paths = sys.argv[1:] or ['docs/visual-parity/reference/text-menu-edit.png', 'target/visual/matcha/text-menu-edit.png']
def measure(path):
    im = Image.open(path).convert('RGB')
    w, h = im.size
    # Popup border: the #A0A0A0 column/row runs below the menu bar.
    xs = [x for x in range(0, 700) if im.getpixel((x, 100)) == (160, 160, 160)]
    left, right = (xs[0], xs[1]) if len(xs) >= 2 else (None, None)
    probe = (left + right) // 2 if left else 200
    ys = [y for y in range(20, h) if im.getpixel((probe, y)) == (160, 160, 160)]
    top, bottom = (ys[0], ys[1]) if len(ys) >= 2 else (None, None)
    seps = [y for y in range(20, h) if im.getpixel((probe, y)) == (215, 215, 215)]
    body = im.getpixel((probe, (top or 30) + 8))
    title = [x for x in range(60, 160) if im.getpixel((x, 20)) == (249, 249, 249)]
    return {
        'border x': (left, right), 'border y': (top, bottom), 'width': right - left + 1 if left else None,
        'separators y': seps, 'body': '#%02X%02X%02X' % body,
        'open title x': (title[0], title[-1]) if title else None,
    }
for p in paths:
    print(p)
    for k, v in measure(p).items():
        print(f'  {k}: {v}')
