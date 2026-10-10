// 设置为独立辅助窗口，登录/翻译复用原有控制器；正常输入服务不加载凭据。
import AppKit

enum SettingsSection: Int, CaseIterable {
  case account, translation, input, appearance
  var title: String {
    switch self {
    case .account: return "账号与学习"
    case .translation: return "翻译"
    case .input: return "输入"
    case .appearance: return "外观"
    }
  }
}

enum SettingsWindow {
  static var controller: SettingsController?
  static func launch() {
    guard !AppearanceSettings.current.isIsolated else { return }
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    process.arguments = ["--settings"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try? process.run()
  }
  static func start(preview: Bool) {
    NSApplication.shared.setActivationPolicy(.regular)
    let controller = SettingsController(preview: preview)
    self.controller = controller
    controller.window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    controller.account.refreshIfSignedIn()
  }
}

final class SettingsController: NSObject, NSWindowDelegate {
  let window: NSWindow
  let account: LearningWindowController
  let translation: AISettingsController
  let input: SettingsInputClient
  private let body = NSView()
  private var tabs: [ActionButton] = []
  private(set) var section = SettingsSection.account
  private let preview: Bool

  init(preview: Bool) {
    self.preview = preview
    account = LearningWindowController(preview: preview, embedded: true)
    translation = AISettingsController(preview: preview, embedded: true)
    input = SettingsInputClient(preview: preview)
    window = NSWindow(
      contentRect: NSRect(
        x: 0, y: 0, width: CommandLine.arguments.contains("--preview-narrow") ? 440 : 560,
        height: 620),
      styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false
    )
    super.init()
    window.delegate = self
    window.title = preview ? "设置 · 隔离界面样例" : "灵果 · 设置"
    window.minSize = NSSize(width: 440, height: 440)
    window.backgroundColor = NativeTheme.background
    let root = CandidateDocumentView()
    root.wantsLayer = true
    root.fill = NativeTheme.background
    let stack = column()
    stack.translatesAutoresizingMaskIntoConstraints = false
    root.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: root.topAnchor, constant: 12),
      stack.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -12),
    ])
    add(
      row([
        NativeTheme.label("设置", size: 22, weight: .medium), NSView(),
        NativeTheme.label("灵果", size: 12, secondary: true),
      ]), to: stack)
    tabs = SettingsSection.allCases.map { section in
      let button = ActionButton(section.title) { [weak self] in self?.select(section) }
      button.font = NativeTheme.font(13)
      return button
    }
    let navigation = row(tabs)
    navigation.distribution = .fillEqually
    add(navigation, to: stack)
    add(body, to: stack)
    body.heightAnchor.constraint(greaterThanOrEqualToConstant: 220).isActive = true
    window.contentView = root
    account.onContentChange = { [weak self] in
      guard let self, self.section == .account else { return }
      self.mount(self.account.contentView)
    }
    input.onChange = { [weak self] in
      guard let self, self.section == .input else { return }
      self.showInput()
    }
    select(.account)
    input.refresh()
    window.center()
  }

  private func column() -> NSStackView {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    return stack
  }
  private func row(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.spacing = 8
    stack.alignment = .centerY
    return stack
  }
  private func add(_ view: NSView, to stack: NSStackView) {
    stack.addArrangedSubview(view)
    view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
  }
  func select(_ section: SettingsSection) {
    account.prepareForNavigation()
    self.section = section
    for (index, tab) in tabs.enumerated() {
      tab.style = index == section.rawValue ? .subtle : .plain
      tab.state = index == section.rawValue ? .on : .off
    }
    switch section {
    case .account: mount(account.contentView)
    case .translation: mount(translation.contentView)
    case .input: showInput(preserveScroll: false)
    case .appearance: showAppearance()
    }
  }
  private func mount(_ view: NSView) {
    for view in body.subviews { view.removeFromSuperview() }
    view.translatesAutoresizingMaskIntoConstraints = false
    body.addSubview(view)
    NSLayoutConstraint.activate([
      view.leadingAnchor.constraint(equalTo: body.leadingAnchor),
      view.trailingAnchor.constraint(equalTo: body.trailingAnchor),
      view.topAnchor.constraint(equalTo: body.topAnchor),
      view.bottomAnchor.constraint(equalTo: body.bottomAnchor),
    ])
    resizeDocument()
  }
  private func form(preserveScroll: Bool = false, _ build: (NSStackView) -> Void) {
    let offset = (body.subviews.first as? NSScrollView)?.contentView.bounds.origin ?? .zero
    let root = CandidateDocumentView()
    root.wantsLayer = true
    root.fill = NativeTheme.background
    let stack = column()
    stack.translatesAutoresizingMaskIntoConstraints = false
    root.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: root.topAnchor, constant: 16),
      stack.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -16),
    ])
    build(stack)
    let scroll = NSScrollView()
    scroll.drawsBackground = false
    scroll.hasVerticalScroller = true
    scroll.documentView = root
    root.translatesAutoresizingMaskIntoConstraints = false
    root.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor).isActive = true
    mount(scroll)
    if preserveScroll { scroll.contentView.scroll(to: offset) }
  }
  private func showInput(preserveScroll: Bool = true) {
    form(preserveScroll: preserveScroll) { stack in
      add(NativeTheme.label("输入", size: 20, weight: .medium), to: stack)
      add(NativeTheme.label(input.message, size: 13, secondary: true), to: stack)
      func choices(_ title: String, values: [(String, Bool, InputSetting)]) {
        add(NativeTheme.label(title, size: 14, weight: .medium), to: stack)
        for (label, selected, setting) in values {
          let button = ActionButton((selected ? "✓ " : "") + label) { [weak self] in
            self?.input.request(setting)
          }
          button.alignment = .left
          button.style = selected ? .subtle : .plain
          button.isEnabled = input.snapshot != nil && !input.waiting
          add(button, to: stack)
        }
      }
      let value = input.snapshot
      choices(
        "输入语言 · 单按 Shift 也能切换",
        values: [
          ("中文拼音", value?.typing == .chinese, .typing(.chinese)),
          ("英文直输", value?.typing == .english, .typing(.english)),
        ])
      choices(
        "中文候选",
        values: [
          ("同时显示英文译词", value?.bilingual == true, .bilingual(true)),
          ("只显示中文", value?.bilingual == false, .bilingual(false)),
        ])
      choices(
        "标点",
        values: PunctuationMode.allCases.map {
          ($0.title, value?.punctuation == $0, .punctuation($0))
        })
      choices(
        "选词记忆",
        values: [
          ("开启记忆", value?.remember == true, .remember(true)),
          ("暂停新增记忆（保留已有词）", value?.remember == false, .remember(false)),
        ])
      let refresh = ActionButton("刷新输入设置") { [weak self] in self?.input.refresh() }
      refresh.isEnabled = !input.waiting
      add(refresh, to: stack)
    }
  }
  private func showAppearance() {
    form { stack in
      add(NativeTheme.label("外观", size: 20, weight: .medium), to: stack)
      add(NativeTheme.label("选择输入法和设置窗口的外观。", size: 13, secondary: true), to: stack)
      for choice in AppearanceChoice.allCases {
        let selected = AppearanceSettings.current.choice == choice
        let button = ActionButton((selected ? "✓ " : "") + choice.title) { [weak self] in
          AppearanceSettings.current.select(choice)
          self?.showAppearance()
        }
        button.style = selected ? .subtle : .plain
        button.alignment = .left
        add(button, to: stack)
      }
      if preview { add(NativeTheme.label("界面样例：外观只在此预览中切换。", secondary: true), to: stack) }
      let update = ActionButton("检查版本更新…") { MacUpdates.launch() }
      update.isEnabled = !preview
      add(update, to: stack)
    }
  }
  private func resizeDocument() {
    window.contentView?.layoutSubtreeIfNeeded()
    guard let scroll = body.subviews.first as? NSScrollView, let document = scroll.documentView
    else { return }
    document.layoutSubtreeIfNeeded()
    let stack = document.subviews.first as? NSStackView
    document.setFrameSize(
      NSSize(
        width: scroll.contentView.bounds.width,
        height: max(scroll.contentView.bounds.height, (stack?.fittingSize.height ?? 0) + 32)))
  }
  func windowDidResize(_ notification: Notification) { resizeDocument() }
  func windowDidBecomeKey(_ notification: Notification) { if !input.waiting { input.refresh() } }
  func windowWillClose(_ notification: Notification) { NSApplication.shared.stop(nil) }
}
