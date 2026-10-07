// 异步过期结果检查与真实系统模型联调；不读宿主应用或用户输入。
import AppKit

enum SentenceTests {
  @MainActor static func stateChecks() async {
    var events: [SentenceTranslation] = []
    let translator = SentenceTranslator(delay: 0) { text in
      // 故意模拟不响应取消的旧任务，检查晚到结果不能覆盖新句子。
      try? await Task.sleep(nanoseconds: text == "旧句" ? 80_000_000 : 10_000_000)
      return text == "旧句" ? "Old sentence." : "New sentence."
    }
    translator.request(key: "old", source: "旧句") { events.append($0) }
    try? await Task.sleep(nanoseconds: 5_000_000)
    translator.request(key: "new", source: "新句") { events.append($0) }
    try? await Task.sleep(nanoseconds: 120_000_000)
    let ready = events.filter {
      if case .ready = $0 { return true }
      return false
    }
    precondition(ready == [.ready(source: "新句", english: "New sentence.")])
    events.removeAll()
    translator.request(key: "cancel", source: "新句") { events.append($0) }
    translator.cancel()
    try? await Task.sleep(nanoseconds: 40_000_000)
    precondition(events == [.loading])
    var state = SessionState()
    state.input = "wojintianxiangxuexiyingyu"
    state.sentence = .ready(source: "我今天想学习英语", english: "I want to learn English today.")
    state.frame = EngineFrame(
      input: state.input, marked: state.input,
      candidates: [
        EngineCandidate(
          text: "我今天想学习英语", syllables: [], translations: [], hasDetails: false)
      ], committed: nil)
    state.move(1)
    precondition(
      state.sentence == .ready(source: "我今天想学习英语", english: "I want to learn English today."))
    state.resetSelection()
    precondition(state.sentence == .none && !state.input.isEmpty)
    state.clear()
    precondition(state.input.isEmpty && state.sentence == .none)
    print("PASS 过期译文、任务取消和组合输入清除")
  }

  @MainActor static func integrationChecks() async throws {
    let engine = try EngineClient(resources: Bundle.main.resourceURL!)
    defer { engine.terminate() }
    for pinyin in [
      "woxiangxuexiyingyu", "wojintianxiangxuexiyingyu",
      "woxiangxuexiyingyuyinweita keyibangzhuwohegengduodepengyoujiaoliubingqieliaojiebutongdewenhua"
        .replacingOccurrences(of: " ", with: ""),
    ] {
      let frame = try engine.request(EngineRequest(action: "query", input: pinyin))
      guard let candidate = frame.candidates.first, candidate.translations.isEmpty else {
        throw EngineFailure.message("测试整句候选未生成。")
      }
      let english = try await LocalSentenceTranslation.translate(candidate.text)
      precondition(english.range(of: "[A-Za-z]", options: .regularExpression) != nil)
      let consumed = try engine.request(
        EngineRequest(
          action: "commit", input: pinyin,
          candidate: candidate.text, syllables: candidate.syllables))
      precondition(consumed.committed == candidate.text && consumed.input.isEmpty)
      print("PASS \(candidate.text) → \(english)")
    }
  }
}
