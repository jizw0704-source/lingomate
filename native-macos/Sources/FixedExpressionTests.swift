// Native JSON decoding, real bundled bridge and temporary confirmed vocabulary.
import Foundation

enum FixedExpressionTests {
  static func run() {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("words.json")
    do {
      var engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      defer { engine.terminate() }
      var state = SessionState()
      precondition(state.appendLetters("chiputao"))
      state.frame = try engine.request(
        EngineRequest(action: "query", input: state.input, context: "fixed-expression-test"))
      let index = state.frame!.candidates.firstIndex { $0.text == "吃葡萄不吐葡萄皮" }!
      precondition(index >= 2 && state.frame!.candidates.count <= 64)
      state.active = index
      let candidate = state.candidate!
      precondition(candidate.syllables.count == 8)
      let committed = try engine.request(
        EngineRequest(
          action: "commit", input: state.input, candidate: candidate.text,
          syllables: candidate.syllables, context: "fixed-expression-test"))
      precondition(committed.committed == candidate.text && committed.input.isEmpty)
      precondition(state.rawRemainder(for: committed.input).isEmpty)
      precondition(!FileManager.default.fileExists(atPath: file.path))
      engine.confirmSelection(committed, context: "fixed-expression-test", chinese: true)
      precondition(FileManager.default.fileExists(atPath: file.path))
      engine.terminate()
      engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      let remembered = try engine.request(
        EngineRequest(action: "query", input: "chiputao", context: "fixed-expression-test"))
      precondition(
        remembered.candidates.contains { $0.text == candidate.text && $0.personal == true })
      state.clear()
      precondition(state.appendLetters("AI") && state.takeRawInput() == "AI")
      print("PASS 原生固定表达：前缀整句候选、绝对索引、整句提交和拼音消耗、输出确认后记忆及隔离引擎重启召回")
    } catch {
      fputs("固定表达检查失败。\n", stderr)
      exit(1)
    }
  }
}
