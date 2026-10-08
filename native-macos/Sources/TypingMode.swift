// 单按 Shift 才切换；组合键、长按、焦点边界不触发切换。
import AppKit
import Carbon

enum TypingMode {
  case chinese, english
  var next: Self { self == .chinese ? .english : .chinese }
  var title: String { self == .chinese ? "中文拼音" : "英文直输" }
}

struct ShiftTap {
  static let events: NSEvent.EventTypeMask = [
    .keyDown, .flagsChanged, .leftMouseDown,
    .rightMouseDown, .otherMouseDown,
  ]
  private var held: Set<UInt16> = []
  private var started: TimeInterval?
  private var eligible = false

  mutating func reset() {
    held.removeAll()
    started = nil
    eligible = false
  }

  mutating func observe(
    type: NSEvent.EventType, key: UInt16, flags: NSEvent.ModifierFlags,
    time: TimeInterval
  ) -> Bool {
    guard type == .flagsChanged else {
      eligible = false
      return false
    }
    guard key == kVK_Shift || key == kVK_RightShift else {
      eligible = false
      return false
    }
    let modifiers = flags.intersection([.shift, .control, .option, .command, .function])
    if modifiers.contains(.shift) {
      if held.contains(key) {  // One Shift released while the other remains held.
        held.remove(key)
        eligible = false
      } else {
        held.insert(key)
        if held.count == 1 {
          started = time
          eligible = modifiers == .shift
        } else {
          eligible = false
        }
      }
      return false
    }
    let duration = time - (started ?? time)
    let toggle =
      held.contains(key) && eligible && modifiers.isEmpty
      && duration >= 0 && duration <= 0.7
    reset()
    return toggle
  }
}
