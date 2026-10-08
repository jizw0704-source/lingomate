// 系统本地模型异步翻译；每次组合输入变化都丢弃过期结果，不保存正文。
import Foundation
import Translation

enum SentenceTranslation: Equatable {
  case none
  case loading
  case ready(source: String, english: String)
  case missingModels
  case unavailable
  case failed
  case aiFailed(AIError)
}

enum LocalTranslationError: Error { case missingModels, unavailable, empty }

enum LocalSentenceTranslation {
  static func translate(_ text: String) async throws -> String {
    guard #available(macOS 26.0, *) else { throw LocalTranslationError.unavailable }
    let source = Locale.Language(identifier: "zh-Hans")
    let target = Locale.Language(identifier: "en")
    let session: TranslationSession
    if #available(macOS 26.4, *) {
      session = TranslationSession(
        installedSource: source, target: target, preferredStrategy: .lowLatency)
    } else {
      session = TranslationSession(installedSource: source, target: target)
    }
    guard await session.isReady else { throw LocalTranslationError.missingModels }
    let response = try await withTaskCancellationHandler {
      try await session.translate(text)
    } onCancel: {
      session.cancel()
    }
    let english = response.targetText.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !english.isEmpty, english.count <= 4000 else { throw LocalTranslationError.empty }
    return english
  }
}

final class SentenceTranslator {
  private var task: Task<Void, Never>?
  private var key: String?
  private var generation = UUID()
  private let translate: (String) async throws -> String
  private let delay: UInt64
  private var observer: NSObjectProtocol?
  private var currentChange: ((SentenceTranslation) -> Void)?

  init(
    delay: UInt64 = 350_000_000,
    translate: @escaping (String) async throws -> String = HybridSentenceTranslation.translate
  ) {
    self.delay = delay
    self.translate = translate
    observer = DistributedNotificationCenter.default().addObserver(
      forName: AISettings.notification, object: nil, queue: .main
    ) { [weak self] _ in self?.settingsChanged() }
  }

  func cancel() {
    task?.cancel()
    task = nil
    key = nil
    generation = UUID()
    currentChange = nil
  }

  func request(key: String, source: String, onChange: @escaping (SentenceTranslation) -> Void) {
    guard self.key != key else { return }
    cancel()
    self.key = key
    currentChange = onChange
    let token = generation
    let translate = self.translate
    let delay = self.delay
    onChange(.loading)
    task = Task { @MainActor [weak self] in
      do {
        try await Task.sleep(nanoseconds: delay)
        let english = try await translate(source)
        guard let self, self.generation == token, !Task.isCancelled else { return }
        onChange(.ready(source: source, english: english))
      } catch {
        guard let self, self.generation == token, !Task.isCancelled else { return }
        if let aiError = error as? AIError {
          onChange(.aiFailed(aiError))
        } else if case LocalTranslationError.missingModels = error {
          onChange(.missingModels)
        } else if case LocalTranslationError.unavailable = error {
          onChange(.unavailable)
        } else {
          onChange(.failed)
        }
      }
    }
  }

  func settingsChanged() {
    let change = currentChange
    cancel()
    change?(.none)
  }

  deinit {
    task?.cancel()
    if let observer { DistributedNotificationCenter.default().removeObserver(observer) }
  }
}
