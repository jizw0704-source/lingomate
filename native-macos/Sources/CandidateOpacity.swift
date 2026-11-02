// Background-only transparency. No window/view alpha, blur, animation or composition changes.
import AppKit
import Carbon

enum CandidateOpacity {
  static func effective(_ percent: Int, reduceTransparency: Bool) -> Int {
    reduceTransparency || !(70...100).contains(percent) ? 100 : percent
  }
  static var accessibilityOverride: Bool {
    NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
      || NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
  }
  static var effectivePercent: Int {
    effective(CandidatePreferences.current.opacity, reduceTransparency: accessibilityOverride)
  }
}

final class CandidateSurfaceView: CandidateDocumentView {
  private var observer: NSObjectProtocol?
  private var accessibilityObserver: NSObjectProtocol?
  private var labelColors: [ObjectIdentifier: NSColor] = [:]
  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    wantsLayer = true
    updateOpacity()
    observer = NotificationCenter.default.addObserver(
      forName: CandidatePreferences.opacityChanged, object: nil, queue: .main
    ) { [weak self] _ in self?.updateOpacity() }
    accessibilityObserver = NSWorkspace.shared.notificationCenter.addObserver(
      forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil,
      queue: .main
    ) { [weak self] _ in self?.updateOpacity() }
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }
  deinit {
    if let observer { NotificationCenter.default.removeObserver(observer) }
    if let accessibilityObserver {
      NSWorkspace.shared.notificationCenter.removeObserver(accessibilityObserver)
    }
  }
  override func viewDidChangeEffectiveAppearance() {
    super.viewDidChangeEffectiveAppearance()
    updateOpacity()
  }
  override func layout() {
    super.layout()
    refreshForeground(self)
  }
  func updateOpacity() {
    fill = NativeTheme.background.withAlphaComponent(
      CGFloat(CandidateOpacity.effectivePercent) / 100)
    refreshForeground(self)
    needsDisplay = true
  }
  private func refreshForeground(_ view: NSView) {
    if let label = view as? NSTextField {
      let id = ObjectIdentifier(label)
      if labelColors[id] == nil { labelColors[id] = label.textColor }
      label.textColor = CandidateOpacity.effectivePercent < 100 ? NativeTheme.ink : labelColors[id]
    }
    if let button = view as? ActionButton {
      button.candidateContrast = true
      button.needsDisplay = true
    }
    for child in view.subviews { refreshForeground(child) }
  }
}

// Isolated visual sample; its checkerboard is drawn by us, never captured from the host.
final class OpacityBackdrop: NSView {
  override func draw(_ dirtyRect: NSRect) {
    NSGraphicsContext.saveGraphicsState()
    defer { NSGraphicsContext.restoreGraphicsState() }
    NSGraphicsContext.current?.cgContext.clip(to: bounds)
    let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
    for x in stride(from: 0, to: Int(bounds.width) + 20, by: 20) {
      for y in stride(from: 0, to: Int(bounds.height) + 20, by: 20) {
        let alternate = (x / 20 + y / 20).isMultiple(of: 2)
        NativeTheme.color(
          dark ? (alternate ? 0x383838 : 0x606060) : (alternate ? 0xDDDDDD : 0xB8B8B8)
        ).setFill()
        NSRect(x: x, y: y, width: 20, height: 20).fill()
      }
    }
  }
}

final class OpacitySlider: NSSlider {
  override var acceptsFirstResponder: Bool { true }
  override func mouseDown(with event: NSEvent) {
    window?.makeFirstResponder(self)
    super.mouseDown(with: event)
  }
  override func keyDown(with event: NSEvent) {
    guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty else {
      super.keyDown(with: event)
      return
    }
    let next: Int
    switch Int(event.keyCode) {
    case kVK_LeftArrow, kVK_DownArrow: next = integerValue - 1
    case kVK_RightArrow, kVK_UpArrow: next = integerValue + 1
    case kVK_Home: next = 70
    case kVK_End: next = 100
    default:
      super.keyDown(with: event)
      return
    }
    integerValue = min(100, max(70, next))
    sendAction(action, to: target)
  }
}

