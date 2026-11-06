/* 編輯視窗綁定開啟時的對話與版本；切換版本只送出 select，不會呼叫送出聊天。 */
"use strict";
(() => {
  let editing = null, sending = false;
  const command = value => send({type:"branch",command:value});
  const dialog = document.createElement("dialog"); dialog.id = "branch-edit-dialog";
  dialog.innerHTML = `<div class="dialog-heading"><h2>編輯提問</h2><button data-close>關閉</button></div>
    <p data-notice></p><label>提問<textarea data-text maxlength="16000" rows="9"></textarea></label>
    <p data-status role="status"></p><div class="dialog-actions"><button data-submit>編輯並重新送出</button></div>`;
  document.body.append(dialog);
  const q = selector => dialog.querySelector(selector);
  const versions = document.createElement("div"); versions.id = "branch-versions"; versions.hidden = true;
  const label = document.createElement("label"); label.textContent = "對話版本 ";
  const select = document.createElement("select"); select.setAttribute("aria-label","對話版本"); label.append(select); versions.append(label);
  $("messages").before(versions);
  q('[data-close]').onclick = () => dialog.close();
  dialog.addEventListener('close', () => { editing=null; sending=false; });
  function edit(message,index) {
    editing={conversation:state.active_id,branch_id:state.branches.id,index,message_count:state.messages.length,request_id:message.request_id||null};
    sending=false;q('[data-text]').value=message.content;q('[data-status]').textContent='';q('[data-submit]').disabled=false;
    q('[data-submit]').textContent=state.branches.project?'放回草稿':'編輯並重新送出';
    q('[data-notice]').textContent=state.branches.project?'僅修改尚未執行的提問並放回草稿；不會自動送出。':'將建立新版本並重新送出，舊版本完整保留。保留範圍的附件失效時，會提示另行附加。';
    dialog.showModal();q('[data-text]').focus();
  }
  q('[data-submit]').onclick=()=>{
    if(!editing || sending || !q('[data-text]').value.trim())return;
    const bound={...editing}, text=q('[data-text]').value;
    const valid=()=>dialog.open && editing?.conversation===bound.conversation && state.active_id===bound.conversation && state.branches?.id===bound.branch_id && state.messages.length===bound.message_count && state.branches.idle;
    const submit=()=>{if(!valid())return;sending=true;q('[data-submit]').disabled=true;send({type:'draft',text:$('prompt').value});command({action:'edit',...bound,text});};
    if(state.branches.project)submit();else window.confirmFastModelSend(submit,valid);
  };
  select.onchange=()=>{
    send({type:'draft',text:$('prompt').value});
    command({action:'select',conversation:state.active_id,current_branch:state.branches.id,target_branch:select.value});
  };
  window.BranchUI={
    sync() {
      const b=state.branches;
      versions.hidden=!b || b.project || b.versions.length<2;
      select.replaceChildren();
      for(const v of b?.versions||[])select.add(new Option(`第 ${v.number}／${b.versions.length} 個版本`,v.id));
      if(b){select.value=b.id;select.disabled=!b.idle;select.title=b.idle?'切換只改變顯示及後續使用的歷史，不會送出':b.notice;}
      if(dialog.open && (!b || state.active_id!==editing?.conversation || b.id!==editing?.branch_id || !state.logged_in))dialog.close();
    },
    attach(tools,message,index) {
      const b=state.branches;if(!b)return;
      if(message.role==='user') {
        const button=node('button','edit-message',b.project?'編輯草稿':'編輯並重新送出');
        const allowed=!b.project || b.draft_index===index;
        button.disabled=!allowed || !b.idle || (!b.project && !b.can_send);
        button.title=!allowed?'這則專案訊息已開始執行，不能原地編輯；請從這裡另開新對話。':!b.idle?b.notice:'保留舊版本';
        button.onclick=()=>edit(message,index);tools.append(button);
      }
      if(b.project) {
        const button=node('button','handoff-message','從這裡另開新對話');button.disabled=!b.idle;
        button.title='只帶入到這則訊息為止的摘要，建立新專案草稿，不自動執行';
        button.onclick=()=>window.ConversationUI.fromMessage(index);tools.append(button);
      }
    },
    receive(event) {
      if(event.type==='branch_done')dialog.close();
      if(event.type==='branch_error' && dialog.open){sending=false;q('[data-submit]').disabled=false;q('[data-status]').textContent=event.message;}
    }
  };
})();
