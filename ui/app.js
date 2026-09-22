/* 前端只負責畫面與使用者操作。憑證、網路、磁碟及 Outlook 均由 Rust 管理。 */
"use strict";
const $ = (id) => document.getElementById(id);
let state = {
  messages: [],
  conversations: [],
  models: [],
  notifications: [],
  config: {
    font_size: 14,
    hotkey: "Win+Esc",
    sidebar_collapsed: false,
    notification_popups: true,
  },
  busy: "none",
  logged_in: false,
  can_send: false,
};
let activeView = "chat",
  lastConversation,
  messageSignature = "",
  lastDraftRevision = -1,
  stickToBottom = true,
  confirmAction = null;
let draftTimer, toastTimer;
let hotkeyRecording = false,
  hotkeyDraft = null;
const bridge = window.chrome?.webview;
function send(command) {
  if (bridge) bridge.postMessage(command);
  else if (window.previewHost) window.previewHost(command);
}
function toast(text) {
  $("toast").textContent = text;
  $("toast").hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    $("toast").hidden = true;
  }, 2600);
}
function ask(title, message, action) {
  $("confirm-title").textContent = title;
  $("confirm-message").textContent = message;
  confirmAction = action;
  $("confirm-dialog").showModal();
}
$("confirm-ok").onclick = () => {
  const action = confirmAction;
  confirmAction = null;
  $("confirm-dialog").close();
  action?.();
};
$("confirm-cancel").onclick = () => {
  $("confirm-dialog").close();
  confirmAction = null;
};
const md = window.markdownit({
  html: false,
  linkify: true,
  typographer: false,
  breaks: false,
});
md.use(window.markdownitFootnote).use(window.markdownitTaskLists, {
  enabled: false,
});
md.use(texmath, {
  engine: katex,
  delimiters: ["dollars", "brackets"],
  katexOptions: {
    trust: false,
    throwOnError: false,
    strict: "warn",
    maxExpand: 500,
    maxSize: 20,
    output: "htmlAndMathml",
  },
});
md.renderer.rules.fence = (tokens, index) => {
  const token = tokens[index],
    language = (token.info.trim().split(/\s+/)[0] || "text").toLowerCase();
  let content = md.utils.escapeHtml(token.content);
  if (hljs.getLanguage(language)) {
    try {
      content = hljs.highlight(token.content, {
        language,
        ignoreIllegals: true,
      }).value;
    } catch {
      /* 保留可讀的原始程式碼。 */
    }
  }
  return `<div class="code-block"><div class="code-heading"><span>${md.utils.escapeHtml(language)}</span><button type="button" class="copy-code">${icon("copy")}複製</button></div><pre><code class="hljs">${content}</code></pre></div>`;
};
// 遠端圖片不自動載入，避免將內網閱讀行為或識別資料傳到圖片主機。
md.renderer.rules.image = (tokens, index) =>
  `<span class="external-image">[圖片：${md.utils.escapeHtml(tokens[index].content || "未命名圖片")}]</span>`;
