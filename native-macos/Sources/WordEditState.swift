// 只跟踪本输入法已提交字的范围；不读取宿主正文。删除须由后续选区元数据确认。
import Foundation

struct WordEditState {
  struct Deletion: Equatable {
    let offset: Int
    let length: Int
    let expectedSelection: NSRange
  }
  enum Resolution {
    case unchanged
    case deleted(Deletion)
    case invalid
  }
  private(set) var start: Int?
  private(set) var count = 0
  private var pending: Deletion?
  private var lastChange: TimeInterval?

  mutating func reset() { self = Self() }

  mutating func discardIfGenerationChanged(previous: UUID?, current: UUID?) -> Bool {
    guard start != nil else { return false }
    guard let previous, let current, previous == current else {
      reset()
      return true
    }
    return false
  }

  func expired(now: TimeInterval = ProcessInfo.processInfo.systemUptime) -> Bool {
    guard let lastChange else { return false }
    return now < lastChange || now - lastChange > 20
  }

  func contains(_ selection: NSRange) -> Bool {
    guard let start, selection.location != NSNotFound, selection.length >= 0,
      selection.location >= start, selection.location <= start + count
    else { return false }
    return selection.length <= start + count - selection.location
  }

  func canAppend(at location: Int, count addition: Int) -> Bool {
    guard location != NSNotFound, location >= 0, addition > 0, addition <= 16,
      location < Int.max - addition, pending == nil
    else { return false }
    guard let start else { return true }
    return location == start + count && count + addition <= 16
  }

  func isAtEnd(_ selection: NSRange) -> Bool {
    guard let start, count > 0 else { return false }
    return pending == nil && selection == NSRange(location: start + count, length: 0)
  }

  mutating func record(at location: Int, count addition: Int, selectionAfter: NSRange) -> Bool {
    guard canAppend(at: location, count: addition),
      selectionAfter == NSRange(location: location + addition, length: 0)
    else {
      reset()
      return false
    }
    if start == nil { start = location }
    count += addition
    lastChange = ProcessInfo.processInfo.systemUptime
    return true
  }

  mutating func prepareBackspace(selection: NSRange) -> Bool {
    guard pending == nil, contains(selection), let start else { return false }
    let location: Int
    let length: Int
    if selection.length > 0 {
      location = selection.location
      length = selection.length
    } else {
      guard selection.location > start else { return false }
      location = selection.location - 1
      length = 1
    }
    pending = Deletion(
      offset: location - start, length: length,
      expectedSelection: NSRange(location: location, length: 0))
    lastChange = ProcessInfo.processInfo.systemUptime
    return true
  }

  mutating func reconcile(selection: NSRange) -> Resolution {
    guard start != nil else { return .unchanged }
    if let pending {
      guard selection == pending.expectedSelection else {
        reset()
        return .invalid
      }
      count -= pending.length
      self.pending = nil
      if count == 0 { reset() }
      return .deleted(pending)
    }
    guard contains(selection) else {
      reset()
      return .invalid
    }
    return .unchanged
  }
}
