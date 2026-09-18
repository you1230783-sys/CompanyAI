/* 個人偏好與本機對話操作。模型提示只描述該次請求，不改掉使用者選定的模型。 */
"use strict";
(() => {
  const controls = ["hotkey-enabled", "selection-icon", "enter-newline", "enter-send", "quick-fast", "quick-current", "always-new-chat"];
  for (const id of controls) $(id).onchange = () => send({
    type: "behavior",
    hotkey_enabled: $("hotkey-enabled").checked,
    selection_icon: $("selection-icon").checked,
    enter_sends: $("enter-send").checked,
    quick_actions_fast: $("quick-fast").checked,
    always_new_chat: $("always-new-chat").checked,
  });

  const rename = node("button", "text-button", "編輯標題");
  const pin = node("button", "text-button", "置頂");
  $("delete-chat").before(rename, pin);
  let renameId = null;
  rename.onclick = () => {
    const current = state.conversations.find(c => c.id === state.active_id);
    if (!current) return;
    renameId = current.id;
    $("rename-title").value = current.title;
    $("rename-dialog").showModal();
    $("rename-title").select();
  };
  $("rename-cancel").onclick = () => $("rename-dialog").close();
  $("rename-save").onclick = () => {
    const title = $("rename-title").value.trim();
    if (!title) return;
    send({ type: "rename_chat", id: renameId, title });
    $("rename-dialog").close();
  };
  pin.onclick = () => {
    const current = state.conversations.find(c => c.id === state.active_id);
    if (current) send({ type: "pin_chat", id: current.id, pinned: !current.pinned });
  };
  const notice = node("div", "model-notice");
  notice.setAttribute("role", "status");
  notice.hidden = true;
  $("model-button").parentElement.append(notice);
  let noticeTimer;
  window.BehaviorUI = {
    render() {
      $("update-status").textContent = state.update_status || "";
      $("download").textContent = state.update_ready ? "立即更新並重新啟動" : "下載更新";
      $("download").disabled = !!state.update_busy;
      const c = state.config;
      $("hotkey-enabled").checked = c.hotkey_enabled !== false;
      $("selection-icon").checked = !!c.selection_icon;
      $("enter-newline").checked = !c.enter_sends;
      $("enter-send").checked = !!c.enter_sends;
      $("quick-fast").checked = c.quick_actions_fast !== false;
      $("quick-current").checked = c.quick_actions_fast === false;
      $("always-new-chat").checked = !!c.always_new_chat;
      $("enter-hint").textContent = c.enter_sends ? "Shift + Enter 換行" : "Ctrl + Enter 送出";
      $("quick-model-hint").textContent = c.quick_actions_fast !== false
        ? "點擊翻譯、摘要或潤飾時，以快速模型送出該次請求，通常能縮短等待時間。"
        : "快捷操作使用目前選擇的模型。品質模型可能需要數分鐘，實際依內容及排隊情況而定。";
      $("hotkey-hint").hidden = c.hotkey_enabled === false;
      const current = state.conversations.find(item => item.id === state.active_id);
      rename.disabled = pin.disabled = !current;
      pin.textContent = current?.pinned ? "取消置頂" : "置頂";
    },
    modelNotice(text) {
      clearTimeout(noticeTimer);
      notice.textContent = text;
      notice.hidden = false;
      notice.classList.remove("fade");
      noticeTimer = setTimeout(() => {
        notice.classList.add("fade");
        noticeTimer = setTimeout(() => { notice.hidden = true; }, 250);
      }, 1500);
    },
  };
})();
