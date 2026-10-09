// 与随包 Rust 引擎通信；故障只清理本实例子进程，不重放提交或确认。
import Darwin
import Foundation

final class EngineClient {
  private final class Connection {
    let identity = UUID()
    let process = Process()
    let input = Pipe()
    let output = Pipe()
    var buffer = Data()
    func close() {
      if process.isRunning {
        // 超时进程可能被挂起；仅杀死本实例拥有的桥接子进程并回收。
        kill(process.processIdentifier, SIGKILL)
        process.waitUntilExit()
      }
      try? input.fileHandleForWriting.close()
      try? input.fileHandleForReading.close()
      try? output.fileHandleForReading.close()
      try? output.fileHandleForWriting.close()
    }
    deinit { close() }
  }

  private let resources: URL
  private let memoryURL: URL?
  private var connection: Connection?
  private var closed = false
  private var restartAfter: UInt64 = 0
  private var restarts = 0
  private let lock = NSLock()
  var processIdentifier: Int32? {
    lock.lock()
    defer { lock.unlock() }
    guard let connection, connection.process.isRunning else { return nil }
    return connection.process.processIdentifier
  }
  var restartCount: Int {
    lock.lock()
    defer { lock.unlock() }
    return restarts
  }

  init(resources: URL, memoryURL: URL? = nil) throws {
    self.resources = resources
    self.memoryURL = memoryURL
    try start(deadline: Self.deadline(5))
  }

  private static func deadline(_ timeout: TimeInterval) -> UInt64 {
    DispatchTime.now().uptimeNanoseconds + UInt64(max(0, min(timeout, 60)) * 1_000_000_000)
  }
  private func start(deadline: UInt64) throws {
    let next = Connection()
    next.process.executableURL = resources.appendingPathComponent("bilingual-ime-bridge")
    next.process.arguments = [resources.path]
    if let memoryURL { next.process.arguments! += ["--memory", memoryURL.path] }
    next.process.standardInput = next.input
    next.process.standardOutput = next.output
    next.process.standardError = FileHandle.nullDevice
    connection = next
    do {
      try next.process.run()
      // 父进程不保留子端，否则子进程退出后读端无法立即收到EOF。
      try next.input.fileHandleForReading.close()
      try next.output.fileHandleForWriting.close()
      _ = fcntl(next.input.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1)
      _ = try exchange(EngineRequest(action: "query", input: ""), deadline: deadline)
    } catch {
      disconnect()
      throw error
    }
  }

  func confirmSelection(_ frame: EngineFrame, context: String, chinese: Bool) {
    guard let token = frame.learningToken else { return }
    do {
      let confirmation = try request(
        EngineRequest(
          action: "confirm", input: "", context: context, learningToken: token,
          chineseOutput: chinese), expectedIdentity: frame.engineIdentity)
      Runtime.memoryWarning = confirmation.memoryWarning
    } catch {
      Runtime.memoryWarning = "文字已输出，选词记忆暂未保存。"
    }
  }

  func cancelLearning(context: String) {
    _ = try? request(EngineRequest(action: "cancel", input: "", context: context))
  }

  func request(
    _ request: EngineRequest, timeout: TimeInterval = 0.5, expectedIdentity: UUID? = nil
  ) throws -> EngineFrame {
    lock.lock()
    defer { lock.unlock() }
    guard !closed else { throw EngineFailure.unavailable }
    if request.action == "confirm" {
      guard let expectedIdentity, connection?.identity == expectedIdentity else {
        throw EngineFailure.unavailable
      }
    }
    let deadline = Self.deadline(timeout)
    if connection?.process.isRunning != true {
      // 只在新查询时恢复；旧提交/确认/取消不能跨越已丢失的会话回执。
      guard request.action == "query", DispatchTime.now().uptimeNanoseconds >= restartAfter else {
        throw EngineFailure.unavailable
      }
      disconnect()
      try start(deadline: deadline)
      restarts += 1
    }
    do {
      return try exchange(request, deadline: deadline)
    } catch EngineFailure.message(let message) {
      // 引擎正常拒绝无效候选，不当作传输故障。
      throw EngineFailure.message(message)
    } catch {
      disconnect()
      throw error
    }
  }

  private func exchange(_ request: EngineRequest, deadline: UInt64) throws -> EngineFrame {
    guard let connection, connection.process.isRunning else { throw EngineFailure.unavailable }
    var data = try JSONEncoder().encode(request)
    data.append(10)
    try connection.input.fileHandleForWriting.write(contentsOf: data)
    while !connection.buffer.contains(10) {
      let now = DispatchTime.now().uptimeNanoseconds
      guard now < deadline else { throw EngineFailure.timeout }
      var descriptor = pollfd(
        fd: connection.output.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0
      )
      let milliseconds = Int32(max(1, (deadline - now) / 1_000_000))
      let status = poll(&descriptor, 1, milliseconds)
      if status < 0 && errno == EINTR { continue }
      guard status > 0 else { throw EngineFailure.timeout }
      var bytes = [UInt8](repeating: 0, count: 8192)
      let count = Darwin.read(descriptor.fd, &bytes, bytes.count)
      if count < 0 && errno == EINTR { continue }
      guard count > 0 else { throw EngineFailure.unavailable }
      connection.buffer.append(contentsOf: bytes.prefix(count))
      guard connection.buffer.count <= 1_000_000 else { throw EngineFailure.unavailable }
    }
    guard let end = connection.buffer.firstIndex(of: 10) else { throw EngineFailure.unavailable }
    let response = Data(connection.buffer[..<end])
    connection.buffer.removeSubrange(...end)
    if let object = try JSONSerialization.jsonObject(with: response) as? [String: Any],
      let error = object["error"] as? String
    {
      throw EngineFailure.message(error)
    }
    var frame = try JSONDecoder().decode(EngineFrame.self, from: response)
    frame.engineIdentity = connection.identity
    return frame
  }

  private func disconnect() {
    connection?.close()
    connection = nil
    restartAfter = DispatchTime.now().uptimeNanoseconds + 100_000_000
  }
  func terminate() {
    lock.lock()
    defer { lock.unlock() }
    closed = true
    disconnect()
  }
  deinit { terminate() }
}
