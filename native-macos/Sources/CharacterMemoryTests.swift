// 连续独立拼音提交：真实桥接与临时词库，不连接IMK宿主或个人配置。
import Foundation

enum CharacterMemoryTests {
  static func run() {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      "lingomate-character-memory-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("words.json")
    let resources = Bundle.main.resourceURL!
    do {
      var engine = try EngineClient(resources: resources, memoryURL: file)
      defer { engine.terminate() }
      func choose(_ input: String, _ text: String, context: String = "same-editor") throws {
        let frame = try engine.request(
          EngineRequest(action: "query", input: input, context: context))
        let candidate = frame.candidates.first { $0.text == text }!
        let committed = try engine.request(
          EngineRequest(
            action: "commit", input: input, candidate: text,
            syllables: candidate.syllables, context: context))
        precondition(committed.committed == text && committed.input.isEmpty)
        engine.confirmSelection(committed, context: context, chinese: true)
        precondition(Runtime.memoryWarning == nil)
      }
      try choose("gai", "钙")
      try choose("tai", "钛")
      try choose("kuang", "矿")
      engine.terminate()
      engine = try EngineClient(resources: resources, memoryURL: file)
      let learned = try engine.request(EngineRequest(action: "query", input: "gaitaikuang"))
      precondition(learned.candidates.first?.text == "钙钛矿")
      precondition(learned.candidates.first?.personal == true)
      _ = try engine.request(EngineRequest(action: "cancel", input: "", context: "same-editor"))
      try choose("gai", "钙")
      try choose("tai", "钛")
      try choose("nie", "镍")
      engine.terminate()
      engine = try EngineClient(resources: resources, memoryURL: file)
      let unknown = try engine.request(EngineRequest(action: "query", input: "gaitainie"))
      precondition(unknown.candidates.first?.text == "钙钛镍")
      precondition(unknown.candidates.first?.personal == true)
      // Native raw/English/punctuation/focus boundaries use this existing cancel channel.
      _ = try engine.request(EngineRequest(action: "cancel", input: "", context: "same-editor"))
      try choose("nie", "镍")
      _ = try engine.request(EngineRequest(action: "cancel", input: "", context: "same-editor"))
      try choose("gai", "钙")
      let isolated = try engine.request(EngineRequest(action: "query", input: "niegai"))
      precondition(!isolated.candidates.contains { $0.text == "镍钙" && $0.personal == true })
      print("PASS 逐字造词：独立拼音钙→钛→矿、未知钙钛镍、重启后优先候选及会话取消；无真实宿主/词库访问")
    } catch {
      fputs("FAIL 逐字造词隔离检查\n", stderr)
      exit(1)
    }
  }
}
