// 设置辅助进程中的显式本地操作；通过同一Rust锁与校验管理词库，不初始化输入引擎。
import Darwin
import Foundation

struct PersonalWord: Decodable {
  let text: String
  let syllables: [String]
  let aliases: [String]
  let count: UInt32
  let last: UInt64
  let prioritized: Bool
  var id: String { text + "\u{1f}" + syllables.joined(separator: "'") }
}

final class PersonalVocabularyStore {
  let file: URL
  private let resources: URL
  private let temporaryDirectory: URL?
  init(resources: URL, file: URL) {
    self.resources = resources
    self.file = file
    temporaryDirectory = nil
  }
  init(preview: Bool) {
    resources = Bundle.main.resourceURL!
    if preview {
      let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
        "lingomate-vocabulary-" + UUID().uuidString)
      temporaryDirectory = directory
      file = directory.appendingPathComponent("words.json")
      try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      let fixture = """
        {"version":1,"sequence":4,"entries":[
        {"text":"钙钛镍","pinyin":"gaitainie","syllables":["gai","tai","nie"],"count":2,"last":4},
        {"text":"学习","pinyin":"xuexi","syllables":["xue","xi"],"count":3,"last":3},
        {"text":"市","pinyin":"shi","syllables":["shi"],"count":1,"last":2},
        {"text":"市","pinyin":"s","syllables":["shi"],"count":1,"last":2}]}
        """
      try? Data(fixture.utf8).write(to: file)
    } else {
      temporaryDirectory = nil
      file = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
        "Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json")
    }
  }
  deinit {
    cleanup()
  }
  func cleanup() {
    if let temporaryDirectory { try? FileManager.default.removeItem(at: temporaryDirectory) }
  }

  func snapshot() throws -> [PersonalWord] { try perform(["action": "list"]) }
  func delete(_ word: PersonalWord) throws -> [PersonalWord] {
    try perform(["action": "delete", "text": word.text, "syllables": word.syllables])
  }
  func prioritize(_ word: PersonalWord, enabled: Bool) throws -> [PersonalWord] {
    try perform([
      "action": "prioritize", "text": word.text, "syllables": word.syllables,
      "prioritized": enabled,
    ])
  }
  private func perform(_ payload: [String: Any]) throws -> [PersonalWord] {
    let process = Process()
    let input = Pipe()
    let output = Pipe()
    process.executableURL = resources.appendingPathComponent("bilingual-ime-bridge")
    process.arguments = ["--vocabulary", file.path]
    process.standardInput = input
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    defer {
      if process.isRunning { kill(process.processIdentifier, SIGKILL) }
      if process.processIdentifier > 0 { process.waitUntilExit() }
      try? input.fileHandleForWriting.close()
      try? input.fileHandleForReading.close()
      try? output.fileHandleForWriting.close()
      try? output.fileHandleForReading.close()
    }
    try process.run()
    try input.fileHandleForReading.close()
    try output.fileHandleForWriting.close()
    _ = fcntl(input.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
    try input.fileHandleForWriting.write(
      contentsOf: JSONSerialization.data(withJSONObject: payload))
    try input.fileHandleForWriting.close()
    let deadline = DispatchTime.now().uptimeNanoseconds + 3_000_000_000
    var data = Data()
    while !data.contains(10) {
      let now = DispatchTime.now().uptimeNanoseconds
      guard now < deadline else { throw failure("词库操作超时，请刷新后重试。") }
      var descriptor = pollfd(
        fd: output.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0)
      let result = poll(&descriptor, 1, Int32(max(1, (deadline - now) / 1_000_000)))
      if result < 0 && errno == EINTR { continue }
      guard result > 0 else { throw failure("词库操作超时，请刷新后重试。") }
      var bytes = [UInt8](repeating: 0, count: 8192)
      let count = Darwin.read(descriptor.fd, &bytes, bytes.count)
      if count < 0 && errno == EINTR { continue }
      guard count > 0, data.count + count <= 4 * 1024 * 1024 else {
        throw failure("词库读取失败，请检查文件格式与存储权限后刷新。")
      }
      data.append(contentsOf: bytes.prefix(count))
    }
    struct Response: Decodable {
      let words: [PersonalWord]?
      let error: String?
    }
    let response = try JSONDecoder().decode(Response.self, from: data)
    if let error = response.error { throw failure(error) }
    guard let words = response.words else { throw failure("词库响应无效，请刷新后重试。") }
    return words
  }
  private func failure(_ message: String) -> NSError {
    NSError(domain: "PersonalVocabulary", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
  }
}
