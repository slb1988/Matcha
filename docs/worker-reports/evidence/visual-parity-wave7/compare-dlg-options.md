# 视觉对照：dlg-options
参考：docs/visual-parity/reference/dlg-options.png；实测：target/visual/matcha/dlg-options.png；差异像素：122194；最大区域位移：7 px。
方法：背景色/分隔线法向色带扫描，先局部定位，再独立检测左/上/右/下四条边（±8 px）；尺寸不同单独报告。ambiguous 表示平坦色带可能有多解，需要人工看并排图；本工具不将文字抗锯齿差异作为验收阈值。
![并排](dlg-options-side.png)
![热图](dlg-options-heat.png)
|区域|参考 x,y,w,h|实测 x,y,w,h|左,上,右,下偏差|色差均值|样本数|多解|结果|
|---|---|---|---|---|---|---|---|
|buttons|0,604,859,51|-2,604,860,52|-2,0,-1,1|0.1|690|False|PASS|
|content|242,0,617,604|242,1,619,603|0,1,2,0|1.859|760|False|PASS|
|navigation|0,0,242,604|7,7,235,597|7,7,0,0|2.423|768|False|<span style="color:red">FAIL (>2 px)</span>|
## 颜色抽样（单通道容差 2）
|token|坐标|参考|实测|最大通道差|通过|
|---|---|---|---|---|---|
