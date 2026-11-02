// 显式推荐操作与学习次数分开；菜单携带输入和引擎会话快照。
import AppKit

struct CandidateFeedback {
  let input: String
  let identity: UUID?
  let candidate: EngineCandidate
  let value: Int
  func matches(_ state: SessionState) -> Bool {
    state.input == input && state.frame?.engineIdentity == identity
      && state.frame?.candidates.contains(where: {
        $0.text == candidate.text && $0.syllables == candidate.syllables
          && $0.symbol != true && $0.feedbackEligible == true
      }) == true
  }
  static func menu(_ state: SessionState, action: ((CandidateFeedback) -> Void)?) -> NSMenuItem {
    let parent = NSMenuItem(title: "调整词语推荐", action: nil, keyEquivalent: "")
    let words = NSMenu(title: "选择词语")
    for index in state.visibleIndices {
      guard let candidate = state.frame?.candidates[index], candidate.symbol != true,
        candidate.feedbackEligible == true
      else { continue }
      let word = NSMenuItem(
        title: "\(index - state.visibleIndices.lowerBound + 1)  \(candidate.text)", action: nil,
        keyEquivalent: "")
      let choices = NSMenu(title: candidate.text)
      for (title, value) in [("优先推荐此词", 1), ("减少推荐此词", -1), ("恢复自动排序", 0)] {
        let snapshot = CandidateFeedback(
          input: state.input, identity: state.frame?.engineIdentity, candidate: candidate,
          value: value)
        let target = FeedbackMenuAction { action?(snapshot) }
        let item = NSMenuItem(
          title: title, action: #selector(FeedbackMenuAction.run(_:)), keyEquivalent: "")
        item.target = target
        item.representedObject = target
        item.state = (candidate.feedback ?? 0) == value ? .on : .off
        item.isEnabled = action != nil
        choices.addItem(item)
      }
      word.submenu = choices
      words.addItem(word)
    }
    if words.items.isEmpty || action == nil {
      parent.isEnabled = false
    } else {
      parent.submenu = words
    }
    parent.toolTip = "只调整完整读音下的词语，不增加使用次数；可在个人词库撤销。"
    return parent
  }
}
private final class FeedbackMenuAction: NSObject {
  let action: () -> Void
  init(_ action: @escaping () -> Void) { self.action = action }
  @objc func run(_ sender: Any?) { action() }
}
