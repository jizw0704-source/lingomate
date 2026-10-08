import Darwin
// 按账号隔离的单词缓存和待同步操作；从不保存正文、拼音或整句译文。
import Foundation

struct LearningAccount: Codable, Equatable {
  let id: String
  let email: String
  let project: String
  var sessionID = UUID()
  var scope: String { project + "|" + id }
}

struct LearnedWord: Codable, Identifiable {
  let english: String
  let chinese: String
  let pos: String
  var uses: Int
  var mastered: Bool
  var id: String { english.lowercased() + "|" + chinese + "|" + pos }
}

struct LearningEvent: Codable, Identifiable {
  let id: UUID
  let english: String
  let chinese: String
  let pos: String
  let kind: String
  let mastered: Bool
  var word: LearnedWord {
    .init(english: english, chinese: chinese, pos: pos, uses: 0, mastered: false)
  }

  static func valid(_ word: LearnedWord) -> Bool {
    guard (1...80).contains(word.english.count), (1...16).contains(word.chinese.count),
      word.pos.count <= 24, word.english.utf8.allSatisfy({ $0 >= 32 && $0 < 127 }),
      word.chinese.unicodeScalars.allSatisfy({ (0x3400...0x9FFF).contains($0.value) })
    else { return false }
    return [word.english, word.chinese, word.pos].allSatisfy {
      !$0.contains("|") && $0.rangeOfCharacter(from: .controlCharacters) == nil
    }
  }
}

struct LearningData: Codable {
  var version = 1
  var active: LearningAccount?
  var words: [String: [LearnedWord]] = [:]
  var pending: [String: [LearningEvent]] = [:]

  func visibleWords(_ account: LearningAccount) -> [LearnedWord] {
    var entries = Dictionary(
      (words[account.scope] ?? []).map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
    for event in pending[account.scope] ?? [] {
      var word = entries[event.word.id] ?? event.word
      if event.kind == "study" { word.uses += 1 } else { word.mastered = event.mastered }
      entries[word.id] = word
    }
    return entries.values.sorted {
      $0.english.localizedStandardCompare($1.english) == .orderedAscending
    }
  }
}

final class LearningStore {
  let directory: URL
  private var file: URL { directory.appendingPathComponent("learning.json") }
  init(directory: URL) { self.directory = directory }

  func transaction<T>(_ operation: (inout LearningData) throws -> T, write: Bool = true) throws -> T
  {
    try FileManager.default.createDirectory(
      at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    let descriptor = open(
      directory.appendingPathComponent("learning.lock").path, O_CREAT | O_RDWR, 0o600)
    guard descriptor >= 0 else { throw AccountError.storage }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX) == 0 else { throw AccountError.storage }
    defer { flock(descriptor, LOCK_UN) }
    var data = LearningData()
    if FileManager.default.fileExists(atPath: file.path) {
      guard let bytes = try? Data(contentsOf: file), bytes.count <= 8_000_000,
        let existing = try? JSONDecoder().decode(LearningData.self, from: bytes),
        existing.version == 1
      else { throw AccountError.storage }
      data = existing
    }
    let result = try operation(&data)
    if write {
      try JSONEncoder().encode(data).write(to: file, options: .atomic)
      try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }
    return result
  }

  func snapshot() throws -> LearningData { try transaction({ $0 }, write: false) }

  func append(_ event: LearningEvent, for account: LearningAccount) throws {
    guard LearningEvent.valid(event.word), ["study", "mastered"].contains(event.kind) else {
      throw AccountError.invalidWord
    }
    try transaction { data in
      guard data.active == account else { throw AccountError.signedOut }
      let pending = data.pending[account.scope] ?? []
      guard pending.count < 5000 else { throw AccountError.queueFull }
      data.pending[account.scope, default: []].append(event)
    }
  }

  func accept(_ ids: Set<UUID>, for account: LearningAccount, words: [LearnedWord]) throws {
    try transaction { data in
      guard data.active == account else { throw AccountError.signedOut }
      data.pending[account.scope] = (data.pending[account.scope] ?? []).filter {
        !ids.contains($0.id)
      }
      data.words[account.scope] = words
    }
  }
}

enum LearningRuntime {
  static var store: LearningStore?
  static var warning: String?
  private static let queue = DispatchQueue(label: "org.local.bilingualcompanion.learning")
  static func account() -> LearningAccount? { try? store?.snapshot().active }
  static func confirmed(_ detail: TranslationDetail, chinese: String, account: LearningAccount?) {
    guard let account, let store else { return }
    let event = LearningEvent(
      id: UUID(), english: detail.word, chinese: chinese, pos: detail.pos, kind: "study",
      mastered: false)
    guard LearningEvent.valid(event.word) else { return }
    queue.async {
      do {
        try store.append(event, for: account)
        AccountWindow.launch(syncOnly: true)
      } catch {
        DispatchQueue.main.async { warning = "文字已输出，学习记录暂未保存。请打开学习页检查。" }
      }
    }
  }
}
