// 临时设置/账号、内存密钥；不启动IMK服务或操作真实输入法配置。
import AppKit

enum SettingsTests {
  static func run() {
    for value in [
      InputSetting.typing(.chinese), .typing(.english), .bilingual(true), .bilingual(false),
      .punctuation(.automatic), .punctuation(.chinese), .punctuation(.english),
      .remember(true), .remember(false),
    ] {
      precondition(InputSetting(payload: value.payload)?.payload == value.payload)
    }
    for value: [AnyHashable: Any] in [
      [:], ["setting": "typing", "value": "unknown"], ["setting": "remember", "value": true],
      ["setting": "punctuation", "value": "3"], ["setting": "endpoint", "value": "test"],
      ["setting": "bilingual", "value": "on", "text": "not allowed"],
    ] { precondition(InputSetting(payload: value) == nil) }
    precondition(InputSettingsSnapshot(payload: [:]) == nil)
    let client = SettingsInputClient(preview: true)
    client.refresh()
    precondition(client.snapshot?.typing == .chinese)
    client.request(.typing(.english))
    precondition(client.snapshot?.typing == .english)
    client.request(.bilingual(false))
    client.request(.remember(false))
    client.request(.punctuation(.english))
    precondition(client.snapshot?.bilingual == false && client.snapshot?.remember == false)
    precondition(client.snapshot?.punctuation == .english)
    for width: CGFloat in [560, 440] {
      let controller = SettingsController(preview: true)
      controller.window.setContentSize(NSSize(width: width, height: 620))
      controller.window.contentView?.layoutSubtreeIfNeeded()
      let email = allViews(controller.window.contentView!).compactMap { $0 as? NSTextField }
        .first { $0.placeholderString == "邮箱" }!
      email.stringValue = "test@example.invalid"
      controller.select(.translation)
      precondition(
        allViews(controller.window.contentView!).contains { ($0 as? NSSecureTextField) != nil })
      controller.select(.account)
      precondition(email.stringValue == "test@example.invalid")
      let advanced = allViews(controller.window.contentView!).compactMap { $0 as? ActionButton }
        .first { $0.title == "高级设置" }!
      precondition(
        !allViews(controller.window.contentView!).contains {
          ($0 as? NSTextField)?.placeholderString == "账号服务 HTTPS 地址"
        })
      advanced.performClick(nil)
      precondition(
        allViews(controller.window.contentView!).contains {
          ($0 as? NSTextField)?.placeholderString == "账号服务 HTTPS 地址"
        })
      controller.select(.input)
      let english = allViews(controller.window.contentView!).compactMap { $0 as? ActionButton }
        .first { $0.title.contains("中文拼音") }!
      english.performClick(nil)
      precondition(controller.input.snapshot?.typing == .chinese)
      controller.select(.appearance)
      let dark = allViews(controller.window.contentView!).compactMap { $0 as? ActionButton }
        .first { $0.title == "黑色" }!
      dark.performClick(nil)
      precondition(AppearanceSettings.current.choice == .dark)
      controller.select(.account)
      precondition(controller.window.contentView != nil && Runtime.engine == nil)
      controller.window.orderOut(nil)
      AppearanceSettings.current.select(.light)
    }
    print("PASS 统一设置560/440布局路由、账号字段保留/高级设置、翻译嵌入、输入枚举白名单/隔离应用和外观；无真实账号/API/钥匙串")
  }
  private static func allViews(_ view: NSView) -> [NSView] {
    [view] + view.subviews.flatMap(allViews)
  }
}
