/* 個人偏好與本機對話操作。模型提示只描述該次請求，不改掉使用者選定的模型。 */
"use strict";
(() => {
  const controls = ["hotkey-enabled", "selection-icon", "enter-newline", "enter-send", "quick-fast", "quick-current", "always-new-chat", "vnc-enabled"];
  for (const id of controls) $(id).onchange = () => send({
    type: "behavior",
    hotkey_enabled: $("hotkey-enabled").checked,
    selection_icon: $("selection-icon").checked,
    enter_sends: $("enter-send").checked,
    quick_actions_fast: $("quick-fast").checked,
    always_new_chat: $("always-new-chat").checked,
    vnc_enabled: $("vnc-enabled").checked,
  });

  let renameId = null;
  function renameChat(id) {
    const current = state.conversations.find(c => c.id === id);
    if (!current) return;
    renameId = current.id;
    $("rename-title").value = current.title;
    $("rename-dialog").showModal();
    $("rename-title").select();
  }
  $("rename-cancel").onclick = () => $("rename-dialog").close();
  $("rename-dialog").addEventListener("close", () => {
    if (!$("rename-dialog").open) { renameId = null; $("rename-title").value = ""; }
  });
  $("rename-save").onclick = () => {
    const title = $("rename-title").value.trim();
    if (!title) return;
    send({ type: "rename_chat", id: renameId, title });
    $("rename-dialog").close();
  };
  const notice = node("div", "model-notice");
  notice.setAttribute("role", "status");
  notice.hidden = true;
  $("model-button").parentElement.append(notice);
  let noticeTimer;
  // 關閉提示不解除原生版本限制；使用者可回設定下載更新，不因狀態推播反覆彈出。
  const updateGate = $("required-update-dialog");
  let updateDismissed = false;
  updateGate.addEventListener("backdrop-dismiss", () => { updateDismissed = true; });
  updateGate.addEventListener("cancel", event => event.preventDefault());
  $("required-update-exit").onclick = () => send({type:"exit"});
  $("required-update-refresh").onclick = () => send({type:"refresh"});
  $("required-update-download").onclick = () => send({type:"download"});
  window.BehaviorUI = {
    renameChat,
    render() {
      $("update-status").textContent = state.update_status || "";
      $("download").textContent = state.update_ready
        ? (state.update_kind === "exe" ? "開啟下載資料夾" : "安裝並重新啟動 LM_AI")
        : "下載更新";
      $("download").disabled = !!state.update_busy;
      $("required-update-status").textContent = state.update_status || state.version_status || "請下載並安裝新版。";
      $("required-update-download").textContent = $("download").textContent;
      $("required-update-download").disabled = !!state.update_busy;
      if (!state.logged_in || !state.update_required) updateDismissed = false;
      if (state.logged_in && state.update_required && !updateDismissed && !updateGate.open) updateGate.showModal();
      if ((!state.logged_in || !state.update_required) && updateGate.open) updateGate.close();
      const c = state.config;
      $("hotkey-enabled").checked = c.hotkey_enabled !== false;
      $("selection-icon").checked = !!c.selection_icon;
      $("enter-newline").checked = !c.enter_sends;
      $("enter-send").checked = !!c.enter_sends;
      $("quick-fast").checked = c.quick_actions_fast !== false;
      $("quick-current").checked = c.quick_actions_fast === false;
      $("always-new-chat").checked = !!c.always_new_chat;
      $("vnc-enabled").checked = !!c.vnc_enabled;
      // 與輸入框 keydown 使用相同的已套用設定；錄製中尚未套用的快捷鍵不顯示於此。
      const sendKey = c.enter_sends ? "Enter" : "Ctrl+Enter";
      const newlineKey = c.enter_sends ? "Shift+Enter" : "Enter";
      const enterHint = `${newlineKey} 換行，${sendKey} 送出`;
      $("enter-hint").textContent = enterHint;
      $("compose-enter-hint").textContent = enterHint;
      $("send").title = `送出（${sendKey}）`;
      $("hotkey-hint").textContent = `${c.hotkey} 選字帶入`;
      $("quick-model-hint").textContent = c.quick_actions_fast !== false
        ? "點擊翻譯、摘要或潤飾時，以快速模型送出該次請求，通常能縮短等待時間。"
        : "快捷操作使用目前選擇的模型。品質模型可能需要數分鐘，實際依內容及排隊情況而定。";
      $("hotkey-hint").hidden = c.hotkey_enabled === false;
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
