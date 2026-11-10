# /// Wave 3 人工像素核验：对三个文件夹场景，逐项比较参考与实测的关键边界/颜色。
# /// compare.ps1 的自动扫描在平坦色带上会虚报；本脚本给出手工测量证据。
import json, sys
from PIL import Image

def px(im, x, y):
    return im.getpixel((x, y))

def find_vline(im, x_from, x_to, y, pred):
    """在 y 行上 [x_from, x_to) 内找第一条满足 pred 的竖线 x。"""
    for x in range(x_from, x_to):
        if pred(px(im, x, y)):
            return x
    return None

def find_hline(im, y_from, y_to, x, pred):
    for y in range(y_from, y_to):
        if pred(px(im, x, y)):
            return y
    return None

def is_color(c, r, g, b, tol=3):
    return abs(c[0]-r) <= tol and abs(c[1]-g) <= tol and abs(c[2]-b) <= tol

checks = [
    # (名称, 类型, 参数, 期望值)
    ("menubar 底 (toolbar 顶 y27 #F0F0F0)", 'hline', (20, 34, 500, lambda c: is_color(c, 240, 240, 240)), 27),
    ("toolbar 底线 y70 #848484", 'hline', (60, 80, 500, lambda c: is_color(c, 132, 132, 132)), 70),
    ("filterbar 高光 y71 #F5F5F5", 'hline', (70, 76, 500, lambda c: is_color(c, 245, 245, 245)), 71),
    ("过滤组合框左边 x43 (#8D8D8D)", 'vline', (38, 50, 93, lambda c: is_color(c, 141, 141, 141)), 43),
    ("过滤组合框顶边 y81", 'hline', (75, 88, 100, lambda c: is_color(c, 141, 141, 141)), 81),
    ("pathbar 高光 y115 #FFFFFF", 'hline', (110, 120, 660, lambda c: is_color(c, 255, 255, 255)), 115),
    ("左路径框左边 x1 #8D8D8D", 'vline', (0, 6, 128, lambda c: is_color(c, 141, 141, 141)), 1),
    ("左路径框右边 x593", 'vline', (588, 600, 128, lambda c: is_color(c, 141, 141, 141)), 593),
    ("右路径框左边 x702", 'vline', (697, 708, 128, lambda c: is_color(c, 141, 141, 141)), 702),
    ("列头顶线 y141 #ECECEC（中缝上也贯通）", 'hline', (135, 146, 680, lambda c: is_color(c, 236, 236, 236)), 141),
    ("列头高光 y142 #FEFEFE", 'hline', (141, 148, 680, lambda c: is_color(c, 254, 254, 254)), 142),
    ("左列头右缘 x664 #A0A0A0", 'vline', (660, 668, 152, lambda c: is_color(c, 160, 160, 160)), 664),
    ("右列头右缘 x1365 #A0A0A0", 'vline', (1360, 1370, 152, lambda c: is_color(c, 160, 160, 160)), 1365),
    ("左树右缘 x664 #C0C0C0（树区）", 'vline', (660, 668, 300, lambda c: is_color(c, 192, 192, 192)), 664),
    ("右树左缘 x702 #C0C0C0（树区）", 'vline', (698, 706, 300, lambda c: is_color(c, 192, 192, 192)), 702),
    ("首行选中底 (300,165) #A8CDF1", 'point', (300, 165), (168, 205, 241)),
    ("树底线 y830 #FEFEFE", 'hline', (824, 836, 680, lambda c: is_color(c, 254, 254, 254)), 830),
    ("日志顶线 y831 #838383", 'hline', (828, 838, 680, lambda c: is_color(c, 131, 131, 131)), 831),
    ("日志底线 y935 #838383", 'hline', (930, 938, 680, lambda c: is_color(c, 131, 131, 131)), 935),
    ("状态栏顶线 y936 #D7D7D7", 'hline', (934, 942, 500, lambda c: is_color(c, 215, 215, 215)), 936),
    ("滚动条槽 #F0F0F0 @x1370,y300", 'point', (1370, 300), (240, 240, 240)),
    ("树接缝只到内容末行：x664 @y500 为白", 'point', (664, 500), (255, 255, 255)),
]

def run(scene, ref_path, act_path):
    ref = Image.open(ref_path).convert('RGB')
    act = Image.open(act_path).convert('RGB')
    rows = []
    for name, kind, args, expect in checks:
        if kind == 'hline':
            y0, y1, x, pred = args
            r = find_hline(ref, y0, y1, x, pred)
            a = find_hline(act, y0, y1, x, pred)
            dev = None if (r is None or a is None) else a - r
            rows.append((name, r, a, dev))
        elif kind == 'vline':
            x0, x1, y, pred = args
            r = find_vline(ref, x0, x1, y, pred)
            a = find_vline(act, x0, x1, y, pred)
            dev = None if (r is None or a is None) else a - r
            rows.append((name, r, a, dev))
        else:
            x, y = args
            r = px(ref, x, y)
            a = px(act, x, y)
            dev = max(abs(r[0]-a[0]), abs(r[1]-a[1]), abs(r[2]-a[2]))
            rows.append((name, f"{r}", f"{a}", dev))
    print(f"== {scene}")
    worst = 0
    for name, r, a, dev in rows:
        flag = "" if dev is not None and abs(dev) <= 2 else "  <<<"
        print(f"  {name}: ref={r} act={a} dev={dev}{flag}")
        if dev is not None:
            worst = max(worst, abs(dev))
    print(f"  worst |dev| = {worst}")

for scene in ["folder-basic", "folder-expanded", "folder-diffs-filter"]:
    run(scene,
        f"docs/visual-parity/reference/{scene}.png",
        f"target/visual/matcha/{scene}.png")
