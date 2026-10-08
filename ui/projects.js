/* 專案工具只在明確建立的專案對話啟用；所有權限與檔案操作由原生層核對。 */
"use strict";
(() => {
  let signature = "", importProject = null, settingsProject = null, removeProject = null;
  let lastNotice = "", pickerRequest = 0, activityKey = "", activitySignature = "";
  const command = value => send({type: "project", command: value});
  // 非模態卡片不攔住其他操作；依原任務代號回答，關閉不代表同意。
  const preferences=document.createElement("section"); preferences.id="project-preferences";
  preferences.className="project-preferences"; preferences.hidden=true; document.body.append(preferences);
  let preferenceKey="";
  function renderPreferences(projects,visible) {
    const rows=visible ? (projects.preferences || []).filter(q=>q.state==="pending") : [];
    const key=JSON.stringify([projects.running_id,rows]); if(key===preferenceKey)return;
    preferenceKey=key;preferences.replaceChildren();preferences.hidden=!rows.length;
    for(const q of rows) {
      const card=node("div","preference-card"),title=node("strong","",q.question),hint=node("p","",`未回答時採用：${q.default}。工作會繼續。`);
      const select=document.createElement("select");select.setAttribute("aria-label",q.question);
      for(const option of q.options){const item=document.createElement("option");item.value=option;item.textContent=option;select.append(item);}select.value=q.default;
      const apply=node("button","primary-button","回答"),custom=document.createElement("input");custom.placeholder="也可自行輸入偏好";custom.maxLength=1000;
      apply.onclick=()=>command({action:"answer_preference",run_id:projects.running_id,question_id:q.id,answer:custom.value.trim()||select.value});
      card.append(title,hint,select,custom,apply);preferences.append(card);
    }
  }
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
    const selected=Array.from(outlookDialog.querySelectorAll("input[data-folder-id]:checked"),input=>input.dataset.folderId);
    command({action:"outlook_consent",...outlookTarget,allow,selected});
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
    const title=node("h2","","選擇可供 AI 使用的 Outlook 資料夾");
    const explanation=node("p","","這份清單只在本機顯示。第一次預設全選，之後沿用上次選擇；取消上層會一併取消子資料夾。未勾選的資料夾不提供 AI，也不允許讀信或匯出。");
    const limit=node("p","","確認後只開放使用範圍，不會立即上傳全部郵件。本機前文比對最多 1,000 封，AI 內文閱讀最多 50 封；不寄信、不修改郵件、不讀附件。設定供本機 Outlook 功能共用；既有對話中已送出的資料無法撤回，變更範圍後請用新對話。");
    const tree=node("div","outlook-folder-tree"), controls=node("div","dialog-actions");
    const folders=pending.folders || [], checks=new Map(), branches=new Map();
    const selectAll=node("button","","全選"),selectNone=node("button","","全部取消");
    const count=node("span","","");
    function refreshCount(){count.textContent=`已勾選 ${Array.from(checks.values()).filter(input=>input.checked).length} / ${folders.length} 個資料夾`;}
    selectAll.onclick=()=>{for(const input of checks.values()) input.checked=true;refreshCount();};
    selectNone.onclick=()=>{for(const input of checks.values()) input.checked=false;refreshCount();};
    controls.append(selectAll,selectNone,count);
    // 清單由原生層先序列舉；使用 textContent，資料夾名稱不能成為 HTML 或指令。
    for(const folder of folders){
      const row=node("div","outlook-folder-row"),label=node("label","","");
      const input=document.createElement("input");input.type="checkbox";input.checked=!!folder.selected;input.dataset.folderId=folder.id;
      checks.set(folder.id,input);label.append(input,document.createTextNode(folder.name));row.append(label);
      const children=document.createElement("details");
      const summary=node("summary","","子資料夾");children.append(summary);row.append(children);branches.set(folder.id,children);
      (branches.get(folder.parent) || tree).append(row);
      input.onchange=()=>{
        for(const child of row.querySelectorAll("input[data-folder-id]")) child.checked=input.checked;
        if(input.checked){
          let parent=folder.parent;
          while(parent){checks.get(parent).checked=true;parent=folders.find(item=>item.id===parent)?.parent;}
        }
        refreshCount();
      };
    }
    for(const branch of branches.values()) if(branch.children.length===1) branch.hidden=true;
    refreshCount();
    const buttons=node("div","dialog-actions"),deny=node("button","","取消本次讀取"),allow=node("button","primary","確認可供 AI 使用的資料夾");
    deny.id="project-outlook-deny";allow.id="project-outlook-allow";
    deny.onclick=()=>replyOutlook(false);allow.onclick=()=>replyOutlook(true);
    buttons.append(deny,allow);outlookDialog.append(title,explanation,controls,tree,limit,buttons);outlookDialog.showModal();deny.focus();
  }
  // 指示由主輸入框送出；此區只保留已提交項目的狀態與修改／撤回入口。
  let supplementSignature = "";
  function renderSupplements(project,visible) {
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
        row.append(button("修改",()=>window.ProjectComposer?.editSupplement(entry),false),button("撤回",()=>command({action:"withdraw_supplement",conversation:state.active_id,run_id:project.running_id,instruction_id:entry.id}),false));
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
    // 僅 Y 異常值有這三種選擇；X 無效必須排除整列，不能套用此示意。
    if (pending.review.groups.some(group=>!group.x_axis)) {
      reviewDialog.append(ChartReviewExamples.create());
    }
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
  $("debug-mode").onchange=()=>command({action:"set_debug",enabled:$("debug-mode").checked});
  // 複製只傳紀錄選擇，長報表由原生重讀並寫剪貼簿，避免超過 JS 訊息上限。
  $("project-token-copy").onclick=()=>{if(diagnosticsRequest)command({action:"diagnostics",...diagnosticsRequest,copy:true});};
  let diagnosticsRequest = null;
  function openDiagnostics(conversation, runId, index=null) {
    diagnosticsRequest = {conversation, run_id:runId, index};
    $("project-diagnostics-text").value = "正在讀取本機紀錄…";
    $("project-token-text").value = "";
    if (!$("project-diagnostics-dialog").open) $("project-diagnostics-dialog").showModal();
    command({action:"diagnostics", ...diagnosticsRequest});
  }
  $("project-diagnostics-close").onclick = () => $("project-diagnostics-dialog").close();
  $("project-diagnostics-dialog").addEventListener("close", () => { diagnosticsRequest = null; $("project-diagnostics-text").value = ""; $("project-token-text").value = ""; $("project-diagnostics-round").replaceChildren(); });
  $("project-diagnostics-round").onchange=()=>{if(diagnosticsRequest)openDiagnostics(diagnosticsRequest.conversation,diagnosticsRequest.run_id,Number($("project-diagnostics-round").value));};
  $("project-diagnostics-refresh").onclick = () => { if (diagnosticsRequest) openDiagnostics(diagnosticsRequest.conversation, diagnosticsRequest.run_id); };
  $("project-diagnostics-copy").onclick = () => {if(diagnosticsRequest)command({action:"diagnostics",...diagnosticsRequest,copy:false});};
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
      if (message.type === "project_diagnostics") {
        if (diagnosticsRequest && $("project-diagnostics-dialog").open && message.conversation === state.active_id &&
            message.conversation === diagnosticsRequest.conversation && message.run_id === diagnosticsRequest.run_id) {
          try { const reports=JSON.parse(message.text); if(diagnosticsRequest.index!=null && reports.selected!==diagnosticsRequest.index)return; $("project-diagnostics-text").value=reports.trace; $("project-token-text").value=reports.tokens;
            const select=$("project-diagnostics-round");select.replaceChildren();
            for(const round of reports.rounds||[]){const option=document.createElement("option");option.value=round.index;option.textContent=`第 ${Number(round.turn)||round.index+1} 輪（紀錄 ${round.index+1}）`;select.append(option);}
            select.value=reports.selected;select.disabled=!(reports.rounds||[]).length; }
          catch { $("project-diagnostics-text").value=message.text; $("project-token-text").value="此紀錄沒有獨立用量統計。"; }
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
    $("debug-mode").checked=!!state.config.debug_mode;
    const selector = $("project-diagnostics-run"), old = selector.value;
    const runs = new Map();
    for (const message of state.messages || []) {
      if (message.request_id) {
        runs.set(message.request_id, `${String(message.content || "專案任務").slice(0, 50)} · ${message.request_id.slice(0, 8)}`);
      }
    }
    if (showActivity) runs.set(projects.running_id, `執行中 · ${projects.running_id.slice(0, 8)}`);
    const diagnosticsSignature = JSON.stringify([state.active_id, [...runs]]);
    if (selector.dataset.signature !== diagnosticsSignature) {
      selector.replaceChildren(); selector.dataset.signature = diagnosticsSignature;
      for (const [id, title] of [...runs].reverse()) selector.add(new Option(title, id));
      if (!runs.size) selector.add(new Option("目前對話沒有執行紀錄", ""));
      if (runs.has(old)) selector.value = old;
    }
    selector.disabled = !state.logged_in || !runs.size;
    $("project-diagnostics-open").disabled = selector.disabled;
    if (diagnosticsRequest && (!state.logged_in || diagnosticsRequest.conversation !== state.active_id)) $("project-diagnostics-dialog").close();
    renderPreferences(projects,showActivity);
    ChartUI.render($("project-charts"), showActivity ? projects.charts : []);
    ChartUI.cleanup();
    AnalysisUI.render($("project-analysis"),showActivity ? projects.analysis : null);
    const key = showActivity ? projects.running_id : "";
    const events = showActivity ? (projects.activity || [projects.status]) : [];
    const nextActivity = JSON.stringify([key, events]);
    const activity = $("project-activity");
    if (nextActivity !== activitySignature) {
      const oldTop = $("transcript").scrollTop, follow = stickToBottom;
      if (key !== activityKey) {
        activity.open = false; activity.replaceChildren();
        $("project-narration").replaceChildren();
        delete $("project-narration").dataset.signature;
      }
      activityKey = key; activitySignature = nextActivity;
      renderProjectActivity(activity, events);
      renderProjectNarration($("project-narration"), events);
      if (showActivity && follow) bottom();
      else $("transcript").scrollTop = oldTop;
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