function renderMarkdown(text) {
  try {
    return DOMPurify.sanitize(md.render(text), {
      USE_PROFILES: { html: true, svg: true, mathMl: true },
      ADD_TAGS: ["eq", "eqn"],
      FORBID_TAGS: ["style", "iframe", "form"],
      FORBID_ATTR: ["srcset"],
    });
  } catch {
    return `<p>${md.utils.escapeHtml(text)}</p>`;
  }
}
/** 僅折疊明確的 Answer → Key points → Sources 格式；保留原文與完整 Markdown 渲染。 */
function renderAssistantReply(text) {
  const root = document.createElement("div");
  root.innerHTML = renderMarkdown(text);
  const sections = [];
  function sectionName(element) {
    // 只辨識最外層標題／段落／清單項，不把程式碼、引用或巢狀條列當成制式章節。
    const first = element.tagName === "LI" ? [...element.childNodes].find((node) => node.textContent.trim()) : element;
    const line = (first?.textContent || "").trim().split("\n")[0]
      .replace(/^\s*[1-5][.)、]\s*/, "").replace(/[：:]\s*$/, "").trim().toLowerCase();
    return ["answer", "key points", "sources", "confidence", "limitations"].includes(line) ? line : null;
  }
  for (const child of root.children) {
    const elements = child.tagName === "OL" ? [...child.children] :
      /^(H[1-6]|P)$/.test(child.tagName) ? [child] : [];
    for (const element of elements) {
      const name = sectionName(element);
      if (name) sections.push({ name, element });
    }
  }
  if (sections[0]?.name !== "answer" || sections[1]?.name !== "key points" || sections[2]?.name !== "sources") {
    return root.innerHTML;
  }
  const details = document.createElement("details");
  details.className = "answer-details";
  const summary = document.createElement("summary");
  summary.textContent = "來源、信心與限制";
  details.append(summary);
  const content = document.createElement("div");
  content.className = "answer-details-content";
  details.append(content);
  const source = sections[2].element;
  let following;
  if (source.tagName === "LI") {
    const originalList = source.parentElement;
    const tail = originalList.cloneNode(false);
    tail.start = (Number(originalList.getAttribute("start")) || 1) + [...originalList.children].indexOf(source);
    following = originalList.nextSibling;
    let next = source;
    while (next) { const after = next.nextSibling; tail.append(next); next = after; }
    content.append(tail);
    if (!originalList.children.length) originalList.remove();
  } else {
    following = source;
  }
  while (following) { const after = following.nextSibling; content.append(following); following = after; }
  root.append(details);
  return root.innerHTML;
}
function node(tag, className, text) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text !== undefined) el.textContent = text;
  return el;
}
function atBottom() {
  const box = $("transcript");
  return box.scrollHeight - box.clientHeight - box.scrollTop < 70;
}
function bottom() {
  const box = $("transcript");
  box.scrollTop = box.scrollHeight;
  stickToBottom = true;
  $("jump-bottom").hidden = true;
}
$("transcript").addEventListener(
  "scroll",
  () => {
    stickToBottom = atBottom();
    $("jump-bottom").hidden = stickToBottom || state.messages.length === 0;
  },
  { passive: true },
);
$("jump-bottom").onclick = bottom;
new ResizeObserver(() => {
  if (stickToBottom) bottom();
}).observe($("messages"));
document.fonts?.ready.then(() => {
  if (stickToBottom) bottom();
});

