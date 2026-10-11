// 本地学习与账号缓存隔离；只存确认输出的词条及聚合计数。
import Darwin
import Foundation

struct LocalLearningData: Codable {
  var version = 1
  var words: [LearnedWord] = []
}

final class LocalLearningStore {
  let directory: URL
  var file: URL { directory.appendingPathComponent("words.json") }
  init(directory: URL) { self.directory = directory }

  private func transaction<T>(_ write: Bool, _ body: (inout LocalLearningData) throws -> T) throws
    -> T
  {
    try FileManager.default.createDirectory(
      at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
    let fd = open(directory.appendingPathComponent("words.lock").path, O_CREAT | O_RDWR, 0o600)
    guard fd >= 0 else { throw AccountError.storage }
    defer { close(fd) }
    let deadline = Date().addingTimeInterval(1)
    while flock(fd, LOCK_EX | LOCK_NB) != 0 {
      guard Date() < deadline else { throw AccountError.storage }
      Thread.sleep(forTimeInterval: 0.01)
    }
    defer { flock(fd, LOCK_UN) }
    var data = LocalLearningData()
    if FileManager.default.fileExists(atPath: file.path) {
      let size = (try FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?
        .intValue
      guard let size, size <= 4_000_000 else { throw AccountError.storage }
      let bytes = try Data(contentsOf: file)
      guard bytes.count <= 4_000_000,
        let stored = try? JSONDecoder().decode(LocalLearningData.self, from: bytes),
        stored.version == 1, stored.words.count <= 5000,
        stored.words.allSatisfy({
          LearningEvent.valid($0) && $0.uses > 0 && $0.uses <= 1_000_000_000
        }),
        Set(stored.words.map(\.id)).count == stored.words.count
      else { throw AccountError.storage }
      guard let object = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
        Set(object.keys) == ["version", "words"],
        let entries = object["words"] as? [[String: Any]],
        entries.allSatisfy({ Set($0.keys) == ["english", "chinese", "pos", "uses", "mastered"] })
      else { throw AccountError.storage }
      data = stored
    }
    let result = try body(&data)
    if write {
      let bytes = try JSONEncoder().encode(data)
      guard bytes.count <= 4_000_000 else { throw AccountError.storage }
      try bytes.write(to: file, options: .atomic)
      try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }
    return result
  }
  func snapshot() throws -> [LearnedWord] {
    try transaction(false) {
      $0.words.sorted { $0.english.localizedStandardCompare($1.english) == .orderedAscending }
    }
  }
  func record(_ word: LearnedWord) throws {
    guard LearningEvent.valid(word) else { throw AccountError.invalidWord }
    try transaction(true) { data in
      if let index = data.words.firstIndex(where: { $0.id == word.id }) {
        data.words[index].uses = min(1_000_000_000, data.words[index].uses + 1)
      } else {
        guard data.words.count < 5000 else { throw AccountError.queueFull }
        data.words.append(
          .init(
            english: word.english, chinese: word.chinese, pos: word.pos, uses: 1, mastered: false))
      }
    }
  }
  func setMastered(_ id: String, _ value: Bool) throws {
    try transaction(true) { data in
      guard let index = data.words.firstIndex(where: { $0.id == id }) else {
        throw AccountError.invalidWord
      }
      data.words[index].mastered = value
    }
  }
  func delete(_ id: String) throws {
    try transaction(true) { $0.words.removeAll { $0.id == id } }
  }
  static func csv(_ words: [LearnedWord]) -> Data {
    func cell(_ value: String) -> String {
      let safe =
        "=+-@".contains(value.trimmingCharacters(in: .whitespaces).first ?? " ")
        ? "'" + value : value
      return "\"" + safe.replacingOccurrences(of: "\"", with: "\"\"") + "\""
    }
    let rows = words.map { word in
      [word.english, word.chinese, word.pos, String(word.uses), word.mastered ? "true" : "false"]
        .map(cell).joined(separator: ",")
    }
    return Data(
      ("\u{FEFF}English,Chinese,POS,Uses,Mastered\r\n" + rows.joined(separator: "\r\n") + "\r\n")
        .utf8)
  }
}
