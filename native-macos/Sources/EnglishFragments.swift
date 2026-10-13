// Only the current controller's confirmed output determines the English fragment.
// No host text, clipboard, input history or automatic title casing is used.
import Foundation

final class EnglishInputPreferences {
  static let key = "capitalizeEnglishFragments"
  static var current = EnglishInputPreferences()
  private let store: UserDefaults?
  private(set) var capitalizeFragments: Bool

  init(store: UserDefaults? = nil) {
    self.store = store
    capitalizeFragments = store?.object(forKey: Self.key) as? Bool ?? true
  }
  static func configure(isolated: Bool) {
    current = EnglishInputPreferences(store: isolated ? nil : UserDefaults.standard)
  }
  func select(_ enabled: Bool) {
    capitalizeFragments = enabled
    store?.set(enabled, forKey: Self.key)
  }
}

struct EnglishFragmentState {
  private(set) var inEnglish = false

  mutating func reset() { inEnglish = false }

  // Calculate without advancing the state; observe only after the host insertion.
  func formatted(_ text: String, enabled: Bool) -> String {
    guard enabled, !inEnglish, let first = text.utf8.first, (97...122).contains(first) else {
      return text
    }
    let word = text.prefix { $0.isASCII && ($0.isLetter || $0 == "'" || $0 == "-") }
    guard !word.utf8.contains(where: { (65...90).contains($0) }),
      !text.contains("://"), !text.contains("@"), !text.contains("_"),
      !text.contains("\\"), !text.contains("/"), !text.hasPrefix("www.")
    else { return text }
    return String(UnicodeScalar(first - 32)) + text.dropFirst()
  }

  mutating func observe(_ text: String) {
    for character in text {
      if character == "\n" || character == "\r" || ".!?。！？".contains(character) {
        inEnglish = false
      } else if character.isASCII && character.isLetter {
        inEnglish = true
      } else if character.isWhitespace || (character.isASCII && !character.isLetter)
        || "，：；（）【】“”‘’".contains(character)
      {
        // Spaces and commas separate words, not English fragments.
        continue
      } else {
        inEnglish = false
      }
    }
  }
}
