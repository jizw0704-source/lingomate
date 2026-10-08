// 临时设置、内存密钥和替身HTTP，不发送真实请求，不访问真实钥匙串。
import AppKit

enum AITests {
  @MainActor static func run() async {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = AISettingsStore(directory: directory)
    var configuration = AIConfiguration(
      endpoint: URL(string: "https://api.example.invalid/v1/chat/completions")!,
      model: "test-model", enabled: true)
    let vault = MemoryAIKeyVault()
    var sent = 0
    let service = AIService { request in
      sent += 1
      precondition(request.httpMethod == "POST" && request.url == configuration.endpoint)
      precondition(request.value(forHTTPHeaderField: "Authorization") == "Bearer isolated-test-key")
      let body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
      precondition(body["model"] as? String == "test-model" && body["stream"] as? Bool == false)
      let messages = body["messages"] as! [[String: String]]
      precondition(messages.count == 2 && messages[1]["content"] == "我今天想学习英语")
      precondition(Set(body.keys) == ["model", "stream", "store", "messages"])
      return (
        Data(
          "{\"choices\":[{\"finish_reason\":\"stop\",\"message\":{\"content\":\"I want to learn English today.\",\"reasoning_content\":\"private reasoning\"}}]}"
            .utf8), 200
      )
    }
    do {
      let empty = try store.load()
      precondition(empty == nil)
      try store.save(configuration)
      try vault.save("isolated-test-key", scope: configuration.keyScope)
      let english = try await service.translate(
        "我今天想学习英语", configuration: configuration, key: vault.load(configuration.keyScope)!)
      precondition(english == "I want to learn English today." && sent == 1)
      let metadata = try Data(contentsOf: directory.appendingPathComponent("settings.json"))
      precondition(!String(decoding: metadata, as: UTF8.self).contains("isolated-test-key"))
      let anotherScope = "https://other.example.invalid/v1/chat/completions"
      precondition(tryLoad(vault, anotherScope) == nil)
      let mini = AIConfiguration.miniMax(international: false)
      precondition(
        mini.endpoint.absoluteString == "https://api.minimax.cn/v1/chat/completions"
          && !mini.enabled)
      var enabledMini = mini
      enabledMini.enabled = true
      let miniEnglish = try await AIService { request in
        let body = try JSONSerialization.jsonObject(with: request.httpBody!) as! [String: Any]
        precondition(
          body["reasoning_split"] as? Bool == true && body["max_completion_tokens"] as? Int == 4096)
        return (
          Data(
            "{\"choices\":[{\"finish_reason\":\"stop\",\"message\":{\"content\":\"Study English.\",\"reasoning_content\":\"never display\"}}]}"
              .utf8), 200
        )
      }.translate("学习英语", configuration: enabledMini, key: "test")
      precondition(miniEnglish == "Study English.")
      configuration.enabled = false
      do {
        _ = try await service.translate(
          "我今天想学习英语", configuration: configuration, key: "isolated-test-key")
        preconditionFailure()
      } catch {}
      precondition(sent == 1)
      configuration.enabled = true
      for (body, status, expected) in [
        (
          "{\"choices\":[{\"finish_reason\":\"stop\",\"message\":{\"content\":\"<think>Reasoning</think> Study.\"}}]}",
          200, AIError.invalidResponse
        ),
        ("{}", 401, AIError.denied), ("{}", 429, .quota), ("{}", 503, .network),
        (
          "{\"choices\":[{\"finish_reason\":\"length\",\"message\":{\"content\":\"Incomplete\"}}]}",
          200, .invalidResponse
        ),
        (
          "{\"choices\":[{\"finish_reason\":\"stop\",\"message\":{\"content\":\"\"}}]}", 200,
          .invalidResponse
        ),
      ] {
        do {
          _ = try await AIService { _ in (Data(body.utf8), status) }.translate(
            "学习英语", configuration: configuration, key: "test")
          preconditionFailure()
        } catch { precondition(error as? AIError == expected) }
      }
      do {
        try AIConfiguration(
          endpoint: URL(string: "http://example.invalid/v1/chat/completions")!, model: "test",
          enabled: true
        ).validate()
        preconditionFailure()
      } catch {}
      try Data("broken".utf8).write(to: directory.appendingPathComponent("settings.json"))
      do {
        _ = try store.load()
        preconditionFailure()
      } catch {}
      let preserved = try Data(contentsOf: directory.appendingPathComponent("settings.json"))
      precondition(preserved == Data("broken".utf8))
      var states: [SentenceTranslation] = []
      let translator = SentenceTranslator(delay: 0) { _ in
        try? await Task.sleep(nanoseconds: 50_000_000)
        return "Late result."
      }
      translator.request(key: "same", source: "学习英语") { states.append($0) }
      translator.settingsChanged()
      try? await Task.sleep(nanoseconds: 80_000_000)
      precondition(states == [.loading, .none])
      translator.request(key: "same", source: "学习英语") { states.append($0) }
      try? await Task.sleep(nanoseconds: 80_000_000)
      precondition(states.last == .ready(source: "学习英语", english: "Late result."))
      print(
        "PASS AI MiniMax预设及思考分离、配置变化取消旧译文、 隔离协议、仅当前中文候选、禁用不请求、密钥不入设置、按地址隔离、鉴权/限流/失败、拒绝截断及损坏保留；未访问真实API/钥匙串"
      )
    } catch { preconditionFailure("AI 隔离检查失败") }
  }
  private static func tryLoad(_ vault: AIKeyVault, _ scope: String) -> String? {
    try? vault.load(scope)
  }
}
