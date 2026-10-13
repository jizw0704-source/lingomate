// 字号检查只使用独立内存状态和人工候选，不读取个人词库。
import AppKit

enum CandidateFontTests {
  static func run() {
    let preferences = CandidatePreferences.current
    let appearance = AppearanceSettings.current.choice
    defer {
      CandidatePreferences.current = preferences
      AppearanceSettings.current.select(appearance)
    }
    CandidatePreferences.current = CandidatePreferences()
    let panel = CandidatePanel()
    defer { panel.orderOut(nil) }
    let sample = EngineCandidate(
      text: "学习", syllables: [],
      translations: [TranslationDetail(word: "study", pos: "n./v.", note: "", example: "")],
      hasDetails: false)
    let state = SessionState(
      input: "xuexi", active: 5,
      frame: EngineFrame(
        input: "xuexi", marked: "xuexi", candidates: Array(repeating: sample, count: 10),
        committed: nil))
    var selected: (Int, Int)?
    panel.onEnglish = { selected = ($0, $1) }
    for theme in [AppearanceChoice.light, .dark] {
      AppearanceSettings.current.select(theme)
      for width: CGFloat in [560, 440] {
        for size in CandidateFontSize.allCases {
          InputSetting.fontSize(size).apply()
          panel.show(
            state, bilingual: true, anchor: NSRect(x: 100, y: 400, width: 1, height: 20),
            maximumWidth: width)
          panel.contentView!.layoutSubtreeIfNeeded()
          let buttons = allViews(panel.contentView!).compactMap { $0 as? ActionButton }
          for button in buttons {
            precondition(button.frame.width >= 43.9 && button.frame.height >= 43.9)
          }
          let chinese = buttons.first { $0.slotPrefix == "1 " }!
          precondition(abs(chinese.font!.pointSize - CGFloat(size.rawValue)) < 0.01)
          let english = buttons.first { $0.title == "study n./v." }!
          precondition(
            abs(english.font!.pointSize - CandidatePreferences.current.scaled(12)) < 0.01)
          precondition(
            abs(english.secondaryFontSize - CandidatePreferences.current.scaled(11)) < 0.01)
          english.performClick(nil)
          precondition(selected?.0 == 5 && selected?.1 == 0)
          precondition(state.input == "xuexi" && state.active == 5 && !state.expanded)
          precondition(!panel.canBecomeKey && !panel.canBecomeMain && panel.frame.height <= 190)
        }
      }
    }
    print("PASS 四档字号、16浅深/宽窄布局、44pt点击区、词性同比例、英文绝对索引及非激活面板")
  }
  private static func allViews(_ view: NSView) -> [NSView] {
    [view] + view.subviews.flatMap(allViews)
  }
}
