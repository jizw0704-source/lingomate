// Display preferences only; previews and tests use isolated stores.
import Foundation

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
  static let fontKey = "candidateFontSize"
  static var current = CandidatePreferences()
  private let store: UserDefaults?
  private(set) var showPartOfSpeech: Bool
  private(set) var fontSize: CandidateFontSize

  init(store: UserDefaults? = nil) {
    self.store = store
    showPartOfSpeech = store?.object(forKey: Self.key) as? Bool ?? true
    fontSize =
      (store?.object(forKey: Self.fontKey) as? Int).flatMap(CandidateFontSize.init(rawValue:))
      ?? .standard
  }
  static func configure(isolated: Bool) {
    current = CandidatePreferences(store: isolated ? nil : UserDefaults.standard)
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
