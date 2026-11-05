# ADR 201：表格来源与行配对边界

状态：本波协调者批准。

- 采用 calamine 0.38（唯一新直接第三方依赖）。与 0.30.1 不同，0.38 提供公开样式读取器，实际保留数字格式串/内置编号；字体、填充与写回仍后置。协调者批准 workspace MSRV 统一从 1.80 提高至 1.88；本机工具链 1.99.1。
- 仅值/公式/数字格式进入本波比较。StyleId 是 sheet 内部去重编号，比较时解析属性而非直接比较编号。
- 行对齐复用 core 的策略；Smart 候选仍须满足键相等且重复键 occurrence 一致，除 Unaligned 和显式 AlignmentOverrides 外不得强配不同键。按键稳定排序使用独立排列，core map 的索引仍指向排列位置。
- 日期容差内部为毫秒，提供天/小时构造。系统区域设置由宿主注入 ReadOptions；未注入时使用明确的 invariant（小数点、逗号千分位、YMD、短横线），不猜测或改动进程区域。
- 帮助主题只读解包至系统临时目录：Type/Regional/Columns/Alignment/Sheets/Handling 及命令列表；未访问任何授权文件或安装目录写操作。日期容差帮助未指明单位，因此 API 明确提供单位转换。

验证与最终限制见本波 worker report。
