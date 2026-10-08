import AppKit

// 用户在此明确选择发送目标和开启在线翻译；保存设置不会发送测试请求。
enum AISettingsWindow {
  static var controller: AISettingsController?
  static func launch() {
    guard !AppearanceSettings.current.isIsolated else { return }
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    process.arguments = ["--ai-settings"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try? process.run()
  }
  static func start(preview: Bool) {
    NSApplication.shared.setActivationPolicy(.regular)
    let controller = AISettingsController(preview: preview)
    self.controller = controller
    controller.window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
  }
}
final class AISettingsController: NSObject, NSWindowDelegate {
  let window: NSWindow
  let store: AISettingsStore
  private let preview: Bool
  private let vault: AIKeyVault
  private let endpoint = NSTextField()
  private let model = NSTextField()
  private let secret = NSSecureTextField()
  private let status = NativeTheme.label("默认使用本机翻译，未开启在线请求。", size: 13, secondary: true)
  private let feedback = NativeTheme.label("", size: 13, secondary: true)
  init(preview: Bool) {
    self.preview = preview
    self.store = AISettings.store!
    vault = preview ? MemoryAIKeyVault() : SystemAIKeyVault()
    window = NSWindow(
      contentRect: .init(
        x: 0, y: 0, width: CommandLine.arguments.contains("--preview-narrow") ? 440 : 560,
        height: 540),
      styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false
    )
    super.init()
    window.delegate = self
    window.title = preview ? "AI 翻译设置 · 隔离界面样例" : "中英输入 · AI 翻译设置"
    window.minSize = NSSize(width: 440, height: 440)
    window.backgroundColor = NativeTheme.background
    let root = CandidateDocumentView()
    root.fill = NativeTheme.background
    root.wantsLayer = true
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    root.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: root.topAnchor, constant: 16),
      stack.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -16),
    ])
    func add(_ view: NSView) {
      stack.addArrangedSubview(view)
      view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }
    let heading = NSStackView(views: [
      NativeTheme.label("AI 翻译", size: 20, weight: .medium), NSView(),
      AppearanceSettings.current.button(),
    ])
    heading.orientation = .horizontal
    heading.spacing = 8
    add(heading)
    add(status)
    if preview {
      add(NativeTheme.label("界面样例：设置和密钥仅在临时内存中，不联网、不访问系统钥匙串。", size: 12, secondary: true))
    }
    let domestic = ActionButton("MiniMax 国内") { [weak self] in self?.preset(international: false) }
    let international = ActionButton("MiniMax 国际") { [weak self] in
      self?.preset(international: true)
    }
    let presets = NSStackView(views: [domestic, international, NSView()])
    presets.orientation = .horizontal
    presets.spacing = 8
    add(presets)
    for (field, title, placeholder) in [
      (endpoint, "接口地址", "https://服务域名/v1/chat/completions"),
      (model, "模型名", "填写服务商提供的模型名"),
      (secret as NSTextField, "API 密钥", "新密钥；留空保留该地址已保存的密钥"),
    ] {
      add(NativeTheme.label(title, size: 12, secondary: true))
      field.placeholderString = placeholder
      field.font = NativeTheme.font(14)
      field.setAccessibilityLabel(title)
      field.translatesAutoresizingMaskIntoConstraints = false
      field.heightAnchor.constraint(equalToConstant: 44).isActive = true
      add(field)
    }
    add(
      NativeTheme.label(
        "启用后，停顿约850毫秒的当前较长中文候选会发送到上述接口，不上传拼音、正文历史或个人词库。词语词表仍在本机。API费用及数据处理由你选择的服务商决定。", size: 12,
        secondary: true))
    let save = ActionButton("保存并启用 AI") { [weak self] in self?.save() }
    save.style = .outlined
    let local = ActionButton("使用本机翻译") { [weak self] in self?.disable() }
    let row = NSStackView(views: [save, NSView(), local])
    row.orientation = .horizontal
    row.spacing = 8
    add(row)
    add(feedback)
    do {
      if let configuration = try store.load() {
        endpoint.stringValue = configuration.endpoint.absoluteString
        model.stringValue = configuration.model
        updateStatus(configuration)
      }
    } catch { feedback.stringValue = "原设置文件暂不可读，已保留；请重新填写后保存。" }
    let scroll = NSScrollView()
    scroll.drawsBackground = false
    scroll.hasVerticalScroller = true
    scroll.documentView = root
    window.contentView = scroll
    root.translatesAutoresizingMaskIntoConstraints = false
    root.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor).isActive = true
    root.layoutSubtreeIfNeeded()
    root.setFrameSize(
      NSSize(width: window.contentLayoutRect.width, height: max(540, stack.fittingSize.height + 32))
    )
    window.center()
  }
  private func preset(international: Bool) {
    let value = AIConfiguration.miniMax(international: international)
    endpoint.stringValue = value.endpoint.absoluteString
    model.stringValue = value.model
    secret.stringValue = ""
    feedback.stringValue = "已填入 MiniMax 预设，尚未保存；请使用对应平台的密钥。"
  }
  private func updateStatus(_ configuration: AIConfiguration) {
    status.stringValue =
      configuration.enabled
      ? "AI 已启用 · \(configuration.endpoint.host ?? "") · \(configuration.model)"
      : "当前使用本机翻译，不发送在线请求。"
  }
  private func save() {
    do {
      guard
        let url = URL(string: endpoint.stringValue.trimmingCharacters(in: .whitespacesAndNewlines))
      else { throw AIError.configuration }
      let value = AIConfiguration(
        endpoint: url, model: model.stringValue.trimmingCharacters(in: .whitespacesAndNewlines),
        enabled: true)
      try value.validate()
      let key = secret.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
      if !key.isEmpty { try vault.save(key, scope: value.keyScope) }
      guard let saved = try vault.load(value.keyScope), !saved.isEmpty else {
        throw AIError.missingKey
      }
      try store.save(value)
      secret.stringValue = ""
      updateStatus(value)
      feedback.stringValue = preview ? "界面样例已保存到临时数据，未调用 API。" : "设置已保存，密钥在系统钥匙串；本次未发送翻译请求。"
      if !preview { AISettings.announce() }
    } catch { feedback.stringValue = (error as? AIError ?? .storage).localizedDescription }
  }
  private func disable() {
    do {
      if var value = try store.load() {
        value.enabled = false
        value.revision = UUID()
        try store.save(value)
        updateStatus(value)
      } else {
        status.stringValue = "当前使用本机翻译，不发送在线请求。"
      }
      feedback.stringValue = preview ? "界面样例已切换，未联网。" : "已关闭 AI 在线翻译；已保存的密钥保留在钥匙串。"
      if !preview { AISettings.announce() }
    } catch { feedback.stringValue = (error as? AIError ?? .storage).localizedDescription }
  }
  func windowWillClose(_ notification: Notification) { NSApplication.shared.stop(nil) }
}
