// 与随包 Rust 引擎通信；只使用本地管道，不读取个人输入法配置。
import Darwin
import Foundation

final class EngineClient {
  private let process = Process()
  private let input = Pipe()
  private let output = Pipe()
  private var buffer = Data()
  private let lock = NSLock()

  init(resources: URL) throws {
    process.executableURL = resources.appendingPathComponent("bilingual-ime-bridge")
    process.arguments = [resources.path]
    process.standardInput = input
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    try process.run()
    _ = try request(EngineRequest(action: "query", input: ""), timeout: 5)
  }

  func request(_ request: EngineRequest, timeout: TimeInterval = 0.5) throws -> EngineFrame {
    lock.lock()
    defer { lock.unlock() }
    guard process.isRunning else { throw EngineFailure.unavailable }
    var data = try JSONEncoder().encode(request)
    data.append(10)
    try input.fileHandleForWriting.write(contentsOf: data)
    let deadline = Date().addingTimeInterval(timeout)
    while !buffer.contains(10) {
      let remaining = deadline.timeIntervalSinceNow
      guard remaining > 0 else {
        terminate()
        throw EngineFailure.timeout
      }
      var descriptor = pollfd(
        fd: output.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0)
      let status = poll(&descriptor, 1, Int32(remaining * 1000))
      guard status > 0 else {
        terminate()
        throw EngineFailure.timeout
      }
      var bytes = [UInt8](repeating: 0, count: 8192)
      let count = Darwin.read(descriptor.fd, &bytes, bytes.count)
      guard count > 0 else { throw EngineFailure.unavailable }
      buffer.append(contentsOf: bytes.prefix(count))
    }
    guard let end = buffer.firstIndex(of: 10) else { throw EngineFailure.unavailable }
    let response = Data(buffer[..<end])
    buffer.removeSubrange(...end)
    if let object = try JSONSerialization.jsonObject(with: response) as? [String: Any],
      let error = object["error"] as? String
    {
      throw EngineFailure.message(error)
    }
    return try JSONDecoder().decode(EngineFrame.self, from: response)
  }

  func terminate() {
    if process.isRunning { process.terminate() }
  }
  deinit { terminate() }
}
