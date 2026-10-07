/* 專案快速入口只準備要求；原生層核對對話／帳號，Outlook 仍另行勾選授權。 */
"use strict";
(() => {
  const dialog = $("project-quick-dialog");
  const command = value => send({type:"project", command:value});
  let target = null, pending = false, ready = false;
  function identity() {
    return {conversation:target.conversation, request_id:target.request_id};
  }
  function current() {
    return target && target.conversation === state.active_id && state.logged_in && !state.update_required;
  }
  function controls() {
    $("project-quick-send").disabled = pending || !ready;
    $("project-quick-choose").disabled = pending;
  }
  function clear() {
    const old = target;
    target = null; pending = false; ready = false;
    if (dialog.open) dialog.close();
    if (old) command({action:"quick_cancel", conversation:old.conversation, request_id:old.request_id});
  }
  function begin(kind) {
    if (pending) return;
    if (kind === "image" && state.config.model === "fast") {
      toast("此模型不支援圖片傳入，請切換至支援圖片的模型。"); return;
    }
    clear();
    target = {conversation:state.active_id, request_id:crypto.randomUUID(), kind};
    pending = true;
    $("project-quick-notes").value = "";
    $("project-quick-path").value = "";
    command({action:"quick_prepare", ...target});
  }
  $("project-outlook-start").onclick = () => begin("outlook");
  $("project-image-start").onclick = () => begin("image");
  $("project-quick-cancel").onclick = clear;
  dialog.addEventListener("close", () => { if (ready && !dialog.open) clear(); });
  $("project-quick-choose").onclick = () => {
    if (current() && !pending) command({action:"quick_choose_image", ...identity()});
  };
  $("project-quick-send").onclick = () => {
    if (!current() || pending || !ready) return;
    const start = $("project-quick-from").value, end = $("project-quick-to").value;
    const path = $("project-quick-path").value.trim();
    if (target.kind === "outlook" && (!start || !end || start > end)) { toast("請確認起訖日期"); return; }
    if (target.kind === "image" && !path) { toast("請先選擇專案中的 JPG／PNG 圖片"); return; }
    pending = true; controls();
    command({action:"quick_submit", ...identity(), start, end, path, notes:$("project-quick-notes").value});
  };
  window.ProjectQuickUI = {
    render() {
      const project = state.conversations?.some(c => c.id === state.active_id && c.project_id);
      for (const id of ["project-outlook-start", "project-image-start"]) {
        $(id).hidden = !project;
        $(id).disabled = !state.can_send || !!state.projects?.running || !!state.work?.incoming;
      }
      $("project-image-start").title = state.config.model === "fast"
        ? "此模型不支援圖片傳入，請切換至支援圖片的模型。"
        : "指定專案圖片閱讀；AI 亦可在一般任務中按需選圖。單張最大5 MB，後續只保留文字重點。";
      if (target && (!current() || (state.projects?.running && !pending))) clear();
    },
    receive(message) {
      if (!current() || message.conversation !== target.conversation || message.request_id !== target.request_id) return;
      if (message.type === "project_quick_ready") {
        if (message.kind !== target.kind) return;
        const outlook = target.kind === "outlook";
        pending = false; ready = true;
        $("project-quick-title").textContent = outlook ? "Outlook 助理" : "圖片辨識";
        $("project-quick-outlook").hidden = !outlook;
        $("project-quick-image").hidden = outlook;
        $("project-quick-from").value = message.start;
        $("project-quick-to").value = message.end;
        $("project-quick-notes").placeholder = outlook ? "例如：只整理需要我回覆、尚待追蹤的事項。" : "預設：描述圖片內容，不需要辨識文字。也可指定要查看的區域或資訊。";
        $("project-quick-send").textContent = outlook ? "開始整理信件" : "送出圖片辨識";
        controls(); dialog.showModal();
      } else if (message.type === "project_quick_image") {
        $("project-quick-path").value = message.path;
      } else if (message.type === "project_quick_error") {
        clear();
      } else if (message.type === "project_quick_ack") {
        if (message.ok) clear();
        else { pending = false; controls(); }
      }
    },
  };
})();
