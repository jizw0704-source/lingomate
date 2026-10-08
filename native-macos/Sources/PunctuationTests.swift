// 标点规则及真实引擎组合提交检查；不创建 IMK 服务、不读写个人词库。
import Foundation

enum PunctuationTests {
  static func run() {
    mapping()
    modesAndQuotes()
    literalTokens()
    compositionRouting()
    engineSubmission()
    print("PASS 标点映射、跟随语言、手动覆盖、引号、数字/地址保护、拼音分隔及剩余拼音")
  }

  private static func mapping() {
    let expected = [
      ",": "，", ".": "。", "?": "？", "!": "！", ":": "：", ";": "；",
      "(": "（", ")": "）", "[": "【", "]": "】", "<": "《", ">": "》", "/": "、", "\\": "、",
    ]
    var state = PunctuationState()
    for (key, value) in expected {
      precondition(state.render(key, mode: .automatic) == value)
    }
    state.confirm(.english)
    for key in expected.keys { precondition(state.render(key, mode: .automatic) == key) }
    state.confirm(.chinese)
    precondition(state.render(",", mode: .automatic) == "，")
    precondition(state.render("a", mode: .automatic) == nil)
  }

  private static func modesAndQuotes() {
    var state = PunctuationState()
    precondition(state.render("\"", mode: .automatic) == "“")
    state.confirm(.chinese)  // Chinese word inside quotes must not reset the closing side.
    state.clearLiteral()
    precondition(state.render("\"", mode: .automatic) == "”")
    precondition(state.render("'", mode: .automatic) == "‘")
    precondition(state.render("'", mode: .automatic) == "’")
    state.confirm(.english)
    precondition(state.render("'", mode: .automatic) == "'")
    precondition(state.render("\"", mode: .automatic) == "\"")
    precondition(state.render(",", mode: .chinese) == "，")
    state.confirm(.chinese)
    precondition(state.render(",", mode: .english) == ",")
    precondition(state.render("\"", mode: .chinese) == "“")
    state.resetContext()
    precondition(state.render("\"", mode: .chinese) == "“")
    precondition(PunctuationMode.automatic.next == .chinese)
    precondition(PunctuationMode.chinese.next == .english)
    precondition(PunctuationMode.english.next == .automatic)
    var independent = PunctuationState()
    precondition(independent.render(",", mode: .automatic) == "，")
    state.confirm(.english)
    precondition(state.render(",", mode: .automatic) == ",")
  }

  private static func literalTokens() {
    var number = PunctuationState()
    number.observeLiteral("3")
    precondition(number.render(".", mode: .automatic) == ".")
    number.observeLiteral("14")
    precondition(number.render(",", mode: .automatic) == "，")
    number.observeLiteral("12")
    precondition(number.render(":", mode: .automatic) == ":")
    number.observeLiteral("30%")
    precondition(number.render(".", mode: .automatic) == "。")
    number.observeLiteral("3")
    precondition(number.render(".", mode: .chinese) == "。")
    number.observeLiteral("3")
    number.clearLiteral()
    precondition(number.render(".", mode: .automatic) == "。")
    number.observeLiteral("3")
    number.resetContext()  // cursor move/delete must not reuse a previous numeric context.
    precondition(number.render(".", mode: .automatic) == "。")
    var address = PunctuationState()
    address.observeLiteral("https")
    for key in [":", "/", "/", ".", "?", "@", "_"] {
      precondition(address.render(key, mode: .chinese) == key)
    }
    precondition(address.isLatinLiteral)
    address.clearLiteral()
    address.confirm(.chinese)
    precondition(address.render(",", mode: .automatic) == "，")
    var contraction = PunctuationState()
    contraction.observeLiteral("I")
    precondition(contraction.render("'", mode: .automatic) == "'")
  }

  private static func compositionRouting() {
    func action(
      _ key: String, _ input: String, candidate: Bool = true,
      mode: PunctuationMode = .automatic
    ) -> PunctuationAction? {
      PunctuationState.action(key: key, input: input, hasCandidate: candidate, mode: mode)
    }
    precondition(action("'", "xi") == nil)
    precondition(action("'", "") == .symbol)
    precondition(action(",", "nihao") == .commitChinese)
    precondition(action(",", "nihao", mode: .english) == .raw)
    precondition(action(":", "https") == .raw)
    precondition(action(".", "www") == .raw)
    precondition(action("@", "test") == .raw)
    precondition(action("_", "test") == .raw)
    precondition(action(".", "example", mode: .english) == .raw)
    precondition(action(",", "user", candidate: false) == .raw)
    precondition(action("-", "shi") == nil)  // Paging belongs to the existing keyboard route.
  }

  private static func engineSubmission() {
    let engine = try! EngineClient(resources: Bundle.main.resourceURL!)
    defer { engine.terminate() }
    func commit(_ input: String, word: String, english: String? = nil) -> EngineFrame {
      let frame = try! engine.request(EngineRequest(action: "query", input: input))
      let candidate = frame.candidates.first { $0.text == word }!
      return try! engine.request(
        EngineRequest(
          action: "commit", input: input,
          candidate: candidate.text, syllables: candidate.syllables, english: english))
    }
    var punctuation = PunctuationState()
    let chinese = commit("nihao", word: "你好")
    precondition(chinese.input.isEmpty)
    punctuation.confirm(.chinese)
    precondition(chinese.committed! + punctuation.render(",", mode: .automatic)! == "你好，")
    let english = commit("xuexi", word: "学习", english: "learn")
    precondition(english.input.isEmpty)
    punctuation.confirm(.english)
    precondition(english.committed! + punctuation.render(",", mode: .automatic)! == "learn,")
    let partial = commit("kaifazhe", word: "开发")
    // Native route retains this composition, omitting punctuation.
    precondition(partial.input == "zhe")
    let remaining = try! engine.request(EngineRequest(action: "query", input: partial.input))
    precondition(!remaining.candidates.isEmpty)
    let separated = try! engine.request(EngineRequest(action: "query", input: "xi'an"))
    precondition(separated.candidates.contains { $0.text == "西安" })
  }
}