function showView(view) {
  if (view === "vnc" && !state.config.vnc_enabled) view = "chat";
  // receive() 會重繪目前頁面，只有真正跨頁才觸發一次離開操作。
  const previous = activeView;
  activeView = view;
  if (previous !== view) {
    if (previous === "tasks") send({ type: "work", command: { action: "clear_completed" } });
    if (previous === "notifications" && state.logged_in) {
      send({ type: "notifications_left" });
    }
  }
  for (const name of ["chat", "notifications", "outlook", "tasks", "vnc"])
    $(name + "-view").hidden = name !== view;
  $("show-notifications").classList.toggle("active", view === "notifications");
  $("show-outlook").classList.toggle("active", view === "outlook");
  $("show-tasks").classList.toggle("active", view === "tasks");
  $("show-vnc").classList.toggle("active", view === "vnc");
  const conversation = state.conversations.find(
    (c) => c.id === state.active_id,
  );
  $("page-title").textContent =
    view === "chat"
      ? conversation?.title || "新對話"
      : view === "notifications"
        ? "通知"
        : view === "tasks"
          ? "工作任務"
          : view === "vnc" ? "VNC 快速連線" : "Outlook 助理";
  $("save-label").hidden = view !== "chat";
  // 只在真正進入助理頁時更新，狀態推播重繪不重複查詢。
  if (previous !== view && view === "outlook") window.MailUI?.enter();
}
function renderMessages() {
  const signature = JSON.stringify([
    state.active_id,
    state.messages,
    state.busy === "chat",
  ]);
  if (signature === messageSignature) return;
  const changed = lastConversation !== state.active_id;
  const shouldFollow = changed || stickToBottom || atBottom();
  const oldTop = $("transcript").scrollTop;
  lastConversation = state.active_id;
  messageSignature = signature;
  const container = $("messages");
  const expanded = new Set(changed ? [] : [...container.querySelectorAll(".answer-details[open]")].map((details) => details.closest("article").dataset.index));
  container.replaceChildren();
  if (!state.messages.length && state.busy !== "chat") {
    const welcome = node("div", "welcome");
    welcome.innerHTML = `<span class="welcome-mark">${icon("sparkles")}</span><h2>讓想法，往前一步。</h2><p>從一個問題開始，或帶入正在處理的文字。<br>你的工作空間，隨時準備好。</p><div class="starter-grid"><button class="starter" data-starter="請幫我整理以下內容的重點：\n\n">${icon("list")}整理一段內容</button><button class="starter" data-starter="請幫我潤飾以下文字：\n\n">${icon("languages")}讓文字更清楚</button></div>`;
    container.append(welcome);
  }
  state.messages.forEach((message, index) => {
    const article = node("article", "message " + message.role);
    article.dataset.index = index;
    const avatar = node("div", "avatar");
    avatar.innerHTML = icon(message.role === "user" ? "user" : "chat");
    const content = node("div", "message-content");
    content.append(
      node("div", "message-meta", message.role === "user" ? "你" : "AI"),
    );
    const bubble = node(
      "div",
      "bubble" + (message.role === "assistant" ? " markdown" : ""),
    );
    if (message.role === "assistant") {
      bubble.innerHTML = renderAssistantReply(message.content);
      if (expanded.has(String(index))) bubble.querySelector(".answer-details")?.setAttribute("open", "");
      bubble.querySelectorAll("table").forEach((table) => {
        const wrapper = node("div", "table-wrap");
        table.replaceWith(wrapper);
        wrapper.append(table);
      });
    } else {
      bubble.textContent = message.content;
      if (message.attachments?.length)
        bubble.append(
          node(
            "div",
            "sent-attachments",
            "附件：" + message.attachments.join("、"),
          ),
        );
    }
    content.append(bubble);
    if (message.incomplete) content.append(node("p", "incomplete-warning", "回覆中斷，後續內容未收到；這裡保留已收到的部分。"));
    const tools = node("div", "message-tools");
    const copy = node("button", "copy-message");
    copy.dataset.index = index;
    copy.innerHTML = icon("copy") + "複製";
    copy.title = "複製原文";
    tools.append(copy);
    content.append(tools);
    article.append(avatar, content);
    container.append(article);
  });
  if (state.busy === "chat") {
    const item = node("div", "message assistant");
    item.innerHTML = `<div class="avatar">${icon("chat")}</div><div class="thinking">AI 正在整理回覆</div>`;
    container.append(item);
  }
  stickToBottom = shouldFollow;
  if (shouldFollow) bottom();
  else {
    $("transcript").scrollTop = oldTop;
    $("jump-bottom").hidden = false;
  }
}
let historySignature = "";
function renderHistory() {
  const list = $("history-list");
  const signature = JSON.stringify([state.conversations, state.active_id, state.busy]);
  // 背景進度推播不重建相同清單，保留鍵盤焦點與滑鼠所在的操作列。
  if (signature === historySignature) return;
  historySignature = signature;
  list.replaceChildren();
  const conversations = [...state.conversations].sort(
    (a, b) => Number(!!b.pinned) - Number(!!a.pinned) || b.updated_at - a.updated_at,
  );
  for (const c of conversations) {
    const row = node("div", "history-row" + (c.id === state.active_id ? " selected" : ""));
    const button = node(
      "button",
      "history-item",
      c.title,
    );
    button.title = (c.pinned ? "已置頂 · " : "") + c.title;
    button.dataset.id = c.id;
    button.disabled = state.busy !== "none";
    if (c.pinned) row.classList.add("pinned");
    const actions = node("div", "history-actions");
    for (const [action, symbol, label] of [
      ["rename", "pen", "編輯標題"],
      ["pin", "pin", c.pinned ? "取消置頂" : "置頂"],
      ["delete", "trash", "刪除對話"],
    ]) {
      const control = node("button", "icon-button");
      control.innerHTML = icon(symbol);
      control.dataset.historyAction = action;
      control.dataset.id = c.id;
      control.title = label;
      control.setAttribute("aria-label", `${label}：${c.title}`);
      control.disabled = state.busy !== "none";
      if (action === "pin") control.setAttribute("aria-pressed", String(!!c.pinned));
      actions.append(control);
    }
    row.append(button, actions);
    list.append(row);
  }
  if (!conversations.length)
    list.append(node("p", "empty-small", "開始對話後會自動保存"));
}
function renderModels() {
  const selected = state.models.find((m) => m.id === state.config.model);
  $("model-label").textContent = selected?.label || "尚無可用模型";
  $("model-button").disabled = state.busy !== "none" || !state.models.length;
  const menu = $("model-menu");
  menu.replaceChildren();
  for (const model of state.models) {
    const button = node("button");
    button.setAttribute("role", "option");
    button.setAttribute(
      "aria-selected",
      String(model.id === state.config.model),
    );
    button.dataset.model = model.id;
    const text = node("span", "", model.label);
    if (model.description)
      text.append(node("span", "model-description", model.description));
    button.append(text);
    menu.append(button);
  }
}
function renderNotifications() {
  $("notification-status").textContent =
    `網站：${state.site_status || "等待同步"} · AI：${state.notification_status || "等待同步"}`;
  $("unread-badge").textContent = state.unread_count || 0;
  $("unread-badge").hidden = !state.unread_count;
  const list = $("notification-list");
  list.replaceChildren();
  for (const event of state.notifications) {
    const isRead = event.source === "site" ? event.is_read : !!event.read_at;
    const card = node(
      "article",
      "notification-card" + (!isRead ? " unread" : ""),
    );
    const symbol = node("div", "avatar");
    symbol.innerHTML = icon("bell");
    const content = node("div", "event-content");
    content.append(
      node(
        "span",
        "subtle",
        event.source === "site" ? `網站 · ${event.origin || "全站"}` : "AI",
      ),
      node("h3", "", event.title),
      node("p", "", event.summary),
      node(
        "time",
        "subtle",
        new Date(event.created_at).toLocaleString("zh-TW"),
      ),
    );
    card.append(symbol, content);
    if (event.source === "site") {
      const button = node(
        "button",
        "secondary-button",
        event.url ? "開啟通知" : isRead ? "已讀" : "標為已讀",
      );
      button.disabled = !!state.site_mutating || (isRead && !event.url);
      button.onclick = () =>
        send({
          type: "site_action",
          command: { action: "read", id: event.id, open: !!event.url },
        });
      card.append(button);
    } else if (!isRead) {
      const read = node("button", "secondary-button", "標為已讀");
      read.dataset.event = event.id;
      card.append(read);
    }
    list.append(card);
  }
  if (!state.notifications.length)
    list.append(
      node(
        "div",
        "empty-small",
        state.logged_in ? "目前沒有通知" : "登入後即可查看網站通知",
      ),
    );
  $("refresh-notifications").disabled =
    !state.logged_in || state.notifications_loading || state.site_loading;
  $("read-all-events").disabled = !state.logged_in || state.site_mutating;
  $("clear-all-events").disabled = !state.logged_in || state.site_mutating;
}
function renderMail() {
  const mail = state.mail;
  $("read-mail").disabled = state.mail_busy;
  $("include-body").disabled = state.mail_busy;
  $("analyze-mail").disabled = !mail || state.mail_busy || !state.can_send;
  $("mail-actions").hidden = !mail;
  if (!mail) {
    $("mail-preview").replaceChildren(
      node(
        "p",
        "empty-small",
        "請在 Classic Outlook 選取一封郵件，再讀取預覽。",
      ),
    );
    return;
  }
  $("include-body").checked = mail.body !== null && mail.body !== undefined;
  const card = node("article", "mail-card");
  card.append(node("h3", "", mail.subject || "（無主旨）"));
  const fields = node("dl", "mail-fields");
  for (const [label, value] of [
    ["寄件者", mail.sender],
    ["收件者", mail.to],
    ["副本", mail.cc],
    ["收件時間", mail.received_at],
    ["狀態", mail.unread ? "未讀" : "已讀"],
  ]) {
    fields.append(node("dt", "", label), node("dd", "", value || "—"));
  }
  card.append(fields);
  if (mail.body !== null && mail.body !== undefined)
    card.append(node("pre", "mail-body", mail.body || "（內文空白）"));
  $("mail-preview").replaceChildren(card);
}
function receive(next) {
  state = next;
  document.documentElement.dataset.theme = state.config.dark_mode ? "dark" : "light";
  $("dark-mode").checked = !!state.config.dark_mode;
  document.documentElement.style.setProperty(
    "--font-size",
    state.config.font_size + "px",
  );
  document.body.classList.toggle("collapsed", state.config.sidebar_collapsed);
  $("collapse").title = state.config.sidebar_collapsed
    ? "展開側欄"
    : "收合側欄";
  $("collapse").setAttribute("aria-label", $("collapse").title);
  $("status").textContent = state.status || "";
  $("status").classList.toggle("error", !!state.error);
  $("account-label").textContent = state.logged_in ? "已登入" : "尚未登入";
  $("settings-account").textContent = state.logged_in
    ? "登入授權有效"
    : "使用瀏覽器取得公司服務授權";
  $("account-dot").classList.toggle("online", state.logged_in);
  $("send").disabled = !state.can_send;
  document
    .querySelectorAll("[data-action]")
    .forEach((button) => (button.disabled = !state.can_send));
  $("new-chat").disabled = state.busy !== "none";
  $("prompt").disabled = state.busy === "chat";
  $("login").disabled = state.busy !== "none" || state.update_required;
  $("logout").disabled = state.busy !== "none" || !state.logged_in;
  $("cancel-login").hidden = state.busy !== "login";
  $("reopen-login").hidden = state.busy !== "login" || !state.login_code;
  $("login-code").textContent = state.login_code
    ? "請核對登入碼：" + state.login_code
    : "";
  $("font-size").value = state.config.font_size;
  $("font-value").textContent = state.config.font_size + " px";
  if (!hotkeyRecording) $("hotkey").value = hotkeyDraft ?? state.config.hotkey;
  $("version-label").textContent =
    `LM_AI ${state.version} · ${state.version_status}`;
  $("notification-popups").checked = state.config.notification_popups;
  $("save-label").textContent = state.history_error
    ? "尚未保存"
    : state.busy === "chat"
      ? "回覆後自動保存"
      : "";
  if (next.draft_revision !== lastDraftRevision) {
    lastDraftRevision = next.draft_revision;
    $("prompt").value = next.draft || "";
    resizePrompt();
  }
  renderHistory();
  renderModels();
  renderMessages();
  renderNotifications();
  renderMail();
  showView(activeView);
  window.WorkUI?.render();
  window.MailUI?.render();
  window.BehaviorUI?.render();
  window.VncUI?.render();
  if (next.focus_draft) {
    showView("chat");
    $("prompt").focus();
    $("prompt").setSelectionRange(
      $("prompt").value.length,
      $("prompt").value.length,
    );
  }
}
function resizePrompt() {
  const input = $("prompt");
  input.style.height = "48px";
  input.style.height = Math.min(160, Math.max(48, input.scrollHeight)) + "px";
}
function submit(action = "send") {
  if (!state.can_send || window.WorkUI?.getFileBusy()) return;
  const text = $("prompt").value;
  if (!text.trim() && !state.work?.attachments?.length) {
    toast("請先輸入文字");
    return;
  }
  clearTimeout(draftTimer);
  stickToBottom = true;
  send({ type: "chat", text, action });
}
$("prompt").addEventListener("input", () => {
  resizePrompt();
  clearTimeout(draftTimer);
  draftTimer = setTimeout(
    () => send({ type: "draft", text: $("prompt").value }),
    250,
  );
});
$("prompt").addEventListener("keydown", (event) => {
  // IME 確認選字的 Enter 不得誤送；Shift+Enter 一律保留換行。
  if (event.key === "Enter" && !event.isComposing && event.keyCode !== 229 && !event.shiftKey && !event.altKey && !event.metaKey && (event.ctrlKey || state.config.enter_sends)) {
    event.preventDefault();
    submit();
  }
});
$("send").onclick = () => submit();
document
  .querySelectorAll("[data-action]")
  .forEach((button) => (button.onclick = () => submit(button.dataset.action)));
