/* 一張入門卡與跨版本摘要；紀錄由 Rust 保存，登入後才可顯示或標記完成。 */
"use strict";
window.OnboardingUI = (() => {
  const dialog = document.createElement("dialog");
  dialog.id = "onboarding-dialog";
  dialog.className = "onboarding-card";
  dialog.setAttribute("aria-labelledby", "onboarding-title");
  dialog.innerHTML = `<div class="onboarding-eyebrow" id="onboarding-eyebrow"></div>
    <h2 id="onboarding-title"></h2><p id="onboarding-intro" class="subtle"></p>
    <div id="onboarding-body"></div><div class="onboarding-actions">
      <button id="onboarding-later" class="secondary-button">稍後再看</button>
      <button id="onboarding-done">開始使用</button></div>`;
  document.body.append(dialog);
  let autoShown = false, topic = "chat", previousFocus = null, timer = null;
  const allowed = () => state.logged_in && !state.update_required && state.onboarding?.enabled;
  function open(which) {
    if (!allowed()) return;
    topic = which;
    previousFocus = document.activeElement;
    const body = $("onboarding-body");
    body.replaceChildren();
    if (which === "chat") {
      $("onboarding-eyebrow").textContent = "一般對話 · 入門 1 / 1";
      $("onboarding-title").textContent = "從一句話開始";
      $("onboarding-intro").textContent = "說明你想完成的事，LM_AI 會協助整理與回答。";
      const steps = [
        ["1", "描述需求", "例如：幫我把這段文字整理成三個重點。"],
        ["2", "需要時附上檔案", "加入文件，並說明希望它幫你找什麼或整理什麼。"],
        ["3", "看完再追問", "可以請它補充、縮短或換個說法；重要內容請再核對。"],
      ];
      for (const [number, title, description] of steps) {
        const row = document.createElement("div"); row.className = "onboarding-step";
        const badge = document.createElement("span"); badge.className = "onboarding-number"; badge.textContent = number;
        const content = document.createElement("div"), heading = document.createElement("strong"), text = document.createElement("p");
        heading.textContent = title; text.textContent = description;
        content.append(heading, text); row.append(badge, content); body.append(row);
      }
      const hint = document.createElement("p"); hint.className = "subtle onboarding-hint";
      hint.textContent = "之後可從「設定 → 一般 → 使用教學」重看。"; body.append(hint);
      $("onboarding-done").textContent = "開始使用";
    } else {
      const info = state.onboarding;
      $("onboarding-eyebrow").textContent = "版本更新";
      $("onboarding-title").textContent = "這次更新了什麼";
      $("onboarding-intro").textContent = `您從 ${info.from_version} 更新到 ${info.to_version}，以下為更新資訊：`;
      const groups = new Map();
      for (const entry of info.updates || []) {
        if (!groups.has(entry.section)) groups.set(entry.section, []);
        for (const text of entry.items || []) groups.get(entry.section).push(`${text}（${entry.version}）`);
      }
      for (const [section, items] of groups) {
        const heading = document.createElement("h3"), list = document.createElement("ul");
        heading.textContent = section + "新增或修改";
        for (const text of items) { const item = document.createElement("li"); item.textContent = text; list.append(item); }
        body.append(heading, list);
      }
      $("onboarding-done").textContent = "知道了";
    }
    autoShown = true;
    dialog.showModal();
    $("onboarding-done").focus();
  }
  $("onboarding-later").onclick = () => dialog.close();
  $("onboarding-done").onclick = () => {
    if (!allowed()) return;
    send({type:"onboarding_ack", topic});
    dialog.close();
  };
  dialog.addEventListener("close", () => {
    if (previousFocus?.isConnected && state.logged_in) previousFocus.focus();
  });
  $("onboarding-replay").onclick = () => {
    if (!allowed()) return;
    $("settings-dialog").close();
    open("chat");
  };
  function render() {
    $("onboarding-storage-status").textContent = state.onboarding?.error || "";
    if (!allowed()) { if (dialog.open) dialog.close(); return; }
    // 不蓋過登入、強制更新、待確認操作或其他 modal；每次啟動最多自動顯示一張。
    if (autoShown || timer || state.busy !== "none" || state.onboarding.error) return;
    timer = setTimeout(() => {
      timer = null;
      if (!allowed() || autoShown || state.busy !== "none" || document.querySelector("dialog[open]")) return;
      if (state.onboarding.updates?.length) open("updates");
      else if (state.onboarding.chat_pending) open("chat");
    }, 300);
  }
  return {render};
})();
