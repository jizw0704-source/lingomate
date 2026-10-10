import AppKit
import Foundation

enum MacUpdateTests {
  static func run() throws {
    var passed = 0
    func check(_ condition: Bool, _ name: String) throws {
      guard condition else { throw InstallFailure(message: "selftest: " + name) }
      passed += 1
    }
    func reject(_ name: String, _ action: () throws -> Void) throws {
      var failed = false
      do { try action() } catch { failed = true }
      try check(failed, name)
    }
    try check(try UpdateVersion("0.12.0") > UpdateVersion("0.11.9"), "numeric version")
    for value in ["1.2", "01.2.3", "1.2.3-beta", "../1.2.3", "999999999999.0.0", "1..2"] {
      try reject("invalid version") { _ = try UpdateVersion(value) }
    }
    for value in [
      "http://github.com/jizw0704-source/lingomate/releases/download/x/a.zip",
      "https://evil.example/a", "https://github.com/another/repo/releases/download/a",
      "https://github.com.evil.example/a",
      "https://github.com:444/jizw0704-source/lingomate/releases/download/x/a",
      "https://user@github.com/jizw0704-source/lingomate/releases/download/x/a",
    ] {
      try reject("untrusted source") {
        _ = try UpdateDownload.allowed(URL(string: value), redirect: false)
      }
    }
    try reject("untrusted redirect") {
      _ = try UpdateDownload.allowed(URL(string: "https://evil.example/a"), redirect: true)
    }
    try check(
      try UpdateDownload.allowed(
        URL(string: "https://release-assets.githubusercontent.com/example"), redirect: true
      ).host == "release-assets.githubusercontent.com", "CDN allowed")
    try check(!UpdatePreferences().due(), "default opt out")
    try check(!UpdatePreferences(autoCheck: true, lastChecked: 100).due(now: 200), "not due")
    try check(
      UpdatePreferences(autoCheck: true, lastChecked: 0).due(now: 86_401), "daily check due")

    let files = FileManager.default
    let root = files.temporaryDirectory.appendingPathComponent(
      "lingomate-update-tests-" + UUID().uuidString)
    try files.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? files.removeItem(at: root) }
    func bundle(_ app: URL, version: String) throws {
      try files.createDirectory(
        at: app.appendingPathComponent("Contents/MacOS"), withIntermediateDirectories: true)
      try files.createDirectory(
        at: app.appendingPathComponent("Contents/Resources"), withIntermediateDirectories: true)
      try Data([0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1]).write(
        to: app.appendingPathComponent(Installer.binary))
      for name in ["a.txt", "b.txt"] {
        try Data(name.utf8).write(to: app.appendingPathComponent("Contents/Resources/" + name))
      }
      try PropertyListSerialization.data(
        fromPropertyList: [
          "CFBundleIdentifier": Installer.identifier, "CFBundleShortVersionString": version,
          "LSMinimumSystemVersion": "13.0",
        ], format: .xml, options: 0
      ).write(to: app.appendingPathComponent("Contents/Info.plist"))
    }
    let source = root.appendingPathComponent("source/BilingualCompanion.app")
    let installed = root.appendingPathComponent("input/BilingualCompanion.app")
    try bundle(source, version: "0.13.0")
    try bundle(installed, version: "0.12.0")
    _ = try Installer.command(
      "/usr/bin/xattr",
      ["-w", "com.apple.quarantine", "0081;00000000;LingoMate-Test;", installed.path])
    let archive = root.appendingPathComponent("update.zip")
    _ = try Installer.command(
      "/usr/bin/ditto",
      ["-c", "-k", "--norsrc", "--noextattr", "--keepParent", source.path, archive.path])
    var release = MacRelease(
      schema: 1, platform: "macos", arch: "arm64", channel: "preview", minimumMacOSMajor: 13,
      status: "published", version: "0.13.0", asset: "lingomate-macos-arm64-0.13.0.zip",
      size: try archive.resourceValues(forKeys: [.fileSizeKey]).fileSize,
      sha256: try UpdateArchive.hash(archive),
      downloadURL:
        "https://github.com/jizw0704-source/lingomate/releases/download/macos-v0.13.0/lingomate-macos-arm64-0.13.0.zip",
      notes: "Synthetic release")
    try check(try release.available(after: "0.12.0", osMajor: 13), "higher release")
    try check(try !release.available(after: "0.13.0", osMajor: 13), "equal release")
    try check(try !release.available(after: "0.14.0", osMajor: 13), "downgrade rejected")
    let empty = MacRelease(
      schema: 1, platform: "macos", arch: "arm64", channel: "preview", minimumMacOSMajor: 13,
      status: "unpublished")
    try check(try !empty.available(after: "0.12.0", osMajor: 13), "unpublished empty")
    try reject("old system") { _ = try release.available(after: "0.12.0", osMajor: 12) }
    for pair in [("arch", "x64"), ("platform", "windows"), ("status", "invalid")] {
      var dictionary =
        try JSONSerialization.jsonObject(with: JSONEncoder().encode(release)) as! [String: Any]
      dictionary[pair.0] = pair.1
      let wrong = try JSONDecoder().decode(
        MacRelease.self, from: JSONSerialization.data(withJSONObject: dictionary))
      try reject("wrong platform or status") {
        _ = try wrong.available(after: "0.12.0", osMajor: 13)
      }
    }
    var changed = release
    changed.sha256 = "bad"
    try reject("bad digest metadata") { _ = try changed.available(after: "0.12.0", osMajor: 13) }
    changed = release
    changed.downloadURL =
      "https://github.com/jizw0704-source/lingomate/releases/download/macos-v0.14.0/lingomate-macos-arm64-0.13.0.zip"
    try reject("release URL mismatch") { _ = try changed.available(after: "0.12.0", osMajor: 13) }
    let originalZIP = try Data(contentsOf: archive)
    try UpdateArchive.inspect(originalZIP)
    passed += 1
    let cancelled = UpdateDownload()
    cancelled.cancel()
    try reject("cancel before request") {
      try cancelled.fetch(
        UpdateDownload.feed, to: root.appendingPathComponent("cancelled.json"), limit: 65_536)
    }
    var duplicate = originalZIP
    let name = Data("b.txt".utf8)
    while let range = duplicate.range(of: name) {
      duplicate.replaceSubrange(range, with: Data("a.txt".utf8))
    }
    try reject("duplicate paths") { try UpdateArchive.inspect(duplicate) }
    var traversal = originalZIP
    if let range = traversal.range(of: Data("BilingualCompanion.app/Contents/Info.plist".utf8)) {
      traversal[range.lowerBound] = 46
    }
    try reject("unsafe or differing local path") { try UpdateArchive.inspect(traversal) }
    var link = originalZIP
    if let central = link.range(of: Data([0x50, 0x4b, 0x01, 0x02])) {
      link[central.lowerBound + 41] = 0xa0
    }
    try reject("symlink entry") { try UpdateArchive.inspect(link) }
    try reject("truncated ZIP") { try UpdateArchive.inspect(originalZIP.prefix(30)) }
    var calls: [String] = []
    var failRegistration = false
    let mock: (String, [String]) throws -> String = { tool, arguments in
      if tool == "/bin/ps" { return "" }
      if tool == "/usr/bin/codesign" { return "" }
      if tool == Installer.registrar {
        calls.append(arguments.joined(separator: " "))
        if failRegistration, arguments.first == "-f",
          let info = NSDictionary(
            contentsOf: installed.appendingPathComponent("Contents/Info.plist")),
          info["CFBundleShortVersionString"] as? String == "0.13.0"
        {
          throw InstallFailure(message: "synthetic register failure")
        }
        return ""
      }
      if tool == installed.appendingPathComponent(Installer.binary).path { return "" }
      return try Installer.command(tool, arguments)
    }
    var store = MacUpdateStore(
      destination: installed, directory: root.appendingPathComponent("Support/Updates"), run: mock,
      isolated: true)
    release.sha256 = String(repeating: "0", count: 64)
    try reject("tampering before extraction") {
      _ = try store.prepare(archive, release: release, to: root.appendingPathComponent("bad"))
    }
    try check(
      !files.fileExists(atPath: root.appendingPathComponent("bad").path),
      "invalid download writes no stage")
    release.sha256 = try UpdateArchive.hash(archive)
    let prepared = try store.prepare(
      archive, release: release, to: root.appendingPathComponent("prepared"))
    try check(try store.version(prepared) == "0.13.0", "verified extraction")
    let newOrigin = Data("0081;00000001;LingoMate-New-Test;".utf8)
    try Installer.preserveQuarantine(newOrigin, at: prepared)
    try store.install(prepared, release: release)
    try check(try store.version(installed) == "0.13.0", "transaction replaces bundle")
    try check(try Installer.quarantine(installed) == newOrigin, "new bundle retains origin")
    let receipt = try JSONDecoder().decode(
      UpdateReceipt.self, from: Data(contentsOf: store.receiptURL))
    try check(
      receipt.previous == "0.12.0"
        && files.fileExists(atPath: store.backups.appendingPathComponent(receipt.archive).path),
      "backup and receipt")
    let backupFile = store.backups.appendingPathComponent(receipt.archive)
    try check(
      try Installer.quarantine(backupFile) == receipt.quarantine, "backup origin retained")
    // Simulate moving the ZIP through a filesystem that does not carry extended attributes.
    try Data(contentsOf: backupFile).write(to: backupFile, options: .atomic)
    try store.restore(to: root.appendingPathComponent("restore"))
    try check(try store.version(installed) == "0.12.0", "explicit verified restore")
    try check(
      try Installer.command("/usr/bin/xattr", ["-p", "com.apple.quarantine", installed.path])
        .trimmingCharacters(in: .whitespacesAndNewlines) == "0081;00000000;LingoMate-Test;",
      "rollback preserves quarantine")
    try reject("stale restore blocked") {
      try store.restore(to: root.appendingPathComponent("again"))
    }
    let prior = try Data(contentsOf: store.receiptURL)
    failRegistration = true
    try reject("registration failure surfaced") { try store.install(prepared, release: release) }
    try check(try store.version(installed) == "0.12.0", "registration failure rolls back")
    try check(try Data(contentsOf: store.receiptURL) == prior, "prior receipt preserved")
    var installer = Installer(
      source: source, destination: installed, backups: store.backups, run: mock)
    installer.excludingPID = getpid()
    installer.run = { _, _ in
      "\(getpid()) \(installed.appendingPathComponent(Installer.binary).path) --updates"
    }
    try installer.ensureStopped()
    passed += 1
    installer.run = { _, _ in
      "\(getpid() + 1) \(installed.appendingPathComponent(Installer.binary).path)"
    }
    try reject("running service blocks overwrite") { try installer.ensureStopped() }
    let lease = try UpdateLease(directory: root.appendingPathComponent("lease"))
    try withExtendedLifetime(lease) {
      try reject("duplicate updater blocked") {
        _ = try UpdateLease(directory: root.appendingPathComponent("lease"))
      }
    }
    store.isolated = false
    store.run = { tool, _ in
      if tool == "/usr/sbin/spctl" { throw InstallFailure(message: "synthetic security rejection") }
      return ""
    }
    try reject("system assessment rejection") { try store.verify(prepared, release: release) }
    print(
      "Mac updates: \(passed) isolated version, source, archive, install, restore and failure checks passed."
    )
  }

  static func windowChecks() {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let store = MacUpdateStore(
      destination: root.appendingPathComponent("BilingualCompanion.app"), directory: root)
    let controller = MacUpdateController(
      store: store, preview: true, background: false, preferences: UpdatePreferences())
    for width in [560.0, 440.0] {
      controller.window.setContentSize(NSSize(width: width, height: 424))
      controller.window.contentView?.layoutSubtreeIfNeeded()
      for button in [controller.primary, controller.restore, controller.close] {
        precondition(button.frame.width >= 44 && button.frame.height >= 44)
      }
      precondition(!controller.primary.frame.intersects(controller.restore.frame))
      precondition(!controller.restore.frame.intersects(controller.close.frame))
      controller.fixture("loading")
      precondition(!controller.primary.isEnabled && controller.close.isEnabled)
      controller.fixture("installing")
      precondition(!controller.close.isEnabled && controller.critical)
      controller.fixture("error")
      precondition(controller.primary.isEnabled && controller.close.isEnabled)
      precondition(controller.primary.accessibilityLabel() == "重新检查")
      controller.fixture("ready")
      precondition(controller.primary.accessibilityLabel() == "安装更新")
    }
    controller.cleanup()
    print(
      "Mac update window: isolated wide/narrow controls, cancellation, retry and critical states passed."
    )
  }
}
