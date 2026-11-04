/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-DraftTools.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'draft-tools-ui-' + Date.now());
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
      const passed=[]; const check=(ok,name)=>{if(!ok)throw Error(name);passed.push(name);};
      window.testCommands=[]; send=c=>testCommands.push(c);
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'draft-fixture',
        config:{...LMUI.getState().config,model:'quality'},models:[{id:'quality',label:'品質'}],
        projects:{items:[],running:false},conversations:[{id:'draft-fixture',title:'合成測試'}],messages:[],
        onboarding:{enabled:false,release_pending:false,practice_welcome_pending:false,tutorials:[],release_notes:[]}};
      LMUI.receive(fixture); LMUI.showView('chat');
      const entry={id:'entry',text:{title:'<img src=x onerror=alert(1)>',category:'文件',body:'替 {{部門}} 摘要',favorite:true},parameters:['部門']};
      $('prompt').value='保留原草稿'; $('show-templates').click();
      check(testCommands.at(-2).type==='draft' && testCommands.at(-2).text==='保留原草稿','opening syncs original draft');
      check(testCommands.at(-1).command.action==='state','opening requests native template state');
      DraftTools.receive({type:'template_state',entries:[entry]});
      const dialog=$('templates-dialog'), q=s=>dialog.querySelector(s);
      check(dialog.open && !dialog.querySelector('img'),'untrusted template title is inert text');
      q('[data-parameters] input').value='{{不再展開}}'; q('[data-parameters] input').dispatchEvent(new Event('input'));
      check(q('[data-preview]').value==='替 {{不再展開}} 摘要','preview substitutes parameters once');
      q('[data-use]').click();
      check(testCommands.at(-1).command.action==='use' && testCommands.at(-1).command.conversation==='draft-fixture','use binds originating conversation');
      check(!testCommands.some(c=>c.type==='chat' || c.type==='project'),'template does not send chat or execute tools');
      DraftTools.receive({type:'template_applied'}); check(!dialog.open,'successful use closes dialog');
      $('screenshot-start').click(); check(testCommands.at(-1).command.action==='prepare','capture begins with native precheck');
      DraftTools.receive({type:'screenshot_prepare',id:'capture1',destination:'一般對話：確認後上傳公司網站',hotkey:true});
      const shot=$('screenshot-dialog'), sq=s=>shot.querySelector(s);
      check(shot.open && sq('[data-accept]').hidden && sq('[data-destination]').textContent.includes('公司網站'),'destination shown before capture');
      sq('[data-begin]').click(); check(testCommands.at(-1).command.action==='begin' && testCommands.at(-1).command.id==='capture1','begin uses native snapshot identity');
      DraftTools.receive({type:'screenshot_preview',id:'old',url:'data:image/jpeg;base64,/9j/2Q=='});
      check(sq('[data-image]').hidden,'stale preview is ignored');
      DraftTools.receive({type:'screenshot_preview',id:'capture1',url:'data:image/jpeg;base64,/9j/2Q=='});
      check(!sq('[data-image]').hidden && !sq('[data-accept]').hidden,'capture requires explicit preview acceptance');
      check(!testCommands.some(c=>c.command?.action==='accept'),'preview never accepts automatically');
      sq('[data-accept]').click(); check(testCommands.at(-1).command.action==='accept' && sq('[data-accept]').disabled,'one-shot accept disables button');
      DraftTools.receive({type:'screenshot_done'}); await new Promise(r=>setTimeout(r,80));
      check(!shot.open && !sq('[data-image]').hasAttribute('src'),'closing clears screenshot preview');
      check(!testCommands.some(c=>c.type==='chat'),'capture does not send question');
      DraftTools.receive({type:'screenshot_error',message:'目前模型不支援截圖'});
      check(shot.open && sq('[data-begin]').hidden && sq('[data-status]').textContent.includes('不支援'),'unsupported model error prevents capture');
      shot.close(); $('show-templates').click(); DraftTools.receive({type:'template_state',entries:[entry]});
      return passed;
    })()`);
    const capture=await call('Page.captureScreenshot',{format:'png'});
    fs.writeFileSync('.build/draft-tools-preview.png',Buffer.from(capture.data,'base64'));
    await call('Emulation.setDeviceMetricsOverride',{width:520,height:800,deviceScaleFactor:1,mobile:false});
    const bounds=await evaluate(`(()=>{const r=$('templates-dialog').getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,scroll:document.documentElement.scrollWidth};})()`);
    if(bounds.left<0 || bounds.right>520 || bounds.top<0 || bounds.bottom>800 || bounds.scroll>520) throw Error('template dialog exceeds narrow viewport');
    checks.push('template editor fits desktop and narrow viewport');
    fs.writeFileSync('.build/draft-tools-ui-verification.json',JSON.stringify({result:'PASS',checks},null,2));
    console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
