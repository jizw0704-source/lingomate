import AppKit

// 独立正常窗口承载账号，输入候选窗口仍不激活或抢焦点。
enum AccountWindow {
  static var window: NSWindow?
  static var controller: LearningWindowController?
  static func launch(syncOnly: Bool = false) {
    guard !AppearanceSettings.current.isIsolated else { return }
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    process.arguments = [syncOnly ? "--sync-learning" : "--account"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try? process.run()
  }
  static func start(preview: Bool) {
    NSApplication.shared.setActivationPolicy(.regular)
    let controller = LearningWindowController(preview: preview)
    self.controller = controller
    self.window = controller.window
    controller.window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    controller.refreshIfSignedIn()
  }
  static func configuration(_ store: LearningStore) throws -> AccountConfiguration? {
    let url = store.directory.appendingPathComponent("service.json")
    guard FileManager.default.fileExists(atPath: url.path) else { return nil }
    let configuration = try JSONDecoder().decode(
      AccountConfiguration.self, from: Data(contentsOf: url))
    try configuration.validate()
    return configuration
  }
  static func sync() async {
    guard let store = LearningRuntime.store else { return }
    do {
      guard let configuration = try configuration(store) else { return }
      try await AccountService(configuration: configuration, store: store).sync()
    } catch {
      // 离线时队列保留，账号窗口可重试；不输出凭据或输入内容。
    }
  }
}

final class LearningWindowController: NSObject, NSWindowDelegate {
  let window: NSWindow
  let store: LearningStore
  private(set) var contentView: NSScrollView!
  var onContentChange: (() -> Void)?
  private let embedded: Bool
  private let preview: Bool
  private var email = ""
  private var code = ""
  private var search = ""
  private var filter = "全部"
  private var visibleLimit = 50
  private var message = ""
  private var busy = false
  private var codeRequested = false
  private var cooldown = Date.distantPast
  private var configurationVisible = false
  private var serviceURL = ""
  private var fields: [String: NSTextField] = [:]
  private var timer: Timer?
  private var renderedFingerprint = ""
  private weak var sendButton: ActionButton?

  init(preview: Bool, embedded: Bool = false) {
    self.preview = preview
    self.embedded = embedded
    store = LearningRuntime.store!
    window = NSWindow(
      contentRect: .init(
        x: 0, y: 0, width: CommandLine.arguments.contains("--preview-narrow") ? 440 : 560,
        height: 480),
      styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false
    )
    super.init()
    window.delegate = self
    window.title = preview ? "学习中心 · 界面样例（未真实登录）" : "灵果 · 学习中心"
    window.minSize = NSSize(width: 440, height: 400)
    window.backgroundColor = NativeTheme.background
    if let configuration = try? AccountWindow.configuration(store) {
      serviceURL = configuration.baseURL.absoluteString
    }
    if preview, CommandLine.arguments.contains("--account-state") {
      let index = CommandLine.arguments.firstIndex(of: "--account-state")!
      let fixture =
        CommandLine.arguments.indices.contains(index + 1) ? CommandLine.arguments[index + 1] : ""
      if ["words", "empty", "many"].contains(fixture) {
        let account = LearningAccount(id: UUID().uuidString, email: "界面测试账号", project: "preview")
        try? store.transaction { data in
          data.active = account
          if fixture == "words" {
            data.words[account.scope] = [
              .init(english: "learn", chinese: "学习", pos: "v.", uses: 2, mastered: false),
              .init(english: "friend", chinese: "朋友", pos: "n.", uses: 1, mastered: true),
            ]
          } else if fixture == "many" {
            data.words[account.scope] = (1...60).map {
              .init(
                english: "sample word \($0)", chinese: "学习", pos: "n.", uses: 1, mastered: false)
            }
          }
        }
      } else if fixture == "error" {
        message = "服务暂不可用，请检查连接后重试。"
      }
    }
    render()
    window.center()
    timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
      guard let self, !self.busy else { return }
      self.sendButton?.isEnabled =
        !self.preview && Date() >= self.cooldown
        && (try? AccountWindow.configuration(self.store)) != nil
      guard let data = try? self.store.snapshot(), data.active != nil,
        self.fingerprint(data) != self.renderedFingerprint
      else { return }
      self.capture()
      self.render(preserveScroll: true)
    }
  }

