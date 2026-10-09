# 安装包与同事试用

灵果分别提供 Mac DMG 和 Windows 安装 EXE 的构建流程。两端版本及功能范围不同，安装包不代表已完成正式发行或实机验收。

| 平台 | 目标设备 | 构建文件 | 当前范围 |
| --- | --- | --- | --- |
| macOS | Apple 芯片，macOS 13+ | `lingomate-macos-arm64-0.11.8.dmg` | 本地测试镜像，内含原生安装程序 |
| Windows | Windows 11，Intel / AMD x64 | `lingomate-windows-x64-0.1.1-setup.exe` | 已在 Windows CI 生成，验证初始安装说明窗口；未发布下载 |

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

当前签名为本地 ad-hoc 签名，未完成 Developer ID 发行签名及公证。下载后 Gatekeeper 是否允许运行尚未验证；不建议关闭安全检查。Intel Mac 不在当前构建范围。Mac 端尚未实现远程更新。

## Windows 构建与安装

在 Windows x64 构建电脑安装 Inno Setup 6（仅构建工具，最终用户不需要），然后在仓库根目录执行：

```powershell
./native-windows/tools/setup.ps1
./native-windows/tools/build.ps1
./native-windows/tools/check.ps1
./native-windows/tools/build-installer.ps1
```

可通过 `-Compiler 'C:\path\ISCC.exe'` 指定编译器。脚本先验证既有运行包，生成 `native-windows/package/installer-0.1.1/` 内的安装 EXE 和 SHA-256 文件；输出目录已存在时停止，不覆盖。打包脚本不安装、不登记、不下载打包工具、不上传附件。

安装 EXE 使用当前用户权限，解包至临时目录后执行随包的安装逻辑，校验文件并登记已复制的独立版本目录。安装器支持文件位于 `%LOCALAPPDATA%\LingoMate\Installer`，运行文件位于 `%LOCALAPPDATA%\LingoMate\Windows\<版本>-<摘要>`；旧版保留，供更新器回退。不会停止应用、结束 ctfmon、切换输入源或重启。执行或登记失败不显示成功完成。

卸载入口位于 Windows 已安装应用列表。确认后只取消本项目输入源登记、删除安装器支持文件；运行历史版本和个人数据保留。取消登记失败时停止卸载。远程更新仍使用经校验的 ZIP，与首次安装 EXE 分开；详见 [更新与发布](../native-windows/UPDATES.md)。

Windows 检查会在构建机器启动初始安装说明窗口，验证中文操作按钮至少 44 像素高，保存该窗口截图后结束本次安装器；不会点击安装或登记 TSF。[最终检查](https://github.com/jizw0704-source/lingomate/actions/runs/37911749463)已通过原生构建、安装 EXE 编译、窗口渲染、更新回归、真实引擎及 DLL 检查。CI 为 Windows Server 2025 x64；实际 Windows 11 的首次安装、系统键盘添加、应用输入、升级、回退、卸载和公司运行策略须实机验收。32 位应用与 Windows ARM 不在当前范围。

## 分发状态与验收

安装包仅在忽略目录或临时构建环境生成；GitHub 源码公开，未发布含完整词库的 Release 附件，Windows 更新清单继续保持 `unpublished`。正式向同事分发前，须完成 [第三方词库分发确认](THIRD_PARTY.md)、两端发行签名安排及目标设备安装验收。Mac 公证与 Windows 签名尚无凭据配置，不要求在聊天中提供密钥。

验收时先使用空白文稿测试 `xuexi` 中文与英文候选、翻页、`API` 回车直出、Shift 英文切换，再测试微信、钉钉及办公应用。Windows 目前不包含 Mac 的个人词记忆、账号学习同步、AI 整句翻译及完整标点功能。记录应用名称、系统版本和复现步骤即可，不收集输入正文或个人词库。
