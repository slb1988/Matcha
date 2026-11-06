# 文件夹比较黄金语料

28 个小型合成目录对，由仓库的 tools/golden/gen_dir_expected.ps1 在系统临时目录生成，
使用本机安装的参考程序脚本模式实际产出。没有复制用户项目文件、授权文件或安装资源。

每个 case：
- left/、right/：原始小体积输入（允许一侧目录没有文件）。
- case.xml：比较准则、过滤、Flatten 标记、逐文件 UTC mtime 与 DOS 属性。
- expected.xml：真实 folder-report XML；仅把基准绝对路径替换为 LEFT_ROOT / RIGHT_ROOT。
- reference.sha256：规范化前的原报告 SHA-256。

Git 不转换输入/报告行尾；测试重新在系统临时目录物化文件，并恢复 manifest 的时间/属性，
不依赖 checkout 的文件时间或属性。当前语料没有网络、junction 或需要额外系统权限的输入。

运行：
```powershell
cargo test -p bc-engine-dir --test golden_dir -- --nocapture
```

解析器在 bc-engine-dir/tests/golden_dir.rs，是有限报告方言解析，不允许 DTD/外部实体；
文件夹结构与文件状态分别核对。输入缺失时可以跳过；非空语料必须至少 20 个 case。
既有语料实际为 28 case、33 个文件配对行、4 个文件夹结构行。

结果口径：
- 总体：33 / 33 = 100%（任务 8.2 接通规则比较后）。
- 已批准例外：0；Wave C 时的 1 行规则比较 Unsupported 已解决，见 KNOWN_DIFFS.md。
- 目录结构：4 / 4 配对一致，不将其冒计为含 status 的文件行。

再生成会调用外部参考程序；请阅读 tools/golden/README.md 的互斥锁与配置恢复约束。
正常 cargo test 只读入库语料，不启动参考程序，不改用户配置。
