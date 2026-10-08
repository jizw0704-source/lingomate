# 中英输入实验版 · macOS 0.10.0

原生输入法使用 Swift / AppKit / InputMethodKit 和随包的青简 Rust 引擎，将选中的中文或英文提交到原应用。中文及词语释义支持 Apple Silicon Mac、macOS 13 以上；Apple本机整句翻译需要 macOS 26 以上和已下载的中英文翻译语言；可选MiniMax模式使用在线API，不要求下载Apple语言，旧系统实机运行待测。网页原型仍在 `../prototype/`，本次整句功能只接入原生版。

## 开始试用

安装位置为 `~/Library/Input Methods/BilingualCompanion.app`，输入源名称为“中英输入实验版”。本机旧版已经启用，新版沿用相同输入源。首次安装到另一台机器，仍需在系统设置 → 键盘 → 文本输入中添加，并自行处理系统授权；不会自动退出登录或重启。

1. 选择“中英输入实验版”，保持“中英候选”模式。
2. 输入 `wojintianxiangxuexiyingyu`，中文候选为“我今天想学习英语”。停止输入片刻，候选窗下方会显示整句英文。先核对选中的中文是否正确，可用 ↑↓ 切换。
3. 按空格输出中文，按 Shift＋空格或点“选英文”输出译文。翻译尚未完成时不会提交过期英文，仍可直接选中文。
4. 输入 `xuexi` 仍显示 `study`、`learn`；Tab 展开其他词语译法与用法。整句当前提供一种译文，可重新翻译；没有多种整句译法。

本机中英文语言已下载。其他机器缺少语言时，候选窗显示“准备语言”；也可从输入源菜单选择“准备本地整句翻译…”，进入系统语言下载界面。准备语言会取消当前未提交拼音，完成后请重新输入。

| 操作 | 结果 |
| --- | --- |
| 单按并松开 Shift，或“中文 · Shift”按钮 / 输入源菜单 | 中文拼音 ↔ 英文直输 |
| 空格，或数字 1–5 | 输出当前页中文候选 |
| Shift＋空格 | 输出当前词语译法，或已完成的当前整句译文 |
| ↑ / ↓ | 在当前页切换中文候选，整句译文随之更新 |
| PageUp / PageDown，`-` / `=`，或翻页按钮 | 上一页 / 下一页，选中该页首项 |
| Tab / Shift＋Tab | 展开词语译法，切换下一种 / 上一种 |
| 中文 / 英文 / 更多按钮 | 直接选择或查看词语用法 |
| Esc | 先收起展开状态，再取消组合输入和翻译任务 |
| Enter | 展开状态输出英文；未展开时输出原样拼音 |
| Control＋Shift＋P，或标点按钮 / 输入源菜单 | 自动 → 固定中文 → 固定英文标点 |
| F6，或模式按钮 | 切换普通中文和中英候选，保留拼音 |

每页显示5项，显示页码与总候选数；桥接层最多返回64项，数量取决于词典实际候选。翻页不提交文字，清除上一候选的展开与译文；首末页不循环。Mac没有专用翻页键时可用Fn＋↑/↓或减号/等号。

一次组合输入最多 240 个拼音字符。英文按原样提交，连续英文词之间请另按空格。快捷键交还宿主前、切换应用或输入源时，会原样提交未完成拼音。安全输入开启时不处理按键。整句任务在继续输入、换候选、取消或提交时失效，不把旧句译文提交到新句。

## 外观

0.8提供 **白色 / 黑色 / 跟随系统**。点击候选窗右上角“外观：白色”选择，也可从系统输入源菜单 → 外观选择；英文直输和没有候选时也可使用菜单。当前选项有勾选标记，切换即时生效，拼音、候选页码、展开译法与翻译任务保持原状。

首次使用沿用白色。选择会保存在本机本应用的 `appearanceChoice` 偏好中，重启服务后继续使用；跟随系统采用 macOS 当前外观。候选窗、更多译法、整句译文、英文模式提示和语言准备窗口使用同一配色；这项设置不改变系统外观。其他模式开关仍按各自的本次会话规则工作。

预览可使用 `--theme light|dark|system`，包括翻译准备窗口的 `--isolated-appearance --theme dark`。所有预览只使用内存外观设置，不读写真实外观偏好；自测使用临时偏好域并在结束时清理。

## 直接打英文

0.6支持常驻英文直输。选择“中英输入实验版”后，轻按并松开一次左或右 **Shift**（0.7秒内），切到英文直输；直接输入 `Hello, world!`，字母、大小写、数字、空格及半角标点交给原应用，不查拼音、不翻译、不学习词条。再轻按一次Shift，切回中文拼音。