final class CandidateOpacityControl: NSView {
  let slider = OpacitySlider(value: 100, minValue: 70, maxValue: 100, target: nil, action: nil)
  let valueLabel = NativeTheme.label("100%", size: 14, weight: .medium)
  let accessibilityLabel = NativeTheme.label("", size: 12, secondary: true)
  let sample = CandidateSurfaceView()
  private var observer: NSObjectProtocol?
  private var accessibilityObserver: NSObjectProtocol?
  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    slider.doubleValue = Double(CandidatePreferences.current.opacity)
    slider.isContinuous = true
    slider.numberOfTickMarks = 31
    slider.allowsTickMarkValuesOnly = true
    slider.target = self
    slider.action = #selector(changed)
    slider.setAccessibilityLabel("候选窗背景不透明度")
    slider.toolTip = "70% 更通透，100% 不透明；方向键微调"
    let title = NativeTheme.label("候选窗背景不透明度", size: 16, weight: .medium)
    let line = NSStackView(views: [slider, valueLabel])
    line.orientation = .horizontal
    line.spacing = 12
    valueLabel.widthAnchor.constraint(equalToConstant: 48).isActive = true
    slider.heightAnchor.constraint(equalToConstant: 44).isActive = true
    let backdrop = OpacityBackdrop()
    backdrop.translatesAutoresizingMaskIntoConstraints = false
    backdrop.heightAnchor.constraint(equalToConstant: 88).isActive = true
    sample.translatesAutoresizingMaskIntoConstraints = false
    sample.layer?.cornerRadius = 16
    sample.layer?.borderWidth = 1
    sample.border = NativeTheme.divider
    let word = NativeTheme.label("学习", size: 18, weight: .medium)
    let english = NativeTheme.label("study  v.", size: 13, secondary: true)
    let content = NSStackView(views: [word, english])
    content.orientation = .vertical
    content.alignment = .leading
    content.spacing = 4
    content.translatesAutoresizingMaskIntoConstraints = false
    sample.addSubview(content)
    backdrop.addSubview(sample)
    NSLayoutConstraint.activate([
      sample.leadingAnchor.constraint(equalTo: backdrop.leadingAnchor, constant: 12),
      sample.trailingAnchor.constraint(equalTo: backdrop.trailingAnchor, constant: -12),
      sample.topAnchor.constraint(equalTo: backdrop.topAnchor, constant: 8),
      sample.bottomAnchor.constraint(equalTo: backdrop.bottomAnchor, constant: -8),
      content.leadingAnchor.constraint(equalTo: sample.leadingAnchor, constant: 16),
      content.centerYAnchor.constraint(equalTo: sample.centerYAnchor),
    ])
    let stack = NSStackView(views: [title, line, backdrop, accessibilityLabel])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: leadingAnchor),
      stack.trailingAnchor.constraint(equalTo: trailingAnchor),
      stack.topAnchor.constraint(equalTo: topAnchor),
      stack.bottomAnchor.constraint(equalTo: bottomAnchor),
      line.widthAnchor.constraint(equalTo: stack.widthAnchor),
      backdrop.widthAnchor.constraint(equalTo: stack.widthAnchor),
      accessibilityLabel.widthAnchor.constraint(equalTo: stack.widthAnchor),
    ])
    observer = NotificationCenter.default.addObserver(
      forName: CandidatePreferences.opacityChanged, object: nil, queue: .main
    ) { [weak self] _ in self?.refresh() }
    accessibilityObserver = NSWorkspace.shared.notificationCenter.addObserver(
      forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil,
      queue: .main
    ) { [weak self] _ in self?.refresh() }
    refresh()
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }
  deinit {
    if let observer { NotificationCenter.default.removeObserver(observer) }
    if let accessibilityObserver {
      NSWorkspace.shared.notificationCenter.removeObserver(accessibilityObserver)
    }
  }
  func refresh() {
    let percent = CandidatePreferences.current.opacity
    slider.integerValue = percent
    valueLabel.stringValue = "\(percent)%"
    slider.setAccessibilityValueDescription("\(percent)% 背景不透明度")
    accessibilityLabel.stringValue =
      CandidateOpacity.accessibilityOverride
      ? "系统已减少透明度或增强对比度，当前以 100% 显示。选择已保留。"
      : "70% 更通透 · 100% 不透明。只改变背景，选择自动保存。"
    sample.updateOpacity()
  }
  @objc func changed() {
    let value = Int(slider.doubleValue.rounded())
    _ = CandidatePreferences.current.selectOpacity(value)
    refresh()
  }
}
