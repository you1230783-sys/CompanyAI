/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-Review.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'review52-' + Date.now());
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
      send=()=>{};const old=LMUI.getState();LMUI.receive({...old,logged_in:true,can_send:true,update_required:false,active_id:'review52',messages:[],conversations:[],projects:{items:[]}});
      const block=(text,label)=>({text,label,format:null});
      ReviewUI.open({name:'設備異常調查.txt',path:'_AI_Output/20261012/設備異常調查.txt',office:false,before_revision:'建立副本時的版本',after_revision:'完成修訂版本',notice:'建立副本時與選定版本的內容快照；不會重新讀取或覆寫目前原檔。',
        before:{blocks:[block('設備異常調查','第1行'),block('10:00 上料後，設備立即停機。','第2行'),block('原因：網路中斷。','第3行'),block('所有資料已核對完成。','第4行')]},
        after:{blocks:[block('設備異常調查','第1行'),block('使用者約10:00上料；LOG於10:03:21記錄開始事件。','第2行'),block('可能與網路重試有關，尚需核對前後事件。','第3行'),block('已搜尋A站09:55–10:15；B站LOG尚未提供。','第4行'),block('來源：A.log，第125–163行。','第5行')]}});
    })()`);
    for(const [name,width,theme] of [['light',1180,'light'],['dark',1180,'dark'],['narrow',650,'light']]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});
      await evaluate('document.documentElement.dataset.theme='+JSON.stringify(theme));await delay(100);
      const measure=await evaluate(`({width:innerWidth,scroll:document.documentElement.scrollWidth,dialog:document.getElementById('review-window').getBoundingClientRect().width})`);
      if(measure.scroll>width||measure.dialog>width)throw Error('review overflow '+name);
      const png=await call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      fs.writeFileSync('.build/review52-'+name+'.png',Buffer.from(png.data,'base64'));
      checks.push('visual '+name);
    }
    fs.writeFileSync('.build/review52-ui.json',JSON.stringify({result:'PASS',checks},null,2));console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
