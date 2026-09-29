/* 專案工具只在明確建立的專案對話啟用；所有權限與檔案操作由原生層核對。 */
"use strict";
(() => {
  let signature = "", importProject = null;
  const command = value => send({type: "project", command: value});
  $("new-project").onclick = () => { $("project-name").value = ""; $("project-dialog").showModal(); $("project-name").focus(); };
  $("project-cancel").onclick = () => $("project-dialog").close();
  $("project-create").onclick = () => {
    const name = $("project-name").value.trim();
    if (!name) { toast("請輸入專案名稱"); return; }
    $("project-dialog").close(); showView("chat"); command({action:"create", name});
  };
  $("project-import-cancel").onclick = () => $("project-import-dialog").close();
  $("project-import-save").onclick = () => {
    const path = $("project-import-path").value.trim(), text = $("project-import-text").value;
    if (!importProject || !path) { toast("請填寫專案內的相對檔名"); return; }
    command({action:"import", id:importProject, path, text});
    $("project-import-dialog").close();
  };
  $("project-import-dialog").addEventListener("close", () => { $("project-import-text").value = ""; importProject = null; });
  $("project-stop").onclick = () => command({action:"stop"});

  function button(label, action, disabled) {
    const result = node("button", "text-button", label); result.type = "button";
    result.disabled = disabled; result.onclick = action; return result;
  }
  window.ProjectUI = {render() {
    const projects = state.projects || {items:[]};
    $("new-project").disabled = !!projects.running || state.busy !== "none" || !!state.work?.incoming;
    $("project-progress").hidden = !projects.status && !projects.error;
    $("project-progress-text").textContent = projects.error || projects.status || "";
    $("project-stop").hidden = !projects.running;
    const next = JSON.stringify([projects.items, projects.running, state.conversations, state.active_id, state.busy, state.work?.incoming]);
    if (next === signature) return;
    signature = next;
    const list = $("project-list"); list.replaceChildren();
    for (const project of projects.items || []) {
      const disabled = !!projects.running || state.busy !== "none" || !!state.work?.incoming;
      const card = node("div", "project-card"), heading = node("div", "project-heading");
      heading.append(node("strong", "", project.name), button("＋", () => {
        showView("chat"); command({action:"new_chat", id:project.id});
      }, disabled));
      heading.lastChild.title = "新增專案對話";
      card.append(heading, node("div", "project-path", project.root));
      const actions = node("div", "project-actions");
      actions.append(button("匯入文字", () => {
        importProject = project.id; $("project-import-path").value = ""; $("project-import-text").value = "";
        $("project-import-dialog").showModal();
      }, disabled));
      if (project.import_count) actions.append(button(`清除快照 (${project.import_count})`, () => ask("清除匯入快照？", "原始文件不變；下次會重新嘗試讀取檔案。", () => command({action:"clear_imports", id:project.id})), disabled));
      actions.append(button("移除專案", () => ask("移除專案授權？", "原始文件與成果不會刪除；專案對話會移到最近對話，並停止使用文件工具。", () => command({action:"remove", id:project.id})), disabled));
      card.append(actions);
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
