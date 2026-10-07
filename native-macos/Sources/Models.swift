// 原生壳只消费引擎返回的候选，不进行查词或排序。
import Foundation

struct TranslationDetail: Codable {
  let word: String
  let pos: String
  let note: String
  let example: String
}

struct EngineCandidate: Codable {
  let text: String
  let syllables: [String]
  let translations: [TranslationDetail]
  let hasDetails: Bool
}

struct EngineFrame: Codable {
  let input: String
  let marked: String
  let candidates: [EngineCandidate]
  let committed: String?
}

struct EngineRequest: Encodable {
  let action: String
  let input: String
  var candidate: String?
  var syllables: [String]?
  var english: String?
}

enum EngineFailure: LocalizedError {
  case unavailable
  case timeout
  case message(String)
  var errorDescription: String? {
    switch self {
    case .unavailable: return "本地引擎未启动。请切回 ABC，再重新选择中英输入实验版。"
    case .timeout: return "本地引擎响应超时。未完成的拼音会原样保留。"
    case .message(let message): return message
    }
  }
}
