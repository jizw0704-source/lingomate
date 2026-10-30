import AppKit

enum CandidateFeedbackTests {
  static func run() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("words.json")
    let engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
    defer { engine.terminate() }
    let store = PersonalVocabularyStore(resources: Bundle.main.resourceURL!, file: file)
    var state = SessionState()
    state.input = "gongshi"
    state.frame = try engine.request(EngineRequest(action: "query", input: state.input))
    let target = state.frame!.candidates.first { $0.text == "公示" }!
    let request = CandidateFeedback(
      input: state.input, identity: state.frame?.engineIdentity, candidate: target, value: 1)
    precondition(request.matches(state))
    var changed = state
    changed.input = "shi"
    precondition(!request.matches(changed))
    changed = state
    changed.frame?.engineIdentity = UUID()
    precondition(!request.matches(changed))
    var selected: CandidateFeedback?
    let menu = CandidateFeedback.menu(state) { selected = $0 }
    let choices = menu.submenu!.items.first!.submenu!
    let item = choices.items[0]
    _ = item.target?.perform(item.action!, with: item)
    precondition(selected?.value == 1 && selected?.matches(state) == true)
    try store.feedback(target, value: 1)
    precondition(!FileManager.default.fileExists(atPath: file.path))
    let ranked = try engine.request(EngineRequest(action: "query", input: state.input))
    precondition(ranked.candidates.first?.text == target.text)
    let row = try store.snapshot().first!
    precondition(row.count == 0 && row.prioritized && row.feedback == 1)
    _ = try store.prioritize(row, enabled: false)
    let reset = try store.snapshot()
    precondition(reset.isEmpty)
    var symbol = target
    symbol.symbol = true
    changed = state
    let fake = EngineFrame(input: "jia", marked: "jia", candidates: [symbol], committed: nil)
    changed.input = "jia"
    changed.frame = fake
    precondition(CandidateFeedback.menu(changed, action: { _ in }).isEnabled == false)
    print("PASS 候选反馈：菜单身份/引擎会话边界、符号排除、未确认词零次数、即时优先及管理恢复；仅临时词库")
  }
}
