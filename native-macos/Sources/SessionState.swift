// 每个应用输入会话独立保留规范拼音和原始大小写；原样提交不产生选词记录。
import Foundation

struct SessionState {
  static let pageSize = 5
  var input = ""
  private var originalInput = ""
  var active = 0
  var expanded = false
  var sense = 0
  var frame: EngineFrame?
  var sentence: SentenceTranslation = .none

  var rawInput: String { originalInput.isEmpty ? input : originalInput }
  var markedInput: String {
    if rawInput != input { return rawInput }
    return frame?.marked.isEmpty == false ? frame!.marked : input
  }

  @discardableResult mutating func appendLetters(_ characters: String) -> Bool {
    guard !characters.isEmpty,
      characters.utf8.allSatisfy({
        (65...90).contains($0) || (97...122).contains($0) || $0 == 39
      }), input.count + characters.count <= 240
    else { return false }
    originalInput = rawInput + characters
    input += characters.lowercased()
    resetSelection()
    return true
  }

  mutating func removeLastLetter() {
    guard !input.isEmpty else { return }
    originalInput = String(rawInput.dropLast())
    input.removeLast()
    resetSelection()
  }

  func rawRemainder(for remainder: String) -> String {
    guard !remainder.isEmpty, input.hasSuffix(remainder) else { return remainder }
    return String(rawInput.suffix(remainder.count))
  }

  mutating func replaceInput(_ normalized: String, original: String) {
    input = normalized
    originalInput = original.lowercased() == normalized ? original : normalized
  }

  mutating func takeRawInput() -> String {
    let result = rawInput
    clear()
    return result
  }

  var page: Int { active / Self.pageSize }
  var pageCount: Int { ((frame?.candidates.count ?? 0) + Self.pageSize - 1) / Self.pageSize }
  var visibleIndices: Range<Int> {
    let count = frame?.candidates.count ?? 0
    let start = min(page * Self.pageSize, count)
    return start..<min(start + Self.pageSize, count)
  }

  func indexOnPage(_ slot: Int) -> Int? {
    guard (0..<Self.pageSize).contains(slot) else { return nil }
    let index = visibleIndices.lowerBound + slot
    return visibleIndices.contains(index) ? index : nil
  }

  @discardableResult mutating func changePage(_ delta: Int) -> Bool {
    guard pageCount > 0 else { return false }
    let next = min(max(page + delta, 0), pageCount - 1)
    guard next != page else { return false }
    active = next * Self.pageSize
    expanded = false
    sense = 0
    sentence = .none
    return true
  }

  mutating func resetSelection() {
    active = 0
    expanded = false
    sense = 0
    sentence = .none
  }

  mutating func clear() {
    input = ""
    originalInput = ""
    frame = nil
    resetSelection()
  }

  var candidate: EngineCandidate? {
    guard let candidates = frame?.candidates, candidates.indices.contains(active) else {
      return nil
    }
    return candidates[active]
  }

  mutating func move(_ delta: Int) {
    let indices = visibleIndices
    let count = indices.count
    guard count > 0 else { return }
    let next = indices.lowerBound + (active - indices.lowerBound + delta + count) % count
    guard next != active else { return }
    active = next
    expanded = false
    sense = 0
    sentence = .none
  }

  mutating func moveSense(_ delta: Int) {
    let count = candidate?.translations.count ?? 0
    guard count > 0 else { return }
    sense = (sense + delta + count) % count
  }
}
