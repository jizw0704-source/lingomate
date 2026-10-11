import AppKit

enum LocalLearningTests {
  static func run() {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = LocalLearningStore(directory: root)
    let word = LearnedWord(english: "learn", chinese: "学习", pos: "v.", uses: 0, mastered: false)
    do {
      precondition((try? store.snapshot().isEmpty) == true)
      try store.record(word)
      try store.record(word)
      try store.setMastered(word.id, true)
      try store.record(word)
      let restarted = LocalLearningStore(directory: root)
      var words = try restarted.snapshot()
      precondition(words.count == 1 && words[0].uses == 3 && words[0].mastered)
      try store.setMastered(word.id, false)
      let group = DispatchGroup()
      for _ in 0..<40 {
        group.enter()
        DispatchQueue.global().async {
          do { try LocalLearningStore(directory: root).record(word) } catch {
            preconditionFailure("并发记录失败")
          }
          group.leave()
        }
      }
      group.wait()
      words = try store.snapshot()
      precondition(words[0].uses == 43 && !words[0].mastered)
      let csv = String(decoding: LocalLearningStore.csv(words), as: UTF8.self)
      precondition(csv.contains("\"learn\",\"学习\",\"v.\",\"43\",\"false\""))
      let formula = LearnedWord(
        english: "=example", chinese: "示例", pos: "", uses: 1, mastered: false)
      precondition(
        String(decoding: LocalLearningStore.csv([formula]), as: UTF8.self).contains("\"'=example\"")
      )
      for width: CGFloat in [560, 440] {
        LearningRuntime.localStore = store
        let controller = SettingsController(preview: true)
        controller.select(.learning)
        controller.window.setContentSize(NSSize(width: width, height: 620))
        controller.window.contentView?.layoutSubtreeIfNeeded()
        let local = controller.localLearning
        precondition(local.table.numberOfRows == 1)
        local.table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        try store.setMastered(word.id, false)
        local.reload()
        local.table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        local.toggleMastered()
        precondition((try? store.snapshot()[0].mastered) == true)
        local.search.stringValue = "missing"
        local.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification))
        precondition(local.table.numberOfRows == 0)
        controller.window.orderOut(nil)
      }
      try store.delete(word.id)
      let savedCloud = LearningRuntime.store
      LearningRuntime.store = nil
      LearningRuntime.localStore = store
      LearningRuntime.confirmed(
        .init(word: "learn", pos: "v.", note: "", example: ""), chinese: "学习", account: nil)
      LearningRuntime.waitForLocalTests()
      precondition((try? store.snapshot()[0].uses) == 1)
      LearningRuntime.store = savedCloud
      try store.delete(word.id)
      precondition((try? store.snapshot().isEmpty) == true)
      let broken = Data("{broken".utf8)
      try broken.write(to: store.file)
      do {
        try store.record(word)
        preconditionFailure("不可覆盖损坏记录")
      } catch {}
      precondition((try? Data(contentsOf: store.file)) == broken)
      print("PASS 本地学习：离线创建、重复计数、手动掌握、重启、并发、删除、损坏保留、CSV安全、560/440学习界面搜索/标记")
    } catch { preconditionFailure("本地学习测试失败：\(error)") }
  }
}
