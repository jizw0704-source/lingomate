# Windows 根因检查 · 2026-10-12

范围为 `codex/windows-dev` 的 Windows 注册／安装、TSF 激活与焦点、按键判定、文档异步编辑、候选窗口、后台工作队列和共享核心 ABI。macOS 与后端没有按 Windows 实机结果宣称通过。0.1.15 补 HKLM 后的钉钉输入验收仍失败；当前 0.1.16 已构建、安装并通过自有窗口的真实系统加载检查，但 10 月 13 日重启后的跨应用键盘验收仍失败。随后确认根因是在 Codex 应用包内安装；包外重装后，用户已验收 Chrome、钉钉、grokbot 和 ChatGPT 的输入。当前代码没有 commit／push。

## 2026-10-13 包外重装验收与候选窗显示根因

包外重装后，跨应用输入通过用户验收。Windows 搜索栏除外，它属于未声明的沉浸式宿主。随后出现的显示问题有三个独立原因：

- 灰度抗锯齿取代了 ClearType；
- 候选窗继承宿主的 DPI 感知，在只做系统 DPI 感知的宿主（钉钉）里，于 144 DPI 屏上被 DWM 位图拉伸；
- 每次按键都清空已显示的候选，造成 400 px 和 560 px 两种尺寸交替出现。

修复与验证见 [Windows QA](../native-windows/QA.md)。

另确认一处包视图残留：Codex 应用包私有注册表里的自有 CLSID 仍指向包内安装的旧版本，覆盖了系统登记，所以 ChatGPT 在重装后继续加载旧 DLL。这一项已在包容器内备份并删除，包视图现在回落到 HKLM 登记。包内的旧版本目录和独立个人词库仍然保留，这个数据分裂问题留待后续处理。

## 2026-10-13 根因更正：安装只存在于 Codex 应用包视图

