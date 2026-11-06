(async () => {
 const checks=[],commands=[],old=LMUI.getState(),frame=()=>new Promise(r=>setTimeout(r,70));
 const check=(ok,name)=>{if(!ok)throw Error(name);checks.push(name);};
 send=c=>commands.push(c);document.querySelectorAll('dialog[open]').forEach(d=>d.close());
 const fixture={...old,logged_in:true,can_send:true,busy:'none',active_id:'branch-chat',
 config:{...old.config,model:'quality'},models:[{id:'quality',label:'品質'}],
 messages:[{role:'user',content:'第一題',request_id:'r1'},{role:'assistant',content:'第一答'},{role:'user',content:'第二題 <img src=x>',request_id:'r2'},{role:'assistant',content:'舊尾段'}],
 conversations:[{id:'branch-chat',title:'分支測試'}],projects:{items:[{id:'p',name:'測試專案',root:'C:/fixture'}],running:false},work:{pending:false,transferring:false},
 branches:{id:'original',number:1,versions:[{id:'original',number:1}],idle:true,can_send:true,project:false,draft_index:null},
 onboarding:{enabled:false,release_pending:false,practice_welcome_pending:false,tutorials:[],release_notes:[]}};
 const show=async()=>{LMUI.receive(fixture);LMUI.showView('chat');await frame();};await show();
 check(document.querySelectorAll('.edit-message').length===2,'all general user messages offer edit');
 check($('branch-versions').hidden,'legacy single branch has no redundant selector');
 const dialog=$('branch-edit-dialog'),q=s=>dialog.querySelector(s);let count=commands.length;
 document.querySelectorAll('.edit-message')[1].click();check(dialog.open && q('[data-text]').value.includes('<img') && !dialog.querySelector('img'),'edit displays exact inert text without execution');
 q('[data-text]').value='改寫第二題';q('[data-close]').click();await frame();check(commands.length===count,'cancel edit makes no request');
 document.querySelectorAll('.edit-message')[1].click();q('[data-text]').value='改寫第二題';q('[data-submit]').click();
 check(commands.at(-1).type==='branch' && commands.at(-1).command.action==='edit' && commands.at(-1).command.index===2 && commands.at(-1).command.request_id==='r2' && commands.at(-1).command.branch_id==='original','edit binds exact conversation version index and request');
 check(commands.at(-1).command.text==='改寫第二題' && !JSON.stringify(commands.at(-1)).includes('舊尾段'),'edit does not send old tail from WebView');
 BranchUI.receive({type:'branch_error',message:'原附件已到期'});check(dialog.open && q('[data-status]').textContent.includes('已到期') && !q('[data-submit]').disabled,'native attachment rejection keeps editable text');
 BranchUI.receive({type:'branch_done'});await frame();
 fixture.branches={...fixture.branches,id:'fork',number:2,versions:[{id:'original',number:1},{id:'fork',number:2}]};fixture.messages=fixture.messages.slice(0,3);fixture.messages[2]={role:'user',content:'改寫第二題',request_id:'new'};await show();
 const select=$('branch-versions').querySelector('select');check(!$('branch-versions').hidden && select.value==='fork' && select.selectedOptions[0].textContent==='第 2／2 個版本','selected version renders after restore');
 count=commands.length;select.value='original';select.dispatchEvent(new Event('change'));check(commands.length===count+2 && commands.at(-1).command.action==='select' && !commands.slice(count).some(c=>c.type==='send'),'switch emits only draft preservation and selection');
 fixture.branches.idle=false;await show();check([...document.querySelectorAll('.edit-message')].every(b=>b.disabled) && select.disabled,'inflight work prevents branching and switching');
 fixture.branches={...fixture.branches,idle:true,project:true,draft_index:null};fixture.conversations[0].project_id='p';await show();
 check([...document.querySelectorAll('.edit-message')].every(b=>b.disabled && b.title.includes('已開始執行')),'executed project edit disabled with reason');
 check(document.querySelectorAll('.handoff-message').length===3,'every project message has draft handoff');
 document.querySelectorAll('.handoff-message')[1].click();const handoff=$('handoff-dialog');handoff.querySelector('[data-prepare]').click();
 check(commands.at(-1).command.through===1 && commands.at(-1).command.project==='p','project handoff binds selected prefix and defaults project');
 const prepare=commands.at(-1).command;ConversationUI.receive({type:'handoff_preview',request:prepare.request,id:'preview',source:'branch-chat',project:'測試專案',root:'C:/fixture',text:'到第一答為止',attachment_count:0,notice:'不帶附件授權'});
 count=commands.length;handoff.querySelector('[data-accept]').click();check(commands.at(-1).command.action==='commit' && commands.length===count+1 && !JSON.stringify(commands.at(-1)).includes('auto_execute'),'handoff confirmation creates draft without send authority');handoff.close();await frame();
 fixture.branches.draft_index=2;fixture.messages[2].request_id=null;await show();document.querySelectorAll('.edit-message')[1].click();
 check(q('[data-submit]').textContent==='放回草稿','unstarted project question is draft-only edit');q('[data-close]').click();await frame();
 fixture.branches.project=false;delete fixture.conversations[0].project_id;await show();document.querySelectorAll('.edit-message')[1].click();
 fixture.active_id='other';fixture.branches.id='other';await show();check(!dialog.open,'conversation switch invalidates edit dialog');
 fixture.active_id='branch-chat';fixture.branches.id='fork';await show();document.querySelectorAll('.edit-message')[1].click();
 return checks;
})()
