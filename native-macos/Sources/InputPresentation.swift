// 输入服务必须允许窗口；候选面板自身保持不激活、不成为键盘窗口。
import AppKit

enum InputPresentation {
  static func configure() {
    guard NSApplication.shared.setActivationPolicy(.accessory) else {
      fputs("输入法候选窗口模式设置失败\n", stderr)
      exit(1)
    }
  }

  static func checks() {
    configure()
    AppearanceSettings.configure(isolated: true, initial: .light)
    let app = NSApplication.shared
    let foreground = NSWorkspace.shared.frontmostApplication?.processIdentifier
    let engine = try! EngineClient(resources: Bundle.main.resourceURL!)
    defer { engine.terminate() }
    var state = SessionState()
    state.input = "xuexi"
    state.frame = try! engine.request(EngineRequest(action: "query", input: state.input))
    let panel = CandidatePanel()
    for width: CGFloat in [560, 440] {
      panel.show(
        state, bilingual: true, anchor: NSRect(x: 100, y: 200, width: 1, height: 20),
        maximumWidth: width)
      RunLoop.current.run(until: Date().addingTimeInterval(0.1))
      precondition(app.activationPolicy() == .accessory)
      precondition(panel.isVisible && panel.frame.width <= width && panel.frame.width >= 400)
      precondition(panel.frame.height <= 182)
      func descendants(_ view: NSView) -> [NSView] {
        [view] + view.subviews.flatMap(descendants)
      }
      let views = descendants(panel.contentView!)
      precondition(
        !views.compactMap { $0 as? NSTextField }.contains { $0.stringValue == state.markedInput })
      let buttons = views.compactMap { $0 as? ActionButton }
      for symbol in ["chevron.left", "chevron.right", "chevron.down", "gearshape"] {
        precondition(
          buttons.contains {
            $0.symbolName == symbol && $0.bounds.width >= 44 && $0.bounds.height >= 44
          })
      }
      var pageDelta = 0
      panel.onPage = { pageDelta = $0 }
      buttons.first { $0.symbolName == "chevron.right" }?.invoke?()
      precondition(pageDelta == 1)
      var openedSettings = false
      panel.onSettings = { openedSettings = true }
      buttons.first { $0.symbolName == "gearshape" }?.invoke?()
      precondition(openedSettings)
      precondition(!panel.canBecomeKey && !panel.canBecomeMain)
      precondition(!panel.isKeyWindow && !panel.isMainWindow && app.keyWindow == nil)
      precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == foreground)
      panel.orderOut(nil)
      precondition(!panel.isVisible)
    }
    for appearance in [AppearanceChoice.light, .dark] {
      AppearanceSettings.current.select(appearance)
      for mode in [TypingMode.chinese, .english] {
        panel.showTypingMode(mode, anchor: NSRect(x: 100, y: 200, width: 1, height: 20))
        RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        precondition(panel.isVisible && panel.frame.width == 360 && panel.frame.height == 68)
        precondition(!panel.isKeyWindow && !panel.isMainWindow && app.keyWindow == nil)
        precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == foreground)
        panel.orderOut(nil)
      }
    }
    print("PASS 输入服务窗口模式：560/440宽候选可见、可隐藏、不抢前台/键盘焦点；独立词库")
  }
}
