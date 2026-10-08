// 自建邮箱验证码服务；访问和续期凭据只由账号辅助进程保存在系统钥匙串。
import AppKit
import Security

// 登录凭据仅由独立账号/同步进程使用，输入法进程只使用账号标识和学习队列。
struct AccountConfiguration: Codable {
  let baseURL: URL
  func validate() throws {
    guard Self.validURL(baseURL) else { throw AccountError.configuration }
  }
  static func validURL(_ url: URL) -> Bool {
    url.scheme == "https" && url.host != nil && url.user == nil && url.password == nil
      && url.query == nil && url.fragment == nil
  }
}

enum AccountError: LocalizedError {
  case configuration, storage, signedOut, invalidWord, queueFull, cancelled, network, identity,
    busy, throttled
  var errorDescription: String? {
    switch self {
    case .configuration: return "登录服务尚未正确配置。请填写已部署账号服务的 HTTPS 地址。"
    case .storage: return "本机学习数据暂不可读写。原文件已保留，请检查磁盘权限。"
    case .signedOut: return "请重新登录后同步。"
    case .invalidWord: return "此内容暂不能加入单词记录。"
    case .queueFull: return "待同步记录已达上限，请先联网同步。"
    case .cancelled: return "登录已取消。"
    case .network: return "服务暂不可用，请检查连接后重试。"
    case .identity: return "登录验证未完成，请重试。"
    case .busy: return "另一项同步正在进行，请稍后重试。"
    case .throttled: return "验证码请求较频繁，请稍后再试。"
    }
  }
}

struct LoginTokens: Codable {
  let access: String
  let refresh: String?
  let expiry: Date
}

protocol AccountVault {
  func load(_ scope: String) throws -> LoginTokens?
  func save(_ tokens: LoginTokens, scope: String) throws
  func remove(_ scope: String) throws
}

struct KeychainAccountVault: AccountVault {
  private func query(_ scope: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "org.local.bilingualcompanion.account",
      kSecAttrAccount as String: scope,
    ]
  }
  func load(_ scope: String) throws -> LoginTokens? {
    var request = query(scope)
    request[kSecReturnData as String] = true
    var result: CFTypeRef?
    let status = SecItemCopyMatching(request as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess, let data = result as? Data else { throw AccountError.storage }
    return try JSONDecoder().decode(LoginTokens.self, from: data)
  }
  func save(_ tokens: LoginTokens, scope: String) throws {
    let data = try JSONEncoder().encode(tokens)
    let status = SecItemUpdate(
      query(scope) as CFDictionary, [kSecValueData as String: data] as CFDictionary)
    if status == errSecItemNotFound {
      var request = query(scope)
      request[kSecValueData as String] = data
      request[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
      guard SecItemAdd(request as CFDictionary, nil) == errSecSuccess else {
        throw AccountError.storage
      }
    } else if status != errSecSuccess {
      throw AccountError.storage
    }
  }
  func remove(_ scope: String) throws {
    let status = SecItemDelete(query(scope) as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw AccountError.storage
    }
  }
}

struct TokenResponse: Decodable {
  struct User: Decodable {
    let id: String
    let email: String
  }
  let accessToken: String
  let refreshToken: String?
  let expiresIn: Int
  let user: User
  enum CodingKeys: String, CodingKey {
    case accessToken = "access_token"
    case refreshToken = "refresh_token"
    case expiresIn = "expires_in"
    case user
  }
  var tokens: LoginTokens {
    .init(
      access: accessToken, refresh: refreshToken,
      expiry: Date().addingTimeInterval(TimeInterval(expiresIn)))
  }
}

final class AccountHTTP: NSObject, URLSessionTaskDelegate {
  typealias Transport = (URLRequest) async throws -> (Data, Int)
  private let transport: Transport?
  init(transport: Transport? = nil) { self.transport = transport }
  func urlSession(
    _ session: URLSession, task: URLSessionTask,
    willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
    completionHandler: @escaping (URLRequest?) -> Void
  ) { completionHandler(nil) }

  func request(
    _ url: URL, method: String = "GET", token: String? = nil,
    json: Data? = nil
  ) async throws -> Data {
    guard AccountConfiguration.validURL(url) else { throw AccountError.configuration }
    var request = URLRequest(url: url, timeoutInterval: 20)
    request.httpMethod = method
    if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
    if let json {
      request.httpBody = json
      request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    }
    let data: Data
    let status: Int
    if let transport {
      (data, status) = try await transport(request)
    } else {
      let session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)
      defer { session.finishTasksAndInvalidate() }
      do {
        let result = try await session.data(for: request)
        data = result.0
        status = (result.1 as? HTTPURLResponse)?.statusCode ?? 0
      } catch { throw AccountError.network }
    }
    guard data.count <= 8_000_000 else { throw AccountError.network }
    if status == 401 || status == 403 { throw AccountError.signedOut }
    if status == 429 { throw AccountError.throttled }
    guard (200...299).contains(status) else { throw AccountError.network }
    return data
  }
}

