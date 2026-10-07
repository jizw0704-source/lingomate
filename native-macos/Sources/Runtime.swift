// 输入法进程只保留本地引擎和显示偏好，不保存输入正文。
import AppKit

enum Runtime {
  static var engine: EngineClient?
  static var bilingual = true
  static var failure: String?
}
