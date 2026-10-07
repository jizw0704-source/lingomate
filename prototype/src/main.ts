import "./style.css";
import {
  type Candidate,
  type EngineResult,
  insertText,
  normalizePinyin,
} from "./model";

function element<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element: ${id}`);
  return found as T;
}
const pinyin = element<HTMLInputElement>("pinyin");
const output = element<HTMLTextAreaElement>("output");
const candidates = element<HTMLDivElement>("candidates");
const details = element<HTMLDivElement>("details");
const queryStatus = element<HTMLDivElement>("query-status");
const expand = element<HTMLButtonElement>("expand");
const collapse = element<HTMLButtonElement>("collapse");
let result: EngineResult = {
  input: "",
  marked: "",
  candidates: [],
  committed: null,
};
let active = 0;
let bilingual = true;
let expanded = false;
let pending = false;
let committing = false;
let revision = 0;
let controller: AbortController | undefined;
let pendingQuery: Promise<void> = Promise.resolve();
let lastEdit: { value: string; cursor: number } | undefined;
let selection = { start: 0, end: 0 };
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

function text(tag: string, content: string, className = "") {
  const node = document.createElement(tag);
  node.textContent = content;
  node.className = className;
  return node;
}
function button(content: string, label: string, className: string) {
  const node = document.createElement("button");
  node.type = "button";
  node.className = className;
  node.textContent = content;
  node.setAttribute("aria-label", label);
  node.disabled = pending || committing;
  return node;
}
function announce(message: string) {
  const notice = element<HTMLDivElement>("notice");
  if (noticeTimer) clearTimeout(noticeTimer);
  notice.textContent = message;
  notice.hidden = false;
  noticeTimer = setTimeout(() => {
    notice.hidden = true;
  }, 2600);
}
function updateOutput() {
  element("count").textContent = `${Array.from(output.value).length} 字符`;
  for (const id of ["clear", "copy"])
    element<HTMLButtonElement>(id).disabled = !output.value || committing;
  element<HTMLButtonElement>("undo").disabled = !lastEdit || committing;
}
function current(): Candidate | undefined {
  return result.candidates[active];
}

function renderDetails() {
  const candidate = current();
  details.replaceChildren();
  element("detail-title").textContent =
    bilingual && candidate ? candidate.text : "译法详情";
  collapse.hidden = !expanded || !bilingual;
  expand.hidden = !bilingual || !candidate?.translations.length || expanded;
  expand.disabled = pending || committing;
  expand.setAttribute("aria-expanded", String(expanded));
  collapse.setAttribute("aria-expanded", String(expanded));
  element("detail-note").hidden = !bilingual || !candidate;
  if (!bilingual) {
    element("detail-summary").textContent = "现在只显示中文候选。";
    details.append(
      text(
        "p",
        "切回“中英候选”，继续查看译法。拼音和已输出文本都会保留。",
        "empty-detail",
      ),
    );
    return;
  }
  if (!candidate) {
    element("detail-summary").textContent = "选中一个候选，查看英文表达。";
    details.append(
      text("p", "可以从 xuexi、kaifa 或 gongzuo 开始。", "empty-detail"),
    );
    return;
  }
  const senses = candidate.translations;
  element("detail-summary").textContent = senses.length
    ? `${senses.length} 种表达，按语境选择。`
    : "这个候选暂时没有英文译法。";
  element("detail-note").textContent = candidate.hasDetails
    ? "用法说明为原型示例，当前覆盖 30 个常用词。"
    : "译词来自青简本地词表，暂未提供详细用法。";
  if (!senses.length)
    details.append(text("p", "仍然可以直接选择中文输出。", "empty-detail"));
  for (const [index, sense] of (expanded
    ? senses
    : senses.slice(0, 2)
  ).entries()) {
    const row = text("div", "", "sense-row");
    const top = text("div", "", "sense-top");
    const title = text("div", "", "sense-title");
    title.append(text("strong", sense.word), text("span", sense.pos));
    const select = button(
      "输出英文",
      `输出英文 ${sense.word}，${sense.pos}`,
      "quiet sense-commit",
    );
    select.dataset.sense = String(index);
    top.append(title, select);
    row.append(top);
    if (expanded) {
      row.append(text("p", sense.note, "sense-note"));
      if (sense.example) row.append(text("p", sense.example, "sense-example"));
    }
    details.append(row);
  }
  if (!expanded && senses.length > 2)
    details.append(
      text(
        "p",
        `还有 ${senses.length - 2} 种表达，展开后可选择。`,
        "more-count",
      ),
    );
}

function renderCandidates() {
  candidates.replaceChildren();
  candidates.classList.toggle("chinese-only", !bilingual);
  element("english-heading").hidden = !bilingual;
  element("english-key").hidden = !bilingual;
  element("bilingual").setAttribute("aria-pressed", String(bilingual));
  element("chinese").setAttribute("aria-pressed", String(!bilingual));
  element("marked").textContent =
    result.input === pinyin.value ? result.marked : pinyin.value;
  const rows = result.candidates.slice(0, 5);
  for (const [index, candidate] of rows.entries()) {
    const row = text(
      "div",
      "",
      `candidate-row${index === active ? " active" : ""}`,
    );
    const chinese = button(
      candidate.text,
      `输出中文 ${candidate.text}`,
      "candidate-chinese",
    );
    chinese.prepend(text("span", String(index + 1), "candidate-number"));
    chinese.dataset.index = String(index);
    chinese.dataset.action = "chinese";
    if (index === active) chinese.setAttribute("aria-current", "true");
    row.append(chinese);
    if (bilingual) {
      const english = text("div", "", "english-options");
      for (const [senseIndex, sense] of candidate.translations
        .slice(0, 2)
        .entries()) {
        const option = button(
          sense.word,
          `输出英文 ${sense.word}（${candidate.text}）`,
          "candidate-english",
        );
        option.prepend(text("span", sense.pos, "pos"));
        option.dataset.index = String(index);
        option.dataset.sense = String(senseIndex);
        option.dataset.action = "english";
        english.append(option);
      }
      if (!candidate.translations.length)
        english.append(text("span", "暂无译词", "no-translation"));
      const more = button(
        "↗",
        `展开 ${candidate.text} 的更多译法`,
        "candidate-more",
      );
      more.dataset.index = String(index);
      more.dataset.action = "more";
      more.disabled ||= !candidate.translations.length;
      row.append(english, more);
    }
    candidates.append(row);
  }
  renderDetails();
}

async function api(
  action: string,
  payload: unknown,
  signal?: AbortSignal,
): Promise<EngineResult> {
  let response: Response;
  try {
    response = await fetch(`/api/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
      signal,
    });
  } catch (error) {
    if (signal?.aborted) throw error;
    throw new Error("本地引擎连接失败。请重新启动原型服务，再点击重试。");
  }
  let data: EngineResult & { error?: string };
  try {
    data = await response.json();
  } catch {
    throw new Error("本地引擎未连接，请确认原型服务已启动后重试。");
  }
  if (!response.ok || data.error)
    throw new Error(data.error || "查询失败，请重试。");
  return data;
}

