// 系统输入法服务入口；预览与自测为独立模式，不冒充系统接入验收。
import AppKit
import InputMethodKit

let arguments = CommandLine.arguments
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

let app = NSApplication.shared
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
if arguments.contains("--setup-translation") {
  TranslationSetup.start()
  app.run()
  exit(0)
}
let isPreview =
  arguments.contains("--preview") || arguments.contains("--preview-sentence")
  || arguments.contains("--preview-paging") || arguments.contains("--preview-learning")
  || arguments.contains("--preview-punctuation")
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
  if let directory = Preview.learningDirectory {
    try? FileManager.default.removeItem(at: directory)
  }
} else {
  app.setActivationPolicy(.prohibited)
  let server = IMKServer(
    name: "org.local.bilingualcompanion_Connection",
    bundleIdentifier: "org.local.bilingualcompanion")
  guard server != nil else {
    fputs("InputMethodKit 服务启动失败\n", stderr)
    exit(1)
  }
  withExtendedLifetime(server) { app.run() }
}
