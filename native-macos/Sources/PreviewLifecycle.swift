// 仅独立界面预览使用：关闭主窗口或到期时收起同进程浮窗，唤醒事件循环以执行清理。
import AppKit

final class PreviewLifecycle: NSObject {
  static let flags: Set<String> = [
    "--preview", "--preview-sentence", "--preview-paging", "--preview-learning",
    "--preview-punctuation", "--preview-mixed", "--preview-typing", "--account-preview",
    "--ai-settings-preview", "--settings-preview", "--local-learning-preview", "--update-preview",
  ]
  private static var active: PreviewLifecycle?
  private var observer: NSObjectProtocol?
  private var timer: Timer?
  private(set) var finished = false

  static func duration(for arguments: [String]) throws -> TimeInterval? {
    let preview = !flags.isDisjoint(with: arguments)
    guard let index = arguments.firstIndex(of: "--preview-duration") else {
      return preview ? 300 : nil
    }
    guard preview, arguments.count > index + 1,
      let seconds = Double(arguments[index + 1]), seconds.isFinite,
      (1...900).contains(seconds),
      arguments.filter({ $0 == "--preview-duration" }).count == 1
    else {
      throw NSError(
        domain: "PreviewLifecycle", code: 1,
        userInfo: [NSLocalizedDescriptionKey: "预览时限须为 1–900 秒，且仅可用于独立界面预览。"])
    }
    return seconds
  }

  static func start(seconds: TimeInterval) {
    let session = PreviewLifecycle(seconds: seconds)
    active = session
    let menu = NSMenu()
    let item = NSMenuItem()
    let commands = NSMenu()
    let quit = NSMenuItem(title: "退出预览", action: #selector(session.quit(_:)), keyEquivalent: "q")
    quit.target = session
    commands.addItem(quit)
    item.submenu = commands
    menu.addItem(item)
    NSApplication.shared.mainMenu = menu
  }

  init(seconds: TimeInterval) {
    super.init()
    observer = NotificationCenter.default.addObserver(
      forName: NSWindow.willCloseNotification, object: nil, queue: .main
    ) { [weak self] notification in
      // 菜单、弹出框、候选面板关闭不应结束预览；主窗口关闭才退出。
      guard let window = notification.object as? NSWindow, !(window is NSPanel),
        window.styleMask.contains(.titled)
      else { return }
      self?.finish()
    }
    let timer = Timer(timeInterval: seconds, repeats: false) { [weak self] _ in self?.finish() }
    self.timer = timer
    RunLoop.main.add(timer, forMode: .common)
  }

  deinit {
    timer?.invalidate()
    if let observer { NotificationCenter.default.removeObserver(observer) }
  }

  @objc func quit(_ sender: Any?) { finish() }

  func finish() {
    guard !finished else { return }
    finished = true
    timer?.invalidate()
    // NSApplication.windows 仅含本进程窗口，不查询或终止正式输入法及宿主应用。
    let app = NSApplication.shared
    for window in app.windows { window.orderOut(nil) }
    app.stop(nil)
    if let event = NSEvent.otherEvent(
      with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0,
      windowNumber: 0, context: nil, subtype: 0, data1: 0, data2: 0)
    {
      app.postEvent(event, atStart: true)
    }
  }
}
