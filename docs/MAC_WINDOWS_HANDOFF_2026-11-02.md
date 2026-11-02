# Mac 0.12.18–0.12.23：Windows 同步参考（2026-11-02）

## 参考状态和依赖

这是与本机 0.12.23 / build45 功能一致的源码交接，不是发布公告。固定功能快照：[`2d3d379ab0f5c67e3e3cd8170482bfc394720c46`](https://github.com/jizw0704-source/lingomate/tree/2d3d379ab0f5c67e3e3cd8170482bfc394720c46)。后续交接文档提交不改变这个功能快照。

参考已定稿，可供阅读、复用和安排实现；尚未合并 Main。当前 Main 为 `731b763`，本整合分支包含下列未合并前置 PR：

| 前置 | 内容 | 状态 |
| --- | --- | --- |
| #17 | 个人词库、借词取字 | 待合并 |
| #19 | 表情、标点、数学符号 | 待合并 |
| #22 | 模式提示、Shift可靠性 | 待合并 |
| #23 | 准确性评测与学习切分 | 待合并 |

整合草稿 PR 相对 Main 的差异因此包含这些前置内容，不能把全部差异都当作本次新增。最近更新的独立范围可用 `git diff 18a6dbec3aed81e4179cc1571e288f5c382f03de..2d3d379ab0f5c67e3e3cd8170482bfc394720c46` 查看。此PR不代替上述PR的审阅/审批，也不自动合并或关闭它们。

Wright 在 `codex/windows-dev` 上独立开发；不要直接覆盖Windows代码或为每次Mac提交建立新Windows分支。原有 #13（大写/固定表达）、#15（统一设置/翻译）、#20（符号）继续沿用，避免重复任务。

## 五项稳定功能交接

| 项目 | 最终行为 | 核心源码 | 功能提交 |
| --- | --- | --- | --- |
| 背景不透明度（0.12.18） | 默认100%，70–100%；只改变背景，文字清晰；系统减少透明度/增强对比度优先 | [CandidateOpacity.swift](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/CandidateOpacity.swift)、[设置](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/SettingsWindow.swift) | `aad84accdd090cf558c4b8f043d02836cb46bac5` |
| 搭配评分/切分/学习（0.12.19） | 有界词格及搭配评分；重复选择先保持、再衰减；不改变词库计数格式 | [ranking.rs](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/prototype/bridge/src/ranking.rs)、[learned_sentence.rs](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/prototype/bridge/src/learned_sentence.rs) | `6701fa4450167b24cecc16810008325f50e75a26` |
| 精简候选区（0.12.20–21） | 删除重复拼音及顶栏；中文/英文/POS主体；右侧44pt更多/下一页，上一页/译法/设置在菜单；保留快捷键 | [CandidatePanel.swift](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/CandidatePanel.swift) | `1c396eb1843454bd10cdaf227b039728762d7826` |
| 推荐反馈（0.12.22） | 更多→调整词语推荐→词语→优先/减少/自动；与个人词库恢复一致，不增加使用次数 | [反馈说明](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/docs/CANDIDATE_FEEDBACK.md)、[feedback.rs](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/prototype/bridge/src/feedback.rs)、[CandidateFeedback.swift](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/CandidateFeedback.swift) | `9e97e84fc6a0cbeb86d747d4f191ab025a63eeb6` |
| 当前会话同音排序（0.12.23） | 只在下一段拼音输入后重排；不主动联想；已确认中文尾部≤32字、60秒有效，边界清空；显式推荐优先 | [上下文说明](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/docs/SESSION_CONTEXT.md)、[ConfirmedContext.swift](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/ConfirmedContext.swift)、[InputController.swift](https://github.com/jizw0704-source/lingomate/blob/2d3d379ab0f5c67e3e3cd8170482bfc394720c46/native-macos/Sources/InputController.swift) | `4883730e604ba84e6d0d6c0325392499f46cf003` |

## Windows 必须显式接入的部分

- Mac CLI启用 `--mac-candidates`，共享 `Adapter::focus_candidates()` 控制该路径。现有Windows C ABI默认不启用。使用进程内DLL时，必须设计并检查显式启用方式；给EXE添加参数不能改变DLL实例。不要为实现一个功能无意启用全部Mac策略。
- 原有 `words.json` version1、`words.preferences.json` 格式保留。显式反馈新增同目录 `words.feedback.json`：version1、text/syllables/value（±1）；恢复自动为移除。与词库共用锁/原子替换，损坏文件不能重建。只有反馈的词条次数为0，管理界面显示“—”。旧客户端不认识此侧文件，不能仅复制界面后假定删除/恢复已同步。
- 管理协议增加 `feedback` action、list反馈状态；候选响应增加可选 `feedbackEligible` / `feedback`。见反馈说明和测试；显式设置不伪造上屏确认。
- 查询新增可选 `preceding`（≤32 CJK字符）；只允许当前会话成功确认的内容、本机内存。每次query替换，缺省/空输入清除，cancel/confirm和非同scope提交清除。不进入响应/日志/词库/账号同步/翻译API。Windows须按TSF会话、成功上屏、光标/焦点/英文/删除/符号/取消/安全输入边界清空；不要照搬IMK回调。
- 不透明度和候选UI是平台实现：Windows须独立处理DPI、UIA、焦点、系统对比度/透明度偏好。44pt是Mac原生尺寸参考，Windows按DIP与实际缩放验收。

## 共同验收用例

| 功能 | 操作 | 预期 | Mac实体结果 | Windows |
| --- | --- | --- | --- | --- |
| 不透明度 | 设置80%，继续输入xuexi；恢复100% | 背景变淡、文字不变淡、保存后重开有效 | 选项/候选/透明效果用户确认；更新后和系统偏好实机仍需分别验 | 待实现/验收 |
| 混合输入回归 | wo空格→AI回车→xuexi空格，不再回车 | 我AI学习，确认AI不发送 | 0.12.19已确认；新版本继续回归 | 待验收 |
| 排序/切分 | 同一套60原例及12搭配例，与前版比较 | 无排名退步、合法消费和确认后记忆 | 自动检查；不能当实体准确率统计 | 待实现/验收 |
| 精简候选区 | 输入wo/haishi，翻页、展开/收起、进入设置 | 无重复拼音/顶栏，入口正常且不丢焦点 | 0.12.20/21均用户确认 | 待实现/验收 |
| 优先/恢复 | gongshi→公示优先→恢复自动，不提交 | 排序变化后可恢复；次数不增加 | 0.12.22用户确认优先和恢复 | 待实现/验收 |
| 减少/管理 | 减少某词，个人词库查该词并恢复 | 不删除基础词；可撤销；反馈单独保存 | 自动/隔离预览通过；真实减少仍待用户验 | 待实现/验收 |
| 上下文同音 | 确认临床后输入shiyan，不确认第二词 | 试验靠前，空白时不弹联想 | 0.12.23 Codex用户确认 | 待实现/验收 |
| 上下文边界 | 临床→AI回车→shiyan；换输入框、删字、Esc/符号后再查询 | 旧上下文不影响下一处；混输和记忆仍正确 | 自动专项通过；真实跨边界/跨应用仍待验 | 待实现/验收 |

## 证据与许可

- 每项对应Mac完整构建/check.sh、格式/clippy及隔离专项通过；相关检查见 native-macos/tools/check.sh。用户实体反馈只按上表计入，不把自动检查/预览当实机。
- 0.12.19在原60例首选53→54、召回57→58；0.12.23保持54/58。12个上下文规则例首选6→9、召回12保持，是规则工程示例，不是盲测泛化准确率。
- Windows当前CI结果以草稿PR的实时检查为准；没有Windows实机证据，不勾选Windows完成。
- 未增加外部语料、模型或生产依赖。新增搭配/上下文规则和合成测试独立编写，项目GPL-3.0-or-later；既有固定许可词库及随包声明保留。见 data/COLLOCATION-NOTICE.md、docs/SESSION_CONTEXT.md。
- 没有上传个人词库、实际输入轨迹、凭据、安装收据或含宿主正文的截图。此交接仅源码和文档；不发布安装包、Release或远程更新清单。

## 参考检查命令

```sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
runtime/python/bin/python prototype/tests/candidate_feedback_test.py
runtime/python/bin/python prototype/tests/session_context_test.py
runtime/python/bin/python native-macos/tools/evaluate_candidates.py --bridge upstream/qingjian/target/release/bilingual-ime-bridge --cases native-macos/evaluation/context-cases.json --output evidence/context/current.json
```

各功能独立同步任务在草稿PR描述中索引，责任人为LightningFlashEvE；排期由Wright决定。Windows PR使用 Refs 关联，实机尚未完成时不使用Closes/Fixes关闭任务。
