import Foundation

enum SymbolCandidateTests {
  static func run() {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("words.json")
    let suite = "lingomate-symbol-test-" + UUID().uuidString
    let preferences = UserDefaults(suiteName: suite)!
    defer { preferences.removePersistentDomain(forName: suite) }
    do {
      let settings = CandidatePreferences(store: preferences)
      precondition(settings.showSymbols)
      settings.selectSymbols(false)
      precondition(!CandidatePreferences(store: preferences).showSymbols)
      settings.selectSymbols(true)
      let engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      defer { engine.terminate() }
      for input in SymbolCandidates.entries.keys.sorted() {
        let original = try engine.request(EngineRequest(action: "query", input: input))
        let augmented = SymbolCandidates.augment(original, enabled: true)
        precondition(augmented.candidates.count <= 64)
        precondition(
          augmented.candidates.prefix(min(2, original.candidates.count)).map(\.text)
            == original.candidates.prefix(2).map(\.text))
        precondition(
          augmented.candidates.filter { $0.symbol != true }.map(\.text)
            == Array(original.candidates.prefix(64 - SymbolCandidates.outputs(for: input).count))
            .map(\.text))
        for symbol in augmented.candidates.filter({ $0.symbol == true }) {
          precondition(SymbolCandidates.output(symbol, input: input, sense: nil) == symbol.text)
          precondition(SymbolCandidates.output(symbol, input: input + "a", sense: nil) == nil)
          precondition(SymbolCandidates.output(symbol, input: input, sense: 0) == nil)
          precondition(symbol.translations.isEmpty && symbol.syllables.isEmpty)
        }
        precondition(
          SymbolCandidates.augment(original, enabled: false).candidates.map(\.text)
            == original.candidates.map(\.text))
        _ = try engine.request(EngineRequest(action: "cancel", input: "", context: "symbols"))
      }
      precondition(SymbolCandidates.outputs(for: "Alpha") == ["α", "Α"])
      precondition(SymbolCandidates.outputs(for: "yu'mao'qiu") == ["🏸"])
      for input in ["yumao", "yumaoqi", "ymq", "alph", "alphafoo", "http", "a@b", ""] {
        precondition(SymbolCandidates.outputs(for: input).isEmpty)
      }
      var state = SessionState()
      precondition(state.appendLetters("Alpha"))
      let frame = try engine.request(EngineRequest(action: "query", input: state.input))
      state.frame = SymbolCandidates.augment(frame, enabled: true)
      state.active = state.frame!.candidates.firstIndex { $0.symbol == true }!
      precondition(state.takeRawInput() == "Alpha" && state.frame == nil)
      let normal = try engine.request(EngineRequest(action: "query", input: "xuexi"))
      precondition(normal.candidates.first?.text == "学习")
      precondition(!FileManager.default.fileExists(atPath: file.path))
      let chosen = normal.candidates[0]
      let receipt = try engine.request(
        EngineRequest(
          action: "commit", input: "xuexi", candidate: chosen.text,
          syllables: chosen.syllables, context: "symbols"))
      engine.confirmSelection(receipt, context: "symbols", chinese: true)
      let before = try Data(contentsOf: file)
      for input in ["zan", "yumaoqiu", "shengluehao", "alpha"] {
        let frame = try engine.request(
          EngineRequest(action: "query", input: input, context: "symbols"))
        let enriched = SymbolCandidates.augment(frame, enabled: true)
        precondition(enriched.candidates.contains { $0.symbol == true })
        engine.cancelLearning(context: "symbols")
        let after = try Data(contentsOf: file)
        precondition(after == before)
      }
      print("PASS 表情符号：完整关键词、前两项中文优先、64项与页码、大小写/分音、开关持久化、拒绝过期/译词、原样回车及真实临时词库不变")
    } catch {
      fputs("表情与符号候选检查失败：\(error)\n", stderr)
      exit(1)
    }
  }
}
