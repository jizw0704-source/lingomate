# 灵果 · LingoMate

PR #7 的候选窗底部提示修复与共享引擎 Mac 兼容检查见 [开发进展](docs/PROGRESS.md#pr-7-候选窗修复与共享引擎复核-2026-10-13)。源码检查、Windows 实机验收和安装包发布分别记录。

Mac 本机试用版已更新至 **0.12.11 / build 33**，保留候选筛选与词频排序，增加英文片段首字母大写和固定表达联想；功能源码通过独立 Mac PR 供参考，尚未合入主分支或公开发布安装包。今日功能、Windows 接入条件及验收用例见 [Wright 交接说明](docs/MAC_FEATURE_HANDOFF_2026-10-13.md)。

**面向 Windows 与 macOS 的开源中英双语输入法。**

输入中文拼音，在同一个候选窗口里选择中文词语或对应的英文释义；写中文时也能直接插入英文，并查看不同译法与词性。灵果将拼音输入、词语查阅和本地单词学习结合起来，方便中英文混合写作。

[Windows 使用指南](native-windows/README.md) · [macOS 使用指南](native-macos/README.md) · [协作开发](CONTRIBUTING.md) · [开发进展](docs/PROGRESS.md)

项目目前处于早期开发与内部试用阶段，尚未提供公开安装包下载。Windows 和 macOS 的功能与验证进度分别维护，具体范围见下文。

## 核心功能

