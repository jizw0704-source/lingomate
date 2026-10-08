import AppKit

enum AppearanceTests {
  static func run() {
    let name = "org.local.bilingualcompanion.test.\(UUID().uuidString)"
    let store = UserDefaults(suiteName: name)!
    defer {
      store.removePersistentDomain(forName: name)
      NSApplication.shared.appearance = nil
    }
    let settings = AppearanceSettings(store: store)
    precondition(settings.choice == .light)
    store.set("invalid", forKey: AppearanceSettings.key)
    precondition(AppearanceSettings(store: store).choice == .light)
    settings.select(.dark, broadcast: false)
    for choice in AppearanceChoice.allCases {
      settings.select(choice, broadcast: false)
      // 使用与生产相同的本地写入路径；隔离域不发跨进程通知。
      precondition(store.string(forKey: AppearanceSettings.key) == choice.rawValue)
      precondition(AppearanceSettings(store: store).choice == choice)
      precondition(store.synchronize())
      let child = Process()
      child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0])
      child.arguments = ["--appearance-readback-test", name, choice.rawValue]
      do { try child.run() } catch { preconditionFailure(error.localizedDescription) }
      child.waitUntilExit()
      precondition(child.terminationStatus == 0)
    }
    let isolated = AppearanceSettings()
    isolated.select(.dark)
    precondition(isolated.choice == .dark && isolated.isIsolated)
    let keys = store.persistentDomain(forName: name)!.keys
    precondition(Array(keys) == [AppearanceSettings.key])

    let window = NSWindow(
      contentRect: .init(x: 0, y: 0, width: 600, height: 500),
      styleMask: [.borderless], backing: .buffered, defer: false)
    let view = CandidateDocumentView()
    view.wantsLayer = true
    view.fill = NativeTheme.background
    view.border = NativeTheme.divider
    window.contentView = view
    let marker = NSTextField(string: "正在输入的拼音")
    view.addSubview(marker)
    for appearance in [NSAppearance.Name.aqua, .darkAqua] {
      // 在同一个现存窗口内改变继承的环境，检查动态文字和 CGColor 图层。
      window.appearance = NSAppearance(named: appearance)
      window.displayIfNeeded()
      var background: NSColor!
      window.effectiveAppearance.performAsCurrentDrawingAppearance {
        background = NativeTheme.background.usingColorSpace(.sRGB)!
        precondition(
          NSColor(cgColor: view.layer!.backgroundColor!)!.usingColorSpace(.sRGB)! == background)
        for surface in [NativeTheme.background, NativeTheme.surface, NativeTheme.divider] {
          precondition(contrast(NativeTheme.ink, surface) >= 4.5)
          precondition(contrast(NativeTheme.muted, surface) >= 4.5)
          precondition(contrast(NativeTheme.green, surface) >= 3)
        }
        for surface in [NativeTheme.background, NativeTheme.surface] {
          precondition(contrast(NativeTheme.green, surface) >= 4.5)
        }
        precondition(contrast(NativeTheme.boundary, NativeTheme.background) >= 3)
      }
      precondition(marker.stringValue == "正在输入的拼音" && marker.superview === view)
    }
    settings.select(.light, broadcast: false)
    settings.select(.system, broadcast: false)
    precondition(NSApplication.shared.appearance == nil)
    window.appearance = nil
    precondition(window.effectiveAppearance.name == NSApplication.shared.effectiveAppearance.name)
    let menu = settings.menu()
    precondition(menu.items.filter { $0.state == .on }.map(\.title) == ["跟随系统"])
    print("外观检查通过：隔离持久化、非法值回退、现存窗口动态颜色、环境继承、菜单及双主题对比度")
  }

  static func readback(_ name: String, expected: String) {
    guard name.hasPrefix("org.local.bilingualcompanion.test."),
      let value = AppearanceChoice(rawValue: expected), let store = UserDefaults(suiteName: name)
    else { exit(1) }
    exit(AppearanceSettings(store: store).choice == value ? 0 : 1)
  }

  private static func contrast(_ foreground: NSColor, _ background: NSColor) -> Double {
    func luminance(_ color: NSColor) -> Double {
      let rgb = color.usingColorSpace(.sRGB)!
      func linear(_ value: CGFloat) -> Double {
        let channel = Double(value)
        return channel <= 0.04045 ? channel / 12.92 : pow((channel + 0.055) / 1.055, 2.4)
      }
      return 0.2126 * linear(rgb.redComponent) + 0.7152 * linear(rgb.greenComponent)
        + 0.0722 * linear(rgb.blueComponent)
    }
    let a = luminance(foreground)
    let b = luminance(background)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)
  }
}
