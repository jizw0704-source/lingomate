# 第三方代码、数据和资源

| 来源 | 固定版本 | 用途 | 许可说明 |
| --- | --- | --- | --- |
| [青简](https://github.com/qingjian-team/qingjian) | c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e | 中文候选引擎、词典和英文词表 | 代码 GPL-3.0-or-later；词表 README 标称 GPL-3.0-or-later，部分打包元数据写 MIT，发行前仍需澄清 |
| [rime-translate](https://github.com/daocatt/rime-translate) | c5ffa0ed2fa2a5530a39929fc356dadd4cea161c | 离线对照研究，不作为原生运行依赖 | 插件 MIT；样例数据按各自说明，不据插件许可推断全部数据许可 |
| Apple Translation | 系统框架 | 本机整句中译英 | 不在仓库内分发框架或语言模型，首次语言资源由系统下载 |

准备脚本下载固定上游到被忽略的 `upstream/`；原生构建从上游复制词典和词表至本地应用，并附带 `LICENSE`、`GLOSSARY-NOTICE.md` 和 `LEXICON-NOTICE.md`。源码仓库不包含这些生成应用或完整词典副本。

本项目不使用青简产品名称、logo和外观，也不分发 MiSans 字体；界面允许回退到系统字体。30个词的用法说明是实验示例，未经过专业审校。

更详细的来源证据保存在 [历史调研](../RESEARCH.md)。源码公开、软件许可和正式二进制分发是不同状态；尚未形成完整数据分发许可结论。
