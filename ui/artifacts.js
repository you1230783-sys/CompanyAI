/* 成果中心只傳索引ID；打包及PNG預覽均由原生端核對路徑與版本。 */
"use strict";
window.ArtifactUI = (() => {
  let conversation=null,offset=0,next=0,total=0;
  let requestId=null,serial=0;
  const disabledBeforeRequest=new Map();
  function busy(value){
    if(value){
      for(const control of $("project-artifact-dialog").querySelectorAll("button,input")){
        if(control.id==="project-artifact-close")continue;
        if(!disabledBeforeRequest.has(control))disabledBeforeRequest.set(control,control.disabled);
        control.disabled=true;
      }
    } else {
      for(const [control,disabled] of disabledBeforeRequest)control.disabled=disabled;
      disabledBeforeRequest.clear();
    }
  }
  const $=id=>document.getElementById(id), selected=new Set(), urls=new Set();
  const make=(tag,text="")=>{const element=document.createElement(tag);element.textContent=text;return element;};
  const labels={verified:"成果版本相符",changed:"成果已變更",unavailable:"成果無法讀取",unknown:"來源待確認",unchanged:"來源版本相符",needs_update:"來源有變更，成果可能需更新"};
  function releasePreviews(){for(const url of urls)URL.revokeObjectURL(url);urls.clear();}
  function request(exportIds=[]){
    if(!conversation)return;
    $("project-artifact-status").textContent=exportIds.length?"正在校驗並打包成果…":"正在核對成果版本…";
    requestId=`artifacts-${++serial}`;
    busy(true);
    send({type:"project",command:{action:"artifact_center",conversation,request_id:requestId,offset,export_ids:exportIds}});
  }
  function action(id,label=null){
    $("project-artifact-status").textContent=label===null?"正在核對預覽圖片…":"正在保存採用版本…";
    requestId=`artifacts-${++serial}`;
    busy(true);
    send({type:"project",command:{action:"artifact_action",conversation,request_id:requestId,id,label}});
  }
  $("project-artifacts").onclick=()=>{conversation=state.active_id;offset=0;selected.clear();$("project-artifact-dialog").showModal();request();};
  $("project-artifact-close").onclick=()=>$("project-artifact-dialog").close();
  $("project-artifact-dialog").addEventListener("close",()=>{
    conversation=null;busy(false);selected.clear();releasePreviews();$("project-artifact-list").replaceChildren();$("project-workflow-list").replaceChildren();
  });
  $("project-artifact-refresh").onclick=()=>{offset=0;request();};
  $("project-artifact-more").onclick=()=>{if(next<total){offset=next;request();}};
  $("project-artifact-export").onclick=()=>{if(!selected.size){toast("請先勾選成果");return;}request([...selected]);};

  function preview(data){
    const card=[...$("project-artifact-list").querySelectorAll("[data-artifact-id]")].find(e=>e.dataset.artifactId===data.preview_id);
    if(!card)return;
    if(typeof data.hex!=="string"||data.hex.length>4*1024*1024||data.hex.length%2||!/^[0-9a-f]+$/i.test(data.hex))return;
    const bytes=Uint8Array.from(data.hex.match(/../g),byte=>parseInt(byte,16));
    const url=URL.createObjectURL(new Blob([bytes],{type:"image/png"}));urls.add(url);
    const old=card.querySelector("img");if(old){URL.revokeObjectURL(old.src);urls.delete(old.src);old.remove();}
    const image=make("img");image.src=url;image.alt=data.path;image.className="artifact-thumbnail";card.append(image);
    $("project-artifact-status").textContent="圖片版本已核對；預覽不代表圖表內容或版面已驗收。";
  }
  function artifact(row,selections){
    const entry=row.entry,card=make("section"),label=make("label"),check=make("input");
    card.dataset.artifactId=entry.id;check.type="checkbox";check.checked=selected.has(entry.id);check.disabled=row.artifact_status!=="verified";
    if(check.disabled)selected.delete(entry.id);
    check.onchange=()=>{if(check.checked)selected.add(entry.id);else selected.delete(entry.id);};
    label.append(check,document.createTextNode(` ${entry.path}`));card.append(label);
    card.append(make("p",`${labels[row.artifact_status]} · ${labels[row.source_status]} · ${new Date(entry.created*1000).toLocaleString()} · 版本 ${entry.sha256.slice(0,12)}`));
    const adopted=Object.entries(selections).filter(([,id])=>id===entry.id).map(([name])=>name);
    if(adopted.length)card.append(make("strong",`目前採用：${adopted.join("、")}`));
    const actions=make("div"), purpose=make("input"), adopt=make("button","設為採用版本");
    purpose.maxLength=80;purpose.placeholder="用途，例如：本週Word報告";purpose.setAttribute("aria-label",`${entry.path} 的採用用途`);
    adopt.disabled=row.artifact_status!=="verified";
    adopt.onclick=()=>{if(!purpose.value.trim()){purpose.focus();toast("請填寫用途；相同用途會改採用這個版本");return;}action(entry.id,purpose.value.trim());};
    actions.append(purpose,adopt);
    if(/\.png$/i.test(entry.path)){
      const show=make("button","預覽圖片");show.disabled=row.artifact_status!=="verified";show.onclick=()=>action(entry.id);actions.append(show);
    }
    card.append(actions);
    const source=make("details"),summary=make("summary","來源與版本"),pre=make("pre",JSON.stringify({sources:entry.sources,omitted:row.sources_omitted,changed_sources:row.changed_sources,run:entry.run,conversation:entry.conversation,scope:row.notice},null,2));
    source.append(summary,pre);card.append(source);return card;
  }
  function receive(message){
    if(message.type!=="project_artifact_center"||message.request_id!==requestId||!conversation||conversation!==state.active_id||conversation!==message.conversation||!$("project-artifact-dialog").open)return;
    busy(false);
    if(!message.ok){$("project-artifact-status").textContent=message.text;return;}
    let data;try{data=JSON.parse(message.text);}catch{$("project-artifact-status").textContent="成果紀錄無法解析";return;}
    if(data.preview_id){preview(data);return;}
    if(data.selection_saved){request();return;}
    if(data.exported){$("project-artifact-status").textContent=`已打包：${data.exported}`;return;}
    if(data.offset!==offset)return;
    next=data.next_offset;total=data.total;
    $("project-artifact-status").textContent=`顯示第 ${total?data.offset+1:0}–${next} 份，共 ${total} 份。已勾選 ${selected.size} 份。`;
    $("project-artifact-more").disabled=next>=total;
    const list=$("project-artifact-list");releasePreviews();list.replaceChildren();
    const selections=data.selected_versions||{};
    if(Object.keys(selections).length){
      const adopted=make("details"),title=make("summary","目前採用版本");adopted.append(title);
      for(const [name,id] of Object.entries(selections)){
        const detail=(data.selected_details||[]).find(item=>item.id===id);
        const line=make("p",`${name} · ${detail?.path||"索引已不存在"} · 登記版本 ${(detail?.sha256||id).slice(0,12)}`),clear=make("button","取消採用");clear.onclick=()=>action("",name);line.append(clear);adopted.append(line);
      }
      list.append(adopted);
    }
    const grouped=new Map();
    for(const row of data.artifacts||[]){const key=row.entry.run;if(!grouped.has(key))grouped.set(key,[]);grouped.get(key).push(row);}
    const runs=new Map((data.runs||[]).map(run=>[run.id,run]));
    // 還沒保存檔案，也列出有交付清單的最近任務。
    for(const run of data.runs||[])if(run.delivery?.items?.length&&!grouped.has(run.id))grouped.set(run.id,[]);
    for(const [id,rows] of grouped){
      const group=make("section");group.className="artifact-group";
      const run=runs.get(id);group.append(make("h3",run?.request||`任務 ${id.slice(0,12)}`));
      if(run)group.append(make("small",`${run.state} · ${new Date(run.updated*1000).toLocaleString()}`));
      window.WorkbenchUI?.delivery(group,run?.delivery);
      for(const row of rows)group.append(artifact(row,selections));list.append(group);
    }
    list.append(make("small",data.runs_notice||""));
    const flows=$("project-workflow-list");flows.replaceChildren();
    for(const flow of data.workflows||[]){
      const button=make("button",`${flow.name}（${flow.steps} 步）`);
      button.onclick=()=>window.WorkbenchUI?.workflow(flow,conversation);flows.append(button);
    }
  }
  function sync(){
    const active=state.logged_in&&state.conversations.some(c=>c.id===state.active_id&&c.project_id);$("project-artifacts").hidden=!active;
    if(conversation&&(!active||conversation!==state.active_id))$("project-artifact-dialog").close();
    const source=$("diagram-source-dialog");if(source.open&&(!active||source.dataset.conversation!==state.active_id))source.close();
  }
  return {receive,sync};
})();
