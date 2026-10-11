import AppKit
import UniformTypeIdentifiers

final class LocalLearningController: NSObject, NSTableViewDataSource, NSTableViewDelegate,
  NSSearchFieldDelegate
{
  let contentView = NSView()
  let table = NSTableView()
  let search = NSSearchField()
  let filter = NSPopUpButton()
  let status = NativeTheme.label("", size: 13, secondary: true)
  let store: LocalLearningStore
  private var words: [LearnedWord] = []
  private var visible: [LearnedWord] = []
  private var failed = false
  private var master: ActionButton!
  private var remove: ActionButton!
  init(store: LocalLearningStore) {
    self.store = store
    super.init()
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    contentView.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
      stack.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
      stack.topAnchor.constraint(equalTo: contentView.topAnchor),
      stack.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
    ])
    func add(_ view: NSView) {
      stack.addArrangedSubview(view)
      view.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    }
    add(NativeTheme.label("本地单词学习", size: 20, weight: .medium))
    add(NativeTheme.label("英文候选确认后自动记录；掌握状态由你标记。仅保存在本机，不自动上传。", size: 13, secondary: true))
    search.placeholderString = "搜索英文或中文"
    search.delegate = self
    search.setAccessibilityLabel("搜索本地单词")
    search.heightAnchor.constraint(equalToConstant: 44).isActive = true
    filter.addItems(withTitles: ["全部", "已学习", "已掌握"])
    filter.target = self
    filter.action = #selector(filterChanged)
    filter.heightAnchor.constraint(equalToConstant: 44).isActive = true
    let tools = NSStackView(views: [search, filter])
    tools.spacing = 8
    search.widthAnchor.constraint(greaterThanOrEqualToConstant: 180).isActive = true
    add(tools)
    for (key, title, width) in [
      ("english", "英文 / 词性", 180.0), ("chinese", "中文", 100.0), ("uses", "次数", 50.0),
      ("mastered", "状态", 75.0),
    ] {
      let column = NSTableColumn(identifier: .init(key))
      column.title = title
      column.width = width
      column.minWidth = key == "english" ? 100 : 45
      table.addTableColumn(column)
    }
    table.rowHeight = 44
    table.delegate = self
    table.dataSource = self
    table.allowsEmptySelection = true
    table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
    table.setAccessibilityLabel("本地学习词条")
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    scroll.borderType = .bezelBorder
    scroll.documentView = table
    scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 140).isActive = true
    add(scroll)
    add(status)
    master = ActionButton("标记掌握") { [weak self] in self?.toggleMastered() }
    remove = ActionButton("删除词条") { [weak self] in self?.deleteSelected() }
    let refresh = ActionButton("刷新") { [weak self] in self?.reload() }
    let export = ActionButton("导出全部…") { [weak self] in self?.exportAll() }
    let actions = NSStackView(views: [master, remove, refresh, export])
    actions.spacing = 4
    actions.distribution = .fillEqually
    add(actions)
    reload()
  }
  func reload() {
    do {
      words = try store.snapshot()
      failed = false
    } catch {
      words = []
      failed = true
    }
    filterChanged()
  }
  @objc private func filterChanged() {
    let selectedID = selected?.id
    let query = search.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
    visible = words.filter {
      (filter.indexOfSelectedItem == 0 || $0.mastered == (filter.indexOfSelectedItem == 2))
        && (query.isEmpty || $0.english.localizedCaseInsensitiveContains(query)
          || $0.chinese.contains(query))
    }
    table.reloadData()
    if let index = visible.firstIndex(where: { $0.id == selectedID }) {
      table.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false)
    } else {
      table.deselectAll(nil)
    }
    status.stringValue =
      failed
      ? "记录读取失败，原文件已保留。请检查存储权限后刷新。"
      : words.isEmpty
        ? "暂无记录。确认输出一个英文候选即可开始。"
        : "共 \(words.count) 个词条 · 已掌握 \(words.filter(\.mastered).count) · 当前显示 \(visible.count)"
    updateSelection()
  }
  func controlTextDidChange(_ notification: Notification) { filterChanged() }
  func numberOfRows(in tableView: NSTableView) -> Int { visible.count }
  func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView?
  {
    guard visible.indices.contains(row) else { return nil }
    let word = visible[row]
    let value: String
    switch tableColumn?.identifier.rawValue {
    case "english": value = word.english + (word.pos.isEmpty ? "" : "  " + word.pos)
    case "chinese": value = word.chinese
    case "uses": value = String(word.uses)
    default: value = word.mastered ? "已掌握" : "已学习"
    }
    let label = NativeTheme.label(value, size: 13)
    label.lineBreakMode = .byTruncatingTail
    label.toolTip = value
    label.setAccessibilityLabel(value)
    return label
  }
  var selected: LearnedWord? {
    visible.indices.contains(table.selectedRow) ? visible[table.selectedRow] : nil
  }
  func tableViewSelectionDidChange(_ notification: Notification) { updateSelection() }
  private func updateSelection() {
    master.isEnabled = selected != nil && !failed
    remove.isEnabled = selected != nil && !failed
    master.title = selected?.mastered == true ? "重新学习" : "标记掌握"
    master.setAccessibilityLabel(master.title)
  }
  func toggleMastered() {
    guard let selected else { return }
    do {
      try store.setMastered(selected.id, !selected.mastered)
      reload()
    } catch { status.stringValue = "未能保存掌握状态，请刷新后重试。" }
  }
  private func deleteSelected() {
    guard let selected else { return }
    let alert = NSAlert()
    alert.messageText = "删除“\(selected.english)”的本地记录？"
    alert.informativeText = "再次确认输出该词时，会重新建立记录。"
    alert.addButton(withTitle: "删除")
    alert.addButton(withTitle: "取消")
    guard alert.runModal() == .alertFirstButtonReturn else { return }
    do {
      try store.delete(selected.id)
      reload()
    } catch { status.stringValue = "删除失败，请刷新后重试。" }
  }
  private func exportAll() {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [.commaSeparatedText]
    panel.nameFieldStringValue = "LingoMate-words.csv"
    guard panel.runModal() == .OK, let url = panel.url else { return }
    do {
      try LocalLearningStore.csv(store.snapshot()).write(to: url, options: .atomic)
      status.stringValue = "已导出全部本地词条。"
    } catch { status.stringValue = "导出失败，记录保持不变。请刷新或选择其他位置。" }
  }
}
