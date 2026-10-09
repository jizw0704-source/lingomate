// 跨进程锁只保护正常输入服务；退出/崩溃自动释放，不保存正文或进程标识。
import Darwin
import Foundation

final class ServiceLease {
  private let descriptor: Int32
  private static var active: ServiceLease?

  private init(descriptor: Int32) { self.descriptor = descriptor }
  deinit { close(descriptor) }

  static func acquire(at file: URL) throws -> ServiceLease? {
    try FileManager.default.createDirectory(
      at: file.deletingLastPathComponent(), withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    let descriptor = open(file.path, O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, 0o600)
    guard descriptor >= 0 else { throw failure() }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else {
      let code = errno
      close(descriptor)
      if code == EWOULDBLOCK { return nil }
      throw NSError(domain: NSPOSIXErrorDomain, code: Int(code))
    }
    return ServiceLease(descriptor: descriptor)
  }

  static func start() {
    let file = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
      "Library/Application Support/BilingualCompanion/Service/service.lock")
    do {
      guard let lease = try acquire(at: file) else {
        print("输入服务已在运行；不启动第二个实例。")
        exit(0)
      }
      active = lease
    } catch {
      fputs("输入服务启动锁不可用；未启动引擎或输入连接。\n", stderr)
      exit(1)
    }
  }

  static func probe(file: URL, hold: Bool) {
    do {
      guard let lease = try acquire(at: file) else { exit(75) }
      withExtendedLifetime(lease) {
        if hold {
          FileHandle.standardOutput.write(Data([49]))
          _ = readLine()
        }
      }
      exit(0)
    } catch { exit(2) }
  }

  private static func failure() -> NSError {
    NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
  }

  static func checks() {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      "bilingual-service-lock-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let file = directory.appendingPathComponent("service.lock")
    func child(hold: Bool = false) -> Process {
      let process = Process()
      process.executableURL = Bundle.main.executableURL!
      process.arguments = ["--service-lock-probe", file.path] + (hold ? ["--hold"] : [])
      return process
    }
    do {
      var lease = try acquire(at: file)
      precondition(lease != nil)
      let blocked = child()
      try withExtendedLifetime(lease) {
        try blocked.run()
        blocked.waitUntilExit()
      }
      precondition(blocked.terminationStatus == 75)
      lease = nil
      let released = child()
      try released.run()
      released.waitUntilExit()
      precondition(released.terminationStatus == 0)
      let holder = child(hold: true)
      let output = Pipe()
      let input = Pipe()
      holder.standardOutput = output
      holder.standardInput = input
      try holder.run()
      defer {
        if holder.isRunning {
          holder.terminate()
          holder.waitUntilExit()
        }
      }
      var ready = pollfd(
        fd: output.fileHandleForReading.fileDescriptor, events: Int16(POLLIN), revents: 0)
      precondition(poll(&ready, 1, 2000) == 1 && ready.revents & Int16(POLLIN) != 0)
      precondition(output.fileHandleForReading.readData(ofLength: 1) == Data([49]))
      let held = try acquire(at: file)
      precondition(held == nil)
      precondition(kill(holder.processIdentifier, SIGKILL) == 0)
      holder.waitUntilExit()
      let afterCrash = try acquire(at: file)
      precondition(afterCrash != nil)
      print("PASS 独立输入服务启动锁：跨进程互斥、正常释放、崩溃释放；无真实输入服务/词库访问")
    } catch {
      fputs("FAIL 输入服务启动锁检查\n", stderr)
      exit(1)
    }
  }
}
