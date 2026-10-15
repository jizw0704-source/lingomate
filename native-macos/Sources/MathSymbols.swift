// Shared-reference project mappings: literal Unicode text, not a font or artwork pack.
import Foundation

struct MathSymbolEntry: Decodable {
  let category: String
  let name: String
  let aliases: [String]
  let outputs: [String]
}

enum MathSymbols {
  private struct Catalog: Decodable {
    let version: Int
    let license: String
    let entries: [MathSymbolEntry]
  }
  enum CatalogError: Error { case invalid }

  static func decode(_ data: Data) throws -> [MathSymbolEntry] {
    guard data.count <= 262_144 else { throw CatalogError.invalid }
    let catalog = try JSONDecoder().decode(Catalog.self, from: data)
    guard catalog.version == 1, catalog.license == "GPL-3.0-or-later",
      !catalog.entries.isEmpty, catalog.entries.count <= 512
    else { throw CatalogError.invalid }
    var aliases = Set<String>()
    for entry in catalog.entries {
      guard !entry.category.isEmpty, !entry.name.isEmpty,
        !entry.aliases.isEmpty, entry.aliases.count <= 16,
        (1...3).contains(entry.outputs.count), Set(entry.outputs).count == entry.outputs.count,
        entry.outputs.allSatisfy({
          !$0.isEmpty && $0.utf16.count <= 32
            && !$0.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) })
        })
      else { throw CatalogError.invalid }
      for alias in entry.aliases {
        guard !alias.isEmpty, alias.count <= 96,
          alias.utf8.allSatisfy({ (97...122).contains($0) }), aliases.insert(alias).inserted
        else { throw CatalogError.invalid }
      }
    }
    return catalog.entries
  }

  static let entries: [MathSymbolEntry] = {
    guard let resource = Bundle.main.url(forResource: "math-symbols", withExtension: "json"),
      let data = try? Data(contentsOf: resource), let entries = try? decode(data)
    else { return [] }
    return entries
  }()
}
