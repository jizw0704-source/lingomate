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
