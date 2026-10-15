# 常用数学符号覆盖清单

Mac 0.12.16 / build 38：14 类、171 项、337 个完整关键词／别称。

本表由项目独立整理的 data/math-symbols.json 再生成，按项目 GPL-3.0-or-later 许可提供；不包含第三方字体或表情图像。
文字字符与名称参考核对：[NIST 常用数学记号](https://dlmf.nist.gov/front/introduction)、[Unicode 数学字符分类](https://www.unicode.org/Public/math/revision-15/MathClassEx-15.html)。没有从这些页面批量导入语料或图片。

只在中文模式按完整关键词匹配；普通候选的前两项及相对顺序保留，再补充最多三项符号。回车仍确认英文字母，Shift 英文直输不匹配。选择的符号按原样输出，不写入中文词库或英文学习记录。

表内括号对直接插入两个括号，光标留在末尾，不自动移入括号；需要分别输入时可用左／右括号关键词。符号是纯文本，不是公式编辑器；上下标和分数字符仅覆盖表内常用形式。数学记号在不同领域可能有约定差异，按候选主动选择。

测试要求涵盖所有别称、重复／冲突、每项≤3个输出、必备基础符号、0–9上下标、原候选排序、首屏索引、分页、失效选择、开关及记忆隔离；真实键盘验收另记。

## 四则与运算

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 加号 | jia / jiahao / jiafa / xiangjia / zhenghao / plus / jiashang | `+`、`＋` |
| 减号 | jian / jianhao / jianfa / jianqu / fuhao / minus / xiangjian | `−`、`-`、`－` |
| 乘号 | cheng / chenghao / chengfa / chengyi / multiply / times / xiangcheng | `×`、`*`、`·` |
| 除号 | chu / chuhao / chufa / chuyi / divide / xiangchu | `÷`、`/`、`∕` |
| 正负号 | zhengfu / zhengfuhao | `±` |
| 负正号 | fuzheng / fuzhenghao | `∓` |
| 点乘 | diancheng / dianchenghao | `·`、`⋅` |
| 叉乘 | chacheng / chachenghao | `×` |
| 阶乘 | jiecheng / jiechenghao | `!` |
| 取模 | qumo / quyu / moyunsuan | `mod`、`%` |
| 复合运算 | fuheyunsuan / fuhehao | `∘` |
| 增量 | zengliang / bianhualiang | `Δ` |
| 张量积 | zhangliangji / keluoneikeji | `⊗` |
| 直和 | zhihe / zhihehao | `⊕` |

## 比较与关系

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 等于 | deng / dengyu / denghao / equal / equals | `=`、`＝` |
| 不等于 | budeng / budengyu / budenghao / budengyuhao | `≠` |
| 大于 | dayu / dayuhao / greaterthan | `>`、`＞` |
| 小于 | xiaoyu / xiaoyuhao / lessthan | `<`、`＜` |
| 大于等于 | dayudengyu / dayuhuodengyu / buxiaoyu / dayudengyuhao | `≥` |
| 小于等于 | xiaoyudengyu / xiaoyuhuodengyu / budayu / xiaoyudengyuhao | `≤` |
| 约等于 | yuedeng / yuedengyu / yuedenghao / jinsidengyu / jinsihao / yuedengyuhao / dayue / yuedengyufuhao | `≈` |
| 恒等于 | hengdeng / hengdengyu / hengdenghao / hengdengyuhao | `≡` |
| 不恒等于 | buhengdengyu | `≢` |
| 远大于 | yuandayu / yuandayuhao | `≫` |
| 远小于 | yuanxiaoyu / yuanxiaoyuhao | `≪` |
| 正比 | zhengbi / zhengbihao / chengzhengbi | `∝` |
| 定义等于 | dingyidengyu / dingyiwei | `≔`、`≝` |
| 渐近等于 | jianjindengyu | `≃` |
| 不约等于 | buyuedengyu / buyuedenghao | `≉` |

## 乘方与根式

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 乘方 | cifang / chengfang | `^` |
| 平方 | pingfang / ercifang | `²` |
| 立方 | lifang / sancifang | `³` |
| 平方根 | genhao / gouhao / kaifang / pingfanggen / sqrt / kaigenhao / ercifanggen | `√` |
| 立方根 | lifanggen / kailifang | `∛` |
| 四次方根 | sicifanggen | `∜` |
| 指数 | ncifang / ndecifang | `ⁿ` |

## 比例与百分数

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 百分号 | baifen / baifenhao / baifenbi / percent | `%` |
| 千分号 | qianfen / qianfenhao / qianfenbi / permille | `‰` |
| 万分号 | wanfen / wanfenhao / wanfenbi | `‱` |
| 比例号 | bilihao / bihao | `∶`、`:` |

## 常用分数

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 二分之一 | erfenzhiyi / yiban | `½` |
| 三分之一 | sanfenzhiyi | `⅓` |
| 三分之二 | sanfenzhier | `⅔` |
| 四分之一 | sifenzhiyi | `¼` |
| 四分之三 | sifenzhisan | `¾` |
| 八分之一 | bafenzhiyi | `⅛` |
| 八分之三 | bafenzhisan | `⅜` |
| 八分之五 | bafenzhiwu | `⅝` |
| 八分之七 | bafenzhiqi | `⅞` |

## 微积分与无穷

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 无穷大 | wuqiong / wuqiongda / wuqiongdahao / wuxian / wuxianda / infinity | `∞` |
| 求和 | qiuhe / qiuhehao / zonghehao / sum | `∑` |
| 连乘 | liancheng / lianchenghao / qiuji | `∏` |
| 积分 | jifen / jifenhao / integral | `∫` |
| 二重积分 | erchongjifen | `∬` |
| 三重积分 | sanchongjifen / tijifen | `∭` |
| 围道积分 | weidaojifen / biquxianjifen | `∮` |
| 闭曲面积分 | biqumianjifen / fengbimianjifen | `∯` |
| 体积分记号 | tijifenhao | `∰` |
| 偏导 | piandao / piandaoshu / pianweifen | `∂` |
| 微分 | weifen / weifenhao | `d`、`ⅆ` |
| 梯度 | tidu / tidufu / nabla / daosanjiao | `∇` |
| 一阶导记号 | daoshu / yijiedaoshu | `′` |
| 二阶导记号 | erjiedaoshu | `″` |
| 三阶导记号 | sanjiedaoshu | `‴` |
| 曲线积分 | quxianjifen | `∫` |
| 面积分 | mianjifen | `∬` |

## 集合与数系

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 属于 | shuyu / shuyuhao | `∈` |
| 不属于 | bushuyu / bushuyuhao | `∉` |
| 包含元素 | baohanyuansu / hanyou | `∋` |
| 空集 | kongji / kongjihao | `∅` |
| 子集 | ziji / zijihao | `⊆` |
| 真子集 | zhenziji / zhenzijihao | `⊊`、`⊂` |
| 超集 | chaoji / baohan | `⊇` |
| 真超集 | zhenchaoji / zhenbaohan | `⊋`、`⊃` |
| 并集 | bingji / bingjihao | `∪` |
| 交集 | jiaoji / jiaojihao | `∩` |
| 差集 | chaji / chajihao | `∖`、`\` |
| 补集 | buji / bujihao | `∁` |
| 自然数集 | ziranshu / ziranshuji / ziranshujihe | `ℕ` |
| 整数集 | zhengshu / zhengshuji / zhengshujihe | `ℤ` |
| 有理数集 | youlishu / youlishuji | `ℚ` |
| 实数集 | shishu / shishuji / shishujihe | `ℝ` |
| 复数集 | fushu / fushuji / fushujihe | `ℂ` |

## 逻辑与推理

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 任意 | renyi / renyihao / quanchengliangci | `∀` |
| 存在 | cunzai / cunzaihao | `∃` |
| 不存在 | bucunzai | `∄` |
| 逻辑非 | luojifei / fouding | `¬` |
| 逻辑与 | luojiyu / hequ | `∧` |
| 逻辑或 | luojihuo / xiqu | `∨` |
| 异或 | yihuo / yihuohao | `⊕` |
| 推出 | tuichu / yunhan | `⇒` |
| 等价 | dengjia / dengjiahao / dangqiejindang | `⇔` |
| 因为 | yinwei | `∵` |
| 所以 | suoyi | `∴` |
| 唯一存在 | weiyicunzai | `∃!` |

## 几何与角度

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 角 | jiao / jiaohao | `∠` |
| 直角 | zhijiao / zhijiaohao | `∟` |
| 角度 | jiaodu / du / dushu | `°` |
| 角分 | jiaofen | `′` |
| 角秒 | jiaomiao | `″` |
| 垂直 | chuizhi / chuizhihao | `⊥` |
| 平行 | pingxing / pingxinghao | `∥` |
| 不平行 | bupingxing | `∦` |
| 三角形 | sanjiaoxing | `△` |
| 圆 | yuan / yuanxing | `○` |
| 圆周率 | yuanzhoulv | `π` |
| 相似 | xiangsi / xiangsihao | `∽`、`~` |
| 全等 | quandeng / quandenghao | `≌`、`≅` |

## 统计与概率

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 总体均值 | zongtijunzhi / zongtipingjunzhi | `μ` |
| 样本均值 | yangbenjunzhi / yangbenpingjunzhi / pingjunzhi | `x̄` |
| 标准差 | biaozhuncha | `σ` |
| 方差 | fangcha | `σ²`、`s²` |
| 样本方差 | yangbenfangcha | `s²` |
| 总体方差 | zongtifangcha | `σ²` |
| 概率 | gailv | `P` |
| 数学期望 | shuxueqiwang / qiwangzhi / qiwang | `E` |
| 相关系数 | xiangguanxishu | `r`、`ρ` |

## 括号与定界

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 圆括号 | kuohao / yuankuohao / xiaokuohao | `（）`、`()` |
| 左圆括号 | zuokuohao / zuoyuankuohao | `（`、`(` |
| 右圆括号 | youkuohao / youyuankuohao | `）`、`)` |
| 方括号 | fangkuohao / zhongkuohao | `[]`、`［］` |
| 左方括号 | zuofangkuohao | `[`、`［` |
| 右方括号 | youfangkuohao | `]`、`］` |
| 花括号 | huakuohao / dakuohao | `{}`、`｛｝` |
| 左花括号 | zuohuakuohao | `{`、`｛` |
| 右花括号 | youhuakuohao | `}`、`｝` |
| 绝对值界符 | jueduizhi / jueduizhihao | `&#124;`、`｜` |
| 范数界符 | fanshu / fanshuhao | `‖` |
| 向上取整 | xiangshangquzheng | `⌈⌉` |
| 向下取整 | xiangxiaquzheng | `⌊⌋` |
| 尖括号 | jiankuohao | `⟨⟩` |
| 左尖括号 | zuojiankuohao | `⟨` |
| 右尖括号 | youjiankuohao | `⟩` |

## 数学省略记号

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 横向省略 | hengxiangshenglue / shulieshenglue | `⋯` |
| 纵向省略 | zongxiangshenglue / shuxiangshenglue / shuzhishengluehao | `⋮` |
| 斜向省略 | xiexiangshenglue | `⋱`、`⋰` |

## 上下标

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 上标零 | shangbiaoling | `⁰` |
| 上标一 | shangbiaoyi | `¹` |
| 上标二 | shangbiaoer | `²` |
| 上标三 | shangbiaosan | `³` |
| 上标四 | shangbiaosi | `⁴` |
| 上标五 | shangbiaowu | `⁵` |
| 上标六 | shangbiaoliu | `⁶` |
| 上标七 | shangbiaoqi | `⁷` |
| 上标八 | shangbiaoba | `⁸` |
| 上标九 | shangbiaojiu | `⁹` |
| 下标零 | xiabiaoling | `₀` |
| 下标一 | xiabiaoyi | `₁` |
| 下标二 | xiabiaoer | `₂` |
| 下标三 | xiabiaosan | `₃` |
| 下标四 | xiabiaosi | `₄` |
| 下标五 | xiabiaowu | `₅` |
| 下标六 | xiabiaoliu | `₆` |
| 下标七 | xiabiaoqi | `₇` |
| 下标八 | xiabiaoba | `₈` |
| 下标九 | xiabiaojiu | `₉` |

## 常用函数记号

| 名称 | 完整拼音／别称 | 候选 |
| --- | --- | --- |
| 对数 | duishu | `log` |
| 自然对数 | ziranduishu | `ln` |
| 常用对数 | changyongduishu | `lg` |
| 指数函数 | zhishuhanshu | `exp` |
| 极限 | jixian | `lim` |
| 正弦 | zhengxian | `sin` |
| 余弦 | yuxian | `cos` |
| 正切 | zhengqie | `tan` |
| 余切 | yuqie | `cot` |
| 正割 | zhengge | `sec` |
| 余割 | yuge | `csc` |
| 最大值 | zuidazhi | `max` |
| 最小值 | zuixiaozhi | `min` |
| 最大公约数 | zuidagongyueshu | `gcd` |
| 最小公倍数 | zuixiaogongbeishu | `lcm` |
