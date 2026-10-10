// Only fixed public release metadata; no input text, credentials or account access.
import CryptoKit
import Darwin
import Foundation

struct UpdateVersion: Comparable {
  let parts: [Int]
  init(_ text: String) throws {
    let segments = text.split(separator: ".", omittingEmptySubsequences: false)
    guard segments.count == 3,
      segments.allSatisfy({
        !$0.isEmpty && $0.allSatisfy({ $0 >= "0" && $0 <= "9" })
          && ($0 == "0" || !$0.hasPrefix("0")) && Int($0) != nil && $0.count <= 9
      })
    else { throw InstallFailure(message: "更新版本格式无效。") }
    parts = segments.map { Int($0)! }
  }
  static func < (lhs: Self, rhs: Self) -> Bool { lhs.parts.lexicographicallyPrecedes(rhs.parts) }
}

struct MacRelease: Codable {
  let schema: Int
  let platform: String
  let arch: String
  let channel: String
  let minimumMacOSMajor: Int
  let status: String
  var version: String?
  var asset: String?
  var size: Int?
  var sha256: String?
  var downloadURL: String?
  var notes: String?
  enum CodingKeys: String, CodingKey {
    case schema, platform, arch, channel, status, version, asset, size, sha256, notes
    case minimumMacOSMajor = "minimum_macos_major"
    case downloadURL = "download_url"
  }

  func available(after current: String, osMajor: Int) throws -> Bool {
    guard schema == 1, platform == "macos", arch == "arm64", channel == "preview",
      minimumMacOSMajor >= 13, minimumMacOSMajor <= osMajor
    else { throw InstallFailure(message: "更新与当前 Mac 系统或芯片不匹配。") }
    let installed = try UpdateVersion(current)
    if status == "unpublished" { return false }
    guard status == "published", let version, let size, let sha256, let downloadURL,
      size > 0, size <= 134_217_728,
      sha256.range(of: "^[a-f0-9]{64}$", options: .regularExpression) != nil,
      asset == "lingomate-macos-arm64-\(version).zip", (notes?.utf8.count ?? 0) <= 8000,
      downloadURL
        == "https://github.com/jizw0704-source/lingomate/releases/download/macos-v\(version)/\(asset!)"
    else { throw InstallFailure(message: "更新清单或校验信息无效。") }
    _ = try UpdateDownload.allowed(URL(string: downloadURL), redirect: false)
    return try UpdateVersion(version) > installed
  }
}

final class UpdateDownload: NSObject, URLSessionDataDelegate, @unchecked Sendable {
  static let feed = URL(
    string:
      "https://raw.githubusercontent.com/jizw0704-source/lingomate/main/updates/macos-preview.json")!
  private let lock = NSLock()
  private var session: URLSession?
  private var task: URLSessionDataTask?
  private var cancelled = false
  private var error: Error?
  private var handle: FileHandle?
  private var limit = 0
  private var received = 0
  private var hops = 0
  private let done = DispatchSemaphore(value: 0)

  static func allowed(_ value: URL?, redirect: Bool) throws -> URL {
    guard let value, value.scheme == "https", value.port == nil || value.port == 443,
      value.user == nil, value.password == nil, value.fragment == nil,
      value == feed
        || (value.host == "github.com"
          && value.path.hasPrefix("/jizw0704-source/lingomate/releases/download/"))
        || (redirect
          && ["release-assets.githubusercontent.com", "objects.githubusercontent.com"].contains(
            value.host ?? ""))
    else { throw InstallFailure(message: "更新地址不属于灵果的可信发布源。") }
    return value
  }

