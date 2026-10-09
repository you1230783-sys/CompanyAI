/* 圖中文字只保存結構化樣式；取消對話框不改排版草稿，完成排版才送原生保存。 */
"use strict";
window.ChartText = (() => {
  const fonts={"sans-serif":"系統預設","Microsoft JhengHei":"微軟正黑體","PMingLiU":"新細明體","DFKai-SB":"標楷體",Arial:"Arial","Times New Roman":"Times New Roman",Consolas:"Consolas"};
  const defaults=()=>({text:"區間1",position:{x:.4,y:.35},font_family:"Microsoft JhengHei",font_size:18,bold:false,italic:false,underline:false,color:"#333333",background:null});
  function open(current, apply, remove) {
    const initial=structuredClone(current || defaults());
    const dialog=document.createElement("dialog"),form=document.createElement("form"),heading=document.createElement("h2"),grid=document.createElement("div");
    dialog.className="chart-text-dialog";dialog.setAttribute("aria-label","圖中文字");heading.textContent=current?"編輯圖中文字":"新增圖中文字";
    grid.className="chart-edit-grid";form.noValidate=true;
    function field(label,key,type,value) {
      const wrap=document.createElement("label"),caption=document.createElement("span");caption.textContent=label;
      const input=document.createElement(type==="textarea"?"textarea":type==="select"?"select":"input");input.dataset.textField=key;
      if(type!=="textarea"&&type!=="select")input.type=type;
      if(type==="checkbox")input.checked=!!value;else input.value=value??"";
      wrap.append(caption,input);grid.append(wrap);return input;
    }
    const text=field("文字（可換行，最多500字）","text","textarea",initial.text);text.maxLength=500;text.rows=3;text.parentElement.className="chart-text-content";
    const font=field("字型","font_family","select",initial.font_family);
    for(const [value,label] of Object.entries(fonts)){const option=document.createElement("option");option.value=value;option.textContent=label;font.append(option);}font.value=initial.font_family;
    const size=field("字級（8–72）","font_size","number",initial.font_size);size.min=8;size.max=72;size.step=1;
    const bold=field("粗體","bold","checkbox",initial.bold),italic=field("斜體","italic","checkbox",initial.italic),underline=field("底線","underline","checkbox",initial.underline);
    const color=field("字色","color","color",initial.color),backgroundEnabled=field("使用底色","background_enabled","checkbox",!!initial.background),background=field("底色","background","color",initial.background||"#ffffff");
    background.disabled=!backgroundEnabled.checked;backgroundEnabled.onchange=()=>background.disabled=!backgroundEnabled.checked;
    const x=field("水平位置（%）","x","number",Number((initial.position.x*100).toFixed(2))),y=field("垂直位置（%）","y","number",Number((initial.position.y*100).toFixed(2)));
    for(const input of [x,y]){input.min=0;input.max=100;input.step=.1;}
    const hint=document.createElement("p");hint.textContent="位置從圖框左上角計算，也可套用後直接拖曳；文字會避讓其他標籤。未安裝的字型由系統替代。";
    const error=document.createElement("p");error.className="error";error.setAttribute("role","alert");
    const actions=document.createElement("div");actions.className="dialog-actions";
    const cancel=document.createElement("button"),save=document.createElement("button");cancel.type="button";cancel.textContent="取消";save.type="submit";save.className="primary";save.textContent="套用文字";actions.append(cancel,save);
    const close=()=>{dialog.close();dialog.remove();};cancel.onclick=close;dialog.oncancel=event=>{event.preventDefault();close();};
    if(current && remove) {
      const button=document.createElement("button");button.type="button";button.textContent="刪除文字";button.className="danger-button";
      button.onclick=()=>{remove();close();};actions.prepend(button);
    }
    form.onsubmit=event=>{event.preventDefault();
      if(!text.value.trim()||[...text.value].length>500){error.textContent="請輸入1–500字的文字。";text.focus();return;}
      if(!size.value||!Number.isInteger(Number(size.value))||size.value<8||size.value>72){error.textContent="字級請填8–72的整數。";size.focus();return;}
      if([x,y].some(input=>!input.value.trim()||!Number.isFinite(Number(input.value))||input.value<0||input.value>100)){error.textContent="位置請填0–100%。";return;}
      apply({text:text.value,position:{x:Number(x.value)/100,y:Number(y.value)/100},font_family:font.value,font_size:Number(size.value),bold:bold.checked,italic:italic.checked,underline:underline.checked,color:color.value,background:backgroundEnabled.checked?background.value:null});close();
    };
    form.append(heading,grid,hint,error,actions);dialog.append(form);document.body.append(dialog);dialog.showModal();text.focus();text.select();
  }
  return {open,defaults};
})();
