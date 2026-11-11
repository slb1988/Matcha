# Tasks

说明：在 master 上开发。全程约束：`cargo clippy --workspace --all-targets -- -D warnings` 零警告、`cargo test --workspace` 通过、不新增第三方依赖、引擎 crate 不依赖 GUI、不提取参考工具图标/位图资源、调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`。每个任务组至少一个 Conventional Commits 提交。

## 1. 字节对齐引擎

- [x] 1.1 实现 `bc_core::align::ByteAlign` 的 None 档（逐位对齐，长度差尾部为 orphan）；验证单测：等长差异、左长/右长截断场景的四分类与顺序保持
- [x] 1.2 实现 Complete 档（锚块骨架 + 区间字节级细化，区间长度上限降级 orphan 段）；验证单测：参考语料（8 改动 + 16 插入 + 32 删除）精确复现 4120/8/32/16 统计，5 字节奇数插入后不错位到文件尾
- [x] 1.3 实现 Fast 档（固定块长滚动哈希锚块，块间不细化）；验证单测：正确性不变量（各自恰好一次、顺序保持）在随机语料上成立，且不产出比 Complete 更精细的配对之外的错误 same
- [x] 1.4 落地 `bc_engine_bin::hex`：`HexRules` 扩展（alignment/bytes_per_line/bytes_per_group/context_count/lock_threshold）、`HexMap::align`、四分类统计；验证单测：spec `hex-byte-alignment` 全部场景
- [x] 1.5 实现 cell packing 派生行模型（每行 N 格、孤儿占位格、行标签=该侧首个真实字节偏移、整行占位标签为空）；验证单测：16 字节整行插入与 5 字节行中插入两种语料的行结构与标签相位

## 2. 加载、保存与后台 job

- [x] 2.1 实现小文件整载（< 阈值不加锁，阈值可配默认 2 MB）与大文件 mmap 只读映射（共享读锁）；验证单测：阈值两侧各一份语料的加载路径与锁语义（大文件编辑/保存被拒绝）
- [x] 2.2 实现对齐 worker（后台执行、可取消、revision 拒绝过期结果）；验证单测：编辑触发重对齐、过期结果不污染视图模型
- [x] 2.3 实现单侧保存/另存（暂存+改名落盘）、Disable editing 拒绝编辑与保存、Open Clipboard；验证单测：保存后仅该侧内容变化、禁编辑侧全部写入路径被拒

## 3. 黄金语料

- [x] 3.1 语料生成器入 `bc-core/tests/golden/hex/`（固定种子伪随机 + 程序化改动/插入/删除）；验证：`cargo test` 生成确定性语料
- [ ] 3.2 用参考工具 `hex-report`（side-by-side + line-numbers、display-all/display-mismatches）生成期望并对拍对齐与统计；验证：黄金测试通过，差异记入 `KNOWN_DIFFS.md`

## 4. 视图渲染

- [x] 4.1 实现 `bc-gui/src/hex/` 网格渲染（地址列、8+8 分组 hex 格、ASCII 列、斜纹占位格、红底差异/孤儿着色、只画可见行）；验证：egui 输入注入 + 渲染断言覆盖 spec 行打包三场景
- [x] 4.2 实现缩略图条、File Info 头、工具栏（All/Diffs/Same/Rules/Format/Copy/Next/Prev/Swap/Reload）、状态栏（结论/Insert/Load time/每侧光标地址）；验证：快照场景与参考截图逐区域对照
- [x] 4.3 实现 Side-by-Side/Over-Under、Thumbnail/File Info/Toolbar/Byte Addresses 开关、Layout 对话框（bytes per group/groups per item/fit to window/context count）；验证：RawInput 回归切换后渲染正确

## 5. 视图交互

- [x] 5.1 实现 Next/Previous Difference Bytes（Ctrl+N/Ctrl+P）与 Go To（十进制/十六进制、选侧、跨侧光标同步）；验证：RawInput 回归覆盖导航顺序与跨侧地址
- [x] 5.2 实现内联查找栏（hex/文本模式、大小写、Prev/Next）与 Replace；验证：RawInput 回归命中/不命中/替换
- [x] 5.3 实现字节编辑（Insert/Overwrite、Cut/Copy/Paste/Delete、Select All、Undo/Redo、Copy to Other Side Ctrl+R）；验证：RawInput 回归编辑→重对齐→统计更新→一次 Undo 还原
- [x] 5.4 实现 Swap Sides、Reload（脏确认）、Recompare（保留编辑）、Hex Compare Info 对话框、Show All/Differences/Same 过滤（Diffs 带 context）；验证：RawInput 回归 + 统计与语料一致

## 6. 会话设置与入口接线

- [x] 6.1 实现 Session Settings 三页（Specs/Format/Comparison，作用域至少 "Use for this view only"）；验证：RawInput 回归切策略触发重对齐、禁编辑生效
- [x] 6.2 `app.rs` 加 `View::HexCompare`，`home.rs` 加会话卡与 `*.bin` 掩码路由，接入 `/fv="Hex Compare"`，text/table 的 Compare Files Using 增加 Hex 互切；验证：路由单测 + RawInput 打开/互切回归
- [x] 6.3 空会话流程（先建视图再逐侧打开）；验证：RawInput 回归

## 7. 验收

- [x] 7.1 大文件性能：256 MB 语料在 Fast 档下对齐可取消、视图滚动流畅，记录耗时；验证：基准数字写入 worker-report
- [ ] 7.2 全量门禁：`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo build --release -p bcompare`；验证：三条命令零告警通过
- [x] 7.3 Matcha 快照与参考截图并排对照（主视图/插入区/Diffs 过滤三场景），几何与着色差异列入 worker-report；验证：对照记录入 `docs/reference-parity/hex-compare.md` 补记
