# 灵果 · LingoMate · macOS 使用指南

当前版本：**0.11.8 / build 20**。原生客户端基于 Swift、AppKit 与 InputMethodKit，使用本地 Rust 引擎生成中文候选，并将选中的中文或英文提交至当前应用。

当前构建面向 Apple Silicon Mac。基础中文输入与词语释义的部署目标为 macOS 13；较早系统的实际运行兼容性尚未验证。Apple 本机整句翻译需要 macOS 26 及以上和中英文语言资源。MiniMax 在线模式需要网络及有效 API 配置。

## 开始使用

安装后，在 macOS 输入源菜单中选择“灵果”，点击应用的文本输入框并输入拼音。默认使用中文拼音与中英候选。输入源采用单色“果”字图标，构建时生成普通与选中状态的 1× / 2× 资源。

例如输入 `xuexi`，可选择“学习”或对应英文译法；输入 `wojintianxiangxuexiyingyu`，可对当前较长中文候选进行整句翻译。译文是否符合语境取决于中文候选及翻译结果，输入法不自动纠正同音错字。

| 操作 | 功能 |
| --- | --- |
| 单按并松开 Shift | 切换中文拼音 / 英文直输 |
| 空格 / 数字 1–5 | 确认当前页中文候选 |
| Shift＋空格 | 确认当前词语译法或已完成的整句译文 |
| ↑ / ↓ | 切换当前页候选 |
| PageUp / PageDown，`-` / `=`，或翻页按钮 | 翻页并选中该页首项 |
| Tab / Shift＋Tab | 展开词语译法，选择下一种 / 上一种 |
| 点击中文、英文或顶部译法入口 | 选择候选或查看其他译法与用法 |
| Esc | 先收起译法，再取消组合输入和翻译任务 |
| Enter / 小键盘 Enter | 有组合输入时提交原样字母并保留大小写；无组合时由当前应用处理 |
| Control＋Shift＋P | 切换自动 / 固定中文 / 固定英文标点 |
| F6 | 切换普通中文 / 中英候选，保留拼音 |

一次组合输入最多 240 个拼音字符。切换应用、输入源或将快捷键交给当前应用时，未完成拼音按原样提交。安全输入开启时不处理按键。继续输入、切换候选、取消或提交会使旧翻译任务失效。

## 中文夹英文

在中文模式中，输入 `wo`＋空格、“AI”＋回车、`xuexi`＋空格，可连续得到“我AI学习”。回车仅确认当前字母组合，随后可继续拼音输入；该次回车不会同时换行或发送消息。无组合输入时，回车恢复由当前应用处理。

此方式保留当前组合中的英文字母大小写和拼音分隔符。数字 1–5 仍用于选择候选，不进行整段混合文本自动识别。原样英文不加入选词记忆或单词学习记录。连续输入较长英文内容时，可使用英文直输。

## 英文直输

单按左或右 Shift 并在 0.7 秒内松开，可切换英文直输；再次单按恢复中文拼音。字母、大小写、数字、空格与半角标点由当前应用处理。F6、Control＋Shift＋P 等按键在英文直输时同样交给当前应用。

Shift 与字母、空格、Tab 或其他修饰键组合时不触发模式切换。长按、同时按两个 Shift、焦点变化或鼠标操作会取消待定切换。切换前存在未确认拼音时，字母按原样提交，旧翻译及待定组词取消。

输入模式在本次服务运行期间跨应用保留，重启服务后默认中文。中文模式恢复既有标点设置，自动标点从中文状态开始。应用自身的智能引号或自动更正可能影响最终显示。

## 候选与译法

每页显示 5 项，最多返回 64 个候选，实际数量取决于词典和个人记忆。数字 1–5 对应当前页；翻页清除前一候选的展开译法与整句译文，首末页不循环。没有专用翻页键时，可使用 Fn＋↑ / ↓ 或 `-` / `=`。

