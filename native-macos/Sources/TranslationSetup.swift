// 官方系统语言模型的准备窗口，与输入法服务分进程运行。
import AppKit
import SwiftUI
import Translation

enum TranslationSetup {
  static var window: NSWindow?

  static func launch() {
    let process = Process()
    process.executableURL = Bundle.main.executableURL
    var arguments = ["--setup-translation"]
    if AppearanceSettings.current.isIsolated {
      arguments += [
        "--isolated-appearance", "--theme", AppearanceSettings.current.choice.rawValue,
      ]
    }
    process.arguments = arguments
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
    window.backgroundColor = NativeTheme.background
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
      Text("本机翻译").font(.system(size: 12)).foregroundStyle(Color(nsColor: NativeTheme.muted))
      Text("让整句也能选英文").font(.system(size: 24, weight: .semibold))
      Text(message).font(.system(size: 15)).foregroundStyle(Color(nsColor: NativeTheme.muted))
        .frame(minHeight: 64, alignment: .topLeading)
      HStack(spacing: 12) {
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
        }.buttonStyle(SetupButtonStyle(outlined: true)).disabled(working)
        Button("关闭") { NSApplication.shared.terminate(nil) }.buttonStyle(
          SetupButtonStyle(outlined: false))
      }
      Text("不上传输入正文，不保存输入记录。准备完毕后重新输入句子即可。")
        .font(.system(size: 12)).foregroundStyle(
          Color(nsColor: NativeTheme.muted))
    }.padding(24).frame(width: 512, alignment: .leading).background(
      Color(nsColor: NativeTheme.background)
    )
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

@available(macOS 15.0, *)
private struct SetupButtonStyle: ButtonStyle {
  var outlined: Bool
  @Environment(\.isEnabled) private var enabled
  @SetupState private var hovered = false
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.font(.system(size: 14, weight: .medium))
      .padding(.horizontal, 16).frame(minWidth: 44, minHeight: 44)
      .foregroundStyle(Color(nsColor: enabled ? NativeTheme.ink : NativeTheme.muted))
      .background(
        enabled && (configuration.isPressed || hovered)
          ? Color(nsColor: NativeTheme.surface) : Color(nsColor: NativeTheme.background)
      )
      .clipShape(RoundedRectangle(cornerRadius: 12))
      .overlay(
        RoundedRectangle(cornerRadius: 12).strokeBorder(
          outlined
            ? Color(
              nsColor: enabled && (configuration.isPressed || hovered)
                ? NativeTheme.muted : NativeTheme.boundary) : Color.clear, lineWidth: 1)
      )
      .onHover { hovered = $0 }
  }
}
