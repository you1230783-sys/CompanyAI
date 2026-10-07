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
  }
  function clear() {
    const old = target;
    target = null; pending = false; ready = false;
    if (dialog.open) dialog.close();
    if (old) command({action:"quick_cancel", conversation:old.conversation, request_id:old.request_id});
  }
  function begin() {
    if (pending) return;
    clear();
    target = {conversation:state.active_id, request_id:crypto.randomUUID(), kind:"outlook"};
    pending = true;
    $("project-quick-notes").value = "";
    command({action:"quick_prepare", ...target});
  }
  $("project-outlook-start").onclick = begin;
  $("project-quick-cancel").onclick = clear;
  dialog.addEventListener("close", () => { if (ready && !dialog.open) clear(); });
  $("project-quick-send").onclick = () => {
    if (!current() || pending || !ready) return;
    const start = $("project-quick-from").value, end = $("project-quick-to").value;
    if (!start || !end || start > end) { toast("請確認起訖日期"); return; }
    pending = true; controls();
    command({action:"quick_submit", ...identity(), start, end, notes:$("project-quick-notes").value});
  };
  window.ProjectQuickUI = {
    render() {
      const project = state.conversations?.some(c => c.id === state.active_id && c.project_id);
      $("project-outlook-start").hidden = !project;
      $("project-outlook-start").disabled = !state.can_send || !!state.projects?.running || !!state.work?.incoming;
      if (target && (!current() || (state.projects?.running && !pending))) clear();
    },
    receive(message) {
      if (!current() || message.conversation !== target.conversation || message.request_id !== target.request_id) return;
      if (message.type === "project_quick_ready") {
        if (message.kind !== target.kind) return;
        pending = false; ready = true;
        $("project-quick-title").textContent = "Outlook 助理";
        $("project-quick-from").value = message.start;
        $("project-quick-to").value = message.end;
        $("project-quick-notes").placeholder = "例如：只整理需要我回覆、尚待追蹤的事項。";
        $("project-quick-send").textContent = "開始整理信件";
        controls(); dialog.showModal();
      } else if (message.type === "project_quick_error") {
        clear();
      } else if (message.type === "project_quick_ack") {
        if (message.ok) clear();
        else { pending = false; controls(); }
      }
    },
  };
})();
