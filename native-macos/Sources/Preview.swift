// 候选窗独立预览；不发送文本给外部应用。
import AppKit

enum Preview {
  static var panel: CandidatePanel?
  static var window: NSWindow?
  static var state = SessionState()
  static var text: NSTextField?
  static var explanation: NSTextField?
  static var outputCaption: NSTextField?
  static var candidateView: NSView?
  static let sentenceTranslator = SentenceTranslator()
  static var learningDirectory: URL?
  static var punctuation = PunctuationState()
  static var punctuationView: NSView?
  static let punctuationPreview = CommandLine.arguments.contains("--preview-punctuation")
  static let typingPreview = CommandLine.arguments.contains("--preview-typing")
  static var shiftTap = ShiftTap()
  static var eventMonitor: Any?
  static let narrowPreview = CommandLine.arguments.contains("--preview-narrow")
  static let fixture: String? = {
    let arguments = CommandLine.arguments
    guard let index = arguments.firstIndex(of: "--ui-state"), arguments.count > index + 1 else {
      return nil
    }
    return arguments[index + 1]
  }()
  static var hasPresented = false
  static func prepareLearning(resources: URL) throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    learningDirectory = directory
    let file = directory.appendingPathComponent("words.json")
    let engine = try EngineClient(resources: resources, memoryURL: file)
    let frame = try engine.request(EngineRequest(action: "query", input: "shi"))
    guard let candidate = frame.candidates.first(where: { $0.text == "市" }) else {
      throw EngineFailure.message("预览候选缺失。")
    }
    let committed = try engine.request(
      EngineRequest(
        action: "commit", input: "shi", candidate: candidate.text,
        syllables: candidate.syllables, context: "preview"))
    engine.confirmSelection(committed, context: "preview", chinese: true)
    engine.terminate()
    Runtime.engine?.terminate()
    Runtime.engine = try EngineClient(resources: resources, memoryURL: file)
  }
  static func start() {
    NSApplication.shared.setActivationPolicy(.regular)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 660, height: 180),
      styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
    window.title = "中英输入实验版 · 候选窗预览（非系统输入测试）"
    window.backgroundColor = NativeTheme.background
    window.center()
    let label = NSTextField(wrappingLabelWithString: "这是原生候选窗预览。点击中英文检查按钮，真实输入请从系统输入源选择。")
    if learningDirectory != nil {
      label.stringValue = "选词记忆预览：已在隔离词库中选过‘市’并重启引擎。此预览不写入个人词库。"
    }
    if punctuationPreview {
      label.stringValue = "标点预览：选择中文或英文，再点标点按钮。此窗口只检查共享规则，不写入个人词库。"
    }
    if typingPreview {
      label.stringValue = "单按 Shift 或点模式按钮切换。英文直输在此窗口的原生编辑框中演示；中文为候选预览。"
      eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.flagsChanged, .keyDown]) {
        event in
        if shiftTap.observe(
          type: event.type, key: event.keyCode,
          flags: event.modifierFlags, time: event.timestamp)
        {
          toggleTyping()
        }
        return event
      }
    }
    if fixture != nil { label.stringValue = "界面状态样例，内容为测试数据；不代表真实翻译结果。" }
    label.font = NativeTheme.font(13)
    label.textColor = NativeTheme.muted
    label.frame = NSRect(x: 24, y: 115, width: 610, height: 40)
    let output = NSTextField(string: "尚未选择")
    output.isEditable = false
    output.font = NativeTheme.font(16)
    output.textColor = NativeTheme.ink
    output.backgroundColor = NativeTheme.surface
    output.frame = NSRect(x: 24, y: 55, width: 610, height: 36)
    window.contentView?.addSubview(label)
    window.contentView?.addSubview(output)
    let caption = NativeTheme.label("输出预览", size: 11, secondary: true)
    caption.translatesAutoresizingMaskIntoConstraints = true
    window.contentView?.addSubview(caption)
    window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    self.window = window
    text = output
    explanation = label
    outputCaption = caption
    panel = CandidatePanel()
    if CommandLine.arguments.contains("--preview-paging") || learningDirectory != nil {
      state.input = "shi"
    } else {
      state.input =
        CommandLine.arguments.contains("--preview-sentence") || fixture != nil
        ? "wojintianxiangxuexiyingyu" : "xuexi"
    }
    state.frame = try? Runtime.engine?.request(EngineRequest(action: "query", input: state.input))
    panel?.onLearning = {
      window.orderOut(nil)
      panel?.orderOut(nil)
      AccountWindow.start(preview: true)
    }
    panel?.onChinese = { index in
      text?.stringValue = state.frame?.candidates[index].text ?? ""
      punctuation.confirm(.chinese)
      draw()
    }
    panel?.onPage = { delta in
      guard state.changePage(delta) else { return }
      translateSentence()
      draw()
    }
    panel?.onEnglish = { index, sense in
      guard let candidate = state.frame?.candidates[index] else { return }
      if candidate.translations.indices.contains(sense) {
        text?.stringValue = candidate.translations[sense].word
        punctuation.confirm(.english)
      } else if case .ready(let source, let english) = state.sentence, source == candidate.text {
        text?.stringValue = english
        punctuation.confirm(.english)
      }
      draw()
    }
    panel?.onExpand = { index in
      state.active = index
      state.expanded = true
      state.sense = 0
      translateSentence()
      draw()
    }
    panel?.onCollapse = {
      state.expanded = false
      draw()
    }
    panel?.onMode = {
      Runtime.bilingual.toggle()
      if !Runtime.bilingual { punctuation.confirm(.chinese) }
      translateSentence()
      draw()
    }
    panel?.onTypingMode = { toggleTyping() }
    panel?.onPunctuation = {
      Runtime.punctuationMode = Runtime.punctuationMode.next
      punctuation.resetContext()
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
    if let fixture {
      sentenceTranslator.cancel()
      if !Runtime.bilingual || Runtime.typingMode == .english {
        state.sentence = .none
        return
      }
      switch fixture {
      case "empty":
        state.frame = EngineFrame(
          input: state.input, marked: state.input, candidates: [], committed: nil)
      case "loading": state.sentence = .loading
      case "failed": state.sentence = .failed
      case "missing": state.sentence = .missingModels
      case "unavailable": state.sentence = .unavailable
      case "overflow":
        state.input = String(repeating: "wojintianxiangxuexiyingyu", count: 10)
        let chinese = String(repeating: "我想学习英语并了解不同的文化。", count: 6)
        state.frame = EngineFrame(
          input: state.input, marked: state.input,
          candidates: [
            EngineCandidate(text: chinese, syllables: [], translations: [], hasDetails: false)
          ], committed: nil)
        state.sentence = .ready(
          source: chinese,
          english: String(
            repeating: "I want to learn English and understand different cultures. ", count: 20))
      default: break
      }
      return
    }
    guard Runtime.typingMode == .chinese, Runtime.bilingual, let candidate = state.candidate,
      candidate.translations.isEmpty
    else {
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
  static func toggleTyping() {
    window?.makeFirstResponder(nil)
    Runtime.typingMode = Runtime.typingMode.next
    punctuation.resetContext()
    punctuation.confirm(Runtime.typingMode == .english ? .english : .chinese)
    text?.isEditable = typingPreview && Runtime.typingMode == .english
    if Runtime.typingMode == .english { text?.stringValue = "" }
    if text?.isEditable == true { window?.makeFirstResponder(text) }
    translateSentence()
    draw()
  }
  static func draw() {
    guard let window else { return }
    if Runtime.typingMode == .english {
      panel?.showTypingMode(
        .english,
        anchor: NSRect(x: window.frame.minX + 24, y: window.frame.minY, width: 1, height: 20))
    } else {
      panel?.show(
        state, bilingual: Runtime.bilingual,
        anchor: NSRect(x: window.frame.minX + 24, y: window.frame.minY, width: 1, height: 20),
        punctuationLabel: punctuation.label(mode: Runtime.punctuationMode),
        maximumWidth: narrowPreview ? 440 : 560,
        maximumHeight: max(240, (window.screen?.visibleFrame.height ?? 900) - 220))
    }
    // 预览将同一候选视图放进标准窗口，方便检查；此模式不验证宿主焦点。
    guard let panel, let view = panel.contentView else { return }
    panel.orderOut(nil)
    candidateView?.removeFromSuperview()
    let height = panel.frame.height
    let extra: CGFloat = punctuationPreview ? 56 : 0
    let oldFrame = window.frame
    let width = max(panel.frame.width, narrowPreview ? 440 : 560) + 48
    window.setContentSize(NSSize(width: width, height: height + 180 + extra))
    explanation?.frame = NSRect(x: 24, y: height + 127, width: width - 48, height: 40)
    outputCaption?.frame = NSRect(x: 24, y: height + 107 + extra, width: width - 48, height: 16)
    text?.frame = NSRect(x: 24, y: height + 57 + extra, width: width - 48, height: 44)
    explanation?.frame.origin.y += extra
    view.removeFromSuperview()
    window.contentView?.addSubview(view)
    view.frame = NSRect(x: 24, y: 24, width: panel.frame.width, height: height)
    candidateView = view
    punctuationView?.removeFromSuperview()
    if punctuationPreview {
      let controls = NSStackView(
        views: [",", ".", "!", "?", "\"", "(", ")"].map { key in
          ActionButton(key, label: "预览标点 \(key)") {
            if let symbol = punctuation.render(key, mode: Runtime.punctuationMode) {
              text?.stringValue += symbol
            }
          }
        })
      controls.orientation = .horizontal
      controls.spacing = 8
      controls.frame = NSRect(x: 24, y: height + 64, width: 600, height: 44)
      window.contentView?.addSubview(controls)
      punctuationView = controls
    }
    if hasPresented {
      window.setFrameOrigin(NSPoint(x: oldFrame.minX, y: oldFrame.maxY - window.frame.height))
    } else {
      window.center()
      hasPresented = true
    }
  }
}
