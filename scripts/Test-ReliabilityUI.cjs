/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-Settings.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'reliability-ui-' + Date.now());
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
      const pause=()=>new Promise(r=>setTimeout(r,450));
      const passed=[];
      const check=(condition,name)=>{if(!condition)throw Error(name);passed.push(name);};
      window.testCommands=[];send=command=>testCommands.push(command);
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'privacy-test',version:'0.9.0',
        config:{...LMUI.getState().config,model:'quality'},models:[{id:'quality',label:'品質'}],
        projects:{items:[],running:false},conversations:[{id:'privacy-test',title:'合成測試'}],messages:[],
        onboarding:{enabled:true,release_pending:false,practice_welcome_pending:false,tutorials:[],release_notes:[],version:'0.9.0'}};
      LMUI.receive(fixture);LMUI.showView('chat');
      check(!$('project-capability-hint').hidden && $('project-capability-hint').textContent.includes('未連結本機資料夾'),'ordinary chat always shows folder boundary');
      $('copy-error').click();check(testCommands.at(-1).type==='copy_error' && Object.keys(testCommands.at(-1)).length===1,'copy obtains native error without accepting injected text');
      $('diagnostics-open').click();check(testCommands.at(-1).type==='export_diagnostics' && !testCommands.at(-1).include_details,'toolbar requests minimal preview, not save');
      const text=JSON.stringify({safe:'<img src=x onerror=alert(1)>',version:'0.9.0'});
      SettingsUI.receive({type:'diagnostics_preview',preview_id:'snapshot-1',text,help:[{code:'13',message:'非預期錯誤',suggestion:'重新啟動'}]});
      check($('diagnostics-preview').open && $('diagnostics-preview-text').readOnly,'diagnostics opens read-only preview');
      check($('diagnostics-preview-text').value===text && !$('diagnostics-preview').querySelector('img'),'preview is exact inert text');
      check(!testCommands.some(c=>c.type==='save_diagnostics'),'preview does not save automatically');
      $('diagnostics-save').click();
      check(JSON.stringify(testCommands.at(-1))===JSON.stringify({type:'save_diagnostics',preview_id:'snapshot-1'}),'confirm submits only native snapshot id');
      check($('diagnostics-save').disabled,'save disabled until native result');
      SettingsUI.receive({type:'diagnostics_exported',text:'已取消匯出。'});
      check(!$('diagnostics-save').disabled && $('diagnostics-preview-status').textContent==='已取消匯出。','cancelled save remains reviewable');
      $('diagnostics-cancel').click();await pause();
      check(testCommands.at(-1).type==='cancel_diagnostics' && $('diagnostics-preview-text').value==='','closing clears private preview and revokes snapshot');
      fixture.logged_in=false;LMUI.receive(fixture);await pause();
      check(!$('diagnostics-open').disabled && AuthUI.canSend({type:'export_diagnostics'}),'diagnostics accessible without login');
      check(!AuthUI.canSend({type:'work'}) && !AuthUI.canSend({type:'copy'}) && !AuthUI.canSend({type:'onboarding_ack',topic:'chat'}),'offline diagnostics does not bypass ordinary command authorization');
      // Reset module scheduling by reloading its script in a fresh page below; this page already ran its first-start timer.
      return passed;
    })()`);
    await call('Page.reload');
    for(let i=0;i<50;i++){if(await evaluate('!!window.SettingsUI && !!window.OnboardingUI'))break;await delay(100);}
    const releaseChecks = await evaluate(`(async () => {
      const pause=()=>new Promise(r=>setTimeout(r,500));
      const passed=[];const check=(c,n)=>{if(!c)throw Error(n);passed.push(n);};
      window.testCommands=[];send=c=>testCommands.push(c);
      const fixture={...LMUI.getState(),logged_in:false,busy:'none',version:'0.9.0',
        onboarding:{enabled:true,release_pending:true,from_version:null,practice_welcome_pending:false,tutorials:[],version:'0.9.0',
          updates:[{version:'0.8.99',section:'舊紀錄',items:['older']},{version:'0.8.100',section:'新紀錄',items:['newer']},{version:'0.9.0',section:'可靠度',items:['current']}],release_notes:[]}};
      LMUI.receive(fixture);await pause();
      check($('onboarding-dialog').open,'first startup announcement opens before login');
      check($('onboarding-intro').textContent.includes('無法確定'),'unknown previous version is explicitly unknown');
      check([...$('onboarding-body').querySelectorAll('h3')].map(n=>n.textContent).join(',')==='0.9.0,0.8.100,0.8.99','release versions use numeric ordering');
      $('onboarding-done').click();
      check(testCommands.some(c=>c.type==='onboarding_ack'&&c.topic==='release'),'acknowledgment uses dedicated release topic');
      fixture.onboarding.release_pending=false;LMUI.receive(fixture);await pause();
      check(!$('onboarding-dialog').open,'acknowledged release does not reopen');
      return passed;
    })()`);
    checks.push(...releaseChecks);
    await evaluate(`SettingsUI.receive({type:'diagnostics_preview',preview_id:'visual-only',text:JSON.stringify({app_version:'0.9.0',platform:'windows-x64',includes_conversation_log:false,events:[{code:'13',message:'非預期錯誤，請先核對成果再重新啟動。'}]},null,2),help:[{code:'13',message:'非預期程式錯誤',suggestion:'請預覽診斷並重新啟動。'}]})`);
    const capture=await call('Page.captureScreenshot',{format:'png'});
    fs.writeFileSync('.build/reliability-preview.png',Buffer.from(capture.data,'base64'));
    await call('Emulation.setDeviceMetricsOverride',{width:520,height:800,deviceScaleFactor:1,mobile:false});
    const bounds=await evaluate(`(()=>{const r=$('diagnostics-preview').getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,scroll:document.documentElement.scrollWidth};})()`);
    if(bounds.left<0 || bounds.right>520 || bounds.top<0 || bounds.bottom>800 || bounds.scroll>520) throw Error('diagnostics preview exceeds narrow viewport');
    checks.push('diagnostics preview fits desktop and narrow viewport');
    fs.writeFileSync('.build/reliability-ui-verification.json',JSON.stringify({result:'PASS',checks},null,2));
    console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
