// 候选窗独立预览；不发送文本给外部应用。
import AppKit

enum Preview {
  static var panel: CandidatePanel?
  static var window: NSWindow?
  static var state = SessionState()
  static var text: NSTextField?
  static var explanation: NSTextField?
  static var candidateView: NSView?
  static let sentenceTranslator = SentenceTranslator()
  static func start() {
    NSApplication.shared.setActivationPolicy(.regular)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 660, height: 180),
      styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
    window.title = "中英输入实验版 · 候选窗预览（非系统输入测试）"
    window.center()
    let label = NSTextField(wrappingLabelWithString: "这是原生候选窗预览。点击中英文检查按钮，真实输入请从系统输入源选择。")
    label.frame = NSRect(x: 24, y: 115, width: 610, height: 40)
    let output = NSTextField(string: "尚未选择")
    output.isEditable = false
    output.frame = NSRect(x: 24, y: 55, width: 610, height: 36)
    window.contentView?.addSubview(label)
    window.contentView?.addSubview(output)
    window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    self.window = window
    text = output
    explanation = label
    panel = CandidatePanel()
    if CommandLine.arguments.contains("--preview-paging") {
      state.input = "shi"
    } else {
      state.input =
        CommandLine.arguments.contains("--preview-sentence") ? "wojintianxiangxuexiyingyu" : "xuexi"
    }
    state.frame = try? Runtime.engine?.request(EngineRequest(action: "query", input: state.input))
    panel?.onChinese = { index in text?.stringValue = state.frame?.candidates[index].text ?? "" }
    panel?.onPage = { delta in
      guard state.changePage(delta) else { return }
      translateSentence()
      draw()
    }
    panel?.onEnglish = { index, sense in
      guard let candidate = state.frame?.candidates[index] else { return }
      if candidate.translations.indices.contains(sense) {
        text?.stringValue = candidate.translations[sense].word
      } else if case .ready(let source, let english) = state.sentence, source == candidate.text {
        text?.stringValue = english
      }
    }
    panel?.onExpand = { index in
      state.active = index
      state.expanded = true
      translateSentence()
      draw()
    }
    panel?.onCollapse = {
      state.expanded = false
      draw()
    }
    panel?.onMode = {
      Runtime.bilingual.toggle()
      translateSentence()
      draw()
    }
    panel?.onRetryTranslation = {
      sentenceTranslator.cancel()
      translateSentence()
    }
    panel?.onSetupTranslation = { TranslationSetup.launch() }
    translateSentence()
    draw()
  }
  static func translateSentence() {
    guard Runtime.bilingual, let candidate = state.candidate, candidate.translations.isEmpty else {
      sentenceTranslator.cancel()
      state.sentence = .none
      return
    }
    let source = candidate.text
    sentenceTranslator.request(key: "\(state.input)|\(state.active)", source: source) { phase in
      state.sentence = phase
      draw()
    }
  }
  static func draw() {
    guard let window else { return }
    panel?.show(
      state, bilingual: Runtime.bilingual,
      anchor: NSRect(x: window.frame.minX + 24, y: window.frame.minY, width: 1, height: 20))
    // 预览将同一候选视图放进标准窗口，方便检查；此模式不验证宿主焦点。
    guard let panel, let view = panel.contentView else { return }
    panel.orderOut(nil)
    candidateView?.removeFromSuperview()
    let height = panel.frame.height
    window.setContentSize(NSSize(width: 660, height: height + 180))
    explanation?.frame = NSRect(x: 24, y: height + 115, width: 610, height: 40)
    text?.frame = NSRect(x: 24, y: height + 65, width: 610, height: 36)
    view.removeFromSuperview()
    window.contentView?.addSubview(view)
    view.frame = NSRect(x: 24, y: 24, width: 600, height: height)
    candidateView = view
    window.center()
  }
}