$("new-chat").onclick = () => {
  showView("chat");
  send({ type: "new_chat" });
};
$("collapse").onclick = () =>
  send({
    type: "preferences",
    font_size: state.config.font_size,
    sidebar_collapsed: !state.config.sidebar_collapsed,
    notification_popups: state.config.notification_popups,
  });
$("settings-button").onclick = () => {
  $("settings-dialog").showModal();
};
document.querySelector(".close-dialog").onclick = () => {
  $("settings-dialog").close();
};
$("font-size").addEventListener("input", () => {
  document.documentElement.style.setProperty(
    "--font-size",
    $("font-size").value + "px",
  );
  $("font-value").textContent = $("font-size").value + " px";
});
$("font-size").addEventListener("change", () =>
  send({
    type: "preferences",
    font_size: Number($("font-size").value),
    sidebar_collapsed: state.config.sidebar_collapsed,
    notification_popups: state.config.notification_popups,
  }),
);
$("notification-popups").onchange = () =>
  send({
    type: "preferences",
    font_size: state.config.font_size,
    sidebar_collapsed: state.config.sidebar_collapsed,
    notification_popups: $("notification-popups").checked,
  });
$("dark-mode").onchange = () => {
  document.documentElement.dataset.theme = $("dark-mode").checked ? "dark" : "light";
  send({ type: "preferences", font_size: state.config.font_size,
    sidebar_collapsed: state.config.sidebar_collapsed,
    notification_popups: state.config.notification_popups,
    dark_mode: $("dark-mode").checked });
};
$("save-hotkey").onclick = () =>
  send({ type: "hotkey", value: $("hotkey").value });
