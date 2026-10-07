# 中文拼音输入法：开源项目考察

考察日期：2026-10-07。第一步范围：阅读源码、构建、离线运行与需求差距核对。产品平台仍未确定；本次仅使用现有 Mac 作为验证环境。

后续进展：已完成[中英输入交互原型](prototype/README.md)，运行地址为 http://127.0.0.1:9037，实际验证结果见[原型检查记录](prototype/QA.md)。它调用青简引擎，但还不是系统输入法。

最新进展：已更新并安装 [macOS 原生输入法实验版 0.2](native-macos/README.md)，新增本机整句翻译；用户已启用实验输入源。真实模型与引擎联调、预览按钮通过，真实键盘及跨应用验收仍待验证，见 [原生 QA](native-macos/QA.md)。网页原型仍使用词语释义。下面“已经实际验证”的表格与未安装说明是第一步离线研究的历史快照。

**建议优先以青简作为功能实验的基础，借鉴 rime-translate 的译词缓存方式。** 青简已经具备“拼音输入 → 中文候选及英文释义 → 通过快捷键输出英文”的主要链路。我们的开发重点可以放在中英分别选择、展开更多译法，以及帮助用户判断译法的简短说明。

## 与我们的需求对照

| 已讨论的需求 | 青简：源码确认 | rime-translate：源码确认 |
|---|---|---|
| 拼音转中文候选 | 自有拼音引擎，可离线查询 | 依赖 Rime 输入引擎 |
| 中文旁同步显示英文 | 已有本地译词与词性 | 已有候选注释 |
| 直接输出英文 | 已有译词提交接口和 Mac 快捷键处理 | 当前过滤器只改注释，候选正文仍为中文 |
| 中英都可独立选择 | 已有键盘选择链路；Mac 候选窗忽略鼠标事件 | 英文只是注释，需增加提交处理和交互 |
| 展开选择其他译法 | 数据结构最多保存两条，未发现所需展开交互 | 无所需展开交互 |
| 译法区别、简短用法 | 当前释义结构无这些字段 | 当前注释不提供相应结构 |
| 边输入边学习 | 已有译词接触记录和生词标记 | 主要是译词展示与缓存 |
| 随时切换普通/辅助翻译输入 | 已有学习语言设置，可关闭译词；具体切换体验未实机验证 | 配置层面可控制注释；具体切换体验未实机验证 |

青简的词汇记录反映接触次数，不能据此判定用户已经学会。两个项目的现状都不能直接等同于我们设想的完整产品。

## 已经实际验证的内容

| 验证项目 | 结果 | 边界与记录 |
|---|---|---|
| 青简核心、译词、词典、渲染测试 | 354 项通过 | `evidence/qingjian-tests.log`；未跑整个仓库所有平台测试 |
| 青简格式和上述模块静态检查 | 通过 | `qingjian-format.log`、`qingjian-lint.log` |
| 青简命令行程序和 Mac 输入法二进制构建 | 通过 | `qingjian-build-retry.log`；初次编译遇到 SDK 路径问题，指定本机 Command Line Tools 后通过 |
| 原版引擎查询 30 个常用词 | 30/30 目标中文词为首候选，均有译词 | `query-summary.json`、`qingjian-queries.txt`；小样本，不代表全词库准确率 |
| 三组逐键拼音查询 | 已运行 | 学习、开发、方便；`qingjian-typing.txt`；未测真实按键时延 |
| 原版候选渲染示例 | 生成 16 张预览，检查其中一张 | `candidate-previews/`；示例数据，非系统输入法现场截图 |
| rime-translate 上游 Lua 测试 | 14 项通过 | 使用 Lupa 的 Lua 5.5 和上游模拟候选；不是实际 librime/Squirrel 联调 |
| rime-translate 样例词典与本地 helper | 构建 22 个样例词条；5 项服务查询检查通过 | 为隔离运行，只替换 helper 两处目录解析；未启用云翻译 |
| 本地研究脚本 | 格式、静态检查、Python 编译检查通过 | `tools/`，仅研究工具 |

没有安装或注册系统输入源，没有修改个人输入法配置，没有接受 Xcode 许可条款。已构建的 Mac 二进制尚未经过系统注册、跨应用输入、鼠标操作和快捷键冲突测试。Windows、Linux、手机平台未实际验证。也没有测量英语学习效果或进行全面翻译质量评估。

下面是上游原版渲染器生成的候选栏示例，**不是新产品设计，也不是实际输入现场截图**：

原版候选栏示例截图属于本地生成证据，未包含在源码仓库中。

## 为什么选青简作实验基础

我们可以复用拼音解析、中文候选排序、译词查询和译词提交的现有链路，避免第一阶段同时处理输入引擎与新交互。其核心引擎与系统接入层分开，也方便先验证功能，再确定产品平台。

