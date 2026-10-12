# 灵果 · 协作维护

## 平台分工

| 维护范围 | 主要维护者 | 工作内容 |
| --- | --- | --- |
| macOS | [项目发起人](https://github.com/jizw0704-source) | 输入法客户端、候选与设置界面、安装与更新、Mac 实机验证 |
| Windows | [Wright](https://github.com/LightningFlashEvE) | 文本服务框架（Text Services Framework，TSF）客户端、候选与设置界面、安装与更新、Windows 实机验证 |
| 共享部分 | 双方协作 | 拼音引擎、候选排序、个人记忆、词库与许可、接口兼容及发布文档 |

macOS 主要位于 `native-macos/`，Windows 主要位于 `native-windows/`。共享桥接位于 `prototype/bridge/`，固定数据来源位于 `data/`，数据构建和核验工具位于 `tools/`。修改前阅读根目录及目标目录的 `AGENTS.md`，保留原有依赖锁文件和固定上游版本。

## 开发与合并

Windows 日常开发使用 `codex/windows-dev` 分支，所有 Windows 相关修改通过 Pull Request 请求合并到 `main`，不直接推送到 `main`。PR 的目标分支（base）为 `main`，来源分支（head）为 `codex/windows-dev`；项目创建者或有权限的维护者审核后合并。未经用户明确要求，不自动合并 PR。

每次 PR 聚焦一项修改，说明具体改动、测试结果和实际验证范围；已有未提交改动只在属于本次任务时纳入提交。PR 合并后，在工作区干净时将最新 `origin/main` 合并回 `codex/windows-dev`，再开始下一项修改。macOS 和共享改动可使用各自的功能分支，例如 `codex/macos-<功能名>` 或 `codex/shared-<功能名>`；共享改动建议由另一平台维护者复核。

共享请求／响应字段、个人词库格式、词库转换与排序规则变更，需要同时检查两端兼容性。平台特有问题可以先在对应客户端修复，并在说明中记录另一平台的影响。版本号分别维护，使用相同的功能名称与验证用例对照进度。

### macOS

在 Apple 芯片 Mac 的仓库根目录执行：

```sh
bash tools/setup.sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
```

安装、备份与输入源恢复见 [Mac 使用指南](native-macos/README.md)和[安装登记记录](native-macos/INSTALLATION.md)。实际输入结果记录在 [Mac 验证记录](native-macos/QA.md)。

### Windows

在 Windows 11 x64、64 位 PowerShell 的仓库根目录执行：

```powershell
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
```

开发环境需要 Python 3.10+ 和 MSVC 构建工具；脚本固定 Rust 1.96.0。制作安装 EXE 还需要 Inno Setup 6：

```powershell
./native-windows/tools/build-installer.ps1
```

使用与开发准备见 [Windows 使用指南](native-windows/README.md)。仓库现有 Windows 自动检查覆盖相关源码的推送和 Pull Request；实机结果记录在 [Windows 验证记录](native-windows/QA.md)。

## 当前实机验证重点

Mac 0.12.9 已由项目发起人在 Codex 中确认逐字输入后的“钙钛矿”和“钙钛镍”靠前整词候选。Windows 0.1.8 已通过原生构建与自动检查，实机试用由 Wright 推进。

两端采用同一组用例核对：中文与英文候选提交、中文夹英文的回车行为、逐字组词候选、重启后记忆保留、英文词性与本地学习记录。Windows 还应核对首次安装、浏览器／微信／钉钉输入、缩放、升级与旧版恢复。反馈包括版本、系统、应用、操作步骤、预期和实际结果，不附聊天正文或个人词库。

自动检查、界面预览和实体键盘验证分别记录。新增测试结果明确对应平台、应用及用例，实际使用表现以各平台反馈为准。

## 安装包与发布

内部试用包及对应源码位于 [GitHub Releases 草稿](https://github.com/jizw0704-source/lingomate/releases)，制作与交付规则见 [安装包说明](docs/PACKAGING.md)。每个包同时保留校验文件、匹配源码和试用说明；Mac 包的源代码覆盖项以随附 `SOURCE-SNAPSHOT.json` 为准。

公开发布、发行签名与远程更新清单另行推进。共享词库、词频和词性数据变更继续核对固定来源、许可与可复现构建，见 [第三方来源](docs/THIRD_PARTY.md)。个人数据、凭据、运行时和生成安装包保存在 Git 忽略目录。
