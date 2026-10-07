# 中英输入实验版 · macOS 0.3.0

原生输入法使用 Swift / AppKit / InputMethodKit 和随包的青简 Rust 引擎，将选中的中文或英文提交到原应用。中文及词语释义支持 Apple Silicon Mac、macOS 13 以上；整句翻译需要 macOS 26 以上和已下载的中英文翻译语言。网页原型仍在 `../prototype/`，本次整句功能只接入原生版。

## 开始试用

安装位置为 `~/Library/Input Methods/BilingualCompanion.app`，输入源名称为“中英输入实验版”。本机旧版已经启用，新版沿用相同输入源。首次安装到另一台机器，仍需在系统设置 → 键盘 → 文本输入中添加，并自行处理系统授权；不会自动退出登录或重启。

1. 选择“中英输入实验版”，保持“中英候选”模式。
2. 输入 `wojintianxiangxuexiyingyu`，中文候选为“我今天想学习英语”。停止输入片刻，候选窗下方会显示整句英文。先核对选中的中文是否正确，可用 ↑↓ 切换。
3. 按空格输出中文，按 Shift＋空格或点“输出英文”输出译文。翻译尚未完成时不会提交过期英文，仍可直接选中文。
4. 输入 `xuexi` 仍显示 `study`、`learn`；Tab 展开其他词语译法与用法。整句当前提供一种译文，可重新翻译；没有多种整句译法。

本机中英文语言已下载。其他机器缺少语言时，候选窗显示“准备翻译语言”；也可从输入源菜单选择“准备本地整句翻译…”，进入系统语言下载界面。准备语言会取消当前未提交拼音，完成后请重新输入。

| 操作 | 结果 |
| --- | --- |
| 空格，或数字 1–5 | 输出当前页中文候选 |
| Shift＋空格 | 输出当前词语译法，或已完成的当前整句译文 |
| ↑ / ↓ | 在当前页切换中文候选，整句译文随之更新 |
| PageUp / PageDown，`-` / `=`，或翻页按钮 | 上一页 / 下一页，选中该页首项 |
| Tab / Shift＋Tab | 展开词语译法，切换下一种 / 上一种 |
| 中文 / 英文 / 展开按钮 | 直接选择或查看词语用法 |
| Esc | 先收起展开状态，再取消组合输入和翻译任务 |
| Enter | 展开状态输出英文；未展开时输出原样拼音 |
| F6，或模式按钮 | 切换普通中文和中英候选，保留拼音 |

每页显示5项，显示页码与总候选数；桥接层最多返回64项，数量取决于词典实际候选。翻页不提交文字，清除上一候选的展开与译文；首末页不循环。Mac没有专用翻页键时可用Fn＋↑/↓或减号/等号。

一次组合输入最多 240 个拼音字符。英文按原样提交，连续英文词之间请另按空格。快捷键交还宿主前、切换应用或输入源时，会原样提交未完成拼音。安全输入开启时不处理按键。整句任务在继续输入、换候选、取消或提交时失效，不把旧句译文提交到新句。

## 翻译与隐私

词语使用随包英文词表，30 个词附手工编写的用法示例；未经过专业审校。至少三个中文字且没有词表释义的当前候选，交给 Apple Translation 框架本机翻译，停止输入约 350 毫秒后开始。框架优先使用低延迟策略（macOS 26.4 以上）。没有接入云端翻译接口，不记录输入内容、不读剪贴板、不采集宿主上下文；下载语言资源需要联网。系统框架可能收集语言、应用标识和性能等非文本统计，见 [Apple Translation 文档](https://developer.apple.com/documentation/translation/translating-text-within-your-app)。本应用不声称关闭操作系统自身的统计。

翻译只针对选中的中文候选，不自动纠正同音错字，也不利用前文上下文。很长的拼音可能需要多次选择中文片段；翻译失败或未准备时保留中文选择和重试入口。

## 开发与检查

从本目录执行：

```sh
bash tools/build.sh
bash tools/check.sh
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-sentence
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --setup-translation
build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --sentence-integration-test
../runtime/python/bin/python tools/install.py
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --register
```

构建使用项目 Rust 运行时、既有锁文件和 Apple Command Line Tools，不需要完整 Xcode；Translation 框架弱链接，旧系统跳过整句功能，旧系统运行仍待实测。检查包括 Swift 格式、类型检查、Rust 格式与 Clippy、8 项真实引擎回归、随包资源自测、翻译取消与过期结果检查、属性和本地签名验证。整句联调需 macOS 26 和本地语言；预览与联调不能代替真实宿主输入验收。

`install.py` 只替换相同 bundle ID 的实验版，旧包保留在 `../evidence/native-backups/`，安装记录为 `../evidence/native-install.json`。安装与注册不主动切换输入源。更新已运行的服务时先切到 ABC，停止该实验版进程、安装并注册，再选择实验输入源；必要时从已安装路径启动服务。不要同时运行构建目录的普通 IMK 服务和已安装服务。

```sh
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --sources
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select
"$HOME/Library/Input Methods/BilingualCompanion.app/Contents/MacOS/BilingualCompanion" --select-id com.apple.keylayout.ABC
```

测试后恢复测试前输入源；本次用户已经使用实验输入法，不沿用 0.1 安装记录中的旧简体拼音恢复目标。`--disable` 仅用于主动停用，保留应用与资源。

## 状态和许可

0.3 已在本机完成构建、本地系统翻译、真实引擎拼音消耗和原生预览按钮检查，并安装更新。自动界面工具的测试按键绕过输入法，连自带简体拼音也只输出原样字母，因此没有据此宣称真实键盘、宿主焦点或跨应用验收完成。具体证据和待验证项见 [QA.md](QA.md)。

当前候选最多64项；尚无整句多译法、学习记录、词库更新、持久设置或跨平台接入。沿用 GPL-3.0-or-later。青简源码保持原样，不使用其商标、图标或外观；来源说明随包保留为 `GLOSSARY-NOTICE.md` 和 `LEXICON-NOTICE.md`。这是本地实验包，未做发行签名、公证或公开发布审核。

不再试用时，先切回系统输入法，在键盘设置移除实验输入源，再把此应用移入废纸篓，保留其他输入法。