  func cancel() {
    lock.lock()
    cancelled = true
    task?.cancel()
    lock.unlock()
  }
  func fetch(_ url: URL, to destination: URL, limit: Int) throws {
    _ = try Self.allowed(url, redirect: false)
    self.limit = limit
    guard FileManager.default.createFile(atPath: destination.path, contents: nil) else {
      throw InstallFailure(message: "无法创建更新下载文件。")
    }
    handle = try FileHandle(forWritingTo: destination)
    defer {
      try? handle?.close()
      session?.invalidateAndCancel()
    }
    let config = URLSessionConfiguration.ephemeral
    config.timeoutIntervalForRequest = 15
    config.timeoutIntervalForResource = 180
    config.httpCookieStorage = nil
    config.urlCredentialStorage = nil
    let queue = OperationQueue()
    queue.maxConcurrentOperationCount = 1
    session = URLSession(configuration: config, delegate: self, delegateQueue: queue)
    var request = URLRequest(url: url)
    request.setValue("LingoMate-Mac-Updater", forHTTPHeaderField: "User-Agent")
    lock.lock()
    task = session!.dataTask(with: request)
    let wasCancelled = cancelled
    if !wasCancelled { task!.resume() }
    lock.unlock()
    if wasCancelled { throw CancellationError() }
    guard done.wait(timeout: .now() + 185) == .success else {
      cancel()
      throw InstallFailure(message: "更新下载超时，请稍后重试。")
    }
    if let error { throw error }
    lock.lock()
    let stopped = cancelled
    lock.unlock()
    if stopped { throw CancellationError() }
  }
  func urlSession(
    _ session: URLSession, task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void
  ) {
    do {
      hops += 1
      guard hops <= 3 else { throw InstallFailure(message: "更新重定向次数过多。") }
      _ = try Self.allowed(request.url, redirect: true)
      completionHandler(request)
    } catch {
      self.error = error
      completionHandler(nil)
      task.cancel()
    }
  }
  func urlSession(
    _ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
  ) {
    guard let http = response as? HTTPURLResponse, http.statusCode == 200,
      response.expectedContentLength <= Int64(limit)
    else {
      error = InstallFailure(message: "更新服务器响应或文件大小无效。")
      completionHandler(.cancel)
      return
    }
    completionHandler(.allow)
  }
  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
    received += data.count
    guard received <= limit else {
      error = InstallFailure(message: "更新文件超出大小限制。")
      dataTask.cancel()
      return
    }
    do { try handle?.write(contentsOf: data) } catch {
      self.error = InstallFailure(message: "更新文件保存失败。")
      dataTask.cancel()
    }
  }
  func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError failure: Error?)
  {
    if failure != nil, error == nil { error = InstallFailure(message: "暂时无法连接更新服务器，请检查网络后重试。") }
    done.signal()
  }
}

