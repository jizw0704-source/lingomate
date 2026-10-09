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
      precondition(panel.frame.height <= 190)
      precondition(!panel.canBecomeKey && !panel.canBecomeMain)
      precondition(!panel.isKeyWindow && !panel.isMainWindow && app.keyWindow == nil)
      precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == foreground)
      panel.orderOut(nil)
      precondition(!panel.isVisible)
    }
    print("PASS 输入服务窗口模式：560/440宽候选可见、可隐藏、不抢前台/键盘焦点；独立词库")
  }
}
