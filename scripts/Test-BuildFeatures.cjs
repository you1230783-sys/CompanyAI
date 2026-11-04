/* 編譯能力的介面回歸；攔截所有原生命令，不使用公司服務。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'feature-review-' + Date.now());
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




    const results=await evaluate(`(() => {
      const checks=[],check=(ok,label)=>{if(!ok)throw Error(label);checks.push(label);};
      window.testCommands=[];send=command=>testCommands.push(command);
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'feature-chat',
        conversations:[{id:'feature-chat',title:'測試',project_id:'project',updated_at:1}],messages:[],
        models:[{id:'quality',label:'品質'}],config:{...LMUI.getState().config,model:'quality',vnc_enabled:true},
        projects:{items:[{id:'project',name:'測試',root:'C:/fixture'}],running:false,status:'',activity:[]}};
      for(let mask=0;mask<16;mask++) {
        document.querySelectorAll('dialog[open]').forEach(dialog=>dialog.close());
        const names=['cad','pet','tarot','vnc'];
        const features=Object.fromEntries(names.map((name,index)=>[name,!!(mask&(1<<index))]));
        LMUI.receive({...fixture,build_features:features});
        check($('show-pet').hidden===!features.pet && $('show-tarot').hidden===!features.tarot,
          '組合'+mask+'：桌寵與塔羅入口符合建置');
        check($('show-vnc').hidden===!features.vnc && $('vnc-enabled').disabled===!features.vnc
          && $('vnc-sync-open').disabled===!features.vnc,
          '組合'+mask+'：VNC入口與設定符合建置');
        check($('vnc-enabled').checked,'組合'+mask+'：停用不清除既有VNC偏好');
        for(const name of ['pet','tarot']) {
          // 模擬每次原生命令完成，避免前一組尚未回覆的busy影響下一組。
          if(name==='pet')PetUI.receive({type:'pet_error',message:''});
          else TarotUI.receive({type:'tarot_error',message:''});
          testCommands.length=0;
          $(name==='pet'?'show-pet':'show-tarot').click();
          check(testCommands.some(command=>command.type===name)===features[name],
            '組合'+mask+'：'+name+'停用時程式觸發也不送命令');
          document.querySelectorAll('dialog[open]').forEach(dialog=>dialog.close());
        }
        $('prompt').value='請閱讀CAD圖面';WorkbenchUI.sync();
        check($('project-capability-hint').textContent.includes('未包含CAD')===!features.cad,
          '組合'+mask+'：CAD能力提示');
        ProjectTutorial.open();
        const tutorial=document.getElementById('project-tutorial');
        check(!!tutorial && tutorial.open,'組合'+mask+'：核心教學仍可開啟');
        const text=tutorial.textContent;
        check(text.includes('VNC 快速連線')===features.vnc && text.includes('CAD')===features.cad,
          '組合'+mask+'：教學不提供停用模組');
        ProjectTutorial.close();
      }
      $('show-reminders').click();
      check(document.querySelector('#reminders-dialog [data-channel]').value==='pet',
        '完整版新增提醒沿用桌寵');
      $('reminders-dialog').close();
      LMUI.receive({...fixture,build_features:{cad:false,pet:false,tarot:false,vnc:false}});
      $('show-reminders').click();
      check(document.querySelector('#reminders-dialog [data-channel]').value==='windows',
        '核心版新增提醒使用Windows');
      check(document.querySelector('#reminders-dialog option[value="pet"]').textContent.includes('改用Windows'),
        '既有桌寵提醒清楚標示替代通知方式');
      return checks;
    })()`);
    fs.writeFileSync('.build/build-features-ui-verification.json',JSON.stringify({result:'PASS',checks:results},null,2));
    console.log(JSON.stringify({result:'PASS',checks:results}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
