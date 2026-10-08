// Pheno v1.4 的原生界面令牌；没有动画或透明材质。
import AppKit

enum NativeTheme {
  static let background = adaptive(0xFFFFFF, 0x171717)
  static let ink = adaptive(0x101010, 0xF7F7F7)
  static let muted = adaptive(0x4A4A4A, 0xBDBDBD)
  static let surface = adaptive(0xF7F7F7, 0x262626)
  static let divider = adaptive(0xE4E4E4, 0x383838)
  static let green = adaptive(0x567700, 0xB5DC70)
  static let boundary = adaptive(0x8F9499, 0x8F9499)

  static func adaptive(_ light: UInt32, _ dark: UInt32) -> NSColor {
    NSColor(name: nil) { appearance in
      color(appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light)
    }
  }

  static func color(_ hex: UInt32) -> NSColor {
    NSColor(
      srgbRed: CGFloat((hex >> 16) & 255) / 255,
      green: CGFloat((hex >> 8) & 255) / 255,
      blue: CGFloat(hex & 255) / 255, alpha: 1)
  }

  static func font(_ size: CGFloat, weight: NSFont.Weight = .regular) -> NSFont {
    NSFont(name: "MiSans", size: size) ?? .systemFont(ofSize: size, weight: weight)
  }

  static func label(
    _ value: String, size: CGFloat = 13, secondary: Bool = false,
    weight: NSFont.Weight = .regular
  ) -> NSTextField {
    let field = NSTextField(wrappingLabelWithString: value)
    field.font = font(size, weight: weight)
    field.textColor = secondary ? muted : ink
    field.translatesAutoresizingMaskIntoConstraints = false
    field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    return field
  }
}

// CGColor 不会自动解析动态外观，随视图的实际 appearance 更新图层。
final class CandidateDocumentView: NSView {
  var fill: NSColor? { didSet { updateColors() } }
  var border: NSColor? { didSet { updateColors() } }
  override var isFlipped: Bool { true }

  override func viewDidChangeEffectiveAppearance() {
    super.viewDidChangeEffectiveAppearance()
    updateColors()
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    updateColors()
  }

  private func updateColors() {
    effectiveAppearance.performAsCurrentDrawingAppearance {
      layer?.backgroundColor = fill?.cgColor
      layer?.borderColor = border?.cgColor
    }
  }
}
