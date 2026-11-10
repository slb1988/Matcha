# Wave 1B 任务组 2 报告

完成 chrome 字体、主题 token、菜单/工具栏/路径栏/状态栏/分割条/滚动条、原创程序化图标及快照场景注册基础接线。组件采用数据驱动结构，动作返回 action_id；禁用项不产生动作。主题初值为 seed：UI 12px、编辑器 12px、面板 RGB(238,238,238)、边框 RGB(170,170,170)、经典蓝 RGB(51,103,173)，后续由组 3 校准。

已知限制：egui 原生菜单键盘助记符和系统滚动条只能近似；图标为 Painter 原创通用隐喻。快照场景支持 icon-catalog/chrome-demo/text-basic/folder-basic/table-basic 注册，批处理脚本位于 tools/visual/snap-matcha.ps1。

验证：`cargo check -p bc-gui -p bcompare` 通过。基准 A/B 与正式截图需在组 3 参考资源合入后执行。合并注意：app.rs/lib.rs 与其他 worker 的最小接线可能需要手工解决冲突。
