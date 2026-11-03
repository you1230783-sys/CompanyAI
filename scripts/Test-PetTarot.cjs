/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-Settings.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'pet-tarot-review-' + Date.now());
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
      const checks=[];const check=(ok,label)=>{if(!ok)throw Error(label);checks.push(label);};
      window.testCommands=[];send=command=>testCommands.push(command);
      LMUI.receive({...LMUI.getState(),logged_in:false,busy:'none'});
      check(!$('show-tarot').disabled && !$('show-pet').disabled,'offline entries enabled');
      $('show-advanced').click();check($('advanced-dialog').open && !$('sidebar').contains($('show-pet')),'advanced menu frees sidebar rows');$('show-tarot').click();check(testCommands.at(-1).command.action==='state','opens native local round');
      const round={id:'test-round',count:3,chosen:[],sent:false,history:[],used:[],remaining:78};
      TarotUI.receive({type:'tarot_state',state:round});
      check(document.querySelectorAll('.tarot-back').length===78,'78 separate card backs');
      check(getComputedStyle(document.querySelector('.tarot-back')).backgroundImage.includes('tarot/back.jpg'),'card back image survives shared dialog button styles');
      document.querySelectorAll('.tarot-back')[13].click();
      check(testCommands.at(-1).command.position===13 && testCommands.at(-1).command.id==='test-round','selected position sent without card identity');
      round.chosen=[{slot:13,name:'愚者',image:'tarot/00.jpg',position:'現況',reversed:false,meaning:'新的開始'},
        {slot:40,name:'寶劍三',image:'tarot/52.jpg',position:'阻礙',reversed:true,meaning:'整理感受'},
        {slot:77,name:'星星',image:'tarot/17.jpg',position:'建議',reversed:false,meaning:'逐步累積信心'}];
      round.used=[13,40,77];round.remaining=75;
      TarotUI.receive({type:'tarot_state',state:round});
      check([...document.querySelectorAll('.tarot-back')].every(b=>b.disabled),'complete round prevents fourth pick');
      check(document.querySelectorAll('.tarot-face.reversed').length===1 && getComputedStyle(document.querySelector('.tarot-face.reversed')).transform==='matrix(-1, 0, 0, -1, 0, 0)','reversed image rotates 180 degrees without a second asset');
      check(document.querySelectorAll('.tarot-card').length===3,'three text fronts with normal and reversed cards');
      document.querySelector('[data-analyze]').click();const before=testCommands.length;
      document.querySelector('[data-submit]').click();
      check(testCommands.length===before && document.querySelector('[data-error]').textContent.includes('填寫'),'question required');
      document.querySelector('[data-question]').value='如何安排工作？';document.querySelector('[data-submit]').click();
      check(testCommands.length===before && document.querySelector('[data-error]').textContent.includes('登入'),'offline analysis does not send');
      AuthUI.apply();check($('tarot-dialog').open,'offline round remains open during auth render');
      LMUI.receive({...LMUI.getState(),logged_in:true,can_send:true,busy:'none'});
      document.querySelector('[data-submit]').click();
      check(testCommands.at(-1).command.question==='如何安排工作？' && testCommands.at(-1).command.action==='analyze','explicit AI button sends exact question and round');
      TarotUI.receive({type:'tarot_state',state:{...round,sent:true}});
      check(document.querySelector('[data-submit]').disabled,'sent round cannot be submitted again');
      document.querySelector('[data-continue]').click();
      check(testCommands.at(-1).command.action==='continue' && testCommands.at(-1).command.id===round.id,'continue uses existing deck');
      TarotUI.receive({type:'tarot_state',state:{...round,id:'next-round',count:1,chosen:[],sent:false,history:[{number:1,question:'如何安排工作？',chosen:round.chosen}]}});
      check(document.querySelectorAll('.tarot-back')[13].disabled && document.querySelector('[data-history]').textContent.includes('愚者'),'previous picks disabled and history retained');
      check(!document.querySelectorAll('.tarot-back')[0].disabled,'unused cards remain available');
      $('tarot-dialog').close();PetUI.open();
      PetUI.receive({type:'pet_state',state:{settings:{enabled:true,skin:'cat',size:120,reduced_motion:false},images:['idle','thinking','working','waiting','success','error'].map(action=>({action,bytes:6000,url:'pets/cat_idle.png'}))}});
      check(document.querySelector('[data-prompt]').value.startsWith('角色名稱／類型：') && document.querySelector('[data-prompt]').value.includes('米奇'),'character first and reminder in AI generation prompt');
      check(document.querySelectorAll('.pet-images img').length===6,'six independently replaceable poses');
      check($('pet-dialog').textContent.includes('只接受 PNG') && $('pet-dialog').textContent.includes('建議先去背') && $('pet-dialog').textContent.includes('500 KB'),'PNG transparency and compression guidance');
      const petState={settings:{enabled:true,skin:'cat',size:120,reduced_motion:false,bubble_style:'round',bubble_color:'#fff8ed',bubble_opacity:100,notice_seconds:5},images:['idle','thinking','working','waiting','success','error'].map(action=>({action,bytes:6000,url:'pets/cat_idle.png'}))};
      check(!document.querySelector('[data-save]'),'no apply button');
      const change=(selector,value)=>{const element=document.querySelector(selector);element.value=value;element.dispatchEvent(new Event('input',{bubbles:true}));};
      change('[data-size]',160);check(testCommands.at(-1).command.size===160,'size saves immediately');
      change('[data-seconds]',0);change('[data-style]','thought');change('[data-color]','#112233');change('[data-transparency]',45);
      PetUI.receive({type:'pet_state',state:petState});
      check(testCommands.at(-1).command.notice_seconds===0 && testCommands.at(-1).command.bubble_style==='thought' && testCommands.at(-1).command.bubble_color==='#112233' && testCommands.at(-1).command.bubble_opacity===55,'rapid edits coalesce without losing the latest preferences');
      check(document.querySelector('[data-size]').value==='160','old acknowledgment does not revert current controls');
      PetUI.receive({type:'pet_state',state:{...petState,settings:{...petState.settings,size:160,notice_seconds:0,bubble_style:'thought',bubble_color:'#112233',bubble_opacity:55}}});
      PetUI.receive({type:'pet_state',state:{settings:petState.settings}});check(document.querySelectorAll('.pet-images img').length===6,'settings-only reply retains the six image previews');
      document.querySelector('[data-generator]').click();check($('pet-generator-dialog').open && !document.querySelector('[data-prompt]').readOnly,'editable character prompt in modal');
      document.querySelector('[data-prompt]').value='角色名稱／類型：一隻貓';document.querySelector('[data-copy]').click();
      check(testCommands.at(-1).command.action==='copy_prompt' && testCommands.at(-1).command.text==='角色名稱／類型：一隻貓','copy sends the edited prompt');
      PetUI.receive({type:'pet_state',state:petState});$('pet-generator-dialog').close();
      document.querySelector('.pet-images button').click();
      check(testCommands.at(-1).command.action==='import' && testCommands.at(-1).command.state==='idle','upload targets one pose');
      return checks;
    })()`);
    for(const width of [1180,520]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:850,deviceScaleFactor:1,mobile:false});
      const overflow=await evaluate(`document.querySelector('#pet-dialog').scrollWidth > document.querySelector('#pet-dialog').clientWidth+2`);
      if(overflow)throw Error('pet modal overflow at '+width);
      const picture=await call('Page.captureScreenshot',{format:'png'});
      fs.writeFileSync('.build/pet-ui-'+width+'.png',Buffer.from(picture.data,'base64'));
    }
    await call('Emulation.setDeviceMetricsOverride',{width:1180,height:850,deviceScaleFactor:1,mobile:false});
    for(const style of ['round','comic','thought']) {
      await evaluate(`PetUI.receive({type:'pet_state',state:{settings:{enabled:true,skin:'cat',size:120,reduced_motion:false,bubble_style:'${style}',bubble_color:'#fff8ed',bubble_opacity:100,notice_seconds:5},images:['idle','thinking','working','waiting','success','error'].map(action=>({action,bytes:6000,url:'pets/cat_idle.png'}))}})`);
      const picture=await call('Page.captureScreenshot',{format:'png'});
      fs.writeFileSync('.build/pet-bubble-'+style+'.png',Buffer.from(picture.data,'base64'));
    }
    for(const animal of ['cat','dog']) {
      await evaluate(`PetUI.receive({type:'pet_state',state:{settings:{enabled:true,skin:'${animal}',size:120,reduced_motion:false,bubble_style:'round',bubble_color:'#fff8ed',bubble_opacity:100,notice_seconds:5},images:['idle','thinking','working','waiting','success','error'].map(action=>({action,bytes:130000,url:'pets/${animal}_'+action+'.png'}))}})`);
      const picture=await call('Page.captureScreenshot',{format:'png'});
      fs.writeFileSync('.build/pet-art-'+animal+'.png',Buffer.from(picture.data,'base64'));
    }
    await evaluate(`$('pet-dialog').close();$('show-advanced').click();`);
    const menuPicture=await call('Page.captureScreenshot',{format:'png'});
    fs.writeFileSync('.build/advanced-menu.png',Buffer.from(menuPicture.data,'base64'));
    fs.writeFileSync('.build/pet-tarot-ui-verification.json',JSON.stringify({result:'PASS',checks:results},null,2));
    console.log(JSON.stringify({result:'PASS',checks:results}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
