/* 成果中心只傳索引ID；打包位置及檔案範圍均由原生端決定。 */
"use strict";
window.ArtifactUI=(()=>{
  let conversation=null,offset=0,next=0,total=0;
  const $=id=>document.getElementById(id);
  const selected=new Set();
  function request(exportIds=[]){
    if(!conversation)return;
    $("project-artifact-status").textContent=exportIds.length?"正在校驗並打包成果…":"正在核對成果版本…";
    send({type:"project",command:{action:"artifact_center",conversation,offset,export_ids:exportIds}});
  }
  $("project-artifacts").onclick=()=>{conversation=state.active_id;offset=0;selected.clear();$("project-artifact-dialog").showModal();request();};
  $("project-artifact-close").onclick=()=>$("project-artifact-dialog").close();
  $("project-artifact-dialog").addEventListener("close",()=>{conversation=null;selected.clear();$("project-artifact-list").replaceChildren();$("project-workflow-list").replaceChildren();});
  $("project-artifact-refresh").onclick=()=>{offset=0;request();};
  $("project-artifact-more").onclick=()=>{if(next<total){offset=next;request();}};
  $("project-artifact-export").onclick=()=>{if(!selected.size){toast("請先勾選成果");return;}request([...selected]);};
  function receive(message){
    if(message.type!=="project_artifact_center")return;
    if(!conversation||conversation!==state.active_id||conversation!==message.conversation||!$("project-artifact-dialog").open)return;
    if(!message.ok){$("project-artifact-status").textContent=message.text;return;}
    let data;try{data=JSON.parse(message.text);}catch{$("project-artifact-status").textContent="成果紀錄無法解析";return;}
    if(data.exported){$("project-artifact-status").textContent=`已打包：${data.exported}`;return;}
    if(data.offset!==offset)return;
    next=data.next_offset;total=data.total;
    $("project-artifact-status").textContent=`顯示第 ${total?data.offset+1:0}–${next} 份，共 ${total} 份。已勾選 ${selected.size} 份。`;
    $("project-artifact-more").disabled=next>=total;
    const list=$("project-artifact-list");list.replaceChildren();
    for(const row of data.artifacts||[]){
      const e=row.entry,card=document.createElement("section"),label=document.createElement("label"),check=document.createElement("input");check.type="checkbox";check.checked=selected.has(e.id);check.disabled=row.artifact_status!=="verified";
      check.onchange=()=>{if(check.checked)selected.add(e.id);else selected.delete(e.id);};
      label.append(check,document.createTextNode(` ${e.path}`));card.append(label);
      const detail=document.createElement("p");
      const status={verified:"成果版本相符",changed:"成果已變更",unavailable:"成果無法讀取",unknown:"來源待確認",unchanged:"來源版本相符",needs_update:"來源有變更，成果可能需更新"};
      detail.textContent=`${status[row.artifact_status]} · ${status[row.source_status]} · ${new Date(e.created*1000).toLocaleString()} · 版本 ${e.sha256.slice(0,12)}`;card.append(detail);
      const source=document.createElement("details"),summary=document.createElement("summary"),pre=document.createElement("pre");summary.textContent="來源與版本";pre.textContent=JSON.stringify({sources:e.sources,omitted:row.sources_omitted,changed_sources:row.changed_sources,run:e.run,conversation:e.conversation,scope:row.notice},null,2);source.append(summary,pre);card.append(source);list.append(card);
    }
    const flows=$("project-workflow-list");flows.replaceChildren();
    for(const flow of data.workflows||[]){
      const button=document.createElement("button");button.textContent=`${flow.name}（${flow.steps} 步）`;
      button.onclick=()=>{const input=$("prompt");const text=`請重跑工作流程「${flow.name}」（workflow_id=${flow.id}）。先載入notes，檢視範本，確認這次來源、日期與參數後逐步執行。\n本次資料與參數：\n`;input.value=input.value?`${input.value}\n${text}`:text;$("project-artifact-dialog").close();input.dispatchEvent(new Event("input",{bubbles:true}));input.focus();};flows.append(button);
    }
  }
  function sync(){const active=state.logged_in&&state.conversations.some(c=>c.id===state.active_id&&c.project_id);$("project-artifacts").hidden=!active;if(conversation&&(!active||conversation!==state.active_id))$("project-artifact-dialog").close();const source=$("diagram-source-dialog");if(source.open&&(!active||source.dataset.conversation!==state.active_id))source.close();}
  return {receive,sync};
})();
