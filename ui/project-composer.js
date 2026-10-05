/* 專案執行中的訊息共用主輸入框；只由原生層決定補充、排程與停止界線。 */
"use strict";
(() => {
  const mode = $("project-send-mode"), prompt = $("prompt");
  let runKey = "", requestSequence = 0, pending = null, editing = null, queueSignature = "";
  const command = value => send({type:"project", command:value});
  const isActive = () => !!state.projects?.running && state.projects.running_conversation === state.active_id;
  const allowed = () => isActive() && state.logged_in && !state.update_required && state.busy === "none"
    && !state.projects.stopping && !pending && !window.WorkUI?.getFileBusy();

  function render() {
    const active = isActive(), project = state.projects || {};
    const key = `${state.active_id}:${active ? project.running_id : ""}`;
    if (key !== runKey) { runKey = key; mode.value = "next_turn"; editing = null; }
    if (!state.logged_in || state.update_required) pending = null;
    $("project-compose-controls").hidden = !active;
    mode.disabled = !allowed() || !!editing;
    // 排程一次保留一則；仍可用下一輪提示補充目前工作。
    for (const option of mode.options) option.disabled = option.value !== "next_turn" && !!project.queued;
    if (project.queued && mode.value !== "next_turn") mode.value = "next_turn";
    $("project-edit-cancel").hidden = !editing;
    $("project-compose-hint").textContent = project.stopping ? "正在停止目前任務；待檔案操作結束後接續。"
      : editing ? "正在修改尚未接收的補充指示；傳送後才更新。"
      : ({next_turn:"加入本次任務的下一輪 AI 請求；已開始的操作會先完成。",
        interrupt:"先停止目前任務、釋放檔案，再開始這則新任務。",
        after_task:"先保存待送訊息；本次完成後自動開始新任務，可在送出前取消。"}[mode.value]);
    prompt.maxLength = active && mode.value === "next_turn" ? 4000 : 16000;
    prompt.placeholder = active ? "直接輸入補充或下一個任務，再選擇傳送方式…" : "輸入訊息，或用快捷鍵帶入選取文字…";
    $("send").title = active ? mode.selectedOptions[0].textContent : "送出";
    $("send").setAttribute("aria-label", $("send").title);
    if (active) $("send").disabled = !allowed();
    else $("send").disabled = !!pending || !state.can_send || !!window.WorkUI?.getFileBusy();
    renderQueue(project);
  }

  function renderQueue(project) {
    const queue = project.queued, host = $("project-queued-message");
    host.hidden = !queue;
    const waiting = queue?.auto_start && project.running && queue.after_run === project.running_id;
    const signature = JSON.stringify([state.active_id, queue, waiting, state.can_send, project.running, state.logged_in, state.error, state.status]);
    if (signature === queueSignature) {
      // 原生拒絕也會推播狀態，即使錯誤文字相同仍恢復按鈕，避免無法重試。
      const start = host.querySelector("[data-queued-send]");
      if (start) start.disabled = !state.can_send;
      return;
    }
    queueSignature = signature; host.replaceChildren();
    if (!queue) return;
    host.append(node("strong", "", waiting ? (queue.interrupt ? "停止後將傳送" : "任務完成後將傳送") : "待送訊息已保留"));
    host.append(node("p", "", queue.text));
    const cancel = node("button", "text-button", "取消排程");
    cancel.disabled = !state.logged_in;
    cancel.onclick = () => command({action:"cancel_queued",conversation:state.active_id,id:queue.id});
    host.append(cancel);
    if (!project.running) {
      const start = node("button", "text-button", "現在傳送");
      start.dataset.queuedSend = "true";
      start.disabled = !state.can_send;
      start.onclick = () => { start.disabled = true; command({action:"send_queued",conversation:state.active_id,id:queue.id}); };
      host.append(start);
    }
    if (!waiting) host.append(node("small", "", "任務暫停、停止、失敗或程式重開後，請確認內容再傳送。"));
  }

  mode.onchange = render;
  $("project-edit-cancel").onclick = () => { editing = null; render(); };
  window.ProjectComposer = {
    render,
    canSend: allowed,
    editSupplement(entry) {
      if (!allowed()) return;
      if (prompt.value.trim()) { toast("輸入框已有草稿，請先處理後再修改補充。"); return; }
      editing = {id:entry.id, run_id:state.projects.running_id};
      mode.value = "next_turn"; prompt.value = entry.text; resizePrompt(); render(); prompt.focus();
      send({type:"draft",text:prompt.value});
    },
    submit(text, action) {
      if (pending) return true;
      if (!isActive()) return false;
      if (!allowed()) return true;
      if (action !== "send") { toast("請用傳送按鈕補充專案指示。"); return true; }
      if (!text.trim()) { toast("請先輸入文字"); return true; }
      if (text.length > prompt.maxLength) { toast(`此傳送方式最多 ${prompt.maxLength} 個字元；文字已保留。`); return true; }
      clearTimeout(draftTimer);
      pending = {conversation:state.active_id,run_id:state.projects.running_id,request_id:++requestSequence,
        mode:mode.value,instruction_id:editing?.id || null,text};
      // 先同步草稿，拒絕時保留；不以「已送出命令」當成原生層已接受。
      send({type:"draft",text});
      command({action:"compose",...pending});
      render(); return true;
    },
    receive(message) {
      if (message.type !== "project_compose_ack" || !pending || message.request_id !== pending.request_id
          || message.conversation !== pending.conversation || message.run_id !== pending.run_id) return;
      const submitted = pending; pending = null;
      if (!message.ok) { toast(message.error || "訊息未送出，文字已保留。"); render(); return; }
      // 等待回覆時新輸入的文字、其他對話的草稿都不可被舊 ack 清除。
      if (state.active_id === submitted.conversation && prompt.value === submitted.text) {
        prompt.value = ""; resizePrompt(); clearTimeout(draftTimer); send({type:"draft",text:""});
      }
      editing = null; mode.value = "next_turn";
      toast(submitted.mode === "next_turn" ? "補充已保存，將在下一輪加入" : "待送訊息已保存");
      render();
    },
  };
})();
