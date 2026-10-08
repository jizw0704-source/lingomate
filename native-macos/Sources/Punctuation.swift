// 只跟踪本次会话输出语言及字符类别，不读取宿主正文或保存标点历史。
import Foundation

enum OutputLanguage {
  case chinese, english
  var title: String { self == .chinese ? "中文" : "英文" }
}

enum PunctuationMode: Int, CaseIterable {
  case automatic, chinese, english
  var title: String {
    switch self {
    case .automatic: return "自动跟随输出语言"
    case .chinese: return "固定中文标点"
    case .english: return "固定英文标点"
    }
  }
  var next: Self { Self(rawValue: (rawValue + 1) % Self.allCases.count)! }
}

enum PunctuationAction { case symbol, raw, commitChinese }

struct PunctuationState {
  private enum Literal { case none, number, latin }
  private var literal = Literal.none
  private var doubleOpening = true
  private var singleOpening = true
  private var appliedMode = PunctuationMode.automatic
  private(set) var lastLanguage = OutputLanguage.chinese
  var isLatinLiteral: Bool { literal == .latin }

  static let symbols: [String: String] = [
    ",": "，", ".": "。", "?": "？", "!": "！", ":": "：", ";": "；",
    "(": "（", ")": "）", "[": "【", "]": "】", "<": "《", ">": "》",
    "/": "、", "\\": "、", "\"": "", "'": "", "@": "@", "_": "_",
  ]

  static func action(key: String, input: String, hasCandidate: Bool, mode: PunctuationMode)
    -> PunctuationAction?
  {
    // Apostrophe within a composition stays a Pinyin syllable separator.
    guard !(key == "'" && !input.isEmpty), symbols[key] != nil else { return nil }
    guard !input.isEmpty else { return .symbol }
    let scheme = ["http", "https", "ftp", "file", "ws", "wss"].contains(input) && key == ":"
    let www = input == "www" && key == "."
    if key == "@" || key == "_" || scheme || www || mode == .english || !hasCandidate {
      return .raw
    }
    return .commitChinese
  }

  func language(mode: PunctuationMode) -> OutputLanguage {
    switch mode {
    case .automatic: return lastLanguage
    case .chinese: return .chinese
    case .english: return .english
    }
  }

  func label(mode: PunctuationMode) -> String {
    "标点：\(language(mode: mode).title) · \(mode == .automatic ? "自动" : "固定")"
  }

  mutating func confirm(_ language: OutputLanguage) {
    if language != lastLanguage { resetQuotes() }
    lastLanguage = language
    literal = .none
  }

  mutating func observeLiteral(_ text: String) {
    for character in text {
      if character.isWhitespace {
        clearLiteral()
      } else if character.isASCII && character.isLetter {
        literal = .latin
        if lastLanguage != .english { resetQuotes() }
        lastLanguage = .english
      } else if character.isASCII && character.isNumber && literal != .latin {
        literal = .number
      } else if literal != .latin && !".:/-+".contains(character) {
        clearLiteral()
      }
    }
  }

  mutating func clearLiteral() { literal = .none }

  mutating func resetContext() {
    clearLiteral()
    resetQuotes()
  }

  private mutating func resetQuotes() {
    doubleOpening = true
    singleOpening = true
  }

  mutating func render(_ key: String, mode: PunctuationMode) -> String? {
    guard let chinese = Self.symbols[key] else { return nil }
    if appliedMode != mode {
      resetQuotes()
      appliedMode = mode
    }
    // Decimal/time/date punctuation and recognized ASCII tokens stay half width.
    if literal == .latin
      || (mode != .chinese && literal == .number && [".", ":", "/"].contains(key))
    {
      return key
    }
    clearLiteral()
    guard language(mode: mode) == .chinese else { return key }
    if key == "\"" {
      let quote = doubleOpening ? "“" : "”"
      doubleOpening.toggle()
      return quote
    }
    if key == "'" {
      let quote = singleOpening ? "‘" : "’"
      singleOpening.toggle()
      return quote
    }
    return chinese
  }
}
