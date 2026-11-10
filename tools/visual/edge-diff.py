"""对齐测量：在指定扫描线上比较参考与实测的结构性边界（忽略文字墨迹）。
打印每个结构边界的 ref/act 位置与偏差。只读工具。
"""
import sys
from PIL import Image

PANEL = (240, 240, 240)
WHITE = (255, 255, 255)
BORDER160 = (160, 160, 160)
BORDER141 = (141, 141, 141)
BORDER215 = (215, 215, 215)
GRAYS = {PANEL, WHITE, BORDER160, BORDER141, BORDER215, (192, 192, 192), (248, 248, 248)}

def col(img, x, y):
    return img.getpixel((x, y))[:3]

def structural_edges_v(img, x):
    """x 列上只保留灰阶/白/面板色之间的突变（忽略彩色文字）。"""
    W, H = img.size
    edges = []
    prev = col(img, x, 0)
    for y in range(1, H):
        c = col(img, x, y)
        if c != prev and c in GRAYS and prev in GRAYS:
            edges.append((y, prev, c))
        prev = c
    return edges

def structural_edges_h(img, y):
    W, H = img.size
    edges = []
    prev = col(img, 0, y)
    for x in range(1, W):
        c = col(img, x, y)
        if c != prev and c in GRAYS and prev in GRAYS:
            edges.append((x, prev, c))
        prev = c
    return edges

def diff_edges(ref_e, act_e, axis):
    print(f"--- {axis} structural edges: ref vs act ---")
    ri = ai = 0
    rpos = [e[0] for e in ref_e]
    apos = [e[0] for e in act_e]
    print("ref:", rpos)
    print("act:", apos)
    # 简单对齐：对每个 ref 边界找 ±3 内最近 act 边界
    used = set()
    for e in ref_e:
        p = e[0]
        best = None
        for q in apos:
            if q in used:
                continue
            if abs(q - p) <= 3 and (best is None or abs(q - p) < abs(best - p)):
                best = q
        if best is not None:
            used.add(best)
            mark = "" if best == p else f"  delta={best-p:+d}"
            print(f"  {p} -> {best}{mark}   {e[1]}->{e[2]}")
        else:
            print(f"  {p} -> MISSING   {e[1]}->{e[2]}")

def main():
    ref = Image.open(sys.argv[1]).convert("RGB")
    act = Image.open(sys.argv[2]).convert("RGB")
    for x in (400, 1000, 60, 730):
        diff_edges(structural_edges_v(ref, x), structural_edges_v(act, x), f"vertical x={x}")
    for y in (300, 875, 945):
        diff_edges(structural_edges_h(ref, y), structural_edges_h(act, y), f"horizontal y={y}")

if __name__ == "__main__":
    main()
