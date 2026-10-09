# 灵果 · LingoMate · Windows 开发版

Windows 版本采用系统原生 Text Services Framework（TSF，文本服务框架），由 Rust 文本服务 DLL 接入应用输入框，并通过独立本地进程调用现有拼音与英文释义引擎。

当前为 **0.1.0 开发版**，首个目标是 Windows 11 的 Intel / AMD 64 位电脑与桌面文本应用。Windows 10、ARM、32 位应用、现代应用及跨应用稳定性需要分别验证。macOS 原生版本继续维护。

## 首版范围

| 功能 | 实现范围 |
| --- | --- |
| 中文拼音、中英候选 | 使用共享真实引擎和离线释义表 |
| 多译法 | Tab 展开与切换，Shift＋Tab 反向切换，支持点击译法 |
| 候选分页 | 每页 5 项，最多 64 项，数字仅对应当前页 |
| 中文夹英文 | 空格确认中文，回车确认原样字母并保留大小写 |
| 英文直输 | 单按并松开 Shift 切换；英文模式按键交给当前应用 |
| 基础标点 | 常用逗号、句号、问号、感叹号、冒号及分号跟随确认输出语言 |
| 系统接入 | COM 类厂、键盘事件、焦点通知、异步文档编辑及非激活候选窗口 |
| 安装维护 | 固定源码构建、校验清单、独立版本目录、登记回退及卸载脚本 |

个人词库、账号学习同步、MiniMax 在线整句翻译、深浅色设置、完整标点规则和“果”字系统图标尚未接入 Windows 客户端。Apple Translation 属于 macOS 系统能力，不能直接移植到 Windows。首版候选面板采用浅色，完整例句、辅助功能与高 DPI 布局待完善。

## 构建与检查

准备 Windows 64 位 PowerShell、Git、Rustup，以及带 MSVC 与 Windows SDK 的 Visual Studio C++ 构建工具。执行：

```powershell
git clone https://github.com/jizw0704-source/lingomate.git
cd lingomate
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
```

准备脚本使用 Rust 1.96.0 与固定上游快照，发现已有源码改动时停止并保留文件。构建产物位于 `native-windows/package/windows-x64/`，包括 DLL、共享引擎、数据与来源说明。构建和检查不会安装或选择输入法。

仓库的 Windows 检查流程执行 MSVC 构建、PowerShell 语法检查、Rust 格式与 Clippy、真实引擎回归及 DLL 类厂生命周期检查。流程不登记系统输入源，也不发布完整数据或安装包。检查结果见 [验证记录](QA.md)。

## 安装与使用

在 Windows 机器完成检查后运行：

```powershell
./native-windows/tools/install.ps1
```

安装脚本校验文件，将新版本放入 `%LOCALAPPDATA%\\LingoMate\\Windows\\` 的独立目录，并登记本项目的 COM 类与中文输入配置。旧版本文件保留，登记失败时尝试恢复原登记。部分系统的 TSF 登记可能需要相应权限；本阶段安装与权限行为仍待实机验证。

完成后通过 Windows 输入切换器手动选择“灵果 · LingoMate”。如系统要求添加中文语言或重新登录，按系统提示处理；脚本不自动重启、注销、切换输入法或关闭其他应用。

| 操作 | 功能 |
| --- | --- |
| 空格 / 数字 1–5 | 确认当前页中文 |
| Shift＋空格 | 确认当前选中的英文译法 |
| Enter | 有组合输入时提交原样字母；空回车由当前应用处理 |
| Tab / Shift＋Tab | 展开及切换译法 |
| ↑ / ↓ | 切换当前页候选 |
| PageUp / PageDown，`-` / `=` | 翻页 |
| Esc | 收起译法，再取消输入 |
| 单按并松开 Shift | 切换中文 / 英文直输 |

可使用 `wo`＋空格、`AI`＋回车、`xuexi`＋空格检查“我AI学习”；输入 `shi` 检查分页，输入 `xuexi` 检查第三种译法。实际键盘验收应包括记事本、浏览器、聊天应用及反复焦点切换。

卸载前先选择其他输入法，再运行：

```powershell
./native-windows/tools/uninstall.ps1
```

卸载只撤销本项目登记，保留版本文件用于回退，不清理其他输入法或个人数据。

## 数据、依赖与验证边界

Windows 客户端不读取宿主正文、选区或剪贴板，不保存输入日志，也不调用在线翻译或账号服务。只将当前拼音及已选择候选通过本机管道交给共享引擎；文字不作为进程参数。首版不启用个人词库，避免不同应用进程并发覆盖同一文件。

Windows 平台接口使用 Microsoft `windows` / `windows-core` Rust 绑定；JSON 协议复用既有 serde 与 serde_json。平台绑定只在 Windows 编译，新增依赖用于系统接口接入。代码采用 GPL-3.0-or-later，数据来源与分发待核对项见 [第三方来源](../docs/THIRD_PARTY.md)。

macOS 上的 Windows 目标类型检查与引擎回归不等同于 Windows 实机输入验收。当前尚未发布正式 Windows 安装包，系统登记、应用兼容性、候选焦点、Shift 实际事件、缩放及长期稳定性仍待验证。
