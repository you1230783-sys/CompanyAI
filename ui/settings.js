/* 設定頁只切換可見區塊，不重建輸入控制項，保留尚未套用的快捷鍵等草稿。 */
"use strict";
window.SettingsUI = (() => {
  let paletteDirty = false;
  const colorInputs = ChartAppearance.defaults().map((color, index) => {
    const label=document.createElement("label"), input=document.createElement("input"), name=document.createElement("span");
    name.textContent=`系列 ${index+1}`;input.type="color";input.value=color;
    input.oninput=()=>{paletteDirty=true;};label.append(name,input);$("chart-palette").append(label);return input;
  });
  $("chart-palette-save").onclick=()=>{paletteDirty=false;send({type:"chart_palette",colors:colorInputs.map(input=>input.value)});};
  $("chart-palette-reset").onclick=()=>{colorInputs.forEach((input,i)=>input.value=ChartAppearance.defaults()[i]);paletteDirty=true;};
  function render(config) {
    ChartAppearance.set(config.chart_palette);
    if(!paletteDirty) colorInputs.forEach((input,i)=>input.value=ChartAppearance.palette()[i]);
  }
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
  return {select, receive, render};
})();
