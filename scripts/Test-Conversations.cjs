/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-DraftTools.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'conversation-ui-' + Date.now());
  fs.mkdirSync(profile, {recursive:true});
  const edge = spawn('C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe', ['--headless=new','--no-first-run','--disable-gpu','--remote-debugging-port=0','--user-data-dir='+profile,'about:blank'], {windowsHide:true,stdio:'ignore'});
  let ws;
  try {
    for (let i=0; i<100 && !fs.existsSync(path.join(profile,'DevToolsActivePort')); i++) await delay(100);
    await delay(300);
    const port = fs.readFileSync(path.join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    ws = new WebSocket(pages.find(p=>p.type==='page').webSocketDebuggerUrl);
    await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
    let sequence=0; const pending=new Map();
    ws.onmessage = event => {const message=JSON.parse(event.data);if(message.id){const item=pending.get(message.id);pending.delete(message.id);message.error?item.reject(message.error):item.resolve(message.result);}};
    const call = (method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
    const evaluate = async expression => {
      const result=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
      if(result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
      return result.result.value;
    };
    await call('Emulation.setDeviceMetricsOverride',{width:1180,height:850,deviceScaleFactor:1,mobile:false});
    await call('Page.navigate',{url:pathToFileURL(path.resolve('ui/index.html')).href});
    for(let i=0;i<50;i++){if(await evaluate('!!window.SettingsUI && !!window.ProjectUI && !!window.AuthUI'))break;await delay(100);}



    const checks = await evaluate(`(async () => {
      const passed=[],check=(ok,name)=>{if(!ok)throw Error(name);passed.push(name);};
      window.testCommands=[];send=c=>testCommands.push(c);
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'general',
        config:{...LMUI.getState().config,model:'quality'},models:[{id:'quality',label:'品質'}],
        conversations:[{id:'general',title:'原對話',updated_at:1},{id:'project-chat',title:'專案對話',project_id:'p',updated_at:2}],
        messages:[{role:'user',content:'幫我看文件'}],projects:{items:[{id:'p',name:'測試專案',root:'C:/fixture'}],running:false},
        onboarding:{enabled:false,release_pending:false,practice_welcome_pending:false,tutorials:[],release_notes:[]}};
      LMUI.receive(fixture);LMUI.showView('chat');filterHistory('all');
      let count=testCommands.length;
      $('new-chat').click();
      check($('new-conversation-dialog').open && testCommands.length===count,'new conversation waits for type selection');
      const choices=$('new-conversation-options').querySelectorAll('button');
      check(choices.length===2 && choices[1].textContent==='專案：測試專案','unified picker includes general and project');
      choices[1].click();check(testCommands.at(-1).command.id==='p' && !$('new-conversation-dialog').open,'project selection creates only selected project chat');
      $('new-chat').click();$('new-conversation-options').querySelector('button').click();
      check(testCommands.at(-1).type==='new_chat','general selection creates general chat');
      $('collapse-projects').click();check($('projects-dialog').open && $('projects-dialog').contains($('project-list')),'project management opens modal');
      $('projects-dialog').querySelector('[data-projects-close]').click();
      $('project-tools-open').click();check($('project-tools-menu').open && $('project-tools-menu').contains($('show-templates')) && $('project-tools-menu').contains($('handoff-start')),'tools modal groups templates and handoff');
      $('project-tools-menu').querySelector('[data-tools-close]').click();
      check([...document.querySelectorAll('.composer-shortcuts [data-action]')].every(b=>!b.hidden),'general composer keeps four quick actions');
      count=testCommands.length;NavigationUI.receive({type:'pet_navigate',target:'project:missing'});
      check(testCommands.length===count,'pet stale project cannot create conversation');
      NavigationUI.receive({type:'pet_navigate',target:'project:p'});
      check(testCommands.at(-1).command.id==='p','pet shortcut opens selected project conversation');
      check($('history-list').querySelectorAll('.history-item').length===2,'combined recent conversations');
      check($('history-list').querySelector('.history-item').dataset.id==='project-chat','recent ordering retained');
      check($('history-list').textContent.includes('專案：測試專案'),'project label is visible');
      filterHistory('general');check($('history-list').querySelectorAll('.history-item').length===1,'general filter');
      document.querySelector('.project-title-toggle').click();check($('history-filter').value==='project:p' && state.active_id==='general','project management filters without switching');
      $('prompt').value='保留的草稿';$('handoff-start').click();
      const dialog=$('handoff-dialog'),q=s=>dialog.querySelector(s);
      check(dialog.open && testCommands.at(-1).type==='draft','opening preserves originating draft');
      q('[data-prepare]').click();const prepare=testCommands.at(-1).command;
      check(prepare.action==='prepare' && prepare.source==='general' && prepare.project==='p','preview request binds source and target');
      ConversationUI.receive({type:'handoff_preview',request:'stale',source:'general',id:'wrong',text:'wrong'});
      check(q('[data-preview]').hidden,'stale preview ignored');
      const preview={type:'handoff_preview',request:prepare.request,source:'general',id:'preview',text:'<img src=x onerror=alert(1)>',project:'測試專案',root:'C:/fixture',attachment_count:2,notice:'附件不會帶入'};
      ConversationUI.receive(preview);
      check(!q('[data-preview]').hidden && !dialog.querySelector('img') && q('[data-text]').value.includes('<img'),'untrusted handoff stays editable inert text');
      check(q('[data-notice]').textContent.includes('附件不會帶入') && q('[data-notice]').textContent.includes('2'),'attachment boundary displayed');
      q('[data-text]').value='使用者修改後需求';q('[data-accept]').click();
      check(testCommands.at(-1).command.action==='commit' && testCommands.at(-1).command.id==='preview' && testCommands.at(-1).command.text==='使用者修改後需求','confirm submits only preview id and edited text');
      check(q('[data-accept]').disabled && !testCommands.some(c=>c.type==='chat'),'confirmation cannot send chat');
      ConversationUI.receive({type:'handoff_done'});await new Promise(r=>setTimeout(r,80));
      check(!dialog.open && !q('[data-text]').value,'closing clears handoff content');
      fixture.active_id='project-chat';fixture.conversations[1].handoff_source='general';
      fixture.messages=[{role:'assistant',content:'model claims success',project_overview:{state:'partial',operations:2,reads:[{tool:'read_file',path:'<img src=x>',range:'終點 10；總量 100'}],outputs:[],failures:[{tool:'publish',message:'denied'}]}}];
      LMUI.receive(fixture);
      check([...document.querySelectorAll('.composer-shortcuts [data-action]')].every(b=>b.hidden),'project composer only exposes screenshot and tools');
      check(!$('handoff-source').hidden && $('handoff-source').textContent.includes('原對話'),'source navigation retained');
      check(document.querySelector('.task-overview').textContent.includes('denied') && !document.querySelector('.task-overview img'),'native overview is rendered as inert text');
      check(document.querySelector('.task-overview').textContent.includes('未驗證'),'overview retains verification limits');
      fixture.messages=[{role:'assistant',content:'all files written'}];LMUI.receive(fixture);
      check(document.querySelector('.task-overview').textContent.includes('沒有可用的原生總覽'),'old answer cannot invent execution evidence');
      check($('project-tools-menu').contains($('project-artifacts')) && !$('project-tools-menu').contains($('project-stop')),'tools grouped while stop stays directly accessible');
      fixture.active_id='general';fixture.messages=[];LMUI.receive(fixture);$('handoff-start').click();
      q('[data-prepare]').click();const last=testCommands.at(-1).command;
      ConversationUI.receive({...preview,request:last.request});
      fixture.active_id='project-chat';LMUI.receive(fixture);await new Promise(r=>setTimeout(r,80));
      check(!dialog.open && !q('[data-text]').value,'switching conversations closes and revokes preview');
      fixture.active_id='general';LMUI.receive(fixture);$('handoff-start').click();
      q('[data-prepare]').click();const finalRequest=testCommands.at(-1).command;
      ConversationUI.receive({...preview,request:finalRequest.request,text:['原提問：整理規格文件。','原回答（未重新驗證）：先確認文件版本。','接下來的需求：請讀取專案資料夾中的新版規格，整理差異。'].join(String.fromCharCode(10,10))});
      return passed;
    })()`);
    const capture=await call('Page.captureScreenshot',{format:'png'});
    fs.writeFileSync('.build/conversation-preview.png',Buffer.from(capture.data,'base64'));
    await call('Emulation.setDeviceMetricsOverride',{width:520,height:800,deviceScaleFactor:1,mobile:false});
    const bounds=await evaluate(`(()=>{const r=$('handoff-dialog').getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,scroll:document.documentElement.scrollWidth};})()`);
    if(bounds.left<0 || bounds.right>520 || bounds.top<0 || bounds.bottom>800 || bounds.scroll>520) throw Error('handoff dialog exceeds narrow viewport');
    checks.push('handoff fits desktop and narrow viewport');
    await evaluate(`$('handoff-dialog').close();`);
    for (const width of [1180,520]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:850,deviceScaleFactor:1,mobile:false});
      const compact=await evaluate(`(()=>{const r=document.querySelector('.quick-actions').getBoundingClientRect();return {height:r.height,scroll:document.documentElement.scrollWidth};})()`);
      if(compact.height>80 || compact.scroll>width)throw Error('composer toolbar exceeds two compact lines: '+JSON.stringify(compact));
      await evaluate(`$('project-tools-open').click();`);
      const modal=await evaluate(`(()=>{const r=$('project-tools-menu').getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom};})()`);
      if(modal.left<0||modal.right>width||modal.top<0||modal.bottom>850)throw Error('tools modal exceeds viewport');
      const screenshot=await call('Page.captureScreenshot',{format:'png'});
      fs.writeFileSync('.build/project-tools-'+width+'.png',Buffer.from(screenshot.data,'base64'));
      await evaluate(`$('project-tools-menu').close();`);
      checks.push('compact toolbar and tools modal fit '+width);
    }
    fs.writeFileSync('.build/conversation-ui-verification.json',JSON.stringify({result:'PASS',checks},null,2));
    console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