当前页中文候选均不超过 8 字时，采用横向排列，主要英文位于对应中文下方。较长中文采用完整换行列表，整句译文紧随当前候选；超高内容可滚动。短词窗口按内容调整宽度，普通布局上限 560 pt，窄幅预览上限 440 pt。

顶部显示拼音、模式、页码、翻页、译法及设置入口。较长拼音或英文的行内显示可能省略，完整内容可通过辅助标签、工具提示或展开译法查看。记忆词不显示额外标记。英文译文未完成时，英文确认入口禁用，中文仍可选择。

## 中英文标点

默认标点跟随当前会话最近确认输出的语言。中文确认后使用 `，。！？：；`、中文括号及成对引号；确认英文词语、整句译文或原样字母后使用对应半角符号。

例如，确认“你好”后按逗号得到“你好，”；确认 `learn` 后按逗号得到 `learn,`。自动模式中，存在拼音时按标点会先确认当前中文候选；若候选仅消耗部分拼音，则保留剩余输入并提示音，标点需在后续确认后输入。

设置 → 输入、系统输入源菜单或 Control＋Shift＋P 可切换自动、固定中文及固定英文。固定模式优先于输出语言；服务重启后恢复自动。固定英文模式会在标点前原样提交未确认字母。

自动模式保留数字后点号、冒号和斜线的半角形式，例如 `3.14`、`12:30`。`https:`、`http:`、`www.` 或邮箱中的 `@` 可触发临时字母直输，空格、换行、光标移动或中文确认后结束。无前缀域名可使用固定英文模式。检测仅依据本次输入，不读取当前应用正文。

拼音中的单引号用于分音，例如 `xi'an`；无组合时用于引号。半角标点不自动补空格。切换应用、移动光标或删除文字会重置引号及临时数字、地址状态。

## 选词记忆

已确认的中文词默认保存在本机，相同拼音下按使用次数及最近选择顺序参与排序。选择英文译法也会记忆对应中文。输入拼音、浏览候选、取消或原样提交字母不产生记忆。

同一组拼音分次确认的中文片段可合成新词；修改、取消、切换应用或原样提交会中断合成。英文输出不参与新词拼接。当前保存上限为每词 16 个汉字、5,000 条拼音映射。

设置 → 输入或输入源菜单可暂停新增记忆，已有词条继续参与排序。开关在服务重启后恢复开启，已保存词条继续保留。个人词库路径为 `~/Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json`。文件损坏时保留原文件并暂停写入；保存失败不撤回已输出文字。

## 统一设置与外观

候选窗口右上角齿轮或系统输入源菜单的“设置”打开独立设置窗口，包含以下页面：

| 页面 | 内容 |
| --- | --- |
| 账号与学习 | 邮箱登录、学习词条、掌握状态及同步；服务地址位于高级设置 |
| 翻译 | 本机 / MiniMax、接口地址、模型与钥匙串密钥配置 |
| 输入 | 中文 / 英文模式、中英候选、标点及选词记忆 |
| 外观 | 白色、黑色及跟随系统 |

输入选项在本次服务运行期间生效；未连接输入服务时对应控制禁用，可刷新状态。外观选择保存在本机，默认白色，切换即时生效并保留拼音、页码、展开与翻译状态。跟随系统采用 macOS 当前外观，设置不改变系统外观。

主动打开设置时，未确认拼音原样提交。设置辅助进程不读取个人词库，也不启动第二个输入服务。

## 本机整句翻译

没有离线释义且至少包含 3 个中文字的当前候选，可使用 Apple Translation；个人词库候选的阈值为 2 个中文字。停止输入约 350 毫秒后启动翻译，macOS 26.4 及以上优先使用低延迟策略。

首次使用需下载中英文语言资源。候选窗口的“准备语言”或输入源菜单的“准备本地整句翻译…”可打开系统语言准备界面。此操作会取消当前未提交拼音，完成后重新输入。语言下载需要联网；准备完成后由本机框架处理翻译。

翻译失败或资源未准备时保留中文选择及重试入口。当前每个整句候选仅提供一种英文译文。

