/* 範本與截圖只協助準備草稿。使用者資料一律透過 textContent/value 顯示。 */
"use strict";
window.DraftTools = (() => {
  const transmit = (type, command) => send({type, command});
  const state = () => window.LMUI?.getState() || {};
  const syncDraft = () => send({type:"draft", text:document.getElementById("prompt").value});
  const modal = document.createElement("dialog");
  modal.id = "templates-dialog";
  modal.className = "draft-tool-dialog";
  modal.innerHTML = `<div class="tarot-heading"><h2>指令範本</h2><button data-close type="button">關閉</button></div>
    <p>把常用提問保存成可重複填寫的草稿。例如「請將以下內容翻譯為 {{語言}}，保留 {{專有名詞}}：{{原文}}」。使用時填入英文、產品名稱與原文，預覽後加入輸入框，再自行按送出。</p>
    <p>範本只加入草稿，不自動送出、不授予工具權限。匯入內容請先檢查；分享前請移除私人資料。</p>
    <div class="draft-template-layout"><section><label>分類篩選<select data-filter><option value="">全部分類</option></select></label>
    <label><input type="checkbox" data-favorites> 只看收藏</label><div data-list class="draft-template-list"></div>
    <button data-new type="button">新增範本</button><label>匯入 JSON<input data-import type="file" accept=".json,application/json"></label></section>
    <section><form data-form><label>標題<input data-title maxlength="100" required></label><label>分類<input data-category maxlength="60"></label>
    <label><input data-favorite type="checkbox"> 收藏</label><label>內容<textarea data-body rows="8" maxlength="16000" required placeholder="請替 {{部門}} 摘要以下內容…"></textarea></label>
    <p class="subtle">參數寫成 {{名稱}}，限文字、數字或底線。請先保存再填入參數。</p>
    <div class="dialog-actions"><button type="submit">保存</button><button data-delete type="button">刪除</button><button data-export type="button">預覽匯出</button></div></form>
    <div data-parameters></div><label>填入草稿的預覽<textarea data-preview rows="6" readonly></textarea></label>
    <button data-use type="button" class="primary-button">加入目前對話草稿</button></section></div>
    <p data-status role="status" aria-live="polite"></p>`;
  document.body.append(modal);
  const find = selector => modal.querySelector(selector);
  let entries = [], selected = null, conversation = null, ready = false, pending = false;
  function controls() {
    for (const node of modal.querySelectorAll("input,textarea,select,button")) node.disabled = pending || !ready;
    find("[data-close]").disabled = false;
    for (const selector of ["[data-delete]","[data-export]","[data-use]"]) find(selector).disabled = pending || !ready || !selected;
  }
  function request(command) {
    pending = true; controls(); find("[data-status]").textContent = "處理中…";
    transmit("template", command);
  }
  function preview() {
    const entry = entries.find(e => e.id === selected);
    if (!entry) { find("[data-preview]").value = "請先保存並選擇範本。"; return; }
    const values = new Map([...find("[data-parameters]").querySelectorAll("input")].map(input => [input.dataset.parameter, input.value]));
    // 單次替換，參數內容中的 {{...}} 不重新解析。
    find("[data-preview]").value = entry.text.body.replace(/\{\{([^{}]+)\}\}/g, (_, name) => values.get(name) ?? "");
  }
  function select(id) {
    selected = id;
    const entry = entries.find(e => e.id === id);
    const text = entry?.text || {title:"",category:"",body:"",favorite:false};
    find("[data-title]").value = text.title;
    find("[data-category]").value = text.category;
    find("[data-body]").value = text.body;
    find("[data-favorite]").checked = text.favorite;
    find("[data-parameters]").replaceChildren();
    for (const name of entry?.parameters || []) {
      const label = document.createElement("label"), input = document.createElement("input");
      label.textContent = name; input.dataset.parameter = name; input.maxLength = 16000;
      input.addEventListener("input", preview); label.append(input); find("[data-parameters]").append(label);
    }
    preview(); controls();
  }
  function list() {
    const filter = find("[data-filter]"), previous = filter.value;
    filter.replaceChildren(new Option("全部分類", ""));
    for (const category of [...new Set(entries.map(e => e.text.category).filter(Boolean))].sort()) filter.add(new Option(category, category));
    filter.value = previous;
    const container = find("[data-list]"); container.replaceChildren();
    for (const entry of entries.filter(e => (!filter.value || e.text.category === filter.value) && (!find("[data-favorites]").checked || e.text.favorite)).sort((a,b) => Number(b.text.favorite)-Number(a.text.favorite) || a.text.title.localeCompare(b.text.title))) {
      const button = document.createElement("button"); button.type = "button";
      button.textContent = (entry.text.favorite ? "★ " : "") + entry.text.title;
      button.addEventListener("click", () => select(entry.id)); container.append(button);
    }
    if (!container.children.length) container.textContent = "沒有符合條件的範本。";
  }
  document.getElementById("show-templates").addEventListener("click", () => {
    syncDraft(); conversation = state().active_id || null;
    ready = false; modal.showModal(); request({action:"state"});
  });
  find("[data-close]").addEventListener("click", () => modal.close());
  find("[data-new]").addEventListener("click", () => select(null));
  find("[data-filter]").addEventListener("change", list);
  find("[data-favorites]").addEventListener("change", list);
  find("[data-form]").addEventListener("submit", event => {
    event.preventDefault(); request({action:"save", id:selected, text:{title:find("[data-title]").value, category:find("[data-category]").value, body:find("[data-body]").value, favorite:find("[data-favorite]").checked}});
  });
  find("[data-delete]").addEventListener("click", () => { if (selected && confirm("刪除此範本？")) request({action:"delete",id:selected}); });
  find("[data-use]").addEventListener("click", () => {
    syncDraft(); const values = Object.fromEntries([...find("[data-parameters]").querySelectorAll("input")].map(input => [input.dataset.parameter,input.value]));
    request({action:"use",id:selected,values,conversation});
  });
  find("[data-import]").addEventListener("change", async event => {
    const file = event.target.files[0]; event.target.value = "";
    if (!file) return;
    if (file.size > 1048576) { find("[data-status]").textContent = "匯入檔最多1 MiB。"; return; }
    try { request({action:"import", json:await file.text()}); }
    catch { pending = false; controls(); find("[data-status]").textContent = "無法讀取匯入檔。"; }
  });
  const exportDialog = document.createElement("dialog"); exportDialog.className = "draft-tool-dialog";
  exportDialog.innerHTML = `<h2>預覽要分享的範本</h2><p>以下內容將以明文 JSON 匯出，請確認沒有私人資料。</p><textarea rows="16" readonly aria-label="匯出內容"></textarea><div class="dialog-actions"><button data-save type="button">選擇位置並匯出</button><button data-cancel type="button">取消</button></div>`;
  document.body.append(exportDialog); let exportId = null;
  find("[data-export]").addEventListener("click", () => request({action:"preview_export",id:selected}));
  exportDialog.querySelector("[data-cancel]").addEventListener("click", () => exportDialog.close());
  exportDialog.querySelector("[data-save]").addEventListener("click", () => { exportDialog.close(); request({action:"export",preview_id:exportId}); });

  const capture = document.createElement("dialog"); capture.id = "screenshot-dialog"; capture.className = "draft-tool-dialog";
  capture.innerHTML = `<h2>截圖提問</h2><p data-destination></p><p data-hint>Windows 11 剪取工具會覆蓋剪貼簿。框選後請確認預覽是你要分享的畫面，避免包含密碼、信件或其他私人資料。</p><img data-image hidden alt="本次截圖預覽"><p data-status role="status"></p><div class="dialog-actions"><button data-begin type="button">開始框選</button><button data-accept type="button" hidden>確認加入</button><button data-cancel type="button">取消</button></div>`;
  document.body.append(capture); const shot = selector => capture.querySelector(selector); let captureId = null;
  document.getElementById("screenshot-start").addEventListener("click", () => { syncDraft(); transmit("screenshot",{action:"prepare"}); });
  shot("[data-begin]").addEventListener("click", () => { shot("[data-begin]").disabled = true; transmit("screenshot",{action:"begin",id:captureId}); });
  shot("[data-accept]").addEventListener("click", () => { syncDraft(); shot("[data-accept]").disabled = true; transmit("screenshot",{action:"accept",id:captureId}); });
  function cancelCapture() { transmit("screenshot",{action:"cancel"}); shot("[data-image]").removeAttribute("src"); captureId = null; }
  shot("[data-cancel]").addEventListener("click", () => capture.close());
  capture.addEventListener("close", cancelCapture);
  return {sync() {
    if (!state().logged_in) {
      if (modal.open) modal.close();
      if (exportDialog.open) exportDialog.close();
      if (capture.open) capture.close();
      shot("[data-image]").removeAttribute("src");
    }
  }, receive(message) {
    switch (message.type) {
      case "template_state":
        entries = message.entries; pending = false; ready = true; list();
        select(entries.some(e => e.id === selected) ? selected : entries[0]?.id || null);
        find("[data-status]").textContent = `${entries.length} / 100 個範本`; break;
      case "template_error": pending = false; controls(); find("[data-status]").textContent = message.message; break;
      case "template_applied": modal.close(); break;
      case "template_export":
        exportId = message.preview_id; exportDialog.querySelector("textarea").value = message.text; exportDialog.showModal(); break;
      case "screenshot_prepare":
        captureId = message.id; shot("[data-destination]").textContent = message.destination;
        shot("[data-status]").textContent = message.hotkey ? "快捷鍵：Ctrl+Alt+S" : "Ctrl+Alt+S 已被占用或保留給選字，仍可使用「截圖提問」按鈕。";
        shot("[data-image]").hidden = true; shot("[data-image]").removeAttribute("src");
        shot("[data-begin]").hidden = false; shot("[data-begin]").disabled = false;
        shot("[data-accept]").hidden = true; shot("[data-accept]").disabled = false;
        shot("[data-accept]").textContent = message.destination.startsWith("一般") ? "確認加入並上傳網站附件" : "確認保存至專案並加入草稿";
        if (!capture.open) capture.showModal(); break;
      case "screenshot_waiting": shot("[data-status]").textContent = "請框選畫面，最多等待45秒；按 Esc 取消框選後，可回到此處取消。"; break;
      case "screenshot_preview":
        if (message.id !== captureId) break;
        shot("[data-image]").src = message.url; shot("[data-image]").hidden = false;
        shot("[data-begin]").hidden = true; shot("[data-accept]").hidden = false;
        shot("[data-status]").textContent = "請核對這張圖片；尚未保存或上傳。"; break;
      case "screenshot_error":
        shot("[data-image]").removeAttribute("src"); shot("[data-image]").hidden = true;
        shot("[data-status]").textContent = message.message; shot("[data-begin]").hidden = true; shot("[data-accept]").hidden = true;
        if (!capture.open) capture.showModal(); break;
      case "screenshot_done": capture.close(); break;
    }
  }};
})();
