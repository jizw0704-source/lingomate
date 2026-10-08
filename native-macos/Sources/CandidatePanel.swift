// 可点击的中英候选与译法展开；不激活窗口，也不打断宿主输入。
import AppKit

final class CandidatePanel: NSPanel {
  override var canBecomeKey: Bool { false }
  override var canBecomeMain: Bool { false }
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
      contentRect: NSRect(x: 0, y: 0, width: 600, height: 360),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    isOpaque = false
    backgroundColor = .clear
    hasShadow = true
    hidesOnDeactivate = false
    becomesKeyOnlyIfNeeded = true
    appearance = NSAppearance(named: .aqua)
    title = "中英输入实验版候选"
    collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    level = .popUpMenu
    setAccessibilityLabel("中英输入实验版候选")
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
    let view = NSView()
    view.wantsLayer = true
    view.layer?.backgroundColor = NativeTheme.divider.cgColor
    view.heightAnchor.constraint(equalToConstant: 1).isActive = true
    return view
  }

  private func surface() -> NSView {
    let view = CandidateDocumentView()
    view.wantsLayer = true
    view.layer?.backgroundColor = NSColor.white.cgColor
    view.layer?.cornerRadius = 16
    view.layer?.borderWidth = 1
    view.layer?.borderColor = NativeTheme.divider.cgColor
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
    punctuationLabel: String = "标点：中文 · 自动", maximumWidth: CGFloat = 600,
    maximumHeight: CGFloat? = nil
  ) {
    let bounds = screenBounds(anchor)
    let width = min(maximumWidth, bounds.width - 16)
    let bodyWidth = width - 32
    let narrow = width < 520
    let hasCandidates = state.frame?.candidates.isEmpty == false
    let content = surface()
    let stack = column([], spacing: 8)
    stack.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
      stack.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16),
    ])
    let heading = row([
      NativeTheme.label("正在输入", size: 11, secondary: true), NSView(),
      NativeTheme.label(bilingual ? "中英候选" : "中文候选", size: 11, secondary: true),
    ])
    add(heading, to: stack)
    let pinyin = NativeTheme.label(state.frame?.marked ?? state.input, size: 20, weight: .medium)
    pinyin.maximumNumberOfLines = 2
    pinyin.lineBreakMode = .byTruncatingTail
    pinyin.toolTip = pinyin.stringValue
    add(pinyin, to: stack)
    let typing = ActionButton("中文 · Shift", label: "中文拼音，切换到英文直输，单按 Shift") {
      [weak self] in self?.onTypingMode?()
    }
    let mode = ActionButton(bilingual ? "双语 · F6" : "仅中文 · F6", label: "切换中英候选显示，F6") {
      [weak self] in self?.onMode?()
    }
    mode.toolTip = "F6 切换普通中文与中英候选；Shift 切换英文直输"
    let shortPunctuation =
      punctuationLabel
      .replacingOccurrences(of: "中文", with: "中")
      .replacingOccurrences(of: "英文", with: "英")
    let punctuation = ActionButton(
      shortPunctuation, label: "\(punctuationLabel)，切换标点模式，Control Shift P"
    ) { [weak self] in self?.onPunctuation?() }
    punctuation.toolTip = "自动 → 固定中文 → 固定英文；Control＋Shift＋P 切换"
    for button in [typing, mode, punctuation] {
      button.style = .subtle
      button.font = NativeTheme.font(12)
    }
    add(row([typing, mode, NSView(), punctuation]), to: stack)
    add(divider(), to: stack)
    if !narrow && bilingual && hasCandidates {
      let inset = NSView()
      inset.widthAnchor.constraint(equalToConstant: 36).isActive = true
      let chineseTitle = NativeTheme.label("中文 · 空格", size: 11, secondary: true)
      chineseTitle.widthAnchor.constraint(equalToConstant: 148).isActive = true
      add(
        row([inset, chineseTitle, NativeTheme.label("英文 · Shift＋空格", size: 11, secondary: true)]),
        to: stack)
    }
    for index in state.visibleIndices {
      guard let candidate = state.frame?.candidates[index] else { continue }
      let number = NativeTheme.label(
        "\(index == state.active ? "›" : "")\(index - state.visibleIndices.lowerBound + 1)",
        size: 13, secondary: index != state.active, weight: .medium)
      number.widthAnchor.constraint(equalToConstant: 24).isActive = true
      let fullChinese = !bilingual || narrow || candidate.translations.isEmpty
      let chineseWidth = fullChinese ? bodyWidth - 32 : 148
      let chinese = fittedButton(
        candidate.text, width: chineseWidth, size: 18,
        label: "输出中文 \(candidate.text)\(candidate.personal == true ? "，个人词库" : "")"
      ) { [weak self] in self?.onChinese?(index) }
      chinese.font = NativeTheme.font(18, weight: .medium)
      let chineseGroup = column([chinese])
      if candidate.personal == true {
        add(NativeTheme.label("记忆", size: 11, secondary: true), to: chineseGroup)
      }
      let chineseRow = row([number, chineseGroup])
      let candidateRow: NSStackView
      if bilingual && !candidate.translations.isEmpty {
        let englishWidth = fullChinese ? bodyWidth - 32 : bodyWidth - 188
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
      } else if bilingual {
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
        candidateRow.layer?.backgroundColor = NativeTheme.surface.cgColor
        candidateRow.layer?.cornerRadius = 10
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
    if state.frame?.candidates.isEmpty != false {
      add(NativeTheme.label("暂未找到候选\n继续输入，或按 Enter 保留原样拼音。", secondary: true), to: stack)
    }
    add(divider(), to: stack)
    if state.pageCount > 0 {
      let previous = ActionButton("上一页", label: "上一页候选，PageUp 或减号") { [weak self] in
        self?.onPage?(-1)
      }
      let next = ActionButton("下一页", label: "下一页候选，PageDown 或等号") { [weak self] in self?.onPage?(1)
      }
      previous.isEnabled = state.page > 0
      next.isEnabled = state.page + 1 < state.pageCount
      previous.style = .subtle
      next.style = .subtle
      let count = state.frame?.candidates.count ?? 0
      let leading = NSView()
      let trailing = NSView()
      let paging = row([
        previous, leading,
        NativeTheme.label(
          "\(state.page + 1) / \(state.pageCount) 页 · \(count) 项", size: 12, secondary: true),
        trailing, next,
      ])
      leading.widthAnchor.constraint(equalTo: trailing.widthAnchor).isActive = true
      add(paging, to: stack)
    }
    if hasCandidates {
      add(
        NativeTheme.label(
          bilingual
            ? "空格 选中文 · Shift＋空格 选英文 · ↑↓ 换候选 · Tab 更多译法"
            : "空格 选中文 · ↑↓ 换候选 · F6 中英候选",
          size: 11, secondary: true), to: stack)
      add(
        NativeTheme.label("1–5 选本页中文 · PageUp / PageDown 或 − / = 翻页", size: 11, secondary: true),
        to: stack)
    } else {
      add(NativeTheme.label("Esc 取消 · Shift 切换英文直输", size: 11, secondary: true), to: stack)
    }
    present(
      content, stack: stack, width: width, anchor: anchor, bounds: bounds,
      maximumHeight: maximumHeight)
  }

  private func addSentence(_ state: SessionState, candidate: EngineCandidate, to stack: NSStackView)
  {
    add(divider(), to: stack)
    add(
      row([
        NativeTheme.label("整句译文", size: 16, weight: .medium), NSView(),
        NativeTheme.label("本机翻译", size: 11, secondary: true),
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
    add(NativeTheme.label("译文对应当前中文候选；换候选会重新翻译。", size: 11, secondary: true), to: stack)
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
    let height = ceil(stack.fittingSize.height) + 32
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
      views.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
      views.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
      views.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
      views.heightAnchor.constraint(equalToConstant: 44),
    ])
    contentView = content
    let bounds = screenBounds(anchor)
    let x = min(max(anchor.minX, bounds.minX + 8), bounds.maxX - 408)
    let y = min(max(anchor.minY - 84, bounds.minY + 8), bounds.maxY - 84)
    setFrame(NSRect(x: x, y: y, width: 400, height: 76), display: true)
    orderFrontRegardless()
  }
}
