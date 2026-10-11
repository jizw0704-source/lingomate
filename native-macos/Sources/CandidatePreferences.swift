// Display preferences only; previews and tests use isolated stores.
import Foundation

final class CandidatePreferences {
  static let key = "showPartOfSpeech"
  static var current = CandidatePreferences()
  private let store: UserDefaults?
  private(set) var showPartOfSpeech: Bool

  init(store: UserDefaults? = nil) {
    self.store = store
    showPartOfSpeech = store?.object(forKey: Self.key) as? Bool ?? true
  }
  static func configure(isolated: Bool) {
    current = CandidatePreferences(store: isolated ? nil : UserDefaults.standard)
  }
  func select(_ show: Bool) {
    showPartOfSpeech = show
    store?.set(show, forKey: Self.key)
  }
}
