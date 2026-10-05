/* 專案工具只在明確建立的專案對話啟用；所有權限與檔案操作由原生層核對。 */
"use strict";
(() => {
  let signature = "", importProject = null, settingsProject = null, removeProject = null;
  let lastNotice = "", pickerRequest = 0, activityKey = "", activitySignature = "";
  const command = value => send({type: "project", command: value});
  const fileDialog=document.createElement("dialog");fileDialog.id="project-file-busy";document.body.append(fileDialog);
  let fileKey="",fileTarget=null;
  function replyFile(retry) {
    if (!fileTarget) return;
    command({action:"file_ready",...fileTarget,retry});
    for (const button of fileDialog.querySelectorAll("button")) button.disabled=true;
  }
  fileDialog.addEventListener("cancel",event=>{event.preventDefault();replyFile(false);});
  fileDialog.addEventListener("close",()=>{if (fileTarget && !fileDialog.open) replyFile(false);});
  function renderFileBusy(project,visible) {
    const pending=visible ? project.file_busy : null;
    const key=pending ? `${project.running_id}:${pending.request_id}` : "";
    if (key===fileKey) return;
    fileKey=key;fileTarget=null;if (fileDialog.open) fileDialog.close();fileDialog.replaceChildren();
    if (!pending) return;
    fileTarget={conversation:state.active_id,run_id:project.running_id,request_id:pending.request_id};
    const title=node("h2","","請關閉檔案後繼續讀取"),message=node("p","",pending.message);
    const hint=node("p","","請先儲存並關閉相關 Office 視窗或檔案，再按「已關閉，再試一次」。程式會接續目前的讀取，不需要重做整個任務。");
    const buttons=node("div","dialog-actions"),later=node("button","","保留進度，稍後繼續"),retry=node("button","primary","已關閉，再試一次");
    later.id="project-file-later";retry.id="project-file-retry";
    later.onclick=()=>replyFile(false);retry.onclick=()=>replyFile(true);
    buttons.append(later,retry);fileDialog.append(title,message,hint,buttons);fileDialog.showModal();
  }
  const outlookDialog=document.createElement("dialog");
  outlookDialog.id="project-outlook-consent";
  document.body.append(outlookDialog);
  let outlookKey="",outlookTarget=null;
  function replyOutlook(allow) {
    if (!outlookTarget) return;
    command({action:"outlook_consent",...outlookTarget,allow});
    for (const button of outlookDialog.querySelectorAll("button")) button.disabled=true;
  }
  outlookDialog.addEventListener("cancel",event=>{event.preventDefault();replyOutlook(false);});
  outlookDialog.addEventListener("close",()=>{if (outlookTarget && !outlookDialog.open) replyOutlook(false);});
  function renderOutlookConsent(project,visible) {
    const pending=visible ? project.outlook_consent : null;
    const key=pending ? `${project.running_id}:${pending.request_id}` : "";
    if (key===outlookKey) return;
    outlookKey=key;outlookTarget=null;
    if (outlookDialog.open) outlookDialog.close();
    outlookDialog.replaceChildren();
    if (!pending) return;
    outlookTarget={conversation:state.active_id,run_id:project.running_id,request_id:pending.request_id};
    const title=node("h2","","允許本次任務讀取 Outlook？");
    const explanation=node("p","","先提供本地 PST 收件資料夾與線上信箱寄件備份的資料夾名稱，再由 AI 挑選資料夾、讀取指定日期的信件標題，並按需要讀取重要信件內文。這些資料會傳給本次使用的 AI 服務。");
    const limit=node("p","","僅讀取已開啟 Classic Outlook 中的資料，不寄信、不改信件、不讀附件。授權只到本次執行區段結束；暫停後繼續需重新同意。");
    const buttons=node("div","dialog-actions"),deny=node("button","","不同意"),allow=node("button","primary","同意讀取標題與選定內文");
    deny.id="project-outlook-deny";allow.id="project-outlook-allow";
    deny.onclick=()=>replyOutlook(false);allow.onclick=()=>replyOutlook(true);
    buttons.append(deny,allow);outlookDialog.append(title,explanation,limit,buttons);outlookDialog.showModal();deny.focus();
  }
  // 補充使用獨立輸入框，避免把主輸入框的未送出草稿誤當成新任務。
  const supplementDialog=document.createElement("dialog");
  supplementDialog.id="project-supplement-dialog";
  const supplementTitle=node("h2","","補充本次任務的指示");
  const supplementHint=node("p","","在下一輪 AI 請求帶入，持續到本次任務結束。已開始的操作會先完成；停止任務請使用停止按鈕。");
  const supplementText=document.createElement("textarea");
  supplementText.id="project-supplement-text";supplementText.rows=6;supplementText.maxLength=4000;
  supplementText.setAttribute("aria-label","補充指示內容");
  const supplementSend=node("button","primary","送出補充"), supplementClose=node("button","","關閉");
  supplementSend.id="project-supplement-send";
  const supplementButtons=node("div","dialog-actions");supplementButtons.append(supplementClose,supplementSend);
  supplementDialog.append(supplementTitle,supplementHint,supplementText,supplementButtons);document.body.append(supplementDialog);
  let supplementTarget=null, supplementSignature="";
  function openSupplement(entry=null) {
    const project=state.projects;
    if (!state.logged_in || !project?.running || project.running_conversation!==state.active_id) return;
    supplementTarget={conversation:state.active_id,run_id:project.running_id,instruction_id:entry?.id || null};
    if (entry) supplementText.value=entry.text;
    supplementSend.disabled=false;
    supplementDialog.showModal();supplementText.focus();
  }
  supplementClose.onclick=()=>supplementDialog.close();
  supplementSend.onclick=()=>{
    if (!supplementTarget || !supplementText.value.trim()) return;
    supplementSend.disabled=true;
    command({action:"supplement",...supplementTarget,text:supplementText.value});
  };
  $("project-supplement").onclick=()=>openSupplement();
  function renderSupplements(project,visible) {
    $("project-supplement").hidden=!visible;
    $("project-supplement").disabled=!visible;
    if (supplementDialog.open && (!visible || supplementTarget?.run_id!==project.running_id)) supplementDialog.close();
    // 原生錯誤同樣會重繪狀態；保留文字，允許修正後重送。
    if (supplementDialog.open) supplementSend.disabled=false;
    const entries=visible ? (project.supplements || []) : [];
    $("project-supplements").hidden=!entries.length;
    const signature=JSON.stringify([project.running_id,entries]);
    if (signature===supplementSignature) return;
    supplementSignature=signature;
    const list=$("project-supplement-list");list.replaceChildren();
    for (const entry of entries) {
      const row=node("div","supplement-entry");
      row.append(node("strong","",({pending:"等待下一輪接收",staged:"已接收，準備下一輪",sent:"已帶入模型請求",withdrawn:"已撤回"}[entry.status] || entry.status)),node("p","",entry.text));
      if (entry.status==="pending") {
        row.append(button("修改",()=>openSupplement(entry),false),button("撤回",()=>command({action:"withdraw_supplement",conversation:state.active_id,run_id:project.running_id,instruction_id:entry.id}),false));
      }
      list.append(row);
    }
  }
  // 決策只由桌面 UI 回傳，工具參數沒有可冒充使用者選擇的欄位。
  const reviewDialog=document.createElement("dialog");
  reviewDialog.className="chart-review-dialog"; reviewDialog.id="chart-review-dialog";
  document.body.append(reviewDialog);
  let reviewKey="", reviewSelection=null;
  function replyReview(choices) {
    if (!reviewSelection) return;
    command({action:"chart_choice",...reviewSelection,choices});
    for (const input of reviewDialog.querySelectorAll("button,select")) input.disabled=true;
  }
  reviewDialog.addEventListener("cancel",event=>{event.preventDefault();replyReview(null);});
  function renderReview(projects,visible) {
    const pending=visible ? projects.chart_review : null;
    const key=pending ? `${projects.running_id}:${pending.request_id}` : "";
    if (key===reviewKey) return;
    reviewKey=key; reviewSelection=null;
    if (reviewDialog.open) reviewDialog.close();
    reviewDialog.replaceChildren();
    if (!pending) return;
    reviewSelection={run_id:projects.running_id,request_id:pending.request_id};
    const heading=document.createElement("h2"); heading.textContent="如何處理圖表中的異常值？";
    const info=document.createElement("p");
    info.textContent=`${pending.review.title} · ${(pending.review.source || "").split(" | ")[0]}\n已轉換 ${pending.review.converted} 格數字文字；${pending.review.blanks} 格空白保留缺值。僅影響圖表。`;
    reviewDialog.append(heading,info);
    const selects=[];
    for (const group of pending.review.groups) {
      const label=document.createElement("label"), text=document.createElement("p"), select=document.createElement("select");
      text.textContent=`${group.column} 欄 · ${group.category} · ${group.count} 格\n${group.samples.join("；")}`;
      for (const choice of group.choices) {
        const option=document.createElement("option"); option.value=choice;
        option.textContent=group.x_axis ? "忽略整列（所有系列同步排除）" : ({gap:"保留缺值（折線中斷）",skip:"略過此點（折線接續，保留原 X 位置）",zero:"設為 0"}[choice]);
        select.append(option);
      }
      // 常見的 NG／文字缺值預設略過該點，仍讓使用者確認或改選。
      if (group.choices.includes("skip")) select.value="skip";
      label.append(text,select); reviewDialog.append(label); selects.push(select);
    }
    const buttons=document.createElement("div"), apply=document.createElement("button"), pause=document.createElement("button");
    apply.textContent="套用並繼續";apply.className="primary";apply.onclick=()=>replyReview(selects.map(s=>s.value));
    pause.textContent="暫存，稍後決定";pause.onclick=()=>replyReview(null);
    buttons.append(pause,apply);reviewDialog.append(buttons);reviewDialog.showModal();
  }
  const findProject = id => state.projects?.items?.find(project => project.id === id);
  let diagnosticsRequest = null;
  function openDiagnostics(conversation, runId) {
    diagnosticsRequest = {conversation, run_id:runId};
    $("project-diagnostics-text").value = "正在讀取本機紀錄…";
    if (!$("project-diagnostics-dialog").open) $("project-diagnostics-dialog").showModal();
    command({action:"diagnostics", ...diagnosticsRequest});
  }
  $("project-diagnostics-close").onclick = () => $("project-diagnostics-dialog").close();
  $("project-diagnostics-dialog").addEventListener("close", () => { diagnosticsRequest = null; $("project-diagnostics-text").value = ""; });
  $("project-diagnostics-refresh").onclick = () => { if (diagnosticsRequest) openDiagnostics(diagnosticsRequest.conversation, diagnosticsRequest.run_id); };
  $("project-diagnostics-copy").onclick = () => send({type:"copy",text:$("project-diagnostics-text").value});
  $("project-diagnostics-open").onclick = () => {
    const runId = $("project-diagnostics-run").value;
    if (!runId) return;
    $("settings-dialog").close();
    openDiagnostics(state.active_id, runId);
  };
  $("new-project").onclick = () => { $("project-name").value = ""; $("project-dialog").showModal(); $("project-name").focus(); };
  $("project-cancel").onclick = () => $("project-dialog").close();
  $("project-create").onclick = () => {
    const name = $("project-name").value.trim();
    if (!name) { toast("請輸入專案名稱"); return; }
    $("project-dialog").close(); showView("chat"); command({action:"create", name});
  };
  function openSettings(project) {
    settingsProject = project.id;
    $("project-edit-name").value = project.name;
    $("project-clear-imports").hidden = !project.import_count;
    $("project-settings-dialog").showModal();
  }
  $("project-settings-close").onclick = () => $("project-settings-dialog").close();
  $("project-settings-dialog").addEventListener("close", () => { settingsProject = null; });
  $("project-rename-save").onclick = () => {
    const name = $("project-edit-name").value.trim();
    if (!name) { toast("請輸入專案名稱"); return; }
    if (settingsProject) command({action:"rename", id:settingsProject, name});
    $("project-settings-dialog").close();
  };
  $("project-settings-import").onclick = () => {
    importProject = settingsProject;
    $("project-settings-dialog").close();
    $("project-import-path").value = "";
    $("project-import-text").value = "";
    $("project-import-save").disabled = true;
    $("project-import-dialog").showModal();
  };
  $("project-clear-imports").onclick = () => {
    const id = settingsProject;
    $("project-settings-dialog").close();
    ask("清除匯入文字？", "下次將重新讀取檔案。", () => command({action:"clear_imports", id}));
  };
  $("project-remove-open").onclick = () => {
    removeProject = settingsProject;
    const project = findProject(removeProject);
    $("project-settings-dialog").close();
    $("project-remove-message").textContent = `移除「${project?.name || "專案"}」後，要如何處理其中的對話？`;
    $("project-remove-dialog").showModal();
  };
  $("project-remove-cancel").onclick = () => $("project-remove-dialog").close();
  $("project-remove-dialog").addEventListener("close", () => { removeProject = null; });
  for (const [choice, deleteChats] of [["move", false], ["delete", true]]) {
    $("project-remove-" + choice).onclick = () => {
      if (removeProject) command({action:"remove", id:removeProject, delete_chats:deleteChats});
      $("project-remove-dialog").close();
    };
  }
  $("project-import-choose").onclick = () => {
    if (importProject) command({action:"choose_import_file", id:importProject, request_id:++pickerRequest});
  };
  $("project-import-cancel").onclick = () => $("project-import-dialog").close();
  $("project-import-save").onclick = () => {
    const path = $("project-import-path").value, text = $("project-import-text").value;
    if (!importProject || !path) { toast("請先選擇檔案"); return; }
    command({action:"import", id:importProject, path, text});
    $("project-import-dialog").close();
  };
  $("project-import-dialog").addEventListener("close", () => {
    $("project-import-text").value = ""; $("project-import-path").value = "";
    importProject = null; pickerRequest++;
  });
  $("project-stop").onclick = () => command({action:"stop"});

  function button(label, action, disabled) {
    const result = node("button", "text-button", label); result.type = "button";
    result.disabled = disabled; result.onclick = action; return result;
  }
  window.ProjectUI = {
    openDiagnostics,
    receive(message) {
      if (message.type==="project_supplement_ack") {
        if (supplementTarget?.run_id===message.run_id) {
          supplementText.value="";supplementDialog.close();supplementTarget=null;
          toast("補充指示已保存，將在下一輪帶入");
        }
        return;
      }
      if (message.type === "project_diagnostics") {
        if (diagnosticsRequest && $("project-diagnostics-dialog").open && message.conversation === state.active_id &&
            message.conversation === diagnosticsRequest.conversation && message.run_id === diagnosticsRequest.run_id) {
          $("project-diagnostics-text").value = message.text;
        }
        return;
      }
      // 原生選檔取消不改欄位；延遲回覆不能寫入另一個專案或已關閉的視窗。
      if (message.type !== "project_import_file" || !$("project-import-dialog").open ||
          message.id !== importProject || message.request_id !== pickerRequest) return;
      $("project-import-path").value = message.path;
      $("project-import-save").disabled = false;
      $("project-import-text").focus();
    },
    render() {
    const projects = state.projects || {items:[]};
    const showActivity = state.logged_in && projects.running && projects.running_conversation === state.active_id;
    renderSupplements(projects,showActivity);
    renderReview(projects,showActivity);
    renderOutlookConsent(projects,showActivity);
    renderFileBusy(projects,showActivity);
    // 診斷只在設定的進階區提供；保留目前對話每次任務的入口，不在聊天區佔位。
    const selector = $("project-diagnostics-run"), old = selector.value;
    const runs = new Map();
    for (const message of state.messages || []) {
      if (message.role === "assistant" && message.request_id && message.project_activity?.length) {
        runs.set(message.request_id, `${String(message.content || "專案任務").slice(0, 50)} · ${message.request_id.slice(0, 8)}`);
      }
    }
    if (showActivity) runs.set(projects.running_id, `執行中 · ${projects.running_id.slice(0, 8)}`);
    const diagnosticsSignature = JSON.stringify([state.active_id, [...runs]]);
    if (selector.dataset.signature !== diagnosticsSignature) {
      selector.replaceChildren(); selector.dataset.signature = diagnosticsSignature;
      for (const [id, title] of [...runs].reverse()) selector.add(new Option(title, id));
      if (!runs.size) selector.add(new Option("目前對話沒有專案執行紀錄", ""));
      if (runs.has(old)) selector.value = old;
    }
    selector.disabled = !state.logged_in || !runs.size;
    $("project-diagnostics-open").disabled = selector.disabled;
    if (diagnosticsRequest && (!state.logged_in || diagnosticsRequest.conversation !== state.active_id)) $("project-diagnostics-dialog").close();
    ChartUI.render($("project-charts"), showActivity ? projects.charts : []);
    ChartUI.cleanup();
    const key = showActivity ? projects.running_id : "";
    const events = showActivity ? (projects.activity || [projects.status]) : [];
    const nextActivity = JSON.stringify([key, events]);
    const activity = $("project-activity");
    if (nextActivity !== activitySignature) {
      if (key !== activityKey) activity.open = false;
      activityKey = key; activitySignature = nextActivity;
      renderProjectActivity(activity, events);
      renderProjectNarration($("project-narration"), events);
      if (showActivity && stickToBottom) bottom();
    }
    $("new-project").disabled = !!projects.running || state.busy !== "none" || !!state.work?.incoming;
    const notice = JSON.stringify([projects.error || projects.status || "", !!projects.error]);
    if (notice !== lastNotice) {
      lastNotice = notice;
      if (projects.error) toast(projects.error);
    }
    $("project-stop").hidden = !projects.running;
    $("project-stop").disabled = false;
    // 設定開啟後若任務開始，不保留可操作的舊設定視窗。
    if (projects.running) {
      for (const id of ["project-settings-dialog", "project-remove-dialog", "project-import-dialog"]) {
        if ($(id).open) $(id).close();
      }
    }
    const next = JSON.stringify([projects.items, projects.running, state.conversations, state.active_id, state.busy, state.work?.incoming]);
    if (next === signature) return;
    signature = next;
    const list = $("project-list"); list.replaceChildren();
    for (const project of projects.items || []) {
      const disabled = !!projects.running || state.busy !== "none" || !!state.work?.incoming;
      const card = node("div", "project-card"), heading = node("div", "project-heading");
      const title = node("strong", "", project.name); title.title = project.name;
      const newChat = button("＋ 新對話", () => {
        showView("chat"); command({action:"new_chat", id:project.id});
      }, disabled);
      newChat.title = "新增專案對話";
      const settings = node("button", "icon-button"); settings.type = "button";
      settings.innerHTML = icon("settings"); settings.title = "專案設定";
      settings.setAttribute("aria-label", `${project.name}：專案設定`);
      settings.disabled = disabled; settings.onclick = () => openSettings(project);
      heading.append(title, newChat, settings); card.append(heading);
      const conversations = state.conversations.filter(c => c.project_id === project.id).sort((a,b) => Number(!!b.pinned)-Number(!!a.pinned) || b.updated_at-a.updated_at);
      for (const c of conversations) {
        const row = node("div", "history-row" + (c.id === state.active_id ? " selected" : ""));
        const chat = node("button", "history-item", c.title); chat.dataset.id = c.id; chat.title = c.title;
        row.append(chat);
        const controls = node("div", "history-actions");
        for (const [action, symbol, label] of [["rename","pen","編輯標題"],["delete","trash","刪除對話"]]) {
          const control = node("button", "icon-button"); control.innerHTML = icon(symbol);
          control.dataset.historyAction = action; control.dataset.id = c.id; control.title = label;
          control.setAttribute("aria-label", `${label}：${c.title}`); controls.append(control);
        }
        row.append(controls); card.append(row);
      }
      list.append(card);
    }
    if (!projects.items?.length) list.append(node("p", "empty-small", "指定資料夾，開始文件工作"));
  }};
})();
