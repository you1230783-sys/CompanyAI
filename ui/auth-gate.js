/* 登入鎖定集中處理，避免各功能重繪或動態新增按鈕後漏掉停用狀態。 */
"use strict";
(() => {
  const loginButtons = new Set(["login-entry", "login-entry-cancel", "login-entry-reopen"]);
  const publicCommands = new Set(["ready", "login", "cancel_login", "reopen_login", "exit", "self_test_result"]);
  const controls = "button,input,select,textarea,a,summary,[contenteditable='true']";
  const locked = new Map();
  const observer = new MutationObserver(() => {
    if (!state.logged_in) lockControls();
  });

  function lockControls() {
    for (const element of document.querySelectorAll(controls)) {
      if (loginButtons.has(element.id)) continue;
      const property = "disabled" in element ? "disabled" : "inert";
      if (!locked.has(element)) locked.set(element, { property, value: element[property] });
      if (!element[property]) element[property] = true;
    }
  }

  function beforeRender() {
    observer.disconnect();
    // 恢復各功能原本的限制，再讓正常 render 依忙碌／附件等狀態重新決定。
    for (const [element, previous] of locked) element[previous.property] = previous.value;
    locked.clear();
  }

  function apply() {
    const loggedIn = !!state.logged_in;
    $("login-panel").hidden = loggedIn;
    $("login-entry").disabled = state.busy !== "none";
    $("login-entry-cancel").hidden = state.busy !== "login";
    $("login-entry-reopen").hidden = state.busy !== "login" || !state.login_code;
    $("login-entry-code").textContent = state.login_code ? "請核對登入碼：" + state.login_code : "";
    if (loggedIn) return;
    // 授權到期／登出時移除遮住登入入口的對話框，不允許在舊 modal 繼續操作。
    for (const dialog of document.querySelectorAll("dialog[open]")) dialog.close();
    $("model-menu").hidden = true;
    lockControls();
    observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["disabled", "inert"] });
  }

  // disabled 配合事件攔截，涵蓋鍵盤、連結、表單以及動態控制項尚未鎖定的同一事件循環。
  for (const type of ["click", "input", "change", "submit", "keydown"]) {
    document.addEventListener(type, event => {
      if (state.logged_in) return;
      if (type === "keydown" && event.key === "Tab") return;
      const control = event.target.closest?.(controls);
      if (control && loginButtons.has(control.id)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
    }, true);
  }
  $("login-entry").onclick = () => send({ type: "login" });
  $("login-entry-cancel").onclick = () => send({ type: "cancel_login" });
  $("login-entry-reopen").onclick = () => send({ type: "reopen_login" });
  window.AuthUI = {
    beforeRender, apply,
    canSend: command => !!state.logged_in || publicCommands.has(command.type),
  };
  apply();
})();
