import Foundation

enum ConfirmedContextTests {
  static func run() throws {
    let identity = UUID()
    var context = ConfirmedContext()
    precondition(context.preceding(identity: identity, now: 0) == nil)
    context.confirmed("临床", chinese: true, identity: identity, now: 1)
    precondition(context.preceding(identity: identity, now: 2) == "临床")
    context.confirmed("试验", chinese: true, identity: identity, now: 3)
    precondition(context.preceding(identity: identity, now: 4) == "临床试验")
    context.confirmed("AI", chinese: false, identity: identity, now: 5)
    precondition(context.preceding(identity: identity, now: 6) == nil)
    for output in ["+", "，", "\n", " ", "😊", "xuexi"] {
      context.confirmed("临床", chinese: true, identity: identity, now: 10)
      context.confirmed(output, chinese: true, identity: identity, now: 11)
      precondition(context.preceding(identity: identity, now: 12) == nil)
    }
    context.confirmed(String(repeating: "中", count: 64), chinese: true, identity: identity, now: 20)
    precondition(context.preceding(identity: identity, now: 21)?.count == 32)
    precondition(context.preceding(identity: identity, now: 81) == nil)
    context.confirmed("临床", chinese: true, identity: identity, now: 100)
    precondition(context.preceding(identity: UUID(), now: 101) == nil)
    context.confirmed("临床", chinese: true, identity: identity, now: 100)
    context.reset()
    precondition(context.preceding(identity: identity, now: 101) == nil)
    // Real engine consumption and original translation validation, only temporary memory.
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let engine = try EngineClient(
      resources: Bundle.main.resourceURL!, memoryURL: directory.appendingPathComponent("words.json")
    )
    defer { engine.terminate() }
    let baseline = try engine.request(EngineRequest(action: "query", input: "shiyan"))
    let ranked = try engine.request(
      EngineRequest(action: "query", input: "shiyan", context: "context-test", preceding: "临床"))
    precondition(baseline.candidates.first?.text == "实验")
    precondition(ranked.candidates.first?.text == "试验")
    let candidate = ranked.candidates[0]
    let committed = try engine.request(
      EngineRequest(
        action: "commit", input: "shiyan", candidate: candidate.text,
        syllables: candidate.syllables, context: "context-test"))
    precondition(committed.committed == "试验" && committed.input.isEmpty)
    precondition(
      !FileManager.default.fileExists(atPath: directory.appendingPathComponent("words.json").path))
    print("PASS 会话上下文：仅确认中文、32字/60秒边界、英文/符号/换行/重启/重置清空；真实同音排序及消费，不新增记忆")
  }
}
