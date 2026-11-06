/* 設定頁只切換可見區塊，不重建輸入控制項，保留尚未套用的快捷鍵等草稿。 */
"use strict";
window.SettingsUI = (() => {
  let paletteDirty = false;
  const colorInputs = ChartAppearance.defaults().map((color, index) => {
    const label=document.createElement("label"), input=document.createElement("input"), name=document.createElement("span");
    name.textContent=`系列 ${index+1}`;input.type="color";input.value=color;
    input.oninput=()=>{paletteDirty=true;};label.append(name,input);$("chart-palette").append(label);return input;
  });
  $("chart-palette-save").onclick=()=>{paletteDirty=false;send({type:"chart_palette",colors:colorInputs.map(input=>input.value)});};
  $("chart-palette-reset").onclick=()=>{colorInputs.forEach((input,i)=>input.value=ChartAppearance.defaults()[i]);paletteDirty=true;};
  const notificationMode = $("notification-mode");
  const petOption = notificationMode.querySelector('[value="pet"]');
  const legacyOption = notificationMode.querySelector('[value="legacy"]');
  const categories = [...document.querySelectorAll("[data-notice-category]")];
  const chatPreview = $("private-chat-notice-preview");
  chatPreview.onchange = () => send({type:"chat_notification_preview", enabled:chatPreview.value === "detail"});
  notificationMode.onchange = () => send({type:"notification_mode", mode:notificationMode.value});
  categories.forEach(input => {
    input.onchange = () => send({type:"notification_category", category:input.dataset.noticeCategory, enabled:input.checked});
  });
  function render(config, features) {
    ChartAppearance.set(config.chart_palette);
    if(!paletteDirty) colorInputs.forEach((input,i)=>input.value=ChartAppearance.palette()[i]);
    const preferences = config.notification_preferences || {mode:"windows", categories:{}};
    chatPreview.value = preferences.chat_preview ? "detail" : "generic";
    const petAvailable = Boolean(features?.pet && config.extra_features_enabled);
    petOption.hidden = petOption.disabled = !petAvailable;
    legacyOption.hidden = preferences.mode !== "legacy";
    const petFallback = preferences.mode === "pet" && !petAvailable;
    notificationMode.value = petFallback ? "windows" : preferences.mode;
    $("notification-mode-legacy").hidden = legacyOption.hidden;
    $("notification-mode-pet-fallback").hidden = !petFallback;
    categories.forEach(input => {
      const category = input.dataset.noticeCategory;
      const legacyEnabled = preferences.legacy_popups !== false || ["reminder", "tray_hint"].includes(category);
      input.checked = preferences.categories?.[category] ?? legacyEnabled;
    });
  }
  const buttons = [...document.querySelectorAll(".settings-tabs [role=tab]")];
  function select(id, focus = false) {
    buttons.forEach(button => {
      const active = button.id === id;
      button.setAttribute("aria-selected", String(active));
      button.tabIndex = active ? 0 : -1;
      $(button.getAttribute("aria-controls")).hidden = !active;
      if (active && focus) button.focus();
    });
    document.querySelector(".settings-content").scrollTop = 0;
  }
  buttons.forEach((button, index) => {
    button.onclick = () => select(button.id);
    button.onkeydown = event => {
      const count = buttons.length;
      const next = {ArrowRight:(index+1)%count, ArrowLeft:(index+count-1)%count, Home:0, End:count-1}[event.key];
      if (next !== undefined) { event.preventDefault(); select(buttons[next].id, true); }
    };
  });
  $("diagnostics-export").onclick = () => {
    $("diagnostics-export-status").textContent = "正在準備預覽，尚未匯出檔案。";
    send({type:"export_diagnostics", include_details:$("diagnostics-include-details").checked});
  };
  // 使用原生產生的快照，預覽中的文字不可編輯，儲存只回傳快照識別碼。
  const preview = document.createElement("dialog");
  preview.id = "diagnostics-preview";
  preview.dataset.offline = "true";
  preview.setAttribute("aria-labelledby", "diagnostics-preview-title");
  preview.innerHTML = `<h2 id="diagnostics-preview-title">預覽錯誤診斷</h2>
    <p>先檢查以下完整內容，再決定是否匯出。預設不包含對話、文件、設定或機台清單；詳細錯誤可能包含檔名、專案路徑、Windows帳號或服務回覆。</p>
    <textarea id="diagnostics-preview-text" readonly rows="14" aria-label="將匯出的完整診斷JSON"></textarea>
    <details><summary>錯誤代碼與處理方式（離線可用）</summary><div id="diagnostics-help"></div></details>
    <p id="diagnostics-preview-status" role="status"></p>
    <div class="button-row"><button id="diagnostics-save">確認並選擇儲存位置</button><button id="diagnostics-cancel" class="secondary-button">取消</button></div>`;
  document.body.append(preview);
  let previewId = null, previewReturn = false;
  $("diagnostics-open").onclick = () => send({type:"export_diagnostics", include_details:false});
  $("copy-error").onclick = () => send({type:"copy_error"});
  $("diagnostics-save").onclick = () => {
    if (!previewId) return;
    $("diagnostics-save").disabled = true;
    send({type:"save_diagnostics", preview_id:previewId});
  };
  $("diagnostics-cancel").onclick = () => preview.close();
  preview.addEventListener("close", () => {
    previewId = null;
    $("diagnostics-preview-text").value = "";
    send({type:"cancel_diagnostics"});
    if (previewReturn && state.logged_in) $("settings-dialog").showModal();
    previewReturn = false;
  });
  function receive(message) {
    if (message.type === "diagnostics_preview") {
      previewId = message.preview_id;
      $("diagnostics-preview-text").value = message.text;
      $("diagnostics-preview-status").textContent = "尚未儲存；匯出內容與上方預覽完全相同。";
      $("diagnostics-save").disabled = false;
      $("diagnostics-help").replaceChildren();
      for (const item of message.help || []) {
        const p = document.createElement("p");
        p.textContent = `${item.code}：${item.message} ${item.suggestion}`;
        $("diagnostics-help").append(p);
      }
      if (!preview.open) {
        previewReturn = $("settings-dialog").open;
        if (previewReturn) $("settings-dialog").close();
        preview.showModal();
      }
    }
    if (message.type === "diagnostics_exported") {
      $("diagnostics-export-status").textContent = message.text;
      $("diagnostics-preview-status").textContent = message.text;
      $("diagnostics-save").disabled = !previewId;
    }
  }
  return {select, receive, render};
})();