- **中英候选一起选**：输入拼音，同时查看中文候选与英文释义，展开同一词语的不同译法。
- **中文里直接写英文**：空格确认中文，回车确认当前英文字母并保留大小写；单按并松开 Shift 可切换英文直输。
- **Mac 英文片段首字母大写**：中文模式确认英文时，默认输出“我Hello world”，已有大小写保留，设置中可关闭。已纳入本机 0.12.11，详见 [Mac 使用说明](native-macos/README.md#英文片段首字母大写)。
- **Mac 固定表达联想**：Mac 端输入开头拼音，例如 `chiputao`，可选择整句“吃葡萄不吐葡萄皮”；保留普通候选，确认输出后记忆。首版 5,427 条，已纳入本机 0.12.11，详见 [功能说明](native-macos/README.md#固定表达联想)。
- **词性提示**：显示 `n.`、`v.`、`adj.`、`adv.` 等英文缩写，可在设置中关闭。词性仅用于展示，不加入提交文本。
- **Mac 借词取字记忆（开发版）**：从“灵活”和“果实”分别保留“灵”“果”，结束词组后记住“灵果”，并与个人词库管理整合。0.12.13 开发构建尚未安装或发布，见 [使用说明](native-macos/README.md#借词取字记忆--mac-01213-开发版)。
- **本机选词记忆**：已确认的中文词语参与后续候选排序；连续逐字输入也可尝试记忆短词，支持暂停写入。
- **Mac 个人词库管理（开发版）**：查看、搜索中文记忆词，删除同读音全部记忆入口，设置或取消优先推荐；尚未安装或发布，见 [使用说明](native-macos/README.md#个人词库管理--mac-01212-开发版)。
- **本地单词学习**：无需登录即可记录选用的英文词条、次数和手动掌握状态，支持搜索、筛选、删除与 CSV 导出。

两端均提供浅色、深色、跟随系统、候选字号和标点规则设置。macOS 另提供 Apple 本机整句翻译及可选 MiniMax 在线翻译；Windows 当前以词语候选和混合输入为主。各平台的完整操作说明见对应使用指南。

## 输入示例

| 场景 | 操作与结果 |
| --- | --- |
| 选择中文或英文 | 输入 `xuexi`，选择“学习”或对应的 `study`、`learn` 等译法 |
| 中英文混写 | `wo`＋空格 → “我”，`AI`＋回车 → “AI”，`xuexi`＋空格 → “学习”，得到“我AI学习” |
| 逐字组词记忆 | 分别确认“钙”“钛”“矿”后，再输入 `gaitaikuang`，可优先选择已记忆的“钙钛矿” |

每页显示 5 个候选，空格或数字 1–5 确认中文，Shift＋空格确认当前英文译法，Tab 展开并切换译法。输入中的回车只确认当前字母；换行或发送消息由后续无组合输入的回车交给当前应用处理。

未知短词记忆采用启发式规则，可能保存前缀或误记短语；同音候选与译文仍需人工选择。各平台快捷键、设置和已验证用例见 [Windows 使用指南](native-windows/README.md)与 [macOS 使用指南](native-macos/README.md)。

## 平台支持与验证

| 平台 | 当前范围 | 验证进度 |
| --- | --- | --- |
| Windows 11 / Intel、AMD x64 | 原生 TSF 输入、中英词语候选、多译法、分页、混合输入、选词记忆、本地学习、外观／字号／标点设置及“果”图标 | 10 月 13 日在普通 PowerShell 重新安装 0.1.16 后，用户已在 Chrome／钉钉／grokbot／ChatGPT 中验收输入，候选窗模糊与逐键变大小已修复并在本机 100%／150% 两块屏上手测通过；Windows 搜索栏不在当前能力范围 |
| macOS / Apple Silicon | 原生系统输入、中英候选、混合输入、记忆、本地学习、翻译与外观设置 | 已有 Codex 实体键盘试用反馈；长期跨应用稳定性与完整新词持久化流程待验证 |
| 网页原型 | 本机浏览器中的中英候选与多译法交互 | 用于交互实验，不接入系统输入 |

Windows 尚未接入账号同步与 MiniMax 整句翻译。Windows 10、ARM 与 32 位应用兼容性待验证。macOS 基础输入的部署目标为 macOS 13，较早系统的实际兼容性待验证；Apple 本机翻译需要 macOS 26 及以上和已下载的中英文语言资源。

自动检查与界面预览的通过范围单独记录，实体键盘输入、安装升级和跨应用表现以实际设备验证为准。详见 [Windows 验证记录](native-windows/QA.md)、[macOS 验证记录](native-macos/QA.md)和[安装登记记录](native-macos/INSTALLATION.md)。

Windows 0.1.16 已修复本机 COMLESS 加载缺少 HKLM 自有 COM 登记的问题，并补齐同账号 UAC 安装、旧版恢复和 URL 上下文打断。2026-10-13 确认，此前跨应用失败的原因是安装和系统加载探针都在 Codex 桌面应用（MSIX 包）的终端中执行，文件与用户登记被重定向到该包私有位置。安装脚本现拒绝在有包身份的进程中运行；在普通 PowerShell 重新安装后，用户已验收 Chrome、钉钉、grokbot 和 ChatGPT 的输入。随后修复了候选窗模糊和逐键变大小（ClearType、候选窗独立的每显示器 DPI、保留当前候选直到新结果返回），已在本机手测通过。改动留在 `codex/windows-dev`，尚未提交、推送或发布，远端 Windows 0.1.10 草稿保留。证据与尚未解决的边界见 [Windows 根因检查](docs/WINDOWS-ROOT-CAUSE-AUDIT.md)和 [验证记录](native-windows/QA.md)。

## 安装与试用

本轮内部试用交付版本为 **Windows 0.1.10** 和 **Mac 0.12.9 / build 31**，保存在维护者可见的 [GitHub Releases 草稿](https://github.com/jizw0704-source/lingomate/releases)中，附带校验文件、对应源码与试用说明。维护者可下载后将完整文件转交试用者；安装包使用者无需准备开发工具。

正式发行签名、Mac 公证与完整安装验收尚未完成，远程更新清单保持未发布。两端已实现版本检查、可选每日提醒、下载校验、确认安装及旧版恢复，实际发布下载与升级仍需验证。制作、交付与验收步骤见 [安装包与同事试用](docs/PACKAGING.md)。

版本历史和具体试用反馈集中在 [开发进展](docs/PROGRESS.md)，各平台安装步骤见对应使用指南。

## 从源码开发

### Windows

准备 Windows 11 x64、64 位 PowerShell、Git、Rustup、Python 3.10+，以及带 MSVC 和 Windows SDK 的 Visual Studio C++ 构建工具：

```powershell
git clone https://github.com/jizw0704-source/lingomate.git
cd lingomate
git switch codex/windows-dev
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
```

脚本固定 Rust 1.96.0，并使用现有依赖锁文件。构建与检查不会安装或选择输入法；实际安装步骤见 [Windows 使用指南](native-windows/README.md#安装与使用)。制作安装 EXE 另需 Inno Setup 6。

### macOS

在 Apple Silicon Mac 上安装 Command Line Tools、uv 与 pnpm，然后执行：

```sh
git clone https://github.com/jizw0704-source/lingomate.git
cd lingomate
bash tools/setup.sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
```

准备脚本固定 Rust 1.96.0 与上游快照，发现已有上游改动时会停止并保留文件。预览窗口用于检查布局与交互；系统安装、备份与输入源恢复见 [macOS 使用指南](native-macos/README.md#安装与更新)。

### 网页原型

按 [网页原型开发说明](prototype/AGENTS.md)准备环境后，在仓库根目录执行：

```sh
pnpm --dir prototype build
pnpm --dir prototype serve
```

访问 [本机原型页面](http://127.0.0.1:9037/)。功能范围与操作见 [网页原型说明](prototype/README.md)。

## 协作维护

macOS 由[项目发起人](https://github.com/jizw0704-source)主要维护，Windows 由 [Wright](https://github.com/LightningFlashEvE) 主要维护。拼音引擎、候选排序、词库与兼容接口由双方协作。

Windows 今后的开发只使用 `codex/windows-dev`，不另建 Windows 功能分支；Pull Request 的来源为该分支，目标为 `main`，由项目创建者或有权限的维护者审核后合并。不得直接推送 `main` 或未经明确要求自动合并。每次 PR 说明改动、测试结果和实际验证范围；共享变更同时检查两端兼容性。详细分工与流程见 [协作维护说明](CONTRIBUTING.md)。

Mac 新功能通过 PR 提供参考实现，再使用 [Mac 新功能同步到 Windows 任务模板](https://github.com/jizw0704-source/lingomate/issues/new?template=macos-to-windows.md)交接给 Wright。任务集中记录用户行为、参考代码及共同验收用例，分别跟踪 Mac 完成、Windows 实现和 Windows 实机验证。完整步骤见 [功能同步流程](CONTRIBUTING.md#mac-新功能同步到-windows)。

当前 Mac 功能、默认设置、安装包源码覆盖范围与 Windows 共同验收用例集中在 [Mac 版本功能基线](docs/MAC_FEATURE_BASELINE.md)，供两端维护者对照。

| 目录 | 内容 |
| --- | --- |
| `native-windows/` | Rust Windows TSF 客户端、候选窗口与构建／安装脚本 |
| `native-macos/` | Swift、AppKit 与 InputMethodKit 输入法客户端 |
| `prototype/` | 浏览器原型、本地 Rust 候选桥接与独立用法示例 |
| `backend/` | 基于 Python 标准库、SQLite 与 SMTP 的自建账号服务 |
| `tools/` | 环境准备、词库构建、来源验证与文档同步工具 |
| `data/` | 固定数据来源及独立排序设置；原始缓存与生成表不提交 |

## 数据与隐私

中文拼音输入、词语释义、个人选词记忆与本地学习记录在本机处理。输入法不读取剪贴板或当前应用正文，不保存逐次输入历史；本地学习只保存词条与聚合元数据，“已学习”表示发生选用，不等于已经掌握。

macOS 的 MiniMax 在线翻译默认关闭。用户主动启用并配置接口后，仅发送当前选中的中文候选与固定翻译指令，密钥保存在系统钥匙串；不发送原始拼音、个人词库、宿主正文或输入历史。Apple 翻译语言资源首次下载需要联网。详情见 [MiniMax 配置](native-macos/README.md#minimax-ai-翻译)与[翻译隐私说明](native-macos/README.md#翻译与隐私)。

自建邮箱验证码与账号同步服务尚未公开部署。账号同步仅针对所选词表英文、对应中文、词性、次数与手动掌握状态，整句和个人词库不进入同步范围；本地学习无需账号，不自动上传。服务准备见 [服务端说明](backend/README.md)。

## 词库与许可

代码采用 **GPL-3.0-or-later**，见 [LICENSE](LICENSE)。中文词库与离线英文释义由固定 **CC-CEDICT** 快照独立生成，改编数据采用 **CC BY-SA 4.0**，保留署名与修改说明；覆盖 120,662 个中文词面，其中 107,757 个带英文释义，并附 30 个项目独立编写的用法示例。

候选排序结合固定 **Google Books Ngram** 词频和项目独立编写的日常先验，只为词库已有词面赋权。英文词性由可精确匹配的 **WordNet 3.0** 索引及已有人工标注提供，无法匹配的义项保持无标注；完整 WordNet 许可随安装资源提供。来源、许可与可复现构建说明见 [第三方来源](docs/THIRD_PARTY.md)，前期调查见 [历史调研](RESEARCH.md)。

项目文档可同步到既有 Obsidian 知识库的 `10 项目/中英输入法/`，项目总览直接镜像本 README。同步仅处理项目文档，检测到手工修改时会停止并保留原文件：

```sh
runtime/python/bin/python tools/sync_obsidian.py --vault '/path/to/Obsidian Vault'
```

2026-10-12 Windows 0.1.12 已修复真实试用暴露的候选鼠标离开通知循环，并完成本机登记；0.1.11 已撤销登记。实际聊天应用待用户复测，代码未提交。当前 Windows UI 尚未完整对齐远端 Mac 0.12.9／build 31，候选布局与设置入口继续对照。[本轮验证](native-windows/QA.md#鼠标离开候选窗卡死修复--0112--2026-10-12)。

## Windows 候选窗与菜单对齐 Mac · 0.1.13 · 2026-10-12

按用户选择，本轮仅对齐候选窗与菜单，参考 Mac 0.12.9／build 31。已同步最新主分支 `04535f7` 的平台交接文档；该次远端更新没有修改 Mac 客户端 UI。Windows 采用同组中性色／绿色英文、12px 留白、圆角、左侧拼音和右侧分页／译法／设置，短词配对横排、长中文换行、词性同行、完整本地提示；更新入口移至“⚙ 设置 → 检查版本更新…”。保留 Windows 系统字体回退和原生菜单。独立设置窗口、账号／整句翻译与完整无障碍没有纳入本次范围。

0.1.12 的鼠标离开修复保留；导航等待期间保持候选几何，按下／释放绑定同一显示版本，高面板限制在工作区并支持滚轮浏览。展开译法的空释义／例句绘制访问异常已由真实 GDI 合成测试复现并修复。MSVC 构建、格式、all-targets Clippy、68 项 Rust、三线程模式 192 轮鼠标循环／27 个行为阶段及两种 PowerShell 各 56 项包检查通过。

已登记本机 `0.1.13-FBF7848BC586`，保留 0.1.12 目录；尚未提交、推送或发布。用户必须完整退出并重开已有宿主才能加载新 DLL。ChatGPT／钉钉的实际切源、鼠标、键盘、跨应用和多屏 DPI 验收仍待用户执行；源码对照与合成绘图不等于 Mac 或 Windows 实体设备验收。Obsidian 路径未配置，本轮只更新仓库文档。

## Windows UI 完整译法滚动 · 0.1.14 · 2026-10-12

当前本机登记版本为 0.1.14，在 0.1.13 候选／菜单对齐基础上，将更多译法改为完整滚动列表，保留绝对译法索引；绘制、tooltip 和点击只处理当前可见区域，不再按六条分组改变面板尺寸。100,000 条合成译法的几何回归确认访问范围受视口限制。Mac 参考仍为 0.12.9／build 31，最新主分支 04535f7 的交接文档已同步，Mac 客户端和数据源未修改。

最终 MSVC 构建、格式、all-targets Clippy、69 项 Rust（共享 8／Windows 61）、三模式 192 轮鼠标循环／27 个行为阶段、PowerShell 7／5.1 各 56 项更新包检查通过。浅／深色、POS 同行／关闭、长中文、空释义／例句、完整译法和有界滚动渲染通过。源码对照和合成检查不替代 Mac 实机或 ChatGPT／钉钉的实际切源、输入和物理鼠标验证。

已同账号可信登记 `%LOCALAPPDATA%\LingoMate\Windows\0.1.14-BBD895C6F73E`，13 项文件校验通过；清单 SHA-256 为 `BBD895C6F73EC5657A67C013830941CBE6BEE5074126BD4C6F26109D57CAC966`，DLL SHA-256 为 `03632A587225E5170FE7E0737E0C28940CEE8B326FAE21B2E6F1A768DBDBA245`。0.1.13／0.1.12 原目录保留；没有自动停止宿主、选择输入源、重启、提交或发布。所有本轮改动仍在 codex/windows-dev 未提交工作区，等待用户真实应用验收。当前用户需完整退出并重开应用加载新 DLL；普通 Setup EXE 和 Mac 构建／设备验证边界保留。Obsidian 路径未配置。

## Windows 宿主兼容补齐 · 0.1.15 · 2026-10-12

用户随后对 0.1.15 手测确认：钉钉仍使用微软输入法；grokbot（独立 Windows 软件）和谷歌浏览器只输出字母，无拼音候选。因此 0.1.15 未通过跨应用验收。真实 ThreadMgr／flags=0 的隔离系统加载探针已确认 Windows 可加载此 DLL、Foreground CLSID 匹配并成功恢复原线程 profile；它未模拟物理键盘，不代表上述应用正常。正在进一步区分应用未载入与激活后英文／焦点／literal 状态门禁。

用户确认 0.1.14 在 ChatGPT 中正常，但钉钉和 grokbot 实际输入仍是微软输入法。钉钉 9.0.1 的 x64 进程在重开后未驻留灵果 DLL；切换器与托盘提示不是实际接入输入线程的证据。只读诊断还发现：灵果虽已能被 EnumEnabledLayoutOrTip 枚举，当前用户 zh-Hans-CN 的 InputMethodTips 原先只列微软拼音；不能据此把所有失败断言为同一根因。

本轮对照固定源码：[微软 SampleIME](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/IME/cpp/SampleIME/SampleIME.cpp)、[Weasel](https://github.com/rime/weasel/blob/4aea1978a351e2ce41af20c8fb88a01ce9802de8/WeaselTSF/WeaselTSF.cpp)、[Mozc](https://github.com/google/mozc/blob/18e76511af9371a9a4409b1d13c3e30884b53e9c/src/win32/tip/tip_text_service.cc)。补齐激活时 GetFocus／GetTop、ITfThreadFocusSink、线程 OPENCLOSE／CONVERSION 双向同步和 GUID_LBI_INPUTMODE 语言栏；仅修改 NATIVE 位，保留其他模式位。队列满时保留 Finish → EnglishMode 边界，期间不接新键；候选窗创建期间递归激活被有界拒绝。跟随系统主题改为只读注册表，不再初始化宿主 WinRT。注册增加已实现的 INPUTMODECOMPARTMENT；未声明沉浸、UIElement 或显示属性支持。

可信安装通过 System32 input.dll 的 BOOL InstallLayoutOrTip 完成单条自有用户项安装，安装 flags=0、卸载 flags=1；不设置默认源或清空其他输入法。升级和恢复保留原 enabled／disabled 状态，同包重试保留原 receipt，失败恢复旧登记／启用状态；卸载先撤销自有用户项再移除登记，安装 EXE 给卸载器保留可信模块副本。

MSVC 构建、格式、all-targets Clippy、75 项 Rust（共享 8／Windows 67）、STA／MTA／COMLESS 共 48 个行为场景与 192 轮鼠标循环、PowerShell 7／5.1 各 101 项模拟安装更新检查通过。新增检查包含已有聚焦上下文激活、线程局部 CBT 递归激活、中英 compartment／语言栏按钮、同文档失焦结束及恢复、双队列饱和模式边界，以及系统主题读取不改变 STA／MTA／未初始化 apartment。普通安装 EXE 编译通过，未以其编译成功代替权限／卸载实机验收。

已用同账号可信 helper 登记 `%LOCALAPPDATA%\LingoMate\Windows\0.1.15-3B4294E1F2F0`，13 项清单及 COM 工厂回读通过，启用状态保持 true；用户 InputMethodTips 现包含微软拼音和灵果。清单 SHA-256 `3B4294E1F2F05DDD9F20A6A05AF3A0578434B8EB7908C7B29D6C77859CB521E7`，TSF DLL SHA-256 `9FD4134D553325B78F65AEC1F089A8E1E3D4FB8C51D011C236AB90D11EF3A992`；0.1.14 原目录保留。钉钉／grokbot／谷歌浏览器已经手测失败；ChatGPT 0.1.15 回归仍待完整退出重开后验收；本轮没有选择系统输入源、停止宿主、重启 ctfmon／电脑、提交、推送或发布。Mac 候选／菜单参考仍是 0.12.9／build 31，未修改 Mac 客户端或数据；共享核心跨平台复核和 Mac 设备检查边界保留。Obsidian 路径未配置，未同步外部笔记库。