Shift配合字母、空格、Tab或其他快捷键时不会切换，仍保留大写、符号、选区和Shift＋空格选译文。长按Shift、同时按两个Shift、与Control/Command/Option/Fn搭配均不作为单按切换。也可点击候选窗“中文 · Shift”按钮或输入源菜单中的当前模式条目；切换后有约1.5秒的状态提示，并可点击反向切换。提示不会成为键盘焦点，也不会隐藏新出现的候选。

切换前若还存在未确认拼音，会把字母原样输出并取消待定翻译/组词记忆，不替你猜选中文。已输出的正文和已保存词条保留。中文模式继续使用原有分页、词语/整句翻译和选词记忆；F6仍切换普通中文 / 中英候选显示，和Shift的英文直输开关分别保留。英文直输不执行中文标点转换，即使此前固定中文标点；切回中文恢复原标点设置，自动模式从中文开始。

英文直输时F6、Control＋Shift＋P及其他普通按键都交给宿主；中文模式才拦截对应功能快捷键。当前输入模式在本次服务中跨应用保留，重启服务后默认中文；无需切到系统ABC。应用的智能引号或自动更正仍可能改变最终文本。单Shift的真实系统事件和跨应用焦点需要实际键盘试用，预览不能代替验收。

## 候选窗布局

0.7将拼音单独放在顶部，下方分别是输入模式、中英候选模式和标点按钮。中文候选与英文译法分列，当前项有 `›` 和浅灰背景；词库记忆在词条旁标明。点击“更多”后，译法、说明和例句紧随当前候选，点击“收起”恢复。

整句译文同样紧随当前候选；翻译中、未准备、失败和不支持的状态保留中文选择，尚未完成时禁用英文输出。长中文完整换行，长译词在候选列省略但保留完整辅助标签及工具提示，展开后可完整阅读。窄面板将中英文改为上下排列，超高内容滚动；圆角和边框不随内容滚走。

底部显示当前页、总项数和快捷键，首末页的越界按钮禁用。没有候选时，只提示继续输入、Enter保留原样拼音、Esc取消和Shift英文直输。现有键盘提交、翻页和模式逻辑保持不变。原生界面使用Pheno v1.4中性样式，MiSans缺失时回退系统字体，无输入动画。

## 中英文标点

0.5 默认自动跟随当前会话最近确认输出的语言。选中文后，逗号/句号/问号等输出 `，。！？：；`，括号输出 `（）【】《》`，单、双引号分别交替输出 `‘’`、`“”`；选择词语英文或整句英文后，使用对应半角符号。切到普通中文模式会把自动标点恢复为中文。

例如输入 `nihao` 并选择“你好”，再按逗号键，输出“你好，”；选择 `xuexi` 的 `learn` 后按逗号，输出 `learn,`。自动模式下，拼音仍在候选窗且按标点时，会先输出当前中文候选，再补标点；如果只消耗部分拼音，则先输出该片段、保留剩余拼音并提示音，待继续选词后再按标点，不丢弃剩余字母。无候选时原样提交字母。

点击候选窗的“标点”按钮，或按 **Control＋Shift＋P**，依次切换自动 / 固定中文 / 固定英文；输入源菜单可直接选择。固定模式优先于最近输出语言，重启服务后恢复自动。固定英文时，标点前尚未确认的字母会原样提交，便于输入域名；若要中文文字搭配半角标点，请先确认中文候选。

自动模式下数字后的点号、冒号及斜线保留半角，例如 `3.14`、`12:30`。数字后直接按点号会优先视为小数点；需要中文句号时可切到固定中文。网址前缀 `https:`、`http:`、`www.` 和邮箱中的 `@` 会进入临时字母直输，后续地址符号保持半角，空格/换行、移动光标或确认中文会结束；不带前缀的 `example.com` 请先切到固定英文，以免被当作拼音。输入正文已有的数字、标点或网址不在检测范围，本功能不读取原应用正文。

拼音组合中的单引号仍用于分音，例如 `xi'an`；未组合时才用作引号。半角英文标点不会自动补空格。切换应用、移动光标或删除文字后会重置引号开闭及临时地址/数字状态。宿主应用自己的智能引号、自动更正可能再次改变字符，需在实际应用中核对。

## 登录与单词学习

0.9候选窗更加紧凑：宽度560pt、内边距12pt，减少重复标题和提示，点击目标仍至少44pt；440pt窄面板中英文上下排列，长内容可滚动。白色、黑色和跟随系统继续有效。

点击候选窗“学习”，或输入源菜单 → 登录与学习记录，打开独立学习中心；进入时未确认拼音按原样保留到宿主，取消旧翻译，不会把它记作单词。登录窗口不会在正常打字过程中自动出现。

自建服务采用邮箱验证码登录：服务设置填写部署后的HTTPS地址 → 发送验证码 → 输入邮箱中的六位验证码 → 验证并登录。验证码过期或失效须重新获取；邮箱发送间隔60秒。短信尚未实现。

