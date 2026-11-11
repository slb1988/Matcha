# 视觉对照：dlg-text-session-settings
参考：docs/visual-parity/reference/dlg-text-session-settings.png；实测：target/visual/matcha/dlg-text-session-settings.png；差异像素：22113；最大区域位移：5 px。
方法：背景色/分隔线法向色带扫描，先局部定位，再独立检测左/上/右/下四条边（±8 px）；尺寸不同单独报告。ambiguous 表示平坦色带可能有多解，需要人工看并排图；本工具不将文字抗锯齿差异作为验收阈值。
![并排](dlg-text-session-settings-side.png)
![热图](dlg-text-session-settings-heat.png)
|区域|参考 x,y,w,h|实测 x,y,w,h|左,上,右,下偏差|色差均值|样本数|多解|结果|
|---|---|---|---|---|---|---|---|
|buttons|0,505,583,58|-2,506,586,58|-2,1,1,1|0.076|740|False|PASS|
|content|8,36,567,469|8,37,566,463|0,1,-1,-5|0.578|950|False|<span style="color:red">FAIL (>2 px)</span>|
|tabs|8,8,567,28|8,8,566,28|0,0,-1,0|0.324|760|False|PASS|
## 颜色抽样（单通道容差 2）
|token|坐标|参考|实测|最大通道差|通过|
|---|---|---|---|---|---|