// 錄製期間暫停舊全域快捷鍵，避免它攔截正在錄製的同一組按鍵。
function startHotkeyRecording() {
  if (!hotkeyRecording) send({ type: "start_hotkey_recording" });
}
function setHotkeyRecording(active) {
  hotkeyRecording = active;
  $("hotkey").classList.toggle("recording", active);
  $("record-hotkey").disabled = active;
  $("save-hotkey").disabled = active;
  $("cancel-hotkey").hidden = !active;
  $("hotkey-record-status").textContent = active
    ? "請在 15 秒內按下組合鍵；單按 Esc 取消。"
    : "點輸入框或「錄製」，直接按組合鍵，再按套用。";
  if (active) {
    $("hotkey").value = "等待按鍵…";
    $("hotkey").focus();
  } else $("hotkey").value = hotkeyDraft ?? state.config.hotkey;
}
$("hotkey").addEventListener("focus", startHotkeyRecording);
$("hotkey").addEventListener("click", startHotkeyRecording);
$("record-hotkey").onclick = startHotkeyRecording;
$("cancel-hotkey").onclick = () => send({ type: "cancel_hotkey_recording" });
$("settings-dialog").addEventListener("close", () => {
  if (hotkeyRecording) send({ type: "cancel_hotkey_recording" });
});
$("settings-dialog").addEventListener("cancel", (event) => {
  if (hotkeyRecording) {
    event.preventDefault();
    send({ type: "cancel_hotkey_recording" });
  }
});
// 原生 WebView2 加速鍵為主要入口；DOM keydown 補足一般鍵及瀏覽器預覽。
// 使用實體 code，避免中文輸入法或不同鍵名大小寫影響組合辨識。
function keyBindingFromEvent(event) {
  const modifiers =
    (event.metaKey ? 8 : 0) |
    (event.ctrlKey ? 2 : 0) |
    (event.altKey ? 1 : 0) |
    (event.shiftKey ? 4 : 0);
  const names = {
    Escape: 27,
    Space: 32,
    Enter: 13,
    NumpadEnter: 13,
    Tab: 9,
    Backspace: 8,
    Delete: 46,
    Insert: 45,
    Home: 36,
    End: 35,
    PageUp: 33,
    PageDown: 34,
    ArrowLeft: 37,
    ArrowUp: 38,
    ArrowRight: 39,
    ArrowDown: 40,
    Minus: 189,
    Equal: 187,
    Comma: 188,
    Period: 190,
    Slash: 191,
    Semicolon: 186,
    Quote: 222,
    Backquote: 192,
    BracketLeft: 219,
    BracketRight: 221,
    Backslash: 220,
    NumpadAdd: 107,
    NumpadSubtract: 109,
    NumpadMultiply: 106,
    NumpadDivide: 111,
    NumpadDecimal: 110,
  };
  let key = names[event.code];
  if (/^Key[A-Z]$/.test(event.code)) key = event.code.charCodeAt(3);
  else if (/^Digit[0-9]$/.test(event.code)) key = event.code.charCodeAt(5);
  else if (/^Numpad[0-9]$/.test(event.code))
    key = 96 + Number(event.code.slice(6));
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(event.code))
    key = 111 + Number(event.code.slice(1));
  return key === undefined ? null : { modifiers, key };
}
$("hotkey").addEventListener("keydown", (event) => {
  if (!hotkeyRecording) return;
  event.preventDefault();
  event.stopPropagation();
  if (event.repeat || event.isComposing) return;
  const binding = keyBindingFromEvent(event);
  if (!binding) return;
  send(
    binding.key === 27 && binding.modifiers === 0
      ? { type: "cancel_hotkey_recording" }
      : { type: "recorded_hotkey", ...binding },
  );
});
for (const id of ["login", "logout", "refresh", "download"])
  $(id).onclick = () => send({ type: id });
