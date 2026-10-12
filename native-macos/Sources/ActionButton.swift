// 非激活候选窗中的原生按钮保持宿主编辑器焦点。
import AppKit

final class ActionButton: NSButton {
  enum Style { case plain, subtle, outlined, accent, selected }
  var invoke: (() -> Void)?
  var style = Style.plain { didSet { needsDisplay = true } }
  var wrapsTitle = false
  var horizontalPadding: CGFloat = 12
  var secondarySuffix = ""
  var secondaryFontSize: CGFloat = 11
  var slotFontSize: CGFloat = 11
  var symbolName: String?
  var symbolSize: CGFloat = 19
  var slotPrefix = ""
  var contentOffsetY: CGFloat = 0
  var compactCapsule = false
  private var hovered = false
  private var hoverArea: NSTrackingArea?

  override var intrinsicContentSize: NSSize {
    let size = super.intrinsicContentSize
    return NSSize(width: max(44, size.width + horizontalPadding * 2), height: max(44, size.height))
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

  override func viewDidChangeEffectiveAppearance() {
    super.viewDidChangeEffectiveAppearance()
    needsDisplay = true
  }

  override func draw(_ dirtyRect: NSRect) {
    let face =
      compactCapsule
      ? bounds.insetBy(dx: 2, dy: 8).offsetBy(dx: 0, dy: contentOffsetY)
      : bounds.insetBy(dx: 1, dy: 1)
    let radius: CGFloat = compactCapsule ? 14 : 10
    let path = NSBezierPath(roundedRect: face, xRadius: radius, yRadius: radius)
    let pressed = isEnabled && isHighlighted
    if style == .selected {
      (pressed ? NativeTheme.muted : NativeTheme.ink).setFill()
      path.fill()
    } else if style == .subtle || (isEnabled && (hovered || pressed)) {
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
    if let symbolName,
      let symbol = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
    {
      let configured =
        symbol.withSymbolConfiguration(
          NSImage.SymbolConfiguration(pointSize: symbolSize, weight: .medium)) ?? symbol
      let tint = isEnabled ? NativeTheme.ink : NativeTheme.muted
      let tinted = NSImage(size: configured.size, flipped: false) { rect in
        configured.draw(in: rect)
        tint.setFill()
        rect.fill(using: .sourceAtop)
        return true
      }
      let size = NSSize(width: symbolSize, height: symbolSize)
      tinted.draw(
        in: NSRect(
          x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2,
          width: size.width, height: size.height))
      return
    }
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = alignment
    paragraph.lineBreakMode = wrapsTitle ? .byWordWrapping : .byTruncatingTail
    let attributes: [NSAttributedString.Key: Any] = [
      .font: font ?? NativeTheme.font(14),
      .foregroundColor: isEnabled
        ? (style == .selected
          ? NativeTheme.background
          : pressed
            ? NativeTheme.ink
            : style == .accent ? NativeTheme.green : contentTintColor ?? NativeTheme.ink)
        : NativeTheme.muted,
      .paragraphStyle: paragraph,
    ]
    let width = max(1, bounds.width - horizontalPadding * 2)
    let height = (title as NSString).boundingRect(
      with: NSSize(width: width, height: .greatestFiniteMagnitude),
      options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: attributes
    ).height
    let rect = NSRect(
      x: horizontalPadding, y: (bounds.height - ceil(height)) / 2 + contentOffsetY, width: width,
      height: ceil(height))
    if !secondarySuffix.isEmpty, title.hasSuffix(secondarySuffix) {
      let suffixAttributes: [NSAttributedString.Key: Any] = [
        .font: NativeTheme.font(secondaryFontSize), .foregroundColor: NativeTheme.muted,
        .paragraphStyle: paragraph,
      ]
      let suffix = secondarySuffix as NSString
      let suffixSize = suffix.size(withAttributes: suffixAttributes)
      let primary = String(title.dropLast(secondarySuffix.count)).trimmingCharacters(
        in: .whitespaces)
      let primaryRect = NSRect(
        x: rect.minX, y: rect.minY,
        width: max(1, rect.width - ceil(suffixSize.width) - 4), height: rect.height)
      (primary as NSString).draw(
        with: primaryRect,
        options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: attributes)
      let suffixRect = NSRect(
        x: rect.maxX - ceil(suffixSize.width),
        y: rect.minY + (rect.height - ceil(suffixSize.height)) / 2,
        width: ceil(suffixSize.width), height: ceil(suffixSize.height))
      suffix.draw(in: suffixRect, withAttributes: suffixAttributes)
      return
    }
    let content = NSMutableAttributedString(string: title, attributes: attributes)
    if !slotPrefix.isEmpty, title.hasPrefix(slotPrefix) {
      content.addAttributes(
        [
          .font: NativeTheme.font(slotFontSize),
          .foregroundColor: style == .selected ? NativeTheme.background : NativeTheme.muted,
        ],
        range: NSRange(location: 0, length: (slotPrefix as NSString).length))
    }
    content.draw(with: rect, options: [.usesLineFragmentOrigin, .usesFontLeading])
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
