import Foundation

enum CandidatePreferencesTests {
  static func run() {
    let domain = "org.local.bilingualcompanion.pos-test.\(UUID().uuidString)"
    let store = UserDefaults(suiteName: domain)!
    defer { store.removePersistentDomain(forName: domain) }
    precondition(CandidatePreferences(store: store).showPartOfSpeech)
    store.set("invalid", forKey: CandidatePreferences.key)
    precondition(CandidatePreferences(store: store).showPartOfSpeech)
    for show in [false, true, false] {
      CandidatePreferences(store: store).select(show)
      precondition(store.synchronize())
      precondition(CandidatePreferences(store: store).showPartOfSpeech == show)
    }
    let saved = CandidatePreferences.current
    CandidatePreferences.current = CandidatePreferences()
    defer { CandidatePreferences.current = saved }
    let detail = TranslationDetail(word: "study", pos: "n./v.", note: "", example: "")
    precondition(detail.displayWord == "study n./v." && detail.word == "study")
    CandidatePreferences.current.select(false)
    precondition(detail.displayWord == "study" && detail.word == "study")
    CandidatePreferences.current.select(true)
    precondition(
      TranslationDetail(word: "unknown", pos: "", note: "", example: "").displayWord == "unknown")
    precondition(
      TranslationDetail(word: "word", pos: "名词", note: "", example: "").displayWord == "word")
    print("PASS 词性默认显示、隔离持久化重读、开关、缩写白名单、未知不标注、输出单词不变")
  }
}
