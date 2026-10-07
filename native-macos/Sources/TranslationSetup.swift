// 官方系统语言模型的准备窗口，与输入法服务分进程运行。
import AppKit
import SwiftUI
import Translation

enum TranslationSetup {
  static var window: NSWindow?

  static func launch() {
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    process.arguments = ["--setup-translation"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try? process.run()
  }

  static func start() {
    NSApplication.shared.setActivationPolicy(.regular)
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 560, height: 320),
      styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.title = "准备本地整句翻译"
    window.appearance = NSAppearance(named: .aqua)
    if #available(macOS 15.0, *) {
      window.contentView = NSHostingView(rootView: TranslationSetupView())
    } else {
      window.contentView = NSTextField(wrappingLabelWithString: "本地整句翻译需要 macOS 26 或以上。词语输入功能仍可使用。")
    }
    window.center()
    window.makeKeyAndOrderFront(nil)
    NSApplication.shared.activate(ignoringOtherApps: true)
    self.window = window
  }
}

@available(macOS 15.0, *)
private typealias SetupState<Value> = SwiftUI.State<Value>

@available(macOS 15.0, *)
private struct TranslationSetupView: View {
  @SetupState private var configuration: TranslationSession.Configuration?
  @SetupState private var working = false
  @SetupState private var message = "首次使用需要准备中英文语言模型。下载由 macOS 完成，翻译文字在本机处理。"
  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("中英整句翻译").font(.system(size: 24, weight: .semibold))
      Text(message).font(.system(size: 15)).frame(minHeight: 64, alignment: .topLeading)
      Button(working ? "正在准备…" : "准备中英文语言") {
        working = true
        message = "请完成系统的语言下载提示。首次下载需要联网，完成后可离线翻译。"
        if #available(macOS 26.4, *) {
          configuration = .init(
            source: Locale.Language(identifier: "zh-Hans"),
            target: Locale.Language(identifier: "en"), preferredStrategy: .lowLatency)
        } else {
          configuration = .init(
            source: Locale.Language(identifier: "zh-Hans"),
            target: Locale.Language(identifier: "en"))
        }
      }.buttonStyle(.borderedProminent).tint(Color(red: 86 / 255, green: 119 / 255, blue: 0))
        .frame(minHeight: 44).disabled(working)
      Button("关闭") { NSApplication.shared.terminate(nil) }.frame(minHeight: 44)
      Text("不上传输入正文，不保存输入记录。准备完毕后重新输入句子即可。")
        .font(.system(size: 12)).foregroundStyle(
          Color(red: 74 / 255, green: 74 / 255, blue: 74 / 255))
    }.padding(24).frame(width: 512, alignment: .leading).background(Color.white)
      .translationTask(configuration) { session in
        do {
          try await session.prepareTranslation()
          message = "中英文语言已准备好。关闭此窗口，重新选择输入法，即可试用整句翻译。"
        } catch {
          message = "语言准备未完成。请检查网络后重试，也可以在系统设置的翻译语言中下载中文和英文。"
        }
        working = false
        configuration = nil
      }
  }
}
