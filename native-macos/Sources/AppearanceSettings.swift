// 只保存本应用的外观选择；预览和测试不访问实际偏好。
import AppKit

enum AppearanceChoice: String, CaseIterable {
  case light, dark, system

  var title: String {
    switch self {
    case .light: return "白色"
    case .dark: return "黑色"
    case .system: return "跟随系统"
    }
  }

  var appearance: NSAppearance? {
    switch self {
    case .light: return NSAppearance(named: .aqua)
    case .dark: return NSAppearance(named: .darkAqua)
    case .system: return nil
    }
  }
}

final class AppearanceSettings: NSObject {
  static let shared = AppearanceSettings()
  static let key = "appearanceChoice"
  private static let notification = Notification.Name(
    "org.local.bilingualcompanion.appearanceChanged")
  private(set) var choice: AppearanceChoice
  private let store: UserDefaults?
  private var observer: NSObjectProtocol?
  private let controls = NSHashTable<ActionButton>.weakObjects()
  var isIsolated: Bool { store == nil }

  init(store: UserDefaults? = nil) {
    self.store = store
    choice = store?.string(forKey: Self.key).flatMap(AppearanceChoice.init(rawValue:)) ?? .light
    super.init()
  }

  private static var configured: AppearanceSettings?
  static var current: AppearanceSettings { configured ?? shared }

  static func configure(isolated: Bool, initial: AppearanceChoice?) {
    let settings = AppearanceSettings(
      store: isolated ? nil : UserDefaults.standard)
    configured = settings
    if let initial, isolated { settings.choice = initial }
    settings.apply()
    if !isolated {
      settings.observer = DistributedNotificationCenter.default().addObserver(
        forName: notification, object: nil, queue: .main
      ) { [weak settings] notification in
        guard let value = notification.userInfo?[key] as? String,
          let choice = AppearanceChoice(rawValue: value)
        else { return }
        settings?.select(choice, broadcast: false)
      }
    }
  }

  func select(_ value: AppearanceChoice, broadcast: Bool = true) {
    guard choice != value else { return }
    choice = value
    store?.set(value.rawValue, forKey: Self.key)
    apply()
    if broadcast, !isIsolated {
      DistributedNotificationCenter.default().postNotificationName(
        Self.notification, object: nil, userInfo: [Self.key: value.rawValue],
        deliverImmediately: true)
    }
  }

  private func apply() {
    NSApplication.shared.appearance = choice.appearance
    for button in controls.allObjects {
      button.title = "外观：\(choice.title)"
      button.setAccessibilityLabel("外观：\(choice.title)，选择白色、黑色或跟随系统")
    }
  }

  @objc func choose(_ sender: NSMenuItem) {
    guard AppearanceChoice.allCases.indices.contains(sender.tag) else { return }
    select(AppearanceChoice.allCases[sender.tag])
  }

  func menu(target: AnyObject? = nil, action: Selector? = nil) -> NSMenu {
    let menu = NSMenu(title: "外观")
    for (index, value) in AppearanceChoice.allCases.enumerated() {
      let item = NSMenuItem(
        title: value.title, action: action ?? #selector(choose(_:)), keyEquivalent: "")
      item.tag = index
      item.target = target ?? self
      item.state = choice == value ? .on : .off
      menu.addItem(item)
    }
    return menu
  }

  func button() -> ActionButton {
    let button = ActionButton("外观：\(choice.title)") {}
    button.style = .subtle
    button.font = NativeTheme.font(12)
    // 固定宽度，较长的“跟随系统”不会挤动其他内容。
    button.widthAnchor.constraint(equalToConstant: 128).isActive = true
    button.invoke = { [weak self, weak button] in
      guard let self, let button else { return }
      menu().popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.maxY), in: button)
    }
    button.setAccessibilityLabel("外观：\(choice.title)，选择白色、黑色或跟随系统")
    controls.add(button)
    return button
  }

  deinit {
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
  }
}
