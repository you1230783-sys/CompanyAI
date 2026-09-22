/* VNC 專用介面：只送出明確的本機操作，不將機台、位址或密碼加入聊天內容。 */
"use strict";
(() => {
  let currentGroup = null;
  let selected = null;
  let renderedRevision = null;
  const command = (value) => send({ type: "vnc", command: value });
  const dialog = $("vnc-manager-dialog");

  function clearForm() {
    selected = null;
    $("vnc-machine-form").reset();
    $("vnc-password").placeholder = "新機台密碼，可留空";
    $("vnc-password").disabled = false;
    $("vnc-delete-machine").disabled = true;
  }

  function editMachine(group, machine) {
    clearForm();
    selected = { group, index: machine.index, revision: state.vnc.revision };
    $("vnc-group").value = group;
    $("vnc-name").value = machine.name;
    $("vnc-ip").value = machine.ip;
    $("vnc-password").placeholder = machine.has_password ? "留空保留已存密碼" : "尚未設定密碼";
    $("vnc-delete-machine").disabled = false;
  }

  function renderGroups() {
    const groups = state.vnc?.groups || [];
    if (!groups.some(group => group.name === currentGroup)) currentGroup = groups[0]?.name ?? null;
    $("vnc-groups").replaceChildren();
    $("vnc-machines").replaceChildren();
    for (const group of groups) {
      const button = node("button", "secondary-button", group.name);
      button.setAttribute("aria-pressed", String(group.name === currentGroup));
      button.onclick = () => { currentGroup = group.name; renderGroups(); };
      $("vnc-groups").append(button);
    }
    const machines = groups.find(group => group.name === currentGroup)?.machines || [];
    for (const machine of machines) {
      const button = node("button", "vnc-machine");
      button.append(node("strong", "", machine.name), node("span", "subtle", machine.ip));
      button.title = `連線至 ${machine.name}`;
      button.disabled = !state.vnc?.loaded;
      // 閉包綁定畫面快照的分類、索引與版次，不允許前端指定任意連線參數。
      const group = currentGroup;
      const revision = state.vnc.revision;
      button.onclick = () => command({ action: "connect", revision, group, index: machine.index });
      $("vnc-machines").append(button);
    }
    if (!machines.length) $("vnc-machines").append(node("p", "subtle", "此分類沒有機台，請使用「機台管理」新增，或放入既有 machines.json 後重新讀取。"));
  }

  function renderManager() {
    $("vnc-manager-rows").replaceChildren();
    $("vnc-group-names").replaceChildren();
    for (const group of state.vnc?.groups || []) {
      const option = document.createElement("option");
      option.value = group.name;
      $("vnc-group-names").append(option);
      for (const machine of group.machines) {
        const row = document.createElement("tr");
        for (const value of [group.name, machine.name, machine.ip, machine.has_password ? "已設定" : "未設定"]) {
          row.append(node("td", "", value));
        }
        const cell = document.createElement("td");
        const edit = node("button", "text-button", "編輯");
        edit.onclick = () => editMachine(group.name, machine);
        cell.append(edit);
        for (const [direction, label, boundary] of [["up", "上移", machine.index === 0], ["down", "下移", machine.index === group.machines.length - 1]]) {
          const move = node("button", "text-button", label);
          move.disabled = boundary;
          const revision = state.vnc.revision;
          move.onclick = () => command({ action: "move_machine", revision, group: group.name, index: machine.index, direction });
          cell.append(move);
        }
        row.append(cell);
        $("vnc-manager-rows").append(row);
      }
    }
  }

  $("show-vnc").onclick = () => {
    if (!state.config.vnc_enabled) return;
    showView("vnc");
    if (!state.vnc?.loaded) command({ action: "open" });
  };
  $("vnc-reload").onclick = () => { clearForm(); command({ action: "reload" }); };
  $("vnc-choose-viewer").onclick = () => command({ action: "choose_viewer" });
  $("vnc-search-viewer").onclick = () => command({ action: "search_viewer" });
  for (const option of ["fullscreen", "viewonly", "autoscaling"]) {
    $("vnc-" + option).onchange = () => command({ action: "options",
      fullscreen: $("vnc-fullscreen").checked,
      viewonly: $("vnc-viewonly").checked,
      autoscaling: $("vnc-autoscaling").checked });
  }
  $("vnc-manage").onclick = () => { clearForm(); renderManager(); dialog.showModal(); };
  $("vnc-manager-close").onclick = () => dialog.close();
  dialog.addEventListener("close", clearForm);
  $("vnc-new-machine").onclick = clearForm;
  $("vnc-clear-password").onchange = () => {
    $("vnc-password").disabled = $("vnc-clear-password").checked;
    if ($("vnc-clear-password").checked) $("vnc-password").value = "";
  };
  $("vnc-machine-form").onsubmit = event => {
    event.preventDefault();
    const password = $("vnc-clear-password").checked ? "" : $("vnc-password").value || null;
    command({ action: "save_machine", revision: selected?.revision ?? state.vnc.revision,
      original: selected ? { group: selected.group, index: selected.index } : null,
      group: $("vnc-group").value.trim(), name: $("vnc-name").value.trim(),
      ip: $("vnc-ip").value.trim(), password });
    // 新輸入的密碼只傳一次；失敗時讓使用者重新輸入，不保存在 JS 狀態中。
    $("vnc-password").value = "";
  };
  $("vnc-delete-machine").onclick = () => {
    if (!selected) return;
    const target = { ...selected };
    // 使用原生於此 dialog 的確認方式，避免另一個對話框被 modal 遮住。
    if (window.confirm(`確定刪除「${$("vnc-name").value}」？`)) {
      command({ action: "delete_machine", ...target });
    }
  };

  window.VncUI = {
    render() {
      const enabled = !!state.config.vnc_enabled;
      $("show-vnc").hidden = !enabled;
      if (!enabled) {
        if (activeView === "vnc") showView("chat");
        if (dialog.open) dialog.close();
        clearForm();
        currentGroup = renderedRevision = null;
        $("vnc-groups").replaceChildren();
        $("vnc-machines").replaceChildren();
        $("vnc-manager-rows").replaceChildren();
        return;
      }
      const data = state.vnc || {};
      for (const id of ["vnc-manage", "vnc-choose-viewer", "vnc-save-machine", "vnc-fullscreen", "vnc-viewonly", "vnc-autoscaling"]) {
        $(id).disabled = !data.loaded;
      }
      $("vnc-search-viewer").disabled = !data.loaded || !!data.searching;
      $("vnc-viewer-path").textContent = data.viewer_path || "尚未指定";
      $("vnc-machines-path").textContent = data.machines_path || "LM_AI.exe 同一資料夾的 machines.json";
      $("vnc-status").textContent = $("vnc-manager-status").textContent = data.status || "請開啟 VNC 頁面讀取設定。";
      for (const option of ["fullscreen", "viewonly", "autoscaling"]) $("vnc-" + option).checked = !!data.options?.[option];
      if (renderedRevision !== data.revision) {
        renderedRevision = data.revision;
        renderGroups();
        renderManager();
      }
    },
    receive(message) { if (message.type === "vnc_saved") clearForm(); },
  };
})();
