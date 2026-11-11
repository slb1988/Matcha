"""重新生成 hex compare 参考采集所用的二进制语料（确定性，与截图所用文件逐字节一致）。

用法：python gen_corpus.py [输出目录]   # 默认输出到本脚本所在目录

语料：
- left.bin (4160B) / right.bin (4144B)：8 处单字节改动（0x100/0x104/0x108、0x200/0x204/0x208/0x20C、0x300）、
  16 字节插入（0x400，内容 00..0F）、32 字节删除（原始坐标 0x5F0-0x60F）。
  期望 Hex Compare Info：4120 same / 32 left orphan / 16 right orphan / 8 difference。
- l2.bin (2048B) / r2.bin (2053B)：5 字节插入（0x123，内容 AA BB CC DD EE），
  用于验证非 16 倍数孤儿段的单元格行打包与行标签相位偏移。
"""
import os
import random
import sys


def gen_primary():
    random.seed(42)
    hdr = b"MZ" + bytes(random.randrange(256) for _ in range(58)) + (0x80).to_bytes(4, "little")
    body = bytes(random.randrange(256) for _ in range(4096))
    left = hdr + body
    b = bytearray(left)
    for off in (0x100, 0x104, 0x108, 0x200, 0x204, 0x208, 0x20C, 0x300):
        b[off] ^= 0xFF
    b[0x400:0x400] = bytes(range(16))  # 插入 16 字节
    del b[0x600:0x620]  # 删除 32 字节（插入后坐标；等于原坐标 0x5F0-0x60F）
    return left, bytes(b)


def gen_odd_insert():
    random.seed(7)
    left = bytes(random.randrange(256) for _ in range(2048))
    r = bytearray(left)
    r[0x123:0x123] = bytes([0xAA, 0xBB, 0xCC, 0xDD, 0xEE])
    return left, bytes(r)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__))
    os.makedirs(out, exist_ok=True)
    pairs = [gen_primary(), gen_odd_insert()]
    names = [("left.bin", "right.bin"), ("l2.bin", "r2.bin")]
    for (l, r), (ln, rn) in zip(pairs, names):
        open(os.path.join(out, ln), "wb").write(l)
        open(os.path.join(out, rn), "wb").write(r)
        print(f"{ln}={len(l)}B {rn}={len(r)}B")


if __name__ == "__main__":
    main()
