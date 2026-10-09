// 状态和随包真实引擎检查，不能代替宿主应用的系统输入验证。
import Foundation

enum SelfTests {
  static func run() {
    var state = SessionState()
    state.input = "xuexi"
    state.expanded = true
    state.resetSelection()
    precondition(state.input == "xuexi" && !state.expanded)
    let engine = try! EngineClient(resources: Bundle.main.resourceURL!)
    checkMixedInput(engine)
    let frame = try! engine.request(EngineRequest(action: "query", input: "xuexi"))
    precondition(frame.candidates.first?.text == "学习")
    precondition(frame.candidates.first!.translations.count >= 3)
    state.frame = frame
    state.move(-1)
    precondition(state.active == 4)
    state.move(1)
    precondition(state.active == 0)
    state.expanded = true
    state.moveSense(2)
    precondition(state.sense == 2)
    let candidate = frame.candidates[0]
    let selected = try! engine.request(
      EngineRequest(
        action: "commit", input: "xuexi", candidate: candidate.text, syllables: candidate.syllables,
        english: "learning"))
    precondition(selected.committed == "learning" && selected.input.isEmpty)
    let partial = try! engine.request(EngineRequest(action: "query", input: "kaifazhe"))
    let develop = partial.candidates.first(where: { $0.text == "开发" })!
    let remaining = try! engine.request(
      EngineRequest(
        action: "commit", input: "kaifazhe", candidate: develop.text, syllables: develop.syllables,
        english: "develop"))
    precondition(remaining.input == "zhe")
    state.clear()
    precondition(state.input.isEmpty && state.frame == nil)
    checkPaging()
    let many = try! engine.request(EngineRequest(action: "query", input: "shi"))
    precondition(many.candidates.count > 9)
    state.input = "shi"
    state.frame = many
    state.changePage(1)
    let index = state.indexOnPage(0)!
    precondition(index == 5)
    let later = many.candidates[index]
    let laterCommit = try! engine.request(
      EngineRequest(
        action: "commit", input: "shi", candidate: later.text, syllables: later.syllables))
    precondition(laterCommit.committed == later.text && laterCommit.input.isEmpty)
    print("PASS 原生会话状态、候选分页边界、当前页数字映射、后页提交、第三译法及部分提交")
    engine.terminate()
  }

  private static func checkMixedInput(_ engine: EngineClient) {
    var state = SessionState()
    for letters in ["AI", "GPU", "Swift", "ChatGPT", "Xi'An"] {
      precondition(state.appendLetters(letters))
      state.frame = try! engine.request(EngineRequest(action: "query", input: state.input))
      precondition(state.input == letters.lowercased() && state.markedInput == letters)
      state.expanded = true
      state.sense = 2
      precondition(state.takeRawInput() == letters)
      precondition(state.input.isEmpty && state.rawInput.isEmpty && !state.expanded)
      precondition(state.frame == nil && state.sentence == .none)
      precondition(state.takeRawInput().isEmpty)
    }
    precondition(state.appendLetters("SwIFt"))
    state.removeLastLetter()
    precondition(state.rawInput == "SwIF" && state.input == "swif")
    precondition(state.appendLetters("T"))
    precondition(state.takeRawInput() == "SwIFT")
    precondition(!state.appendLetters("学") && !state.appendLetters("1") && !state.appendLetters(""))
    precondition(state.input.isEmpty && state.rawInput.isEmpty)
    precondition(state.appendLetters(String(repeating: "A", count: 240)))
    precondition(!state.appendLetters("B") && state.rawInput.count == 240)
    state.clear()
    precondition(state.appendLetters("xuexi"))
    state.frame = try! engine.request(EngineRequest(action: "query", input: state.input))
    precondition(state.markedInput == state.frame!.marked)
    state.clear()
    for english in [false, true] {
      precondition(state.appendLetters("KaifaZhe"))
      let frame = try! engine.request(EngineRequest(action: "query", input: state.input))
      let develop = frame.candidates.first(where: { $0.text == "开发" })!
      let next = try! engine.request(
        EngineRequest(
          action: "commit", input: state.input, candidate: develop.text,
          syllables: develop.syllables, english: english ? "develop" : nil))
      precondition(next.committed == (english ? "develop" : "开发") && next.input == "zhe")
      let remaining = state.rawRemainder(for: next.input)
      state.clear()
      state.replaceInput(next.input, original: remaining)
      precondition(state.input == "zhe" && state.takeRawInput() == "Zhe")
    }
    precondition(state.appendLetters("NIHAO"))
    let frame = try! engine.request(EngineRequest(action: "query", input: state.input))
    let hello = frame.candidates.first(where: { $0.text == "你好" })!
    let committed = try! engine.request(
      EngineRequest(
        action: "commit", input: state.input, candidate: hello.text, syllables: hello.syllables))
    precondition(committed.committed == "你好" && committed.input.isEmpty)
    let remaining = state.rawRemainder(for: committed.input)
    state.clear()
    state.replaceInput(committed.input, original: remaining)
    precondition(state.appendLetters("GPU") && state.takeRawInput() == "GPU")
    var punctuation = PunctuationState()
    punctuation.observeLiteral("GPU")
    punctuation.clearLiteral()
    precondition(!punctuation.isLatinLiteral && punctuation.language(mode: .automatic) == .english)
    precondition(state.appendLetters("xuexi") && state.input == "xuexi")
    state.clear()
    state.replaceInput("shi", original: "outdated")
    precondition(state.takeRawInput() == "shi")
    print("PASS 原样大小写、展开后原样提交、退格、清空与长度边界、真实中英文部分提交剩余大小写")
  }

  private static func checkPaging() {
    for count in [0, 1, 5, 6, 9, 12, 64] {
      var state = SessionState()
      state.input = "shi"
      state.frame = EngineFrame(
        input: "shi", marked: "shi",
        candidates: (0..<count).map {
          EngineCandidate(text: "候选\($0)", syllables: ["shi"], translations: [], hasDetails: false)
        }, committed: nil)
      precondition(state.pageCount == (count + 4) / 5)
      precondition(!state.changePage(-1))
      guard count > 0 else {
        precondition(state.visibleIndices.isEmpty && state.indexOnPage(0) == nil)
        precondition(!state.changePage(1))
        continue
      }
      for page in 0..<state.pageCount {
        precondition(state.page == page && state.active == page * 5)
        for slot in 0..<5 {
          let expected = page * 5 + slot
          precondition(state.indexOnPage(slot) == (expected < count ? expected : nil))
        }
        precondition(state.indexOnPage(-1) == nil && state.indexOnPage(5) == nil)
        state.move(-1)
        precondition(state.active == state.visibleIndices.upperBound - 1)
        state.move(1)
        precondition(state.active == page * 5)
        state.sentence = .ready(source: "候选", english: "candidate")
        state.expanded = true
        state.sense = 2
        if page + 1 < state.pageCount {
          precondition(state.changePage(1))
          precondition(state.sentence == .none && !state.expanded && state.sense == 0)
        } else {
          precondition(!state.changePage(1))
          precondition(state.sentence != .none && state.expanded && state.sense == 2)
        }
      }
      state.resetSelection()
      precondition(state.page == 0 && state.input == "shi")
    }
  }
}
