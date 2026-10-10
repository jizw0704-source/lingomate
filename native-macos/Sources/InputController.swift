// InputMethodKit 接入：按键交给本地引擎，选中的文字提交到原应用。
import AppKit
import Carbon
import InputMethodKit

@objc(BilingualInputController)
final class BilingualInputController: IMKInputController {
  private var state = SessionState()
  var hasComposition: Bool { !state.input.isEmpty }
  private(set) var isSessionActive = false
  var hasTextClient: Bool { textClient != nil }
  private var punctuation = PunctuationState()
  private var shiftTap = ShiftTap()
  private var modeNotice = UUID()
  private var textClient: IMKTextInput?
  private let learningContext = UUID().uuidString
  private let sentenceTranslator = SentenceTranslator()
  private lazy var panel: CandidatePanel = {
    let panel = CandidatePanel()
    panel.onSettings = { [weak self] in
      self?.rawCommit()
      SettingsWindow.launch()
    }
    panel.onLearning = { [weak self] in
      self?.rawCommit()
      AccountWindow.launch()
    }
    panel.onAISettings = { [weak self] in
      self?.rawCommit()
      AISettingsWindow.launch()
    }
    panel.onChinese = { [weak self] in self?.submit(index: $0) }
    panel.onEnglish = { [weak self] in self?.submit(index: $0, sense: $1) }
    panel.onPage = { [weak self] in self?.changePage($0) }
    panel.onExpand = { [weak self] in
      guard let self else { return }
      self.state.active = $0
      self.state.expanded = true
      self.state.sense = 0
      self.updateSentenceTranslation()
      self.draw()
    }
    panel.onCollapse = { [weak self] in
      self?.state.expanded = false
      self?.draw()
    }
    panel.onMode = { [weak self] in self?.toggleMode(nil) }
    panel.onTypingMode = { [weak self] in self?.toggleTyping(nil) }
    panel.onPunctuation = { [weak self] in self?.cyclePunctuation() }
    panel.onRetryTranslation = { [weak self] in self?.updateSentenceTranslation(force: true) }
    panel.onSetupTranslation = { [weak self] in
      self?.discard()
      TranslationSetup.launch()
    }
    return panel
  }()
  private let noReplacement = NSRange(location: NSNotFound, length: 0)

  private func mainSync<T>(_ operation: () -> T) -> T {
    if Thread.isMainThread { return operation() }
    return DispatchQueue.main.sync(execute: operation)
  }