登录后的英文词表提交产生“已学习”记录，包含所选英文、对应中文、词性及选用次数。多词译法可以作为一个词条；“已学习”不代表掌握。手动标记掌握或撤销，支持搜索及全部/待掌握/已掌握筛选；较多词条先显示50条，再点“显示更多”，保留滚动位置。整句本机或AI译文、中文选择、英文直输、取消和原样拼音不加入学习单词。正常词语选用后后台尝试同步；学习页打开和“同步”按钮也可刷新。离线保留操作，事件去重避免重试增加多次计数。退出后不继续记录此前账号，缓存按账号和服务地址分开；退出不会删除云端数据。

凭据仅在独立账号辅助进程中访问系统钥匙串；本机学习缓存和待同步队列保存在`~/Library/Application Support/BilingualCompanion/Learning/`。服务地址保存在该目录`service.json`，不是密码。不要把学习目录、凭据、验证码或个人词库提交Git或复制到Obsidian。

**当前没有服务器、域名及邮件通道，按用户选择先交付部署说明。** 未连接服务时按钮禁用并显示状态，不能真实在线登录。客户端/服务隔离协议和UI样例检查不等于实际邮件送达、Keychain保存或跨设备验收。[部署说明](../backend/README.md)。

## 选词记忆

0.4 默认记住已确认输出的中文词，下次输入相同拼音优先显示并标记“记忆”。同一拼音下先按使用次数、再按最近选择顺序排序；个人词库在服务或电脑重启后保留。选择英文译法也记住对应中文词，不保存英文译文。

例如第一次输入 `shi`，翻页选择“市”；再输入 `shi`，“市”会来到第一项。输入法不会在仅输入拼音、浏览候选、取消或原样输出拼音时学习。

同一段拼音分次选出的中文会合成一个新词。测试示例：输入 `shiqing`，先选“市”，再从剩余 `qing` 选择“青”；下次输入 `shiqing` 可直接选择“市青”。这只是用于检查的新词示例；修改/取消拼音或切换应用会中断合成，两个独立输入的词不会自动拼起来。当前只保存最多16个汉字的词/短语，词库最多5000条拼音映射。词库中没有释义的两字及以上新词可使用当前选择的本机或AI翻译。

输入源菜单的“选词记忆（本次开启/本次暂停）”可暂停新增学习，已有词仍可优先显示；该开关重启服务后恢复开启。个人词库仅保存在 `~/Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json`，不会同步到 GitHub 或 Obsidian。文件损坏时保留原文件并暂停写入；保存失败不会撤回已输出文字，可在输入源菜单查看提示。

## MiniMax AI 翻译

0.10增加可选在线整句翻译，默认关闭，词语的随包释义继续在本机使用。

1. 在macOS输入源菜单选择“AI 翻译设置…”，或在整句候选区点击“本机翻译”。打开设置时未确认拼音会原样输出，不记作单词。
2. 选择“MiniMax 国内”或“MiniMax 国际”。预设完整接口分别为`https://api.minimax.cn/v1/chat/completions`和`https://api.minimax.io/v1/chat/completions`，模型`MiniMax-M2.7-highspeed`；模型名可按账号实际权限修改。国内、国际平台须使用各自对应的密钥，预设只填字段，不启用或联网。
3. 自己填写对应平台的API密钥，点“保存并启用 AI”。首次留空不会启用；后续留空只保留同一完整地址的已保存密钥。保存本身不调用API，也没有假“连接成功”。状态显示启用只代表配置保存成功，密钥/模型权限由首次实际请求检验。
4. 重新输入`wojintianxiangxuexiyingyu`，先核对中文。没有词表释义且至少三个中文字的候选（个人词库候选至少两个字）停顿约850毫秒后开始在线翻译；候选区标记“AI 在线翻译”。空格选中文，Shift＋空格或“选英文”选译文。译文尚未完成不能提交旧英文。
5. 随时回设置点“使用本机翻译”。关闭AI保留钥匙串密钥，不会自动删除。设置变化、继续输入、换候选或取消会使旧结果失效；不会自动把旧句再次上传，需要重新输入或重试。

