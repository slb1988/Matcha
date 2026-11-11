# 十六进制 / 图片 / 版本 / 注册表 / 媒体比较（Hex & Binary Compare）

> 非文本比较引擎家族（P2–P4）。状态：**未开始**——数据模型与入口已搭好，
> 所有函数体为 `todo!()`（openspec `hex-compare` 0/23）。

## 功能简介（规划）

- Hex compare：字节级对齐与差异（`HexContent`/`HexRules`/`HexMap`，复用
  `bc_core::align::ByteAlign`）。
- Picture compare：容差比较、颜色替换、混合模式（`PixRules`/`Image`/`PixDiff`/`BlendMode`）。
- Version compare：PE 版本信息提取。
- Registry compare：.reg 解析 / 活动 hive 读取。
- Media compare：ID3/Vorbis/MP4 原子标签读取。
- Picture/Version/Media 共享 `tag` 名值树模型（`TagValue`/`TagRow`/`TagRules`/`align_tags`）。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-bin/src/lib.rs` | 全部模型与 `todo!()` 入口（单文件脚手架） |
| 对齐原语 | `crates/bc-core` | `align::ByteAlign` 字节对齐 |

## 关键语义

- 这是 M3+ / P2–P4 范围，不在 M1–M3 的 1:1 行为目标内。
- 动手前先读 `docs/reference-parity/hex-compare.md`（参考工具行为）与
  `openspec/changes/hex-compare/`（任务清单 23 项）。
- 实现时把 `lib.rs` 的 `#![allow(unused_variables)]` 脚手架属性移除。

## 使用与验证

暂无（未实现）。验收标准见 openspec change 与 reference-parity 文档。

## 深入阅读

- `crates/bc-engine-bin/README.md`
- `docs/reference-parity/hex-compare.md`
- `openspec/changes/hex-compare/`
