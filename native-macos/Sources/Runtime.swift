// 仅经确认的词语保存在本机词库，不记录输入过程或宿主上下文。
import AppKit

enum Runtime {
  static var engine: EngineClient?
  static var bilingual = true
  static var failure: String?
  static var memoryWarning: String?
  static var remember = true
  static var punctuationMode = PunctuationMode.automatic
}
