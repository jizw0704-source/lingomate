// Independently curated literal Unicode mappings; no downloaded corpus or artwork.
import Foundation

enum SymbolCandidates {
  // Exact complete keywords only. No prefix, abbreviation or host-context matching.
  static let entries: [String: [String]] = {
    var result: [String: [String]] = [:]
    func add(_ keys: String, _ values: [String]) {
      for key in keys.split(separator: " ") { result[String(key)] = values }
    }
    add("zan dianzan", ["👍"])
    add("yumaoqiu", ["🏸"])
    add("kaixin", ["😊"])
    add("weixiao", ["🙂"])
    add("daxiao", ["😄"])
    add("ganxie xiexie", ["🙏"])
    add("qingzhu", ["🎉"])
    add("aixin", ["❤️"])
    add("zuqiu", ["⚽"])
    add("lanqiu", ["🏀"])
    add("pingpangqiu", ["🏓"])
    add("wangqiu", ["🎾"])
    add("kafei", ["☕"])
    add("shengluehao", ["……", "…"])
    add("zuojiantou", ["←", "<--", "⇐"])
    add("youjiantou", ["→", "-->", "⇒"])
    add("shangjiantou", ["↑"])
    add("xiajiantou", ["↓"])
    add("shuangxiangjiantou", ["↔", "⇔"])
    add("dayudengyu", ["≥"])
    add("xiaoyudengyu", ["≤"])
    add("budengyu", ["≠"])
    add("yuedengyu", ["≈"])
    add("zhengfuhao", ["±"])
    add("chenghao", ["×"])
    add("chuhao", ["÷"])
    add("wuxian wuxianda", ["∞"])
    add("sheshidu", ["℃"])
    add("huashidu", ["℉"])
    add("du", ["°"])
    add("pingfang", ["²"])
    add("lifang", ["³"])
    add("gouhao", ["√"])
    add("alpha aerfa", ["α", "Α"])
    add("beta beita", ["β", "Β"])
    add("gamma gama", ["γ", "Γ"])
    add("delta deerta", ["δ", "Δ"])
    add("epsilon aipuxilong", ["ε", "Ε"])
    add("zeta jieta", ["ζ", "Ζ"])
    add("eta yita", ["η", "Η"])
    add("theta xita", ["θ", "Θ"])
    add("iota", ["ι", "Ι"])
    add("kappa kapa", ["κ", "Κ"])
    add("lambda lamuda", ["λ", "Λ"])
    add("mu miu", ["μ", "Μ"])
    add("nu niu", ["ν", "Ν"])
    add("xi kexi", ["ξ", "Ξ"])
    add("omicron", ["ο", "Ο"])
    add("pi pai", ["π", "Π"])
    add("rho rou", ["ρ", "Ρ"])
    add("sigma xigema", ["σ", "Σ"])
    add("tau tao", ["τ", "Τ"])
    add("upsilon", ["υ", "Υ"])
    add("phi fai", ["φ", "Φ"])
    add("chi", ["χ", "Χ"])
    add("psi puxi", ["ψ", "Ψ"])
    add("omega oumiga", ["ω", "Ω"])
    return result
  }()

  static func outputs(for input: String) -> [String] {
    let key = input.lowercased().replacingOccurrences(of: "'", with: "")
    return entries[key] ?? []
  }

  static func augment(_ frame: EngineFrame, enabled: Bool) -> EngineFrame {
    guard enabled else { return frame }
    let symbols = outputs(for: frame.input).filter { output in
      !frame.candidates.contains { $0.text == output }
    }.prefix(3).map {
      EngineCandidate(text: $0, syllables: [], translations: [], hasDetails: false, symbol: true)
    }
    guard !symbols.isEmpty else { return frame }
    // Preserve the first two ordinary candidates; original ordering stays stable.
    let split = min(2, frame.candidates.count)
    let candidates =
      Array(frame.candidates.prefix(split)) + symbols
      + frame.candidates.dropFirst(split).prefix(64 - split - symbols.count)
    return EngineFrame(
      input: frame.input, marked: frame.marked, candidates: candidates,
      committed: frame.committed, learningToken: frame.learningToken, editToken: frame.editToken,
      memoryWarning: frame.memoryWarning, engineIdentity: frame.engineIdentity)
  }

  // Revalidate against the current exact input; never trust a stale presentation flag.
  static func output(_ candidate: EngineCandidate, input: String, sense: Int?) -> String? {
    guard sense == nil, candidate.symbol == true, candidate.syllables.isEmpty,
      candidate.translations.isEmpty, outputs(for: input).contains(candidate.text)
    else { return nil }
    return candidate.text
  }
}
