// Isolated preferences, confirmed-output state and bundled bridge validation.
import Foundation

enum EnglishFragmentTests {
  static func run() {
    var fragment = EnglishFragmentState()
    func insert(_ text: String, enabled: Bool = true) -> String {
      let output = fragment.formatted(text, enabled: enabled)
      fragment.observe(output)
      return output
    }
    precondition(insert("我想说") == "我想说")
    precondition(insert(" ") == " ")
    precondition(insert("hello") == "Hello")
    precondition(insert(" ") == " ")
    precondition(insert("world") == "world")
    precondition(insert(", ") == ", ")
    precondition(insert("again") == "again")
    for separator in ["，", ":", "：", ";", "；", "（", "）", "“", "”"] {
      precondition(insert(separator) == separator)
      precondition(insert("world") == "world")
    }
    for boundary in ["中文", ".", "!", "?", "。", "！", "？", "\n", "\r"] {
      _ = insert(boundary)
      precondition(insert("hello world") == "Hello world")
    }
    for text in [
      "AI", "iPhone", "eBay", "ChatGPT", "Xi'An", "HTTP", "http://example.invalid",
      "https://example.invalid", "user@example.invalid", "www.example.invalid",
      "hello_world", "folder/file", "folder\\file", "3d", "你好",
    ] {
      fragment.reset()
      precondition(insert(text) == text)
    }
    fragment.reset()
    precondition(insert("don't") == "Don't")
    fragment.reset()
    precondition(insert("hello", enabled: false) == "hello")
    precondition(insert("world") == "world")
    fragment.reset()
    precondition(fragment.formatted("hello", enabled: true) == "Hello")
    precondition(fragment.formatted("hello", enabled: true) == "Hello")
    precondition(!fragment.inEnglish)
    fragment.observe("http")  // Unconfirmed/literal output is observed without formatting.
    fragment.observe("://example.invalid")
    precondition(fragment.formatted("path", enabled: true) == "path")
    preferences()
    bridge()
    print("PASS 英文片段首字母、连续词/中文/标点/换行边界、已有大小写/技术文本保护、关闭和隔离持久化；桥接原词校验保留")
  }

  private static func preferences() {
    let domain = "org.local.bilingualcompanion.english-fragment-test.\(UUID().uuidString)"
    let store = UserDefaults(suiteName: domain)!
    defer { store.removePersistentDomain(forName: domain) }
    precondition(EnglishInputPreferences(store: store).capitalizeFragments)
    store.set("invalid", forKey: EnglishInputPreferences.key)
    precondition(EnglishInputPreferences(store: store).capitalizeFragments)
    for enabled in [false, true, false] {
      EnglishInputPreferences(store: store).select(enabled)
      precondition(store.synchronize())
      precondition(EnglishInputPreferences(store: store).capitalizeFragments == enabled)
    }
  }

  private static func bridge() {
    let engine = try! EngineClient(resources: Bundle.main.resourceURL!)
    defer { engine.terminate() }
    let frame = try! engine.request(EngineRequest(action: "query", input: "xuexi"))
    let candidate = frame.candidates.first(where: { $0.text == "学习" })!
    let english = candidate.translations.first(where: { $0.word == "learn" })!
    let committed = try! engine.request(
      EngineRequest(
        action: "commit", input: "xuexi", candidate: candidate.text,
        syllables: candidate.syllables, english: english.word))
    precondition(committed.committed == "learn" && committed.input.isEmpty)
    var fragment = EnglishFragmentState()
    let output = fragment.formatted(committed.committed!, enabled: true)
    precondition(output == "Learn" && english.word == "learn")
    fragment.observe(output)
    precondition(fragment.formatted("more", enabled: true) == "more")
  }
}
