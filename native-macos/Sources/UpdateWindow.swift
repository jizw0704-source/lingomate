import AppKit
import Darwin

struct UpdatePreferences: Codable {
  var autoCheck = false
  var lastChecked: Double = 0
  func due(now: Double = Date().timeIntervalSince1970) -> Bool {
    autoCheck && lastChecked.isFinite && lastChecked >= 0 && now - lastChecked >= 86_400
  }
}

final class UpdateLease {
  private let descriptor: Int32
  init(directory: URL) throws {
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    descriptor = open(
      directory.appendingPathComponent("updater.lock").path,
      O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw InstallFailure(message: "无法创建更新锁。") }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
      close(descriptor)
      throw InstallFailure(message: "另一个更新窗口正在工作，请先关闭它。")
    }
  }
  deinit { close(descriptor) }
}

enum MacUpdates {
  static var controller: MacUpdateController?
  static var timer: Timer?
  static func launch(background: Bool = false) {
    guard !AppearanceSettings.current.isIsolated else { return }
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    process.arguments = [background ? "--update-background" : "--updates"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try? process.run()
  }
  static func schedule() {
    launch(background: true)
    timer = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) { _ in
      launch(background: true)
    }
  }
  static func start(preview: Bool, background: Bool) {
    let home = FileManager.default.homeDirectoryForCurrentUser
    let directory = home.appendingPathComponent(
      "Library/Application Support/BilingualCompanion/Updates")
    let store = MacUpdateStore(
      destination: home.appendingPathComponent("Library/Input Methods/BilingualCompanion.app"),
      directory: directory)
    let preferences = preview ? UpdatePreferences() : loadPreferences(directory)
    if background && !preferences.due() { return }
    NSApp.setActivationPolicy(background ? .accessory : .regular)
    let controller = MacUpdateController(
      store: store, preview: preview, background: background, preferences: preferences)
    self.controller = controller
    NSApp.delegate = controller
    if !preview {
      do { controller.lease = try UpdateLease(directory: directory) } catch {
        if background { return }
        controller.fail(error)
        controller.primary.isEnabled = false
        controller.restore.isEnabled = false
        controller.automatic.isEnabled = false
      }
    }
    if background { controller.check() } else { controller.show() }
    NSApp.run()
    controller.cleanup()
  }
  static func loadPreferences(_ directory: URL) -> UpdatePreferences {
    let url = directory.appendingPathComponent("settings.json")
    guard let values = try? url.resourceValues(forKeys: [.fileSizeKey, .isSymbolicLinkKey]),
      values.isSymbolicLink != true, (values.fileSize ?? 4097) <= 4096,
      let data = try? Data(contentsOf: url),
      let value = try? JSONDecoder().decode(UpdatePreferences.self, from: data),
      value.lastChecked.isFinite, value.lastChecked >= 0
    else { return UpdatePreferences() }
    return value
  }
}

final class MacUpdateController: NSObject, NSWindowDelegate, NSApplicationDelegate {
  enum State {
    case idle, checking, available, downloading, ready, installing, restoring, completed, failed
  }
  let window: NSWindow
  let primary: ActionButton
  let restore: ActionButton
  let close: ActionButton
  let automatic = NSButton(checkboxWithTitle: "每天自动检查更新（只提醒，不自动安装）", target: nil, action: nil)
  let status = NativeTheme.label("检查灵果的新版本。", size: 15)
  let notes = NativeTheme.label("更新来源：灵果 GitHub Releases。下载并通过校验后，由你确认安装。", secondary: true)
  private(set) var state: State = .idle
  let store: MacUpdateStore
  let preview: Bool
  private var background: Bool
  private var preferences: UpdatePreferences
  private var network: UpdateDownload?
  private var generation = UUID()
  private var release: MacRelease?
  private var prepared: URL?
  private let workspace = FileManager.default.temporaryDirectory.appendingPathComponent(
    "lingomate-update-" + UUID().uuidString)
  var lease: UpdateLease?
  var critical: Bool { state == .installing || state == .restoring }

