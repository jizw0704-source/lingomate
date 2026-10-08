// 非激活候选窗中的原生按钮保持宿主编辑器焦点。
import AppKit

final class ActionButton: NSButton {
  enum Style { case plain, subtle, outlined, accent }
  var invoke: (() -> Void)?
  var style = Style.plain { didSet { needsDisplay = true } }
  var wrapsTitle = false
  private var hovered = false
  private var hoverArea: NSTrackingArea?

  override var intrinsicContentSize: NSSize {
    let size = super.intrinsicContentSize
    return NSSize(width: max(44, size.width + 24), height: max(44, size.height))
  }

  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    if let hoverArea { removeTrackingArea(hoverArea) }
    let area = NSTrackingArea(
      rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
      owner: self, userInfo: nil)
    addTrackingArea(area)
    hoverArea = area
  }

  override func mouseEntered(with event: NSEvent) {
    hovered = true
    needsDisplay = true
  }

  override func mouseExited(with event: NSEvent) {
    hovered = false
    needsDisplay = true
  }

  override func draw(_ dirtyRect: NSRect) {
    let path = NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 10, yRadius: 10)
    let pressed = isEnabled && isHighlighted
    if style == .subtle || (isEnabled && (hovered || pressed)) {
      (pressed ? NativeTheme.divider : NativeTheme.surface).setFill()
      path.fill()
    }
    if style == .outlined {
      (isEnabled && (hovered || pressed) ? NativeTheme.muted : NativeTheme.boundary).setStroke()
      path.lineWidth = 1
      path.stroke()
    }
    if window?.firstResponder === self {
      NativeTheme.green.setStroke()
      path.lineWidth = 2
      path.stroke()
    }
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = alignment
    paragraph.lineBreakMode = wrapsTitle ? .byWordWrapping : .byTruncatingTail
    let attributes: [NSAttributedString.Key: Any] = [
      .font: font ?? NativeTheme.font(14),
      .foregroundColor: isEnabled
        ? (pressed
          ? NativeTheme.ink
          : style == .accent ? NativeTheme.green : contentTintColor ?? NativeTheme.ink)
        : NativeTheme.muted,
      .paragraphStyle: paragraph,
    ]
    let width = max(1, bounds.width - 24)
    let height = (title as NSString).boundingRect(
      with: NSSize(width: width, height: .greatestFiniteMagnitude),
      options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: attributes
    ).height
    let rect = NSRect(
      x: 12, y: (bounds.height - ceil(height)) / 2, width: width, height: ceil(height))
    (title as NSString).draw(
      with: rect, options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: attributes)
  }

  override func drawFocusRingMask() {
    NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 10, yRadius: 10).fill()
  }

  convenience init(_ title: String, label: String? = nil, action: @escaping () -> Void) {
    self.init(frame: .zero)
    self.title = title
    setAccessibilityLabel(label ?? title)
    invoke = action
    target = self
    self.action = #selector(performAction)
    isBordered = false
    bezelStyle = .regularSquare
    focusRingType = .exterior
    font = NativeTheme.font(14)
    contentTintColor = NativeTheme.ink
    translatesAutoresizingMaskIntoConstraints = false
    heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
    widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
  }

  @objc private func performAction() { invoke?() }
}
