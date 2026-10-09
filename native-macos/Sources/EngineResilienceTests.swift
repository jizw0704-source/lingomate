// 只终止本测试创建的子进程，使用临时词库；不触碰已安装输入服务。
import Darwin
import Foundation

enum EngineResilienceTests {
  static func run() {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      "bilingual-resilience-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    do {
      guard let resources = Bundle.main.resourceURL else { throw EngineFailure.unavailable }
      let memory = directory.appendingPathComponent("words.json")
      let engine = try EngineClient(resources: resources, memoryURL: memory)
      defer { engine.terminate() }
      let query = EngineRequest(action: "query", input: "xuexi", context: "isolated")
      let first = try engine.request(query).candidates.first { $0.text == "学习" }!
      let commit = EngineRequest(
        action: "commit", input: "xuexi", candidate: first.text, syllables: first.syllables,
        context: "isolated")
      let stale = try engine.request(commit)
      precondition(stale.committed == "学习" && stale.learningToken != nil)
      let pid = engine.processIdentifier!
      precondition(kill(pid, SIGKILL) == 0)
      usleep(50_000)
      // 旧提交不会自行启动引擎，也不会重放。
      fails { _ = try engine.request(commit) }
      precondition(engine.restartCount == 0)
      let recovered = try engine.request(query)
      precondition(recovered.candidates.contains { $0.text == "学习" })
      precondition(engine.restartCount == 1 && engine.processIdentifier != pid)
      let fresh = try engine.request(commit)
      precondition(stale.learningToken == fresh.learningToken)
      precondition(stale.engineIdentity != fresh.engineIdentity)
      engine.confirmSelection(stale, context: "isolated", chinese: true)
      precondition(!FileManager.default.fileExists(atPath: memory.path))
      engine.confirmSelection(fresh, context: "isolated", chinese: true)
      precondition(FileManager.default.fileExists(atPath: memory.path))
      // 挂起独立桥接进程，验证超时有界、旧管道销毁及下一次查询恢复。
      let hanging = engine.processIdentifier!
      precondition(kill(hanging, SIGSTOP) == 0)
      let started = DispatchTime.now().uptimeNanoseconds
      do {
        _ = try engine.request(query, timeout: 0.05)
        preconditionFailure("挂起的进程不应返回查询结果")
      } catch EngineFailure.timeout {}
      let elapsed = Double(DispatchTime.now().uptimeNanoseconds - started) / 1_000_000_000
      precondition(elapsed < 1 && engine.processIdentifier == nil)
      precondition(kill(hanging, 0) == -1 && errno == ESRCH)
      fails { _ = try engine.request(commit) }
      usleep(150_000)
      let afterTimeout = try engine.request(query)
      precondition(afterTimeout.candidates.contains { $0.text == "学习" })
      precondition(engine.restartCount == 2)
      // 无效候选属于协议拒绝，不重启健康进程。
      let healthy = engine.processIdentifier
      fails {
        _ = try engine.request(
          EngineRequest(action: "commit", input: "xuexi", candidate: "银行", syllables: ["yin"]))
      }
      precondition(engine.processIdentifier == healthy)
      // 3000次查询涵盖连续编辑、后页和80+字母句子；只统计时间。
      let inputs = [
        "x", "xu", "xue", "xuex", "xuexi", "shi", "kaifazhe",
        "woxiangxuexiyingyuyinweitakeyibangzhuwohegengduodepengyoujiaoliubingqieliaojiebutongdewenhua",
      ]
      var durations: [Double] = []
      for index in 0..<3000 {
        let text = inputs[index % inputs.count]
        let began = DispatchTime.now().uptimeNanoseconds
        let frame = try engine.request(EngineRequest(action: "query", input: text))
        precondition(frame.input == text && !frame.candidates.isEmpty)
        durations.append(Double(DispatchTime.now().uptimeNanoseconds - began) / 1_000_000)
      }
      durations.sort()
      precondition(engine.restartCount == 2)
      print(
        String(
          format: "PASS 3000次独立引擎查询：P95 %.2fms，最大 %.2fms；异常退出/挂起超时恢复、旧提交不重放、旧回执拒绝、正常拒绝不重启",
          durations[Int(Double(durations.count) * 0.95)], durations.last!))
      engine.terminate()
      fails { _ = try engine.request(query) }
      precondition(engine.processIdentifier == nil)
    } catch {
      fputs("FAIL 引擎故障恢复：\(error.localizedDescription)\n", stderr)
      exit(1)
    }
  }
  private static func fails(_ operation: () throws -> Void) {
    do {
      try operation()
      preconditionFailure("操作应失败")
    } catch {}
  }
}