此前所有安装、探针和诊断开关都在 Codex 桌面应用的终端中执行。该应用是 MSIX 包 `OpenAI.Codex`（进程名 `ChatGPT.exe`，即用户所称能正常工作的 ChatGPT 宿主），其子进程继承包身份：新建的 `%LOCALAPPDATA%` 文件写入 `%LOCALAPPDATA%\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\`，HKCU 写入进入包私有注册表。真实 HKLM 中却有指向该路径的登记；helper 写 HKLM 前必须通过 DLL 存在性检查，由此推断提权 helper 也运行在包视图中。

| 包外复核（无包身份终端） | 结果 |
| --- | --- |
| `%LOCALAPPDATA%\LingoMate` | 不存在；父目录最后变更为 10 月 10 日 |
| Codex 包 `LocalCache\Local\LingoMate\Windows` | 0.1.8 至 0.1.16 全部存在，0.1.16 清单／DLL 摘要与下文一致 |
| HKLM CLSID InprocServer32 | 指向上述包外不存在的路径 |
| HKLM TIP profile／类别 | 存在，`Enable=1`，无 HKCU 覆盖，因此切换器仍列出灵果 |
| HKCU CLSID、TIP 用户项、`User Profile` | 均不存在；`Get-WinUserLanguageList` 只有微软拼音 |
| 包外按 CLSID 创建灵果 | `0x8007007E`，找不到模块 |
| ChatGPT PID 30460 | 仍映射该路径下的 DLL，模块路径为包内虚拟视图 |

因此 Chrome、钉钉和 grokbot 选择灵果时，TSF 无法载入 DLL：进程内没有灵果模块，并回退为无 TIP 的字母输入或原微软拼音。SearchHost 还受沉浸式／AppContainer 要求约束，即使正确安装也不在当前已声明能力内。下文的系统加载探针、`system-legacy-auto.log` 和诊断记录都在包视图内取得，只能证明包内行为；COMLESS 直接查 HKLM 的静态分析与 A/B 结论仍然有效，HKLM 映射仍需保留。

`Get-LingoMateInstallRoot` 现在先查询 `GetCurrentPackageFullName`，有包身份就拒绝，覆盖 `install.ps1`、`uninstall.ps1`、更新 worker 和系统加载探针。修复验收需要在普通 PowerShell 中重新执行 `install.ps1`（同账号 UAC），再由用户在完全重开的 Chrome、钉钉、grokbot 中输入 `xuexi`；在此之前跨应用问题仍未解决。startup ETW 跟踪未运行，只在包外安装后仍失败时才需要。

## 2026-10-13 重启后证据

用户确认仅 ChatGPT 正常显示拼音／候选，Chrome、钉钉、grokbot 仍只有字母、没有候选窗；Windows 搜索栏能选择灵果但实际使用微软输入法。正常启动时间 09:38:26，ctfmon／TextInputHost 已更新到本次会话，因此旧驻留会话不足以解释失败。

ChatGPT 主进程加载精确 0.1.16 TSF／核心 DLL，诊断为 ActivateReady／HRESULT 0／English 0／ThreadFocus 1；已枚举 Chrome、钉钉和 SearchHost 不含灵果 DLL。不是仅依据诊断缺失作结论。ChatGPT／Chrome／钉钉主进程均为 medium、非 elevated、非 AppContainer，签名、图像加载及扩展点策略均为 0。metadata 证据在忽略的 `evidence/windows-local-2026-10-13/`，不含窗口标题、输入文本或按键。

自有 Win32 EDIT 的自动 IMM→TSF bridge 可加载 .16，foreground service 为灵果、文档为 transitory，并恢复原线程 profile；该探针仍有显式线程 ActivateProfile，不代表真实切换器选择链或键盘通过。下一步对实际应用的系统 profile 激活阶段作有限技术诊断；不再把旧缓存或未签名作为没有证据的统一根因。

## 当前 0.1.16 状态

已安装的 Windows x64 包共 13 个文件，摘要如下：

| 对象 | SHA-256 |
| --- | --- |
| 包 manifest | `52F90B953BB3B59F1F5FE6D0F06968202A01330CF35DE515C74B85435C8035B3` |
| 灵果 TSF DLL | `195D0DF3E5D26A0C6061D2EFE84B2B8D6D6477F604706571236A939F804BD829` |

普通非管理员用户运行 `install.ps1`，经同一 SID 的 UAC 提权完成机器级自有 CLSID 登记，已在本机真实通过；安装后的 HKCU／HKLM 类登记一致。旧版本与个人数据保留。Build／Check 已通过 84 项 Rust 测试、48 个 owned TSF 行为阶段，PowerShell 7 与 Windows PowerShell 5.1 各 134 项检查，以及固定数据检查和 Inno Setup 构建。URL `BreakContext` 修复与下文的默认关闭诊断已包含在该构建。

0.1.16 的真实系统检查使用显式 `OwnFocus`，普通与 `TF_TMAE_COMLESS=8` 两种入口均通过：自有窗口与 TSF 线程确实获得焦点，foreground service 为自有灵果 CLSID，Windows 加载精确的 0.1.16 安装 DLL 并核对 x64 PE，随后恢复原线程 profile、自有窗口文档关联与之前的 foreground HWND。相关加载、焦点与恢复阶段为 `S_OK`。本地证据为 `evidence/windows-local-2026-10-12/root-cause-0.1.16-system-profile-focused.log`。

这次检查只使用自有窗口和空合成 ACP 文档，没有键盘输入或外部宿主输入验收。真实 Chrome、钉钉与 Grokbot 是否出现候选、提交中文、切换中英文及保持稳定，仍待用户正常重启后的人工验收；不能用系统加载通过替代这些结果。

## 已确认的系统加载缺口

`native-windows/src/tsf/registration.rs` 原先只登记 `HKCU\Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32`。普通 COM 使用 HKCR 合并视图，所以 COM 工厂检查和普通 ThreadMgr 加载成功；本机 Windows COMLESS 实现直接查询 HKLM，不回退 HKCU。机器级自有 CLSID 缺失时，Windows 可以报告 profile 已选中，却没有实例化该输入服务。

官方 `ITfThreadMgrEx::ActivateEx` 的 `TF_TMAE_COMLESS` 正是不用 COM 的加载方式。[官方 API](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfthreadmgrex-activateex) 此结论不泛化成所有 COM 都要求 HKLM。[一般 COM 合并视图](https://learn.microsoft.com/en-us/windows/win32/sysinfo/merged-view-of-hkey-classes-root)

本机证据：Windows `msctf.dll` 版本 `10.0.26100.8875`，SHA-256 `EFCAFF195F990235A506F72749019F0BE312627CC4AC1464455E6E2AC118B156`。匹配微软公开 PDB `C53C99BECADFB8301612A532743A59881` 的 `ComLessCreateInstance` 调用 `GetComModuleName`；后者查 HKLM InprocServer32／LocalServer32，缺失直接失败。证据取自磁盘 DLL 与公开符号，没有读取宿主内存或正文。[微软公开符号](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/microsoft-public-symbols)

| 对照条件 | profile 身份 | 实际 foreground service | 灵果 DLL | 结果 |
| --- | --- | --- | --- | --- |
| 0.1.15，普通真实 ThreadMgr，只有 HKCU 类登记 | 灵果 | 灵果 | 已加载，安装路径与 PE 匹配 | 通过 |
| 同 DLL、同 GUID、TF_TMAE_COMLESS=8，HKLM 缺失 | 仍为灵果 | S_FALSE，无接键服务 | 未加载 | 失败 |
| 只新增自有 HKLM 类登记，其余条件不变 | 灵果 | 灵果 | 已加载，路径与 PE 匹配 | 通过 |

以上采用新自有进程、空合成 ACP 文档、flags=0 的线程 profile 激活，并成功还原原线程 profile。没有全局选择源、物理按键注入或输入内容读取。根因属于已复现的系统加载契约；不能把这张表等同于钉钉实机输入已经通过。

## 验证体系的缺口

原 `tsf_host.cpp` 虽使用真实 TSF context，却通过 OwnedManager 截获订阅并直接调用 key/event sinks；底层管理器以 NOACTIVATETIP 启动。它能检查候选、编辑、队列及焦点，但绕过了操作系统从 profile 到 DLL 的加载。这使得 48 个行为场景通过后，真实 COMLESS 入口的缺失仍没有被发现。

新增 `system_profile_host.cpp` 与显式本机 `test_system_profile.ps1` 检查普通／COMLESS 的实际 Windows 加载，核对活动 profile、foreground service、DLL 安装路径／架构及原 profile 恢复。新测试支持显式 `OwnFocus`，并核对自有窗口的实际 native／TSF thread focus 和 window association 恢复。隐藏窗口上的 profile 激活即使返回 `S_OK`，也可能延迟 DLL 加载；只有返回值而没有实际 foreground service 和正确 DLL，不能算通过。0.1.16 已按上文取得焦点并通过两种入口。该检查只用于已经安装的可信本机包，不进入默认 CI，不登记输入法；真实键盘和现代／传统应用仍单独验收。

## 已确认的另一处状态错误

URL／邮箱的 literal 上下文保留在 worker。原 TSF 快捷键／导航取消仅重置 UI 缓存，而 worker 的 `CancelMemory` 只取消记忆，所以 `https://` → 上下文打断 → `wo` 可以仍逐字输出英文。新增 FIFO `BreakContext`，同时取消记忆并重置 worker 的 punctuation/literal 状态；单纯 `CancelMemory` 保留 URL 大小写操作的语义。两个真实核心回归分别确认恢复“我”候选、以及单纯取消记忆仍保持 URL literal。

