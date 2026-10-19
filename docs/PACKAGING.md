# 安装包与同事试用

提交前整合主线PR #11后已重建0.1.21运行包，14文件清单SHA-256为 `13A6D51882A33FC8182D382B96B1F8CE131A96EEA529F4ACC7F44DAD453F7C7D`，完整 `check.ps1 -SkipUpdates`通过。下文Setup EXE是在整合PR #11前生成的保留产物，其原摘要仍有效；未重新制作或上传Setup，不宣称旧EXE包含当前Git提交全部共享源码。安装／实机验收及公开发布均独立，记录见[QA](../native-windows/QA.md)与[Issue #15](https://github.com/jizw0704-source/lingomate/issues/15)。

2026-10-14源码审核：用户要求先推送Windows 0.1.21开发源码并请求合并。以下“未提交／推送”属于产包时的阶段记录，安装包仍只保存在本地；本次PR不公开二进制、更新源、真实配置或凭据，也不代表安装／实机验收完成。

## Windows 0.1.27–0.1.28 钉钉验收本地包 · 2026-10-19

两个包都用 `build.ps1 -WordNetSource runtime/wordnet-3.0-pinned.bin` 生成，并在已确认为真实视图的终端中用 `install.ps1` 安装。安装后核对：14 个文件、句柄物理路径、HKCU／HKLM 登记都一致，旧版本目录保留。

- 0.1.27：没有启用整句翻译服务时，不发请求，也不显示整句区块。
- 0.1.28：在 0.1.27 基础上，让 5 到 8 字的紧凑候选横向加宽、不折行。

钉钉实机验收见 [Windows QA](../native-windows/QA.md)。两个包都只在本机，没有制作或上传 Setup EXE，也没有发布。

| 产物 | 0.1.27 SHA-256 | 0.1.28 SHA-256 |
| --- | --- | --- |
| 14文件 `manifest.json` | `C0B40A21FFBDCBE7CC9C05B23ACCB9EE2705E3C8725879DFD4F70A60673E72E3` | `83EDAC0B7018770DC68A2B09FE9B4460D72516A81C1EF41EC5D3E207024CC2FB` |
| `lingomate_tsf.dll` | `89F5AF28084459BD44CDA3F75CB1084AF542FD88671FF74C08B1FE12D6097FB0` | `8B2BFDE28B630E24754BED198CEDA4AE7A6C87FDC2BC04FC229B699B285816AF` |
| `lingomate-desktop.exe` | `4A5223F67A64ED72C56CB71E1AA00D211DF2350B6A22B09B5B9D5FEEDB41CF5D` | `E9CED2FD0D34A72CCFA99D7AA9EE652346B4B18853E4AFE0A909831AEF0B641C` |

## Windows 0.1.21 API反馈与候选重试本地包 · 2026-10-14

最终包包含API密钥保存掩码、待保存状态与独立连接测试，以及候选翻译队列繁忙后的等待重试、helper异常独立提示。已有凭据与旧版本保留。0.1.20中间安装包保留；0.1.21尚未安装／登记、提交／推送或发布。126项Windows Rust、格式／all-targets Clippy、MSVC及14文件manifest校验通过，真实设置按钮单次MiniMax检查2.6秒成功；实际候选／截图故障及既有COMLESS未通过的边界见[Windows QA](../native-windows/QA.md)。

| 产物 | SHA-256／大小 |
| --- | --- |
| 14文件 `manifest.json` | `A021A6BDFFE26F22786AA96614F2F3A88C922C68B15C677311CDDBD5780698CB` |
| `lingomate-desktop.exe` | `72CAC42B49576C38EA345C921454BE3815606B403E27FE4217762120BDD5D585` |
| `lingomate_tsf.dll` | `74F44142CB917D8870143FAACE0A51E899727FE29599A550A091EE7A14C7CDEB` |
| `native-windows/package/installer-0.1.21/lingomate-windows-x64-0.1.21-setup.exe` | `FC12A99F735792040464BA4430AE2B5F52DC782FEA4B08A4F0F1BDDCDB5E1419`，6,691,451字节 |

中间0.1.20安装EXE的SHA-256为 `1C3779485ABED74A8A9C5AF9D2637C9D5B77B0842B8D6EDA383A024227DA30D3`，6,691,769字节。该窗口已用于实际密钥状态与测试按钮检查；最终0.1.21程序 `--identity` 确认unpacked=true／version=0.1.21，不代表实际宿主键盘验收。

## Windows 0.1.19 设置修复本地包 · 2026-10-13

已生成本地14文件包及Inno安装EXE，修复设置页滚动残影／输入框遮挡，把“准备本地整句翻译…”更名为“本机翻译设置”，明确AI启用与候选窗提交说明。已有配置、凭据及旧版本保留。0.1.18中间安装包保留但未安装；0.1.19尚未安装／登记、提交／推送或发布。自动检查与COMLESS未通过的边界见[Windows QA](../native-windows/QA.md)。

| 产物 | SHA-256／大小 |
| --- | --- |
| 14文件 `manifest.json` | `B8EE3632AB36B91427CE8634C733AD339C8E039CED6424ACF55AF63E678C6F3F` |
| `lingomate-desktop.exe` | `CA9B13EBE5E61FD45A2F8B6A14AC449DCDC1D4FB26D0F393D59C9FEC06750653` |
| `lingomate_tsf.dll` | `FB020E6ABCFBF11F84FCF0B433E54A267BFA8B3BAB19647FA78EBF86865BFCE3` |
| `native-windows/package/installer-0.1.19/lingomate-windows-x64-0.1.19-setup.exe` | `B159C4CBC9E237FC61D2D9B67AB6E8CE666C70512BFE06A7C57F1BCF09A90671`，6,687,808字节 |

