// 仅保存进程内计数和状态，不保存按键、正文、候选或宿主信息。
import AppKit
import Carbon
import InputMethodKit

enum InputDiagnostics {
  static var activations = 0
  static var deactivations = 0
  static var events = 0
  static var rejectedClients = 0
  static var secureRejections = 0
  static var controllerRegistered = false
  static weak var controller: BilingualInputController?
  private static var observer: NSObjectProtocol?
  private static let request = Notification.Name("org.local.bilingualcompanion.statusRequest")
  private static let response = Notification.Name("org.local.bilingualcompanion.statusResponse")

  static func snapshot() -> [String: Any] {
    [
      "version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        ?? "unknown",
      "activations": activations, "deactivations": deactivations, "events": events,
      "rejectedClients": rejectedClients, "secureRejections": secureRejections,
      "controllerRegistered": controllerRegistered,
      "controllerAlive": controller != nil,
      "compositionActive": controller?.hasComposition ?? false,
      "typingMode": Runtime.typingMode == .chinese ? "chinese" : "english",
      "bilingual": Runtime.bilingual, "enginePresent": Runtime.engine != nil,
      "engineFailure": Runtime.failure != nil, "secureInput": IsSecureEventInputEnabled(),
      "presentationAllowed": NSApplication.shared.activationPolicy() == .accessory,
      "candidateWindowVisible": NSApplication.shared.windows.contains {
        $0 is CandidatePanel && $0.isVisible
      },
    ]
  }
  static func start() {
    observer = DistributedNotificationCenter.default().addObserver(
      forName: request, object: nil, queue: .main
    ) { note in
      guard let nonce = note.object as? String, UUID(uuidString: nonce) != nil else { return }
      DistributedNotificationCenter.default().postNotificationName(
        response, object: nonce, userInfo: snapshot(), deliverImmediately: true)
    }
  }
  static func query() {
    let nonce = UUID().uuidString
    let center = DistributedNotificationCenter.default()
    observer = center.addObserver(forName: response, object: nonce, queue: .main) { note in
      guard let value = note.userInfo,
        let bytes = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
        let text = String(data: bytes, encoding: .utf8)
      else { return }
      print(text)
      exit(0)
    }
    center.postNotificationName(request, object: nonce, userInfo: nil, deliverImmediately: true)
    DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
      fputs("输入法状态服务未响应；此命令不会启动新输入法实例。\n", stderr)
      exit(1)
    }
    NSApplication.shared.setActivationPolicy(.prohibited)
    NSApplication.shared.run()
  }
  static func checks() {
    let expected: Set<String> = [
      "version", "activations", "deactivations", "events", "rejectedClients", "secureRejections",
      "controllerRegistered", "controllerAlive", "compositionActive", "typingMode", "bilingual",
      "enginePresent", "engineFailure", "secureInput",
      "presentationAllowed", "candidateWindowVisible",
    ]
    precondition(Set(snapshot().keys) == expected)
    controllerRegistered = NSClassFromString("BilingualInputController") != nil
    precondition(NSStringFromClass(BilingualInputController.self) == "BilingualInputController")
    precondition(NSClassFromString("BilingualInputController") == BilingualInputController.self)
    precondition(
      BilingualInputController.instancesRespond(to: #selector(IMKInputController.handle(_:client:)))
    )
    print("PASS 输入状态仅允许计数/布尔/模式，无文字或宿主信息；ObjC类名和按键入口匹配")
  }
}
