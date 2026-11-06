/* 私聊畫面獨立於 AI 對話。所有伺服器資料只以文字節點顯示。 */
"use strict";
window.PrivateChatUI = (() => {
  const symbols = "!@#$%^&*()-_=+[]{};:,.?/~";
  const defaultPolicy = {when:"leave", minutes:0, windows_lock:true};
  const delays = {leave:[0,1,5,15,30,60], since_unlock:[5,15,30,60,240,480,720,1440]};
  const labels = {active:"可聊天", offline:"離線", disabled:"帳號已停用", removed:"已解除好友"};
  const request = (action, fields = {}) => send({type:"private_chat", command:{action, ...fields}});
  const text = (element, value) => { element.textContent = value ?? ""; };
  function askInChat(title, description, action) {
    ask(title, description, action);
    $("confirm-dialog").dataset.privateChat = "true";
  }
  let current = {}, selected = null;

  const pane = document.createElement("dialog");
  pane.id = "private-chat";
  pane.setAttribute("aria-label", "私聊");
  pane.innerHTML = `
    <div class="private-chat-layout">
      <aside aria-label="私聊選單">
        <button type="button" data-return class="nav-button"><img src="app.ico" alt="" />返回主界面</button>
        <button type="button" data-advanced class="nav-button">進階功能</button>
        <button type="button" data-vnc class="nav-button" hidden>VNC</button>
        <button type="button" data-add class="nav-button">加入好友</button>
        <div data-friends class="private-chat-friends" aria-label="好友清單"></div>
      </aside>
      <section class="private-chat-conversation">
        <header>
          <h2 data-friend-title>私聊</h2>
          <button data-lock type="button">上鎖</button>
        </header>
        <p data-pane-status role="status"></p>
        <p data-thread-state></p>
        <button data-remove-friend type="button" hidden>解除好友</button>
        <div data-thread class="private-chat-thread" aria-live="polite"></div>
        <footer>
          <p>聊天內容僅加密儲存於本機，刪除後無法復原。雙方各存一份。</p>
        </footer>
      </section>
    </div>`;
  document.body.append(pane);
  const inPane = selector => pane.querySelector(selector);
  const messages = window.PrivateChatMessages.mount(pane, command => request("message", {command}), askInChat);

  const contacts = document.createElement("dialog");
  contacts.id = "private-chat-contacts";
  contacts.setAttribute("aria-labelledby", "private-chat-contacts-title");
  contacts.innerHTML = `
    <div class="dialog-heading">
      <h2 id="private-chat-contacts-title">加入好友</h2>
      <button data-close type="button" aria-label="關閉加入好友">✕</button>
    </div>
    <p>自己的聊天 ID <strong data-id></strong></p>
    <div class="button-row"><button data-copy type="button">複製 ID</button><button data-regenerate type="button">重新產生 ID</button></div>
    <p class="subtle">重新產生後舊 ID 立即失效，既有好友不受影響。</p>
    <h3>收到的邀請</h3><div data-incoming></div>
    <h3>送出的邀請</h3><div data-outgoing></div>
    <form data-lookup><label>對方的聊天 ID <input name="code" placeholder="XXXX-XXXX" maxlength="9" required /></label><button type="submit">搜尋</button></form>
    <p data-found hidden></p><button data-invite type="button" hidden>確認並送出邀請</button>
    <h3>黑名單</h3><div data-blocked></div>
    <p data-contacts-status role="status"></p>`;
  document.body.append(contacts);
  const inContacts = selector => contacts.querySelector(selector);

  const password = document.createElement("dialog");
  password.id = "private-chat-password";
  password.setAttribute("aria-labelledby", "private-chat-password-title");
  password.innerHTML = `
    <h2 id="private-chat-password-title"></h2>
    <form>
      <label data-current>目前聊天密碼<input name="old" type="password" autocomplete="current-password" /></label>
      <label><span data-label>聊天密碼</span><input name="password" type="password" required /></label>
      <label data-repeat>再輸入一次<input name="repeat" type="password" autocomplete="new-password" /></label>
      <p class="subtle">6 到 64 個字元，接受大寫英文 A～Z、小寫英文 a～z、數字 0～9 及下列符號（大小寫不同）。</p>
      <p data-symbols class="subtle"></p>
      <p class="subtle">混合大小寫、數字與符號會更安全。</p>
      <p data-password-status role="status"></p>
      <div class="button-row"><button data-submit type="submit">確定</button><button data-cancel type="button">取消</button></div>
    </form>`;
  document.body.append(password);
  const form = password.querySelector("form");
  const secret = form.elements.namedItem("password");
  const old = form.elements.namedItem("old");
  const repeated = form.elements.namedItem("repeat");
  const passwordStatus = password.querySelector("[data-password-status]");
  let passwordAction = "unlock";
  text(password.querySelector("[data-symbols]"), [...symbols].join(" "));

  function passwordProblem(value) {
    const invalid = [...value].find(character => !/^[a-zA-Z0-9]$/.test(character) && !symbols.includes(character));
    if (invalid) {
      const name = invalid === " " ? "空白"
        : /[\u0000-\u001f\u007f]/.test(invalid) ? "U+" + invalid.codePointAt(0).toString(16).padStart(4,"0").toUpperCase()
        : "「" + invalid + "」";
      return "聊天密碼不能包含" + name + "。";
    }
    if (value.length < 6 || value.length > 64) return "聊天密碼必須為 6 到 64 個字元。";
    return "";
  }

  function showPassword(action, hasPassword) {
    if (!password.open) form.reset();
    passwordAction = action;
    const change = action === "change_password";
    const setup = change || !hasPassword;
    password.querySelector("[data-current]").hidden = !change;
    password.querySelector("[data-repeat]").hidden = !setup;
    text(password.querySelector("[data-label]"), setup ? "新聊天密碼" : "聊天密碼");
    text($("private-chat-password-title"), change ? "變更聊天密碼" : setup ? "設定聊天密碼" : "解鎖私聊");
    secret.autocomplete = setup ? "new-password" : "current-password";
    if (!password.open) password.showModal();
  }

  function closePassword() {
    form.reset();
    text(passwordStatus, "");
    if (password.open) password.close();
  }

  for (const input of [old, secret, repeated]) {
    input.addEventListener("input", () => text(passwordStatus, input.value ? passwordProblem(input.value) : ""));
  }
  form.onsubmit = event => {
    event.preventDefault();
    const change = passwordAction === "change_password";
    const confirm = !password.querySelector("[data-repeat]").hidden;
    const invalid = (change && passwordProblem(old.value)) || passwordProblem(secret.value)
      || (confirm && secret.value !== repeated.value ? "兩次輸入的密碼不同。" : "");
    if (invalid) { text(passwordStatus, invalid); return; }
    if (change) request("change_password", {old:old.value, new:secret.value});
    else request("unlock", {password:secret.value});
    form.reset();
    if (change) closePassword();
  };
  function cancelPassword() {
    if (passwordAction === "unlock") request("leave");
    closePassword();
  }
  password.querySelector("[data-cancel]").onclick = cancelPassword;
  password.addEventListener("cancel", event => { event.preventDefault(); cancelPassword(); });

  function shortPerson(profile) { return profile.nickname + " · " + profile.employee_no; }
  function button(label, action, ariaLabel) {
    const result = document.createElement("button");
    result.type = "button";
    text(result, label);
    if (ariaLabel) result.setAttribute("aria-label", ariaLabel);
    result.onclick = action;
    result.disabled = Boolean(current.busy);
    return result;
  }
  function row(label, ...actions) {
    const result = document.createElement("div");
    result.className = "private-chat-person";
    const name = document.createElement("span");
    text(name, label);
    result.append(name, ...actions);
    return result;
  }

  function renderThread(friend) {
    text(inPane("[data-friend-title]"), friend ? shortPerson(friend) : "私聊");
    text(inPane("[data-thread-state]"), friend ? labels[friend.status] ?? "目前無法聊天" : "從左側選擇好友。");
    const remove = inPane("[data-remove-friend]");
    remove.hidden = !friend || friend.status === "removed";
    remove.disabled = Boolean(current.busy);
    remove.onclick = friend ? () => askInChat("解除好友", "確定要解除此好友？", () => request("remove_friend", {id:friend.friend_id})) : null;
    messages.renderFriend(friend, current.busy);
  }

  function renderPeople(snapshot) {
    const profile = snapshot.profile;
    text(inContacts("[data-id]"), profile?.chat_id);
    inContacts("[data-copy]").disabled = inContacts("[data-regenerate]").disabled = current.busy || !profile;
    const friends = snapshot.friends || [];
    const list = inPane("[data-friends]");
    list.replaceChildren();
    for (const friend of friends) {
      const item = button(shortPerson(friend) + (friend.status === "active" ? "" : " · " + (labels[friend.status] || "暫不可用")), () => {
        if (selected === friend.friend_id) return;
        selected = friend.friend_id;
        renderPeople(snapshot);
        messages.select(selected);
      });
      item.disabled = false;
      item.setAttribute("aria-pressed", String(selected === friend.friend_id));
      list.append(item);
    }
    renderThread(friends.find(friend => friend.friend_id === selected));
    inContacts("[data-incoming]").replaceChildren(...(snapshot.invites?.incoming || []).map(invite => {
      const answer = response => request("answer", {id:invite.invite_id,response});
      return row(shortPerson(invite), button("✓", () => answer("accept"), "接受"),
        button("✗", () => answer("reject"), "拒絕"), button("👤", () => answer("block"), "加入黑名單"));
    }));
    inContacts("[data-outgoing]").replaceChildren(...(snapshot.invites?.outgoing || []).map(invite =>
      row(shortPerson(invite) + (invite.status ? " · " + invite.status : ""))
    ));
    inContacts("[data-blocked]").replaceChildren(...(snapshot.blocked || []).map(entry =>
      row(shortPerson(entry), button("解除黑名單", () => request("unblock", {employee_no:entry.employee_no})))
    ));
    const found = inContacts("[data-found]");
    found.hidden = inContacts("[data-invite]").hidden = !current.lookup;
    inContacts("[data-invite]").disabled = Boolean(current.busy);
    text(found, current.lookup ? shortPerson(current.lookup) : "");
  }

  function clearScreen() {
    const confirmation = $("confirm-dialog");
    if (pane.open && confirmation.open && confirmation.dataset.privateChat) {
      confirmation.close();
      delete confirmation.dataset.privateChat;
    }
    if (contacts.open) contacts.close();
    inContacts("[data-lookup]").reset();
    if (pane.open) pane.close();
    selected = null;
    messages.clear();
    renderPeople({});
    text(inPane("[data-pane-status]"), "");
    text(inContacts("[data-contacts-status]"), "");
  }

  function receive(message) {
    if (message.type !== "private_chat_state") {
      if (pane.open && current.enabled) messages.receive(message);
      return;
    }
    current = message;
    renderAvailability(state);
    text($("private-chat-settings-status"), message.status);
    if (!message.open) clearScreen();
    if (message.authenticate && message.enabled) {
      showPassword("unlock", message.has_password);
      text(passwordStatus, message.status);
    } else if (password.open && (passwordAction === "unlock" || !message.enabled)) closePassword();
    password.querySelector("[data-submit]").disabled = Boolean(message.busy);
    if (message.open && message.enabled) {
      if (!pane.open) pane.showModal();
      text(inPane("[data-pane-status]"), message.status);
      text(inContacts("[data-contacts-status]"), message.status);
      renderPeople(message.contacts || {});
    }
  }

  const when = $("private-chat-lock-when");
  const minutes = $("private-chat-lock-minutes");
  function setMinutes(mode, value) {
    minutes.replaceChildren(...delays[mode].map(delay => {
      const option = document.createElement("option");
      option.value = delay;
      text(option, delay === 0 ? "立即" : delay < 60 ? delay + " 分鐘" : delay === 1440 ? "1 天" : delay / 60 + " 小時");
      return option;
    }));
    minutes.value = String(value);
  }
  function savePolicy() {
    request("policy", {policy:{when:when.value, minutes:Number(minutes.value), windows_lock:$("private-chat-windows-lock").checked}});
  }
  when.onchange = () => { setMinutes(when.value, delays[when.value][0]); savePolicy(); };
  minutes.onchange = $("private-chat-windows-lock").onchange = savePolicy;
  function renderAvailability(appState) {
    const available = Boolean(appState.logged_in && appState.config.extra_features_enabled
      && appState.private_chat_available && current.available !== false);
    const enabled = available && Boolean(current.enabled);
    $("private-chat-settings").hidden = !available;
    $("private-chat-enabled").checked = enabled;
    $("private-chat-enabled").disabled = Boolean(current.busy);
    $("private-chat-lock-settings").hidden = !enabled;
    $("show-private-chat").hidden = !enabled;
    $("private-chat-notices").hidden = !enabled;
    inPane("[data-vnc]").hidden = !appState.config.vnc_enabled || !featureEnabled("vnc");
    const policy = appState.config.private_chat_lock || defaultPolicy;
    when.value = policy.when;
    setMinutes(policy.when, policy.minutes);
    $("private-chat-windows-lock").checked = policy.windows_lock;
    if (!available && (pane.open || contacts.open || password.open)) {
      current = {};
      clearScreen();
      closePassword();
    }
  }

  $("private-chat-enabled").onchange = () => {
    if (!$("private-chat-enabled").checked) { request("enable", {enabled:false}); return; }
    $("private-chat-enabled").checked = false;
    ask("在這台電腦啟用私聊", "每個帳號同時只能有一台私聊裝置。在這裡啟用後，原裝置會停止接收私聊。", () => request("enable", {enabled:true}));
  };
  $("show-private-chat").onclick = () => { $("advanced-dialog")?.close(); request("open"); };
  $("private-chat-password-change").onclick = () => showPassword("change_password", true);
  $("private-chat-reset").onclick = () => ask("重設聊天", "本機所有聊天紀錄將永久無法讀取。確定要重設？", () => request("reset", {confirmed:true}));
  $("private-chat-erase").onclick = () => ask("清除本機聊天資料", "本機所有聊天紀錄將永久無法讀取。確定要清除？", () => request("erase", {confirmed:true}));
  inPane("[data-return]").onclick = () => request("leave");
  inPane("[data-lock]").onclick = () => request("lock");
  inPane("[data-advanced]").onclick = () => $("show-advanced").click();
  inPane("[data-vnc]").onclick = () => $("show-vnc").click();
  inPane("[data-add]").onclick = () => { if (!contacts.open) contacts.showModal(); request("refresh"); };
  pane.addEventListener("cancel", event => { event.preventDefault(); request("leave"); });
  inContacts("[data-close]").onclick = () => contacts.close();
  inContacts("[data-copy]").onclick = () => request("copy_id");
  inContacts("[data-regenerate]").onclick = () => askInChat("重新產生聊天 ID", "舊 ID 立即失效，既有好友不受影響。", () => request("regenerate"));
  inContacts("[data-lookup]").onsubmit = event => {
    event.preventDefault();
    request("lookup", {code:event.currentTarget.elements.namedItem("code").value});
  };
  inContacts("[data-invite]").onclick = () => request("invite");
  return {receive, renderAvailability};
})();
