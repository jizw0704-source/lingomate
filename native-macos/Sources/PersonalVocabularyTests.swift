// 隔离词库、真实Rust管理/输入桥接与AppKit界面；不读取用户文件或宿主正文。
import AppKit

enum PersonalVocabularyTests {
  static func run() {
    do {
      let resources = Bundle.main.resourceURL!
      for appearance in [AppearanceChoice.light, .dark] {
        AppearanceSettings.current.select(appearance)
        for width: CGFloat in [560, 440] {
          let controller = SettingsController(preview: true)
          controller.window.setContentSize(NSSize(width: width, height: 620))
          controller.select(.vocabulary)
          let local = controller.personalVocabulary
          defer { controller.cleanupVocabulary() }
          wait(local)
          controller.window.contentView?.layoutSubtreeIfNeeded()
          precondition(local.table.numberOfRows == 3)
          precondition(
            allViews(controller.window.contentView!).compactMap { $0 as? ActionButton }.allSatisfy {
              $0.bounds.height >= 44
            })
          for query in ["市", "s", "SHI", "钙钛镍", "gai tai nie", "gai'tai'nie"] {
            local.search.stringValue = query
            local.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification))
            precondition(local.table.numberOfRows == 1)
          }
          local.search.stringValue = "missing"
          local.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification))
          precondition(local.table.numberOfRows == 0 && local.status.stringValue.contains("未找到"))
          local.search.stringValue = "市"
          local.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification))
          local.table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
          let word = local.selected!
          let before = try Data(contentsOf: local.store.file)
          local.togglePriority()
          precondition(local.busy)
          wait(local)
          precondition(local.selected?.prioritized == true)
          let unchanged = try Data(contentsOf: local.store.file)
          precondition(unchanged == before)
          let engine = try EngineClient(resources: resources, memoryURL: local.store.file)
          let query = try engine.request(.init(action: "query", input: "shi", context: "temporary"))
          precondition(query.candidates.first?.text == "市")
          local.togglePriority()
          wait(local)
          precondition(local.selected?.prioritized == false)
          controller.select(.input)
          controller.select(.vocabulary)
          wait(local)
          precondition(local.search.stringValue == "市" && local.table.numberOfRows == 1)
          local.deleteConfirmed(word)
          wait(local)
          precondition(local.table.numberOfRows == 0)
          let remaining = try local.store.snapshot()
          precondition(!remaining.contains { $0.text == "市" })
          let after = try engine.request(.init(action: "query", input: "shi", context: "temporary"))
          precondition(!after.candidates.contains { $0.text == "市" && $0.personal == true })
          engine.terminate()
          let broken = Data("{broken".utf8)
          try broken.write(to: local.store.file)
          local.reload()
          wait(local)
          precondition(local.table.numberOfRows == 0 && local.status.stringValue.contains("未完成"))
          let preserved = try Data(contentsOf: local.store.file)
          precondition(preserved == broken)
          try FileManager.default.removeItem(at: local.store.file)
          local.reload()
          wait(local)
          precondition(local.status.stringValue.contains("暂无"))
          controller.window.orderOut(nil)
          controller.cleanupVocabulary()
          precondition(
            !FileManager.default.fileExists(
              atPath: local.store.file.deletingLastPathComponent().path))
        }
      }
      let closing = PersonalVocabularyController(store: PersonalVocabularyStore(preview: true))
      closing.reload()
      closing.cleanup()
      precondition(
        !FileManager.default.fileExists(atPath: closing.store.file.deletingLastPathComponent().path)
      )
      wait(closing)
      AppearanceSettings.current.select(.light)
      print("PASS 个人词库：真实桥接查看/别名合并/搜索/取消优先/删除后运行中重读、560/440深浅色布局、空/损坏状态；仅临时文件")
    } catch { preconditionFailure("个人词库测试失败：\(error)") }
  }
  private static func wait(_ controller: PersonalVocabularyController) {
    let deadline = Date().addingTimeInterval(5)
    while controller.busy && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
    }
    precondition(!controller.busy)
  }
  private static func allViews(_ view: NSView) -> [NSView] {
    [view] + view.subviews.flatMap(allViews)
  }
}
