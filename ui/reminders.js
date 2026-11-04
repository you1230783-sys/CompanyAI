/* 單次提醒由原生端保存及排程，介面只負責輸入與顯示本機時間。 */
"use strict";
window.ReminderUI = (() => {
  const dialog = document.createElement("dialog");
  dialog.id = "reminders-dialog";
  dialog.dataset.offline = "true";
  dialog.className = "reminders-dialog";
  dialog.innerHTML = `<div class="tarot-heading"><h2>提醒事項</h2><button data-close aria-label="關閉提醒事項">✕</button></div>
    <p>新增一則提醒，選擇時間與通知方式；也可編輯或刪除。所有資料只保存在這台電腦。</p>
    <p class="subtle">程式需保持執行，縮到托盤仍可提醒；完全退出期間到期的項目，會在下次啟動時依序補送。Windows 勿擾或系統通知設定可能影響顯示。桌寵泡泡設為0秒或全透明時，改用 Windows 通知。提醒使用此處選擇的方式。</p>
    <form data-form><label>提醒內容<input data-title type="text" maxlength="180" required placeholder="例如：下午開會、休息一下或回覆文件"></label>
    <div class="reminder-fields"><label>提醒時間（本機時間）<input data-time type="datetime-local" required></label><label>通知方式<select data-channel><option value="pet">桌寵</option><option value="windows">Windows 通知</option></select></label></div>
    <div class="dialog-actions"><button data-save type="submit" class="primary-button">新增提醒</button><button data-reset type="button">取消編輯／清空</button></div></form>
    <p data-status role="status" aria-live="polite"></p><div data-list class="reminder-list"></div>`;
  document.body.append(dialog);
  const find = selector => dialog.querySelector(selector);
  let entries = [], editing = null, pending = null, ready = false, workflow = null;
  const repeat=document.createElement("select"); repeat.innerHTML='<option value="0">單次</option><option value="1">每24小時</option><option value="7">每7天</option>';
  const repeatLabel=document.createElement("label");repeatLabel.textContent="重複週期（固定間隔，夏令時間變更後請重新設定）";repeatLabel.append(repeat);find("[data-form]").prepend(repeatLabel);
  const workflowHint=document.createElement("p");find("[data-form]").prepend(workflowHint);
  function localDate(value) {
    const d = new Date(value * 1000), pad = v => String(v).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
  function reset() {
    editing = null; workflow = null; repeat.value="0"; workflowHint.textContent="";
    find("[data-title]").value = "";
    find("[data-time]").value = localDate(Math.floor(Date.now()/1000) + 3600);
    find("[data-channel]").value = featureEnabled("pet") ? "pet" : "windows";
    find("[data-save]").textContent = "新增提醒";
  }
  function controls() {
    for (const control of dialog.querySelectorAll("form input,form select,form button,[data-list] button")) control.disabled = !!pending || !ready;
  }
  function transmit(command) {
    pending = command.action;
    controls();
    find("[data-status]").textContent = "處理中…";
    send({type:"reminder", command});
  }
  function render() {
    const list = find("[data-list]");
    list.replaceChildren();
    if (!entries.length) { list.textContent = "尚無提醒事項。"; return; }
    const sorted = [...entries].sort((a,b) => Number(a.notified_at != null)-Number(b.notified_at != null) || a.due_at-b.due_at);
    for (const entry of sorted) {
      const row = document.createElement("article"), details = document.createElement("div"), title = document.createElement("strong"), subtitle = document.createElement("p"), actions = document.createElement("div");
      row.className = "reminder-row";
      title.textContent = entry.title;
      subtitle.textContent = `${new Date(entry.due_at*1000).toLocaleString()} · ${entry.channel === "pet" ? "桌寵" : "Windows"} · ${entry.notified_at != null ? "已送出提醒" : "等待提醒"}`;
      details.append(title,subtitle);
      for (const [label, action] of [["編輯", () => {
        editing = entry.id; repeat.value=String(entry.repeat_days||0);workflow=entry.workflow?{conversation:entry.workflow.conversation,template:entry.workflow.template}:null;workflowHint.textContent=workflow?"流程提醒：只通知，不自動執行；載入後填入參數與確認。":"";
        find("[data-title]").value = entry.title;
        find("[data-time]").value = localDate(entry.due_at);
        find("[data-channel]").value = entry.channel;
        find("[data-save]").textContent = "保存變更";
        find("[data-status]").textContent = "編輯已提醒的項目時，請選擇未來時間重新排程。";
        find("[data-title]").focus();
      }], ["刪除", () => transmit({action:"delete",id:entry.id})]]) {
        const button = document.createElement("button"); button.type = "button"; button.textContent = label; button.onclick = action; actions.append(button);
      }
      if(entry.workflow){const load=document.createElement("button");load.textContent="載入流程草稿";load.onclick=()=>transmit({action:"load_workflow",id:entry.id});actions.append(load);}
      row.append(details,actions); list.append(row);
    }
  }
  find("[data-close]").onclick = () => dialog.close();
  find("[data-reset]").onclick = reset;
  find("[data-form]").onsubmit = event => {
    event.preventDefault();
    if (pending || !ready) return;
    const title = find("[data-title]").value.trim(), input = find("[data-time]").value;
    const timestamp = new Date(input).getTime(), due_at = Math.floor(timestamp/1000);
    // 避免夏令時間不存在的時刻被Date自動挪到另一個時間。
    if (!title || !Number.isFinite(timestamp) || localDate(due_at) !== input || timestamp <= Date.now()) {
      find("[data-status]").textContent = "請填寫內容並選擇有效的未來時間。"; return;
    }
    transmit({action:"save",id:editing,title,due_at,channel:find("[data-channel]").value,repeat_days:Number(repeat.value),workflow});
  };
  function open() {
    const petOption = find('[data-channel] option[value="pet"]');
    petOption.textContent = featureEnabled("pet") ? "桌寵" : "桌寵（此建置改用Windows通知）";
    if (!editing && !featureEnabled("pet")) find("[data-channel]").value = "windows";
    if (!dialog.open) dialog.showModal();
    transmit({action:"state"});
  }
  $("show-reminders").onclick = open;
  reset(); controls();
  return {open, schedule(flow,conversation) { reset();workflow={conversation,template:flow.id};find("[data-title]").value=`準備執行：${flow.name}`.slice(0,180);workflowHint.textContent="到期只通知。載入時核對帳號、專案及流程版本；填入本次參數後由您送出。";open(); }, receive(message) {
    if(message.type==="reminder_flow"){pending=null;controls();dialog.close();window.WorkbenchUI?.workflow(message.flow,message.conversation);return true;}
    if (message.type === "reminder_error") {
      pending = null; find("[data-status]").textContent = message.message; controls(); return true;
    }
    if (message.type !== "reminder_state") return false;
    entries = message.entries || [];
    if (message.acknowledgement) {
      if (pending === "save" || (pending === "delete" && !entries.some(e => e.id === editing))) reset();
      pending = null;
    }
    ready = true;
    if (!pending) find("[data-status]").textContent = `已保存 ${entries.length} 項提醒（最多200項）。`;
    render(); controls(); return true;
  }};
})();
