import Foundation

// Only synthetic bundles and temporary paths; system registration and signing are replaced.
func installerTests() throws {
  let files = FileManager.default
  for scenario in ["first", "replace", "running", "stage", "registration", "unrelated", "symlink"] {
    let root = files.temporaryDirectory.appendingPathComponent(
      "lingomate-installer-test-" + UUID().uuidString)
    try files.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? files.removeItem(at: root) }
    let source = root.appendingPathComponent("build/BilingualCompanion.app")
    let destination = root.appendingPathComponent("input/BilingualCompanion.app")
    func bundle(_ path: URL, _ content: String, _ identifier: String = Installer.identifier) throws
    {
      try files.createDirectory(
        at: path.appendingPathComponent("Contents/MacOS"), withIntermediateDirectories: true)
      try Data(content.utf8).write(to: path.appendingPathComponent(Installer.binary))
      let data = try PropertyListSerialization.data(
        fromPropertyList: ["CFBundleIdentifier": identifier], format: .xml, options: 0)
      try data.write(to: path.appendingPathComponent("Contents/Info.plist"))
    }
    try bundle(source, "new")
    if scenario != "first" {
      try bundle(
        destination, "old", scenario == "unrelated" ? "test.unrelated" : Installer.identifier)
    }
    let original = destination
    if scenario == "symlink" {
      let other = root.appendingPathComponent("unrelated.app")
      try files.moveItem(at: destination, to: other)
      try files.createSymbolicLink(at: destination, withDestinationURL: other)
    }
    var registered = false
    var installer = Installer(
      source: source, destination: destination, backups: root.appendingPathComponent("backups"))
    installer.run = { executable, arguments in
      if executable == "/bin/ps" {
        return scenario == "running"
          ? destination.appendingPathComponent(Installer.binary).path : ""
      }
      if executable == "/usr/bin/codesign" {
        if scenario == "stage", arguments.last != source.path {
          throw InstallFailure(message: "synthetic invalid stage")
        }
        return ""
      }
      if executable == Installer.registrar
        || executable == destination.appendingPathComponent(Installer.binary).path
      {
        if executable == Installer.registrar, arguments.first == "-f" {
          registered = true
          if scenario == "registration",
            try Data(contentsOf: destination.appendingPathComponent(Installer.binary))
              == Data("new".utf8)
          {
            throw InstallFailure(message: "synthetic registration failure")
          }
        }
        return ""
      }
      return try Installer.command(executable, arguments)
    }
    var succeeded = false
    var backup: URL?
    do {
      backup = try installer.install()
      succeeded = true
    } catch {}
    let expectedSuccess = scenario == "first" || scenario == "replace"
    guard succeeded == expectedSuccess else {
      throw InstallFailure(message: "selftest result: " + scenario)
    }
    let expected = expectedSuccess ? "new" : "old"
    guard
      try Data(contentsOf: original.appendingPathComponent(Installer.binary)) == Data(expected.utf8)
    else {
      throw InstallFailure(message: "original files changed: " + scenario)
    }
    if scenario == "replace" {
      guard let backup else { throw InstallFailure(message: "verified backup missing") }
      let extracted = root.appendingPathComponent("archive")
      _ = try Installer.command("/usr/bin/ditto", ["-x", "-k", backup.path, extracted.path])
      guard
        try Data(
          contentsOf: extracted.appendingPathComponent("BilingualCompanion.app/" + Installer.binary)
        ) == Data("old".utf8)
      else {
        throw InstallFailure(message: "backup differs")
      }
    }
    if ["running", "stage", "unrelated", "symlink"].contains(scenario), registered {
      throw InstallFailure(message: "unsafe registration in " + scenario)
    }
    guard
      !(try files.contentsOfDirectory(atPath: destination.deletingLastPathComponent().path))
        .contains(where: { $0.hasPrefix(".lingomate-") && $0 != ".lingomate-install.lock" })
    else {
      throw InstallFailure(message: "temporary application retained")
    }
  }
  print("Installer: 7 isolated first-install, replacement, backup and failure checks passed.")
}
