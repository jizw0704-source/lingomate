// 可点击的中英候选与译法展开；不激活窗口，也不打断宿主输入。
import AppKit

final class CandidatePanel: NSPanel {
  override var canBecomeKey: Bool { false }
  override var canBecomeMain: Bool { false }
  var onLearning: (() -> Void)?
  var onSettings: (() -> Void)?
  var onAISettings: (() -> Void)?
  var onChinese: ((Int) -> Void)?
  var onEnglish: ((Int, Int) -> Void)?
  var onPage: ((Int) -> Void)?
  var onExpand: ((Int) -> Void)?
  var onCollapse: (() -> Void)?
  var onMode: (() -> Void)?
  var onTypingMode: (() -> Void)?
  var onPunctuation: (() -> Void)?
  var onRetryTranslation: (() -> Void)?
  var onSetupTranslation: (() -> Void)?

  init() {
    super.init(
      contentRect: NSRect(x: 0, y: 0, width: 560, height: 360),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    isOpaque = false
    backgroundColor = .clear
    hasShadow = true
    hidesOnDeactivate = false
    becomesKeyOnlyIfNeeded = true
    title = "灵果候选"
    collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    level = .popUpMenu
    setAccessibilityLabel("灵果候选")
  }

  private func row(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.alignment = .centerY
    stack.spacing = 8
    return stack
  }

  private func column(_ views: [NSView], spacing: CGFloat = 4) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = spacing
    return stack
  }

  private func add(_ view: NSView, to stack: NSStackView, full: Bool = true) {
    stack.addArrangedSubview(view)
    if full { view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true }
  }

  private func divider() -> NSView {
    let view = CandidateDocumentView()
    view.wantsLayer = true
    view.fill = NativeTheme.divider
    view.heightAnchor.constraint(equalToConstant: 1).isActive = true
    return view
  }

  private func surface() -> NSView {
    let view = CandidateDocumentView()
    view.wantsLayer = true
    view.fill = NativeTheme.background
    view.layer?.cornerRadius = 16
    view.layer?.borderWidth = 1
    view.border = NativeTheme.divider
    return view
  }

  private func fittedButton(
    _ title: String, width: CGFloat, size: CGFloat = 14,
    label: String? = nil, action: @escaping () -> Void
  ) -> ActionButton {
    let button = ActionButton(title, label: label, action: action)
    button.font = NativeTheme.font(size)
    button.alignment = .left
    button.wrapsTitle = true
    button.toolTip = title
    let height = (title as NSString).boundingRect(
      with: NSSize(width: max(1, width - 24), height: .greatestFiniteMagnitude),
      options: [.usesLineFragmentOrigin, .usesFontLeading],
      attributes: [.font: button.font!]
    ).height
    button.widthAnchor.constraint(equalToConstant: width).isActive = true
    button.heightAnchor.constraint(equalToConstant: max(44, ceil(height) + 16)).isActive = true
    return button
  }

