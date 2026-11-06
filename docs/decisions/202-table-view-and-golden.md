# ADR 202：表格视图、后台编辑与黄金边界

状态：实施中；以下范围已获协调者批准。

- GUI 增加对 bc-engine-table 的内部依赖；不引入第三方依赖。
- 会话持久化统一后置到 session-and-cli，不发明临时专用会话文件；Save Session / Save Session As 保留明确禁用菜单。内存 Session Settings 本波实现。
- Compare Files Using 可切到文本；Compare Parent Folders 在文件夹线接入前禁用。文件选择/保存沿用路径框和最近列表，不引入原生对话框依赖。
- 真实桌面可能锁定：使用 egui RawInput、真实 tessellation、本应用 framebuffer 截图，不尝试解锁；物理鼠标和原生 IME 单列人工验收。
- 引擎默认同步便捷 API 不变；GUI 显式启用 deferred edits，EditItem 仅应用内容/规则/锚点并使结果过期，350 ms 后以 std 线程调用 TableRealignJob::run。旧修订结果拒绝，陈旧期间绘制安全的暂态行号，不在 update 中重对齐。

黄金必须持共享参考工具锁；只操作指定会话/格式 XML，备份、finally 恢复与字节核对；不访问授权文件、不写安装目录。
