// 系统输入法服务入口；预览与自测为独立模式，不冒充系统接入验收。
import AppKit
import InputMethodKit

let arguments = CommandLine.arguments
if arguments.contains("--service-lock-test") {
  ServiceLease.checks()
  exit(0)
}
if let index = arguments.firstIndex(of: "--service-lock-probe"), arguments.count > index + 1 {
  ServiceLease.probe(
    file: URL(fileURLWithPath: arguments[index + 1]), hold: arguments.contains("--hold"))
}
if let index = arguments.firstIndex(of: "--input-apply") {
  guard arguments.count == index + 3,
    let setting = InputSetting(payload: [
      "setting": arguments[index + 1], "value": arguments[index + 2],
    ])
  else {
    fputs("输入设置参数无效。\n", stderr)
    exit(2)
  }
  SettingsInputClient.applyFromCommandLine(setting)
  exit(0)
}
if arguments.contains("--engine-resilience-test") {
  EngineResilienceTests.run()
  exit(0)
}
if arguments.contains("--input-status") {
  InputDiagnostics.query()
  exit(0)
}
if arguments.contains("--input-diagnostic-test") {
  InputDiagnostics.checks()
  exit(0)
}
if arguments.contains("--sources") {
  InputSources.list(includeDisabled: arguments.contains("--all"))
  exit(0)
}
if arguments.contains("--register") { exit(InputSources.register()) }
if arguments.contains("--check-enabled") { exit(InputSources.checkEnabled()) }
if arguments.contains("--disable") { exit(InputSources.disable()) }
if arguments.contains("--select") { exit(InputSources.select(InputSources.identifier)) }
if let index = arguments.firstIndex(of: "--select-id"), arguments.count > index + 1 {
  exit(InputSources.select(arguments[index + 1]))
}
if arguments.contains("--selftest") {
  SelfTests.run()
  exit(0)
}
if arguments.contains("--punctuation-test") {
  PunctuationTests.run()
  exit(0)
}
if arguments.contains("--typing-test") {
  TypingTests.run()
  exit(0)
}

if let index = arguments.firstIndex(of: "--appearance-readback-test"), arguments.count > index + 2 {
  AppearanceTests.readback(arguments[index + 1], expected: arguments[index + 2])
}
let app = NSApplication.shared
if arguments.contains("--input-window-test") {
  InputPresentation.checks()
  exit(0)
}
if arguments.contains("--ai-test") {
  Task { @MainActor in
    await AITests.run()
    exit(0)
  }
  app.run()
  exit(0)
}
if arguments.contains("--account-test") {
  Task { @MainActor in
    await AccountTests.run()
    exit(0)
  }
  app.run()
  exit(0)
}
if arguments.contains("--appearance-test") {
  AppearanceTests.run()
  exit(0)
}
if arguments.contains("--sentence-state-test") || arguments.contains("--sentence-integration-test")
{
  app.setActivationPolicy(.prohibited)
  Task { @MainActor in
    if arguments.contains("--sentence-state-test") {
      await SentenceTests.stateChecks()
      exit(0)
    }
    do {
      try await SentenceTests.integrationChecks()
      exit(0)
    } catch {
      fputs("整句联调未通过：\(error.localizedDescription)\n", stderr)
      exit(1)
    }
  }
  app.run()
  exit(0)
}
let isPreview =
  arguments.contains("--preview") || arguments.contains("--preview-sentence")
  || arguments.contains("--preview-paging") || arguments.contains("--preview-learning")
  || arguments.contains("--preview-punctuation")
  || arguments.contains("--preview-mixed")
  || arguments.contains("--preview-typing") || arguments.contains("--account-preview")
  || arguments.contains("--ai-settings-preview")
  || arguments.contains("--settings-preview") || arguments.contains("--settings-test")
let previewTheme: AppearanceChoice? = {
  guard let index = arguments.firstIndex(of: "--theme"), arguments.count > index + 1 else {
    return nil
  }
  return AppearanceChoice(rawValue: arguments[index + 1])
}()
AppearanceSettings.configure(
  isolated: isPreview || arguments.contains("--isolated-appearance"), initial: previewTheme)
