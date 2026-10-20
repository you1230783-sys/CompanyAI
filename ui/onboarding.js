/* 教學內容由原生目錄提供；每張卡片各自保存完成旗標，重看不需重設。 */
"use strict";
window.OnboardingUI = (() => {
  const dialog = document.createElement("dialog");
  dialog.id = "onboarding-dialog";
  dialog.className = "onboarding-card";
  dialog.setAttribute("aria-labelledby", "onboarding-title");
  dialog.innerHTML = `<div class="onboarding-eyebrow" id="onboarding-eyebrow"></div>
    <h2 id="onboarding-title"></h2><p id="onboarding-intro" class="subtle"></p>
    <div id="onboarding-body"></div><div class="onboarding-actions">
      <button id="onboarding-previous" class="secondary-button">上一張</button>
      <button id="onboarding-later" class="secondary-button">稍後再看</button>
      <button id="onboarding-done">看完了</button></div>`;
  document.body.append(dialog);
  let autoShown = false, timer = null, previousFocus = null, returnToSettings = false;
  let mode = "lesson", sequence = [], position = 0, catalogSignature = "";
  const allowed = () => state.logged_in && !state.update_required && state.onboarding?.enabled;
  const lessons = () => state.onboarding?.tutorials || [];

  function beginPractice(topic, fromSettings = false, group = null) {
    if (!allowed() || window.ProjectTutorial == null) return;
    if (fromSettings) $("settings-dialog").close();
    autoShown = true;
    // 只保存選單已提供與教學完成旗標；練習內容不進正式專案、對話或草稿。
    send({type:"onboarding_welcome_dismiss"});
    window.ProjectTutorial.open({topic, group,
      completed: lessons().filter(item => item.completed).map(item => item.id),
      onComplete: id => send({type:"onboarding_ack", topic:id}),
      onClose: () => {
        if (fromSettings && allowed()) {
          $("settings-dialog").showModal(); window.SettingsUI.select(3);
        }
      },
    });
  }
  $("project-tutorial-menu").onclick = () => beginPractice(null, true);

  function appendText(parent, tag, text, className = "") {
    const node = document.createElement(tag);
    node.textContent = text;
    if (className) node.className = className;
    parent.append(node);
    return node;
  }

  // 使用 textContent 呈現目錄文字，避免更新公告或教學內容被當作 HTML。
  function renderReleases(parent, entries) {
    parent.replaceChildren();
    if (!entries.length) {
      appendText(parent, "p", "目前沒有可顯示的更新紀錄。", "subtle");
      return;
    }
    const versions = new Map();
    for (const entry of entries) {
      if (!versions.has(entry.version)) versions.set(entry.version, []);
      versions.get(entry.version).push(entry);
    }
    // 版本按數字比較，避免把 0.8.100 排在 0.8.99 前面。
    const compare = (a, b) => {
      const aa = a.split(".").map(Number), bb = b.split(".").map(Number);
      for (let i = 0; i < Math.max(aa.length, bb.length); i++) {
        const difference = (bb[i] || 0) - (aa[i] || 0);
        if (difference) return difference;
      }
      return 0;
    };
    for (const version of [...versions.keys()].sort(compare)) {
      const section = document.createElement("section");
      section.className = "release-entry";
      appendText(section, "h3", version);
      for (const entry of versions.get(version)) {
        appendText(section, "h4", entry.section);
        const list = document.createElement("ul");
        for (const text of entry.items || []) appendText(list, "li", text);
        section.append(list);
      }
      parent.append(section);
    }
  }

  function paintLesson() {
    const lesson = sequence[position];
    const body = $("onboarding-body");
    body.replaceChildren();
    $("onboarding-eyebrow").textContent = `${lesson.group} · ${position + 1} / ${sequence.length}`;
    $("onboarding-title").textContent = lesson.title;
    $("onboarding-intro").textContent = lesson.intro;
    lesson.steps.forEach(([title, description], index) => {
      const row = document.createElement("div"), content = document.createElement("div");
      row.className = "onboarding-step";
      appendText(row, "span", String(index + 1), "onboarding-number");
      appendText(content, "strong", title);
      appendText(content, "p", description);
      row.append(content); body.append(row);
    });
    appendText(body, "p", "之後可從「設定 → 教學與更新日誌」選擇任何主題重看。", "subtle onboarding-hint");
    $("onboarding-previous").hidden = position === 0;
    $("onboarding-later").textContent = "稍後再看";
    $("onboarding-done").textContent = position + 1 < sequence.length ? "看完了，下一張" : "看完了";
  }

  function show(fromSettings) {
    previousFocus = document.activeElement;
    returnToSettings = fromSettings;
    if (fromSettings) $("settings-dialog").close();
    autoShown = true;
    dialog.showModal();
    $("onboarding-done").focus();
  }

  function beginLessons(items, fromSettings = false) {
    if (!allowed() || !items.length) return;
    if (items[0].interactive) { beginPractice(items.length === 1 ? items[0].id : null, fromSettings, items[0].group); return; }
    mode = "lesson"; sequence = items; position = 0;
    paintLesson(); show(fromSettings);
  }

  function openUpdates() {
    if (!allowed()) return;
    mode = "release";
    const info = state.onboarding;
    $("onboarding-eyebrow").textContent = "版本更新";
    $("onboarding-title").textContent = "更新日誌";
    $("onboarding-intro").textContent = "目前版本 " + info.version;
    renderReleases($("onboarding-body"), info.release_notes || []);
    $("onboarding-previous").hidden = true;
    $("onboarding-later").textContent = "返回設定";
    $("onboarding-done").textContent = "關閉";
    show(true);
  }

  $("onboarding-previous").onclick = () => {
    if (mode === "lesson" && position > 0) { position--; paintLesson(); }
  };
  $("onboarding-later").onclick = () => dialog.close();
  $("onboarding-done").onclick = () => {
    if (!allowed()) return;
    if (mode === "lesson") {
      send({type:"onboarding_ack", topic:sequence[position].id});
      if (++position < sequence.length) { paintLesson(); return; }
    }
    // 更新日誌只供手動查看，不記錄已讀或自動彈出。
    dialog.close();
  };
  dialog.addEventListener("close", () => {
    if (returnToSettings && allowed()) {
      $("settings-dialog").showModal();
      window.SettingsUI.select(3);
    }
    returnToSettings = false;
    if (previousFocus?.isConnected && allowed()) previousFocus.focus();
  });
  $("updates-replay").onclick = () => openUpdates();

  function renderCatalog() {
    const info = state.onboarding || {};
    $("onboarding-storage-status").textContent = info.error || "";
    const signature = JSON.stringify(info.tutorials);
    if (signature === catalogSignature) return;
    catalogSignature = signature;
    const catalog = $("tutorial-catalog"); catalog.replaceChildren();
    const groups = new Map();
    for (const lesson of lessons()) {
      if (!groups.has(lesson.group)) groups.set(lesson.group, []);
      groups.get(lesson.group).push(lesson);
    }
    for (const [group, items] of groups) {
      const section = document.createElement("section"), heading = document.createElement("div");
      section.className = "tutorial-group"; heading.className = "tutorial-section-heading";
      appendText(heading, "h4", group);
      const all = appendText(heading, "button", items[0]?.interactive ? "選擇練習" : "依序閱讀", "secondary-button");
      all.type = "button"; all.onclick = () => beginLessons(items, true);
      section.append(heading);
      for (const lesson of items) {
        const button = document.createElement("button");
        button.type = "button"; button.className = "tutorial-topic";
        appendText(button, "span", lesson.title);
        appendText(button, "span", lesson.completed ? "重看" : lesson.interactive ? "互動練習" : "未閱讀", "subtle");
        button.onclick = () => beginLessons([lesson], true);
        section.append(button);
      }
      catalog.append(section);
    }
  }

  function render() {
    renderCatalog();
    if (!allowed()) { if (dialog.open) dialog.close(); window.ProjectTutorial?.close(); return; }
    // 首次只提供選單。跳過後不再自動連播舊卡片；所有主題仍可在設定手動重看。
    if (autoShown || timer || state.busy !== "none" || state.onboarding.error) return;
    timer = setTimeout(() => {
      timer = null;
      if (!allowed() || autoShown || state.busy !== "none" || document.querySelector("dialog[open]")) return;
      if (typeof activeView !== "undefined" && activeView !== "chat") return;
      if (state.onboarding.practice_welcome_pending) { beginPractice(null); return; }
      autoShown = true;
    }, 300);
  }
  return {render};
})();
