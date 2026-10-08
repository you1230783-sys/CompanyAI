/* 設定頁只切換可見區塊，不重建輸入控制項，保留尚未套用的快捷鍵等草稿。 */
"use strict";
window.SettingsUI = (() => {
  const buttons = [...document.querySelectorAll(".settings-tabs [role=tab]")];
  function select(index, focus = false) {
    buttons.forEach((button, position) => {
      const active = position === index;
      button.setAttribute("aria-selected", String(active));
      button.tabIndex = active ? 0 : -1;
      $(button.getAttribute("aria-controls")).hidden = !active;
    });
    document.querySelector(".settings-content").scrollTop = 0;
    if (focus) buttons[index].focus();
  }
  buttons.forEach((button, index) => {
    button.onclick = () => select(index);
    button.onkeydown = event => {
      const next = {ArrowRight:(index+1)%3, ArrowLeft:(index+2)%3, Home:0, End:2}[event.key];
      if (next !== undefined) { event.preventDefault(); select(next, true); }
    };
  });
  $("diagnostics-export").onclick = () => {
    $("diagnostics-export-status").textContent = "請選擇儲存位置。";
    send({type:"export_diagnostics", include_details:$("diagnostics-include-details").checked});
  };
  function receive(message) {
    if (message.type === "diagnostics_exported") $("diagnostics-export-status").textContent = message.text;
  }
  return {select, receive};
})();
