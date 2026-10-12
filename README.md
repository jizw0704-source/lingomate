# 灵果 · LingoMate

**面向 Windows 与 macOS 的开源中英双语输入法。**

输入中文拼音，在同一个候选窗口里选择中文词语或对应的英文释义；写中文时也能直接插入英文，并查看不同译法与词性。灵果将拼音输入、词语查阅和本地单词学习结合起来，方便中英文混合写作。

[Windows 使用指南](native-windows/README.md) · [macOS 使用指南](native-macos/README.md) · [协作开发](CONTRIBUTING.md) · [开发进展](docs/PROGRESS.md)

项目目前处于早期开发与内部试用阶段，尚未提供公开安装包下载。Windows 和 macOS 的功能与验证进度分别维护，具体范围见下文。

## 核心功能

- **中英候选一起选**：输入拼音，同时查看中文候选与英文释义，展开同一词语的不同译法。
- **中文里直接写英文**：空格确认中文，回车确认当前英文字母并保留大小写；单按并松开 Shift 可切换英文直输。
- **词性提示**：显示 `n.`、`v.`、`adj.`、`adv.` 等英文缩写，可在设置中关闭。词性仅用于展示，不加入提交文本。
- **本机选词记忆**：已确认的中文词语参与后续候选排序；连续逐字输入也可尝试记忆短词，支持暂停写入。
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
| Windows 11 / Intel、AMD x64 | 原生 TSF 输入、中英词语候选、多译法、分页、混合输入、选词记忆、本地学习、外观／字号／标点设置及“果”图标 | MSVC 构建、真实引擎回归及 DLL 生命周期检查通过；本机已登记，用户确认记事本 `wo` → “我”；候选修复后的宿主重载、跨应用与高 DPI 待验证 |
| macOS / Apple Silicon | 原生系统输入、中英候选、混合输入、记忆、本地学习、翻译与外观设置 | 已有 Codex 实体键盘试用反馈；长期跨应用稳定性与完整新词持久化流程待验证 |
| 网页原型 | 本机浏览器中的中英候选与多译法交互 | 用于交互实验，不接入系统输入 |

Windows 尚未接入账号同步与 MiniMax 整句翻译。Windows 10、ARM 与 32 位应用兼容性待验证。macOS 基础输入的部署目标为 macOS 13，较早系统的实际兼容性待验证；Apple 本机翻译需要 macOS 26 及以上和已下载的中英文语言资源。

自动检查与界面预览的通过范围单独记录，实体键盘输入、安装升级和跨应用表现以实际设备验证为准。详见 [Windows 验证记录](native-windows/QA.md)、[macOS 验证记录](native-macos/QA.md)和[安装登记记录](native-macos/INSTALLATION.md)。

2026-10-12 的 Windows 本机试用发现候选空白：引擎异常提示被窄窗口裁掉，连续输入还会推迟重试。Windows 开发分支已修正提示绘制、首次响应时限及恢复查询，并安装独立的本地修复包；聊天应用后续已重新加载新 DLL，候选实际输入反馈仍待确认。普通安装 EXE 的登记异常也已记录，完整安装验收尚未完成。此本地修复没有替换维护者草稿附件或启用远程更新。

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

Windows 日常开发使用 `codex/windows-dev` 分支，相关修改通过 Pull Request 请求合并到 `main`，由项目创建者或有权限的维护者审核后合并。每次 PR 说明改动、测试结果和实际验证范围；共享变更同时检查两端兼容性。详细分工与流程见 [协作维护说明](CONTRIBUTING.md)。

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
