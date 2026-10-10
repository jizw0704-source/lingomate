import AppKit
import Foundation

final class InstallerWindow: NSObject, NSApplicationDelegate {
  var window: NSWindow!
  let status = NSTextField(wrappingLabelWithString: "")
  let installButton = NSButton(title: "安装灵果", target: nil, action: nil)
  let closeButton = NSButton(title: "退出", target: nil, action: nil)
  var busy = false
  let preview = CommandLine.arguments.contains("--preview")

  func applicationDidFinishLaunching(_ notification: Notification) {
    window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 528, height: 360),
      styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = "灵果 · 安装"
    window.center()
    let title = NSTextField(labelWithString: "安装灵果输入法")
    title.font = .systemFont(ofSize: 24, weight: .semibold)
    let info = NSTextField(
      wrappingLabelWithString:
        "适用于 Apple 芯片 Mac，macOS 13 或更新版本。\n安装到当前用户目录，保留个人词库和旧版备份。\n\n安装前请切换到其他输入法，并退出正在运行的灵果。")
    info.font = .systemFont(ofSize: 14)
    status.font = .systemFont(ofSize: 14)
    status.stringValue = preview ? "界面预览，不会安装或修改输入法。" : "点击安装后，在系统键盘设置中添加“灵果”输入源。"
    installButton.target = self
    installButton.action = #selector(installClicked)
    installButton.keyEquivalent = "\r"
    closeButton.target = self
    closeButton.action = #selector(closeClicked)
    closeButton.keyEquivalent = "\u{1b}"
    for button in [installButton, closeButton] { button.bezelStyle = .rounded }
    let buttons = NSStackView(views: [closeButton, installButton])
    buttons.spacing = 16
    let stack = NSStackView(views: [title, info, status, buttons])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 24
    stack.translatesAutoresizingMaskIntoConstraints = false
    window.contentView!.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor, constant: 32),
      stack.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor, constant: -32),
      stack.topAnchor.constraint(equalTo: window.contentView!.topAnchor, constant: 32),
      installButton.heightAnchor.constraint(equalToConstant: 44),
      closeButton.heightAnchor.constraint(equalToConstant: 44),
      installButton.widthAnchor.constraint(equalToConstant: 132),
      closeButton.widthAnchor.constraint(equalToConstant: 120),
    ])
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }

  @objc func installClicked() {
    if preview {
      status.stringValue = "预览模式：安装未执行。"
      return
    }
    guard !busy else { return }
    busy = true
    installButton.isEnabled = false
    closeButton.isEnabled = false
    window.standardWindowButton(.closeButton)?.isEnabled = false
    status.stringValue = "正在校验并安装，请稍候…"
    let source = Bundle.main.resourceURL!.appendingPathComponent("BilingualCompanion.app")
    let home = FileManager.default.homeDirectoryForCurrentUser
    DispatchQueue.global(qos: .userInitiated).async {
      let result = Result {
        try Installer(
          source: source,
          destination: home.appendingPathComponent("Library/Input Methods/BilingualCompanion.app"),
          backups: home.appendingPathComponent(
            "Library/Application Support/BilingualCompanion/InstallBackups")
        ).install()
      }
      DispatchQueue.main.async {
        self.busy = false
        self.closeButton.isEnabled = true
        self.window.standardWindowButton(.closeButton)?.isEnabled = true
        switch result {
        case .success:
          self.status.stringValue = "安装完成。请在“系统设置 → 键盘 → 输入源”中添加灵果。若未出现，请保存工作后重新登录。"
        case .failure(let error):
          self.status.stringValue = error.localizedDescription
          self.installButton.title = "重试安装"
          self.installButton.isEnabled = true
        }
        self.window.setContentSize(
          NSSize(width: 528, height: max(360, self.status.fittingSize.height + 300)))
      }
    }
  }

  @objc func closeClicked() { if !busy { NSApp.terminate(nil) } }
  func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
    busy ? .terminateCancel : .terminateNow
  }
  func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

if CommandLine.arguments.contains("--selftest") {
  try installerTests()
} else {
  let application = NSApplication.shared
  let delegate = InstallerWindow()
  application.setActivationPolicy(.regular)
  application.delegate = delegate
  application.run()
}
