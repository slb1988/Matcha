# 文件夹黄金与 GUI 验证工具

## 黄金采集

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/golden/gen_dir_expected.ps1 -WorkerId <本次实际 worker 标识>
cargo test -p bc-engine-dir --test golden_dir -- --nocapture
```

参数可指定 ReferenceExe / OutputRoot；默认使用已获授权的本机安装路径。
只运行目录比较、内容比较与报告命令，不运行复制/删除/同步脚本。
输入先在系统临时目录生成，随后只把小体积合成输入、manifest、XML 期望和哈希入库。

### 跨 worker 互斥与恢复

1. CreateNew 独占创建 `%TEMP%/matcha-reference-tool.lock`，写入 owner、UTC 时间与 PID。
2. 失败最多等待 10 分钟后退出，调用方稍后重试；**不删除别人/旧 worker 的锁来抢占**。
3. 持锁后、每次调用前及配置写入前确认无其他 BCompare.exe 运行。
4. Flatten 需要唯一临时命名会话：只备份/写入 BCSessions.xml，不读取或修改
   BCFileFormats.xml、其他设置或任何授权文件。
5. 命名会话放在 BCSessions 根下；Rules 必须带 Value="TDirRules"；
   本机实测 Flatten 发布值为 State/DirHandling="dirFlatten"。
6. 每次 /solo /silent 调用按句柄等待退出；超时只停止本脚本拥有的进程。
7. finally 原样恢复 BCSessions.xml 字节及原 mtime，验证逐字节相同，再清临时目录和自有锁。
   如果恢复失败/出现外部进程，保留备份和锁并报错，不能宣称恢复成功或开始下一位 worker。
8. 不将用户配置内容纳入仓库。provenance.json 只记录程序/生成器哈希、版本、时间与 case 数。

`-CaseName` 可重放单例；`-Probe` 只跑 same。
timezone-offset 是本机参考脚本拒绝的文档例子，只有显式 CaseName 才尝试；
正常套件不制造它的期望，详见语料 KNOWN_DIFFS.md。

## 真实 native 目录基准

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/golden/bench_dir.ps1
```

脚本先构建最新 GUI，再生成两侧各 100,000 个临时文件，运行：
`matcha --bench-dir <left> <right>`。
600 个计时样本之前有 30 个 warm-up；所有行 22 px，spacing 在 show_rows 之前归零，
随机跳转覆盖完整行索引，统计使用实际绘制范围。验收为 update p99 <4 ms。
完成后退出/清理临时树；可显式 KeepFixture 供人工复查，结束后请清理它。

截图来自 egui 的 native framebuffer 回调，不依赖 PrintWindow（OpenGL 常返回黑图）。
BMP 由小型无依赖写出器生成，再由系统组件转成 PNG；不会捕捉其他窗口。

## 普通两目录启动与可交互桌面复测

```powershell
powershell -File tools/golden/gui_smoke.ps1 -Action Start
powershell -File tools/golden/gui_smoke.ps1 -Action Capture -Name folder-waveC-smoke
powershell -File tools/golden/gui_smoke.ps1 -Action Close
```

只控制当前 worktree 的 matcha.exe 和脚本自己创建的临时数据；Close 只关闭该 PID。
Click 在窗口不是前台时拒绝，不把无效鼠标消息伪装为成功。
F12 请求当前 viewport 的诊断截图；该机制不尝试解锁桌面或捕获其他应用。
当前环境物理输入受限，组件/worker 回归与 native 截图为协调者批准的等价验收；
项目负责人应在可交互桌面按 worker 报告的人工清单复测。
