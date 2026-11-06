# bc-gui

正式的 egui 0.37 文本比较视图。唯一依赖 GUI 工具包的 crate；没有 GUI 自己的 mmap、unsafe 或新增第三方依赖。底层只读锁与编码由 bc-core / bc-engine-text 负责。

## 结构

- `app.rs`：View 切换、全局 Prefs、关闭保护、基准与应用 framebuffer 截图；接线集中在 View / HomeAction / ViewAction。
- `home.rs`：路径框、两文件拖放、最近文件对（Matcha 设置目录）。
- `text/view.rs`：TextMap 控制器、差异导航、独立保存、会话命令、确认框。
- `text/render.rs` / `gutter.rs`：可见行自绘、空隙、来源行号、三态/行内/语法色、左右/上下布局、双轴同步。
- `text/editor.rs`：UTF-8 字节光标、选区、键盘、剪贴板和 IME Preedit/Commit，局部 LineEdit 与 undo/redo；不是整文件 TextEdit。
- `text/projection.rs`：区段式 All / Differences / Same / Context 索引与折叠展开。
- `text/nav_bar.rs`：512 桶缩略条、可视区/当前行及定位。
- `text/worker.rs`：一个 std worker，调用 Job::run；350 ms 防抖 Realign2Job，修订号不符的结果拒绝。
- `find_panel.rs` / `settings_panel.rs` / `status_bar.rs`：查找替换、公开策略名称与重要性规则、颜色/备份偏好、双侧状态/计数。
- `format_panel.rs`：25 内置格式只读列表、复制/新建用户格式、事务验证 XML 编辑、导入/导出/持久化。
- `bench.rs`：保持原有 `bench.*` 格式；等待真实比较完成后开始预热和计量。
- `dir_view.rs`：文件夹比较。单个 worker 独占 DirTree，GUI 只读 latest-only 不可变行快照；侧限定选择、列排序/详情、过滤、导航、比较、文件操作与局部刷新。打开文件对时 worker 只校验路径，正式 `TextCompareView` 在 UI 线程构建（其 TextContent 含 Rc/RefCell），以浮动窗口显示，关闭/退出走文本视图自身的保存确认。
- `sync_view.rs`：五种预设/自定义同步、预览动作、逐项覆盖与冲突导航；成功同步在本视图内维护基线，不推进未解决冲突。
- `dialogs/*`、`filters_bar.rs`、`legend.rs`、`log_panel.rs`：路径/操作/确认对话框、过滤预设、颜色图例和虚拟化操作日志。
- `bench_dir.rs`：文件夹视图原生窗口基准与 framebuffer 截图（`bench.dir.*`）。

## 运行与测试

```powershell
cargo run --release -p bcompare -- <left> <right>
cargo run --release -p bcompare -- <left> <right> --bench=600
cargo run --release -p bcompare -- <left> <right> --bench=60 --screenshot=target/text.bmp
cargo run --release -p bcompare -- <left-folder> <right-folder>
cargo run --release -p bcompare -- --bench-dir <left-folder> <right-folder>
cargo test -p bc-gui --lib
```

两个目录参数自动进入文件夹比较；Home 也接受拖放两个文件夹。`tools/golden/bench_dir.ps1` 构建后生成两侧各 10 万文件的临时树并验证 p99 <4 ms。过滤预设保存在 APPDATA 下 `Matcha/folder-filters.xml`（或 MATCHA_SETTINGS_DIR）。文件操作只在显式范围确认及分类确认之后执行。文件夹线决策见 [ADR 104](../../docs/decisions/104-folder-ui-and-virtual-pairs.md)。

截图来自本应用 framebuffer，BMP 写入不需要图像库，不读取其他应用或锁屏桌面。Windows 使用系统微软雅黑作为 CJK 后备字体，不复制/捆绑字体文件。

## 编辑、保存与后台任务约束

- Live TextContent 留在 UI 线程；worker 只拿 ContentSnapshot。
- 绘制不调用 align_map，不在每帧扫描全文件/统计；状态统计和缩略桶在比较完成/视图投影改变后更新。
- 保存前 cancel + join worker。区块复制可能让另一侧持有来源 mmap，因此用同一个 MaterializeContext 物化**两侧**当前/原始/撤销/重做历史，释放 context 后调用 materialize_and_save。失败不会截断原文件，原历史保留。
- 另存目标保持原来源路径；后续 Save 写入目标。仅保存一侧不会清除另一侧 dirty。无路径内容用路径框保存。
- 换编码重新解码原保存字节，有修改先确认；手动格式覆盖保留到 Recompare，Reload 重新按路径检测。
- 内置格式通过复制创建用户格式，不直接编辑内置数据；用户配置无效时给出错误，而非静默忽略。

## 验证边界与后续

18 个 GUI 回归使用真实 egui RawInput（鼠标、键盘、拖放、IME）与 tessellation，并覆盖保存/撤销/对齐/过滤；正式原生程序另跑 110 MB 基准与 GPU 截图。原生 Windows 中文输入法候选窗因锁定桌面待负责人手验，不能以提交事件测试冒称已完成原生候选窗验收。

原生文件选择器、持久化会话、打印/报告、词内差异导航、Word Wrap、书签、外部工具及独立详细面板后置。路径文本框和最近列表是本期正式打开/保存入口；Compare Parent Folders 在两侧均为已保存磁盘文件时启用，打开两侧父目录的文件夹比较。逐命令取舍见 [对照清单](../../docs/reference-parity/text-compare.md)。
