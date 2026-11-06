/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-DiffPreview.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'diff-preview-' + Date.now());
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



    const checks=await evaluate(`(async()=>{const checks=[];await runReviewTests((ok,label)=>{if(!ok)throw Error(label);checks.push(label);});return checks;})()`);
    await evaluate(`(() => {
      send=()=>{};const old=LMUI.getState();LMUI.receive({...old,logged_in:true,can_send:true,update_required:false,active_id:'diff-preview',messages:[],conversations:[],projects:{items:[]}});
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      const target={conversation:'diff-preview',message_index:1,request_id:'diff-test',path:'_AI_Output/報告.xlsx'};
      DiffPreviewUI.button(target).click();
      DiffPreviewUI.receive({type:'project_diff_preview',...target,offset:0,data:{name:'報告.xlsx',summary:'快照範圍共 30 處差異',source_notice:'原件已變更，差異以建立副本時的版本為準。',shown:30,total_changes:30,offset:0,page_size:25,next_offset:25,
        changes:[{location:'工作表!A2',before:{kind:'formula',text:'=1+1',calculation:{value:2},format:{bold:false}},after:{kind:'formula',text:'=2',calculation:{value:2},format:{bold:true}}},
          {kind:'added',location:'工作表!B3',before:null,after:{kind:'text',text:'新增中文與符號 😀 <script> 不執行'}}],
        notice:'依建立時快照比較；公式、計算值與格式分開顯示。未涵蓋項目不能推定相同。'}});
    })()`);
    for(const [name,width,theme] of [['light',1180,'light'],['dark',1180,'dark'],['narrow',650,'light']]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});
      await evaluate('document.documentElement.dataset.theme='+JSON.stringify(theme));await delay(100);
      const measure=await evaluate(`({width:innerWidth,scroll:document.documentElement.scrollWidth,dialog:document.getElementById('diff-preview-window').getBoundingClientRect().width})`);
      if(measure.scroll>width||measure.dialog>width)throw Error('review overflow '+name);
      const png=await call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      fs.writeFileSync('.build/diff-preview-'+name+'.png',Buffer.from(png.data,'base64'));
      checks.push('M8 visual '+name);
    }
    fs.writeFileSync('.build/diff-preview-ui.json',JSON.stringify({result:'PASS',checks},null,2));console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
