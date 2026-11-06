(async () => {
  const checks=[], commands=[], old=LMUI.getState(), frame=()=>new Promise(r=>setTimeout(r,80));
  const check=(ok,name)=>{if(!ok)throw Error(name);checks.push(name);};
  send=c=>commands.push(c);
  document.querySelectorAll('dialog[open]').forEach(d=>d.close());
  const fixture={...old,logged_in:true,can_send:false,busy:'none',active_id:'a',messages:[],
    config:{...old.config,model:'quality'},models:[{id:'quality',label:'品質'}],
    conversations:[{id:'a',title:'任務 A',project_id:'p'},{id:'b',title:'任務 B',project_id:'p'},{id:'c',title:'第三項',project_id:'p'}],
    projects:{items:[{id:'p',name:'共同專案',root:'C:/fixture'}],running:true,running_id:'run-a',running_conversation:'a',running_count:2,running_conversations:['a','b'],capacity_full:true,status:'A 進度',activity:[],charts:[]},
    work:{pending:false,transferring:false,tasks:[{id:'run-a',conversation_id:'a',title:'專案 A',project:true,active:true,state:'running',message:'A 進度'},{id:'run-b',conversation_id:'b',title:'專案 B',project:true,active:true,state:'running',message:'等待 A 完成 Word'}]},
    onboarding:{enabled:false,release_pending:false,practice_welcome_pending:false,tutorials:[],release_notes:[]}};
  const show=async()=>{LMUI.receive(fixture);LMUI.showView('chat');await frame();};
  await show();
  check(document.querySelectorAll('.running-task').length===2,'sidebar marks exactly two running conversations');
  check($('task-list').textContent.includes('A 進度') && $('task-list').textContent.includes('等待 A 完成 Word'),'task cards preserve separate progress and waiting');
  const stops=()=>[...$('task-list').querySelectorAll('button')].filter(b=>b.textContent==='停止專案任務');
  check(stops().length===2,'each project card has a stop button');
  stops()[0].click();check(commands.at(-1).command.conversation==='a' && commands.at(-1).command.run_id==='run-a','A card stop binds A identity');
  stops()[1].click();check(commands.at(-1).command.conversation==='b' && commands.at(-1).command.run_id==='run-b','B card stop binds B identity');
  fixture.active_id='c';fixture.projects={...fixture.projects,running:false,running_id:null,running_conversation:null,status:null};await show();
  check($('send').disabled && $('send').title.includes('2 個任務'),'third conversation blocked with capacity explanation');
  check($('project-stop').hidden,'inactive third conversation has no unrelated stop button');
  fixture.active_id='b';fixture.projects={...fixture.projects,running:true,running_id:'run-b',running_conversation:'b',status:'B 進度',file_busy:{request_id:'file-b',message:'B 的文件'}};await show();
  check($('project-file-busy').open && $('project-file-busy').textContent.includes('B 的文件'),'B file prompt displays only B');
  const before=commands.length;
  fixture.active_id='a';fixture.projects={...fixture.projects,running_id:'run-a',running_conversation:'a',file_busy:null};await show();
  check(!$('project-file-busy').open && !commands.slice(before).some(c=>c.command?.action==='file_ready'),'switch away closes stale prompt without authorizing it');
  $('project-stop').click();check(commands.at(-1).command.action==='stop_task' && commands.at(-1).command.run_id==='run-a','conversation stop uses current run');
  fixture.work.tasks[1]={id:'scheduled-run',conversation_id:'b',title:'流程排程',schedule:true,active:true,state:'running',message:'等待 Excel'};await show();
  const scheduled=[...$('task-list').querySelectorAll('button')].find(b=>b.textContent==='停止流程排程');
  scheduled.click();check(commands.at(-1).type==='schedule' && commands.at(-1).command.action==='stop_execution' && commands.at(-1).command.execution==='scheduled-run','schedule stop binds execution identity');
  check(![...scheduled.parentElement.querySelectorAll('button')].some(b=>['取消任務','停止追蹤','移除任務'].includes(b.textContent)),'schedule card does not expose remote task commands');
  const deadline=Date.now()+20000;
  const retry=(id,reason)=>({text:'等待重試',at:'2026-11-07 00:00:00',retry:{id,attempt:4,limit:50,continuous:true,reason,total_seconds:500,due_at_millis:deadline,waiting:true}});
  fixture.projects={...fixture.projects,retry_enabled:true,retry_run_id:'run-a',activity:[retry('retry-a','排隊逾時')]};await show();
  const activity=$('project-activity');activity.open=true;
  const clock=activity.querySelector('li .project-retry-clock');
  check(clock?.textContent.includes('排隊逾時') && activity.querySelectorAll('li').length===1,'retry countdown is one activity row');
  const beforeTick=clock.textContent;
  for(let tick=0;tick<25 && clock.textContent===beforeTick;tick++) await new Promise(resolve=>setTimeout(resolve,100));
  check(activity.querySelector('li .project-retry-clock')===clock && clock.textContent!==beforeTick,'retry timer updates the same text node');
  $('project-retry').checked=false;$('project-retry').dispatchEvent(new Event('change'));
  check(commands.at(-1).command.run_id==='run-a' && commands.at(-1).command.conversation==='a','retry preference change binds current run');
  fixture.active_id='b';fixture.projects={...fixture.projects,running_id:'run-b',running_conversation:'b',retry_run_id:'run-b',activity:[retry('retry-b','請求過多')]};await show();
  check(activity.querySelector('summary').textContent.includes('請求過多') && !activity.textContent.includes('排隊逾時'),'changing conversations switches to its own retry deadline');
  const sent=retry('retry-b','請求過多');sent.retry.waiting=false;sent.text='已重新送出';
  fixture.projects.activity=[sent];await show();
  check(activity.querySelectorAll('li').length===1 && !activity.querySelector('.project-retry-clock') && activity.textContent.includes('已重新送出'),'finished retry keeps a single final row');
  fixture.active_id='a';fixture.projects={...fixture.projects,running:false,running_count:0,capacity_full:false,running_id:null,
    running_conversation:null,running_conversations:[],activity:[],batch:{entries:[],window:null,unread:null}};await show();
  $('prompt').value='整理合成測試文件';$('batch-enqueue').click();
  const queued=commands.at(-1);
  check(queued.command?.command?.action==='enqueue' && queued.command.command.text==='整理合成測試文件' && $('batch-dialog').open,'batch button queues a draft without sending a chat');
  fixture.projects.batch={window:'2026-11-06',unread:'2026-11-06',entries:[{id:'batch-test',conversation:'a',project:'共同專案',title:'合成測試',state:'waiting_user',detail:'等待計畫核准',window:'2026-11-06'}]};await show();
  check($('batch-summary').textContent.includes('等待你處理 1 項') && $('batch-summary').textContent.includes('等待計畫核准'),'batch summary shows native waiting reason');
  $('batch-close').click();
  return checks;
})()
