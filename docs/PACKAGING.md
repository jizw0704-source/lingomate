# 安装包与同事试用

灵果分别提供 Mac DMG 和 Windows 安装 EXE 的构建流程。两端版本及功能范围不同，安装包不代表已完成正式发行或实机验收。

| 平台 | 目标设备 | 构建文件 | 当前范围 |
| --- | --- | --- | --- |
| macOS | Apple 芯片，macOS 13+ | `lingomate-macos-arm64-0.11.9.dmg` | 已上传维护者可见的 Release 草稿，内含原生安装程序 |
| Windows | Windows 11，Intel / AMD x64 | `lingomate-windows-x64-0.1.1-setup.exe` | 已构建、检查并上传同一 Release 草稿；未开放公开下载 |

2026-10-10，Mac 0.12.0 / build 22 的新 DMG 与更新 ZIP 已在本地生成。DMG 镜像校验、只读挂载后的应用版本 / 字节 / 安装器签名，以及真实更新 ZIP 路径检查通过。文件位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.0.dmg` 和 `native-macos/build/updates/verified-2026-10-10/release-0.12.0/`。随后已按用户授权通过 DMG 安装器更新本机，旧版备份与输入设置保留；未上传替换上述维护者草稿。本地签名仍为 ad-hoc，远程清单仍未发布。Windows 当前源码为 0.1.2，维护者草稿仍为 0.1.1。

## 新词库构建与历史包区分

当前源码 Mac 0.12.1 / build 23、Windows 0.1.3 统一从固定 CC-CEDICT 快照生成基础词库与英文释义，不复制旧混合词库或语料产物。数据及改编使用 CC BY-SA 4.0；完整原始文件头、署名、修改说明、摘要及许可全文随新包提供，两端检查拒绝旧表和缺失声明。来源缺口的替换结果见 [第三方来源](THIRD_PARTY.md#当前词库来源与替换结果)。旧版本的 DMG、EXE、ZIP 和维护者草稿均保留旧数据，不能将此次结果应用到旧包。

签名、公证、对应源码交付及目标设备安装／输入验收仍与数据替换分开处理。当前不发布 Release 或启用更新源。

Mac 0.12.1 的本地 DMG 位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.1.dmg`；更新 ZIP 位于 `native-macos/build/updates/license-replacement-2026-10-10/release-0.12.1/`。已验证镜像、只读挂载版本 / 数据 / 声明 / 签名以及更新 ZIP 的完整性和数据字节。随后按用户授权通过 DMG 安装器将本机更新到 0.12.1 / build 23，保留经校验的 0.12.0 ZIP 备份及个人数据，安装字节与验证构建一致；用户已确认 Codex 中英候选与混合输入。更新元数据保持未发布，维护者草稿尚未替换。Windows 0.1.3 的[原生构建与安装 EXE 检查](https://github.com/jizw0704-source/lingomate/actions/runs/38030807769)已通过，包含包内新数据及完整声明校验；未登记或执行真实安装，未导出完整新包到公开附件。

## 候选排序版本

Mac 0.12.2 / build 24 与 Windows 0.1.4 的源码共用许可明确的 Google Books Ngram 词频与完整词优先排序。官方许可、版本、摘要及加工说明加入已有两份 NOTICE，包内文件清单保持兼容；未导入书籍正文或新增 Google 词条。[来源与处理说明](THIRD_PARTY.md#候选排序与词频来源)提供重建方法和有限回归结果。

Mac 新 DMG 位于 `native-macos/build/packages/lingomate-macos-arm64-0.12.2.dmg`。完整构建检查、镜像和只读挂载后的版本／build、最终桥接、词库、声明及签名核对通过；本机仍安装 0.12.1。本地更新 ZIP 位于 `native-macos/build/updates/ranking-20261010/release-0.12.2/`，摘要／大小、CRC／路径及桥接／数据／声明逐字节校验通过，元数据保持未发布。Windows 0.1.4 的 [MSVC 构建、来源／候选回归和安装 EXE 编译检查](https://github.com/jizw0704-source/lingomate/actions/runs/38037661244)通过，未登记 TSF 或执行实际安装。历史本地包和维护者草稿保留，新包未上传、公开发布或启用更新源；发行签名、公证与新版实机输入仍待验证。

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

当前签名为本地 ad-hoc 签名，未完成 Developer ID 发行签名及公证。下载后 Gatekeeper 是否允许运行尚未验证；不建议关闭安全检查。Intel Mac 不在当前构建范围。Mac 源码 0.12.0 已包含独立远程更新器，旧版需要先手动安装一次；维护者草稿仍为 0.11.9，没有更新器。远程 ZIP 与首次安装 DMG 分开，准备方法见 [Mac 软件更新](../native-macos/README.md#软件更新)。

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
