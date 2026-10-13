# 灵果 · Mac 新功能交接 · 2026-10-13

供 Wright 开发 Windows 版本时参考。本机已安装 **Mac 0.12.11 / build 33**：保留本机 0.12.10 的候选筛选与词频排序，增加英文片段首字母大写和固定表达联想。安装和实体键盘结果在 [安装记录](../native-macos/INSTALLATION.md)分别记录；本文件不表示 Windows 已实现，也不表示代码已进入 GitHub 主分支。

## 参考实现与范围

| 功能 | 参考提交 | 本次整合 |
| --- | --- | --- |
| 候选筛选与排序 | 主分支 `a03b3843963f51ca920b21b58360f1f0d78b0baa`，PR #11 | 保留 0.12.10 行为，参与下列两项功能的兼容检查 |
| 英文片段首字母大写 | 本地 `d10dba2` | 默认开启；设置可关闭并保存 |
| 固定表达联想 | 本地 `02b59df` | 新目录及确认后记忆；整合时移入最新共享 Adapter |

完整开发历史在 `codex/macos-fixed-expressions`，尚未推送。本地提交只能在同一仓库读取；Wright 从 GitHub 拉取前，应先按 [协作流程](../CONTRIBUTING.md#mac-新功能同步到-windows)提交、审核并合并 Mac PR，再建立每项 Windows 同步任务。本文件可作为任务正文依据，不能用未上传的本地提交链接替代可访问的参考。

## 英文片段首字母大写

### 用户行为

中文模式下，回车确认字母或选择英文候选时，新的英文片段只将第一个全小写单词的首字母大写。例如“我”之后确认 `hello`、继续确认 `world`，得到“我Hello world”（空格需用户输入）。连续英文中的其他词保持小写；中文、句末标点及换行可开启下一个片段。空格、逗号、冒号和普通括号不让后续每个词重复大写。

这项规则针对中英混输的英文片段；中文之后即使没有句号，也视为新片段。它不是每个单词首字母大写，也不扫描宿主前文判断句首。只观察当前输入会话已确认的本地输出；输入框／输入源切换时重置，不保证读取或补齐此前已存在于文稿中的英文状态。

已有大小写如 `AI`、`iPhone`、`eBay`、`ChatGPT` 保留；网址、邮箱、带下划线及路径等字面量保留。单按 Shift 进入英文直输时，仍将按键交给宿主，本功能不自动改写该模式的文字。回车在有组合时只确认英文，不同时发送；组合为空时仍交回宿主。原样字母不计入单词学习记录。

### 默认值与设置

默认开启。入口为灵果设置的输入页“英文片段首字母大写”；关闭后保持原样，偏好跨重启保存。切换开关不提交或取消当前拼音；设置保持滚动位置。字号和词性设置不受影响。

### 参考文件

- [EnglishFragments.swift](../native-macos/Sources/EnglishFragments.swift)：纯格式规则、片段状态与偏好键 `capitalizeEnglishFragments`。
- [InputController.swift](../native-macos/Sources/InputController.swift)：确认译词、整句英文和回车字母时的输出格式化；原候选先经过引擎验证，格式化不改写桥接候选或学习元数据。
- [SettingsInput.swift](../native-macos/Sources/SettingsInput.swift)、[SettingsWindow.swift](../native-macos/Sources/SettingsWindow.swift)、[InputDiagnostics.swift](../native-macos/Sources/InputDiagnostics.swift)：开关、白名单及状态回读。
- [EnglishFragmentTests.swift](../native-macos/Sources/EnglishFragmentTests.swift)、[SettingsTests.swift](../native-macos/Sources/SettingsTests.swift)：大小写、边界、关闭、偏好重读及设置检查。

Windows 应在自己的原生提交成功路径上采用同样规则，使用独立持久偏好；不得先改写桥接请求中的译词而绕过原词校验，也不得读取宿主聊天内容或剪贴板作为上下文。

## 固定表达联想

### 用户行为

在中文模式输入 `chiputao`，普通候选之后可选择“吃葡萄不吐葡萄皮”，一次提交整句并清空这段拼音。至少两个完整音节触发，例如 `chipu` 也可召回；`chi'pu'tao` 支持分音。单音节 `chi`、未完成 `chiput`、简拼 `cpt` 或额外尾随 `chiputaoxuexi` 不触发这条表达。

通常保留前两项普通候选，再显示最多三条不同的完整表达；普通候选不足两项时，紧随实际结果。总量不超过 64，页数按实际数量计算。空格仍确认当前普通首选，不自动把联想整句写入正文；用户用候选按钮、数字或方向键选择需要的完整表达。已确认输出中文后不另行读取宿主文字并自动续写。

确认成功后，开启选词记忆时保存完整词条、真实读音及本次输入前缀。重启可从前缀和完整拼音召回；同一前缀的联想区按既有次数／次序加权，保留普通候选位置。仅查询、浏览、取消或暂停记忆不会写入新记录；过期、重放、伪造候选／读音／译法被拒绝。

此功能补全中文固定表达，不新增编造英文、额外翻译 API 或后台网络请求。英文候选继续由既有释义和显式选择的翻译方式提供，不承诺每条固定表达都有自然英文译文。

### 数据与接入

首版 5,427 条：固定 CC-CEDICT 的 5,426 条 idiom 词条，加上固定维基百科修订中的上述绕口令第一分句。限 4–16 个汉字、匹配完整读音；沿用既有许可词频和权重，补充分句的权重 1 为基线值，不是语料频率。数据按 CC BY-SA 4.0 提供，保留署名、固定链接、改编说明和完整许可，详见 [第三方说明](THIRD_PARTY.md#固定表达联想数据)。

| 项目 | 文件或要求 |
| --- | --- |
| 固定来源 | `data/source.json`、`data/expressions-source.json` |
| 构建工具 | `tools/prepare_expressions.py`；原始来源摘要或许可不符合时拒绝构建 |
| 新增包资源 | `fixed-expressions.json`、`EXPRESSIONS-NOTICE.md`，必须同时分发；生成数据和原始缓存不进 Git |
| 共享算法 | `prototype/bridge/src/expressions.rs`；纯本地前缀匹配和已确认记忆排序 |
| 最新共享调用 | `prototype/bridge/src/lib.rs` 的 `Adapter::enable_fixed_expressions(root)` |
| Mac 启用 | `EngineClient.swift` 同时传入 `--mac-candidates`、`--fixed-expressions`；默认 CLI 不启用两项策略 |
| 接口兼容 | 请求／响应、个人词库 JSON 和 C ABI 的既有字段不变；默认 C ABI 不自动加载新资源或启用联想 |
| Windows 接入 | 先生成并分发两份资源；若使用进程内 C ABI，需设计显式启用入口并验证旧调用兼容，不能仅给独立 EXE 增加参数便视为 DLL 已启用 |
| 回执与记忆 | 原生输出成功后才确认 context-bound receipt；取消、故障或旧引擎回执不能持久化 |

## 保留的候选筛选与排序

Mac 0.12.10 的完整字词／同音词排序、简拼、未完成音节、分段词和必要单字保留。筛选限制弱相关补全与整句拼接；同一读音和消费范围内采用现有许可词频及个人记忆。固定表达候选在普通筛选完成后插入，不被弱补全筛选删除，不挤掉前两项普通候选。普通部分提交仍保留未消费拼音，例如 `kaifazhe` 选“开发”后保留 `zhe`。

参考 `prototype/bridge/src/focused_candidates.rs`、`ranking.rs`、`lib.rs` 与 `prototype/bridge/tests/focused_candidates.rs`。Windows 候选策略仍需单独启用、对照和实机验收；本轮不改变 Windows 默认行为。

## 共同验收用例

除非另有注明，下面用例在中文模式、空白草稿中执行；聊天应用完成后不追加回车发送。自动检查和预览通过不替代实体键盘验收。

| 用例 | 操作与预期结果 | Mac 状态 | Windows 状态 |
| --- | --- | --- | --- |
| 中文夹英文 | `wo`＋空格 → `hello`＋回车，应得到“我Hello”；确认英文时未发送 | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 连续英文 | 新会话确认 `hello world` → “Hello world”；逗号后再次确认 `world` 仍小写 | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 新片段 | 中文、句号或换行后确认 `hello` → “Hello” | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 已有大小写及字面量 | `AI`、`iPhone`、`https://example.invalid` 保持原样 | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 开关与偏好 | 关闭大写开关后 `hello` 原样；重启仍关闭，候选和滚动位置保持 | 隔离及原生预览通过；新版服务待验证 | 待实现／实机验证 |
| 英文直输 | 单按 Shift 进入英文模式，`hello` 保持小写 | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 固定表达前缀 | `chiputao`／`chi'pu'tao` 可选择完整“吃葡萄不吐葡萄皮”，提交后拼音为空 | 真实桥接及原生隔离检查通过；新版实机待验证 | 待实现／实机验证 |
| 普通候选与分页 | 第一、二项普通候选保持；最多三条完整联想，总量≤64；按实际数量分页 | 双策略组合回归通过；新版实机待验证 | 待实现／实机验证 |
| 不自动补全 | 查询后取消或按普通首选，不自动插入整句；`chi`／`chiput`／`cpt`／额外尾随不触发上述示例 | 自动检查通过；新版实机待验证 | 待实现／实机验证 |
| 记忆与重启 | 选中整句并成功确认后重启，可用前缀和完整拼音召回；关闭记忆或取消不保存 | 临时词库回归通过；真实个人词库待验证 | 待实现／实机验证 |
| 校验与回执 | 伪造文本、读音或译文不能提交，取消后的回执、重复回执不写入 | 自动检查通过 | 待接入相同约束 |
| 原功能保留 | `xuexi` 学习候选、`AI` 回车、`kaifazhe` 部分提交、分页和词性不退化 | 最新完整 Mac 检查通过；新版实机待验证 | 现有兼容检查通过；新增功能待验证 |

## 验证、交付与待办

本机 Apple Silicon 构建面向 macOS 13+；早期系统兼容、真实跨应用、睡眠唤醒、长期使用及 Windows 11 设备继续单独验收。开发检查使用临时词库和偏好，没有读取真实聊天正文、个人词库或凭据。

在仓库根目录执行 `bash native-macos/tools/build.sh`、`bash native-macos/tools/check.sh`。新数据的检查为 `runtime/python/bin/python tools/test_prepare_expressions.py`；两项候选策略组合、回执及重启回归为 `runtime/python/bin/python prototype/tests/expressions_test.py`。原生测试入口为 `--english-fragment-test`、`--fixed-expression-test`，独立预览为 `--preview-mixed`、`--preview-expressions`。

本次未自动创建或分配 GitHub 任务，没有给 Wright 发送消息；远程上传、PR 合并、Release 草稿及更新清单分别处理。建议后续依次提交 Mac 参考 PR，再为“英文片段首字母大写”“固定表达联想”各建一项 Windows 同步任务，引用本表用例。候选筛选参考现有 PR #11，避免复制已有任务。