let aiDirectory =
  isPreview
  ? FileManager.default.temporaryDirectory.appendingPathComponent(
    "bilingual-ai-" + UUID().uuidString)
  : FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
    "Library/Application Support/BilingualCompanion/AITranslation")
AISettings.store = AISettingsStore(directory: aiDirectory)
if arguments.contains("--ai-translate") {
  app.setActivationPolicy(.prohibited)
  Task { @MainActor in
    await HybridSentenceTranslation.helper()
    exit(0)
  }
  app.run()
  exit(0)
}
if arguments.contains("--ai-settings") || arguments.contains("--ai-settings-preview") {
  AISettingsWindow.start(preview: isPreview)
  app.run()
  if isPreview { try? FileManager.default.removeItem(at: aiDirectory) }
  exit(0)
}
let learningDirectory =
  isPreview
  ? FileManager.default.temporaryDirectory.appendingPathComponent(
    "bilingual-study-" + UUID().uuidString)
  : FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
    "Library/Application Support/BilingualCompanion/Learning")
LearningRuntime.store = LearningStore(directory: learningDirectory)
if arguments.contains("--settings") || arguments.contains("--settings-preview")
  || arguments.contains("--settings-test")
{
  if arguments.contains("--settings-test") {
    SettingsTests.run()
  } else {
    SettingsWindow.start(preview: isPreview)
    app.run()
  }
  if isPreview { try? FileManager.default.removeItem(at: learningDirectory) }
  if isPreview { try? FileManager.default.removeItem(at: aiDirectory) }
  exit(0)
}
if arguments.contains("--account") || arguments.contains("--account-preview") {
  AccountWindow.start(preview: isPreview)
  app.run()
  if isPreview { try? FileManager.default.removeItem(at: learningDirectory) }
  if isPreview { try? FileManager.default.removeItem(at: aiDirectory) }
  exit(0)
}
if arguments.contains("--sync-learning") {
  app.setActivationPolicy(.prohibited)
  Task { @MainActor in
    await AccountWindow.sync()
    exit(0)
  }
  app.run()
  exit(0)
}
if arguments.contains("--setup-translation") {
  TranslationSetup.start()
  app.run()
  exit(0)
}
if !isPreview { ServiceLease.start() }
do {
  guard let resources = Bundle.main.resourceURL else { throw EngineFailure.unavailable }
  let memoryURL =
    isPreview
    ? nil
    : FileManager.default.homeDirectoryForCurrentUser
      .appendingPathComponent(
        "Library/Application Support/BilingualCompanion/PersonalVocabulary/words.json")
  Runtime.engine = try EngineClient(resources: resources, memoryURL: memoryURL)
  if arguments.contains("--preview-learning") { try Preview.prepareLearning(resources: resources) }
} catch { Runtime.failure = error.localizedDescription }

if isPreview {
  Preview.start()
  app.run()
  Runtime.engine?.terminate()
  try? FileManager.default.removeItem(at: learningDirectory)
  try? FileManager.default.removeItem(at: aiDirectory)
  if let directory = Preview.learningDirectory {
    try? FileManager.default.removeItem(at: directory)
  }
} else {
  InputPresentation.configure()
  // 明确引用注册的ObjC类，避免仅字符串查找时退回基类。
  InputDiagnostics.controllerRegistered = NSClassFromString("BilingualInputController") != nil
  guard NSStringFromClass(BilingualInputController.self) == "BilingualInputController",
    NSClassFromString("BilingualInputController") == BilingualInputController.self
  else {
    fputs("输入控制器注册失败\n", stderr)
    exit(1)
  }
  let server = IMKServer(
    name: "org.local.bilingualcompanion_Connection",
    bundleIdentifier: "org.local.bilingualcompanion")
  guard server != nil else {
    fputs("InputMethodKit 服务启动失败\n", stderr)
    exit(1)
  }
  InputDiagnostics.start()
  RuntimeSettings.start()
  withExtendedLifetime(server) { app.run() }
}