  func show(
    _ state: SessionState, bilingual: Bool, anchor: NSRect,
    punctuationLabel: String = "标点：中文 · 自动", maximumWidth: CGFloat = 560,
    maximumHeight: CGFloat? = nil
  ) {
    let bounds = screenBounds(anchor)
    let inline = state.visibleIndices.allSatisfy {
      (state.frame?.candidates[$0].text.count ?? 0) <= 8
    }
    let wantedWidth = inline ? inlineWidth(state, bilingual: bilingual) : maximumWidth
    let width = min(maximumWidth, wantedWidth, bounds.width - 16)
    let bodyWidth = width - 24
    let narrow = width < 500
    let content = surface()
    let stack = column([], spacing: 4)
    stack.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
      stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -12),
      stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 12),
      stack.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -12),
    ])
    let pinyin = NativeTheme.label(state.markedInput, size: 14, weight: .medium)
    pinyin.maximumNumberOfLines = 1
    pinyin.lineBreakMode = .byTruncatingTail
    pinyin.toolTip = pinyin.stringValue
    pinyin.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    let settings = ActionButton("⚙", label: "打开设置：登录、学习、翻译、输入与外观") { [weak self] in
      self?.onSettings?()
    }
    settings.widthAnchor.constraint(equalToConstant: 44).isActive = true
    settings.toolTip = "设置 · 中文/英文切换 · 双语 · 标点 · 外观 · 学习"
    let details = ActionButton(state.expanded ? "收起" : "译法", label: "选择候选并展开其他译法，Tab") {}
    details.widthAnchor.constraint(equalToConstant: 44).isActive = true
    details.font = NativeTheme.font(12)
    details.horizontalPadding = 8
    details.isEnabled =
      bilingual
      && state.visibleIndices.contains {
        state.frame?.candidates[$0].translations.isEmpty == false
      }
    details.invoke = { [weak self, weak details] in
      guard let self, let details else { return }
      if state.expanded {
        self.onCollapse?()
        return
      }
      let menu = NSMenu(title: "更多译法")
      for index in state.visibleIndices {
        guard let candidate = state.frame?.candidates[index], !candidate.translations.isEmpty else {
          continue
        }
        let item = NSMenuItem(
          title: "\(index - state.visibleIndices.lowerBound + 1)  \(candidate.text)",
          action: #selector(self.expandMenuItem(_:)), keyEquivalent: "")
        item.target = self
        item.tag = index
        menu.addItem(item)
      }
      menu.popUp(positioning: nil, at: NSPoint(x: 0, y: details.bounds.minY), in: details)
    }
    let previous = ActionButton("‹", label: "上一页候选，PageUp 或减号") { [weak self] in
      self?.onPage?(-1)
    }
    let next = ActionButton("›", label: "下一页候选，PageDown 或等号") { [weak self] in
      self?.onPage?(1)
    }
    for button in [previous, next] {
      button.widthAnchor.constraint(equalToConstant: 44).isActive = true
      button.font = NativeTheme.font(20)
    }
    previous.isEnabled = state.page > 0
    next.isEnabled = state.page + 1 < state.pageCount
    let page = NativeTheme.label(
      state.pageCount > 0 ? "\(state.page + 1)/\(state.pageCount)" : "0/0",
      size: 11, secondary: true)
    page.setContentCompressionResistancePriority(.required, for: .horizontal)
    page.toolTip = "共 \(state.frame?.candidates.count ?? 0) 个候选 · −/= 翻页"
    let mode = NativeTheme.label(bilingual ? "中英" : "中文", size: 11, secondary: true)
    mode.toolTip = "空格 中文 · 回车 原样字母 · Shift＋空格 译文 · Shift 英文直输 · \(punctuationLabel)"
    let header = row([pinyin, NSView(), mode, previous, page, next, details, settings])
    header.spacing = 0
    add(header, to: stack)
    add(divider(), to: stack)
    if inline {
      addInlineCandidates(state, bilingual: bilingual, to: stack, width: bodyWidth)
      if let candidate = state.candidate {
        if state.expanded && bilingual && !candidate.translations.isEmpty {
          addDetails(state, candidate: candidate, to: stack, width: bodyWidth)
        }
        if bilingual && state.sentence != .none {
          addSentence(state, candidate: candidate, to: stack)
        }
      }
    } else {
      for index in state.visibleIndices {
        guard let candidate = state.frame?.candidates[index] else { continue }
        let number = NativeTheme.label(
          "\(index == state.active ? "›" : "")\(index - state.visibleIndices.lowerBound + 1)",
          size: 13, secondary: index != state.active, weight: .medium)
        number.widthAnchor.constraint(equalToConstant: 24).isActive = true
        let fullChinese = !bilingual || narrow || candidate.translations.isEmpty
        let chineseWidth = fullChinese ? bodyWidth - 32 : 128
        let chinese = fittedButton(
          candidate.text, width: chineseWidth, size: 16,
          label: "输出中文 \(candidate.text)"
        ) { [weak self] in self?.onChinese?(index) }
        chinese.font = NativeTheme.font(16, weight: .medium)
        let chineseGroup = row([chinese])
        let chineseRow = row([number, chineseGroup])
        var candidateRow: NSView
        if bilingual && !candidate.translations.isEmpty {
          let englishWidth = fullChinese ? bodyWidth - 32 : bodyWidth - 168
          var englishViews: [NSView] = []
          let senses = Array(candidate.translations.prefix(2))
          let senseWidth =
            (englishWidth - 52 - 8 - CGFloat(senses.count - 1) * 8) / CGFloat(senses.count)
          for (senseIndex, sense) in senses.enumerated() {
            let english = ActionButton(sense.word, label: "输出英文 \(sense.word)") { [weak self] in
              self?.onEnglish?(index, senseIndex)
            }
            english.style = .accent
            english.alignment = .left
            english.widthAnchor.constraint(equalToConstant: senseWidth).isActive = true
            english.cell?.lineBreakMode = .byTruncatingTail
            english.toolTip = sense.word
            englishViews.append(english)
          }
          let more = ActionButton("更多", label: "展开 \(candidate.text) 的译法与用法") { [weak self] in
            self?.onExpand?(index)
          }
          more.font = NativeTheme.font(12)
          more.contentTintColor = NativeTheme.muted
          more.widthAnchor.constraint(equalToConstant: 52).isActive = true
          englishViews.append(more)
          let englishRow = row(englishViews)
          if narrow {
            let indent = NSView()
            indent.widthAnchor.constraint(equalToConstant: 24).isActive = true
            candidateRow = column([chineseRow, row([indent, englishRow])], spacing: 0)
          } else {
            candidateRow = row([chineseRow, englishRow])
          }
        } else if bilingual && index == state.active && state.sentence == .none {
          let status =
            candidate.text.count >= 3 || (candidate.personal == true && candidate.text.count >= 2)
            ? (index == state.active ? "当前候选的整句译文见下方" : "选中后可翻译整句") : "暂无英文译词"
          let indent = NSView()
          indent.widthAnchor.constraint(equalToConstant: 24).isActive = true
          candidateRow = column(
            [
              chineseRow, row([indent, NativeTheme.label(status, size: 12, secondary: true)]),
            ], spacing: 0)
        } else {
          candidateRow = chineseRow
        }
        candidateRow.wantsLayer = true
        if index == state.active {
          let highlight = CandidateDocumentView()
          highlight.wantsLayer = true
          highlight.fill = NativeTheme.surface
          highlight.layer?.cornerRadius = 10
          candidateRow.translatesAutoresizingMaskIntoConstraints = false
          highlight.addSubview(candidateRow)
          NSLayoutConstraint.activate([
            candidateRow.leadingAnchor.constraint(equalTo: highlight.leadingAnchor),
            candidateRow.trailingAnchor.constraint(equalTo: highlight.trailingAnchor),
            candidateRow.topAnchor.constraint(equalTo: highlight.topAnchor),
            candidateRow.bottomAnchor.constraint(equalTo: highlight.bottomAnchor),
          ])
          candidateRow = highlight
        }
        add(candidateRow, to: stack)
        if state.expanded, bilingual, index == state.active, !candidate.translations.isEmpty {
          addDetails(state, candidate: candidate, to: stack, width: bodyWidth)
          add(divider(), to: stack)
        }
        if bilingual, index == state.active, state.sentence != .none {
          addSentence(state, candidate: candidate, to: stack)
          add(divider(), to: stack)
        }
      }
    }
    if state.frame?.candidates.isEmpty != false {
      add(NativeTheme.label("暂未找到候选\n继续输入，或按回车输出原样字母。", secondary: true), to: stack)
    }
    stack.toolTip =
      bilingual
      ? "1–5 / 空格 选中文 · Shift＋空格 英文 · Tab 译法 · − / = 翻页"
      : "1–5 / 空格 选中文 · − / = 翻页 · F6 双语"
    present(
      content, stack: stack, width: width, anchor: anchor, bounds: bounds,
      maximumHeight: maximumHeight)
  }

  @objc private func expandMenuItem(_ item: NSMenuItem) {
    onExpand?(item.tag)
  }

  private func inlineWidth(_ state: SessionState, bilingual: Bool) -> CGFloat {
    let widths = state.visibleIndices.compactMap { index -> CGFloat? in
      guard let candidate = state.frame?.candidates[index] else { return nil }
      let title = "\(index - state.visibleIndices.lowerBound + 1) \(candidate.text)"
      let chinese =
        (title as NSString).size(withAttributes: [.font: NativeTheme.font(15)]).width + 24
      let english =
        bilingual
        ? ((candidate.translations.first?.word ?? "") as NSString)
          .size(withAttributes: [.font: NativeTheme.font(13)]).width + 24 : 0
      return min(140, max(44, chinese, english))
    }
    return max(400, ceil(widths.reduce(0, +)) + CGFloat(max(0, widths.count - 1)) * 4 + 24)
  }

  private func addInlineCandidates(
    _ state: SessionState, bilingual: Bool, to stack: NSStackView, width: CGFloat
  ) {
    guard !state.visibleIndices.isEmpty else { return }
    let indices = Array(state.visibleIndices)
    let columnWidth = (width - CGFloat(indices.count - 1) * 4) / CGFloat(indices.count)
    let hasEnglish =
      bilingual
      && indices.contains {
        state.frame?.candidates[$0].translations.isEmpty == false
      }
    let candidates = row([])
    candidates.spacing = 4
    candidates.alignment = .top
    for index in indices {
      guard let candidate = state.frame?.candidates[index] else { continue }
      let slot = index - state.visibleIndices.lowerBound + 1
      let chinese = fittedButton(
        "\(slot) \(candidate.text)",
        width: columnWidth, size: 15,
        label: "输出中文 \(candidate.text)"
      ) { [weak self] in self?.onChinese?(index) }
      chinese.toolTip = candidate.text
      if index == state.active { chinese.style = .subtle }
      let cell = column([chinese], spacing: 0)
      cell.widthAnchor.constraint(equalToConstant: columnWidth).isActive = true
      if hasEnglish {
        if let sense = candidate.translations.first {
          let english = ActionButton(sense.word, label: "输出英文 \(sense.word)") { [weak self] in
            self?.onEnglish?(index, 0)
          }
          english.font = NativeTheme.font(13)
          english.style = .accent
          english.alignment = .left
          english.widthAnchor.constraint(equalToConstant: columnWidth).isActive = true
          english.toolTip = "\(sense.word) · 更多译法请点顶部‘译法’或按 Tab"
          cell.addArrangedSubview(english)
        } else {
          let missing = NativeTheme.label("—", size: 12, secondary: true)
          missing.heightAnchor.constraint(equalToConstant: 44).isActive = true
          missing.toolTip = "此候选暂无词语译法；中文仍可选择"
          cell.addArrangedSubview(missing)
        }
      }
      candidates.addArrangedSubview(cell)
    }
    add(candidates, to: stack)
  }

  private func addSentence(_ state: SessionState, candidate: EngineCandidate, to stack: NSStackView)
  {
    add(divider(), to: stack)
    let translationLabel: String
    if case .aiFailed = state.sentence {
      translationLabel = "AI 在线翻译"
    } else {
      translationLabel = AISettings.label
    }
    add(
      row([
        NativeTheme.label("整句译文", size: 16, weight: .medium), NSView(),
        ActionButton(translationLabel, label: "打开翻译设置，选择本机或 AI 在线翻译") { [weak self] in
          self?.onAISettings?()
        },
      ]), to: stack)
    let status: String
    var ready = false
    switch state.sentence {
    case .ready(_, let english):
      status = english
      ready = true
    case .loading: status = "正在翻译…你仍可先选择中文。"
    case .missingModels: status = "首次使用需准备中英文语言，下载后可在本机翻译。"
    case .unavailable: status = "整句翻译需要 macOS 26 或以上，词语译法仍可使用。"
    case .failed: status = "暂未完成翻译，请重试；中文仍可选择。"
    case .aiFailed(let error): status = error.localizedDescription
    case .none: status = ""
    }
    add(NativeTheme.label(status, size: 15, secondary: !ready), to: stack)
    let english = ActionButton("选英文", label: "输出整句英文，Shift＋空格") { [weak self] in
      self?.onEnglish?(state.active, 0)
    }
    english.isEnabled = ready
    english.style = .outlined
    let chinese = ActionButton("选中文", label: "输出中文 \(candidate.text)，空格") { [weak self] in
      self?.onChinese?(state.active)
    }
    let retry = ActionButton(state.sentence == .missingModels ? "准备语言" : "重试") { [weak self] in
      if state.sentence == .missingModels {
        self?.onSetupTranslation?()
      } else {
        self?.onRetryTranslation?()
      }
    }
    retry.isEnabled = state.sentence != .loading && state.sentence != .unavailable
    add(row([english, chinese, NSView(), retry]), to: stack)
    statusLabelTooltip(stack, text: "译文对应当前中文候选；换候选会重新翻译。")
  }

  private func statusLabelTooltip(_ stack: NSStackView, text: String) {
    stack.toolTip = text
  }

  private func addDetails(
    _ state: SessionState, candidate: EngineCandidate, to stack: NSStackView, width: CGFloat
  ) {
    add(divider(), to: stack)
    let close = ActionButton("收起", label: "收起译法，Esc") { [weak self] in self?.onCollapse?() }
    add(
      row([
        NativeTheme.label("\(candidate.text) · 更多译法", size: 16, weight: .medium), NSView(), close,
      ]), to: stack)
    for (senseIndex, sense) in candidate.translations.enumerated() {
      let prefix = senseIndex == state.sense ? "› " : ""
      let title = "\(prefix)\(sense.word)\(sense.pos.isEmpty ? "" : " · \(sense.pos)")"
      let select = fittedButton(
        title, width: width, size: 16,
        label:
          "输出英文 \(sense.word)，第 \(senseIndex + 1) 种译法\(senseIndex == state.sense ? "，当前选中" : "")"
      ) { [weak self] in self?.onEnglish?(state.active, senseIndex) }
      select.style = .accent
      add(select, to: stack)
      if !sense.note.isEmpty { add(NativeTheme.label(sense.note, secondary: true), to: stack) }
      if !sense.example.isEmpty {
        add(NativeTheme.label("例句 · \(sense.example)", secondary: true), to: stack)
      }
    }
    add(
      NativeTheme.label(
        candidate.hasDetails
          ? "用法为原型示例 · Tab 换译法 · Shift＋空格 选英文"
          : "本地词表释义，尚未提供完整用法。", size: 11, secondary: true), to: stack)
  }

  private func screenBounds(_ anchor: NSRect) -> NSRect {
    (NSScreen.screens.first { $0.frame.intersects(anchor) } ?? NSScreen.main)?.visibleFrame
      ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
  }

  private func present(
    _ content: NSView, stack: NSStackView, width: CGFloat, anchor: NSRect, bounds: NSRect,
    maximumHeight: CGFloat?
  ) {
    content.setFrameSize(NSSize(width: width, height: 400))
    content.layoutSubtreeIfNeeded()
    let height = ceil(stack.fittingSize.height) + 24
    let actualHeight = min(height, maximumHeight ?? bounds.height - 16)
    content.setFrameSize(NSSize(width: width, height: height))
    let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: width, height: actualHeight))
    scroll.drawsBackground = false
    scroll.hasVerticalScroller = height > actualHeight
    scroll.autohidesScrollers = true
    scroll.scrollerStyle = .overlay
    scroll.autoresizingMask = [.width, .height]
    scroll.documentView = content
    scroll.contentView.scroll(to: .zero)
    // 圆角和边框属于固定面板，不随文档滚动消失。
    content.layer?.cornerRadius = 0
    content.layer?.borderWidth = 0
    let frame = surface()
    frame.frame = scroll.frame
    frame.layer?.masksToBounds = true
    frame.addSubview(scroll)
    contentView = frame
    let x = min(max(anchor.minX, bounds.minX + 8), bounds.maxX - width - 8)
    let below = anchor.minY - actualHeight - 8
    let y = below >= bounds.minY + 8 ? below : min(anchor.maxY + 8, bounds.maxY - actualHeight - 8)
    setFrame(
      NSRect(x: x, y: max(bounds.minY + 8, y), width: width, height: actualHeight), display: true)
    orderFrontRegardless()
  }

  func showTypingMode(_ mode: TypingMode, anchor: NSRect) {
    let content = surface()
    let symbol = NativeTheme.label(mode == .english ? "EN" : "中", size: 20, weight: .medium)
    symbol.widthAnchor.constraint(equalToConstant: 32).isActive = true
    let title = column([
      NativeTheme.label(mode.title, size: 16, weight: .medium),
      NativeTheme.label("单按 Shift 随时切换", size: 11, secondary: true),
    ])
    let toggle = ActionButton(mode == .english ? "切回中文" : "切到英文") { [weak self] in
      self?.onTypingMode?()
    }
    toggle.style = .subtle
    let views = row([symbol, title, NSView(), toggle])
    views.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(views)
    NSLayoutConstraint.activate([
      views.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
      views.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -12),
      views.topAnchor.constraint(equalTo: content.topAnchor, constant: 12),
      views.heightAnchor.constraint(equalToConstant: 44),
    ])
    contentView = content
    let bounds = screenBounds(anchor)
    let x = min(max(anchor.minX, bounds.minX + 8), bounds.maxX - 368)
    let y = min(max(anchor.minY - 76, bounds.minY + 8), bounds.maxY - 76)
    setFrame(NSRect(x: x, y: y, width: 360, height: 68), display: true)
    orderFrontRegardless()
  }
}
