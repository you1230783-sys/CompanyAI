/* 導覽只打開頁面或建立空白對話；不代替使用者送出內容。 */
"use strict";
window.NavigationUI = (() => {
  const projects = $("projects-dialog"), tools = $("project-tools-menu"), picker = $("new-conversation-dialog");
  $("collapse-projects").onclick = () => projects.showModal();
  $("project-tools-open").onclick = () => tools.showModal();
  projects.querySelector('[data-projects-close]').onclick = () => projects.close();
  tools.querySelector('[data-tools-close]').onclick = () => tools.close();
  picker.querySelector('[data-conversation-close]').onclick = () => picker.close();
  // 先關閉入口，讓後續設定／範本對話框取得正確焦點與Escape行為。
  for (const dialog of [projects, tools]) dialog.addEventListener('click', event => {
    if (event.target.closest('button') && !event.target.closest('button').disabled) dialog.close();
  }, true);
  function newConversation() {
    if ($('new-chat').disabled) return;
    const list=$('new-conversation-options');list.replaceChildren();
    const options=[{name:'一般對話',id:null},...(state.projects?.items||[]).map(p=>({name:`專案：${p.name}`,id:p.id}))];
    for (const item of options) {
      const button=node('button','',item.name);
      button.disabled=!!item.id && (!!state.projects?.running || !!state.work?.incoming);
      button.onclick=()=>{picker.close();showView('chat');send(item.id?{type:'project',command:{action:'new_chat',id:item.id}}:{type:'new_chat'});};
      list.append(button);
    }
    picker.showModal();
  }
  function receive(message) {
    if(message.type!=="pet_navigate")return;
    const target=message.target;
    if(target==="main")return;
    if(target==="vnc") { $("show-vnc").click();return; }
    if(target==="settings") { window.PetUI?.open();return; }
    if(target==="tarot") { window.TarotUI?.open();return; }
    if(target==="projects") { $("collapse-projects").click();return; }
    if(target==="schedules") { window.ScheduleUI?.showList();return; }
    if(!state.logged_in || $("new-chat").disabled)return;
    if(target==="new_chat") {showView('chat');send({type:'new_chat'});return;}
    if(typeof target==='string' && target.startsWith('project:')) {
      const id=target.slice(8);
      if(!state.projects?.running && !state.work?.incoming && state.projects?.items?.some(p=>p.id===id)) {
        showView('chat');send({type:'project',command:{action:'new_chat',id}});
      }
    }
  }
  return {newConversation,receive};
})();
