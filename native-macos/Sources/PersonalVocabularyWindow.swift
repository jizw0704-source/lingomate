import AppKit

final class PersonalVocabularyController: NSObject, NSTableViewDataSource, NSTableViewDelegate,
  NSSearchFieldDelegate
{
  let contentView = NSView()
  let table = NSTableView()
  let search = NSSearchField()
  let status = NativeTheme.label("", size: 13, secondary: true)
  let store: PersonalVocabularyStore
  private(set) var words: [PersonalWord] = []
  private var visible: [PersonalWord] = []
  private(set) var busy = false
  private var failed = false
  private var message: String?
  private let queue = DispatchQueue(label: "lingomate.vocabulary.management")
  private var priority: ActionButton!
  private var remove: ActionButton!
  private var refresh: ActionButton!

  init(store: PersonalVocabularyStore) {
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
    add(NativeTheme.label("个人词库", size: 20, weight: .medium))
    add(NativeTheme.label("管理已确认输入的中文词。同一读音可设置一个优先词，取消后恢复自动排序。", size: 13, secondary: true))
    search.placeholderString = "搜索汉字、全拼或简拼"
    search.delegate = self
    search.setAccessibilityLabel("搜索个人词库")
    search.heightAnchor.constraint(equalToConstant: 44).isActive = true
    add(search)
    for (key, title, width) in [
      ("text", "词语", 130.0), ("reading", "读音", 150.0),
      ("count", "次数", 55.0), ("priority", "推荐", 75.0),
    ] {
      let column = NSTableColumn(identifier: .init(key))
      column.title = title
      column.width = width
      column.minWidth = key == "reading" ? 100 : key == "text" ? 80 : 50
      table.addTableColumn(column)
    }
    table.rowHeight = 44
    table.delegate = self
    table.dataSource = self
    table.allowsEmptySelection = true
    table.allowsMultipleSelection = false
    table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
    table.setAccessibilityLabel("个人中文词条")
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    scroll.borderType = .bezelBorder
    scroll.documentView = table
    scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 140).isActive = true
    add(scroll)
    add(status)
    priority = ActionButton("优先推荐") { [weak self] in self?.togglePriority() }
    remove = ActionButton("删除词条…") { [weak self] in self?.confirmDelete() }
    refresh = ActionButton("刷新") { [weak self] in self?.reload() }
    for (button, key, hint) in [
      (priority!, "p", "⌘P 设置或取消优先推荐"),
      (remove!, "d", "⌘D 打开删除确认"),
      (refresh!, "r", "⌘R 刷新个人词库"),
    ] {
      button.keyEquivalent = key
      button.keyEquivalentModifierMask = .command
      button.toolTip = hint
    }
    let actions = NSStackView(views: [priority, remove, refresh])
    actions.spacing = 8
    actions.distribution = .fillEqually
    add(actions)
    add(
      NativeTheme.label(
        "仅保存在本机。删除会移除该读音的全部记忆入口；基础词库仍可能提供同名候选，再次确认输入会重新记忆。", size: 12, secondary: true))
    updateSelection()
  }
  func reload() { perform { try $0.snapshot() } }
  func cleanup() {
    // 操作子进程限时3秒；退出预览前回收它，避免清理后再生成临时文件。
    queue.sync {}
    store.cleanup()
  }
  private func perform(
    success: String? = nil,
    _ operation: @escaping (PersonalVocabularyStore) throws -> [PersonalWord]
  ) {
    guard !busy else { return }
    busy = true
    message = nil
    status.stringValue = "正在读取或保存个人词库…"
    updateSelection()
    queue.async { [weak self] in
      guard let self else { return }
      let result = Result { try operation(self.store) }
      DispatchQueue.main.async {
        self.busy = false
        switch result {
        case .success(let words):
          self.words = words
          self.failed = false
          self.message = success
        case .failure(let error):
          self.words = []
          self.failed = true
          self.message = error.localizedDescription
        }
        self.filterChanged()
      }
    }
  }
  private func filterChanged() {
    let selectedID = selected?.id
    let query = search.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    let compact = query.replacingOccurrences(of: " ", with: "").replacingOccurrences(
      of: "'", with: "")
    visible = words.filter {
      query.isEmpty || $0.text.contains(query)
        || (!compact.isEmpty
          && ($0.syllables.joined().contains(compact)
            || $0.aliases.contains { $0.contains(compact) }))
    }
    table.reloadData()
    if let index = visible.firstIndex(where: { $0.id == selectedID }) {
      table.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false)
    } else {
      table.deselectAll(nil)
    }
    status.stringValue =
      message
      ?? (words.isEmpty
        ? "暂无个人词条。确认输入中文后，可在这里查看记忆。"
        : visible.isEmpty
          ? "未找到匹配词条。请修改搜索内容。"
          : "共 \(words.count) 个词条 · 当前显示 \(visible.count)")
    updateSelection()
  }
  func controlTextDidChange(_ notification: Notification) {
    guard !busy else { return }
    message = nil
    filterChanged()
  }
  func numberOfRows(in tableView: NSTableView) -> Int { visible.count }
  func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView?
  {
    guard visible.indices.contains(row) else { return nil }
    let word = visible[row]
    let value: String
    switch tableColumn?.identifier.rawValue {
    case "text": value = word.text
    case "reading": value = word.syllables.joined(separator: " ")
    case "count": value = String(word.count)
    default: value = word.prioritized ? "优先" : "自动"
    }
    let label = NativeTheme.label(value, size: 13)
    // 原生选中行会向cell转发强调背景样式，系统label色保持选中文字可读。
    label.textColor = .labelColor
    label.lineBreakMode = .byTruncatingTail
    label.toolTip = value
    label.setAccessibilityLabel(value)
    let cell = NSTableCellView()
    cell.textField = label
    cell.addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 8),
      label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -8),
      label.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
    ])
    return cell
  }
  var selected: PersonalWord? {
    visible.indices.contains(table.selectedRow) ? visible[table.selectedRow] : nil
  }
  func tableViewSelectionDidChange(_ notification: Notification) { updateSelection() }
  private func updateSelection() {
    priority.isEnabled = selected != nil && !busy && !failed
    remove.isEnabled = selected != nil && !busy && !failed
    refresh.isEnabled = !busy
    search.isEnabled = !busy
    priority.title = selected?.prioritized == true ? "取消优先" : "优先推荐"
    priority.setAccessibilityLabel(priority.title)
  }
  func togglePriority() {
    guard let selected, !busy, !failed else { return }
    perform(success: selected.prioritized ? "已恢复自动排序，下次查询生效。" : "已设置该读音的优先推荐，下次查询生效。") {
      try $0.prioritize(selected, enabled: !selected.prioritized)
    }
  }
  private func confirmDelete() {
    guard let selected, !busy, !failed, let window = contentView.window else { return }
    let alert = NSAlert()
    alert.messageText = "删除“\(selected.text)”（\(selected.syllables.joined(separator: " "))）的记忆？"
    alert.informativeText = "全拼、简拼等记忆入口和优先推荐一并移除。基础词库不会删除；再次确认该词会重新建立记忆。"
    alert.addButton(withTitle: "取消")
    alert.addButton(withTitle: "删除")
    alert.beginSheetModal(for: window) { [weak self] response in
      if response == .alertSecondButtonReturn { self?.deleteConfirmed(selected) }
    }
  }
  func deleteConfirmed(_ word: PersonalWord) {
    guard !busy, !failed else { return }
    perform(success: "已删除该读音的个人记忆，下次查询生效。") { try $0.delete(word) }
  }
}