接口采用[MiniMax官方OpenAI兼容格式](https://platform.minimax.cn/docs/api-reference/text-openai-api)，将思考内容单独返回，仅显示最终译文。只发送当前候选和固定翻译指令，不发送原始拼音、宿主正文、历史、个人词库或登录信息。最大候选512字符，MiniMax单次生成上限4096 token（含思考），单次请求超时20秒；拒绝重定向、截断、空输出和含思考标签的响应。AI不是自动纠错：中文候选同音错字仍需用户核对。当前整句仍只有一种译法，整句AI译文不加入学习账号的单词记录。

密钥通过独立设置/翻译辅助进程访问系统钥匙串，按完整接口地址隔离，不保存到设置JSON、进程参数、临时文件、日志、仓库或Obsidian。公开配置位于`~/Library/Application Support/BilingualCompanion/AITranslation/settings.json`，不要提交本机目录。API可能产生费用，数据处理按所选服务商政策；取消本机任务无法保证撤销服务商已经处理的请求或费用。连续打字会取消旧任务，但每次停顿和手动重试仍可能产生请求，当前没有用量账单或配额管理。

鉴权失败、限流、断网或不完整输出会显示提示，中文仍可选；可重试或显式切回本机，不静默换来源。隔离测试不读取真实密钥、不发送付费请求。本轮没有真实MiniMax密钥，因此速度、译文质量、实际Keychain授权和真实宿主键盘仍待试用；现有Apple三句模型联调继续通过。

## 翻译与隐私

词语使用随包青简英文释义表，固定快照含232,213个不重复中文条目；上游释义由离线大模型生成。30个词附本项目多译法和手工用法示例，未经过专业审校。默认将至少三个中文字（个人词库候选至少两个字）且没有词表释义的当前候选，交给 Apple Translation 框架本机翻译，停止输入约 350 毫秒后开始。框架优先使用低延迟策略（macOS 26.4 以上）。默认未接入云端翻译接口；用户显式启用AI时按上一节发送当前候选，本机选词记忆保存已确认中文词、拼音、使用次数及最近选择顺序，不保存逐次输入过程、不读剪贴板、不采集宿主上下文；用户登录后独立账号服务只同步所选英文词条、对应中文、词性、次数和手动掌握状态，不上传整句、拼音或个人词库；下载语言资源需要联网。系统框架可能收集语言、应用标识和性能等非文本统计，见 [Apple Translation 文档](https://developer.apple.com/documentation/translation/translating-text-within-your-app)。本应用不声称关闭操作系统自身的统计。

翻译只针对选中的中文候选，不自动纠正同音错字，也不利用前文上下文。很长的拼音可能需要多次选择中文片段；翻译失败或未准备时保留中文选择和重试入口。

## 开发与检查

从本目录执行：

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

构建使用项目 Rust 运行时、既有锁文件和 Apple Command Line Tools，不需要完整 Xcode；Translation 框架弱链接，旧系统跳过Apple本机整句功能，旧系统运行仍待实测。检查包括隔离外观保存/子进程读回、环境继承和双主题对比度，Swift 格式、类型检查、Rust 格式与 Clippy、8 项真实引擎回归、10 项隔离词库记忆回归、Shift单按/组合保护/焦点边界及中文引擎恢复检查、标点映射/语言/边界及真实引擎组合提交检查、随包资源自测、翻译取消与过期结果检查、属性和本地签名验证。整句联调需 macOS 26 和本地语言；预览与联调不能代替真实宿主输入验收。

`install.py` 只替换相同 bundle ID 的实验版，旧包保留在 `../evidence/native-backups/`，安装记录为 `../evidence/native-install.json`。安装与注册不主动切换输入源。更新已运行的服务时先切到 ABC，停止该实验版进程、安装并注册，再选择实验输入源；必要时从已安装路径启动服务。不要同时运行构建目录的普通 IMK 服务和已安装服务。

```sh
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --sources
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select-id com.apple.keylayout.ABC
```

测试后恢复测试前输入源；本次用户已经使用实验输入法，不沿用 0.1 安装记录中的旧简体拼音恢复目标。`--disable` 仅用于主动停用，保留应用与资源。

## 状态和许可

0.8 增加三种外观与本地持久记忆，检查正常/窄候选、更多译法、整句译文、英文模式提示和准备窗口。未改变 macOS 全局外观，真实系统深浅色切换及宿主菜单/焦点仍待试用。0.7 已在本机完成构建、真实引擎及本机翻译检查，并检查原生预览的正常/窄面板、长内容滚动、更多译法、分页、翻译状态及英文直输布局，安装更新。预览与实际输入法的事件路径不同；没有据此宣称真实键盘、宿主焦点或跨应用验收完成。具体证据和待验证项见 [QA.md](QA.md)。

当前候选最多64项；已支持本地选词记忆；尚无整句多译法、学习统计页面、词库管理界面、其他持久设置或跨平台接入。沿用 GPL-3.0-or-later。青简源码保持原样，不使用其商标、图标或外观；来源说明随包保留为 `GLOSSARY-NOTICE.md` 和 `LEXICON-NOTICE.md`。这是本地实验包，未做发行签名、公证或公开发布审核。

不再试用时，先切回系统输入法，在键盘设置移除实验输入源，再把此应用移入废纸篓，保留其他输入法。
