"""人工边界测量：对 compare.ps1 判 FAIL 的平坦区域，用内容行上的可见跃迁测量真实几何。
用法: python tools/visual/manual-verify.py <ref.png> <act.png>
"""
import sys
from PIL import Image

GRAY = (240, 240, 240)

def px(img, x, y):
    return img.getpixel((x, y))[:3]

def is_grayish(c, tol=6):
    return all(abs(c[i] - GRAY[i]) <= tol for i in range(3))

def measure(label, ref_f, act_f):
    r, a = ref_f(), act_f()
    d = None if (r is None or a is None) else a - r
    print(f"{label:34s} ref={r} act={a} delta={d}")

def main():
    ref = Image.open(sys.argv[1]).convert("RGB")
    act = Image.open(sys.argv[2]).convert("RGB")

    # gutter.right 右缘（=pane.right 左缘 x=762）：用文本行 y=130（行1 白底），
    # 从右向左找最后一个面板灰像素。
    def gutter_right(img):
        y = 130
        for x in range(770, 750, -1):
            if is_grayish(px(img, x, y)):
                return x + 1  # 首个非灰列
        return None
    measure("gutter.right right edge@y130", lambda: gutter_right(ref), lambda: gutter_right(act))

    # gutter 右缘在 EOF 灰区（y=300）不可见，两者皆平。
    print("gutter.right EOF area: both flat #F0F0F0 ->",
          all(is_grayish(px(ref, x, 300)) and is_grayish(px(act, x, 300)) for x in range(755, 770)))

    # info 条边界：y=110 横扫 716..726 与 48..56。
    def hline(img, y, x0, x1):
        return [px(img, x, y) for x in range(x0, x1)]
    print("info.left right edge zone REF:", hline(ref, 110, 714, 722))
    print("info.left right edge zone ACT:", hline(act, 110, 714, 722))
    print("info.right left edge zone REF:", hline(ref, 110, 719, 727))
    print("info.right left edge zone ACT:", hline(act, 110, 719, 727))
    print("info.left left edge zone REF:", hline(ref, 110, 49, 57))
    print("info.left left edge zone ACT:", hline(act, 110, 49, 57))

    # paneStatus.left 右缘：y=877，REF 在 717 白、722 处 160 边。
    print("paneStatus seam REF:", hline(ref, 877, 714, 726))
    print("paneStatus seam ACT:", hline(act, 877, 714, 726))

    # pathbar.left 右缘：y=83。
    print("pathbar.left right zone REF:", hline(ref, 83, 708, 722))
    print("pathbar.left right zone ACT:", hline(act, 83, 708, 722))

    # statusbar 右缘 y=945。
    print("statusbar right REF:", hline(ref, 945, 1378, 1384))
    print("statusbar right ACT:", hline(act, 945, 1378, 1384))
    print("statusbar left REF:", hline(ref, 945, 0, 6))
    print("statusbar left ACT:", hline(act, 945, 0, 6))

    # splitter 718..721 在 pane 区 y=300：REF/ACT 颜色带。
    print("splitter y300 REF:", hline(ref, 300, 716, 724))
    print("splitter y300 ACT:", hline(act, 300, 716, 724))
    # splitter 顶/底
    print("splitter top REF:", [px(ref, 719, y) for y in range(120, 128)])
    print("splitter top ACT:", [px(act, 719, y) for y in range(120, 128)])
    print("splitter bottom REF:", [px(ref, 719, y) for y in range(884, 891)])
    print("splitter bottom ACT:", [px(act, 719, y) for y in range(884, 891)])

    # 右窗格文本起点（pane.right 内容左缘）
    def text_start(img, y):
        for x in range(755, 780):
            c = px(img, x, y)
            if sum(c) < 700:
                return x
        return None
    measure("pane.right text ink start@y205", lambda: text_start(ref, 205), lambda: text_start(act, 205))

if __name__ == "__main__":
    main()
