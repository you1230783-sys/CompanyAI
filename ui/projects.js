/* 專案工具只在明確建立的專案對話啟用；所有權限與檔案操作由原生層核對。 */
"use strict";
(() => {
  let signature = "", importProject = null, settingsProject = null, removeProject = null;
  let lastNotice = "", pickerRequest = 0, activityKey = "", activitySignature = "";
  const command = value => send({type: "project", command: value});
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
  $("project-diagnostics-live").onclick = () => { if (state.projects?.running_id) openDiagnostics(state.active_id, state.projects.running_id); };
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
    $("project-diagnostics-live").hidden = !showActivity;
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