function refresh() {
  controller?.abort();
  controller = new AbortController();
  const signal = controller.signal;
  const requestRevision = ++revision;
  const input = pinyin.value;
  pending = true;
  active = 0;
  expanded = false;
  element("retry").hidden = true;
  queryStatus.textContent = input
    ? "正在查询本地候选…"
    : "输入拼音后，候选会出现在这里。";
  renderCandidates();
  pendingQuery = (async () => {
    try {
      const data = await api("query", { input }, signal);
      if (requestRevision !== revision) return;
      result = data;
      queryStatus.textContent = input
        ? data.candidates.length
          ? "点击即可输出，展开查看用法。"
          : "没有匹配的中文候选，可按 Enter 输出原样拼音。"
        : "输入拼音后，候选会出现在这里。";
      element("engine-state").textContent = "青简本地引擎 · 离线查询";
    } catch (error) {
      if (signal.aborted || requestRevision !== revision) return;
      result = { input, marked: input, candidates: [], committed: null };
      queryStatus.textContent =
        error instanceof Error ? error.message : "查询失败，请重试。";
      element("retry").hidden = false;
      element("engine-state").textContent = "本地引擎连接失败";
    } finally {
      if (requestRevision === revision) {
        pending = false;
        renderCandidates();
      }
    }
  })();
}

function insertCommitted(committed: string) {
  lastEdit = { value: output.value, cursor: selection.start };
  const next = insertText(
    output.value,
    selection.start,
    selection.end,
    committed,
  );
  output.value = next.value;
  output.setSelectionRange(next.cursor, next.cursor);
  selection = { start: next.cursor, end: next.cursor };
  updateOutput();
  announce(`已输出：${committed}`);
}

async function commit(index: number, senseIndex?: number) {
  if (committing) return;
  const actionRevision = revision;
  await pendingQuery;
  if (committing || actionRevision !== revision) return;
  const candidate = result.candidates[index];
  if (
    !candidate ||
    result.input !== pinyin.value ||
    (senseIndex !== undefined && !bilingual)
  )
    return;
  const english =
    senseIndex === undefined
      ? undefined
      : candidate.translations[senseIndex]?.word;
  if (senseIndex !== undefined && !english) {
    announce("这个候选暂时没有英文译法。");
    return;
  }
  committing = true;
  pinyin.disabled = true;
  output.readOnly = true;
  renderCandidates();
  updateOutput();
  try {
    const data = await api("commit", {
      input: pinyin.value,
      candidate: candidate.text,
      syllables: candidate.syllables,
      english,
    });
    if (data.committed === null) throw new Error("提交失败，请重试。");
    insertCommitted(data.committed);
    pinyin.value = data.input;
    result = data;
    active = 0;
    expanded = false;
    revision++;
    queryStatus.textContent = data.input
      ? "剩余拼音已保留，可以继续选择。"
      : "输入下一段拼音，继续写。";
  } catch (error) {
    queryStatus.textContent =
      error instanceof Error ? error.message : "提交失败，请重试。";
    announce("没有写入文字，拼音已保留，请重新选择。");
  } finally {
    committing = false;
    pinyin.disabled = false;
    output.readOnly = false;
    renderCandidates();
    updateOutput();
    pinyin.focus();
  }
}