  override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
    mainSync {
      InputDiagnostics.events += 1
      if event?.type == .keyDown { InputDiagnostics.keyDowns += 1 }
      if event?.type == .flagsChanged { InputDiagnostics.modifierEvents += 1 }
      InputDiagnostics.controller = self
      guard let event, let client = sender as? IMKTextInput else {
        InputDiagnostics.rejectedClients += 1
        return false
      }
      textClient = client
      if IsSecureEventInputEnabled() {
        InputDiagnostics.secureRejections += 1
        discard()
        punctuation.resetContext()
        shiftTap.reset()
        return false
      }
      if [.leftMouseDown, .rightMouseDown, .otherMouseDown].contains(event.type) {
        rawCommit()
        punctuation.resetContext()
        shiftTap.reset()
        return false
      }
      guard event.type == .keyDown || event.type == .flagsChanged else { return false }
      if shiftTap.observe(
        type: event.type, key: event.keyCode,
        flags: event.modifierFlags, time: event.timestamp)
      {
        toggleTyping(nil)
      }
      guard event.type == .keyDown else { return false }
      // Every normal key, including punctuation and shortcuts, belongs to the host in English.
      guard Runtime.typingMode == .chinese else {
        InputDiagnostics.englishPassThroughs += 1
        return false
      }
      let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
      if event.keyCode == kVK_ANSI_P, flags.contains([.control, .shift]),
        !flags.contains(.command), !flags.contains(.option)
      {
        if !event.isARepeat { cyclePunctuation() }
        return true
      }
      if flags.contains(.command) || flags.contains(.control) || flags.contains(.option) {
        rawCommit()
        punctuation.resetContext()
        return false
      }
      if event.keyCode == 97 {
        toggleMode(nil)
        return true
      }
      guard Runtime.engine != nil else { return false }
      switch event.keyCode {
      case 51:
        guard !state.input.isEmpty else {
          punctuation.resetContext()
          return false
        }
        state.removeLastLetter()
        refresh()
        return true
      case 53:
        punctuation.resetContext()
        if state.expanded {
          state.expanded = false
          draw()
          return true
        }
        guard !state.input.isEmpty else { return false }
        discard()
        return true
      case 125, 126:
        guard !state.input.isEmpty else {
          punctuation.resetContext()
          return false
        }
        state.move(event.keyCode == 125 ? 1 : -1)
        updateSentenceTranslation()
        draw()
        return true
      case 116, 121:
        guard !state.input.isEmpty else { return false }
        changePage(event.keyCode == 116 ? -1 : 1)
        return true
      case 48:
        punctuation.clearLiteral()
        guard !state.input.isEmpty, Runtime.bilingual else {
          rawCommit()
          return false
        }
        if state.expanded {
          state.moveSense(flags.contains(.shift) ? -1 : 1)
        } else {
          state.expanded = true
          state.sense = 0
        }
        draw()
        return true
      case 49:
        punctuation.clearLiteral()
        guard !state.input.isEmpty else { return false }
        if !event.isARepeat {
          submit(
            index: state.active,
            sense: flags.contains(.shift) && Runtime.bilingual ? state.sense : nil)
        }
        return true
      case 36, 76:
        punctuation.resetContext()
        guard !state.input.isEmpty else { return false }
        rawCommit()
        // 原样单词已经结束，下一组字母重新进入中文拼音；英文标点语言保留。
        punctuation.clearLiteral()
        return true
      case 115, 119, 123, 124:
        punctuation.resetContext()
        guard !state.input.isEmpty else { return false }
        rawCommit()
        return false
      default: break
      }
      guard let characters = event.characters, !characters.isEmpty else {
        InputDiagnostics.emptyCharacterEvents += 1
        return false
      }
      if state.input.isEmpty, punctuation.isLatinLiteral,
        characters.utf8.allSatisfy({ (33...126).contains($0) })
      {
        punctuation.observeLiteral(characters)
        return false
      }
      if !state.input.isEmpty, !flags.contains(.shift), characters == "-" || characters == "=" {
        changePage(characters == "-" ? -1 : 1)
        return true
      }
      if !state.input.isEmpty, let digit = Int(characters), (1...5).contains(digit) {
        if !event.isARepeat {
          if let index = state.indexOnPage(digit - 1) {
            submit(index: index)
          } else {
            NSSound.beep()
          }
        }
        return true
      }
      if let action = PunctuationState.action(
        key: characters, input: state.input,
        hasCandidate: state.candidate != nil, mode: Runtime.punctuationMode)
      {
        return submitPunctuation(characters, action: action)
      }
      if state.appendLetters(characters) {
        refresh()
        return true
      }
      rawCommit()
      punctuation.observeLiteral(characters)
      return false
    }
  }

  override func recognizedEvents(_ sender: Any!) -> Int { Int(ShiftTap.events.rawValue) }

  private func changePage(_ delta: Int) {
    guard state.changePage(delta) else { return }
    updateSentenceTranslation()
    draw()
  }

  private func refresh() {
    if state.input.isEmpty {
      Runtime.engine?.cancelLearning(context: learningContext)
      sentenceTranslator.cancel()
      state.sentence = .none
      textClient?.setMarkedText(
        "", selectionRange: NSRange(location: 0, length: 0), replacementRange: noReplacement)
      panel.orderOut(nil)
      return
    }
    do {
      guard let engine = Runtime.engine else { throw EngineFailure.unavailable }
      state.frame = try engine.request(
        EngineRequest(action: "query", input: state.input, context: learningContext))
      Runtime.failure = nil
      Runtime.memoryWarning = state.frame?.memoryWarning
      let marked = state.markedInput
      if let textClient {
        textClient.setMarkedText(
          marked, selectionRange: NSRange(location: marked.utf16.count, length: 0),
          replacementRange: noReplacement)
        InputDiagnostics.markedUpdates += 1
      }
      updateSentenceTranslation()
      draw()
    } catch {
      Runtime.failure = error.localizedDescription
      rawCommit()
    }
  }

  private func updateSentenceTranslation(force: Bool = false) {
    guard Runtime.bilingual, !state.input.isEmpty, let candidate = state.candidate,
      candidate.translations.isEmpty,
      candidate.text.count >= 3 || (candidate.personal == true && candidate.text.count >= 2)
    else {
      sentenceTranslator.cancel()
      state.sentence = .none
      return
    }
    if force { sentenceTranslator.cancel() }
    let input = state.input
    let index = state.active
    let source = candidate.text
    sentenceTranslator.request(key: "\(input)|\(index)|\(source)", source: source) {
      [weak self] phase in
      guard let self, self.state.input == input, self.state.active == index,
        self.state.candidate?.text == source, Runtime.bilingual
      else { return }
      self.state.sentence = phase
      self.draw()
    }
  }

  private func draw() {
    modeNotice = UUID()
    guard Runtime.typingMode == .chinese, !state.input.isEmpty, let client = textClient else {
      panel.orderOut(nil)
      return
    }
    var anchor = NSRect.zero
    _ = client.attributes(forCharacterIndex: 0, lineHeightRectangle: &anchor)
    if anchor == .zero {
      anchor = NSRect(origin: NSEvent.mouseLocation, size: NSSize(width: 1, height: 20))
    }
    panel.show(
      state, bilingual: Runtime.bilingual, anchor: anchor,
      punctuationLabel: punctuation.label(mode: Runtime.punctuationMode))
  }

  @discardableResult private func submit(index: Int, sense: Int? = nil) -> Bool {
    guard let textClient, !IsSecureEventInputEnabled() else {
      discard()
      return false
    }
    guard let candidates = state.frame?.candidates, candidates.indices.contains(index) else {
      NSSound.beep()
      return false
    }
    let candidate = candidates[index]
    let account = LearningRuntime.account()
    var request = EngineRequest(
      action: "commit", input: state.input, candidate: candidate.text,
      syllables: candidate.syllables, context: learningContext)
    var sentenceEnglish: String?
    if let sense {
      guard Runtime.bilingual else { return false }
      if candidate.translations.isEmpty {
        guard sense == 0, index == state.active,
          case .ready(let source, let english) = state.sentence, source == candidate.text
        else {
          NSSound.beep()
          return false
        }
        // 译文只来自当前选定翻译任务；用中文候选消耗拼音，不放宽桥接器的译词校验。
        sentenceEnglish = english
      } else if candidate.translations.indices.contains(sense) {
        request.english = candidate.translations[sense].word
      } else {
        NSSound.beep()
        return false
      }
    }
    do {
      guard let engine = Runtime.engine else { throw EngineFailure.unavailable }
      let next = try engine.request(request)
      guard let committed = next.committed else { throw EngineFailure.message("该候选暂不能提交。") }
      if sentenceEnglish != nil, committed != candidate.text {
        throw EngineFailure.message("中文候选已变化，请重新选择。")
      }
      // insertText replaces the marked composition in the host, never the clipboard.
      let remaining = state.rawRemainder(for: next.input)
      state.clear()
      sentenceTranslator.cancel()
      textClient.insertText(sentenceEnglish ?? committed, replacementRange: noReplacement)
      InputDiagnostics.insertCalls += 1
      punctuation.confirm(sense == nil ? .chinese : .english)
      if let sense, candidate.translations.indices.contains(sense), sentenceEnglish == nil {
        LearningRuntime.confirmed(
          candidate.translations[sense], chinese: candidate.text, account: account)
      }
      if Runtime.remember {
        engine.confirmSelection(next, context: learningContext, chinese: sense == nil)
      } else {
        engine.cancelLearning(context: learningContext)
      }
      state.replaceInput(next.input, original: remaining)
      state.frame = next
      refresh()
      return true
    } catch {
      Runtime.failure = error.localizedDescription
      NSSound.beep()
      return false
    }
  }

  private func submitPunctuation(_ key: String, action: PunctuationAction) -> Bool {
    switch action {
    case .raw: rawCommit()
    case .commitChinese:
      guard submit(index: state.active) else { return true }
      // A partial candidate may have been output; retain its remaining composition.
      guard state.input.isEmpty else {
        NSSound.beep()
        return true
      }
    case .symbol: break
    }
    guard let symbol = punctuation.render(key, mode: Runtime.punctuationMode) else { return false }
    if let textClient {
      textClient.insertText(symbol, replacementRange: noReplacement)
      InputDiagnostics.insertCalls += 1
    }
    return true
  }

  private func rawCommit() {
    if !state.input.isEmpty { Runtime.engine?.cancelLearning(context: learningContext) }
    sentenceTranslator.cancel()
    let raw = state.takeRawInput()
    panel.orderOut(nil)
    if !raw.isEmpty, let textClient {
      textClient.insertText(raw, replacementRange: noReplacement)
      InputDiagnostics.insertCalls += 1
      punctuation.observeLiteral(raw)
    }
  }

  private func discard() {
    Runtime.engine?.cancelLearning(context: learningContext)
    sentenceTranslator.cancel()
    state.clear()
    panel.orderOut(nil)
    textClient?.setMarkedText(
      "", selectionRange: NSRange(location: 0, length: 0), replacementRange: noReplacement)
  }

  override func activateServer(_ sender: Any!) {
    mainSync {
      InputDiagnostics.activations += 1
      InputDiagnostics.controller = self
      shiftTap.reset()
      textClient = (sender as? IMKTextInput) ?? client()
      isSessionActive = true
      // 每次激活明确指定会话的字母布局；不切换系统输入源或修改全局键盘设置。
      textClient?.overrideKeyboard(withKeyboardNamed: "com.apple.keylayout.ABC")
    }
  }

  override func deactivateServer(_ sender: Any!) {
    mainSync {
      InputDiagnostics.deactivations += 1
      isSessionActive = false
      rawCommit()
      shiftTap.reset()
      modeNotice = UUID()
      punctuation.resetContext()
      textClient = nil
    }
  }

  override func commitComposition(_ sender: Any!) {
    mainSync {
      if let client = sender as? IMKTextInput { textClient = client }
      rawCommit()
    }
  }

  override func hidePalettes() {
    mainSync {
      modeNotice = UUID()
      panel.orderOut(nil)
    }
  }

  override func menu() -> NSMenu! {
    mainSync {
      let menu = NSMenu(title: "灵果")
      let settings = NSMenuItem(
        title: "设置…", action: #selector(openSettings(_:)), keyEquivalent: "")
      settings.target = self
      menu.addItem(settings)
      let updates = NSMenuItem(
        title: "检查更新…", action: #selector(openUpdates(_:)), keyEquivalent: "")
      updates.target = self
      menu.addItem(updates)
      let typing = NSMenuItem(
        title: "\(Runtime.typingMode.title) · 单按 Shift 切换",
        action: #selector(toggleTyping(_:)), keyEquivalent: "")
      typing.target = self
      menu.addItem(typing)
      let mode = NSMenuItem(
        title: Runtime.bilingual ? "中英候选（已开启）" : "开启中英候选", action: #selector(toggleMode(_:)),
        keyEquivalent: "")
      mode.target = self
      mode.isEnabled = Runtime.typingMode == .chinese
      mode.state = Runtime.bilingual ? .on : .off
      menu.addItem(mode)
      let punctuationItem = NSMenuItem(
        title: punctuation.label(mode: Runtime.punctuationMode),
        action: nil, keyEquivalent: "")
      let choices = NSMenu(title: "标点")
      for choice in PunctuationMode.allCases {
        let item = NSMenuItem(
          title: choice.title, action: #selector(setPunctuation(_:)), keyEquivalent: "")
        item.tag = choice.rawValue
        item.target = self
        item.state = Runtime.punctuationMode == choice ? .on : .off
        choices.addItem(item)
      }
      punctuationItem.submenu = choices
      punctuationItem.isEnabled = Runtime.typingMode == .chinese
      menu.addItem(punctuationItem)
      let appearance = NSMenuItem(title: "外观", action: nil, keyEquivalent: "")
      appearance.submenu = AppearanceSettings.current.menu(
        target: self, action: #selector(setAppearance(_:)))
      menu.addItem(appearance)
      let ai = NSMenuItem(
        title: "AI 翻译设置…", action: #selector(openAISettings(_:)), keyEquivalent: "")
      ai.target = self
      menu.addItem(ai)
      let learning = NSMenuItem(
        title: "登录与学习记录…", action: #selector(openLearning(_:)), keyEquivalent: "")
      learning.target = self
      menu.addItem(learning)
      if let warning = LearningRuntime.warning {
        let item = NSMenuItem(title: warning, action: nil, keyEquivalent: "")
        item.isEnabled = false
        menu.addItem(item)
      }
      let setup = NSMenuItem(
        title: "准备本地整句翻译…", action: #selector(prepareTranslation(_:)), keyEquivalent: "")
      setup.target = self
      menu.addItem(setup)
      let memory = NSMenuItem(
        title: Runtime.remember ? "选词记忆（本次开启）" : "选词记忆（本次暂停）", action: #selector(toggleMemory(_:)),
        keyEquivalent: "")
      memory.target = self
      memory.state = Runtime.remember ? .on : .off
      menu.addItem(memory)
      if let warning = Runtime.memoryWarning {
        let item = NSMenuItem(title: warning, action: nil, keyEquivalent: "")
        item.isEnabled = false
        menu.addItem(item)
      }
      if let failure = Runtime.failure {
        let item = NSMenuItem(title: failure, action: nil, keyEquivalent: "")
        item.isEnabled = false
        menu.addItem(item)
      }
      menu.addItem(NSMenuItem(title: "实验版 · 本地引擎 · 30 词用法示例", action: nil, keyEquivalent: ""))
      return menu
    }
  }

  @objc private func openLearning(_ sender: Any?) {
    mainSync {
      rawCommit()
      AccountWindow.launch()
    }
  }
  @objc private func openSettings(_ sender: Any?) {
    mainSync {
      rawCommit()
      SettingsWindow.launch()
    }
  }
  @objc private func openUpdates(_ sender: Any?) {
    mainSync {
      rawCommit()
      MacUpdates.launch()
    }
  }

  func applySetting(_ setting: InputSetting) {
    mainSync {
      if IsSecureEventInputEnabled() { discard() } else { rawCommit() }
      shiftTap.reset()
      modeNotice = UUID()
      setting.apply()
      punctuation.resetContext()
      punctuation.confirm(Runtime.typingMode == .english ? .english : .chinese)
      panel.orderOut(nil)
    }
  }
  @objc private func openAISettings(_ sender: Any?) {
    mainSync {
      rawCommit()
      AISettingsWindow.launch()
    }
  }

  @objc private func setAppearance(_ sender: NSMenuItem) {
    mainSync { AppearanceSettings.current.choose(sender) }
  }

  @objc private func toggleMode(_ sender: Any?) {
    Runtime.bilingual.toggle()
    if !Runtime.bilingual { punctuation.confirm(.chinese) }
    state.expanded = false
    state.sense = 0
    updateSentenceTranslation()
    draw()
  }

  @objc private func toggleTyping(_ sender: Any?) {
    mainSync {
      rawCommit()
      shiftTap.reset()
      Runtime.typingMode = Runtime.typingMode.next
      punctuation.resetContext()
      punctuation.confirm(Runtime.typingMode == .english ? .english : .chinese)
      let notice = UUID()
      modeNotice = notice
      guard let client = textClient else { return }
      var anchor = NSRect.zero
      _ = client.attributes(forCharacterIndex: 0, lineHeightRectangle: &anchor)
      if anchor == .zero {
        anchor = NSRect(origin: NSEvent.mouseLocation, size: NSSize(width: 1, height: 20))
      }
      panel.showTypingMode(Runtime.typingMode, anchor: anchor)
      DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
        guard let self, self.modeNotice == notice else { return }
        self.panel.orderOut(nil)
      }
    }
  }

  private func cyclePunctuation() {
    Runtime.punctuationMode = Runtime.punctuationMode.next
    punctuation.resetContext()
    draw()
  }

  @objc private func setPunctuation(_ sender: NSMenuItem) {
    mainSync {
      guard let mode = PunctuationMode(rawValue: sender.tag) else { return }
      Runtime.punctuationMode = mode
      punctuation.resetContext()
      draw()
    }
  }

  @objc private func toggleMemory(_ sender: Any?) {
    mainSync {
      Runtime.remember.toggle()
      Runtime.engine?.cancelLearning(context: learningContext)
    }
  }

  @objc private func prepareTranslation(_ sender: Any?) {
    mainSync {
      discard()
      TranslationSetup.launch()
    }
  }
}
