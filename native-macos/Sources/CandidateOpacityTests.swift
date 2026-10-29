// No real preferences, input service, host text, clipboard or personal data.
import AppKit

enum CandidateOpacityTests {
  static func readback(_ suite: String, expected: String) {
    precondition(suite.hasPrefix("org.local.bilingualcompanion.test-opacity."))
    let preferences = CandidatePreferences(store: UserDefaults(suiteName: suite)!)
    precondition(String(preferences.opacity) == expected)
  }
  static func notificationProbe(_ suite: String) {
    precondition(suite.hasPrefix("org.local.bilingualcompanion.test-opacity."))
    let preferences = CandidatePreferences(
      store: UserDefaults(suiteName: suite)!, opacityChannel: Notification.Name(suite))
    preferences.observeOpacity()
    print("ready")
    fflush(stdout)
    let deadline = Date().addingTimeInterval(5)
    while preferences.opacity != 70 && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    }
    precondition(preferences.opacity == 70)
  }
  static func run() {
    let previous = CandidatePreferences.current
    let theme = AppearanceSettings.current.choice
    defer {
      CandidatePreferences.current = previous
      AppearanceSettings.current.select(theme)
    }
    let name = "org.local.bilingualcompanion.test-opacity.\(UUID().uuidString)"
    let store = UserDefaults(suiteName: name)!
    defer { store.removePersistentDomain(forName: name) }
    for invalid in [-1, 0, 69, 101, 900] {
      store.set(invalid, forKey: CandidatePreferences.opacityKey)
      precondition(CandidatePreferences(store: store).opacity == 100)
    }
    store.removeObject(forKey: CandidatePreferences.opacityKey)
    let preferences = CandidatePreferences(store: store)
    precondition(preferences.opacity == 100)
    CandidatePreferences.current = preferences
    for value in [70, 83, 100] {
      precondition(preferences.selectOpacity(value))
      precondition(CandidatePreferences(store: store).opacity == value)
      store.synchronize()
      let process = Process()
      process.executableURL = Bundle.main.executableURL
      process.arguments = ["--opacity-readback-test", name, String(value)]
      try! process.run()
      process.waitUntilExit()
      precondition(process.terminationStatus == 0)
      precondition(CandidateOpacity.effective(value, reduceTransparency: false) == value)
      precondition(CandidateOpacity.effective(value, reduceTransparency: true) == 100)
    }
    precondition(!preferences.selectOpacity(69) && !preferences.selectOpacity(101))
    precondition(preferences.opacity == 100)
    for invalid: [AnyHashable: Any] in [
      ["percent": 69], ["percent": 101], ["percent": "70"], ["percent": true], ["percent": 70.5],
      ["percent": 70, "extra": 1],
    ] {
      precondition(CandidatePreferences.opacityValue(invalid) == nil)
    }
    precondition(CandidatePreferences.opacityValue(["percent": 70]) == 70)
    // Cross-process delivery uses a unique channel, never the real running service.
    let recipient = Process()
    let ready = Pipe()
    recipient.executableURL = Bundle.main.executableURL
    recipient.arguments = ["--opacity-notification-test", name]
    recipient.standardOutput = ready
    try! recipient.run()
    let signal = ready.fileHandleForReading.availableData
    precondition(String(decoding: signal, as: UTF8.self).contains("ready"))
    let sender = CandidatePreferences(
      store: store, broadcastsChanges: true, opacityChannel: Notification.Name(name))
    sender.observeOpacity()
    sender.selectOpacity(70)
    recipient.waitUntilExit()
    precondition(recipient.terminationStatus == 0)
    for value in [83, 100, 70] { sender.selectOpacity(value) }
    RunLoop.current.run(until: Date().addingTimeInterval(0.1))
    precondition(sender.opacity == 70)  // Own delayed notifications must not rewind the slider.
    // Both themes and widths use the real panel. Opacity never changes alphaValue,
    // candidate indices, page, raw casing, marked input or focus policy.
    let candidate = EngineCandidate(
      text: "学习", syllables: ["xue", "xi"], translations: [], hasDetails: false)
    let state = SessionState(
      input: "xuexi", active: 5,
      frame: EngineFrame(
        input: "xuexi", marked: "xuexi", candidates: Array(repeating: candidate, count: 10),
        committed: nil))
    let panel = CandidatePanel()
    defer { panel.orderOut(nil) }
    var selected: Int?
    panel.onChinese = { selected = $0 }
    for appearance in [AppearanceChoice.light, .dark] {
      AppearanceSettings.current.select(appearance)
      for width: CGFloat in [560, 440] {
        panel.show(
          state, bilingual: true, anchor: NSRect(x: 100, y: 400, width: 1, height: 20),
          maximumWidth: width)
        panel.contentView!.layoutSubtreeIfNeeded()
        let frame = panel.frame
        let root = panel.contentView as! CandidateSurfaceView
        precondition(allViews(root).filter { $0 is CandidateSurfaceView }.count == 1)
        for value in [70, 83, 100] {
          preferences.selectOpacity(value)
          precondition(
            abs(root.fill!.alphaComponent - CGFloat(CandidateOpacity.effectivePercent) / 100)
              < 0.001)
          precondition(panel.alphaValue == 1 && panel.frame == frame)
          precondition(allViews(root).allSatisfy { $0.alphaValue == 1 })
          precondition(!panel.canBecomeKey && !panel.canBecomeMain && !panel.isKeyWindow)
          let button = allViews(root).compactMap { $0 as? ActionButton }.first {
            $0.slotPrefix == "1 "
          }!
          button.performClick(nil)
          precondition(selected == 5 && state.active == 5 && state.input == "xuexi")
        }
      }
    }
    // Slider changes preview and saved percent immediately, without remounting it.
    let control = CandidateOpacityControl()
    control.slider.doubleValue = 70
    control.changed()
    precondition(preferences.opacity == 70 && control.valueLabel.stringValue == "70%")
    precondition(control.slider.isContinuous && control.slider.numberOfTickMarks == 31)
    let right = NSEvent.keyEvent(
      with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0,
      context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: 124)!
    control.slider.keyDown(with: right)
    precondition(preferences.opacity == 71 && control.valueLabel.stringValue == "71%")
    control.slider.integerValue = 100
    control.slider.keyDown(with: right)
    precondition(preferences.opacity == 100)
    let keyWindow = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: .titled,
      backing: .buffered, defer: false)
    keyWindow.contentView = control
    precondition(keyWindow.makeFirstResponder(control.slider))
    precondition(keyWindow.firstResponder === control.slider)
    keyWindow.orderOut(nil)
    // Worst-case host backdrop: light candidate over black, dark over white.
    for (ink, background, backdrop) in [
      (0x101010, 0xFFFFFF, 0x000000), (0xF7F7F7, 0x171717, 0xFFFFFF),
    ] {
      let alpha = 0.7
      let channels: (Int) -> [Double] = { hex in [16, 8, 0].map { Double((hex >> $0) & 255) / 255 }
      }
      let mixed = zip(channels(background), channels(backdrop)).map {
        alpha * $0 + (1 - alpha) * $1
      }
      let luminance: ([Double]) -> Double = { rgb in
        let linear = rgb.map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
        return zip(linear, [0.2126, 0.7152, 0.0722]).map(*).reduce(0, +)
      }
      let a = luminance(channels(ink))
      let b = luminance(mixed)
      precondition((max(a, b) + 0.05) / (min(a, b) + 0.05) >= 4.5)
    }
    print("PASS 候选背景70–100、默认/非法值/独立进程持久化、实时预览、系统减少透明度策略、深浅宽窄、文字不变淡、最差背景对比度、焦点与绝对索引不变")
  }
  private static func allViews(_ view: NSView) -> [NSView] {
    [view] + view.subviews.flatMap(allViews)
  }
}
