# 黄金语料差异登记

当前状态：30 个合成输入已就绪，尚未生成两种策略的完整参考期望；一致率尚不可计算，不能报告为 100%。

| strategy | case | 原因与证据 |
|---|---|---|

正式回归条目数目前为 0；这不表示零差异，而是完整同设置期望尚未就绪。

## 诊断记录（不是回归白名单）

- diagnostic-myers / 011-similar-identifier：参考默认 Myers 把替换报告为 Changed，当前 Quick 输出孤立行；NeverAlignDiffs=True 会恢复参考孤立行输出。
- diagnostic-myers / 012-unrelated-replacement：同上，三行替换默认逐行打包。
- diagnostic-standard / 012-unrelated-replacement：参考默认 Standard 也把完全不相似的三行替换打包，项目明确要求不强配。

证据位于 tools/golden/evidence，均为正常脚本的原始输出。尚未核实全部重要性配置，故不能把诊断记录转成白名单或计算完整一致率；待协调者决定消息 msg_7ff4080efbd7 后生成正式期望。Myers 必须 100%，Standard 至少 95%，测试不接受部分生成的期望集合。
