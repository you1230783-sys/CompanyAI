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
let lastStatusNotice = "";
let hotkeyRecording = false,
  hotkeyDraft = null;
const bridge = window.chrome?.webview;
function send(command) {
  if (window.AuthUI && !window.AuthUI.canSend(command)) return;
  if (bridge) bridge.postMessage(command);
  else if (window.previewHost) window.previewHost(command);
}
// 共用三秒通知，狀態重繪不重設計時；對話框開啟時也能看見錯誤。
function toast(text) {
  if (!text) return;
  const notice = $("toast");
  notice.textContent = text;
  notice.hidden = false;
  if (notice.showPopover) {
    // 若中途開了模態框，重新加入 top layer，避免新的錯誤提示被遮住。
    if (notice.matches(":popover-open")) notice.hidePopover();
    notice.showPopover();
  }
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    if (notice.hidePopover && notice.matches(":popover-open")) notice.hidePopover();
    notice.hidden = true;
  }, 3000);
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
$("confirm-dialog").addEventListener("backdrop-dismiss", () => { confirmAction = null; });
$("confirm-dialog").addEventListener("cancel", () => { confirmAction = null; });
$("confirm-dialog").addEventListener("close", () => {
  if (!$("confirm-dialog").open) confirmAction = null;
});
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
/** 舊格式只辨識編號章節，標籤與網頁端一致，避免正文出現關鍵字就被收合。 */
function replySectionName(element) {
  let title = element;
  if (element.tagName === "LI") {
    title = [...element.childNodes].find((child) => child.textContent.trim());
  }
  if (!title || (title.nodeType === Node.ELEMENT_NODE &&
      !/^(H[1-6]|P|STRONG|EM|SPAN)$/.test(title.tagName))) return null;
  // Markdown 編號可能已變成 <ol><li>，也可能仍在標題／粗體段落的文字內。
  const line = title.textContent.trim().split("\n")[0];
  const numbered = line.match(/^(?:\d+[.)、．]|[（(]\d+[）)])\s*(.*)$/);
  if (element.tagName !== "LI" && !numbered) return null;
  // 冒號後可直接接正文；必須先完整命中標籤，不接受「來源相關說明」等部分匹配。
  const name = (numbered ? numbered[1] : line).split(/[：:]/)[0]
    .replace(/\s+/g, " ").trim().toLowerCase();
  const aliases = {
    answer: "answer", content: "answer", "回答": "answer", "內容": "answer",
    keypoint: "key_points", "key points": "key_points", keypoints: "key_points", "回答重點": "key_points",
    source: "sources", sources: "sources", references: "sources", "引用資料庫": "sources", "內容引用處": "sources", "來源": "sources", "來源摘要": "sources",
    confidence: "confidence", "信心度": "confidence",
    limitation: "limitations", limitations: "limitations", "限制": "limitations", "回答限制": "limitations",
  };
  return Object.hasOwn(aliases, name) ? aliases[name] : null;
}

/**
 * 只收合已辨識的來源、信心及限制章節。回答與重點不論先後順序都保留在外。
 * 無法確認是制式回覆時完整顯示；不修改訊息原文，複製與歷史保存仍使用原文。
 */