// Inspect central and local ZIP paths before invoking the system extractor.
enum UpdateArchive {
  static func inspect(_ data: Data, backup: Bool = false) throws {
    func number(_ at: Int, _ length: Int) throws -> Int {
      guard at >= 0, at + length <= data.count else { throw InstallFailure(message: "更新压缩包结构不完整。") }
      return (0..<length).reduce(0) { $0 | (Int(data[at + $1]) << ($1 * 8)) }
    }
    guard data.count >= 22, data.count <= 134_217_728 else {
      throw InstallFailure(message: "更新压缩包大小无效。")
    }
    var end: Int?
    for offset in stride(from: data.count - 22, through: max(0, data.count - 65_557), by: -1) {
      if try number(offset, 4) == 0x0605_4b50,
        try offset + 22 + number(offset + 20, 2) == data.count
      {
        end = offset
        break
      }
    }
    guard let end, try number(end + 4, 4) == 0 else {
      throw InstallFailure(message: "不支持此更新压缩包格式。")
    }
    let count = try number(end + 10, 2)
    let start = try number(end + 16, 4)
    guard count > 0, count <= 4096, try number(end + 8, 2) == count,
      try start + number(end + 12, 4) == end
    else { throw InstallFailure(message: "更新压缩包目录无效。") }
    var cursor = start
    var total = 0
    var seen = Set<String>()
    for _ in 0..<count {
      guard try number(cursor, 4) == 0x0201_4b50 else {
        throw InstallFailure(message: "更新压缩包目录损坏。")
      }
      let flags = try number(cursor + 8, 2)
      let method = try number(cursor + 10, 2)
      let size = try number(cursor + 24, 4)
      let packed = try number(cursor + 20, 4)
      let length = try number(cursor + 28, 2)
      let extra = try number(cursor + 30, 2)
      let comment = try number(cursor + 32, 2)
      let local = try number(cursor + 42, 4)
      let attrs = try number(cursor + 38, 4)
      guard cursor + 46 + length + extra + comment <= end else {
        throw InstallFailure(message: "更新压缩包路径损坏。")
      }
      let name = String(decoding: data[(cursor + 46)..<(cursor + 46 + length)], as: UTF8.self)
      let parts = name.split(separator: "/", omittingEmptySubsequences: false)
      let allowedRoot =
        name.hasPrefix("BilingualCompanion.app/")
        || (backup && name.hasPrefix("__MACOSX/"))
      let kind = (attrs >> 16) & 0xf000
      total += size
      guard allowedRoot,
        name.utf8.allSatisfy({
          (45...57).contains($0) || (65...90).contains($0) || $0 == 95 || (97...122).contains($0)
        }),
        !parts.contains(".."), !parts.contains("."), !name.contains("//"),
        seen.insert(name.trimmingCharacters(in: CharacterSet(charactersIn: "/")).lowercased())
          .inserted,
        flags & 0x41 == 0, [0, 8].contains(method), [0, 0x8000, 0x4000].contains(kind),
        kind != 0x4000 || name.hasSuffix("/"), kind != 0x8000 || !name.hasSuffix("/"),
        size <= 134_217_728, packed <= 134_217_728, total <= 268_435_456,
        try number(cursor + 34, 2) == 0, try number(local, 4) == 0x0403_4b50,
        try number(local + 6, 2) == flags, try number(local + 8, 2) == method,
        try number(local + 26, 2) == length
      else { throw InstallFailure(message: "更新压缩包含无效路径、重复文件、链接或超大文件。") }
      let payload = try local + 30 + length + number(local + 28, 2)
      guard payload + packed <= start,
        data[(local + 30)..<(local + 30 + length)] == data[(cursor + 46)..<(cursor + 46 + length)]
      else { throw InstallFailure(message: "更新压缩包文件记录不一致。") }
      cursor += 46 + length + extra + comment
    }
    guard cursor == end, seen.contains("bilingualcompanion.app/contents/info.plist"),
      seen.contains("bilingualcompanion.app/contents/macos/bilingualcompanion")
    else { throw InstallFailure(message: "更新压缩包缺少灵果应用。") }
  }
  static func hash(_ file: URL) throws -> String {
    let handle = try FileHandle(forReadingFrom: file)
    defer { try? handle.close() }
    var value = SHA256()
    while let data = try handle.read(upToCount: 65_536), !data.isEmpty { value.update(data: data) }
    return value.finalize().map { String(format: "%02x", $0) }.joined()
  }
}

struct UpdateReceipt: Codable {
  let installed: String
  let previous: String
  let archive: String
  let sha256: String
  let quarantine: Data?
}

