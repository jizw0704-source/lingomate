# 第三方代码、数据与资源

本文记录项目使用或研究的第三方资源、固定版本与许可信息。第三方名称仅用于来源归属，不代表产品品牌或合作关系。

| 来源 | 固定版本 | 用途 | 许可说明 |
| --- | --- | --- | --- |
| [青简](https://github.com/qingjian-team/qingjian) | c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e | 中文候选引擎、词典与英文释义表 | 代码及释义表 README 标注 GPL-3.0-or-later；中文字词库 README 明确各来源分别适用许可，不以单一许可证覆盖；当前使用原始 TSV，按释义表 README 的 GPL-3.0-or-later 处理；不采用其他打包元数据中的 MIT 标记 |
| [rime-translate](https://github.com/daocatt/rime-translate) | c5ffa0ed2fa2a5530a39929fc356dadd4cea161c | 前期离线对照研究，不作为原生运行依赖 | 插件为 MIT；样例数据依据各自许可说明判断 |
| Apple Translation | 系统框架 | 本机整句中译英 | 仓库不分发框架或语言模型；语言资源由系统下载 |

Windows 原生接入使用 Microsoft 的 [`windows-rs`](https://github.com/microsoft/windows-rs) 平台绑定（`windows` / `windows-core` 0.62.2，MIT OR Apache-2.0），仅在 Windows 编译时启用。TSF 接口及组合输入处理参考固定上游源码与 Microsoft 系统文档，使用本项目独立标识及界面，不复用第三方图标或输入日志 / 上下文采集功能。

## 构建与分发范围

Windows 安装 EXE 使用 [Inno Setup 6](https://jrsoftware.org/isinfo.php) 编译（仅构建工具，不作为输入服务运行依赖）。主要流程的中文文本由本项目编写，少见系统诊断回退到编译器的英文消息；项目不复制第三方中文翻译文件或自动下载编译器。使用及发行按 Inno Setup 官方许可执行；官方请求商业用户购买许可，当前 CI 为开发检查。该工具许可不代替运行词库的数据分发确认。

准备脚本将固定上游快照下载至被版本控制忽略的 `upstream/` 目录。原生构建将所需词典与释义表复制至本地应用，并附带 `LICENSE`、`GLOSSARY-NOTICE.md` 和 `LEXICON-NOTICE.md`。2026-10-10 起，新构建的 `LEXICON-NOTICE.md` 还附加 THUOCL、LCCC 和 Unicode 许可全文及待确认项；文件名保持不变，两端更新校验清单继续覆盖声明文件。源码仓库不包含生成的应用或完整词典副本。

项目不使用第三方产品商标、标志或界面资产，也不分发 MiSans 字体；界面支持系统字体回退。项目编写的 30 个词语用法示例尚未经过专业审校。

本项目源码采用 GPL-3.0-or-later。源码公开不代表所有数据的二进制分发许可已完成核验；中文混合词库仍有来源授权及语料许可版本待确认，完整数据分发尚未通过核验。详细来源证据见 [历史调研](../RESEARCH.md)。

2026-10-09 安装包上传核对：固定快照的字词库 README 列出规范字、liuxilu 通用词表与 THUOCL 等独立来源，并明确“不以单一许可证覆盖所有来源”；不能用仓库代码的 GPL 标识替代逐项数据许可。用户请求上传的完整安装包先保存在维护者可见的 GitHub Release 草稿中，不公开发布，且不启用更新清单。该存储动作不改变数据分发确认状态。

## 2026 年 10 月 10 日词库分发核对

结论：**尚不能将当前完整词库标记为已获准分发。** 主要缺口为常用词与规范字转录的授权证据，以及语料统计和抽取产物的来源与许可版本。THUOCL、Unicode、LCCC 的公开许可已找到；补充其声明不替代其他来源的授权确认。

### 实际随包的数据

已核对两端构建脚本、共享桥接的 `Adapter::load` 和固定上游 `Engine::new`。两端使用同一份 `dict.tsv` 和 `glossary-en.tsv`，另附本项目 30 项用法示例 `details.json`。主引擎没有加载上游独立 bigram 或神经模型，但 **dict.tsv 内已有语料生成的词频及抽取词条**，仍属于核对范围。

| 随包文件 | 非注释条目 | SHA-256 |
| --- | ---: | --- |
| `dict.tsv` | 92,825 | `7787d6d219674dd51297652456c55ea2d49a65b5e571d2309a62add0ec2d2279` |
| `glossary-en.tsv` | 232,213 | `7b9676979aa227bde7a2d541354a59b73a04f33acd36ef5d6c433ce789b81625` |
| `details.json` | 30 个词语 | `bb13096adfcfe79c5a7e8af4e760dd42805b55018442d5b6f7a612edee3ea332` |

这些摘要对应固定快照与当前项目示例，后续更换数据应重新核对。上游的 20 万级全量统计不等于当前基础包的条目数。未随包的内容包括独立领域词库、英文补全表及 wordfreq、日语／西班牙语／英译中释义、五笔、笔画、表情数据及 `.qj` 词典容器；rime-translate、ECDICT 和 CC-CEDICT 亦不属于当前原生包。

### 来源与待调整项

| 数据来源 | 已取得证据 | 处理结论 |
| --- | --- | --- |
| [THUOCL](https://github.com/thunlp/THUOCL/blob/master/LICENSE) 领域词 | 固定上游保留 MIT 全文，版权人为 THUNLP；公开原始许可一致 | 可依 MIT 条件使用及分发，保留版权和许可全文。两端新构建已补带全文；不由此推定整张混合表为 MIT |
| [Unihan / Unicode](https://www.unicode.org/license.txt) 读音 | 固定上游说明采用 Unicode License V3；官方允许数据分发并要求保留声明 | 已补带固定上游现存 V3 文本及核对日官方 V3 文本。具体 Unihan 下载版本和输入摘要未留存，仍需补证；不声称表情目录的声明即为实际读音输入的原始附件 |
| [liuxilu 常用词表](https://github.com/liuxilu/Proofread-Modern-Chinese-Common-Lexicon/tree/52ae8e979af1c956939dd4531827bc7d3f980d10) | 校对版来自《现代汉语常用词表（草案）》；核对仓库树、说明，未找到明确许可 | 需取得涵盖原始词表及校对版本的再分发依据，或替换来源。公开可下载及“做码表”的提示不足以形成完整授权链；不据此断定使用必然违法 |
| [iDvel 规范字转录](https://github.com/iDvel/The-Table-of-General-Standard-Chinese-Characters/tree/76c8512624d566ce0c6180948c9a02a4d9b1e0cf) 与 [shengdoushi 分级表](https://github.com/shengdoushi/common-standard-chinese-characters-table/tree/d9b599a9c9cc0dd2d58cad829e285bc780cd4451) | 固定上游仅取字形和分级；核对两仓库未找到许可。iDvel 的注音另有来源，但当前并未直接采用 | 需明确官方规范数据与转录成果的可用范围，或从许可明确的原始来源重新构建；不把未采用的注音许可套到字形数据 |
| [LCCC](https://huggingface.co/datasets/thu-coai/lccc#licensing-information) 语料 | 数据集卡明确给出 MIT 全文，版权人为 lemon234071 | 已补带其全文；具体语料输入版本、抽取步骤与产物对应关系需补齐，不复制原始对话到分发包 |
| [中文维基数据集](https://huggingface.co/datasets/wikimedia/wikipedia/blob/main/README.md) 词频及抽取产物 | 固定上游声明使用 `20231101.zh`、标注 CC BY-SA 4.0；核对日数据集卡标注 CC BY-SA 3.0 / GFDL | 存在版本不一致。需确认实际输入快照、适用许可、归属及修改说明，并明确词频数字和抽取词条各自范围；当前不擅自将整表重新许可为 4.0，也不认定统计数据当然不受条件约束 |
| 上游英文释义 | 固定快照 `assets/glossary/README.md` 明确 GPL-3.0-or-later，并说明 DeepSeek 离线生成 | 按该声明保留 GPL 及来源说明；二进制发行须提供匹配版本的对应源码和构建资料。机器生成译文不消除中文源词条的待确认项；其他 `.qj` 元数据不作为改用 MIT 的依据 |
| 本项目用法示例 | `prototype/data/details.json` 中 30 项，源码随 GPL-3.0-or-later 提供 | 保留项目许可；准确性仍需人工审校 |

转录仓库的核对提交是 **2026-10-10 在线证据快照**，不是已证明的原始导入版本。固定上游没有为每个合并词条记录完整来源、变换和许可；不能通过删除看似属于单一来源的词条就证明其余词库已获授权。生成说明见固定快照的 [`QINGJIAN.md`](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/assets/lexicon/QINGJIAN.md) 和 [`landscape.md`](https://github.com/qingjian-team/qingjian/blob/c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e/docs/design/landscape.md)。

### 分发前的处理顺序

1. 补齐中文常用词、规范字转录、Unihan 与语料输入的版本及授权链；无法补齐的来源用许可明确的数据重新构建。保留逐来源清单、摘要及生成步骤。
2. 单独处理维基衍生部分的许可版本、归属及修改说明；同时检查英文释义表中的源词条。取得足够证据前不声明整张表为 MIT 或已完成分发许可核验。
3. 发包时提供该版本的对应源码下载及完整构建资料，包括固定上游与修改后的桥接；不能只以随包 GPL 文本或可移动的 main 分支链接替代对应源码。
4. 重新构建两端安装包，检查包内实际声明和数据摘要。既有 DMG、EXE、更新 ZIP 和维护者草稿尚未补入本轮新增全文；本机已安装应用也未因文档修改而变化。

本轮调整仅补全已确认的声明与记录核对结论，不改变候选数据、个人词库、运行中的输入服务、安装包发布状态或更新清单。完整数据分发继续保持待确认。
