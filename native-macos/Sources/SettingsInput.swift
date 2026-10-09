// 设置辅助窗口只交换枚举/布尔值，不启动第二个输入服务、不传递输入文字。
import AppKit

enum InputSetting {
  case typing(TypingMode)
  case bilingual(Bool)
  case punctuation(PunctuationMode)
  case remember(Bool)

  var payload: [String: String] {
    switch self {
    case .typing(let mode):
      return ["setting": "typing", "value": mode == .chinese ? "chinese" : "english"]
    case .bilingual(let enabled): return ["setting": "bilingual", "value": enabled ? "on" : "off"]
    case .punctuation(let mode): return ["setting": "punctuation", "value": String(mode.rawValue)]
    case .remember(let enabled): return ["setting": "remember", "value": enabled ? "on" : "off"]
    }
  }
  init?(payload: [AnyHashable: Any]) {
    guard payload.count == 2, let name = payload["setting"] as? String,
      let value = payload["value"] as? String
    else { return nil }
    switch (name, value) {
    case ("typing", "chinese"): self = .typing(.chinese)
    case ("typing", "english"): self = .typing(.english)
    case ("bilingual", "on"): self = .bilingual(true)
    case ("bilingual", "off"): self = .bilingual(false)
    case ("remember", "on"): self = .remember(true)
    case ("remember", "off"): self = .remember(false)
    case ("punctuation", _):
      guard let raw = Int(value), let mode = PunctuationMode(rawValue: raw) else { return nil }
      self = .punctuation(mode)
    default: return nil
    }
  }
  func apply() {
    switch self {
    case .typing(let value): Runtime.typingMode = value
    case .bilingual(let value): Runtime.bilingual = value
    case .punctuation(let value): Runtime.punctuationMode = value
    case .remember(let value): Runtime.remember = value
    }
  }
}

enum RuntimeSettings {
  static let change = Notification.Name("org.local.bilingualcompanion.inputSetting")
  private static var observer: NSObjectProtocol?
  static func start() {
    observer = DistributedNotificationCenter.default().addObserver(
      forName: change, object: nil, queue: .main
    ) { notification in
      guard let nonce = notification.object as? String, UUID(uuidString: nonce) != nil,
        let payload = notification.userInfo, let setting = InputSetting(payload: payload)
      else { return }
      if let controller = InputDiagnostics.controller {
        controller.applySetting(setting)
      } else {
        setting.apply()
      }
      DistributedNotificationCenter.default().postNotificationName(
        InputDiagnostics.response, object: nonce, userInfo: InputDiagnostics.snapshot(),
        deliverImmediately: true)
    }
  }
}

struct InputSettingsSnapshot {
  let typing: TypingMode
  let bilingual: Bool
  let punctuation: PunctuationMode
  let remember: Bool
  init?(payload: [AnyHashable: Any]) {
    guard let mode = payload["typingMode"] as? String, ["chinese", "english"].contains(mode),
      let bilingual = payload["bilingual"] as? Bool, let remember = payload["remember"] as? Bool,
      let raw = payload["punctuationMode"] as? Int, let punctuation = PunctuationMode(rawValue: raw)
    else { return nil }
    typing = mode == "chinese" ? .chinese : .english
    self.bilingual = bilingual
    self.remember = remember
    self.punctuation = punctuation
  }
}

final class SettingsInputClient {
  // 维护时恢复重载前的模式；仅请求现有服务，不启动引擎或第二个IMK实例。
  static func applyFromCommandLine(_ setting: InputSetting) {
    let client = SettingsInputClient(preview: false)
    client.onChange = {
      guard !client.waiting else { return }
      guard let value = client.snapshot else {
        fputs("输入设置未应用：现有服务没有响应。\n", stderr)
        exit(1)
      }
      let actual: String
      switch setting {
      case .typing: actual = value.typing == .chinese ? "chinese" : "english"
      case .bilingual: actual = value.bilingual ? "on" : "off"
      case .punctuation: actual = String(value.punctuation.rawValue)
      case .remember: actual = value.remember ? "on" : "off"
      }
      guard actual == setting.payload["value"] else { exit(1) }
      print("输入设置已应用并回读确认。")
      exit(0)
    }
    NSApplication.shared.setActivationPolicy(.prohibited)
    client.request(setting)
    withExtendedLifetime(client) { NSApplication.shared.run() }
  }
  private let preview: Bool
  private var observer: NSObjectProtocol?
  private var nonce: String?
  private(set) var snapshot: InputSettingsSnapshot?
  private(set) var waiting = false
  private(set) var message = "正在读取输入设置…"
  var onChange: (() -> Void)?
  init(preview: Bool) { self.preview = preview }
  func refresh() { request(nil) }
  func request(_ setting: InputSetting?) {
    if preview {
      setting?.apply()
      snapshot = InputSettingsSnapshot(payload: InputDiagnostics.snapshot())
      message = "界面样例：只改变此预览，不影响正在使用的输入法。"
      onChange?()
      return
    }
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
    let nonce = UUID().uuidString
    self.nonce = nonce
    waiting = true
    message = setting == nil ? "正在读取输入设置…" : "正在应用…"
    onChange?()
    observer = DistributedNotificationCenter.default().addObserver(
      forName: InputDiagnostics.response, object: nonce, queue: .main
    ) { [weak self] notification in
      guard let self, self.nonce == nonce, let payload = notification.userInfo,
        let snapshot = InputSettingsSnapshot(payload: payload)
      else { return }
      self.snapshot = snapshot
      self.finish(message: "输入设置已连接；此页选项在本次输入法运行期间生效。")
    }
    DistributedNotificationCenter.default().postNotificationName(
      setting == nil ? InputDiagnostics.request : RuntimeSettings.change,
      object: nonce, userInfo: setting?.payload, deliverImmediately: true)
    DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
      guard let self, self.nonce == nonce else { return }
      self.snapshot = nil
      self.finish(message: "输入法暂未连接。请先启用灵果，再点刷新。")
    }
  }
  private func finish(message: String) {
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
    observer = nil
    nonce = nil
    waiting = false
    self.message = message
    onChange?()
  }
  deinit {
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
  }
}