  init(store: MacUpdateStore, preview: Bool, background: Bool, preferences: UpdatePreferences) {
    self.store = store
    self.preview = preview
    self.background = background
    self.preferences = preferences
    primary = ActionButton("检查更新") {}
    restore = ActionButton("恢复上一版本") {}
    close = ActionButton("关闭") {}
    window = NSWindow(
      contentRect: NSRect(
        x: 0, y: 0, width: CommandLine.arguments.contains("--preview-narrow") ? 440 : 560,
        height: 424), styleMask: [.titled, .closable, .resizable, .miniaturizable],
      backing: .buffered, defer: false)
    super.init()
    window.title = preview ? "灵果 · 更新界面预览" : "灵果 · 软件更新"
    window.minSize = NSSize(width: 440, height: 424)
    window.delegate = self
    window.backgroundColor = NativeTheme.background
    let view = CandidateDocumentView()
    view.wantsLayer = true
    view.fill = NativeTheme.background
    let title = NativeTheme.label("软件更新", size: 24, weight: .medium)
    let header = NSStackView(views: [title, NSView(), AppearanceSettings.current.button()])
    header.spacing = 16
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.drawsBackground = false
    let document = CandidateDocumentView()
    document.wantsLayer = true
    document.fill = NativeTheme.surface
    notes.translatesAutoresizingMaskIntoConstraints = false
    document.addSubview(notes)
    scroll.documentView = document
    document.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
      notes.leadingAnchor.constraint(equalTo: document.leadingAnchor, constant: 16),
      notes.trailingAnchor.constraint(equalTo: document.trailingAnchor, constant: -16),
      notes.topAnchor.constraint(equalTo: document.topAnchor, constant: 12),
      notes.bottomAnchor.constraint(equalTo: document.bottomAnchor, constant: -12),
      scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 128),
    ])
    automatic.state = preferences.autoCheck ? .on : .off
    automatic.target = self
    automatic.action = #selector(autoChanged)
    automatic.font = NativeTheme.font(12)
    automatic.heightAnchor.constraint(equalToConstant: 44).isActive = true
    let buttons = NSStackView(views: [primary, restore, close])
    buttons.spacing = 8
    buttons.distribution = .fillEqually
    let stack = NSStackView(views: [header, status, scroll, automatic, buttons])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 16
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    for child in [header, status, scroll, automatic, buttons] {
      child.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
      stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 24),
      stack.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -24),
    ])
    primary.style = .subtle
    primary.keyEquivalent = "\r"
    close.keyEquivalent = "\u{1b}"
    primary.invoke = { [weak self] in self?.act() }
    restore.invoke = { [weak self] in self?.restoreClicked() }
    close.invoke = { [weak self] in self?.closeClicked() }
    window.contentView = view
    window.center()
    if preview {
      notes.stringValue = "隔离预览：不联网、不读取个人更新设置、不安装或恢复应用。"
      if let at = CommandLine.arguments.firstIndex(of: "--update-state"),
        CommandLine.arguments.count > at + 1
      {
        fixture(CommandLine.arguments[at + 1])
      }
    }
  }
  func show(activate: Bool = true) {
    window.makeKeyAndOrderFront(nil)
    if activate { NSApp.activate(ignoringOtherApps: true) }
  }
  func set(_ state: State, _ message: String, _ action: String) {
    self.state = state
    status.stringValue = message
    primary.title = action
    primary.setAccessibilityLabel(action)
    primary.isEnabled = ![.checking, .downloading, .installing, .restoring, .completed].contains(
      state)
    restore.isEnabled = ![.checking, .downloading, .installing, .restoring].contains(state)
    automatic.isEnabled = restore.isEnabled
    close.isEnabled = !critical
    close.title = state == .checking || state == .downloading ? "取消" : "关闭"
    close.setAccessibilityLabel(close.title)
    window.standardWindowButton(.closeButton)?.isEnabled = !critical
  }
  func fail(_ error: Error) {
    prepared = nil
    release = nil
    set(.failed, "更新未完成：\(error.localizedDescription)", "重新检查")
  }
  private func savePreferences() throws {
    guard !preview else { return }
    try FileManager.default.createDirectory(at: store.directory, withIntermediateDirectories: true)
    try JSONEncoder().encode(preferences).write(
      to: store.directory.appendingPathComponent("settings.json"), options: .atomic)
  }
  @objc private func autoChanged() {
    preferences.autoCheck = automatic.state == .on
    do { try savePreferences() } catch {
      automatic.state = .off
      preferences.autoCheck = false
      fail(InstallFailure(message: "无法保存更新偏好。"))
    }
  }
  func check() {
    guard !preview else {
      fixture("available")
      return
    }
    preferences.lastChecked = Date().timeIntervalSince1970
    do { try savePreferences() } catch {
      fail(InstallFailure(message: "无法保存更新检查记录。"))
      return
    }
    set(.checking, "正在检查新版本…", "请稍候")
    let downloader = UpdateDownload()
    network = downloader
    let token = UUID()
    generation = token
    DispatchQueue.global(qos: .utility).async { [self] in
      let result = Result { () throws -> MacRelease? in
        try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)
        let file = workspace.appendingPathComponent(UUID().uuidString + ".json")
        defer { try? FileManager.default.removeItem(at: file) }
        let current = try store.version(store.destination)
        try downloader.fetch(UpdateDownload.feed, to: file, limit: 65_536)
        let metadata = try JSONDecoder().decode(MacRelease.self, from: Data(contentsOf: file))
        return try metadata.available(
          after: current, osMajor: ProcessInfo.processInfo.operatingSystemVersion.majorVersion)
          ? metadata : nil
      }
      DispatchQueue.main.async { [self] in
        guard generation == token else { return }
        network = nil
        if background {
          guard case .success(.some) = result else {
            NSApp.stop(nil)
            return
          }
          background = false
          window.orderFrontRegardless()  // Reminder must not activate the host's input focus.
        }
        switch result {
        case .success(let metadata):
          release = metadata
          if let metadata {
            notes.stringValue = metadata.notes ?? ""
            set(.available, "发现新版本 \(metadata.version!)。", "下载更新")
          } else {
            set(.idle, "当前没有更高版本可用。", "重新检查")
          }
        case .failure(let error): fail(error)
        }
      }
    }
  }
  private func download() {
    guard let release, let address = release.downloadURL, let url = URL(string: address) else {
      return
    }
    set(.downloading, "正在下载并校验更新…", "请稍候")
    let downloader = UpdateDownload()
    network = downloader
    let token = UUID()
    generation = token
    DispatchQueue.global(qos: .utility).async { [self] in
      let result = Result { () throws -> URL in
        let zip = workspace.appendingPathComponent(UUID().uuidString + ".zip")
        defer { try? FileManager.default.removeItem(at: zip) }
        try downloader.fetch(url, to: zip, limit: release.size!)
        return try store.prepare(
          zip, release: release, to: workspace.appendingPathComponent(UUID().uuidString))
      }
      DispatchQueue.main.async { [self] in
        guard generation == token else { return }
        network = nil
        switch result {
        case .success(let app):
          prepared = app
          set(.ready, "更新已下载，通过校验与系统安全检查。", "安装更新")
        case .failure(let error): fail(error)
        }
      }
    }
  }
  private func confirm(_ title: String) -> Bool {
    let alert = NSAlert()
    alert.messageText = title
    alert.informativeText = "请先切换到其他输入法，保存工作并退出灵果输入服务。更新器不会关闭聊天软件或重启电脑；完成后请重新打开需要使用灵果的应用。"
    alert.addButton(withTitle: "取消")
    alert.addButton(withTitle: "确认")
    return alert.runModal() == .alertSecondButtonReturn
  }
  private func act() {
    if preview {
      switch state {
      case .available: fixture("ready")
      case .ready: fixture("installed")
      default: fixture("available")
      }
      return
    }
    switch state {
    case .available: download()
    case .ready:
      guard let prepared, let release, confirm("安装灵果更新？") else { return }
      mutate(restoring: false) { try self.store.install(prepared, release: release) }
    default: check()
    }
  }
  private func restoreClicked() {
    if preview {
      fixture("restored")
      return
    }
    guard confirm("恢复上一版本？") else { return }
    mutate(restoring: true) {
      try self.store.restore(to: self.workspace.appendingPathComponent(UUID().uuidString))
    }
  }
  private func mutate(restoring: Bool, action: @escaping () throws -> Void) {
    set(
      restoring ? .restoring : .installing, restoring ? "正在恢复上一版本，请保持窗口打开…" : "正在安装，请保持窗口打开…", "请稍候"
    )
    DispatchQueue.global(qos: .userInitiated).async { [self] in
      let result = Result {
        try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)
        try action()
      }
      DispatchQueue.main.async { [self] in
        switch result {
        case .success:
          set(.completed, restoring ? "已恢复上一版本。" : "更新已安装。", "已完成")
          notes.stringValue = "旧文件已保留。请保存工作，重新打开使用灵果的应用，再选择灵果。若系统尚未重新连接，请保存工作后手动重新登录。"
        case .failure(let error): fail(error)
        }
      }
    }
  }
  private func closeClicked() {
    guard !critical else { return }
    if state == .checking || state == .downloading {
      generation = UUID()
      network?.cancel()
      network = nil
      prepared = nil
      release = nil
      set(.idle, "更新操作已取消，当前版本未修改。", "重新检查")
    } else {
      window.close()
    }
  }
  func windowShouldClose(_ sender: NSWindow) -> Bool { !critical }
  func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
    critical ? .terminateCancel : .terminateNow
  }
  func windowWillClose(_ notification: Notification) {
    network?.cancel()
    NSApp.stop(nil)
  }
  func cleanup() {
    network?.cancel()
    try? FileManager.default.removeItem(at: workspace)
  }
  func fixture(_ name: String) {
    notes.stringValue = "隔离界面示例，不联网、不修改更新设置或应用。\n新版本示例：改进候选和设置。"
    switch name {
    case "available": set(.available, "发现新版本（界面示例）。", "下载更新")
    case "ready": set(.ready, "下载与校验完成（界面示例）。", "安装更新")
    case "loading": set(.checking, "正在检查更新（界面示例）…", "请稍候")
    case "installing": set(.installing, "正在安装（界面示例）…", "请稍候")
    case "error": set(.failed, "网络暂不可用（界面示例）。", "重新检查")
    case "installed": set(.completed, "安装完成（界面示例），实际未安装。", "已完成")
    case "restored": set(.completed, "恢复完成（界面示例），实际版本未修改。", "已完成")
    default: set(.idle, "当前没有更新（界面示例）。", "重新检查")
    }
  }
}
