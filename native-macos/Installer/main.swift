import AppKit
import Darwin
import Foundation

struct InstallFailure: LocalizedError {
  let message: String
  var errorDescription: String? { message }
}

struct Installer {
  static let binary = "Contents/MacOS/BilingualCompanion"
  static let identifier = "org.local.bilingualcompanion"
  static let registrar =
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
  let source: URL
  let destination: URL
  let backups: URL
  var run: (String, [String]) throws -> String = Installer.command

  static func command(_ executable: String, _ arguments: [String]) throws -> String {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = arguments
    // A file avoids pipe-buffer deadlocks when a system tool emits diagnostics.
    let log = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    guard FileManager.default.createFile(atPath: log.path, contents: nil) else {
      throw InstallFailure(message: "无法创建临时诊断文件。")
    }
    defer { try? FileManager.default.removeItem(at: log) }
    let handle = try FileHandle(forWritingTo: log)
    defer { try? handle.close() }
    process.standardOutput = handle
    process.standardError = handle
    try process.run()
    process.waitUntilExit()
    let output = String(decoding: try Data(contentsOf: log), as: UTF8.self)
    guard process.terminationStatus == 0 else {
      throw InstallFailure(message: "安装操作未完成（\(process.terminationStatus)）。\n\(output)")
    }
    return output
  }

  func verify(_ app: URL) throws {
    let values = try app.resourceValues(forKeys: [.isSymbolicLinkKey, .isDirectoryKey])
    guard values.isSymbolicLink != true, values.isDirectory == true,
      let info = NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist")),
      info["CFBundleIdentifier"] as? String == Self.identifier
    else { throw InstallFailure(message: "应用身份不符或路径是链接，已保留原文件。") }
    _ = try run("/usr/bin/codesign", ["--verify", "--deep", "--strict", app.path])
  }

  func ensureStopped() throws {
    let rows = try run("/bin/ps", ["-axo", "command"])
    let path = destination.appendingPathComponent(Self.binary).path
    guard
      !rows.split(separator: "\n").contains(where: {
        $0.trimmingCharacters(in: .whitespaces) == path
          || $0.trimmingCharacters(in: .whitespaces).hasPrefix(path + " ")
      })
    else {
      throw InstallFailure(
        message: "灵果仍在运行。请切换到其他输入法，关闭灵果设置窗口，并在“活动监视器”中退出 BilingualCompanion 后重试。安装器不会关闭你的应用。")
    }
  }

  func install() throws -> URL? {
    let files = FileManager.default
    try verify(source)
    try files.createDirectory(
      at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
    let lock = destination.deletingLastPathComponent().appendingPathComponent(
      ".lingomate-install.lock")
    let descriptor = open(lock.path, O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw InstallFailure(message: "无法创建安装锁，原文件未修改。") }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
      throw InstallFailure(message: "另一个安装器正在工作，请稍后重试。")
    }
    if files.fileExists(atPath: destination.path) { try verify(destination) }
    try ensureStopped()
    let temporary = destination.deletingLastPathComponent().appendingPathComponent(
      ".lingomate-" + UUID().uuidString)
    try files.createDirectory(at: temporary, withIntermediateDirectories: false)
    var retainRecovery = false
    defer { if !retainRecovery { try? files.removeItem(at: temporary) } }
    let stage = temporary.appendingPathComponent("BilingualCompanion.app")
    let previous = temporary.appendingPathComponent("previous.app")
    _ = try run("/usr/bin/ditto", ["--noextattr", "--norsrc", "--noacl", source.path, stage.path])
    try verify(stage)
    var backup: URL?
    if files.fileExists(atPath: destination.path) {
      try files.createDirectory(at: backups, withIntermediateDirectories: true)
      let archive = backups.appendingPathComponent(UUID().uuidString + ".zip")
      _ = try run(
        "/usr/bin/ditto",
        ["-c", "-k", "--sequesterRsrc", "--keepParent", destination.path, archive.path])
      let extracted = temporary.appendingPathComponent("backup-check")
      _ = try run("/usr/bin/ditto", ["-x", "-k", archive.path, extracted.path])
      let restored = extracted.appendingPathComponent(destination.lastPathComponent)
      try verify(restored)
      for relative in ["Contents/Info.plist", Self.binary] {
        guard
          try Data(contentsOf: restored.appendingPathComponent(relative))
            == Data(contentsOf: destination.appendingPathComponent(relative))
        else { throw InstallFailure(message: "备份校验失败，原应用未修改。") }
      }
      backup = archive
    }
    try ensureStopped()
    var movedOld = false
    var movedNew = false
    do {
      if files.fileExists(atPath: destination.path) {
        _ = try run(Self.registrar, ["-u", destination.path])
        try files.moveItem(at: destination, to: previous)
        movedOld = true
      }
      try files.moveItem(at: stage, to: destination)
      movedNew = true
      try verify(destination)
      _ = try run(Self.registrar, ["-f", destination.path])
      _ = try run(destination.appendingPathComponent(Self.binary).path, ["--register"])
    } catch {
      do {
        if movedNew {
          _ = try? run(Self.registrar, ["-u", destination.path])
          try files.removeItem(at: destination)
        }
        if movedOld { try files.moveItem(at: previous, to: destination) }
        if files.fileExists(atPath: destination.path) {
          _ = try run(Self.registrar, ["-f", destination.path])
          _ = try run(destination.appendingPathComponent(Self.binary).path, ["--register"])
        }
      } catch {
        retainRecovery = true
        throw InstallFailure(message: "安装与恢复登记未完成，旧文件已保留。恢复目录：\(temporary.path)")
      }
      throw error
    }
    return backup
  }
}

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
