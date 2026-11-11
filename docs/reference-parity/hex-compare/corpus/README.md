# Hex Compare 参考语料

本目录语料用于 `hex-compare` change 的参考采集与黄金对拍，全部可由 `gen_corpus.py` 确定性复现：

```bash
python gen_corpus.py        # 在本目录重新生成 4 个 .bin
```

| 文件 | 大小 | 构造 | 期望四分类（Hex Compare Info） |
|---|---|---|---|
| `left.bin` / `right.bin` | 4160 / 4144 B | 8 处单字节改动（0x100/0x104/0x108、0x200/0x204/0x208/0x20C、0x300）；0x400 处插入 16 字节（`00..0F`）；删除原坐标 0x5F0–0x60F 共 32 字节 | 4120 same / 8 difference / 32 left orphan / 16 right orphan |
| `l2.bin` / `r2.bin` | 2048 / 2053 B | 0x123 处插入 5 字节（`AA BB CC DD EE`），非 16 倍数长度 | 2048 same / 0 difference / 0 left orphan / 5 right orphan |

对应截图证据：`../01-main.png`（改动区）、`../06-insertion-closeup.png`（整行孤儿）、`../22-diffs-only.png`（Diffs 过滤 + 上下文）、`../23-odd-insert.png` 与 `../24-odd-insert-all.png`（行中孤儿与相位偏移）、`../20-info.png`（四分类统计）。

参考工具侧驱动方式（Windows，Beyond Compare 5）：直接双击/命令行打开两个 `.bin` 文件即路由到 Hex Compare；黄金对拍用脚本 `hex-report layout:side-by-side options:display-all,line-numbers output-to:<file>`（见 `scripting_reference.html`），调用前持 `%TEMP%\matcha-reference-tool.lock`。
