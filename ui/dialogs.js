/* 所有 HTML dialog 共用外部點擊關閉；未來新增 dialog 也自動套用。 */
"use strict";
(() => {
  const bound = new WeakSet();
  function bind(dialog) {
    if (bound.has(dialog)) return;
    bound.add(dialog);
    let backdropDown = false;
    const outside = event => {
      const box = dialog.getBoundingClientRect();
      return event.target === dialog && (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom);
    };
    dialog.addEventListener("pointerdown", event => {
      backdropDown = (event.button ?? 0) === 0 && outside(event);
    });
    dialog.addEventListener("pointercancel", () => { backdropDown = false; });
    dialog.addEventListener("close", () => { backdropDown = false; });
    dialog.addEventListener("click", event => {
      if (dialog.open && backdropDown && outside(event)) {
        // 先通知各視窗取消待確認動作，再關閉；不模擬按下「確認」或「儲存」。
        dialog.dispatchEvent(new Event("backdrop-dismiss"));
        dialog.close();
      }
      backdropDown = false;
    });
  }
  const bindAll = () => document.querySelectorAll("dialog").forEach(bind);
  bindAll();
  new MutationObserver(bindAll).observe(document.body, { childList: true, subtree: true });
})();
