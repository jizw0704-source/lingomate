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
  var excludingPID: Int32?
  var backupPrepared: ((URL) throws -> Void)?

  static func quarantine(_ file: URL) throws -> Data? {
    let count = getxattr(file.path, "com.apple.quarantine", nil, 0, 0, XATTR_NOFOLLOW)
    if count < 0, errno == ENOATTR { return nil }
    guard count >= 0, count <= 4096 else {
      throw InstallFailure(message: "无法读取应用的系统来源标记，原文件未修改。")
    }
    var data = Data(count: count)
    let read = data.withUnsafeMutableBytes {
      getxattr(file.path, "com.apple.quarantine", $0.baseAddress, count, 0, XATTR_NOFOLLOW)
    }
    guard read == count else { throw InstallFailure(message: "系统来源标记读取不完整。") }
    return data
  }

  static func preserveQuarantine(_ data: Data?, at file: URL) throws {
    guard let data else { return }
    guard data.count <= 4096,
      data.withUnsafeBytes({
        setxattr(file.path, "com.apple.quarantine", $0.baseAddress, data.count, 0, XATTR_NOFOLLOW)
      }) == 0
    else { throw InstallFailure(message: "无法保留系统来源标记，已停止安装。") }
  }

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
    let deadline = Date().addingTimeInterval(120)
    while process.isRunning, Date() < deadline { Thread.sleep(forTimeInterval: 0.05) }
    if process.isRunning {
      process.terminate()
      Thread.sleep(forTimeInterval: 0.1)
      if process.isRunning { kill(process.processIdentifier, SIGKILL) }
      process.waitUntilExit()
      throw InstallFailure(message: "系统安装操作超时，未确认成功。")
    }
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
    let rows = try run("/bin/ps", ["-axo", "pid=,command="])
    let path = destination.appendingPathComponent(Self.binary).path
    guard
      !rows.split(separator: "\n").contains(where: {
        let row = $0.trimmingCharacters(in: .whitespaces)
        let parts = row.split(maxSplits: 1, whereSeparator: { $0 == " " || $0 == "\t" })
        if let first = parts.first, let pid = Int32(first) {
          if pid == excludingPID { return false }
          let command = parts.count > 1 ? String(parts[1]) : ""
          return command == path || command.hasPrefix(path + " ")
        }
        return row == path || row.hasPrefix(path + " ")
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
    // --norsrc also disables extended attributes/quarantine unless re-enabled.
    _ = try run(
      "/usr/bin/ditto",
      ["--norsrc", "--extattr", "--qtn", "--noacl", source.path, stage.path])
    try verify(stage)
    var backup: URL?
    if files.fileExists(atPath: destination.path) {
      let quarantine = try Self.quarantine(destination)
      try files.createDirectory(at: backups, withIntermediateDirectories: true)
      let archive = backups.appendingPathComponent(UUID().uuidString + ".zip")
      _ = try run(
        "/usr/bin/ditto",
        ["-c", "-k", "--sequesterRsrc", "--keepParent", destination.path, archive.path])
      // ZIP omits this special attribute; ditto propagates the archive's origin on extraction.
      try Self.preserveQuarantine(quarantine, at: archive)
      let extracted = temporary.appendingPathComponent("backup-check")
      _ = try run("/usr/bin/ditto", ["-x", "-k", archive.path, extracted.path])
      let restored = extracted.appendingPathComponent(destination.lastPathComponent)
      try verify(restored)
      guard try Self.quarantine(restored) == quarantine else {
        throw InstallFailure(message: "备份的系统来源标记校验失败，原应用未修改。")
      }
      for relative in ["Contents/Info.plist", Self.binary] {
        guard
          try Data(contentsOf: restored.appendingPathComponent(relative))
            == Data(contentsOf: destination.appendingPathComponent(relative))
        else { throw InstallFailure(message: "备份校验失败，原应用未修改。") }
      }
      try backupPrepared?(archive)
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
