# Mac 候选准确性评测

60 个独立编写的合成工程用例：常用词 30、整句 12、技术词 6、分音／简拼／纠错与部分消费边界 6、偏好衰减 4、已学词整句 2。用例按项目 GPL-3.0-or-later 提供；没有真实输入记录、宿主正文或新增外部语料。引擎仍只使用现有许可明确、固定校验值的 CC-CEDICT、Google Books Ngram 和原有独立先验。

这是一组可持续补充的回归用例，不是随机抽样或独立盲测，不能用来推断全部中文输入的准确率。同音词往往存在多个合理答案；`accepted` 代表这个测试的目标，不表示其他候选语言上错误。

## 运行

从项目根目录执行，使用项目 uv Python 环境和已构建的桥接进程：

```sh
bash native-macos/tools/build.sh
runtime/python/bin/python native-macos/tools/evaluate_candidates.py --bridge upstream/qingjian/target/release/bilingual-ime-bridge --output evidence/accuracy/current.json
runtime/python/bin/python native-macos/tools/evaluate_candidates.py --bridge upstream/qingjian/target/release/bilingual-ime-bridge --baseline evidence/accuracy/baseline.json --output evidence/accuracy/current.json
runtime/python/bin/python native-macos/tools/test_evaluate_candidates.py
runtime/python/bin/python prototype/tests/mac_accuracy_test.py
```

先用待比较版本保存基线可执行文件及报告，再用新版本执行第二条比较命令。报告保存在忽略的 evidence/ 中，不提交可执行文件或真实词库。比较时必须使用同一数据资源及完全相同的用例，包括学习次数和间隔；报告校验 fixtures SHA-256、用例 ID、输入、类别和目标，任一候选排名退步都以非零状态退出。未命中记作 65（候选上限 64），不能通过修改目标来掩盖退步。

每个用例建立独立临时 version-1 词库。学习场景的 count 和 age 是合成状态；查询、重复查询和取消不得改变该文件。另有真实桥接回归：提交、取得回执、确认两次、模拟 300 次后续确认、重启、整句提交、再次确认和重启召回；覆盖只查询、取消、未确认、一次误选和 legacy 默认策略。

报告给出首选／前五／所有候选命中数、逐条结果、冷进程首查询耗时，以及一次预热后五次查询的 P95（nearest rank）。冷时间包含启动／加载／首次索引，不是单次按键延迟；热时间也只代表本机桥接查询，不能替代完整 IMK 或大个人词库的实测。

## 本轮结果（2026-10-23）

基线为 Main `731b763`，在同一 Mac、相同数据和相同 60 个用例上先运行基线，再运行优化版本：

| 指标 | 基线 | 优化 |
| --- | ---: | ---: |
| 首选命中 | 50/60 | 53/60 |
| 前五命中 | 57/60 | 57/60 |
| 所有候选命中 | 57/60 | 57/60 |
| 偏好保持首选 | 2/4 | 4/4 |
| 已学词整句首选 | 0/2 | 1/2 |
| 热查询 P95 | 1.128 ms | 1.142 ms |
| 冷进程首查询 P95 | 121.469 ms | 171.119 ms |

逐条排名没有退步。热查询接近，首次使用学习整句索引有额外初始化成本（本轮两条约 180 ms，含整个进程启动），不能宣称性能提升。Windows 自动检查与实体键盘体验分别验收。

仍未召回目标：“明天我们一起打乒乓球”“我们需要新的方法”“我们需要公示”（已学“公示”）。仍有同音歧义目标未排第一，例如事实、先、电导。保留这些失败项，不更改 accepted、不增加针对这些句子的硬编码词条。

## 算法边界

只在 `--mac-candidates` 下启用；Windows C ABI、legacy CLI 和网页默认路径不变。重复确认（至少两次）的同读音偏好采用缓慢衰减；一次误选保留原来的衰减规则。排序仍限制在相同读音和消费长度组内。

整句路径只用字典中精确匹配的完整音节和已确认词，不读取前文、不联网、不引入语言模型。最多考察四条完整分音，每条 3–32 音节、每词最多 8 音节、字典同跨度最多六个基础词；用动态规划比较路径。只有重复确认词实际进入路径时才补充或前移候选，没有该证据时保留原引擎结果。已有词典／个人整词不转成合成句；保留普通词、单字、分段候选和 64 项上限。所有新候选仍由适配层重新生成并校验，再交真实引擎消费拼音；成功上屏确认前不记忆。
