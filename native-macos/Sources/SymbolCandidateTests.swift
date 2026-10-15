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
      precondition(MathSymbols.entries.count == 171)
      precondition(Set(MathSymbols.entries.map(\.category)).count == 14)
      let mathAliases = MathSymbols.entries.flatMap(\.aliases)
      precondition(mathAliases.count == 337 && Set(mathAliases).count == mathAliases.count)
      for entry in MathSymbols.entries {
        for alias in entry.aliases {
          precondition(SymbolCandidates.outputs(for: alias) == entry.outputs)
        }
      }
      let requiredMath = [
        "jia": "+", "jian": "−", "cheng": "×", "chu": "÷", "dengyu": "=",
        "dayu": ">", "xiaoyu": "<", "genhao": "√", "jifen": "∫", "qiuhe": "∑",
        "shuyu": "∈", "luojiyu": "∧", "shangbiaoling": "⁰", "xiabiaojiu": "₉",
      ]
      for (alias, output) in requiredMath {
        precondition(SymbolCandidates.outputs(for: alias).contains(output))
        precondition(SymbolCandidates.outputs(for: alias.uppercased()).contains(output))
      }
      let invalidCatalogs = [
        "{}",
        "{\"version\":2,\"license\":\"GPL-3.0-or-later\",\"entries\":[]}",
      ]
      for invalid in invalidCatalogs {
        do {
          _ = try MathSymbols.decode(Data(invalid.utf8))
          preconditionFailure("Malformed math catalog accepted")
        } catch {}
      }
      let engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      defer { engine.terminate() }
      for input in SymbolCandidates.entries.keys.sorted() {
        let original = try engine.request(EngineRequest(action: "query", input: input))
        let augmented = SymbolCandidates.augment(original, enabled: true)
        precondition(augmented.candidates.count <= 64)
        if mathAliases.contains(input) {
          var page = SessionState()
          precondition(page.appendLetters(input))
          page.frame = augmented
          precondition(page.pageCount == (augmented.candidates.count + 4) / 5)
          for (index, candidate) in augmented.candidates.enumerated() where candidate.symbol == true
          {
            precondition(index >= min(2, original.candidates.count) && index < 5)
            page.active = index
            precondition(page.indexOnPage(index % 5) == index)
            precondition(
              SymbolCandidates.output(page.candidate!, input: input, sense: nil) == candidate.text)
          }
          precondition(page.takeRawInput() == input && page.frame == nil)
        }
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
      let punctuation: [String: [String]] = [
        "gantanhao": ["！", "!"], "jingtanhao": ["！", "!"], "wenhao": ["？", "?"],
        "douhao": ["，", ","], "maohao": ["：", ":"], "fenhao": ["；", ";"],
        "juhao": ["。", "."], "dunhao": ["、"], "pozhehao": ["——", "—", "--"],
      ]
      for (input, expected) in punctuation {
        precondition(SymbolCandidates.outputs(for: input) == expected)
        let original = try engine.request(EngineRequest(action: "query", input: input))
        var page = SessionState()
        page.input = input
        page.frame = SymbolCandidates.augment(original, enabled: true)
        precondition(page.pageCount == (page.frame!.candidates.count + 4) / 5)
        for output in expected {
          let index = page.frame!.candidates.firstIndex { $0.text == output && $0.symbol == true }!
          precondition(index >= min(2, original.candidates.count) && index < 5)
          page.active = index
          precondition(page.indexOnPage(index % 5) == index)
          precondition(SymbolCandidates.output(page.candidate!, input: input, sense: nil) == output)
        }
      }
      for input in ["gantan", "gantanha", "gth", "wenha", "wh", "douha", "dh"] {
        precondition(SymbolCandidates.outputs(for: input).isEmpty)
      }
      precondition(SymbolCandidates.outputs(for: "gan'tan'hao") == ["！", "!"])
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
      for input in ["zan", "yumaoqiu", "shengluehao", "alpha"] + punctuation.keys.sorted()
        + mathAliases
      {
        let frame = try engine.request(
          EngineRequest(action: "query", input: input, context: "symbols"))
        let enriched = SymbolCandidates.augment(frame, enabled: true)
        precondition(enriched.candidates.contains { $0.symbol == true })
        engine.cancelLearning(context: "symbols")
        let after = try Data(contentsOf: file)
        precondition(after == before)
      }
      print("PASS 数学覆盖：14类171项337别称，逐项普通候选优先、首屏数字/分页、英文原样确认与临时词库不变；畸形资源拒绝")
      print("PASS 表情符号：完整关键词、前两项中文优先、64项与页码、大小写/分音、开关持久化、拒绝过期/译词、原样回车及真实临时词库不变")
    } catch {
      fputs("表情与符号候选检查失败：\(error)\n", stderr)
      exit(1)
    }
  }
}