## MiniMax AI 翻译

MiniMax 为可选在线翻译，默认关闭；离线词语释义仍在本机使用。

1. 从设置 → 翻译或输入源菜单的“AI 翻译设置…”打开配置。
2. 选择国内或国际预设。完整接口分别为 `https://api.minimax.cn/v1/chat/completions` 和 `https://api.minimax.io/v1/chat/completions`；项目预设模型为 `MiniMax-M2.7-highspeed`，可依据账号权限调整。
3. 填写对应平台的 API 密钥，选择“保存并启用 AI”。首次配置必须提供密钥；后续留空仅保留同一完整接口地址的已存密钥。保存操作不发送测试请求，配置成功不代表接口调用已通过。
4. 重新输入中文。无离线释义的较长候选停顿约 850 毫秒后请求翻译，候选区显示“AI 在线翻译”。核对中文和译文后，通过 Shift＋空格或英文入口确认。
5. 选择“使用本机翻译”可关闭在线模式，保留已存密钥。设置变化、继续输入、换候选或取消会使旧结果失效。

接口采用 [MiniMax 官方 OpenAI 兼容格式](https://platform.minimax.cn/docs/api-reference/text-openai-api)，思考内容与最终译文分离。请求仅包含当前中文候选及固定翻译指令；候选上限 512 字符，单次生成上限 4,096 token（含思考），超时 20 秒。重定向、截断、空输出及包含思考标签的译文不会提交。

密钥由独立辅助进程访问系统钥匙串，按完整接口地址隔离，不写入设置 JSON、进程参数、日志、仓库或 Obsidian。公开配置位于 `~/Library/Application Support/BilingualCompanion/AITranslation/settings.json`。鉴权失败、限流或断网时显示状态并保留中文，可重试或主动切回本机。

API 调用可能产生服务商费用；本机取消不能保证撤销已处理的请求。当前未提供账单或配额管理。真实 MiniMax 请求、延迟、质量与钥匙串授权尚待验证。

## 登录与单词学习

账号功能采用自建邮箱验证码服务。当前尚未部署公网服务器、域名及发信通道，在线登录暂不可用；未登录不影响输入法使用。部署步骤见 [服务端说明](../backend/README.md)与[部署说明](../backend/deploy/README.md)。短信登录尚未实现。

服务部署后，在账号与学习的高级设置填写 HTTPS 地址，发送并输入六位邮箱验证码。验证码发送间隔为 60 秒；过期或失效后需重新获取。

登录后，确认词表英文译法会记录所选英文、对应中文、词性及次数。记录支持搜索、全部 / 待掌握 / 已掌握筛选，以及手动标记或撤销掌握。“已学习”表示发生选用，不代表已经掌握。列表首先显示 50 条，可继续加载。

中文确认、原样字母、英文直输、取消及整句译文不加入单词记录。离线操作保存在当前账号队列中，联网后可重试；同步事件去重，避免重复计数。退出后停止该账号的记录，缓存按账号与服务地址隔离；退出不删除云端数据。

账号凭据由独立辅助进程访问系统钥匙串。缓存及待同步队列位于 `~/Library/Application Support/BilingualCompanion/Learning/`，其中 `service.json` 仅保存服务地址。真实邮件、账号登录及跨设备同步仍待部署后验证。

## 翻译与隐私

离线释义表的固定快照包含 232,213 个不重复中文条目。上游释义由离线大模型生成；本项目另为 30 个词编写多译法与用法示例，内容尚未经过专业审校。翻译处理所选中文，不读取前文，也不自动修正同音错字。

输入法不读取剪贴板或当前应用正文，不记录逐次输入过程。个人词库保存已确认的中文词、拼音、使用次数及最近选择顺序。启用 MiniMax 后仅发送当前候选与固定指令；登录账号仅同步词表英文、对应中文、词性、次数及掌握状态。原始拼音、正文历史、整句与个人词库不上传至学习服务。

Apple 框架可能收集语言、应用标识与性能等非文本统计，具体范围见 [Apple Translation 文档](https://developer.apple.com/documentation/translation/translating-text-within-your-app)。个人词库、学习缓存、凭据及验证码不属于仓库或 Obsidian 文档同步范围。

## 开发与检查

先在仓库根目录完成环境准备，再从本目录执行：

```sh
bash tools/build.sh
bash tools/check.sh
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --preview-narrow
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --appearance-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-settings-preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --ai-settings-preview --preview-narrow --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --ui-state ai-failed
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview --ui-state failed
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-learning
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-punctuation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-typing
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --typing-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --punctuation-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-sentence
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --setup-translation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --sentence-integration-test
../runtime/python/bin/python tools/install.py
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --register
```

设置与混合输入可使用以下隔离预览：

```sh
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --settings-preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --settings-preview --preview-narrow --theme dark
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --settings-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-mixed
```

构建使用项目 Rust 运行时、现有锁文件与 Apple Command Line Tools。Translation 框架弱链接，较早系统跳过 Apple 整句翻译；实际兼容性待测。预览使用隔离词库及内存外观状态，不读取个人配置或真实密钥。

检查涵盖 Swift 格式与类型、Rust 格式与 Clippy、真实引擎与选词记忆回归、输入状态、标点、翻译取消、资源及本地签名。整句联调需要 macOS 26 与已下载语言。预览和自动检查的通过范围与系统输入实际验收分别记录在 [QA](QA.md)中。

## 安装与更新

安装路径为 `~/Library/Input Methods/BilingualCompanion.app`，系统显示名称为“灵果”。内部应用、输入源、连接标识及数据目录沿用既有值，以保留词库与配置。

`tools/install.py` 仅替换相同 bundle ID 的应用，旧版保存于 `../evidence/native-backups/`，安装记录位于 `../evidence/native-install.json`。运行中的服务会拒绝覆盖。更新前先切换至 ABC，停止本项目输入服务，再安装并登记；完成后重新选择“灵果”。安装与登记本身不主动切换输入源。

同时运行构建目录与安装目录中的正常输入服务会争用连接，应使用安装路径中的服务。历史安装、备份与重载记录见 [INSTALLATION](INSTALLATION.md)。

```sh
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --sources
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select-id com.apple.keylayout.ABC
```

维护或测试结束后恢复原输入源。`--disable` 用于主动停用，保留应用及资源。退出试用时，先切回系统输入法，再从键盘设置移除“灵果”并移除应用。

## 输入无响应时的检查

通过 macOS 菜单先选择 ABC，再选择“灵果”，重新点击输入框进行测试。必要时在原生空白文稿中对照。部分已打开应用曾在更新后无响应，完全退出并重新打开该应用后恢复；具体原因及长期稳定性尚未确认。

系统登记成功、进程运行或选择命令返回成功，均不能单独证明应用已经接收输入。只读状态命令可辅助检查：

```sh
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --input-status
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --input-diagnostic-test
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --input-window-test
```

诊断仅保存内存中的会话、事件计数和状态布尔值，不记录字符、候选或应用标识；退出服务后清零。`markedUpdates` 与 `insertCalls` 表示客户端接口调用次数，不证明应用已显示文字。

## 验证状态与许可

0.11.8 已完成构建、隔离检查、图标资源与系统登记核验。0.11.7 的基本混合输入已有实体键盘反馈，系统菜单名称亦已确认更新；新版“果”字在实际系统菜单中的显示仍待确认。长期跨应用稳定性、较早系统兼容性、在线账号及真实 MiniMax 调用仍待验证。当前未提供整句多译法、词库管理界面、学习复习策略或输入选项持久化。

项目采用 GPL-3.0-or-later。来源说明随本地构建附带，详见 [第三方来源](../docs/THIRD_PARTY.md)。源码已公开，正式安装包尚未发布，完整数据分发许可、发行签名及公证仍待完成。
