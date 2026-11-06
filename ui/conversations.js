/* 對話交接、來源導覽及原生任務總覽；所有外部文字均以 textContent/value 顯示。 */
"use strict";
(() => {
  let pending=null, request=null, origin=null;
  const command=value=>send({type:"handoff",command:value});
  const dialog=document.createElement("dialog");dialog.id="handoff-dialog";
  dialog.innerHTML=`<div class="dialog-heading"><h2>帶入專案繼續</h2><button data-close>關閉</button></div>
    <p>先選擇專案並準備交接摘要。原對話保留，確認後只建立新草稿。</p>
    <label>目的專案<select data-project aria-label="目的專案"></select></label>
    <button data-prepare>準備交接預覽</button><p data-status role="status"></p>
    <section data-preview hidden><p data-destination></p><p data-notice></p>
    <label>交接摘要與接下來的需求（最近對話摘錄，可編輯）<textarea data-text maxlength="16000" rows="12"></textarea></label>
    <div class="dialog-actions"><button data-accept>建立專案草稿</button></div></section>`;
  document.body.append(dialog);
  const q=s=>dialog.querySelector(s);
  function invalidate() {
    if(pending || request)command({action:"cancel",id:pending || request});
    pending=null;request=null;q('[data-preview]').hidden=true;q('[data-text]').value="";
  }
  dialog.addEventListener('close',()=>{if(!dialog.open){invalidate();origin=null;}});
  q('[data-close]').onclick=()=>dialog.close();
  q('[data-project]').onchange=invalidate;
  $('handoff-start').onclick=()=>{
    invalidate();origin=state.active_id;
    send({type:'draft',text:$('prompt').value});
    const select=q('[data-project]');select.replaceChildren();
    for(const project of state.projects?.items||[])select.add(new Option(project.name,project.id));
    q('[data-status]').textContent=select.options.length ? '選擇目的專案後，先預覽內容。' : '尚無專案，請先建立專案，再回到原對話帶入。';
    q('[data-prepare]').disabled=!select.options.length;
    dialog.showModal();
  };
  q('[data-prepare]').onclick=()=>{
    invalidate();request=crypto.randomUUID();q('[data-status]').textContent='正在準備…';
    command({action:'prepare',source:origin,project:q('[data-project]').value,request});
  };
  q('[data-accept]').onclick=()=>{
    if(!pending)return;q('[data-accept]').disabled=true;
    command({action:'commit',id:pending,text:q('[data-text]').value});
  };
  function pathHint() {
    const general=!state.conversations?.find(c=>c.id===state.active_id)?.project_id;
    $('handoff-path-hint').hidden=!state.logged_in || !general || !/(?:[A-Za-z]:[\\/]|\\\\[^\s\\]+\\)/.test($('prompt').value);
  }
  $('prompt').addEventListener('input',pathHint);
  window.ConversationUI={
    receive(message) {
      if(message.type==='handoff_preview' && dialog.open && message.request===request && message.source===origin && origin===state.active_id) {
        pending=message.id;q('[data-text]').value=message.text;
        q('[data-destination]').textContent=`目的地：${message.project}（${message.root}）`;
        q('[data-notice]').textContent=`${message.notice} 原對話有 ${message.attachment_count} 個保留的網站附件。`;
        q('[data-status]').textContent='請檢查摘錄與需求；內容只會在你之後送出草稿時交給模型。';
        q('[data-preview]').hidden=false;q('[data-accept]').disabled=false;
      } else if(message.type==='handoff_done') {pending=null;dialog.close();}
      else if(message.type==='handoff_error' && dialog.open && (message.id===pending || message.id===request)) {
        q('[data-status]').textContent=message.message;q('[data-accept]').disabled=false;
      }
    },
    sync() {
      const current=state.conversations?.find(c=>c.id===state.active_id);
      const project=state.projects?.items?.find(p=>p.id===current?.project_id);
      $('handoff-start').hidden=!!current?.project_id;
      $('handoff-start').disabled=!state.logged_in || !state.active_id || state.busy!=='none' || !!state.projects?.running || !!state.work?.pending || !!state.work?.transferring;
      if(dialog.open && (!state.logged_in || origin!==state.active_id || state.busy!=='none' || state.projects?.running))dialog.close();
      const link=$('handoff-source');link.hidden=!current?.handoff_source;
      const source=state.conversations?.find(c=>c.id===current?.handoff_source);
      link.textContent=source ? `回到來源對話：${source.title}` : '來源對話已刪除';link.disabled=!source || state.busy!=='none';
      link.onclick=()=>{if(source)send({type:'select_chat',id:source.id});};
      $('project-tools-hint').textContent=project ? `專案：${project.name}。純文字問題可直接在此提問，不必切回一般對話。` : '建立或選擇專案對話後，即可使用文件與資料夾工具。';
      pathHint();
    },
    overview(container, message, isProject, messageIndex) {
      if(message.role!=='assistant' || !isProject)return;
      const value=message.project_overview, card=node('section','task-overview');
      card.append(node('strong','','本次任務總覽'));
      if(!value || value.unavailable) {
        card.append(node('p','','此任務沒有可用的原生總覽紀錄；不能從回答文字推定已讀檔、已寫入或驗證成功。'));
      } else {
        const states={completed:'已結束',partial:'部分完成',paused:'已暫停',cancelled:'已停止',failed:'未完成',waiting_user:'等待補充'};
        card.append(node('p','',`${states[value.state]||'狀態待確認'} · 已記錄 ${value.operations} 項操作 · ${value.outputs.length} 個已保存成果 · ${value.failures.length} 項失敗／未知${value.truncated?'（清單已截斷）':''}`));
        function section(title,rows,format) {
          const detail=document.createElement('details');detail.append(node('summary','',`${title}（${rows.length}）`));
          const list=node('ul');for(const row of rows)list.append(node('li','',format(row)));
          if(!rows.length)list.append(node('li','','沒有可列出的紀錄。'));detail.append(list);card.append(detail);
        }
        section('讀取／查詢範圍',value.reads,r=>`${r.path} · ${r.tool} · ${r.range}`);
        const outputs=document.createElement('details');outputs.append(node('summary','',`已保存副本／成果（${value.outputs.length}）`));
        const list=node('ul');
        for(const path of value.outputs) {
          const row=node('li','',path),preview=value.previews?.find(p=>p.path===path);
          row.append(node('p','muted',preview?.summary||'舊紀錄未保存預覽摘要。'));
          if(preview?.status==='available' && message.request_id && Number.isInteger(messageIndex)) {
            row.append(window.DiffPreviewUI.button({conversation:state.active_id,message_index:messageIndex,request_id:message.request_id,path}));
          }
          list.append(row);
        }
        if(!value.outputs.length)list.append(node('li','','沒有可列出的紀錄。'));
        outputs.append(list);card.append(outputs);
        section('失敗／結果未知',value.failures,r=>`${r.tool}：${r.message}`);
        card.append(node('p','muted','未驗證：工具成功不代表全文已讀完或內容正確；未列出的檔案、外部服務效果及語意結論仍需另行核對。此卡不會重播任何操作。'));
      }
      container.append(card);
    }
  };
})();