  private func column(_ views: [NSView] = [], spacing: CGFloat = 8) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = spacing
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
  }
  private func row(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.alignment = .centerY
    stack.spacing = 8
    return stack
  }
  private func add(_ view: NSView, to stack: NSStackView) {
    stack.addArrangedSubview(view)
    view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
  }
  private func button(_ title: String, primary: Bool = false, action: @escaping () -> Void)
    -> ActionButton
  {
    let button = ActionButton(title, action: action)
    button.style = primary ? .outlined : .plain
    button.isEnabled = !busy
    return button
  }
  private func field(_ placeholder: String, key: String, value: String) -> NSTextField {
    let field = NSTextField(string: value)
    field.placeholderString = placeholder
    field.font = NativeTheme.font(14)
    field.setAccessibilityLabel(placeholder)
    field.translatesAutoresizingMaskIntoConstraints = false
    field.heightAnchor.constraint(equalToConstant: 44).isActive = true
    field.isEnabled = !busy
    fields[key] = field
    return field
  }
  private func capture() {
    email = fields["email"]?.stringValue.trimmingCharacters(in: .whitespacesAndNewlines) ?? email
    code = fields["code"]?.stringValue.trimmingCharacters(in: .whitespacesAndNewlines) ?? code
    search = fields["search"]?.stringValue ?? search
    serviceURL =
      fields["url"]?.stringValue.trimmingCharacters(in: .whitespacesAndNewlines) ?? serviceURL
  }
  func prepareForNavigation() { capture() }
  private func perform(_ operation: @escaping () async throws -> Void) {
    capture()
    busy = true
    render()
    Task { @MainActor in
      do { try await operation() } catch { message = error.localizedDescription }
      busy = false
      render()
    }
  }
  private func service() throws -> AccountService {
    guard !preview, let configuration = try AccountWindow.configuration(store) else {
      throw AccountError.configuration
    }
    return AccountService(configuration: configuration, store: store)
  }

  private func fingerprint(_ data: LearningData) -> String {
    guard let account = data.active else { return "signed-out" }
    return account.sessionID.uuidString + "|" + String(data.pending[account.scope]?.count ?? 0)
      + data.visibleWords(account).map { $0.id + String($0.uses) + String($0.mastered) }.joined(
        separator: ";")
  }

  func refreshIfSignedIn() {
    guard !preview, !busy, (try? store.snapshot().active) != nil else { return }
    perform {
      try await self.service().sync()
      self.message = "已同步。"
    }
  }

  func render(preserveScroll: Bool = false) {
    let previousOffset = contentView?.contentView.bounds.origin ?? .zero
    fields = [:]
    let root = CandidateDocumentView()
    root.wantsLayer = true
    root.fill = NativeTheme.background
    let stack = column()
    root.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: root.topAnchor, constant: 16),
      stack.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -16),
    ])
    add(
      row(
        [
          NativeTheme.label(
            embedded ? ((try? store.snapshot().active) == nil ? "登录账号" : "我的学习") : "学习中心", size: 20,
            weight: .medium), NSView(),
        ] + (embedded ? [] : [AppearanceSettings.current.button()])), to: stack)
    if preview {
      add(NativeTheme.label("界面样例：使用隔离测试数据，没有真实登录或联网同步。", size: 12, secondary: true), to: stack)
    }
    do {
      let data = try store.snapshot()
      renderedFingerprint = fingerprint(data)
      if let account = data.active {
        dashboard(account, data: data, to: stack)
      } else {
        login(to: stack)
      }
    } catch { add(NativeTheme.label(error.localizedDescription, secondary: true), to: stack) }
    if !message.isEmpty { add(NativeTheme.label(message, size: 12, secondary: true), to: stack) }
    let scroll = NSScrollView()
    scroll.drawsBackground = false
    scroll.hasVerticalScroller = true
    scroll.documentView = root
    contentView = scroll
    if !embedded { window.contentView = scroll }
    root.translatesAutoresizingMaskIntoConstraints = false
    root.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor).isActive = true
    root.layoutSubtreeIfNeeded()
    root.setFrameSize(
      NSSize(
        width: window.contentLayoutRect.width,
        height: max(window.contentLayoutRect.height, stack.fittingSize.height + 32)))
    scroll.contentView.scroll(to: preserveScroll ? previousOffset : .zero)
    onContentChange?()
  }

  private func login(to stack: NSStackView) {
    if !embedded { add(NativeTheme.label("登录，留下你的学习进度", size: 16, weight: .medium), to: stack) }
    add(NativeTheme.label("用邮箱验证码登录。选用的英文词语加入已学习，掌握情况由你确认。", size: 13, secondary: true), to: stack)
    add(field("邮箱", key: "email", value: email), to: stack)
    let send = button("发送验证码", primary: true) { [weak self] in
      guard let self else { return }
      capture()
      guard AccountService.validEmail(email) else {
        message = "请输入完整邮箱地址。"
        render()
        return
      }
      perform {
        try await self.service().sendCode(email: self.email)
        self.codeRequested = true
        self.cooldown = Date().addingTimeInterval(60)
        self.message = "验证码已发送，请检查邮箱。"
      }
    }
    sendButton = send
    send.isEnabled =
      !busy && !preview && Date() >= cooldown && (try? AccountWindow.configuration(store)) != nil
    add(row([field("六位验证码", key: "code", value: code), send]), to: stack)
    let signIn = button(busy ? "正在处理…" : "验证并登录", primary: true) { [weak self] in
      guard let self else { return }
      perform {
        try await self.service().verify(email: self.email, code: self.code)
        self.code = ""
        self.message = "登录成功，正在同步学习记录。"
        try await self.service().sync()
        self.message = "已同步。"
      }
    }
    signIn.isEnabled = !busy && !preview && codeRequested
    add(
      row([
        signIn, NSView(),
        button("高级设置") { [weak self] in
          self?.capture()
          self?.configurationVisible.toggle()
          self?.render(preserveScroll: true)
        },
      ]), to: stack)
    if (try? AccountWindow.configuration(store)) == nil {
      add(NativeTheme.label("登录服务暂未开放，邮箱登录和云端学习记录尚不可用。", size: 12, secondary: true), to: stack)
    }
    if configurationVisible {
      add(field("账号服务 HTTPS 地址", key: "url", value: serviceURL), to: stack)
      add(
        button("保存服务地址", primary: true) { [weak self] in
          guard let self, !preview else { return }
          capture()
          do {
            guard let url = URL(string: serviceURL) else { throw AccountError.configuration }
            let configuration = AccountConfiguration(baseURL: url)
            try configuration.validate()
            try FileManager.default.createDirectory(
              at: store.directory, withIntermediateDirectories: true,
              attributes: [.posixPermissions: 0o700])
            let file = store.directory.appendingPathComponent("service.json")
            try JSONEncoder().encode(configuration).write(to: file, options: .atomic)
            try FileManager.default.setAttributes(
              [.posixPermissions: 0o600], ofItemAtPath: file.path)
            message = "服务地址已保存，可以发送验证码。"
            configurationVisible = false
            codeRequested = false
          } catch { message = error.localizedDescription }
          render()
        }, to: stack)
    }
    add(
      NativeTheme.label("登录后同步单词与掌握标记；不上传整句、拼音或输入正文。未登录仍能正常使用输入法。", size: 12, secondary: true),
      to: stack)
  }

  private func dashboard(_ account: LearningAccount, data: LearningData, to stack: NSStackView) {
    add(
      row([
        NativeTheme.label(account.email, size: 13), NSView(),
        button("退出登录") { [weak self] in
          guard let self else { return }
          if preview {
            try? store.transaction { $0.active = nil }
            message = "界面样例已退出，未操作真实账号。"
            render()
            return
          }
          perform {
            try await self.service().logout()
            self.codeRequested = false
            self.code = ""
            self.message = "本机已退出登录。"
          }
        },
      ]), to: stack)
    let words = data.visibleWords(account)
    let pending = data.pending[account.scope]?.count ?? 0
    add(
      NativeTheme.label(
        "已学习 \(words.count) · 已掌握 \(words.filter(\.mastered).count) · 待同步 \(pending)", size: 14,
        weight: .medium), to: stack)
    add(
      row([
        field("搜索英文或中文", key: "search", value: search),
        button("搜索") { [weak self] in
          self?.capture()
          self?.visibleLimit = 50
          self?.render()
        },
      ]), to: stack)
    add(
      row(
        ["全部", "待掌握", "已掌握"].map { choice in
          button((filter == choice ? "› " : "") + choice) { [weak self] in
            self?.capture()
            self?.filter = choice
            self?.visibleLimit = 50
            self?.render()
          }
        } + [
          NSView(),
          button(preview ? "同步（样例）" : (busy ? "同步中…" : "同步"), primary: true) { [weak self] in
            guard let self else { return }
            perform {
              try await self.service().sync()
              self.message = "已同步。"
            }
          },
        ]), to: stack)
    let matches = words.filter { word in
      (filter == "全部" || (filter == "已掌握" ? word.mastered : !word.mastered))
        && (search.isEmpty || word.english.localizedCaseInsensitiveContains(search)
          || word.chinese.contains(search))
    }
    if matches.isEmpty {
      add(
        NativeTheme.label(
          words.isEmpty ? "还没有学习记录。回到输入法，选择一个英文译词开始。" : "没有符合筛选的词语。", size: 13, secondary: true),
        to: stack)
    }
    for word in matches.prefix(visibleLimit) {
      let label = column(
        [
          NativeTheme.label(word.english, size: 16, weight: .medium),
          NativeTheme.label(
            "\(word.chinese) · \(word.pos) · 选用 \(word.uses) 次", size: 12, secondary: true),
        ], spacing: 4)
      let mastered = button(word.mastered ? "已掌握 · 撤销" : "标记掌握") { [weak self] in
        guard let self else { return }
        do {
          try store.append(
            .init(
              id: UUID(), english: word.english, chinese: word.chinese, pos: word.pos,
              kind: "mastered", mastered: !word.mastered), for: account)
          message = preview ? "界面样例已更新，未联网。" : "已保存掌握标记，待同步。"
          if !preview { AccountWindow.launch(syncOnly: true) }
        } catch { message = error.localizedDescription }
        render(preserveScroll: true)
      }
      add(row([label, NSView(), mastered]), to: stack)
    }
    if matches.count > visibleLimit {
      add(
        button("显示更多（还有 \(matches.count - visibleLimit) 个）") { [weak self] in
          self?.capture()
          self?.visibleLimit += 50
          self?.render(preserveScroll: true)
        }, to: stack)
    }
    add(
      NativeTheme.label("已学习表示选用过译词；已掌握由你手动确认。退出只退出本机，不删除云端记录。", size: 12, secondary: true),
      to: stack)
  }
  deinit { timer?.invalidate() }
  func windowWillClose(_ notification: Notification) {
    NSApplication.shared.stop(nil)
  }
  func windowDidBecomeKey(_ notification: Notification) {
    guard !busy, let data = try? store.snapshot(),
      fingerprint(data) != renderedFingerprint
    else { return }
    capture()
    render()
  }
}