function renderAssistantReply(text, payload = null, receivedReplies = []) {
  // 完成結果以欄位為準，不再從 answer 的文字內容猜測章節邊界。
  if (payload && typeof payload.answer === "string" && payload.answer.trim()) {
    return renderStructuredReply(payload, receivedReplies);
  }
  const root = document.createElement("div");
  const originalHtml = renderMarkdown(normalizeReplyHeadings(text));
  root.innerHTML = originalHtml;
  const blocks = [];
  for (const child of [...root.childNodes]) {
    if (child.nodeType !== Node.ELEMENT_NODE) {
      blocks.push({ element: child, name: null, level: 0 });
      continue;
    }
    // 編號清單可把多個章節包在同一個 <ol>；依章節拆開並保留原始編號。
    // 未命名的清單項繼續留在原章節，巢狀清單不參與章節辨識。
    if (child.tagName === "OL" && [...child.children].some(replySectionName)) {
      let group;
      let number = Number(child.getAttribute("start") || 1);
      for (const item of [...child.children]) {
        if (item.hasAttribute("value")) number = Number(item.getAttribute("value"));
        const name = replySectionName(item);
        if (!group || name) {
          group = child.cloneNode(false);
          group.start = number;
          blocks.push({ element: group, name, level: 0 });
        }
        group.append(item);
        number += 1;
      }
    } else {
      const heading = /^H[1-6]$/.test(child.tagName);
      blocks.push({
        element: child,
        name: heading || child.tagName === "P" ? replySectionName(child) : null,
        level: heading ? Number(child.tagName.slice(1)) : 0,
      });
    }
  }
  // 有正文及重點才啟用制式章節收合，避免普通文章的 Sources 標題被誤藏。
  const names = new Set(blocks.map((block) => block.name));
  if (!names.has("answer") || !names.has("key_points") ||
      !["sources", "confidence", "limitations"].some((name) => names.has(name))) {
    return originalHtml;
  }
  const details = document.createElement("details");
  details.className = "answer-details";
  const summary = document.createElement("summary");
  summary.textContent = "來源、信心與限制";
  details.append(summary);
  const content = document.createElement("div");
  content.className = "answer-details-content";
  details.append(content);
  root.replaceChildren();
  let secondary = false;
  let sectionLevel = 0;
  for (const block of blocks) {
    if (block.name) {
      secondary = ["sources", "confidence", "limitations"].includes(block.name);
      sectionLevel = block.level;
    } else if (block.level && (!sectionLevel || block.level <= sectionLevel)) {
      // 同級／上級的新標題結束前一章節；未知章節保持可見，避免吞掉補充正文。
      secondary = false;
      sectionLevel = block.level;
    }
    (secondary ? content : root).append(block.element);
  }
  if (content.children.length) root.append(details);
  return root.innerHTML;
}

/** 結構化回答的正文與重點固定可見；空欄位不產生空標題或空的收合區。 */
function renderStructuredReply(payload, receivedReplies = []) {
  const root = document.createElement("div");
  const answer = document.createElement("div");
  answer.className = "answer-body";
  answer.innerHTML = renderMarkdown(payload.answer);
  root.append(answer);
  const sections = payload.sections || {};
  function appendList(parent, title, values) {
    const items = (values || []).filter((value) => typeof value === "string" && value.trim());
    if (!items.length) return;
    parent.append(node("h3", "", title));
    const list = document.createElement("ul");
    for (const value of items) {
      const item = document.createElement("li");
      item.innerHTML = renderMarkdown(value);
      list.append(item);
    }
    parent.append(list);
  }
  appendList(root, "回答重點", sections.key_points);
  const content = node("div", "answer-details-content");
  appendList(content, "來源摘要", sections.sources);
  if (sections.confidence?.trim()) {
    content.append(node("h3", "", "信心度"));
    const confidence = document.createElement("div");
    confidence.innerHTML = renderMarkdown(sections.confidence);
    content.append(confidence);
  }
  appendList(content, "回答限制", sections.limitations);
  if (payload.citations?.length) {
    content.append(node("h3", "", "引用文件"));
    const list = document.createElement("ul");
    for (const citation of payload.citations) {
      const item = document.createElement("li");
      if (typeof citation === "string") {
        item.innerHTML = renderMarkdown(citation);
      } else {
        // 引用物件的 schema 尚未限定；完整唯讀呈現，禁止當成 HTML、路徑或指令執行。
        item.append(node("pre", "citation-data", JSON.stringify(citation, null, 2)));
      }
      list.append(item);
    }
    content.append(list);
  }
  for (const received of receivedReplies) {
    content.append(node("h3", "", received.from_stream ? "串流期間收到的原文" : "完整原始回覆"));
    // 尚無法安全對齊的文字明確另列，使用者仍能一鍵展開，不當成完成答案的新欄位。
    content.append(node("pre", "citation-data", received.text));
  }
  if (content.children.length) {
    const details = node("details", "answer-details");
    details.append(node("summary", "", receivedReplies.length ? "來源、信心、限制與原文" : "來源、信心與限制"), content);
    root.append(details);
  }
  return root.innerHTML;
}

