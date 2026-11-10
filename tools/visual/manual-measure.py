"""手动像素测量：沿水平/垂直线扫描颜色突变，定位核心区域边界。
用法: python tools/visual/manual-measure.py <ref.png> <act.png>
只读，不改 compare.ps1 的判定规则，用于自动检测不可靠时的人工复核。
"""
import sys
from PIL import Image

def col(img, x, y):
    return img.getpixel((x, y))[:3]

def near(a, b, tol=8):
    return all(abs(int(x)-int(y)) <= tol for x, y in zip(a, b))

def vscan(img, x, y0, y1):
    """返回 x 列上 y0..y1 间颜色突变点列表。"""
    out = []
    prev = col(img, x, y0)
    for y in range(y0+1, y1):
        c = col(img, x, y)
        if not near(c, prev, 12):
            out.append((y, prev, c))
        prev = c
    return out

def hscan(img, y, x0, x1):
    out = []
    prev = col(img, x0, y)
    for x in range(x0+1, x1):
        c = col(img, x, y)
        if not near(c, prev, 12):
            out.append((x, prev, c))
        prev = c
    return out

def show(name, img):
    W, H = img.size
    print(f"== {name} {W}x{H} ==")
    # 垂直分区：在 x=400（左窗格内）扫 y 边界
    print("-- vertical edges at x=400 --")
    for y, a, b in vscan(img, 400, 0, H):
        print(f"  y={y}: {a} -> {b}")
    print("-- vertical edges at x=10 (thumbnail strip) --")
    for y, a, b in vscan(img, 10, 0, H):
        print(f"  y={y}: {a} -> {b}")

def main():
    ref = Image.open(sys.argv[1]).convert("RGB")
    act = Image.open(sys.argv[2]).convert("RGB")
    show("REFERENCE", ref)
    show("ACTUAL", act)
    # 水平分区：左窗格内 y=200 扫 x 边界（gutter/pane/splitter）
    for label, img in (("REF", ref), ("ACT", act)):
        print(f"-- horizontal edges at y=200 ({label}) --")
        for x, a, b in hscan(img, 200, 0, img.size[0]):
            print(f"  x={x}: {a} -> {b}")
        print(f"-- horizontal edges at y=875 paneStatus ({label}) --")
        for x, a, b in hscan(img, 875, 0, img.size[0]):
            print(f"  x={x}: {a} -> {b}")

if __name__ == "__main__":
    main()