struct MacUpdateStore {
  let destination: URL
  let directory: URL
  var run: (String, [String]) throws -> String = Installer.command
  var isolated = false
  var receiptURL: URL { directory.appendingPathComponent("receipt.json") }
  var backups: URL {
    directory.deletingLastPathComponent().appendingPathComponent("InstallBackups")
  }
  func version(_ app: URL) throws -> String {
    guard let info = NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist")),
      info["CFBundleIdentifier"] as? String == Installer.identifier,
      let version = info["CFBundleShortVersionString"] as? String
    else { throw InstallFailure(message: "未找到已安装的灵果版本。") }
    _ = try UpdateVersion(version)
    return version
  }
  func verify(_ app: URL, release: MacRelease? = nil) throws {
    let enumerator = FileManager.default.enumerator(
      at: app, includingPropertiesForKeys: [.isSymbolicLinkKey])
    while let file = enumerator?.nextObject() as? URL {
      if try file.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink == true {
        throw InstallFailure(message: "更新应用包含链接，已停止安装。")
      }
    }
    try Installer(source: app, destination: destination, backups: backups, run: run).verify(app)
    let value = try version(app)
    if let release {
      guard value == release.version else { throw InstallFailure(message: "应用版本与更新清单不一致。") }
      guard let info = NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist")),
        let minimum = info["LSMinimumSystemVersion"] as? String,
        (1...3).contains(minimum.split(separator: ".", omittingEmptySubsequences: false).count)
      else { throw InstallFailure(message: "应用需要更高版本的 macOS。") }
      let segments = minimum.split(separator: ".", omittingEmptySubsequences: false).map(
        String.init)
      let required = try UpdateVersion(
        (segments + Array(repeating: "0", count: 3 - segments.count)).joined(separator: "."))
      let os = ProcessInfo.processInfo.operatingSystemVersion
      guard
        try required <= UpdateVersion("\(os.majorVersion).\(os.minorVersion).\(os.patchVersion)")
      else {
        throw InstallFailure(message: "应用需要更高版本的 macOS。")
      }
      let binary = try Data(
        contentsOf: app.appendingPathComponent(Installer.binary), options: .mappedIfSafe)
      guard binary.count >= 8, Array(binary.prefix(8)) == [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1]
      else { throw InstallFailure(message: "更新应用不是 Apple 芯片版本。") }
      if !isolated {
        // Keep system origin protection. Never strip quarantine or weaken Gatekeeper.
        _ = try run(
          "/usr/bin/xattr",
          [
            "-w", "com.apple.quarantine",
            "0081;\(String(Int(Date().timeIntervalSince1970), radix: 16));LingoMateUpdater;\(UUID().uuidString)",
            app.path,
          ])
        _ = try run("/usr/sbin/spctl", ["--assess", "--type", "execute", app.path])
      }
    }
  }
  func extract(_ zip: URL, to root: URL, backup: Bool = false) throws -> URL {
    try UpdateArchive.inspect(Data(contentsOf: zip, options: .mappedIfSafe), backup: backup)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    _ = try run("/usr/bin/ditto", ["-x", "-k", zip.path, root.path])
    return root.appendingPathComponent("BilingualCompanion.app")
  }
  func prepare(_ zip: URL, release: MacRelease, to root: URL) throws -> URL {
    guard try zip.resourceValues(forKeys: [.fileSizeKey]).fileSize == release.size,
      try UpdateArchive.hash(zip) == release.sha256
    else { throw InstallFailure(message: "下载校验失败，旧版本未修改。") }
    let app = try extract(zip, to: root)
    try verify(app, release: release)
    return app
  }
  func install(_ app: URL, release: MacRelease) throws {
    guard
      try release.available(
        after: version(destination),
        osMajor: ProcessInfo.processInfo.operatingSystemVersion.majorVersion)
    else { throw InstallFailure(message: "没有可安装的更高版本。") }
    try verify(app, release: release)
    let files = FileManager.default
    try files.createDirectory(at: directory, withIntermediateDirectories: true)
    let priorReceipt = try? Data(contentsOf: receiptURL)
    var installer = Installer(source: app, destination: destination, backups: backups, run: run)
    installer.excludingPID = getpid()
    installer.backupPrepared = { archive in
      let receipt = UpdateReceipt(
        installed: release.version!, previous: try version(destination),
        archive: archive.lastPathComponent, sha256: try UpdateArchive.hash(archive),
        quarantine: try Installer.quarantine(destination))
      try JSONEncoder().encode(receipt).write(to: receiptURL, options: .atomic)
    }
    do { _ = try installer.install() } catch {
      if let priorReceipt {
        try? priorReceipt.write(to: receiptURL, options: .atomic)
      } else {
        try? files.removeItem(at: receiptURL)
      }
      throw error
    }
  }
  func restore(to root: URL) throws {
    let data = try Data(contentsOf: receiptURL)
    guard data.count <= 8192 else { throw InstallFailure(message: "恢复记录无效。") }
    let receipt = try JSONDecoder().decode(UpdateReceipt.self, from: data)
    guard receipt.installed == (try version(destination)), receipt.archive.hasSuffix(".zip"),
      UUID(uuidString: String(receipt.archive.dropLast(4))) != nil
    else { throw InstallFailure(message: "当前版本与恢复记录不一致。") }
    let archive = backups.appendingPathComponent(receipt.archive)
    guard try archive.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true,
      try UpdateArchive.hash(archive) == receipt.sha256
    else { throw InstallFailure(message: "旧版备份校验失败，当前版本未修改。") }
    let app = try extract(archive, to: root, backup: true)
    // A backup copied to a different filesystem may lose its extended attributes.
    try Installer.preserveQuarantine(receipt.quarantine, at: app)
    try verify(app)
    guard try version(app) == receipt.previous else { throw InstallFailure(message: "备份版本不一致。") }
    var installer = Installer(source: app, destination: destination, backups: backups, run: run)
    installer.excludingPID = getpid()
    _ = try installer.install()
    // Retain receipt/ZIP as evidence; version guard prevents repeated stale restore.
  }
}