function showDetails(index: number) {
  active = index;
  expanded = true;
  renderCandidates();
  details.querySelector<HTMLButtonElement>("button")?.focus();
}
function setMode(value: boolean) {
  bilingual = value;
  expanded = false;
  renderCandidates();
  announce(
    value ? "已切换为中英候选，拼音已保留。" : "已切换为普通中文，拼音已保留。",
  );
}
element("bilingual").addEventListener("click", () => setMode(true));
element("chinese").addEventListener("click", () => setMode(false));
expand.addEventListener("click", () => showDetails(active));
function closeDetails() {
  expanded = false;
  renderDetails();
  expand.focus();
}
collapse.addEventListener("click", closeDetails);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && expanded) {
    event.preventDefault();
    closeDetails();
  }
});
candidates.addEventListener("click", (event) => {
  const target = (event.target as HTMLElement).closest<HTMLButtonElement>(
    "button",
  );
  if (!target || target.disabled) return;
  const index = Number(target.dataset.index);
  if (target.dataset.action === "more") showDetails(index);
  else
    void commit(
      index,
      target.dataset.action === "english"
        ? Number(target.dataset.sense)
        : undefined,
    );
});
details.addEventListener("click", (event) => {
  const target = (event.target as HTMLElement).closest<HTMLButtonElement>(
    "button[data-sense]",
  );
  if (target && !target.disabled)
    void commit(active, Number(target.dataset.sense));
});
pinyin.addEventListener("input", (event) => {
  if ((event as InputEvent).isComposing) return;
  const typed = pinyin.value;
  pinyin.value = normalizePinyin(typed);
  if (typed.toLowerCase() !== pinyin.value)
    announce("请用系统英文键盘输入拼音字母。");
  refresh();
});
pinyin.addEventListener("compositionend", () => {
  pinyin.value = normalizePinyin(pinyin.value);
  refresh();
});
pinyin.addEventListener("keydown", (event) => {
  if (event.isComposing || event.ctrlKey || event.metaKey || event.altKey)
    return;
  if (event.key === " " && pinyin.value) {
    event.preventDefault();
    if (!event.repeat)
      void commit(active, event.shiftKey && bilingual ? 0 : undefined);
  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const count = Math.min(5, result.candidates.length);
    if (!pending && count) {
      active = (active + (event.key === "ArrowDown" ? 1 : count - 1)) % count;
      expanded = false;
      renderCandidates();
    }
  } else if (/^[1-5]$/.test(event.key) && !event.shiftKey) {
    event.preventDefault();
    void commit(Number(event.key) - 1);
  } else if (event.key === "Enter" && pinyin.value && !committing) {
    event.preventDefault();
    insertCommitted(pinyin.value);
    pinyin.value = "";
    refresh();
  }
});
for (const event of ["select", "keyup", "click", "blur"] as const) {
  output.addEventListener(event, () => {
    selection = { start: output.selectionStart, end: output.selectionEnd };
  });
}
output.addEventListener("input", () => {
  selection = { start: output.selectionStart, end: output.selectionEnd };
  lastEdit = undefined;
  updateOutput();
});
element("retry").addEventListener("click", refresh);
element("clear-pinyin").addEventListener("click", () => {
  if (!committing) {
    pinyin.value = "";
    refresh();
    pinyin.focus();
  }
});
element("clear").addEventListener("click", () => {
  output.value = "";
  selection = { start: 0, end: 0 };
  lastEdit = undefined;
  updateOutput();
  announce("文本已清空。");
});
element("undo").addEventListener("click", () => {
  if (!lastEdit) return;
  output.value = lastEdit.value;
  output.setSelectionRange(lastEdit.cursor, lastEdit.cursor);
  selection = { start: lastEdit.cursor, end: lastEdit.cursor };
  lastEdit = undefined;
  updateOutput();
  announce("已撤销上次输出；继续输入下一段拼音。");
});
element("copy").addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(output.value);
    announce("文本已复制。");
  } catch {
    output.focus();
    output.select();
    announce("无法自动复制，文本已选中，请使用系统复制快捷键。");
  }
});
for (const example of document.querySelectorAll<HTMLButtonElement>(
  "[data-example]",
)) {
  example.addEventListener("click", () => {
    if (!committing) {
      pinyin.value = example.dataset.example ?? "";
      refresh();
      pinyin.focus();
    }
  });
}
refresh();