## Windows 0.1.17 对齐开发包 · 2026-10-13

本地MSVC release和Inno编译完成，当前包包含独立设置／翻译helper、五页设置、默认OFF的API句译、账号词条客户端、受验证的本机模型准备及Mac候选／快捷键对齐。自动检查和隔离界面结果见[Windows QA](../native-windows/QA.md)与[实施记录](WINDOWS-MAC-PARITY-IMPLEMENTATION-2026-10-13.md)。**本机安装文件、HKLM登记和设置程序直接启动已核验，现有Chrome／钉钉／ChatGPT仍加载0.1.16，新版入口及真实键盘待验；未提交、推送或上传发布**。旧0.1.16文件保留。

| 校验对象 | 当前结果 |
| --- | --- |
| 14文件 `manifest.json` | SHA-256 `E4C662244541043F2DEF1D39917CB9C80D40EDC06FADD5F3D628BEB04EB42B6B` |
| `lingomate-desktop.exe` | SHA-256 `5BA6EE81E15A202D6B548095338F199FAA698C74B8F2D2203B8310B188CA4C06` |
| `lingomate_tsf.dll` | SHA-256 `65B3C0E6A55299A648C20FD2D6436034429E71EBA9265582D24FA4F028918A4F` |
| Inno安装EXE | `native-windows/package/installer-0.1.17/lingomate-windows-x64-0.1.17-setup.exe`，6,686,864字节 |
| 安装EXE SHA-256 | `DBC67F392A266C3FC318D58C90573DF1FDF0633055C97992C808735C7DF7BF25` |

安装前与安装完成两份试用说明已更新新功能、在线AI默认关闭、服务配置及实机验收边界；首轮未更新说明的EXE保留在ignored `package/installer-0.1.17-before-notes/`，不作为当前试用包。Inno成功不等于安装／UAC／卸载／恢复已验收，也不表示取得发行签名。当前更新清单和已有草稿保持原状。

只有确认本地测试后才安装和执行真实键盘用例，记录包摘要、登记、无包身份helper、Chrome／钉钉／grokbot／ChatGPT或Codex的实际上屏、DPI／鼠标及旧版恢复。API、公开邮件账号、本机模型和读屏／真实多屏分别验收，不以调试器身份probe或隔离截图代替。

## Windows 0.1.16 包外安装验收与显示修复 · 2026-10-13

在没有应用包身份的终端运行 `install.ps1`（同账号 UAC）后，用户确认 Chrome、钉钉、grokbot、ChatGPT／Codex 的输入与上屏正常。

显示修复后的新包安装到 `%LOCALAPPDATA%\LingoMate\Windows\0.1.16-64BC6D58515B`：HKCU／HKLM 指向同一个 DLL，`ThreadingModel=Apartment`，`0.1.16-52F90B953BB3` 保留。用户已在 Chrome、钉钉、ChatGPT 和 Cursor 中确认显示正常。

| 校验对象 | SHA-256 |
| --- | --- |
| `manifest.json` | `64BC6D58515BC6BC729781AD3011D26805643A45AA1F9AF26A00D93C80AE514A` |
| `lingomate_tsf.dll` | `D15A219027BC3E9249E3879A56019F1ABD0673B8291404C110CCAE6CF40DECFA` |

版本号仍是 0.1.16，与上一次构建同号不同内容；正式交付前需要决定是否升版本。Inno 安装 EXE 本轮没有重新编译，实际 Setup EXE 的安装和卸载流程仍未验收。新包只在本机，没有提交、上传或发布。验证与限制见 [Windows QA](../native-windows/QA.md)。

## Windows 0.1.16 重启后验收未通过 · 2026-10-13

本机 0.1.16 安装与两处登记保留原状态，未发布。用户正常重启后仅 ChatGPT 有拼音／候选，Chrome／钉钉／grokbot 无候选且只出字母；Windows 搜索栏切到灵果后仍使用微软输入法。以下安装／系统加载检查不替代这项失败反馈；不能交付为跨应用修复完成。调查见[根因检查](WINDOWS-ROOT-CAUSE-AUDIT.md)和[Windows QA](../native-windows/QA.md)。

## Windows 本机安装验证 · 0.1.16 · 2026-10-12

普通用户权限运行 `install.ps1`，在登记阶段经同一账号 UAC helper 成功安装到 `%LOCALAPPDATA%\LingoMate\Windows\0.1.16-52F90B953BB3`。HKCU／HKLM64 的自有 CLSID 均指向该目录的同一 DLL，`ThreadingModel=Apartment`，13 项包文件校验通过；旧版目录保留。安装器仍以普通权限启动，辅助进程只处理自有登记，不执行下载包中的脚本，不设置默认输入源、停止宿主或重启。

| 校验对象 | SHA-256 |
| --- | --- |
| `manifest.json` | `52F90B953BB3B59F1F5FE6D0F06968202A01330CF35DE515C74B85435C8035B3` |
| `lingomate_tsf.dll` | `195D0DF3E5D26A0C6061D2EFE84B2B8D6D6477F604706571236A939F804BD829` |

本轮构建、固定数据核验、84 项 Rust、48 个自有 TSF 阶段／192 轮鼠标循环、两种 PowerShell 各 134 项隔离安装检查及 Inno 编译通过。普通／COMLESS 的真实系统加载、空合成文档焦点和原线程 profile 恢复也通过；完整 Setup EXE 安装／卸载、实体键盘与真实跨应用验收分别记录，不能由上述结果推定通过。[根因与验证边界](WINDOWS-ROOT-CAUSE-AUDIT.md)、[Windows QA](../native-windows/QA.md)。

