// 非激活候选窗中的原生按钮保持宿主编辑器焦点。
import AppKit

final class ActionButton: NSButton {
  var invoke: (() -> Void)?

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
    font = NSFont(name: "MiSans", size: 14) ?? .systemFont(ofSize: 14)
    contentTintColor = NSColor(srgbRed: 16 / 255, green: 16 / 255, blue: 16 / 255, alpha: 1)
    translatesAutoresizingMaskIntoConstraints = false
    heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
    widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
  }

  @objc private func performAction() { invoke?() }
}
