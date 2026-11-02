// 宿主编辑范围模型与真实桥接隔离测试；不连接真实输入会话或读取个人词库。
import AppKit

enum WordEditMemoryTests {
  static func run() {
    _ = NSApplication.shared
    AppearanceSettings.configure(isolated: true, initial: .light)
    rangeChecks()
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      "lingomate-word-edit-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("words.json")
    do {
      var engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      defer { engine.terminate() }
      var state = WordEditState()
      var receipt: EngineFrame?
      func choose(_ input: String, _ text: String, start: Int) throws {
        let frame = try engine.request(
          EngineRequest(action: "query", input: input, context: "test-editor"))
        let candidate = frame.candidates.first { $0.text == text }!
        let committed = try engine.request(
          EngineRequest(
            action: "commit", input: input, candidate: text,
            syllables: candidate.syllables, context: "test-editor"))
        receipt = engine.confirmSelection(
          committed, context: "test-editor", chinese: true, trackEdits: true)
        precondition(receipt?.editToken != nil)
        precondition(
          state.record(
            at: start, count: text.count,
            selectionAfter: NSRange(location: start + text.count, length: 0)))
      }
      func backspace(at position: Int) throws {
        precondition(state.prepareBackspace(selection: NSRange(location: position, length: 0)))
        guard
          case .deleted(let deletion) = state.reconcile(
            selection: NSRange(location: position - 1, length: 0))
        else { preconditionFailure("删除范围未确认") }
        let previous = receipt!
        receipt = try engine.request(
          EngineRequest(
            action: "edit-memory", input: "", context: "test-editor",
            learningToken: previous.editToken, editStart: deletion.offset,
            editLength: deletion.length), expectedIdentity: previous.engineIdentity)
      }
      try choose("linghuo", "灵活", start: 40)
      try backspace(at: 42)
      try choose("guoshi", "果实", start: 41)
      try backspace(at: 43)
      let before = try engine.request(EngineRequest(action: "query", input: "lingguo"))
      precondition(!before.candidates.contains { $0.text == "灵果" && $0.personal == true })
      let previous = receipt!
      _ = try engine.request(
        EngineRequest(
          action: "finish-memory", input: "", context: "test-editor",
          learningToken: previous.editToken), expectedIdentity: previous.engineIdentity)
      engine.terminate()
      engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      let learned = try engine.request(EngineRequest(action: "query", input: "lingguo"))
      precondition(learned.candidates.first?.text == "灵果")
      precondition(learned.candidates.first?.personal == true)
      do {
        _ = try engine.request(
          EngineRequest(
            action: "edit-memory", input: "", context: "test-editor",
            learningToken: previous.editToken, editStart: 0, editLength: 1),
          expectedIdentity: previous.engineIdentity)
        preconditionFailure("旧引擎回执不可用于删改")
      } catch {}
      let store = PersonalVocabularyStore(resources: Bundle.main.resourceURL!, file: file)
      let management = PersonalVocabularyController(store: store)
      management.reload()
      wait(management)
      management.search.stringValue = "ling'guo"
      management.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification))
      precondition(management.table.numberOfRows == 1)
      management.table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
      let word = management.selected!
      precondition(word.text == "灵果" && word.syllables == ["ling", "guo"] && word.count == 1)
      management.togglePriority()
      wait(management)
      precondition(management.selected?.prioritized == true)
      management.togglePriority()
      wait(management)
      precondition(management.selected?.prioritized == false)
      management.deleteConfirmed(word)
      wait(management)
      precondition(management.table.numberOfRows == 0)
      let removed = try engine.request(EngineRequest(action: "query", input: "lingguo"))
      precondition(!removed.candidates.contains { $0.text == "灵果" && $0.personal == true })
      engine.terminate()
      engine = try EngineClient(resources: Bundle.main.resourceURL!, memoryURL: file)
      let deletedAfterRestart = try engine.request(EngineRequest(action: "query", input: "lingguo"))
      precondition(
        !deletedAfterRestart.candidates.contains { $0.text == "灵果" && $0.personal == true })
      management.cleanup()
      print("PASS 借词取字与词库管理：创建灵果、完整读音/单次计数、结束后重启召回、搜索/优先/取消/删除及重启后不再出现个人记忆")
      print("PASS 借词取字：灵活删活＋果实删实→灵果，结束后保存与重启召回；删除范围及旧引擎回执保护")
    } catch {
      fputs("FAIL 借词取字隔离检查\n", stderr)
      exit(1)
    }
  }

  private static func wait(_ controller: PersonalVocabularyController) {
    let deadline = Date().addingTimeInterval(5)
    while controller.busy && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
    }
    precondition(!controller.busy)
  }

  private static func rangeChecks() {
    var state = WordEditState()
    precondition(
      !state.record(at: NSNotFound, count: 2, selectionAfter: NSRange(location: 0, length: 0)))
    precondition(!state.canAppend(at: Int.max - 1, count: 2))
    precondition(!state.canAppend(at: Int.max - 1, count: 1))
    precondition(state.record(at: 10, count: 3, selectionAfter: NSRange(location: 13, length: 0)))
    precondition(state.expired(now: ProcessInfo.processInfo.systemUptime + 21))
    precondition(!state.expired())
    precondition(state.isAtEnd(NSRange(location: 13, length: 0)))
    precondition(!state.isAtEnd(NSRange(location: 12, length: 0)))
    precondition(!state.isAtEnd(NSRange(location: 11, length: 2)))
    precondition(state.prepareBackspace(selection: NSRange(location: 13, length: 0)))
    // The host ignored deletion; don't assume its own pending text was removed.
    guard case .invalid = state.reconcile(selection: NSRange(location: 13, length: 0)) else {
      preconditionFailure("未执行的删除必须失效")
    }
    precondition(state.count == 0)
    precondition(state.record(at: 10, count: 3, selectionAfter: NSRange(location: 13, length: 0)))
    precondition(state.prepareBackspace(selection: NSRange(location: 10, length: 2)))
    guard case .deleted(let deletion) = state.reconcile(selection: NSRange(location: 10, length: 0))
    else { preconditionFailure("前缀选区删除必须确认") }
    precondition(deletion.offset == 0 && deletion.length == 2 && state.count == 1)
    precondition(!state.prepareBackspace(selection: NSRange(location: 10, length: 0)))
    precondition(!state.canAppend(at: 10, count: 2))
    precondition(state.record(at: 11, count: 2, selectionAfter: NSRange(location: 13, length: 0)))
    precondition(state.prepareBackspace(selection: NSRange(location: 12, length: 0)))
    guard case .deleted(let middle) = state.reconcile(selection: NSRange(location: 11, length: 0))
    else { preconditionFailure("中间字删除必须确认") }
    precondition(middle.offset == 1 && state.count == 2)
    guard case .invalid = state.reconcile(selection: NSRange(location: 9, length: 0)) else {
      preconditionFailure("范围外光标必须结束跟踪")
    }
    precondition(!state.record(at: 1, count: 2, selectionAfter: NSRange(location: 2, length: 0)))
    precondition(!state.record(at: 1, count: 17, selectionAfter: NSRange(location: 18, length: 0)))
    let generation = UUID()
    precondition(state.record(at: 1, count: 2, selectionAfter: NSRange(location: 3, length: 0)))
    precondition(!state.discardIfGenerationChanged(previous: generation, current: generation))
    precondition(state.discardIfGenerationChanged(previous: generation, current: UUID()))
    precondition(state.start == nil && state.count == 0)
    precondition(state.record(at: 10, count: 2, selectionAfter: NSRange(location: 12, length: 0)))
    precondition(state.prepareBackspace(selection: NSRange(location: 10, length: 2)))
    guard case .deleted = state.reconcile(selection: NSRange(location: 10, length: 0)) else {
      preconditionFailure("全删范围需确认")
    }
    precondition(
      state.start == nil && state.count == 0 && !state.isAtEnd(NSRange(location: 10, length: 0)))
  }
}