| 相关部分 | 可以借鉴或复用的能力 | 本项目需要补充的内容 |
|---|---|---|
| `qingjian-core` | 查询候选、注释译词、提交指定译法 | 完整译法详情的接口和状态 |
| `qingjian-dictionary` | 拼音词典查询 | 先保持原有行为 |
| `qingjian-translate` | 本地译词、个人词表层 | 可展开的释义、用法说明和经过检查的多义词数据 |
| `qingjian-learning` | 使用习惯与译词接触记录 | 学习反馈的定义；接触次数不直接视为掌握 |
| `qingjian-render` 和各平台候选窗 | 候选显示与系统接入的参考 | 中英独立选择、点击、展开、焦点管理 |

关键代码证据：

- [释义最多两条](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/crates/qingjian-core/src/candidate/translation.rs#L18)，而且[词表加载也会截断](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/crates/qingjian-translate/src/glossary/mod.rs#L61)。不能只调大显示数量就获得完整译法。
- [现有释义字段](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/crates/qingjian-core/src/candidate/sense.rs#L7)包括词性、译文、读音和生词标记，没有用法区别或例句。
- [指定译法提交接口](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/crates/qingjian-core/src/engine/commit/mod.rs#L64)已存在；它也处理拼音消耗和相关使用记录。
- [Mac 候选窗忽略鼠标事件](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/apps/macos/src/candidates/window.rs#L186)。点击中英文输出需要补充事件和命中位置处理。
- [rime-translate 过滤器](https://github.com/daocatt/rime-translate/blob/c5ffa0ed2fa2a5530a39929fc356dadd4cea161c/rime/rime_translate.lua#L345)只给候选加注释。它适合参考离线查词和异步缓存，不能直接提供英文独立选择功能。

## 译词数据与授权核对

青简当前英文表有 232,213 条词条，9,872 条带两种译法，没有超过两种的词条。表中“学习”为 study / learn，“开发”为 develop / exploit。只显示英文并不能帮助用户判断语境；例如“开发软件”和“开发资源”的选择需要说明。当前“行”被标为 `v. go / v. OK`，也提示词性和语义需要检查。上述数字来自本地词表统计，不是人工质量审查结果。

青简的[词表说明](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/assets/glossary/README.md)说明部分译词由模型生成。rime-translate 的样例反向词典把“果实”返回为 `apple|fruit`，说明反向关联结果也需要校验，不能把每项都当作可互换译文。

授权必须分别核对代码、词典数据和品牌：

- 青简 [README](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/README.md)标明代码 GPL-3.0-or-later，项目名称和 logo 不包含在代码授权中。直接修改和分发的方案应以该许可为前提；最终复用范围与发布方式尚未确定。
- 青简词表 README 标明 GPL-3.0-or-later，但 [Mac 打包脚本](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/apps/macos/scripts/bundle.sh#L124)给英文/日文元数据填写 MIT，存在不一致。再分发前需要澄清；随包其他词典还应逐项检查其[数据来源清单](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/docs/design/landscape.md)。
- rime-translate [代码许可](https://github.com/daocatt/rime-translate/blob/c5ffa0ed2fa2a5530a39929fc356dadd4cea161c/LICENSE)为 MIT；它记录的 ECDICT 数据许可为 MIT，CC-CEDICT 为 CC-BY-SA 4.0。插件代码许可不能代替词典数据的许可。

本次没有对外发布、复制品牌或形成完整的数据授权审查结论。

## 下一步建议：先验证一个最小的双语选择流程

以“学习”为例：输入 `xuexi`，显示中文“学习”和英文 study / learn；可以选择中文输出，也可以选择任一英文输出；展开后显示简短用法区别，关闭展开不丢失拼音。

先准备 30 个经过逐项检查的常用词及多种译法。紧凑候选栏保持少量常用译法，展开时再查询完整详情，避免打字过程中频繁联网或把所有释义塞进候选栏。未命中详情时继续正常中文输入。

下一阶段验收应覆盖：中英各自提交一次且无重复文字；切换模式保留拼音；展开/收起不意外提交；无译词时仍可选中文；部分词提交后剩余拼音正确保留。系统接入、跨应用兼容性与真实学习效果另行验证。

## 研究快照与复现

| 项目 | 本次固定提交 | 提交日期 |
|---|---|---|
| [青简](https://github.com/qingjian-team/qingjian) | `c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e` | 2026-09-29 |
| [rime-translate](https://github.com/daocatt/rime-translate) | `c5ffa0ed2fa2a5530a39929fc356dadd4cea161c` | 2026-09-06 |

两个上游目录在最终检查时均无源码改动。研究脚本在 `tools/`，本地隔离运行环境在 `runtime/`，测试记录与预览在 `evidence/`；运行时和生成文件已列入 `.gitignore`。精确复现命令见本目录 `AGENTS.md`。没有创建提交或远程仓库。