先前补 HKLM 登记后钉钉仍无候选、只出字母；驻留 ctfmon／TextInputHost 仍为 10 月 10 日启动，需要用户正常重启后复测 Chrome／钉钉／grokbot，不能保证重启即可解决。0.1.16 仅在本机，未提交、推送、上传或发布；维护者 Windows 0.1.10 草稿与未发布更新清单保留。没有新增 Mac 验证，Obsidian 路径未配置。

## Windows 宿主兼容补齐 · 0.1.15 · 2026-10-12

用户随后对 0.1.15 手测确认：钉钉仍使用微软输入法；grokbot（独立 Windows 软件）和谷歌浏览器只输出字母，无拼音候选。因此 0.1.15 未通过跨应用验收。真实 ThreadMgr／flags=0 的隔离系统加载探针已确认 Windows 可加载此 DLL、Foreground CLSID 匹配并成功恢复原线程 profile；它未模拟物理键盘，不代表上述应用正常。正在进一步区分应用未载入与激活后英文／焦点／literal 状态门禁。

用户确认 0.1.14 在 ChatGPT 中正常，但钉钉和 grokbot 实际输入仍是微软输入法。钉钉 9.0.1 的 x64 进程在重开后未驻留灵果 DLL；切换器与托盘提示不是实际接入输入线程的证据。只读诊断还发现：灵果虽已能被 EnumEnabledLayoutOrTip 枚举，当前用户 zh-Hans-CN 的 InputMethodTips 原先只列微软拼音；不能据此把所有失败断言为同一根因。

