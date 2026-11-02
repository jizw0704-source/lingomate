// 仅保存当前输入会话成功确认的中文尾部；不读宿主正文、不持久化或联网。
import Foundation

struct ConfirmedContext {
  private var tail = ""
  private var updated: TimeInterval = 0
  private var identity: UUID?
  mutating func reset() {
    tail = ""
    updated = 0
    identity = nil
  }
  mutating func confirmed(
    _ text: String, chinese: Bool, identity: UUID?,
    now: TimeInterval = ProcessInfo.processInfo.systemUptime
  ) {
    guard chinese, !text.isEmpty,
      text.unicodeScalars.allSatisfy({ (0x3400...0x9FFF).contains($0.value) })
    else {
      reset()
      return
    }
    if self.identity != identity || now - updated > 60 || now < updated { reset() }
    tail = String((tail + text).suffix(32))
    updated = now
    self.identity = identity
  }
  mutating func preceding(identity: UUID?, now: TimeInterval = ProcessInfo.processInfo.systemUptime)
    -> String?
  {
    guard !tail.isEmpty, self.identity == identity, now >= updated, now - updated <= 60 else {
      reset()
      return nil
    }
    return tail
  }
}
