// Pheno v1.4 的原生界面令牌；没有动画或透明材质。
import AppKit

enum NativeTheme {
  static let ink = color(0x101010)
  static let muted = color(0x4A4A4A)
  static let surface = color(0xF7F7F7)
  static let divider = color(0xE4E4E4)
  static let green = color(0x567700)
  static let boundary = color(0x8F9499)

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

final class CandidateDocumentView: NSView {
  override var isFlipped: Bool { true }
}