这条错误会在已接键后直接提交英文，不能解释钉钉／Chrome 进程完全没有灵果 DLL。不能把两种阶段混称为引擎不可用。

## 默认关闭的阶段诊断

0.1.16 已构建固定阶段的 opt-in 诊断。只有 `HKCU\Software\LingoMate\Diagnostics\Enabled` 为准确的 DWORD `1` 时才启用，默认关闭；按当前 PID／TID 保存最近快照，字段限定为 `Stage`、`HRESULT`、`ActivationFlags`、`ThreadFlags`、`English`、`ThreadFocus`。失败只更新 HRESULT，保留具体失败操作的 Stage；未知字段不冒充 0，模式变化不覆盖线程焦点。诊断没有输入文本、键码、输入长度或逐键记录。

Factory／Activate 入口只检查一次启用状态，模式与焦点记录通过有界、非阻塞内存队列交给后台写入注册表。后台每次发布再检查开关，所有诊断失败都被忽略，不中断输入服务。没有记录也可能来自未启用、队列丢弃或注册表失败，不能单凭记录缺失断言未激活。诊断尚未替代真实应用的人工验收。

## 继续排查的边界

0.1.15 补 HKLM 后，用户仍反馈钉钉无候选、状态栏“拼”且只出字母；当时其进程与 Chrome 已确认重开，却仍未发现灵果模块。已核对的 GUID、0804 语言、Apartment、用户 InputMethodTips、类别、数据来源与包摘要一致；钉钉已查看进程的签名／图像加载／扩展点策略均为 0，没有相应策略拒绝的证据。0.1.16 的持久化安装和自有聚焦窗口系统加载现已通过，但 Chrome／钉钉／Grokbot 的重启后实机键盘验收尚未完成。HKLM 缺口是真实且必要的修复，真实应用链路仍不能标为解决。

后续先由用户正常重启并复测真实应用；若仍异常，再用已经构建的固定阶段 HRESULT／模式／焦点技术诊断区分系统未加载、激活失败与模式／焦点分支，并检验驻留服务缓存的假设。不能在没有证据时添加 IMMERSIVE／UIElement／SecureMode 类别，借用微软 HKL，或把 Qt 的存在当成禁用 TSF 的证明。当前 Weasel 固定提交已经移除 IMM，旧 FindIME 只识别历史安装。[Weasel 当前安装源码](https://github.com/rime/weasel/blob/4aea1978a351e2ce41af20c8fb88a01ce9802de8/WeaselSetup/imesetup.cpp)

整个过程保持旧安装版本、个人数据和 `codex/windows-dev` 未提交工作；未关闭宿主、重启 ctfmon、自动注销／重启、推送或发布。Obsidian 镜像路径未配置。
