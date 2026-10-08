import AppKit

private final class TestAccountVault: AccountVault {
  var entries: [String: LoginTokens] = [:]
  func load(_ scope: String) throws -> LoginTokens? { entries[scope] }
  func save(_ tokens: LoginTokens, scope: String) throws { entries[scope] = tokens }
  func remove(_ scope: String) throws { entries[scope] = nil }
}

enum AccountTests {
  @MainActor static func run() async {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = LearningStore(directory: directory)
    let vault = TestAccountVault()
    let userID = UUID().uuidString
    let response: [String: Any] = [
      "access_token": "isolated-access", "refresh_token": "isolated-refresh", "expires_in": 300,
      "user": ["id": userID, "email": "test@example.invalid"],
    ]
    var calls: [String] = []
    var sentEvents: [LearningEvent] = []
    let http = AccountHTTP { request in
      let path = request.url!.path
      calls.append(path)
      if path == "/v1/auth/code" { return (Data("{}".utf8), 200) }
      if path == "/v1/auth/verify" || path == "/v1/auth/refresh" {
        return (try JSONSerialization.data(withJSONObject: response), 200)
      }
      if path == "/v1/events" {
        precondition(request.value(forHTTPHeaderField: "Authorization") == "Bearer isolated-access")
        sentEvents = try JSONDecoder().decode([LearningEvent].self, from: request.httpBody!)
        return (try JSONEncoder().encode(sentEvents.map(\.id)), 200)
      }
      if path == "/v1/words" {
        return (
          try JSONEncoder().encode([
            LearnedWord(english: "learn", chinese: "学习", pos: "v.", uses: 1, mastered: true)
          ]), 200
        )
      }
      if path == "/v1/auth/logout" { return (Data("{}".utf8), 200) }
      return (Data(), 503)
    }
    let service = AccountService(
      configuration: .init(baseURL: URL(string: "https://account.example.invalid")!), store: store,
      vault: vault, http: http)
    do {
      precondition(
        !AccountService.validEmail("bad") && AccountService.validEmail("test@example.invalid"))
      do {
        try AccountConfiguration(baseURL: URL(string: "http://example.invalid")!).validate()
        preconditionFailure()
      } catch {}
      try await service.sendCode(email: "test@example.invalid")
      let unsigned = try store.snapshot()
      precondition(unsigned.active == nil)
      try await service.verify(email: "test@example.invalid", code: "123456")
      let account = try store.snapshot().active!
      precondition(account.id == userID)
      try store.append(
        .init(
          id: UUID(), english: "learn", chinese: "学习", pos: "v.", kind: "study", mastered: false),
        for: account)
      try store.append(
        .init(
          id: UUID(), english: "learn", chinese: "学习", pos: "v.", kind: "mastered", mastered: true),
        for: account)
      let before = try store.snapshot().visibleWords(account)
      precondition(before.count == 1 && before[0].uses == 1 && before[0].mastered)
      // 未登录/其他账号不能写入；句子不能混入单词上传。
      let other = LearningAccount(
        id: UUID().uuidString, email: "other@example.invalid", project: account.project)
      do {
        try store.append(
          .init(
            id: UUID(), english: "friend", chinese: "朋友", pos: "n.", kind: "study", mastered: false),
          for: other)
        preconditionFailure()
      } catch {}
      // 同一用户退出后重新登录也不能接受旧会话排队的提交。
      let newerSession = LearningAccount(
        id: account.id, email: account.email, project: account.project)
      try store.transaction { $0.active = newerSession }
      do {
        try store.append(
          .init(
            id: UUID(), english: "friend", chinese: "朋友", pos: "n.", kind: "study", mastered: false),
          for: account)
        preconditionFailure()
      } catch {}
      try store.transaction { $0.active = account }
      let offlineService = AccountService(
        configuration: service.configuration, store: store, vault: vault,
        http: AccountHTTP { _ in (Data(), 503) })
      do {
        try await offlineService.sync()
        preconditionFailure()
      } catch {}
      let offline = try store.snapshot()
      precondition(offline.pending[account.scope]?.count == 2)
      precondition(
        !LearningEvent.valid(
          .init(
            english: "I learned something.", chinese: "今天我学到了很多东西。", pos: "", uses: 0,
            mastered: false)))
      try vault.save(
        .init(access: "expired", refresh: "isolated-refresh", expiry: .distantPast),
        scope: account.scope)
      try await service.sync()
      let after = try store.snapshot()
      precondition(after.pending[account.scope]?.isEmpty == true)
      precondition(after.visibleWords(account).first?.mastered == true)
      precondition(sentEvents.count == 2 && calls.contains("/v1/auth/refresh"))
      let persisted = try LearningStore(directory: directory).snapshot()
      precondition(persisted.active == account)
      try await service.logout()
      let loggedOut = try store.snapshot()
      precondition(loggedOut.active == nil)
      precondition(vault.entries.isEmpty)
      let bytes = try Data(contentsOf: directory.appendingPathComponent("learning.json"))
      precondition(!String(decoding: bytes, as: UTF8.self).contains("isolated-access"))
      // 损坏文件保留，不能被默认空状态覆盖。
      try Data("broken".utf8).write(to: directory.appendingPathComponent("learning.json"))
      do {
        _ = try store.snapshot()
        preconditionFailure()
      } catch {}
      let preserved = try Data(contentsOf: directory.appendingPathComponent("learning.json"))
      precondition(preserved == Data("broken".utf8))
      print("PASS 隔离邮箱登录协议、续期、退出、按账号队列、掌握标记、重载、正文排除及损坏保留；无真实邮件/钥匙串访问")
    } catch { preconditionFailure("隔离账号测试失败") }
  }
}
