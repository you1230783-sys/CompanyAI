/* 週報精靈僅保存當次填寫內容；原生層核對素材路徑、身分與啟動條件。 */
"use strict";
(() => {
  const command = value => send({type:"project", command:value});
  const info = $("weekly-info-dialog"), input = $("weekly-input-dialog");
  const confirm = $("weekly-confirm-dialog"), location = $("project-default-dialog");
  let target = null, submitting = false;

  function clear() {
    const old = target;
    target = null;
    submitting = false;
    for (const dialog of [confirm, input, info]) if (dialog.open) dialog.close();
    if (old) command({action:"weekly_cancel", ...old});
  }
  function current() {
    return target && target.conversation === state.active_id && state.logged_in && !state.update_required;
  }
  function controls() {
    $("weekly-info-ok").disabled = submitting;
    $("weekly-send").disabled = submitting;
    $("weekly-confirm-yes").disabled = submitting;
  }
  $("project-default-folder").onclick = () => {
    if (!$("project-name").value.trim()) { toast("請輸入專案名稱"); return; }
    location.showModal();
  };
  $("project-default-cancel").onclick = () => location.close();
  for (const place of ["downloads", "desktop"]) {
    $(`project-default-${place}`).onclick = () => {
      const name = $("project-name").value.trim();
      location.close();
      $("project-dialog").close();
      showView("chat");
      command({action:"create_default", name, location:place});
    };
  }
  $("weekly-start").onclick = () => {
    clear();
    target = {conversation:state.active_id, request_id:crypto.randomUUID()};
    $("weekly-notes").value = "";
    controls();
    info.showModal();
  };
  $("weekly-info-cancel").onclick = clear;
  $("weekly-input-cancel").onclick = clear;
  // 外部點擊或 Escape 取消準備，不刪資料；確認視窗關閉則返回仍開啟的輸入視窗。
  for (const dialog of [info, input]) dialog.addEventListener("close", () => {
    if (target && !info.open && !input.open && !confirm.open) clear();
  });
  $("weekly-info-ok").onclick = () => {
    if (!current() || submitting) return;
    submitting = true;
    controls();
    command({action:"weekly_prepare", ...target});
  };
  $("weekly-folder").onclick = () => {
    if (current()) command({action:"weekly_open", ...target});
  };
  $("weekly-send").onclick = () => {
    if (!current() || submitting) return;
    if (!$("weekly-from").value || !$("weekly-to").value || $("weekly-from").value > $("weekly-to").value) {
      toast("請確認週報開始與結束日期");
      return;
    }
    confirm.showModal();
    $("weekly-confirm-no").focus();
  };
  $("weekly-confirm-no").onclick = () => confirm.close();
  $("weekly-confirm-yes").onclick = () => {
    if (!current() || submitting) return;
    submitting = true;
    controls();
    command({action:"weekly_submit", ...target, confirmed:true,
      start:$("weekly-from").value, end:$("weekly-to").value, notes:$("weekly-notes").value});
  };
  window.WeeklyUI = {
    render() {
      const project = state.conversations?.some(c => c.id === state.active_id && c.project_id);
      document.querySelectorAll("[data-action]").forEach(button => { button.hidden = !!project; });
      $("weekly-start").hidden = !project;
      $("weekly-start").disabled = !state.can_send || !!state.projects?.running || !!state.work?.incoming;
      if (target && (!current() || (state.projects?.running && !submitting))) clear();
    },
    receive(message) {
      if (!target || message.conversation !== target.conversation || message.request_id !== target.request_id || !current()) return;
      if (message.type === "weekly_ready") {
        submitting = false;
        $("weekly-folder").textContent = message.path;
        $("weekly-from").value = message.start;
        $("weekly-to").value = message.end;
        controls();
        input.showModal();
        info.close();
      } else if (message.type === "weekly_error") {
        submitting = false;
        controls();
      } else if (message.type === "weekly_submit_ack") {
        if (message.ok) clear();
        else { submitting = false; controls(); if (confirm.open) confirm.close(); }
      }
    },
  };
})();