final class AccountService {
  let configuration: AccountConfiguration
  let store: LearningStore
  let vault: AccountVault
  let http: AccountHTTP
  init(
    configuration: AccountConfiguration, store: LearningStore,
    vault: AccountVault = KeychainAccountVault(), http: AccountHTTP = AccountHTTP()
  ) {
    self.configuration = configuration
    self.store = store
    self.vault = vault
    self.http = http
  }

  static func validEmail(_ value: String) -> Bool {
    value.count <= 254
      && value.range(
        of: "^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\\.[A-Za-z0-9-]+)+$",
        options: .regularExpression) != nil
  }

  func sendCode(email: String) async throws {
    try configuration.validate()
    guard Self.validEmail(email) else { throw AccountError.identity }
    _ = try await http.request(
      configuration.baseURL.appendingPathComponent("v1/auth/code"), method: "POST",
      json: try JSONSerialization.data(withJSONObject: ["email": email]))
  }

  func verify(email: String, code: String) async throws {
    guard Self.validEmail(email), code.count == 6, code.utf8.allSatisfy({ (48...57).contains($0) })
    else { throw AccountError.identity }
    let bytes: Data
    do {
      bytes = try await http.request(
        configuration.baseURL.appendingPathComponent("v1/auth/verify"), method: "POST",
        json: try JSONSerialization.data(withJSONObject: ["email": email, "code": code]))
    } catch AccountError.signedOut { throw AccountError.identity }
    let response = try JSONDecoder().decode(TokenResponse.self, from: bytes)
    guard UUID(uuidString: response.user.id) != nil, !response.accessToken.isEmpty,
      response.accessToken.count <= 256, (1...2_592_000).contains(response.expiresIn),
      response.user.email.lowercased() == email.lowercased()
    else {
      throw AccountError.identity
    }
    let account = LearningAccount(
      id: response.user.id, email: response.user.email,
      project: configuration.baseURL.absoluteString)
    try store.transaction { data in
      try vault.save(response.tokens, scope: account.scope)
      data.active = account
    }
  }

  func logout() async throws {
    let account = try store.snapshot().active
    let tokens = try account.flatMap { try vault.load($0.scope) }
    try store.transaction { data in
      if let account = data.active { try vault.remove(account.scope) }
      data.active = nil
    }
    if let tokens {
      _ = try await http.request(
        configuration.baseURL.appendingPathComponent("v1/auth/logout"), method: "POST",
        token: tokens.access,
        json: try JSONSerialization.data(withJSONObject: ["refresh_token": tokens.refresh ?? ""]))
    }
  }

  func sync() async throws {
    let descriptor = open(
      store.directory.appendingPathComponent("sync.lock").path, O_CREAT | O_RDWR, 0o600)
    guard descriptor >= 0 else { throw AccountError.storage }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw AccountError.busy }
    defer { flock(descriptor, LOCK_UN) }
    let snapshot = try store.snapshot()
    guard let account = snapshot.active, account.project == configuration.baseURL.absoluteString,
      var tokens = try vault.load(account.scope)
    else { throw AccountError.signedOut }
    if tokens.expiry.timeIntervalSinceNow < 60 {
      guard let refresh = tokens.refresh else { throw AccountError.signedOut }
      let bytes = try await http.request(
        configuration.baseURL.appendingPathComponent("v1/auth/refresh"), method: "POST",
        json: try JSONSerialization.data(withJSONObject: ["refresh_token": refresh]))
      let response = try JSONDecoder().decode(TokenResponse.self, from: bytes)
      guard response.user.id == account.id, response.user.email == account.email,
        !response.accessToken.isEmpty, response.accessToken.count <= 256,
        (1...2_592_000).contains(response.expiresIn)
      else { throw AccountError.identity }
      tokens = .init(
        access: response.accessToken, refresh: response.refreshToken ?? refresh,
        expiry: response.tokens.expiry)
      try store.transaction { data in
        guard data.active == account else { throw AccountError.signedOut }
        try vault.save(tokens, scope: account.scope)
      }
    }
    guard try store.snapshot().active == account else { throw AccountError.signedOut }
    let pending = snapshot.pending[account.scope] ?? []
    var acknowledged = Set<UUID>()
    if !pending.isEmpty {
      let bytes = try await http.request(
        configuration.baseURL.appendingPathComponent("v1/events"), method: "POST",
        token: tokens.access, json: try JSONEncoder().encode(pending))
      let ids = try JSONDecoder().decode([UUID].self, from: bytes)
      acknowledged = Set(ids).intersection(Set(pending.map(\.id)))
    }
    let bytes = try await http.request(
      configuration.baseURL.appendingPathComponent("v1/words"), token: tokens.access)
    let words = try JSONDecoder().decode([LearnedWord].self, from: bytes)
    guard words.count <= 5000, words.allSatisfy(LearningEvent.valid),
      words.allSatisfy({ $0.uses >= 0 })
    else { throw AccountError.network }
    try store.accept(acknowledged, for: account, words: words)
  }
}
