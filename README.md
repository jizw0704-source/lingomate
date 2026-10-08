# 中英输入实验版 · Bilingual IME

中文拼音输入时，同时选择中文和英文：词语支持展开更多译法；整句使用 Apple 本机翻译。当前版本 **0.5.0**，macOS 为第一个原生试用平台，保留网页原型供交互实验。

项目处于本地实验阶段。构建、真实引擎、系统本机模型及候选视图已检查；真实键盘、跨应用焦点、重登录加载及翻译质量仍待验证。没有把预览或自动脚本成功当作完整系统验收。

## 使用

在输入源中选择“中英输入实验版”，输入拼音：

- 空格输出中文；Shift＋空格输出选中词语译法或已完成的整句英文。
- 每页5个候选，数字1–5选择当前页；↑↓在当前页切换。
- PageUp / PageDown、`-` / `=` 或上一页 / 下一页按钮翻页。
- Tab 展开词语译法与用法；F6切换普通中文 / 中英候选。
- 标点自动跟随确认输出的中英文；候选窗、输入源菜单或 Control＋Shift＋P 可手动切换。
- 已确认选过的词会优先出现并标记“记忆”；个人词库重启后保留。

示例：`shi` 可产生64个候选、13页；`wojintianxiangxuexiyingyu` 可显示“我今天想学习英语”和 `I want to learn English today.`。候选数量随输入变化，桥接层最多返回64项，一次组合输入最多240个拼音字符。

[完整使用指南](native-macos/README.md) · [验证记录及待测项](native-macos/QA.md) · [项目进展](docs/PROGRESS.md) · [历史开源调研](RESEARCH.md)

## 开发

当前自动构建面向 Apple Silicon macOS。中文与词语释义的部署目标为macOS 13；整句翻译运行需要macOS 26及中英文语言。编译需要包含 Translation API 的新 SDK；本机以 Apple Command Line Tools 27验证，其他工具链尚未验证。先安装 Command Line Tools、uv、pnpm；准备脚本将 Rust 1.96.0 与固定上游快照放入忽略的本地目录。

```sh
git clone https://github.com/jizw0704-source/bilingual-ime.git
cd bilingual-ime
bash tools/setup.sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
native-macos/build/BilingualCompanion.app/Contents/MacOS/BilingualCompanion --preview-paging
```

准备脚本不覆盖有改动或提交不符的上游。预览不接收宿主键盘、不安装输入源。安装和试用见原生指南，安装前请了解此实验包的本地签名与系统授权要求。

网页原型采用 TypeScript / Vite、Python 本地服务和共享 Rust 桥接器；只显示前5项及词语释义，整句翻译与分页本次接入原生版。

```sh
pnpm --dir prototype build
pnpm --dir prototype serve
```

访问 http://127.0.0.1:9037 。[网页开发及回归命令](prototype/AGENTS.md)

## 架构与数据

`native-macos/`：Swift / AppKit / InputMethodKit 壳、候选分页、本机整句翻译与语言准备窗口。

`prototype/bridge/`：青简 Rust 适配器，验证候选与词语译法，保留剩余拼音。生成的整句英文来自本地系统任务，用已验证中文候选消耗拼音后提交，不允许客户端任意英文绕过词表校验。

`prototype/data/`：30个词语及人工编写的多译法用法示例。`tools/`：固定上游准备、离线研究与项目笔记同步脚本。上游、运行时、模型、构建产物与个人安装记录不提交到仓库。

选词记忆只在本机保存已确认的中文词、拼音、使用次数及最近选择顺序，不保存逐次输入过程或英文译文，不读剪贴板或宿主上下文，不接入云端翻译接口。首次系统语言下载需要联网；Apple 框架自身的非文本统计行为见其文档和原生指南。

## 许可

本项目采用 **GPL-3.0-or-later**，见 [LICENSE](LICENSE)。依赖青简固定提交，保留其版权及数据来源说明；不使用其产品商标或品牌资产。[第三方来源与尚待澄清的数据许可](docs/THIRD_PARTY.md)。这不是发行签名、公证或公开二进制发布。

## Obsidian

当前笔记保存在既有知识库的 `10 项目/中英输入法/`。此为按需文档镜像，源码以 GitHub 仓库为主；不会建立后台同步或覆盖手写笔记。

```sh
runtime/python/bin/python tools/sync_obsidian.py --vault '/path/to/Obsidian Vault'
```

脚本只更新专门标记的镜像文件，复制使用指南、项目进展、验证记录与历史调研；新建项目总览包含仓库入口和文档链接。