$("cancel-login").onclick = () => send({ type: "cancel_login" });
$("reopen-login").onclick = () => send({ type: "reopen_login" });
$("model-button").onclick = () => {
  const open = $("model-menu").hidden;
  $("model-menu").hidden = !open;
  $("model-button").setAttribute("aria-expanded", String(open));
};
document.addEventListener("click", (event) => {
  const model = event.target.closest("[data-model]");
  if (model) {
    send({ type: "model", id: model.dataset.model });
    $("model-menu").hidden = true;
    $("model-button").setAttribute("aria-expanded", "false");
  }
  if (!event.target.closest(".model-picker")) {
    $("model-menu").hidden = true;
    $("model-button").setAttribute("aria-expanded", "false");
  }
  const historyAction = event.target.closest("[data-history-action]");
  if (historyAction && !historyAction.disabled) {
    const id = historyAction.dataset.id;
    const conversation = state.conversations.find(c => c.id === id);
    if (conversation) {
      if (historyAction.dataset.historyAction === "rename") window.BehaviorUI?.renameChat(id);
      else if (historyAction.dataset.historyAction === "pin") send({ type: "pin_chat", id, pinned: !conversation.pinned });
      else if (historyAction.dataset.historyAction === "delete") ask(
        "刪除此對話？",
        `「${conversation.title}」\n這會刪除此 Windows 使用者保存的整段對話，無法復原。`,
        () => send({ type: "delete_chat", id }),
      );
    }
  }
  const history = event.target.closest(".history-item");
  if (history) {
    showView("chat");
    send({ type: "select_chat", id: history.dataset.id });
  }
  const copy = event.target.closest(".copy-message");
  if (copy) {
    send({
      type: "copy",
      text: state.messages[Number(copy.dataset.index)].content,
    });
  }
  const code = event.target.closest(".copy-code");
  if (code) {
    send({
      type: "copy",
      text: code.closest(".code-block").querySelector("code").textContent,
    });
  }
  const starter = event.target.closest("[data-starter]");
  if (starter) {
    $("prompt").value = starter.dataset.starter;
    resizePrompt();
    $("prompt").focus();
    send({ type: "draft", text: $("prompt").value });
  }
  const read = event.target.closest("[data-event]");
  if (read) send({ type: "read_event", id: read.dataset.event });
  const link = event.target.closest(".markdown a");
  if (link) {
    event.preventDefault();
    const href = link.getAttribute("href") || "";
    if (href.startsWith("#")) {
      const target = document.getElementById(href.slice(1));
      target?.scrollIntoView();
    } else if (/^https?:\/\//i.test(href)) {
      ask("開啟連結", `使用預設瀏覽器開啟：\n${href}`, () =>
        send({ type: "open_link", url: href }),
      );
    } else toast("此連結類型不支援");
  }
});
$("show-notifications").onclick = () => showView("notifications");
$("show-outlook").onclick = () => showView("outlook");
$("refresh-notifications").onclick = () => send({ type: "refresh_events" });
$("read-mail").onclick = () => send({ type: "read_mail" });
$("include-body").onchange = () => {
  if ($("include-body").checked) {
    $("include-body").checked = false;
    ask(
      "讀取郵件內文？",
      "將讀取剛才選定郵件的純文字內文供你預覽，不包含附件；確認分析後才會傳給公司 AI。",
      () => send({ type: "read_mail_body" }),
    );
  } else send({ type: "omit_mail_body" });
};
$("analyze-mail").onclick = () =>
  ask(
    "將郵件資料送交 AI 分析？",
    state.mail?.body != null
      ? "將送出預覽中的郵件欄位與內文，分析結果會保存於本機。"
      : "只送出預覽中的主旨、寄件者、收件者、副本、時間與未讀狀態，不包含內文或附件。",
    () => {
      showView("chat");
      send({ type: "analyze_mail" });
    },
  );
