// Display preferences only; previews and tests use isolated stores.
import AppKit

enum CandidateFontSize: Int, CaseIterable {
  case small = 16
  case standard = 18
  case large = 20
  case extraLarge = 22
  var title: String {
    switch self {
    case .small: return "小"
    case .standard: return "标准"
    case .large: return "大"
    case .extraLarge: return "特大"
    }
  }
}

final class CandidatePreferences {
  static let key = "showPartOfSpeech"
  static let symbolsKey = "showSymbolCandidates"
  static let opacityKey = "candidateBackgroundOpacity"
  static let opacityChanged = Notification.Name("org.local.bilingualcompanion.opacityChanged")
  static let opacityBroadcast = Notification.Name("org.local.bilingualcompanion.opacityPreference")
  static let fontKey = "candidateFontSize"
  static var current = CandidatePreferences()
  private let store: UserDefaults?
  private(set) var showPartOfSpeech: Bool
  private(set) var showSymbols: Bool
  private var observer: NSObjectProtocol?
  private let broadcastsChanges: Bool
  private let opacitySender = UUID().uuidString
  private let opacityChannel: Notification.Name
  private(set) var opacity: Int
  private(set) var fontSize: CandidateFontSize

  init(
    store: UserDefaults? = nil, broadcastsChanges: Bool = false,
    opacityChannel: Notification.Name = CandidatePreferences.opacityBroadcast
  ) {
    self.opacityChannel = opacityChannel
    self.broadcastsChanges = broadcastsChanges
    let saved = store?.object(forKey: Self.opacityKey) as? Int ?? 100
    opacity = (70...100).contains(saved) ? saved : 100
    self.store = store
    showSymbols = store?.object(forKey: Self.symbolsKey) as? Bool ?? true
    showPartOfSpeech = store?.object(forKey: Self.key) as? Bool ?? true
    fontSize =
      (store?.object(forKey: Self.fontKey) as? Int).flatMap(CandidateFontSize.init(rawValue:))
      ?? .standard
  }
  static func configure(isolated: Bool) {
    current = CandidatePreferences(
      store: isolated ? nil : UserDefaults.standard, broadcastsChanges: !isolated)
    if !isolated { current.observeOpacity() }
  }
  static func opacityValue(_ payload: [AnyHashable: Any]?) -> Int? {
    guard let payload, payload.count == 1, let percent = payload["percent"] as? Int,
      (70...100).contains(percent)
    else { return nil }
    return percent
  }
  func observeOpacity() {
    guard observer == nil else { return }
    observer = DistributedNotificationCenter.default().addObserver(
      forName: opacityChannel, object: nil, queue: .main
    ) { [weak self] notification in
      guard let self, notification.object as? String != self.opacitySender,
        let percent = Self.opacityValue(notification.userInfo)
      else { return }
      self.selectOpacity(percent, broadcast: false)
    }
  }
  deinit {
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
  }
  @discardableResult func selectOpacity(_ percent: Int, broadcast: Bool = true) -> Bool {
    guard (70...100).contains(percent) else { return false }
    guard opacity != percent else { return true }
    opacity = percent
    store?.set(percent, forKey: Self.opacityKey)
    NotificationCenter.default.post(name: Self.opacityChanged, object: nil)
    if broadcastsChanges && broadcast {
      DistributedNotificationCenter.default().postNotificationName(
        opacityChannel, object: opacitySender, userInfo: ["percent": percent],
        deliverImmediately: true
      )
    }
    return true
  }
  func selectSymbols(_ enabled: Bool) {
    showSymbols = enabled
    store?.set(enabled, forKey: Self.symbolsKey)
  }
  func scaled(_ size: CGFloat) -> CGFloat { size * CGFloat(fontSize.rawValue) / 18 }
  func selectFont(_ size: CandidateFontSize) {
    fontSize = size
    store?.set(size.rawValue, forKey: Self.fontKey)
  }
  func select(_ show: Bool) {
    showPartOfSpeech = show
    store?.set(show, forKey: Self.key)
  }
}
