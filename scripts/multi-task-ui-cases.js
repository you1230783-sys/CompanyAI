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
  return checks;
})()
