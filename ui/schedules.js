/* 固定流程排程：原生產生預覽，明確確認後才保存授權；所有內容以文字呈現。 */
"use strict";
window.ScheduleUI = (() => {
  let form = null, previewId = null, pending = false, confirmButton = null, previewBox = null, formStatus = null;
  let list = null, listBody = null;
  let previewRequestId = null;
  const make = (tag, text) => { const node=document.createElement(tag); if(text!=null)node.textContent=text; return node; };
  const sendCommand = command => send({type:"schedule",command});
  function dialog(title) {
    const node=make("dialog");node.className="pet-dialog";
    const header=make("div");header.className="tarot-heading";
    const close=make("button","關閉");close.onclick=()=>node.close();
    header.append(make("h2",title),close);node.append(header);document.body.append(node);
    return node;
  }
  function field(parent,title,control) {const label=make("label",title);label.append(control);parent.append(label);return control;}
  function open(flow,conversation,parameters) {
    if(form)form.remove();
    previewId=null;pending=false;form=dialog("設定固定流程排程");form.id="schedule-create";
    form.append(make("p",`流程：${flow.name}。只在程式開啟／托盤、登入有效且沒有其他工作時執行。超過到期時間5分鐘不補跑；不會啟動AI改寫流程。`));
    const due=make("input");due.type="datetime-local";
    const now=new Date(Date.now()+600000);due.value=new Date(now-now.getTimezoneOffset()*60000).toISOString().slice(0,16);
    field(form,"首次時間（目前Windows時區）",due);
    const repeat=make("select");for(const [value,text] of [[0,"單次"],[1,"每24小時"],[7,"每7天"]]){const option=make("option",text);option.value=value;repeat.append(option);}
    field(form,"重複間隔（固定時間差，非工作日／夏令時間日曆）",repeat);
    form.append(make("pre",JSON.stringify(parameters,null,2)));
    const preview=make("button","預覽固定步驟與權限");preview.id="schedule-preview";
    confirmButton=make("button","確認授權並保存排程");confirmButton.id="schedule-confirm";confirmButton.disabled=true;
    previewBox=make("pre");previewBox.id="schedule-plan";
    formStatus=make("p");formStatus.setAttribute("role","status");
    const invalidate=()=>{previewId=null;confirmButton.disabled=true;previewBox.textContent="時間已變更，請重新預覽。";};
    due.oninput=invalidate;repeat.onchange=invalidate;
    preview.onclick=()=>{
      const time=Math.floor(new Date(due.value).getTime()/1000);
      if(!Number.isFinite(time)||time<=Date.now()/1000){formStatus.textContent="請選未來時間。";return;}
      pending=true;previewId=null;confirmButton.disabled=true;preview.disabled=true;due.disabled=true;repeat.disabled=true;
      formStatus.textContent="正在由原生核對範本…";
      previewRequestId=crypto.randomUUID();
      sendCommand({action:"preview",request_id:previewRequestId,conversation,template:flow.id,parameters,due_at:time,repeat_days:Number(repeat.value)});
    };
    form.addEventListener("close",()=>{previewId=null;pending=false;});
    confirmButton.onclick=()=>{if(!previewId)return;const id=previewId;previewId=null;confirmButton.disabled=true;sendCommand({action:"confirm",preview_id:id});};
    form.append(preview,confirmButton,previewBox,formStatus);form.showModal();
  }
  function showList() {
    if(!list){list=dialog("流程排程與執行審計");list.id="schedule-list";listBody=make("div");list.append(make("p","新增排程：從專案成果中心的已保存流程，填入參數後選擇「設定排程」。最多50項排程、100次執行審計；到上限停止新增執行，保留紀錄供維護者封存。"),listBody);}
    if(!list.open)list.showModal();sendCommand({action:"state"});
  }
  function action(parent,text,command) {const b=make("button",text);b.onclick=()=>sendCommand(command);parent.append(b);}
  function receive(event) {
    if(event.type==="schedule_preview" && form?.open && pending && event.request_id===previewRequestId) {
      pending=false;previewId=event.id;
      form.querySelectorAll("input,select,#schedule-preview").forEach(control=>control.disabled=false);
      confirmButton.disabled=false;
      previewBox.textContent=`${event.notice}\n專案根目錄：${event.root}\n核准資料與完整固定步驟：\n${JSON.stringify(event.grant,null,2)}`;
      formStatus.textContent="請核對參數、來源、輸出名稱、程式碼與每一步，再確認。預覽有效2分鐘。";
    } else if(event.type==="schedule_error") {
      pending=false;previewId=null;if(confirmButton)confirmButton.disabled=true;
      if(formStatus)formStatus.textContent=event.message;
      form?.querySelectorAll("input,select,#schedule-preview").forEach(control=>control.disabled=false);
      if(listBody){const error=make("p",event.message);error.setAttribute("role","alert");listBody.prepend(error);}
    } else if(event.type==="schedule_state") {
      if(form?.open && !previewId && !pending){formStatus.textContent="排程狀態已更新。";}
      if(!listBody)return;
      listBody.replaceChildren();
      if(event.running)action(listBody,"停止目前排程",{action:"stop"});
      if(!event.entries?.length)listBody.append(make("p","目前帳號沒有排程。"));
      for(const entry of event.entries||[]) {
        const card=make("section");card.append(make("h3",entry.grant.template.name),make("p",`${entry.status}；${entry.enabled?"啟用":"停用"}；下次：${entry.next_due?new Date(entry.next_due*1000).toLocaleString():"無"}`));
        action(card,"停用並停止",{action:"disable",id:entry.id});
        const audits=(event.audit||[]).filter(a=>a.schedule===entry.id).reverse();
        if(!event.running && entry.execution && audits.some(a=>a.execution===entry.execution&&a.status==="已暫停，可從未執行步驟續接")) {
          action(card,"核對後續接尚未執行步驟",{action:"resume",id:entry.id});
        }
        for(const audit of audits){const details=make("details");details.append(make("summary",`${new Date(audit.started_at*1000).toLocaleString()}：${audit.status}，已通過${audit.next_step}步`),make("pre",JSON.stringify(audit,null,2)));card.append(details);}
        listBody.append(card);
      }
    }
  }
  $("show-schedules").onclick=showList;
  return {open,showList,receive};
})();