/** 舊回覆可能將五段 Markdown 壓在同一行；僅拆完整的固定五段，保護程式碼範例。 */
function normalizeReplyHeadings(text) {
  let fence = null;
  return text.split("\n").map((line) => {
    const marker = line.trimStart().match(/^(`{3,}|~{3,})/)?.[0][0];
    if (fence || marker) {
      if (fence === marker) fence = null;
      else if (!fence) fence = marker;
      return line;
    }
    if (line.includes("`")) return line;
    const pattern = /(?:^|\s)(#{1,6}\s+\d+[.)、]\s+(Answer|Key points|Sources|Confidence|Limitations))(?=\s|[:：]|$)/gi;
    const headings = [...line.matchAll(pattern)];
    if (headings.length === 5 && headings[0].index === 0 &&
        headings.map((entry) => entry[2].toLowerCase()).join(",") === "answer,key points,sources,confidence,limitations") {
      return headings.map((entry, index) => {
        const body = line.slice(entry.index + entry[0].length, headings[index + 1]?.index ?? line.length).replace(/^\s*[:：]?\s*/, "");
        return entry[1] + "\n\n" + body;
      }).join("\n\n");
    }
    // 一般分行回覆也允許標題後直接接正文；限實際的 Markdown 編號標題。
    return line.replace(/^(#{1,6}\s+\d+[.)、]\s+(?:Answer|Key points|Sources|Confidence|Limitations))\s+(.+)$/i, "$1\n\n$2");
  }).join("\n");
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
  if (!state.logged_in) view = "chat";
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
  const project = view === "chat" && state.logged_in
    ? state.projects?.items?.find(item => item.id === conversation?.project_id) : null;
  $("project-context").textContent = project?.root || "";
  $("project-context").title = project?.root || "";
  $("project-context").hidden = !project;
  // 只在真正進入助理頁時更新，狀態推播重繪不重複查詢。
  if (previous !== view && view === "outlook") window.MailUI?.enter();
}
// 原生 runner 將 progress_note 與工具活動一起保存，舊對話不需要資料遷移。
const PROJECT_NOTE_PREFIX = "AI 進度筆記：";

/** 工具紀錄以純文字呈現；結束後用固定標題，避免長筆記出現在收合列。 */
function renderProjectActivity(details, events, finished = false) {
  const history = (events || []).filter(text => typeof text === "string").slice(-120);
  details.className = "project-activity";
  if (!history.length) { details.hidden = true; details.replaceChildren(); return; }
  details.hidden = false;
  const latest = history.at(-1);
  const label = finished ? "查看工具紀錄與進度筆記"
    : latest.startsWith(PROJECT_NOTE_PREFIX) ? "AI 已更新進度筆記" : latest;
  let summary = details.querySelector("summary");
  let list = details.querySelector(".project-activity-history");
  if (!list) {
    summary = node("summary"); list = node("ol", "project-activity-history");
    details.append(summary, list);
  }
  summary.textContent = label;
  const previous = [...list.children];
  const oldTop = list.scrollTop;
  const follow = details.open && list.scrollHeight - list.clientHeight - oldTop < 12;
  const top = list.getBoundingClientRect().top;
  const anchor = previous.find(item => item.getBoundingClientRect().bottom > top);
  const offset = anchor ? anchor.getBoundingClientRect().top - top : 0;
  // 找出舊尾段與新開頭的重疊；新增紀錄只 append，120筆輪替時仍保留可見的節點。
  let keep = 0;
  for (let count = Math.min(previous.length, history.length); count > 0; count--) {
    if (previous.slice(-count).every((item, index) => item.textContent === history[index])) {
      keep = count; break;
    }
  }
  previous.slice(0, previous.length - keep).forEach(item => item.remove());
  history.slice(keep).forEach(text => list.append(node("li", "", text)));
  if (follow) list.scrollTop = list.scrollHeight;
  else if (anchor?.isConnected && list.contains(anchor)) {
    list.scrollTop += anchor.getBoundingClientRect().top - list.getBoundingClientRect().top - offset;
  } else list.scrollTop = oldTop;
}

/** 僅供執行中的區域使用；累積筆記只顯示最新一份，舊版本留在工具紀錄。 */
function renderProjectNarration(container, events) {
  const history = (events || []).filter(text => typeof text === "string");
  const notes = history.filter(text => text.startsWith("AI 說明："));
  const latestNote = history.filter(text => text.startsWith(PROJECT_NOTE_PREFIX)).at(-1);
  // 一般工具活動沒有改變說明／筆記時，不重建正在閱讀的內容。
  const signature = JSON.stringify([notes, latestNote]);
  if (container.dataset.signature === signature) return;
  container.dataset.signature = signature;
  const previousNote = container.querySelector(".project-progress-note-text");
  const noteTop = previousNote?.scrollTop || 0;
  const followNote = previousNote && previousNote.scrollHeight - previousNote.clientHeight - noteTop < 12;
  container.replaceChildren();
  container.hidden = !notes.length && !latestNote;
  for (const note of notes) {
    const text = node("div", "project-narration markdown");
    text.innerHTML = renderMarkdown(note.slice(6));
    container.append(text);
  }
  if (latestNote) {
    const card = node("section", "project-progress-note");
    card.setAttribute("aria-label", "AI 進度筆記");
    card.append(node("strong", "", "AI 進度筆記"));
    // 筆記只作文字，保留換行；不解譯其中的 HTML 或可執行連結。
    card.append(node("div", "project-progress-note-text", latestNote.slice(PROJECT_NOTE_PREFIX.length)));
    container.append(card);
    const text = card.querySelector(".project-progress-note-text");
    text.scrollTop = followNote ? text.scrollHeight : noteTop;
  }
}

/** 通知只顯示短摘要；保留原資料與原本的開啟／已讀動作。 */
function notificationPreview(text) {
  const value = String(text || "").replace(/\s+/gu, " ").trim();
  const characters = typeof Intl.Segmenter === "function"
    ? [...new Intl.Segmenter("zh-TW", {granularity:"grapheme"}).segment(value)].map(part => part.segment)
    : Array.from(value);
  return characters.slice(0, 50).join("") + (characters.length > 50 ? "…" : "");
}

/** 只將專案內成果路徑轉成定位連結；原生端再次驗證目前專案與檔案邊界。
 * 同時支援舊版本的行內 code，以及模型產生的 Markdown 連結。
 */
function linkProjectArtifacts(bubble) {
  if (!state.conversations?.some(c => c.id === state.active_id && c.project_id)) return;
  for (const item of bubble.querySelectorAll("code, a")) {
    if (item.closest("pre")) continue;
    const value = item.tagName === "A" ? item.getAttribute("href") : item.textContent;
    if (!/^_AI_Output[/\\][^\r\n]+$/u.test(value || "")) continue;
    const link = node("a", "project-artifact", item.textContent);
    link.href = "#";
    link.dataset.path = value;
    link.dataset.conversation = state.active_id;
    link.title = "在檔案總管中顯示";
    item.replaceWith(link);
  }
  // 舊回覆有時直接列出一整行路徑，沒有 Markdown code 或連結標記。
  const walker = document.createTreeWalker(bubble, NodeFilter.SHOW_TEXT);
  const plain = [];
  while (walker.nextNode()) {
    if (!walker.currentNode.parentElement.closest("a,code,pre")) plain.push(walker.currentNode);
  }
  for (const text of plain) {
    const lines = text.textContent.split("\n");
    if (!lines.some(line => /^_AI_Output[/\\].+\.(txt|md|docx|doc|docm|xlsx|xls|xlsm|xlsb|pptx|ppt|pptm|png)$/iu.test(line.trim()))) continue;
    const fragment = document.createDocumentFragment();
    lines.forEach((line,index) => {
      if (index) fragment.append(document.createTextNode("\n"));
      const path = line.trim();
      if (/^_AI_Output[/\\].+\.(txt|md|docx|doc|docm|xlsx|xls|xlsm|xlsb|pptx|ppt|pptm|png)$/iu.test(path)) {
        const link = node("a","project-artifact",path);
        link.href = "#"; link.dataset.path = path; link.dataset.conversation = state.active_id;
        link.title = "在檔案總管中顯示"; fragment.append(link);
      } else fragment.append(document.createTextNode(line));
    });
    text.replaceWith(fragment);
  }
}

function renderMessages() {
  const signature = JSON.stringify([
    state.active_id,
    state.messages,
    state.busy === "chat",
    state.retry,
  ]);
  if (signature === messageSignature) {
    const retry = document.querySelector("#messages .retry-message");
    if (retry) retry.disabled = !state.retry?.enabled;
    const resume = document.querySelector("#messages .resume-project");
    if (resume) resume.disabled = !state.retry?.enabled || state.projects?.running;
    return;
  }
  const changed = lastConversation !== state.active_id;
  const shouldFollow = changed || stickToBottom || atBottom();
  const oldTop = $("transcript").scrollTop;
  lastConversation = state.active_id;
  messageSignature = signature;
  const container = $("messages");
  const expanded = new Set(changed ? [] : [...container.querySelectorAll(".answer-details[open]")].map((details) => details.closest("article").dataset.index));
  const activityExpanded = new Set(changed ? [] : [...container.querySelectorAll(".project-activity[open]")].map(details => details.closest("article").dataset.index));
  const activityScroll = new Map(changed ? [] : [...container.querySelectorAll(".project-activity-history")].map(list => [list.closest("article").dataset.index, list.scrollTop]));
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
      bubble.innerHTML = renderAssistantReply(message.content, message.response_payload, message.received_replies);
      linkProjectArtifacts(bubble);
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
    if (message.project_activity?.length) {
      // 最終答案與過程分開；完成、暫停或失敗後都只在收合紀錄保留中途說明。
      const details = document.createElement("details");
      details.open = activityExpanded.has(String(index));
      renderProjectActivity(details, message.project_activity, true);
      content.append(details);
    }
    content.append(bubble);
    if (message.project_analysis) {
      const analysis = node("div");
      AnalysisUI.render(analysis,message.project_analysis,true); content.append(analysis);
    }
    if (message.project_charts?.length) {
      const charts = node("div", "project-charts"); content.append(charts);
      ChartUI.render(charts, message.project_charts, {conversation:state.active_id,message_index:index,request_id:message.request_id,styles:message.project_chart_styles || {}});
    }
    if (message.incomplete) content.append(node("p", "incomplete-warning", "回覆中斷，後續內容未收到；這裡保留已收到的部分。"));
    const tools = node("div", "message-tools");
    const copy = node("button", "copy-message");
    copy.dataset.index = index;
    copy.innerHTML = icon("copy") + "複製";
    copy.title = "複製原文";
    tools.append(copy);
    if (message.project_paused && index === state.messages.length - 1 && state.conversations.some(c => c.id === state.active_id && c.project_id)) {
      const resume = node("button", "resume-project", "繼續");
      resume.disabled = !state.retry?.enabled || state.projects?.running;
      resume.title = "從暫存的工作副本與進度繼續，不重新執行整個任務";
      resume.onclick = () => { if (resume.disabled) return; resume.disabled = true; send({type:"project", command:{action:"resume",conversation:state.active_id,run_id:message.request_id,message_count:state.messages.length}}); };
      tools.append(resume);
    } else if (state.retry && index === state.messages.length - 1) {
      const retry = node("button", "retry-message", "重新再試一次");
      retry.innerHTML = icon("refresh") + "重新再試一次";
      retry.disabled = !state.retry.enabled;
      retry.title = retry.disabled ? "請先等待目前工作結束或確認原請求狀態" : "使用原提問重新嘗試，保留這次回覆";
      retry.dataset.conversation = state.active_id;
      retry.dataset.index = state.retry.user_index;
      retry.dataset.count = state.retry.message_count;
      retry.dataset.request = state.retry.request_id || "";
      tools.append(retry);
    }
    content.append(tools);
    article.append(avatar, content);
    container.append(article);
    const activityList = article.querySelector(".project-activity-history");
    if (activityList && activityScroll.has(String(index))) activityList.scrollTop = activityScroll.get(String(index));
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
  const conversations = state.conversations.filter(c => !c.project_id).sort(
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
      node("h3", "", notificationPreview(event.title)),
      node("p", "", notificationPreview(event.summary)),
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
  window.AuthUI?.beforeRender();
  state = next;
  document.documentElement.dataset.theme = state.config.dark_mode ? "dark" : "light";
  $("dark-mode").checked = !!state.config.dark_mode;
  document.documentElement.style.setProperty(
    "--font-size",
    state.config.font_size + "px",
  );
  for (const [section, label, list] of [["projects", "專案", "project-list"], ["recent", "最近對話", "history-list"]]) {
    const collapsed = !!state.config[section + "_collapsed"];
    $(list).hidden = collapsed;
    $("collapse-" + section).textContent = (collapsed ? "▸ " : "▾ ") + label;
    $("collapse-" + section).setAttribute("aria-expanded", String(!collapsed));
  }
  document.body.classList.toggle("collapsed", state.config.sidebar_collapsed);
  $("collapse").title = state.config.sidebar_collapsed
    ? "展開側欄"
    : "收合側欄";
  $("collapse").setAttribute("aria-label", $("collapse").title);
  const statusNotice = JSON.stringify([state.status_notice_id, state.status || "", !!state.error]);
  if (statusNotice !== lastStatusNotice) {
    lastStatusNotice = statusNotice;
    toast(state.status);
  }
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
  $("logout").disabled = state.busy !== "none" || !state.logged_in;
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
  window.ProjectUI?.render();
  window.WeeklyUI?.render();
  window.ProjectQuickUI?.render();
  renderModels();
  renderMessages();
  renderNotifications();
  renderMail();
  showView(activeView);
  window.WorkUI?.render();
  window.ProjectComposer?.render();
  window.MailUI?.render();
  window.BehaviorUI?.render();
  window.VncUI?.render();
  window.AuthUI?.apply();
  if (next.focus_draft && state.logged_in) {
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
  if (window.ProjectComposer?.submit($("prompt").value, action)) return;
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
$("update-shortcut").onclick = () => send({type: "download"});
for (const id of ["logout", "refresh", "download"])
  $(id).onclick = () => send({ type: id });
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
  const retry = event.target.closest(".retry-message");
  if (retry && !retry.disabled) {
    retry.disabled = true;
    send({type:"retry_chat", conversation_id:retry.dataset.conversation,
      user_index:Number(retry.dataset.index), message_count:Number(retry.dataset.count),
      request_id:retry.dataset.request || null});
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
  const artifact = event.target.closest(".project-artifact");
  if (artifact) {
    event.preventDefault();
    send({type:"project", command:{action:"reveal", conversation:artifact.dataset.conversation, path:artifact.dataset.path}});
    return;
  }
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
    else if (event.data.type === "self_test") window.runSelfTest?.(event.data.reply_fixture);
    else {
      receiveHotkeyMessage(event.data);
        window.ProjectUI?.receive(event.data);
        window.ChartUI?.receive(event.data);
      window.WeeklyUI?.receive(event.data);
      window.ProjectQuickUI?.receive(event.data);
      window.ProjectComposer?.receive(event.data);
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

// 每個區塊各自保存；不受整個側欄的收合狀態影響。
for (const section of ["projects", "recent"]) {
  $("collapse-" + section).onclick = () => send({type:"collapse_section", section, collapsed:!state.config[section + "_collapsed"]});
}