function receiveHotkeyMessage(message) {
  if (message.type === "hotkey_recording") setHotkeyRecording(message.active);
  else if (message.type === "hotkey_recorded") {
    hotkeyDraft = message.value;
    setHotkeyRecording(false);
    $("hotkey").value = message.value;
    $("hotkey-record-status").textContent =
      "已錄製 " + message.value + "，按「套用」後生效。";
  } else if (message.type === "hotkey_saved") {
    hotkeyDraft = null;
    $("hotkey").value = message.value;
    $("hotkey-record-status").textContent = "目前使用 " + message.value;
  } else if (message.type === "hotkey_error")
    $("hotkey-record-status").textContent = message.message;
}
if (bridge) {
  bridge.addEventListener("message", (event) => {
    if (event.data.type === "state") receive(event.data.state);
    else if (event.data.type === "show_notifications")
      showView("notifications");
    else if (event.data.type === "show_tasks") showView("tasks");
    else if (event.data.type === "model_notice") window.BehaviorUI?.modelNotice(event.data.text);
    else if (event.data.type === "toast") toast(event.data.text);
    else if (event.data.type === "self_test") window.runSelfTest?.();
    else {
      receiveHotkeyMessage(event.data);
      window.WorkUI?.receive(event.data);
      window.VncUI?.receive(event.data);
    }
  });
  send({ type: "ready" });
}
// 本機 UI 預覽使用與桌面完全相同的 DOM／CSS；不載入憑證，也不呼叫公司 API。
window.LMUI = {
  keyBindingFromEvent,
  receiveHotkeyMessage,
  receive,
  showView,
  renderMarkdown,
  renderAssistantReply,
  bottom,
  atBottom,
  getState: () => state,
  getFollow: () => stickToBottom,
  setFollow: (value) => {
    stickToBottom = value;
  },
};

