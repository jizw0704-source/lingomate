// 通过系统公开接口注册本应用；登记与切换分开，便于恢复原输入源。
import AppKit
import Carbon

enum InputSources {
  static let identifier = "org.local.bilingualcompanion.Hans"

  static func property(_ source: TISInputSource, _ key: CFString) -> String? {
    guard let pointer = TISGetInputSourceProperty(source, key) else { return nil }
    return Unmanaged<CFString>.fromOpaque(pointer).takeUnretainedValue() as String
  }

  static func sources(id: String? = nil, includeDisabled: Bool = false) -> [TISInputSource] {
    // Carbon 的属性常量需先初始化 TIS；先获取列表，再按 ID 筛选。
    guard let result = TISCreateInputSourceList(nil, includeDisabled) else { return [] }
    let all = result.takeRetainedValue() as! [TISInputSource]
    guard let id else { return all }
    return all.filter { property($0, kTISPropertyInputSourceID) == id }
  }

  static func enabled(_ source: TISInputSource) -> Bool {
    guard let pointer = TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled) else {
      return false
    }
    return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(pointer).takeUnretainedValue())
  }

  static func checkEnabled() -> Int32 {
    let mode = sources(id: identifier, includeDisabled: true).contains(where: enabled)
    let parent = sources(id: "org.local.bilingualcompanion", includeDisabled: true).contains(
      where: enabled)
    return mode && parent ? 0 : 2
  }

  static func list(includeDisabled: Bool = false) {
    let current = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
    let currentID = property(current, kTISPropertyInputSourceID)
    let result = sources(includeDisabled: includeDisabled).map {
      source in
      [
        "id": property(source, kTISPropertyInputSourceID) ?? "",
        "name": property(source, kTISPropertyLocalizedName) ?? "",
        "current": property(source, kTISPropertyInputSourceID) == currentID,
        "enabled": enabled(source),
        "type": property(source, kTISPropertyInputSourceType) ?? "",
      ] as [String: Any]
    }
    let data = try! JSONSerialization.data(
      withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
    print(String(decoding: data, as: UTF8.self))
  }

  static func register() -> Int32 {
    let status = TISRegisterInputSource(Bundle.main.bundleURL as CFURL)
    guard status == noErr else {
      print("注册失败：\(status)")
      return 1
    }
    let parent = sources(id: "org.local.bilingualcompanion", includeDisabled: true)
    let found = parent + sources(id: identifier, includeDisabled: true)
    guard !found.isEmpty else {
      print("系统尚未发现新输入源；需要在键盘设置中添加，必要时重新登录。")
      return 2
    }
    for source in found {
      let result = TISEnableInputSource(source)
      guard result == noErr else {
        print("启用失败：\(result)")
        return 1
      }
    }
    // 系统重扫新包可能覆盖首次启用；回读必须使用新进程。
    for _ in 0..<12 {
      for source in found { _ = TISEnableInputSource(source) }
      RunLoop.current.run(until: Date(timeIntervalSinceNow: 1.5))
      let verify = Process()
      verify.executableURL = Bundle.main.executableURL
      verify.arguments = ["--check-enabled"]
      do {
        try verify.run()
        verify.waitUntilExit()
      } catch { return 1 }
      if verify.terminationStatus == 0 {
        print("已注册并启用；尚未切换当前输入源。")
        return 0
      }
    }
    print("已注册，但启用尚未生效；请在系统键盘设置中添加。")
    return 2
  }

  static func select(_ id: String) -> Int32 {
    guard let source = sources(id: id, includeDisabled: true).first(where: enabled) else {
      print("输入源尚未启用：\(id)")
      return 2
    }
    let result = TISSelectInputSource(source)
    guard result == noErr else {
      print("切换失败：\(result)")
      return 1
    }
    let current = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
    print("当前输入源：\(property(current, kTISPropertyInputSourceID) ?? "unknown")")
    return property(current, kTISPropertyInputSourceID) == id ? 0 : 2
  }

  static func disable() -> Int32 {
    let own =
      sources(id: identifier, includeDisabled: true)
      + sources(id: "org.local.bilingualcompanion", includeDisabled: true)
    for source in own {
      let result = TISDisableInputSource(source)
      if result != noErr { return 1 }
    }
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 1))
    print("实验输入法已停用；应用和词库仍保留。")
    return 0
  }
}
