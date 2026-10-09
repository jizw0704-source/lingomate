# 第三方代码、数据与资源

本文记录项目使用或研究的第三方资源、固定版本与许可信息。第三方名称仅用于来源归属，不代表产品品牌或合作关系。

| 来源 | 固定版本 | 用途 | 许可说明 |
| --- | --- | --- | --- |
| [青简](https://github.com/qingjian-team/qingjian) | c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e | 中文候选引擎、词典与英文释义表 | 代码为 GPL-3.0-or-later；词表 README 标注 GPL-3.0-or-later，部分打包元数据标注 MIT，正式分发前需澄清 |
| [rime-translate](https://github.com/daocatt/rime-translate) | c5ffa0ed2fa2a5530a39929fc356dadd4cea161c | 前期离线对照研究，不作为原生运行依赖 | 插件为 MIT；样例数据依据各自许可说明判断 |
| Apple Translation | 系统框架 | 本机整句中译英 | 仓库不分发框架或语言模型；语言资源由系统下载 |

Windows 原生接入使用 Microsoft 的 [`windows-rs`](https://github.com/microsoft/windows-rs) 平台绑定（`windows` / `windows-core` 0.62.2，MIT OR Apache-2.0），仅在 Windows 编译时启用。TSF 接口及组合输入处理参考固定上游源码与 Microsoft 系统文档，使用本项目独立标识及界面，不复用第三方图标或输入日志 / 上下文采集功能。

## 构建与分发范围

Windows 安装 EXE 使用 [Inno Setup 6](https://jrsoftware.org/isinfo.php) 编译（仅构建工具，不作为输入服务运行依赖）。主要流程的中文文本由本项目编写，少见系统诊断回退到编译器的英文消息；项目不复制第三方中文翻译文件或自动下载编译器。使用及发行按 Inno Setup 官方许可执行；官方请求商业用户购买许可，当前 CI 为开发检查。该工具许可不代替运行词库的数据分发确认。

准备脚本将固定上游快照下载至被版本控制忽略的 `upstream/` 目录。原生构建将所需词典与释义表复制至本地应用，并附带 `LICENSE`、`GLOSSARY-NOTICE.md` 和 `LEXICON-NOTICE.md`。源码仓库不包含生成的应用或完整词典副本。

项目不使用第三方产品商标、标志或界面资产，也不分发 MiSans 字体；界面支持系统字体回退。项目编写的 30 个词语用法示例尚未经过专业审校。

本项目源码采用 GPL-3.0-or-later。源码公开不代表所有数据的二进制分发许可已完成核验；完整数据分发结论仍待明确。详细来源证据见 [历史调研](../RESEARCH.md)。