本轮对照固定源码：[微软 SampleIME](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/IME/cpp/SampleIME/SampleIME.cpp)、[Weasel](https://github.com/rime/weasel/blob/4aea1978a351e2ce41af20c8fb88a01ce9802de8/WeaselTSF/WeaselTSF.cpp)、[Mozc](https://github.com/google/mozc/blob/18e76511af9371a9a4409b1d13c3e30884b53e9c/src/win32/tip/tip_text_service.cc)。补齐激活时 GetFocus／GetTop、ITfThreadFocusSink、线程 OPENCLOSE／CONVERSION 双向同步和 GUID_LBI_INPUTMODE 语言栏；仅修改 NATIVE 位，保留其他模式位。队列满时保留 Finish → EnglishMode 边界，期间不接新键；候选窗创建期间递归激活被有界拒绝。跟随系统主题改为只读注册表，不再初始化宿主 WinRT。注册增加已实现的 INPUTMODECOMPARTMENT；未声明沉浸、UIElement 或显示属性支持。

可信安装通过 System32 input.dll 的 BOOL InstallLayoutOrTip 完成单条自有用户项安装，安装 flags=0、卸载 flags=1；不设置默认源或清空其他输入法。升级和恢复保留原 enabled／disabled 状态，同包重试保留原 receipt，失败恢复旧登记／启用状态；卸载先撤销自有用户项再移除登记，安装 EXE 给卸载器保留可信模块副本。

MSVC 构建、格式、all-targets Clippy、75 项 Rust（共享 8／Windows 67）、STA／MTA／COMLESS 共 48 个行为场景与 192 轮鼠标循环、PowerShell 7／5.1 各 101 项模拟安装更新检查通过。新增检查包含已有聚焦上下文激活、线程局部 CBT 递归激活、中英 compartment／语言栏按钮、同文档失焦结束及恢复、双队列饱和模式边界，以及系统主题读取不改变 STA／MTA／未初始化 apartment。普通安装 EXE 编译通过，未以其编译成功代替权限／卸载实机验收。

已用同账号可信 helper 登记 `%LOCALAPPDATA%\LingoMate\Windows\0.1.15-3B4294E1F2F0`，13 项清单及 COM 工厂回读通过，启用状态保持 true；用户 InputMethodTips 现包含微软拼音和灵果。清单 SHA-256 `3B4294E1F2F05DDD9F20A6A05AF3A0578434B8EB7908C7B29D6C77859CB521E7`，TSF DLL SHA-256 `9FD4134D553325B78F65AEC1F089A8E1E3D4FB8C51D011C236AB90D11EF3A992`；0.1.14 原目录保留。钉钉／grokbot／谷歌浏览器已经手测失败；ChatGPT 0.1.15 回归仍待完整退出重开后验收；本轮没有选择系统输入源、停止宿主、重启 ctfmon／电脑、提交、推送或发布。Mac 候选／菜单参考仍是 0.12.9／build 31，未修改 Mac 客户端或数据；共享核心跨平台复核和 Mac 设备检查边界保留。Obsidian 路径未配置，未同步外部笔记库。

## Windows UI 完整译法滚动 · 0.1.14 · 2026-10-12

当前本机登记版本为 0.1.14，在 0.1.13 候选／菜单对齐基础上，将更多译法改为完整滚动列表，保留绝对译法索引；绘制、tooltip 和点击只处理当前可见区域，不再按六条分组改变面板尺寸。100,000 条合成译法的几何回归确认访问范围受视口限制。Mac 参考仍为 0.12.9／build 31，最新主分支 04535f7 的交接文档已同步，Mac 客户端和数据源未修改。

最终 MSVC 构建、格式、all-targets Clippy、69 项 Rust（共享 8／Windows 61）、三模式 192 轮鼠标循环／27 个行为阶段、PowerShell 7／5.1 各 56 项更新包检查通过。浅／深色、POS 同行／关闭、长中文、空释义／例句、完整译法和有界滚动渲染通过。源码对照和合成检查不替代 Mac 实机或 ChatGPT／钉钉的实际切源、输入和物理鼠标验证。

已同账号可信登记 `%LOCALAPPDATA%\LingoMate\Windows\0.1.14-BBD895C6F73E`，13 项文件校验通过；清单 SHA-256 为 `BBD895C6F73EC5657A67C013830941CBE6BEE5074126BD4C6F26109D57CAC966`，DLL SHA-256 为 `03632A587225E5170FE7E0737E0C28940CEE8B326FAE21B2E6F1A768DBDBA245`。0.1.13／0.1.12 原目录保留；没有自动停止宿主、选择输入源、重启、提交或发布。所有本轮改动仍在 codex/windows-dev 未提交工作区，等待用户真实应用验收。当前用户需完整退出并重开应用加载新 DLL；普通 Setup EXE 和 Mac 构建／设备验证边界保留。Obsidian 路径未配置。

## Windows 候选窗与菜单对齐 Mac · 0.1.13 · 2026-10-12

按用户选择，本轮仅对齐候选窗与菜单，参考 Mac 0.12.9／build 31。已同步最新主分支 `04535f7` 的平台交接文档；该次远端更新没有修改 Mac 客户端 UI。Windows 采用同组中性色／绿色英文、12px 留白、圆角、左侧拼音和右侧分页／译法／设置，短词配对横排、长中文换行、词性同行、完整本地提示；更新入口移至“⚙ 设置 → 检查版本更新…”。保留 Windows 系统字体回退和原生菜单。独立设置窗口、账号／整句翻译与完整无障碍没有纳入本次范围。

0.1.12 的鼠标离开修复保留；导航等待期间保持候选几何，按下／释放绑定同一显示版本，高面板限制在工作区并支持滚轮浏览。展开译法的空释义／例句绘制访问异常已由真实 GDI 合成测试复现并修复。MSVC 构建、格式、all-targets Clippy、68 项 Rust、三线程模式 192 轮鼠标循环／27 个行为阶段及两种 PowerShell 各 56 项包检查通过。

已登记本机 `0.1.13-FBF7848BC586`，保留 0.1.12 目录；尚未提交、推送或发布。用户必须完整退出并重开已有宿主才能加载新 DLL。ChatGPT／钉钉的实际切源、鼠标、键盘、跨应用和多屏 DPI 验收仍待用户执行；源码对照与合成绘图不等于 Mac 或 Windows 实体设备验收。Obsidian 路径未配置，本轮只更新仓库文档。

## Windows 0.1.12 鼠标卡死修复 · 2026-10-12

0.1.11 的实际鼠标试用使 ChatGPT 卡死，已撤销登记。0.1.12 修复鼠标离开通知循环、重复重绘及轮询重入，已构建并登记到本机；59 项 Rust、三模式共 192 轮候选鼠标循环／27 个场景阶段及两种 PowerShell 各 56 项包检查通过。真实 ChatGPT／钉钉仍待用户复测，代码未提交或推送，旧草稿和更新清单保留。Windows 当前 UI 尚未完整对齐远端 Mac 0.12.9／build 31，下一步对照候选布局与设置入口。

## Windows 核心重构包 · 2026-10-12

当前 `codex/windows-dev` 在已同步远端的基础上开发 0.1.11，增加同目录 `lingomate_engine.dll`，与 TSF DLL、共享命令行引擎和固定数据一并生成。新安装／更新必须通过完整 13 项清单及摘要校验；历史 12 项 stdio 包和最早 8 项包仅用于经校验的回退。本轮重构已完成本机打包和登记，尚未提交或发布，远端 0.1.10 草稿仍是旧架构。本机旧版已注销并运行官方卸载器，残留版本文件清理待确认；此前草稿附件和更新清单保持原状。包检查和后台核心的隔离成功不代表 ChatGPT／Codex 或钉钉实际输入通过。详见 [Windows 验证记录](../native-windows/QA.md#宿主激活与引擎重构--2026-10-12)。

## Windows 0.1.10 最新修复交付 · 2026-10-12

本轮交付包含已授权合并的 PR #2 外观、字号、标点、图标及最新候选恢复修复。新版本采用 0.1.10，保留既有 0.1.9 文件；合并状态见 [PR #2](https://github.com/jizw0704-source/lingomate/pull/2)。维护者从 [Release 草稿](https://github.com/jizw0704-source/lingomate/releases)下载安装 EXE、SHA-256、精确源码 ZIP 及其 SHA-256、试用说明五份文件后转交同事。

产物名称为 `lingomate-windows-x64-0.1.10-setup.exe`、`lingomate-windows-0.1.10-project-source.zip` 及 `lingomate-windows-0.1.10-trial-guide.txt`，本机忽略目录为 `native-windows/package/installer-0.1.10/`。构建来源、大小和 SHA-256 以草稿记录和随包校验文件为准；源码 ZIP 的 `SOURCE-SNAPSHOT.json` 记录对应提交，不混入本机未提交文件。

仍为内部预览，更新清单未公开。普通安装 EXE 的 TSF 登记权限及异常退出码问题尚未单独修复；Windows 11 实际安装和跨应用输入须由 Wright 先核验，不能以编译或退出码判断安装成功。安装注意事项见试用说明及 [Windows 验证记录](../native-windows/QA.md)。Mac 安装和旧附件保留。

## Windows 0.1.9 外观与输入规则试用包 · 2026-10-12

Windows **0.1.9** 的安装 EXE、SHA-256、对应源码 ZIP 及其校验文件、试用说明共五份已上传[双平台内部草稿](https://github.com/jizw0704-source/lingomate/releases)，并下载回核对字节、大小和 GitHub 摘要。安装 EXE 为 5,516,168 字节，SHA-256 为 `9a4e5ce52a86b39cb3da49d64323deb8279d6f01c0bad66a2aca43954c647f4e`；源码固定为 `d5ae1d9f18bcec0c12902ea7b0ef5b8358d60be7`。此前 40 份附件的身份、大小和摘要保留，草稿现有 45 份附件，Mac 0.12.9 安装包保持原状。

[受控构建和上传](https://github.com/jizw0704-source/lingomate/actions/runs/38214316742)通过，基于固定构建引用 `windows-draft-build-0.1.9-2026.10.12`。源码 ZIP 与该提交的 185 个项目文件逐项一致，无本机覆盖文件、个人配置、运行时或生成词库。没有安装或注册输入源，也未公开发布或启用更新清单；Windows 11 的安装、实体键盘、主题、DPI、跨应用及“果”图标展示仍待 Wright 实机核对。

本机五份文件位于 `native-windows/package/installer-0.1.9/`。维护者下载后将完整文件转交同事；操作与本次新增功能见 `lingomate-windows-0.1.9-trial-guide.txt`。功能通过 [PR #2](https://github.com/jizw0704-source/lingomate/pull/2) 请求合并到 main，尚未合并。

## 连续逐字记忆安装包 · 2026-10-12

Mac **0.12.9 / build 31** 与 Windows **0.1.8** 安装包、校验文件、对应源码及试用说明已上传[内部试用草稿](https://github.com/jizw0704-source/lingomate/releases)并完成下载回校验；草稿仅维护者可见，须下载后转交同事。未公开发布或启用远程更新清单。本机已安装 Mac 0.12.9，用户已确认 Codex 中已知词与新组合“钙钛镍”的逐字输入及靠前整词候选；整词提交、重启保留、跨应用与 Windows 实机仍待验收。 既有三十份附件保留，新增双平台各五份文件；早期 Mac 0.12.8 / Windows 0.1.7 的并发保存检查存在偶发失败，建议使用修正版。

Windows 源码固定为 `27463a6301e872f57668e2daf5e4d5063b372a68`，见[构建与上传](https://github.com/jizw0704-source/lingomate/actions/runs/38199615445)。Mac 源码 ZIP 包含该基线及明确记录的字号／设置源文件覆盖项，不可用基线提交单独替代；包内 `SOURCE-SNAPSHOT.json` 记录逐文件哈希。安装文件仅保存于忽略目录，未提交生成词库或个人数据。Mac 采用临时签名且未公证，Windows 尚未完成发行签名，两端新设备与长期稳定性仍需试用验收。

## Windows 0.1.6 安装包草稿 · 2026-10-12

已将 Windows 0.1.6 的五份附件新增至[双平台内部草稿](https://github.com/jizw0704-source/lingomate/releases)：`lingomate-windows-x64-0.1.6-setup.exe`、SHA-256 校验文件、`lingomate-windows-0.1.6-project-source.zip` 及其校验文件、`lingomate-windows-0.1.6-trial-guide.txt`。Mac 0.12.7 及此前十五份附件身份、大小和摘要保持不变；该阶段两端试用版本为 Mac 0.12.7 / Windows 0.1.6，草稿仍未发布，更新清单未启用。

[受控构建和上传](https://github.com/jizw0704-source/lingomate/actions/runs/38192767635)基于固定提交 `6909ba41262dbb8144c716ccbcad856fd9abe157`，构建引用为 `windows-draft-build-0.1.6-2026.10.12`。草稿目标同步为此 Windows 构建；Mac 对应源码仍由其独立快照交付。Windows 原生构建、格式／Clippy、单元与真实引擎回归、词库／词性许可和数据再生成、更新回归、DLL 生命周期、安装 EXE 编译和初始窗口检查均通过；未安装或登记 TSF。

安装 EXE 为 5,480,288 字节，SHA-256 为 `4af61bef0a2032845e47ce313f25e2d144b22446a8e2ed087a39a1a854c0286d`。全部五份附件上传后下载回校验：字节、大小、GitHub 摘要和校验文件一致。源码 ZIP 只含固定提交的项目文件、构建脚本和来源说明，不混入本机修改、学习记录、账号或密钥；第三方源码和依赖通过固定准备脚本获取。本机保存于 `native-windows/package/installer-0.1.6/`。

维护者可下载后将 EXE、校验文件、对应源码和试用说明转交 Windows 11 Intel／AMD x64 同事。发行签名、真实首次安装、实体键盘、本地学习计数、跨应用、高 DPI、升级／恢复及卸载仍待实机验收。草稿没有公开发布或自动更新步骤。

## Mac 0.12.7 安装包草稿 · 2026-10-12

已将 Mac 0.12.7 / build 29 的五份附件上传至[既有双平台内部草稿](https://github.com/jizw0704-source/lingomate/releases)：`lingomate-macos-arm64-0.12.7.dmg`、SHA-256 校验文件、`lingomate-macos-0.12.7-project-source.zip` 及其校验文件、`lingomate-macos-0.12.7-trial-guide.txt`。旧 Mac 0.12.2 和 Windows 0.1.4 的十份附件身份、大小与摘要保持一致，草稿仍为未发布预览。

DMG 为 5,787,789 字节，SHA-256 为 `3143a17b4fbcd42c813d4b9b372ada0908e7aeaf663ea44670524762b1477596`。镜像检查、只读挂载后 18 个应用文件与本机已安装版一致性、安装器和应用签名、七项隔离安装器测试、预览退出测试、词库来源及 NOTICE 检查通过。上传后重新下载全部五份附件，大小、逐字节内容、GitHub 摘要和校验文件一致。

源码 ZIP 含 178 个文件，以 `2a9ff5db22e0d07f07bfd16b991dd50d10cb754b` 为基线，加上 14 个显式 macOS 构建修改；包含尚未合入基线的字号与界面代码。`SOURCE-SNAPSHOT.json` 记录基线、覆盖文件及安装前核验的构建输入摘要，下载回源码与这些输入逐项相符。不包含其他本机未提交后端修改、个人配置、学习记录、账号、密钥、运行时或生成数据；固定第三方源码与数据由准备脚本获取。转交同事时同时提供安装包、校验文件、对应源码及试用说明。

本机已安装 0.12.7，用户以实体键盘确认 Codex 中英文候选显示；新设备首次安装、下载后的系统安全检查、其他提交、跨应用与长期稳定性仍待验证。Mac 尚未完成 Developer ID 发行签名和公证，Intel Mac 不在范围。普通同事无法直接访问草稿；由维护者下载后转交。未上传远程更新 ZIP，未启用更新清单。

2026-10-11：新增词性功能的 Mac 0.12.3 DMG 已在本地生成、只读挂载核对并校验；Windows 0.1.5 的原生构建、设置持久化测试、安装 EXE 编译及初始窗口检查通过。此次新版安装包尚未上传草稿；下文已上传的试用包仍为 Mac 0.12.2 / Windows 0.1.4。当前 Mac 本机仍安装 0.12.2，Windows 11 实际试用待验证，更新清单保持未发布。

灵果分别提供 Mac DMG 和 Windows 安装 EXE 的构建流程。两端版本及功能范围不同，安装包不代表已完成正式发行或实机验收。

| 平台 | 目标设备 | 构建文件 | 当前范围 |
| --- | --- | --- | --- |
| macOS | Apple 芯片，macOS 13+ | `lingomate-macos-arm64-0.12.2.dmg` | 已上传双平台内部草稿，附对应项目源码与试用说明；下载回校验通过 |
| Windows | Windows 11，Intel / AMD x64 | `lingomate-windows-x64-0.1.4-setup.exe` | 已生成并保存在同一双平台内部草稿，下载回校验通过；未开放公开下载 |

2026-10-10，Mac 0.12.0 / build 22 的新 DMG 与更新 ZIP 已在本地生成。DMG 镜像校验、只读挂载后的应用版本 / 字节 / 安装器签名，以及真实更新 ZIP 路径检查通过。文件位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.0.dmg` 和 `native-macos/build/updates/verified-2026-10-10/release-0.12.0/`。随后已按用户授权通过 DMG 安装器更新本机，旧版备份与输入设置保留；未上传替换上述维护者草稿。本地签名仍为 ad-hoc，远程清单仍未发布。当时 Windows 源码为 0.1.2，维护者草稿为 0.1.1；最新安装包见下节。

## 新词库构建与历史包区分

Mac 0.12.1 / build 23、Windows 0.1.3 起统一从固定 CC-CEDICT 快照生成基础词库与英文释义，不复制旧混合词库或语料产物。数据及改编使用 CC BY-SA 4.0；完整原始文件头、署名、修改说明、摘要及许可全文随新包提供，两端检查拒绝旧表和缺失声明。来源缺口的替换结果见 [第三方来源](THIRD_PARTY.md#当前词库来源与替换结果)。本节所列旧版本 DMG、EXE、ZIP 和历史维护者草稿保留旧数据，不能将此次结果应用到旧包。

签名、公证、对应源码交付及目标设备安装／输入验收仍与数据替换分开处理。当前不发布 Release 或启用更新源。

Mac 0.12.1 的本地 DMG 位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.1.dmg`；更新 ZIP 位于 `native-macos/build/updates/license-replacement-2026-10-10/release-0.12.1/`。已验证镜像、只读挂载版本 / 数据 / 声明 / 签名以及更新 ZIP 的完整性和数据字节。随后按用户授权通过 DMG 安装器将本机更新到 0.12.1 / build 23，保留经校验的 0.12.0 ZIP 备份及个人数据，安装字节与验证构建一致；用户已确认 Codex 中英候选与混合输入。更新元数据保持未发布，维护者草稿尚未替换。Windows 0.1.3 的[原生构建与安装 EXE 检查](https://github.com/jizw0704-source/lingomate/actions/runs/38030807769)已通过，包含包内新数据及完整声明校验；未登记或执行真实安装，未导出完整新包到公开附件。

## 候选排序版本

Mac 0.12.2 / build 24 与 Windows 0.1.4 的源码共用许可明确的 Google Books Ngram 词频与完整词优先排序。官方许可、版本、摘要及加工说明加入已有两份 NOTICE，包内文件清单保持兼容；未导入书籍正文或新增 Google 词条。[来源与处理说明](THIRD_PARTY.md#候选排序与词频来源)提供重建方法和有限回归结果。

Mac 新 DMG 位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.2.dmg`。完整构建检查、镜像和只读挂载后的版本／build、最终桥接、词库、声明及签名核对通过；本机已安装 0.12.2，用户已确认 Codex 中英候选和“我AI学习”混合输入，英文确认回车未发送消息。本地更新 ZIP 位于 `native-macos/build/updates/ranking-20261010/release-0.12.2/`，摘要／大小、CRC／路径及桥接／数据／声明逐字节校验通过，元数据保持未发布。Windows 0.1.4 的 [MSVC 构建、来源／候选回归和安装 EXE 编译检查](https://github.com/jizw0704-source/lingomate/actions/runs/38037661244)通过，未登记 TSF 或执行实际安装。历史本地包和旧维护者草稿保留，Mac 新包已于 2026-10-11 补入 Windows 0.1.4 的维护者草稿，两端新包见下节；均未公开发布或启用更新源；发行签名、公证与新版实机输入仍待验证。

## Mac 0.12.2 内部试用包上传

2026-10-11，已将 Mac 0.12.2 / build 24 的 DMG、SHA-256 文件、对应项目源码快照和试用说明补入[双平台内部草稿](https://github.com/jizw0704-source/lingomate/releases)。构建输入与固定提交 `699e5b7e7a89cb3f05c147e3de514228466497a9` 一致，项目源码快照只包含该提交已跟踪文件，第三方源码及依赖由固定准备脚本获取；不含本机未提交修改、个人数据或生成表。

本机转交副本位于 `native-macos/build/packages/trial-0.12.2/`，共五个文件。DMG 大小为 5,555,997 字节，SHA-256 为 `45a912105f85a50234237cc94dc312eaadbdd21b5bc0f00e51dec70490bcc53f`。五份附件下载回后与本地字节及 GitHub 摘要一致；镜像校验、只读挂载后的版本、18 个应用文件、两层签名及七项隔离安装检查通过。挂载已卸载，本轮未执行安装或重载输入服务。

Mac、Windows 各五份附件均保存在本草稿中，旧双平台草稿保留。维护者下载后可将对应安装文件、源码和 `lingomate-macos-0.12.2-trial-guide.txt` 或 Windows 试用说明转交同事。Mac 目标为 Apple 芯片；Windows 目标为 Windows 11 Intel / AMD x64；普通同事无法直接访问草稿链接。

用户已确认本机 0.12.2 在 Codex 显示中英候选，并能混合输入“我AI学习”，英文确认回车未发送消息。新设备安装及系统安全检查、候选排序、其他提交、跨应用、休眠唤醒、Windows 11 实机与长期稳定性仍待确认；发行签名、公证和公开更新清单未完成。

## Windows 0.1.4 安装包保存与上传

2026-10-10，[生成与上传流程](https://github.com/jizw0704-source/lingomate/actions/runs/38038928439)基于固定提交 `699e5b7e7a89cb3f05c147e3de514228466497a9` 重建 Windows MSVC 程序、核对词库及声明、通过候选／记忆／更新回归、安装 EXE 编译和初始窗口检查后，将 EXE 与校验文件上传至独立[维护者草稿](https://github.com/jizw0704-source/lingomate/releases)。原双平台草稿保留，旧 Mac 与 Windows 附件未覆盖。

本机副本位于 `native-windows/package/installer-0.1.4/`，安装文件为 `lingomate-windows-x64-0.1.4-setup.exe`，大小 5,327,681 字节。上传后下载回校验，文件与构建 `.sha256` 及 GitHub 附件摘要一致，SHA-256 为 `422f1377d2350793fa643bc9fcaecff95b57f7535337bbfc548105aff21246bb`。同目录保存项目源码快照、对应校验文件和 `lingomate-windows-0.1.4-trial-guide.txt`；它们也已附在草稿中。源码快照只包含上述提交的已跟踪项目文件，第三方源码与依赖通过固定准备脚本取得，不混入本机修改、个人数据或生成表。

维护者下载后可将安装 EXE、试用说明与对应源码转交同事；普通同事不能直接通过草稿链接下载。当前目标为 Windows 11、Intel / AMD x64。未执行实际安装或登记 TSF，发行签名、Windows 11 实体键盘、跨应用及真实升级／回退待确认。草稿继续保持仅维护者可见的未发布状态，远程更新清单未启用。

## Mac 构建与安装

在仓库根目录执行：

```sh
bash native-macos/tools/build.sh
bash native-macos/tools/check.sh
bash native-macos/tools/package.sh
```

DMG 与 SHA-256 文件位于被 Git 忽略的 `native-macos/build/packages/`。打包前验证应用签名，并运行七项临时应用安装回归。已有同名 DMG 时停止，保留原文件。

打开 DMG，双击“安装灵果”，确认后安装至当前用户的 `~/Library/Input Methods/BilingualCompanion.app`。不需要 Python、Rust、开发工具或管理员权限。旧版经压缩、解压、身份、签名及关键文件对照校验后，保存至 `~/Library/Application Support/BilingualCompanion/InstallBackups/`。新应用先暂存校验；登记失败尝试恢复原应用与登记，恢复失败保留恢复目录并报告。

安装器不结束输入服务，不关闭宿主，不自动切换输入源、注销或重启。灵果仍在运行时拒绝覆盖；用户须先切换其他输入法、退出灵果设置与正常服务。安装后手动在系统键盘设置添加灵果。系统发现、实体键盘输入与跨应用连接须实际确认，登记命令成功不等同于输入可用。

安装器界面预览在安装包内执行 `Contents/MacOS/LingoMateInstaller --preview`，不读取个人词库或执行安装。独立安装逻辑回归使用 `--selftest`，只操作临时模拟应用。

当前签名为本地 ad-hoc 签名，未完成 Developer ID 发行签名及公证。下载后 Gatekeeper 是否允许运行尚未验证；不建议关闭安全检查。Intel Mac 不在当前构建范围。Mac 源码 0.12.0 已包含独立远程更新器，旧版需要先手动安装一次；历史维护者草稿中的 0.11.9 没有更新器；当前 0.12.2 内部草稿已包含更新器，清单仍未发布。远程 ZIP 与首次安装 DMG 分开，准备方法见 [Mac 软件更新](../native-macos/README.md#软件更新)。

## Windows 构建与安装

在 Windows x64 构建电脑安装 Inno Setup 6（仅构建工具，最终用户不需要），然后在仓库根目录执行：

```powershell
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
./native-windows/tools/build-installer.ps1
```

可通过 `-Compiler 'C:\path\ISCC.exe'` 指定编译器。脚本先验证既有运行包，生成 `native-windows/package/installer-<版本>/` 内的安装 EXE 和 SHA-256 文件；输出目录已存在时停止，不覆盖。打包脚本不安装、不登记、不下载打包工具、不上传附件。

安装 EXE 使用当前用户权限，解包至临时目录后执行随包的安装逻辑，校验文件并登记已复制的独立版本目录。安装器支持文件位于 `%LOCALAPPDATA%\LingoMate\Installer`，运行文件位于 `%LOCALAPPDATA%\LingoMate\Windows\<版本>-<摘要>`；旧版保留，供更新器回退。不会停止应用、结束 ctfmon、切换输入源或重启。执行或登记失败不显示成功完成。

卸载入口位于 Windows 已安装应用列表。确认后只取消本项目输入源登记、删除安装器支持文件；运行历史版本和个人数据保留。取消登记失败时停止卸载。远程更新仍使用经校验的 ZIP，与首次安装 EXE 分开；详见 [更新与发布](../native-windows/UPDATES.md)。

Windows 检查会在构建机器启动初始安装说明窗口，验证中文操作按钮至少 44 像素高，保存该窗口截图后结束本次安装器；不会点击安装或登记 TSF。[最终检查](https://github.com/jizw0704-source/lingomate/actions/runs/37911749463)已通过原生构建、安装 EXE 编译、窗口渲染、更新回归、真实引擎及 DLL 检查。CI 为 Windows Server 2025 x64；实际 Windows 11 的首次安装、系统键盘添加、应用输入、升级、回退、卸载和公司运行策略须实机验收。32 位应用与 Windows ARM 不在当前范围。

## 分发状态与验收

用户于 2026-10-09 请求上传两端安装包。DMG、Windows 安装 EXE 及各自 SHA-256 文件已附在 `installer-preview-2026.10.09` 的 [GitHub Release 草稿](https://github.com/jizw0704-source/lingomate/releases)中，仅供仓库维护者查看；未公开发布，也未开放同事下载。Windows 包由 [受控构建上传](https://github.com/jizw0704-source/lingomate/actions/runs/37918649277)生成并直接附到草稿，不使用公开构建附件。上传前后校验保持一致，草稿状态已回读核对，匿名草稿页面返回 404。

2026-10-09，Mac 附件已替换为 0.11.9 / build 21，旧版 0.11.8 DMG 与校验附件已从草稿移除，本地旧包保留。新镜像的包内版本、主程序及三份图标资源与验证构建一致，安装器签名和七项隔离回归通过；上传后的下载文件与本地字节及 SHA-256 一致。Windows 两份附件的身份、大小与摘要保持不变。Mac 构建来自 [c8f6527](https://github.com/jizw0704-source/lingomate/commit/c8f6527871b685766a167ecda8c1eb4c32fa8c59)，Windows 附件仍来自原 [84b348a](https://github.com/jizw0704-source/lingomate/commit/84b348a5223de130ce7005004646a322f6f95130)；草稿标签保留原构建引用，Release 说明分别列出两端来源。草稿状态保持不变，真实 DMG 首次安装及下载后的系统安全检查仍待验收。

Mac 和 Windows 更新清单均保持 `unpublished`，草稿不触发远程更新。正式向同事分发前，须核对新包确实包含 [已替换的新词库和声明](THIRD_PARTY.md#当前词库来源与替换结果)，并完成对应源码交付、两端发行签名安排及目标设备安装验收。Mac 公证与 Windows 签名尚无凭据配置，不要求在聊天中提供密钥。

后续上传 Windows 包可使用手动工作流：先创建预发布草稿并将 `target_commitish` 设为对应完整提交号，再执行 `gh workflow run windows-package-draft.yml --ref <同一提交的标签> -f release_id=<草稿编号>`。工作流验证目的地仍为草稿、源码版本一致，完成全部检查后才上传 EXE 与校验文件；没有发布步骤，不覆盖已有同名附件。

验收时先使用空白文稿测试 `xuexi` 中文与英文候选、翻页、`API` 回车直出、Shift 英文切换，再测试微信、钉钉及办公应用。Windows 目前不包含 Mac 的个人词记忆、账号学习同步、AI 整句翻译及完整标点功能。记录应用名称、系统版本和复现步骤即可，不收集输入正文或个人词库。


替换前的 2026-10-10 词库核对发现中文常用词及规范字转录尚缺明确授权记录，维基语料许可版本亦有差异。两端构建脚本已补充 THUOCL、LCCC、Unicode 许可全文到既有 `LEXICON-NOTICE.md`，但不因此解除分发待确认状态。历史安装包未更新。实际随包数据、摘要及处理顺序见 [词库分发核对](THIRD_PARTY.md#2026-年-10-月-10-日旧词库分发核对)。
