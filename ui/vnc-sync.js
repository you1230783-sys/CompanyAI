/* 網站同步只由按鈕啟動；預覽使用文字節點，不執行網站提供的 HTML 或連結。 */
"use strict";
(() => {
  const command = value => send({ type: "vnc", command: value });
  let settingsRevision = null;
  let previewId = null;
  let comparedRevision = null;
  const selected = new Set();
  const endpointInputs = [];
  for (let i = 0; i < 10; i++) {
    const label = node("label", "", `機台 API ${i + 1}`);
    const input = document.createElement("input");
    input.id = `vnc-sync-api-${i}`;
    input.maxLength = 2048;
    input.placeholder = "例如 api/machine/info_map.php?floor=2F";
    label.append(input);
    $("vnc-sync-endpoints").append(label);
    endpointInputs.push(input);
  }
  function save(start) {
    if (!state.config.vnc_enabled || state.vnc?.syncing) return;
    const settings = { username: $("vnc-sync-user").value.trim(), password: $("vnc-sync-password").value,
      endpoints: endpointInputs.map(input => input.value.trim()) };
    for (const name of ["root", "home", "login", "logout"]) settings[name] = $("vnc-sync-" + name).value.trim();
    command({ action: "sync_settings", settings, clear_password: $("vnc-sync-clear-password").checked, start });
    $("vnc-sync-password").value = "";
  }
  $("vnc-sync-save").onclick = () => save(false);
  $("vnc-sync-start").onclick = () => save(true);
  $("vnc-sync-cancel").onclick = () => command({ action: "cancel_sync" });
  $("vnc-sync-clear-password").onchange = () => {
    $("vnc-sync-password").disabled = $("vnc-sync-clear-password").checked;
    if ($("vnc-sync-clear-password").checked) $("vnc-sync-password").value = "";
  };
  $("vnc-import-discard").onclick = () => command({ action: "discard_import", preview_id: previewId });
  $("vnc-import-refresh").onclick = () => {
    if (!state.vnc?.syncing && previewId) command({ action: "refresh_import", preview_id: previewId });
  };
  $("vnc-import-apply").onclick = () => {
    if (!selected.size) return;
    command({ action: "import", revision: state.vnc.revision, preview_id: previewId, indices: [...selected] });
  };

  function renderPreview(preview) {
    const groups = new Map();
    for (const [index, machine] of (preview?.machines || []).entries()) {
      if (!groups.has(machine.group)) groups.set(machine.group, []);
      groups.get(machine.group).push({ index, ...machine });
    }
    $("vnc-import-groups").replaceChildren();
    for (const [name, machines] of groups) {
      const section = node("section", "vnc-import-group");
      const label = node("label", "check-row");
      const all = document.createElement("input"); all.type = "checkbox"; all.className = "vnc-import-category";
      label.append(all, node("strong", "", `${name}（${machines.length} 台）`)); section.append(label);
      const boxes = [];
      const available = machines.filter(m => m.comparison !== "same");
      all.disabled = !available.length;
      function changed() {
        const count = available.filter(m => selected.has(m.index)).length;
        all.checked = !!available.length && count === available.length;
        all.indeterminate = count > 0 && count < available.length;
        $("vnc-import-apply").disabled = !selected.size;
        $("vnc-import-apply").textContent = selected.size ? `匯入已選 ${selected.size} 台` : "匯入已選項目";
      }
      for (const machine of machines) {
        const row = node("label", "check-row vnc-import-machine");
        const checkbox = document.createElement("input"); checkbox.type = "checkbox";
        checkbox.disabled = machine.comparison === "same";
        checkbox.onchange = () => { if (checkbox.checked) selected.add(machine.index); else selected.delete(machine.index); changed(); };
        row.append(checkbox, node("span", "", machine.name), node("span", "subtle", machine.ip));
        if (machine.comparison === "same" || machine.comparison === "changed") {
          row.classList.add(`vnc-compare-${machine.comparison}`);
          row.append(node("span", "vnc-comparison", machine.comparison === "same" ? "與現有設定一致" : "與現有設定不相符"));
        }
        section.append(row); boxes.push(checkbox);
      }
      all.onchange = () => { machines.forEach((m, i) => {
        boxes[i].checked = !boxes[i].disabled && all.checked;
        if (boxes[i].checked) selected.add(m.index); else selected.delete(m.index);
      }); changed(); };
      $("vnc-import-groups").append(section);
    }
    $("vnc-import-apply").disabled = true;
    $("vnc-import-apply").textContent = "匯入已選項目";
  }

  window.VncSyncUI = {
    render() {
      const enabled = !!state.config.vnc_enabled;
      const data = state.vnc || {};
      const settings = data.sync_settings;
      $("vnc-sync-controls").hidden = !enabled;
      if (!enabled) {
        $("vnc-sync-user").value = $("vnc-sync-password").value = "";
        settingsRevision = previewId = comparedRevision = null; selected.clear(); renderPreview(null);
        $("vnc-import-preview").hidden = true;
        return;
      }
      if (settings && settingsRevision !== data.settings_revision) {
        settingsRevision = data.settings_revision;
        $("vnc-sync-user").value = settings.username;
        $("vnc-sync-password").value = "";
        $("vnc-sync-password").placeholder = settings.has_password ? "已記住密碼；留空沿用" : "輸入網站登入密碼";
        $("vnc-sync-clear-password").checked = false;
        for (const name of ["root", "home", "login", "logout"]) $("vnc-sync-" + name).value = settings[name];
        endpointInputs.forEach((input, i) => input.value = settings.endpoints[i] || "");
      }
      for (const input of $("vnc-sync-controls").querySelectorAll("input,button")) input.disabled = !settings || !!data.syncing || !!data.session_open;
      $("vnc-sync-password").disabled ||= $("vnc-sync-clear-password").checked;
      $("vnc-sync-cancel").hidden = !data.syncing || !!data.closing;
      $("vnc-sync-cancel").disabled = !data.syncing || !!data.closing;
      $("vnc-reload").disabled = !!data.syncing;
      $("vnc-import-refresh").disabled = !!data.syncing || !data.preview;
      $("vnc-import-discard").disabled = !!data.syncing;
      $("vnc-import-preview").hidden = !data.preview;
      if ((data.preview?.id ?? null) !== previewId || data.revision !== comparedRevision) {
        previewId = data.preview?.id ?? null;
        comparedRevision = data.revision;
        selected.clear(); renderPreview(data.preview);
      }
      if (data.syncing) $("vnc-import-apply").disabled = true;
    },
  };
})();