// 只處理真正的背景點擊，點內容留白或從內容拖到外面不誤關閉。
let settingsBackdropDown = false;
function outsideSettings(event) {
  const dialog = $("settings-dialog"),
    box = dialog.getBoundingClientRect();
  return (
    event.target === dialog &&
    (event.clientX < box.left ||
      event.clientX > box.right ||
      event.clientY < box.top ||
      event.clientY > box.bottom)
  );
}
$("settings-dialog").addEventListener("pointerdown", (event) => {
  settingsBackdropDown = outsideSettings(event);
});
$("settings-dialog").addEventListener("click", (event) => {
  if (settingsBackdropDown && outsideSettings(event))
    $("settings-dialog").close();
  settingsBackdropDown = false;
});

$("read-all-events").onclick = () => {
  send({ type: "all_events", dismiss: false });
  send({ type: "site_action", command: { action: "read_all" } });
};
$("clear-all-events").onclick = () =>
  ask(
    "刪除全站通知並清除 AI 通知？",
    "會透過網站 API 刪除你目前的全站鈴鐺通知，網站也會同步移除；AI 通知僅從本機清除，對話與任務保留。網站操作失敗時會保留全站通知。",
    () => {
      send({ type: "all_events", dismiss: true });
      send({ type: "site_action", command: { action: "delete_all" } });
    },
  );
