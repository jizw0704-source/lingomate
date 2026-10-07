// InputMethodKit 接入：按键交给本地引擎，选中的文字提交到原应用。
import AppKit
import Carbon
import InputMethodKit

@objc(BilingualInputController)
final class BilingualInputController: IMKInputController {
  private var state = SessionState()
  private var textClient: IMKTextInput?
  private let learningContext = UUID().uuidString
  private let sentenceTranslator = SentenceTranslator()
  private lazy var panel: CandidatePanel = {
    let panel = CandidatePanel()
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
      guard let event, let client = sender as? IMKTextInput, event.type == .keyDown else {
        return false
      }
      textClient = client
      if IsSecureEventInputEnabled() {
        discard()
        return false
      }
      let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
      if flags.contains(.command) || flags.contains(.control) || flags.contains(.option) {
        rawCommit()
        return false
      }
      if event.keyCode == 97 {
        toggleMode(nil)
        return true
      }
      guard Runtime.engine != nil else { return false }
      switch event.keyCode {
      case 51:
        guard !state.input.isEmpty else { return false }
        state.input.removeLast()
        state.resetSelection()
        refresh()
        return true
      case 53:
        if state.expanded {
          state.expanded = false
          draw()
          return true
        }
        guard !state.input.isEmpty else { return false }
        discard()
        return true
      case 125, 126:
        guard !state.input.isEmpty else { return false }
        state.move(event.keyCode == 125 ? 1 : -1)
        updateSentenceTranslation()
        draw()
        return true
      case 116, 121:
        guard !state.input.isEmpty else { return false }
        changePage(event.keyCode == 116 ? -1 : 1)
        return true
      case 48:
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
        guard !state.input.isEmpty else { return false }
        if !event.isARepeat {
          submit(
            index: state.active,
            sense: flags.contains(.shift) && Runtime.bilingual ? state.sense : nil)
        }
        return true
      case 36, 76:
        guard !state.input.isEmpty else { return false }
        if state.expanded && Runtime.bilingual {
          submit(index: state.active, sense: state.sense)
        } else {
          rawCommit()
        }
        return true
      case 123, 124:
        guard !state.input.isEmpty else { return false }
        rawCommit()
        return false
      default: break
      }
      guard let characters = event.characters, !characters.isEmpty else { return false }
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
      let lower = characters.lowercased()
      if lower.utf8.allSatisfy({ (97...122).contains($0) || $0 == 39 }),
        state.input.count + lower.count <= 240
      {
        state.input += lower
        state.resetSelection()
        refresh()
        return true
      }
      rawCommit()
      return false
    }
  }

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
      Runtime.memoryWarning = state.frame?.memoryWarning
      let marked = state.frame?.marked.isEmpty == false ? state.frame!.marked : state.input
      textClient?.setMarkedText(
        marked, selectionRange: NSRange(location: marked.utf16.count, length: 0),
        replacementRange: noReplacement)
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
    guard !state.input.isEmpty, let client = textClient else {
      panel.orderOut(nil)
      return
    }
    var anchor = NSRect.zero
    _ = client.attributes(forCharacterIndex: 0, lineHeightRectangle: &anchor)
    if anchor == .zero {
      anchor = NSRect(origin: NSEvent.mouseLocation, size: NSSize(width: 1, height: 20))
    }
    panel.show(state, bilingual: Runtime.bilingual, anchor: anchor)
  }

  private func submit(index: Int, sense: Int? = nil) {
    guard textClient != nil, !IsSecureEventInputEnabled() else {
      discard()
      return
    }
    guard let candidates = state.frame?.candidates, candidates.indices.contains(index) else {
      NSSound.beep()
      return
    }
    let candidate = candidates[index]
    var request = EngineRequest(
      action: "commit", input: state.input, candidate: candidate.text,
      syllables: candidate.syllables, context: learningContext)
    var sentenceEnglish: String?
    if let sense {
      guard Runtime.bilingual else { return }
      if candidate.translations.isEmpty {
        guard sense == 0, index == state.active,
          case .ready(let source, let english) = state.sentence, source == candidate.text
        else {
          NSSound.beep()
          return
        }
        // 译文只来自当前本地任务；用中文候选消耗拼音，不放宽桥接器的译词校验。
        sentenceEnglish = english
      } else if candidate.translations.indices.contains(sense) {
        request.english = candidate.translations[sense].word
      } else {
        NSSound.beep()
        return
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
      state.clear()
      sentenceTranslator.cancel()
      textClient?.insertText(sentenceEnglish ?? committed, replacementRange: noReplacement)
      if Runtime.remember {
        engine.confirmSelection(next, context: learningContext, chinese: sense == nil)
      } else {
        engine.cancelLearning(context: learningContext)
      }
      state.input = next.input
      state.frame = next
      refresh()
    } catch {
      Runtime.failure = error.localizedDescription
      NSSound.beep()
    }
  }

  private func rawCommit() {
    if !state.input.isEmpty { Runtime.engine?.cancelLearning(context: learningContext) }
    sentenceTranslator.cancel()
    let raw = state.input
    state.clear()
    panel.orderOut(nil)
    if !raw.isEmpty { textClient?.insertText(raw, replacementRange: noReplacement) }
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
    mainSync { textClient = sender as? IMKTextInput }
  }

  override func deactivateServer(_ sender: Any!) {
    mainSync {
      rawCommit()
      textClient = nil
    }
  }

  override func commitComposition(_ sender: Any!) {
    mainSync {
      if let client = sender as? IMKTextInput { textClient = client }
      rawCommit()
    }
  }

  override func hidePalettes() { mainSync { panel.orderOut(nil) } }

  override func menu() -> NSMenu! {
    mainSync {
      let menu = NSMenu(title: "中英输入实验版")
      let mode = NSMenuItem(
        title: Runtime.bilingual ? "中英候选（已开启）" : "开启中英候选", action: #selector(toggleMode(_:)),
        keyEquivalent: "")
      mode.target = self
      mode.state = Runtime.bilingual ? .on : .off
      menu.addItem(mode)
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

  @objc private func toggleMode(_ sender: Any?) {
    Runtime.bilingual.toggle()
    state.expanded = false
    state.sense = 0
    updateSentenceTranslation()
    draw()
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
