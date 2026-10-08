// 可点击的候选和页内译法展开；没有动画，不抢走当前应用的输入焦点。
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
  private let ink = NSColor(srgbRed: 16 / 255, green: 16 / 255, blue: 16 / 255, alpha: 1)
  private let muted = NSColor(srgbRed: 74 / 255, green: 74 / 255, blue: 74 / 255, alpha: 1)
  private let green = NSColor(srgbRed: 86 / 255, green: 119 / 255, blue: 0, alpha: 1)

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

  private func label(_ value: String, size: CGFloat = 13, secondary: Bool = false) -> NSTextField {
    let field = NSTextField(wrappingLabelWithString: value)
    field.font = NSFont(name: "MiSans", size: size) ?? .systemFont(ofSize: size)
    field.textColor = secondary ? muted : ink
    field.translatesAutoresizingMaskIntoConstraints = false
    return field
  }

  private func row(_ views: [NSView]) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.alignment = .centerY
    stack.spacing = 8
    return stack
  }

  private func divider() -> NSView {
    let view = NSView()
    view.wantsLayer = true
    view.layer?.backgroundColor =
      NSColor(srgbRed: 228 / 255, green: 228 / 255, blue: 228 / 255, alpha: 1).cgColor
    view.heightAnchor.constraint(equalToConstant: 1).isActive = true
    return view
  }

  func show(
    _ state: SessionState, bilingual: Bool, anchor: NSRect,
    punctuationLabel: String = "标点：中文 · 自动"
  ) {
    let width: CGFloat = 600
    let content = NSView()
    content.wantsLayer = true
    content.layer?.backgroundColor = NSColor.white.cgColor
    content.layer?.cornerRadius = 16
    content.layer?.borderWidth = 1
    content.layer?.borderColor =
      NSColor(srgbRed: 228 / 255, green: 228 / 255, blue: 228 / 255, alpha: 1).cgColor
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.distribution = .fill
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
      stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
      stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
      stack.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16),
    ])
    let mode = ActionButton(bilingual ? "中英候选 · F6" : "普通中文 · F6", label: "切换中英候选显示") {
      [weak self] in self?.onMode?()
    }
    let typing = ActionButton("中文拼音 · Shift", label: "切换到英文直输，单按 Shift") {
      [weak self] in self?.onTypingMode?()
    }
    mode.contentTintColor = green
    let punctuation = ActionButton(
      punctuationLabel, label: "\(punctuationLabel)，切换标点模式，Control Shift P"
    ) {
      [weak self] in self?.onPunctuation?()
    }
    punctuation.toolTip = "自动 → 固定中文 → 固定英文；Control＋Shift＋P 切换"
    let head = row([
      label(state.frame?.marked ?? state.input, size: 18), NSView(), typing, punctuation, mode,
    ])
    stack.addArrangedSubview(head)
    head.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    let line = divider()
    stack.addArrangedSubview(line)
    line.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    for index in state.visibleIndices {
      guard let candidate = state.frame?.candidates[index] else { continue }
      let number = index - state.visibleIndices.lowerBound + 1
      let chinese = ActionButton(
        "\(index == state.active ? "›" : " ") \(number)   \(candidate.text)\(candidate.personal == true ? " · 记忆" : "")",
        label: "输出中文 \(candidate.text)\(candidate.personal == true ? "，个人词库" : "")"
      ) { [weak self] in self?.onChinese?(index) }
      chinese.alignment = .left
      chinese.font = NSFont(name: "MiSans", size: 16) ?? .systemFont(ofSize: 16, weight: .medium)
      chinese.widthAnchor.constraint(
        equalToConstant: !bilingual || candidate.translations.isEmpty ? 300 : 152
      ).isActive = true
      chinese.cell?.lineBreakMode = .byTruncatingTail
      chinese.toolTip = candidate.text
      var views: [NSView] = [chinese]
      if bilingual {
        for (senseIndex, sense) in candidate.translations.prefix(2).enumerated() {
          let english = ActionButton(sense.word, label: "输出英文 \(sense.word)") { [weak self] in
            self?.onEnglish?(index, senseIndex)
          }
          let textWidth = (sense.word as NSString).size(withAttributes: [.font: english.font!])
            .width
          english.widthAnchor.constraint(equalToConstant: min(152, max(44, textWidth + 16)))
            .isActive = true
          english.cell?.lineBreakMode = .byTruncatingTail
          english.toolTip = sense.word
          views.append(english)
        }
        if candidate.translations.isEmpty {
          views.append(
            label(
              candidate.text.count >= 3 || (candidate.personal == true && candidate.text.count >= 2)
                ? (index == state.active ? "整句译文见下方" : "选中后翻译") : "暂无译词",
              secondary: true))
        }
        views.append(NSView())
        let more = ActionButton("展开", label: "展开 \(candidate.text) 的译法") { [weak self] in
          self?.onExpand?(index)
        }
        more.isEnabled = !candidate.translations.isEmpty
        views.append(more)
      } else {
        views.append(NSView())
      }
      let candidateRow = row(views)
      candidateRow.wantsLayer = true
      if index == state.active {
        candidateRow.layer?.backgroundColor =
          NSColor(srgbRed: 247 / 255, green: 247 / 255, blue: 247 / 255, alpha: 1).cgColor
        candidateRow.layer?.cornerRadius = 10
      }
      stack.addArrangedSubview(candidateRow)
      candidateRow.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }
    if state.frame?.candidates.isEmpty != false {
      stack.addArrangedSubview(label("暂无中文候选，按 Enter 输出原样拼音。", secondary: true))
    }
    if state.pageCount > 0 {
      let previous = ActionButton("上一页", label: "上一页候选，PageUp 或减号") { [weak self] in
        self?.onPage?(-1)
      }
      let next = ActionButton("下一页", label: "下一页候选，PageDown 或等号") { [weak self] in self?.onPage?(1)
      }
      previous.isEnabled = state.page > 0
      next.isEnabled = state.page + 1 < state.pageCount
      let count = state.frame?.candidates.count ?? 0
      let paging = row([
        previous, NSView(),
        label("第 \(state.page + 1) / \(state.pageCount) 页 · 共 \(count) 项", secondary: true),
        NSView(), next,
      ])
      stack.addArrangedSubview(paging)
      paging.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }
    if bilingual, state.sentence != .none, let candidate = state.candidate {
      let line = divider()
      stack.addArrangedSubview(line)
      line.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      stack.addArrangedSubview(label("整句翻译 · 当前候选", size: 16))
      let chinese = label(candidate.text, size: 14)
      stack.addArrangedSubview(chinese)
      chinese.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      let status: String
      var ready = false
      switch state.sentence {
      case .ready(_, let english):
        status = english
        ready = true
      case .loading: status = "正在本机翻译…中文仍可直接选择。"
      case .missingModels: status = "中英文翻译语言尚未准备。首次下载后，可在本机翻译整句。"
      case .unavailable: status = "本地整句翻译需要 macOS 26 或以上。中文和词语译词仍可使用。"
      case .failed: status = "整句翻译暂未完成，请重试。中文仍可正常选择。"
      case .none: status = ""
      }
      let translated = label(status, size: 14, secondary: !ready)
      stack.addArrangedSubview(translated)
      translated.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      translated.heightAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
      let english = ActionButton("输出英文 · Shift＋空格") { [weak self] in
        self?.onEnglish?(state.active, 0)
      }
      english.isEnabled = ready
      english.contentTintColor = green
      let chineseButton = ActionButton("输出中文 · 空格") { [weak self] in self?.onChinese?(state.active)
      }
      let retry = ActionButton(state.sentence == .missingModels ? "准备翻译语言" : "重新翻译") {
        [weak self] in
        if state.sentence == .missingModels {
          self?.onSetupTranslation?()
        } else {
          self?.onRetryTranslation?()
        }
      }
      retry.isEnabled = state.sentence != .loading && state.sentence != .unavailable
      let actions = row([english, chineseButton, NSView(), retry])
      stack.addArrangedSubview(actions)
      actions.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      stack.addArrangedSubview(label("本机翻译当前选中的中文；换候选会重新翻译。", size: 11, secondary: true))
    }
    if state.expanded, bilingual, let candidate = state.candidate,
      !candidate.translations.isEmpty
    {
      let separator = divider()
      stack.addArrangedSubview(separator)
      separator.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      let close = ActionButton("收起 · Esc") { [weak self] in self?.onCollapse?() }
      let title = row([label("\(candidate.text) · 译法与用法", size: 16), NSView(), close])
      stack.addArrangedSubview(title)
      title.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
      for (senseIndex, sense) in candidate.translations.enumerated() {
        let select = ActionButton(
          "\(sense.pos) \(sense.word)", label: "输出英文 \(sense.word)，第 \(senseIndex + 1) 种译法"
        ) { [weak self] in self?.onEnglish?(state.active, senseIndex) }
        select.alignment = .left
        select.contentTintColor = senseIndex == state.sense ? green : ink
        stack.addArrangedSubview(select)
        if !sense.note.isEmpty { stack.addArrangedSubview(label(sense.note, secondary: true)) }
        if !sense.example.isEmpty {
          stack.addArrangedSubview(label(sense.example, secondary: true))
        }
      }
      stack.addArrangedSubview(
        label(
          candidate.hasDetails ? "用法为原型示例；Tab 换译法，Shift＋空格输出所选英文。" : "本地词表释义，尚未提供完整用法。", size: 11,
          secondary: true))
    }
    stack.addArrangedSubview(
      label(
        bilingual
          ? "空格 中文　Shift＋空格 英文　↑↓ 换候选　Tab 展开/换译法\nPageUp / PageDown 或 − / = 翻页　1–5 选择当前页中文"
          : "空格 中文　↑↓ 换候选　F6 切换中英候选\nPageUp / PageDown 或 − / = 翻页　1–5 选择当前页中文",
        size: 11, secondary: true))
    content.setFrameSize(NSSize(width: width, height: 400))
    content.layoutSubtreeIfNeeded()
    let height = stack.fittingSize.height + 32
    let screens = NSScreen.screens
    let screen =
      screens.first(where: { $0.frame.intersects(anchor) }) ?? NSScreen.main ?? screens.first
    let bounds = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
    let actualHeight = min(height, bounds.height - 16)
    content.setFrameSize(NSSize(width: width, height: height))
    let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: width, height: actualHeight))
    scroll.drawsBackground = false
    scroll.hasVerticalScroller = height > actualHeight
    scroll.autoresizingMask = [.width, .height]
    scroll.documentView = content
    contentView = scroll
    let x = min(max(anchor.minX, bounds.minX + 8), bounds.maxX - width - 8)
    let below = anchor.minY - actualHeight - 8
    let y = below >= bounds.minY + 8 ? below : min(anchor.maxY + 8, bounds.maxY - actualHeight - 8)
    setFrame(
      NSRect(x: x, y: max(bounds.minY + 8, y), width: width, height: actualHeight), display: true)
    orderFrontRegardless()
  }

  func showTypingMode(_ mode: TypingMode, anchor: NSRect) {
    let content = NSView(frame: NSRect(x: 0, y: 0, width: 360, height: 76))
    content.wantsLayer = true
    content.layer?.backgroundColor = NSColor.white.cgColor
    content.layer?.cornerRadius = 16
    content.layer?.borderWidth = 1
    content.layer?.borderColor =
      NSColor(srgbRed: 228 / 255, green: 228 / 255, blue: 228 / 255, alpha: 1).cgColor
    let toggle = ActionButton(mode == .english ? "切回中文 · Shift" : "切到英文 · Shift") {
      [weak self] in self?.onTypingMode?()
    }
    let views = row([label(mode.title, size: 16), NSView(), toggle])
    views.frame = NSRect(x: 16, y: 16, width: 328, height: 44)
    content.addSubview(views)
    contentView = content
    let bounds =
      (NSScreen.screens.first { $0.frame.intersects(anchor) } ?? NSScreen.main)?.visibleFrame
      ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
    let x = min(max(anchor.minX, bounds.minX + 8), bounds.maxX - 368)
    let y = min(max(anchor.minY - 84, bounds.minY + 8), bounds.maxY - 84)
    setFrame(NSRect(x: x, y: y, width: 360, height: 76), display: true)
    orderFrontRegardless()
  }
}
