"""Generate the Matcha app icon (tea leaf split into two panes).

Concept (docs/features/gui-shell.md, "应用图标"): a matcha tea leaf whose midrib is the
divider between the left and right panes; side veins are the "lines" being compared.
Every vein pair mirrors across the midrib except one: on the right it is shifted and
highlighted in the diff accent (orange) - the difference Matcha finds.
Name pun: Match·a (matching) / 抹茶 ≈ 抹差 (wipe out the differences).

Each size is drawn separately (16x supersampled, then downscaled) so small sizes get
fewer veins and thicker strokes instead of a blurred large drawing.

Usage:  python -I tools/icon/gen_icon.py
Writes: assets/icon/matcha.ico (16..256), matcha-256.png, matcha-1024.png
"""

import math
import os
import sys

from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "assets", "icon")

LEFT = (156, 197, 75, 255)  # matcha, light (left pane)
RIGHT = (122, 171, 52, 255)  # matcha, deeper (right pane)
OUTLINE = (47, 84, 24, 255)  # dark tea green: outline, midrib, stem
VEIN = (236, 245, 208, 255)  # pale cream veins
DIFF = (240, 138, 36, 255)  # diff accent

SS = 16  # supersampling factor

TIP_Y, BASE_Y, STEM_Y = 0.05, 0.80, 0.96
HALF_W = 0.36


def half_width(t):
    """Leaf half width at t (0 = base, 1 = tip): round shoulders, pointed tip."""
    return HALF_W * math.sin(math.pi * t ** 0.8) ** 0.75 * (1.0 - 0.18 * t)


def leaf_y(t):
    return BASE_Y + (TIP_Y - BASE_Y) * t


def leaf_points(side, n=200):
    """Polygon of one leaf half (side = -1 left, +1 right), unit coordinates."""
    pts = [(0.5, BASE_Y)]
    for i in range(1, n):
        t = i / n
        pts.append((0.5 + side * half_width(t), leaf_y(t)))
    pts.append((0.5, TIP_Y))
    return pts


def t_at_y(y):
    return (y - BASE_Y) / (TIP_Y - BASE_Y)


def vein(side, y0, reach=0.80, rise=0.13, n=24):
    """A slightly curved side vein from the midrib towards the edge."""
    w = half_width(t_at_y(y0 - rise * 0.6)) * reach
    pts = []
    for i in range(n + 1):
        u = i / n
        x = 0.5 + side * w * u
        y = y0 - rise * (u ** 1.4)
        pts.append((x, y))
    return pts


def layout(size):
    """(left vein ys, right vein ys, index of the differing right vein, stroke widths)."""
    if size <= 16:
        ys = [0.47]
        diff = 0
        shift = 0.10
        w = dict(outline=1.15, mid=1.25, vein=1.2, stem=1.4)
    elif size <= 32:
        ys = [0.60, 0.38]
        diff = 1
        shift = 0.08
        w = dict(outline=1.3, mid=1.5, vein=1.3, stem=1.8)
    else:
        ys = [0.64, 0.48, 0.32]
        diff = 1
        shift = 0.075
        w = dict(outline=size / 34, mid=size / 30, vein=size / 40, stem=size / 22)
    right = list(ys)
    right[diff] += shift
    return ys, right, diff, w


def draw(size):
    S = size * SS
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    def px(pts):
        return [(x * S, y * S) for x, y in pts]

    ys_l, ys_r, diff, w = layout(size)
    lw = lambda k: max(1, round(w[k] * SS))

    # Stem (behind the leaf), gently curving left.
    stem = [(0.5 - 0.06 * (u ** 2), BASE_Y - 0.02 + (STEM_Y - BASE_Y + 0.02) * u) for u in (i / 20 for i in range(21))]
    d.line(px(stem), fill=OUTLINE, width=lw("stem"), joint="curve")

    left, right = leaf_points(-1), leaf_points(+1)
    d.polygon(px(left), fill=LEFT)
    d.polygon(px(right), fill=RIGHT)

    # Veins: mirrored pairs, except the differing one on the right.
    for y in ys_l:
        d.line(px(vein(-1, y)), fill=VEIN, width=lw("vein"), joint="curve")
    for i, y in enumerate(ys_r):
        colour = DIFF if i == diff else VEIN
        width = lw("vein") if i != diff else max(lw("vein"), round(w["vein"] * SS * 1.25))
        d.line(px(vein(+1, y)), fill=colour, width=width, joint="curve")

    # Outline of the whole leaf.
    outline = left + list(reversed(right))
    d.line(px(outline + [outline[0]]), fill=OUTLINE, width=lw("outline"), joint="curve")

    # Midrib = the pane divider, from base to just under the tip.
    d.line(px([(0.5, BASE_Y), (0.5, TIP_Y + 0.06)]), fill=OUTLINE, width=lw("mid"))

    return img.resize((size, size), Image.LANCZOS)


def main():
    os.makedirs(OUT, exist_ok=True)
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    frames = {s: draw(s) for s in sizes}
    frames[256].save(
        os.path.join(OUT, "matcha.ico"),
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=[frames[s] for s in sizes if s != 256],
    )
    frames[256].save(os.path.join(OUT, "matcha-256.png"))
    draw(1024).save(os.path.join(OUT, "matcha-1024.png"))
    if "--preview" in sys.argv:
        sheet = Image.new("RGBA", (sum(sizes) + 10 * len(sizes) + 10, 276), (255, 255, 255, 255))
        x = 10
        for s in sizes:
            sheet.alpha_composite(frames[s], (x, 10))
            x += s + 10
        sheet.save(os.path.join(ROOT, "tmp", "icon-preview.png"))


if __name__ == "__main__":
    main()
