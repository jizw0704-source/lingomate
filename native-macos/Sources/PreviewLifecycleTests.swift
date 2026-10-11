// 独立子进程验证预览关闭与到期，不访问真实输入服务、词库、账号或偏好。
import AppKit

enum PreviewLifecycleTests {
  static func run() {
    precondition(try! PreviewLifecycle.duration(for: []) == nil)
    precondition(try! PreviewLifecycle.duration(for: ["--settings"]) == nil)
    precondition(try! PreviewLifecycle.duration(for: ["--settings-test"]) == nil)
    for flag in PreviewLifecycle.flags {
      precondition(try! PreviewLifecycle.duration(for: [flag]) == 300)
      precondition(
        try! PreviewLifecycle.duration(for: [flag, "--preview-duration", "30"]) == 30)
    }
    for args in [
      ["--preview-duration", "30"], ["--preview", "--preview-duration"],
      ["--preview", "--preview-duration", "0"], ["--preview", "--preview-duration", "901"],
      ["--preview", "--preview-duration", "nan"], ["--preview", "--preview-duration", "inf"],
      ["--preview", "--preview-duration", "10", "--preview-duration", "20"],
    ] {
      do {
        _ = try PreviewLifecycle.duration(for: args)
        preconditionFailure("invalid preview duration accepted")
      } catch {}
    }
    for action in ["close", "timeout", "quit"] {
      let child = Process()
      child.executableURL = Bundle.main.executableURL!
      child.arguments = ["--preview-lifecycle-probe", action]
      do {
        try child.run()
        let deadline = Date().addingTimeInterval(5)
        while child.isRunning && Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if child.isRunning {
          child.terminate()
          child.waitUntilExit()
          preconditionFailure("owned preview failed to exit")
        }
        precondition(child.terminationReason == .exit && child.terminationStatus == 0)
      } catch { preconditionFailure("preview probe could not launch") }
    }
    print("PASS 预览时限、主窗关闭、退出命令及浮窗清理；未启动正式输入服务")
  }

  static func probe(_ action: String) {
    precondition(["close", "timeout", "quit"].contains(action))
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let lifecycle = PreviewLifecycle(seconds: action == "timeout" ? 0.15 : 3)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 160, height: 80), styleMask: [.titled, .closable],
      backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    window.title = "灵果 · 预览退出隔离检查"
    let panel = NSPanel(
      contentRect: NSRect(x: 0, y: 0, width: 160, height: 44),
      styleMask: [.nonactivatingPanel], backing: .buffered, defer: false)
    window.orderFront(nil)
    panel.orderFront(nil)
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
      panel.close()
      precondition(!lifecycle.finished)
      panel.orderFront(nil)
      if action == "close" { window.performClose(nil) }
      if action == "quit" { lifecycle.quit(nil) }
    }
    app.run()
    precondition(lifecycle.finished && !window.isVisible && !panel.isVisible)
    lifecycle.finish()  // 重复清理安全。
    print("PASS preview exit \(action)")
  }
}
