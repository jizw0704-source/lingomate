# 第三方代码、数据与资源

本文记录当前分发来源与旧版审计证据。**macOS 0.12.1、Windows 0.1.3 起，基础词库与英文释义统一改用固定 CC-CEDICT 数据；旧混合词库、语料词频及旧释义表不再作为构建输入。** 数据改编按 CC BY-SA 4.0 提供，代码仍为 GPL-3.0-or-later。第三方名称仅用于来源归属，不代表产品品牌或合作关系。

| 来源 | 固定版本 | 用途 | 许可说明 |
| --- | --- | --- | --- |
| [青简](https://github.com/qingjian-team/qingjian) | c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e | 中文候选引擎代码 | GPL-3.0-or-later；保留固定源码引用，当前不使用其中文字典、释义及语料生成数据 |
| [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict) | 2026-10-09T09:12:50Z；镜像 8359384e656a3437b2cca08f13c092745db8d3e4 | 中文词面、拼音及英文释义 | 原始文件头与发布页明确 CC BY-SA 4.0；同许可提供改编数据、保留署名与修改说明 |
| [Google Books Ngram](https://books.google.com/ngrams/info) | 简体中文 unigram 20200217，出版年份 1980–2019 | 已有词面的基础排序词频 | 官方许可问答明确数据可用于任何用途；保留来源及处理说明，标识 LicenseRef-Google-Books-Ngram |
| [rime-translate](https://github.com/daocatt/rime-translate) | c5ffa0ed2fa2a5530a39929fc356dadd4cea161c | 前期离线对照研究，不作为原生运行依赖 | 插件为 MIT；样例数据依据各自许可说明判断 |
| Apple Translation | 系统框架 | 本机整句中译英 | 仓库不分发框架或语言模型；语言资源由系统下载 |

Windows 原生接入使用 Microsoft 的 [`windows-rs`](https://github.com/microsoft/windows-rs) 平台绑定（`windows` / `windows-core` 0.62.2，MIT OR Apache-2.0），仅在 Windows 编译时启用。TSF 接口及组合输入处理参考固定上游源码与 Microsoft 系统文档，使用本项目独立标识及界面，不复用第三方图标或输入日志 / 上下文采集功能。

## 构建与分发范围

Windows 安装 EXE 使用 [Inno Setup 6](https://jrsoftware.org/isinfo.php) 编译（仅构建工具，不作为输入服务运行依赖）。主要流程的中文文本由本项目编写，少见系统诊断回退到编译器的英文消息；项目不复制第三方中文翻译文件或自动下载编译器。使用及发行按 Inno Setup 官方许可执行；官方请求商业用户购买许可，当前 CI 为开发检查。该工具许可不代替运行词库的数据分发确认。

准备脚本将引擎固定快照下载至忽略的 `upstream/`。`tools/prepare_lexicon.py` 独立下载并验证 `data/source.json` 中的 CC-CEDICT 快照，以及 `data/frequency-source.json` 固定的 Google Books 统计；原始数据缓存和生成表均被 Git 忽略。原生构建只复制新生成的 `dict.tsv` 与 `glossary-en.tsv`，附带 GPL `LICENSE` 及包含 CC 数据署名、完整原始文件头、输入／输出摘要、修改说明和许可全文的两份 `NOTICE`。安装、更新文件名不变，Windows 校验清单继续覆盖声明文件。

项目不使用第三方商标、标志、字体或界面资产。独立的 30 项词语用法示例按 GPL-3.0-or-later 提供，未经过专业审校；它们不并入 CC-CEDICT 释义表。以下旧数据审计不适用于新生成数据，也不代表旧包可以公开分发。

## 当前词库来源与替换结果

CC-CEDICT 官方下载与固定镜像解压后逐字节一致，原始文件 SHA-256 为 `9836ed4ad048d3acd915c9bcb67dc6cfd173c371f78f02072b142767bba0e722`。镜像只用于取得相同数据，未使用其 Apache 许可的 Go 实现。发布页及文件内均为 **CC BY-SA 4.0**；旧 Wiki 首页的 3.0 描述不能代替此次文件的明确声明。公开原始输入见[固定镜像文件](https://github.com/rhcarvalho/cedict/blob/8359384e656a3437b2cca08f13c092745db8d3e4/cedict_1_0_ts_utf-8_mdbg.txt)，条件见 [CC BY-SA 4.0 正式文本](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en)。

| 数据 | 数量 | 许可及处理 |
| --- | ---: | --- |
| 原始 CC-CEDICT | 125,244 个源条目 | 保留发布者、社区作者、原始 CEDICT 版权、原文件头、固定快照和摘要 |
| `dict.tsv` | 120,662 个中文词面，121,377 条读音 | CC BY-SA 4.0；仅用本快照简体词面与读音生成，不导入旧词库或独立规范字表 |
| `glossary-en.tsv` | 107,757 个词面 | CC BY-SA 4.0；同一源文件的完整英文义项，去重并过滤引用型元信息，不复用旧 DeepSeek 释义 |
| `data/ranking.json` | 项目独立编写的排序设置 | 数据贡献按 CC BY-SA 4.0；其数值为基础排序权重，不是外部语料的真实频率 |
| `details.json` | 30 项项目独立示例 | GPL-3.0-or-later，单独提供，不改变 CC 表的许可 |

允许按 CC 条件用于商业及非商业分发，要求保留署名、许可与修改说明，改编数据按相同许可提供，不额外限制接收者对数据的许可权利。代码的 GPL 条件单独适用，二进制发行仍需提供匹配版本的对应源码及构建资料。

转换仅处理纯汉字简体词面，限制 32 字；拼音去声调、转小写，`u:`／`ü` 转 `v`，儿化 `r` 转 `er`；同词同读音去重；过滤非汉字词面、未知或不可解析读音，以及不适合直接提交的英文引用说明。本次过滤 820 个源条目，其余合并为上述数量。默认降低专名的排序权重。0.12.2 / 0.1.4 起，将许可明确的官方统计与已有 122 个独立日常先验结合，不使用旧词表、旧词频或原始输入日志。`provenance.jsonl` 分别记录源行号、原始统计次数、独立先验及最终权重，生成清单保留两类原始来源和排序设置摘要。

离线英文条目从旧表的 232,213 项调整为 107,757 项；源头更明确，但覆盖及义项呈现有所变化。CC-CEDICT 是词典释义，不是上下文整句翻译。原有独立例句和个人记忆保留；缺少释义的较长候选仍可按既有条件使用 Apple 或用户主动启用的 MiniMax。基础排序需要实际试用继续调整，有限回归不等同于完整词库质量验收。

## 候选排序与词频来源

macOS 0.12.2 / Windows 0.1.4 共用以下基础排序；个人记忆目前由 macOS 客户端接入，Windows 仅验证共享桥接的隔离记忆逻辑。

- 完整拼音匹配的词库词优先于自动拼接句；完整原样读音优先于拼写纠错结果。简拼、前缀及其他纠错路径继续使用引擎排序。
- 基础权重来自官方 Google Books 简体中文 unigram **20200217**，仅匹配固定 CC-CEDICT 已有完整词面，排除带词性标签条目。累计出版年份 **1980–2019** 的 match_count，53,957 个词面得到统计，共 8,802,962,337 次。最大累计次数缩放为 1,000,000，至少为 2；专名读音降为四分之一。未观测读音使用原有 10 / 专名 1 的默认值。
- 叠加已有 122 个独立编写的日常先验权重，最终不超过 1,000,000。它们不称为语料词频；保留这部分可避免书面语统计将“我想学习英语”排成“我向学习英语”。两类数值分别保存在来源记录中。
- 本机确认后的选词在相同读音组内加权，不跨组挤掉完整词句。使用次数加分上限为 20，按后续确认选词数递减，256 次后回到基础排序；不按日期衰减，也不删除词条。确认收据、单次消费和剩余拼音校验保留。

固定文件 SHA-256 为 `6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86`，大小 60,833,224 字节；[官方下载索引](https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html)和 [Google 官方使用许可](https://books.google.com/ngrams/info)已核对。官方明确表示数据可用于任何用途，项目保留署名、链接、版本与加工说明，以 **LicenseRef-Google-Books-Ngram** 表示该许可，不将原始数据误称为 MIT、Apache 或 CC。合成 CC-CEDICT 的改编词库仍按 CC BY-SA 4.0 提供。[完整来源声明](licenses/GOOGLE-BOOKS-NGRAM-NOTICE.txt)随两端原有 NOTICE 文件打包。

词频来源有书面语、出版年代、分词及 OCR 偏差，不代表聊天场景或个人习惯。没有导入书籍正文，也没有训练或加载上下文语言模型；整句仍以一元词频路径为基础。固定 30 组独立样例的合理词进入前五位从 26 / 30 提升为 30 / 30，首选从 18 / 30 提升为 28 / 30。这是有限工程回归，不能视为整体准确率；同音歧义及跨应用真实输入仍需验收。

### 重建与包内校验

```sh
runtime/python/bin/python tools/prepare_lexicon.py
runtime/python/bin/python tools/prepare_lexicon.py --check
runtime/python/bin/python tools/test_prepare_lexicon.py
runtime/python/bin/python tools/test_frequency.py
runtime/python/bin/python prototype/tests/ranking_test.py
runtime/python/bin/python tools/prepare_lexicon.py --check --package native-macos/build/BilingualCompanion.app/Contents/Resources
```

Windows 构建使用已安装的 Python 3.10+ 标准库执行同一脚本。生成表无需新增产品运行依赖，接收安装包的用户不需要 Python。构建前自动准备数据；两端完整检查同时验证新表、来源、声明和包内字节。错误摘要、错误许可、缺失声明或旧包数据均会拒绝通过；共享桥接缺失新数据时不会回退到旧上游目录。

词库来源缺口已通过独立替换处理，旧混合表的未确认来源不再进入新构建。此结论限于本节列出的新词库输入与生成物；旧 DMG／EXE／ZIP 和维护者草稿不会因源码更新而获得相同状态。发行签名、公证、对应源码交付及 Windows 11 实机验收仍应在开放下载前完成。

## 旧安装包与混合词库审计

以下为替换前的来源与审计记录，保留用于解释旧包为何没有开放下载。

2026-10-09 安装包上传核对：固定快照的字词库 README 列出规范字、liuxilu 通用词表与 THUOCL 等独立来源，并明确“不以单一许可证覆盖所有来源”；不能用仓库代码的 GPL 标识替代逐项数据许可。用户请求上传的完整安装包先保存在维护者可见的 GitHub Release 草稿中，不公开发布，且不启用更新清单。该存储动作不改变数据分发确认状态。

### 2026 年 10 月 10 日旧词库分发核对

结论：**尚不能将旧完整混合词库标记为已获准分发。** 主要缺口为常用词与规范字转录的授权证据，以及语料统计和抽取产物的来源与许可版本。THUOCL、Unicode、LCCC 的公开许可已找到；补充其声明不替代其他来源的授权确认。

### 旧版实际随包的数据

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

### 旧版审计提出的处理顺序

1. 补齐中文常用词、规范字转录、Unihan 与语料输入的版本及授权链；无法补齐的来源用许可明确的数据重新构建。保留逐来源清单、摘要及生成步骤。
2. 单独处理维基衍生部分的许可版本、归属及修改说明；同时检查英文释义表中的源词条。取得足够证据前不声明整张表为 MIT 或已完成分发许可核验。
3. 发包时提供该版本的对应源码下载及完整构建资料，包括固定上游与修改后的桥接；不能只以随包 GPL 文本或可移动的 main 分支链接替代对应源码。
4. 重新构建两端安装包，检查包内实际声明和数据摘要。既有 DMG、EXE、更新 ZIP 和维护者草稿尚未补入本轮新增全文；本机已安装应用也未因文档修改而变化。

本轮调整仅补全已确认的声明与记录核对结论，不改变候选数据、个人词库、运行中的输入服务、安装包发布状态或更新清单。完整数据分发继续保持待确认。
