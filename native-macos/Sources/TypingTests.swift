// 真实使用的 Shift 状态机与隔离引擎检查，不能代替 IMK 宿主键盘验收。
import AppKit
import Carbon

enum TypingTests {
  static func run() {
    for key in [UInt16(kVK_Shift), UInt16(kVK_RightShift)] {
      var tap = ShiftTap()
      precondition(!tap.observe(type: .flagsChanged, key: key, flags: .shift, time: 1))
      precondition(tap.observe(type: .flagsChanged, key: key, flags: [], time: 1.1))
      precondition(!tap.observe(type: .flagsChanged, key: key, flags: [], time: 1.2))
    }
    for key in [kVK_ANSI_A, kVK_Space, kVK_Tab, kVK_ANSI_P, kVK_Return] {
      var tap = ShiftTap()
      _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 1)
      precondition(!tap.observe(type: .keyDown, key: UInt16(key), flags: .shift, time: 1.1))
      precondition(!tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 1.2))
    }
    for modifier: NSEvent.ModifierFlags in [.command, .control, .option, .function] {
      var tap = ShiftTap()
      _ = tap.observe(
        type: .flagsChanged, key: UInt16(kVK_Shift), flags: [.shift, modifier], time: 1)
      _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Command), flags: .shift, time: 1.1)
      precondition(!tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 1.2))
      _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 2)
      precondition(tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 2.1))
    }
    var tap = ShiftTap()
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 1)
    precondition(!tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 2))
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 3)
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_RightShift), flags: .shift, time: 3.1)
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 3.2)
    precondition(
      !tap.observe(type: .flagsChanged, key: UInt16(kVK_RightShift), flags: [], time: 3.3))
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 4)
    tap.reset()
    precondition(!tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 4.1))
    _ = tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .shift, time: 5)
    _ = tap.observe(type: .leftMouseDown, key: 0, flags: .shift, time: 5.1)
    precondition(!tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: [], time: 5.2))
    _ = tap.observe(
      type: .flagsChanged, key: UInt16(kVK_Shift), flags: [.shift, .capsLock], time: 6)
    precondition(
      tap.observe(type: .flagsChanged, key: UInt16(kVK_Shift), flags: .capsLock, time: 6.1))
    for event: NSEvent.EventTypeMask in [.keyDown, .flagsChanged, .leftMouseDown, .rightMouseDown] {
      precondition(ShiftTap.events.contains(event))
    }
    precondition(TypingMode.chinese.next == .english && TypingMode.english.next == .chinese)
    engineRecovery()
    print("PASS 左右单 Shift、组合键/Shift空格/大小写保护、长按/双Shift/焦点/鼠标边界及中文恢复")
  }

  private static func engineRecovery() {
    let engine = try! EngineClient(resources: Bundle.main.resourceURL!)
    defer { engine.terminate() }
    _ = try! engine.request(EngineRequest(action: "query", input: "nihao", context: "test"))
    // Mode switch cancels learning and emits raw composition, without committing a candidate.
    _ = try! engine.request(EngineRequest(action: "cancel", input: "", context: "test"))
    let frame = try! engine.request(EngineRequest(action: "query", input: "xuexi", context: "test"))
    precondition(frame.candidates.first?.text == "学习")
    let word = frame.candidates[0]
    let english = try! engine.request(
      EngineRequest(
        action: "commit", input: "xuexi",
        candidate: word.text, syllables: word.syllables, english: "learn"))
    precondition(english.committed == "learn" && english.input.isEmpty)
  }
}
