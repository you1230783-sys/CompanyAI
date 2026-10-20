/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-Settings.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const delay = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  const profile = path.resolve('.build', 'settings-review-' + Date.now());
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


    const checks=[];
    // 使用正式目錄，避免只驗證沒有教學卡／更新內容的空白分頁。
    const tutorials=JSON.parse(fs.readFileSync('assets/tutorials.json','utf8')).map(item=>({...item,completed:true}));
    const releases=JSON.parse(fs.readFileSync('assets/release-notes.json','utf8'));
    await evaluate(`(() => {
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      window.testCommands=[];send=message=>testCommands.push(message);
      window.testFixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'review-settings',
        version:'0.8.47',models:[{id:'quality',label:'品質'}],notifications:[],
        onboarding:{enabled:true,tutorials:${JSON.stringify(tutorials)},release_notes:${JSON.stringify(releases)},version:${JSON.stringify(releases.at(-1).version)}},
        conversations:[{id:'review-settings',title:'設定來源檢查',updated_at:1}],
        config:{...LMUI.getState().config,debug_mode:true,model:'quality'},
        messages:[{role:'assistant',request_id:'old-run',content:'早先任務'},{role:'user',request_id:'latest-run',content:'目前任務'}],
        projects:{items:[],running:true,running_id:'latest-run',running_conversation:'review-settings',status:'資料核對中',
          activity:['讀取檔案'],preferences:[{id:'q',question:'這兩個時間段如何繪圖？',options:['同一張圖，加入參考線區隔','分開各張圖'],default:'同一張圖，加入參考線區隔',state:'pending'}]}};
      LMUI.receive(testFixture);LMUI.showView('chat');
      window.check=(condition,text)=>{if(!condition)throw Error(text);};
      const card=document.querySelector('#project-preferences'),select=card.querySelector('select'),custom=card.querySelector('input');
      check(select.options.length===3 && custom.hidden,'Other is an explicit option');
      select.value='__custom__';select.dispatchEvent(new Event('change'));
      check(!custom.hidden && card.querySelector('button').disabled,'empty Other cannot submit');
      custom.value='同圖但使用不同顏色';custom.dispatchEvent(new Event('input'));card.querySelector('button').click();
      check(testCommands.at(-1).command.answer==='同圖但使用不同顏色','custom preference reaches original run');
      select.value=select.options[0].value;select.dispatchEvent(new Event('change'));card.querySelector('button').click();
      check(testCommands.at(-1).command.answer===select.value,'ordinary choice ignores earlier custom draft');
      testFixture.projects.preferences=[];LMUI.receive(testFixture);
      $('settings-dialog').showModal();
      const tabs=[...document.querySelectorAll('.settings-tabs [role=tab]')];
      check(tabs.map(t=>t.textContent.trim()).join(',')==='一般,操作,進階,教學與更新日誌','four settings tabs');
      $('hotkey').value='Win+F11';tabs[1].click();tabs[2].click();tabs[0].click();
      check($('hotkey').value==='Win+F11','tabs preserve input drafts');
      tabs[0].dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));
      check(!$('settings-panel-3').hidden && $('settings-panel-0').hidden,'End selects the last tutorial tab');
      check($('tutorial-catalog').closest('#settings-panel-3') && $('updates-replay').closest('#settings-panel-3'),'tutorials and release history have their own panel');
      check(document.querySelectorAll('#tutorial-catalog .tutorial-topic').length===55,'all 55 interactive tutorial topics rendered');
      check(!$('release-history'),'no duplicate inline release history');
      tabs[3].dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true}));
      check(!$('settings-panel-2').hidden && $('settings-panel-3').hidden,'ArrowLeft returns to Advanced');
      check($('vnc-enabled').closest('#settings-panel-2') && $('advanced-settings').closest('#settings-panel-2'),'VNC and diagnostics in Advanced');
      $('project-diagnostics-run').value='old-run';
      testFixture.messages.push({role:'user',request_id:'new-run',content:'新增任務'});LMUI.receive(testFixture);
      check($('project-diagnostics-run').value==='old-run','new state keeps selected old task');
      check($('project-diagnostics-run').size===4,'task picker is a visible list');
      $('diagnostics-export').click();
      check(testCommands.at(-1).type==='export_diagnostics' && !testCommands.at(-1).include_details,'error export defaults to no details');
      SettingsUI.receive({type:'diagnostics_exported',text:'已取消匯出。'});
      check($('diagnostics-export-status').textContent==='已取消匯出。','export completion displayed beside button');
      $('project-diagnostics-open').click();
      check(testCommands.at(-1).command.run_id==='old-run','opens selected old task');
      window.reportFor=(request,selected,trace)=>({type:'project_diagnostics',conversation:request.conversation,run_id:request.run_id,view_request:request.view_request,text:JSON.stringify({trace,tokens:'usage only',rounds:[0,1,2].map(index=>({index,turn:index+1})),selected})});
      const latest=testCommands.at(-1).command;ProjectUI.receive(reportFor(latest,2,'latest trace'));
      $('project-diagnostics-prev').click();const previous=testCommands.at(-1).command;
      check(previous.index===1,'previous round uses actual report index');ProjectUI.receive(reportFor(previous,1,'previous trace'));
      $('project-diagnostics-refresh').click();check(testCommands.at(-1).command.index===1,'refresh keeps selected round');
      const stale=testCommands.at(-1).command;
      $('project-diagnostics-latest').click();const newest=testCommands.at(-1).command;
      ProjectUI.receive(reportFor(stale,1,'STALE PRIVATE TRACE'));
      check(!$('project-diagnostics-text').value.includes('STALE'),'late response ignored');
      ProjectUI.receive(reportFor(newest,2,'NEW TRACE'));
      check($('project-diagnostics-text').value==='NEW TRACE','new response accepted');
      $('project-token-copy').click();check(testCommands.at(-1).command.copy===true && !JSON.stringify(testCommands.at(-1)).includes('TRACE'),'usage copy does not carry content');
    })()`);
    checks.push('settings tabs, Other preferences, selected task, export default, round navigation and stale replies');
    async function capture(name,width,theme) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:800,deviceScaleFactor:1,mobile:false});
      await evaluate('document.documentElement.dataset.theme='+JSON.stringify(theme));await delay(80);
      await evaluate(`check([...document.querySelectorAll('.settings-tabs button')].every(b=>getComputedStyle(b).backgroundColor!==getComputedStyle(b.closest('dialog')).backgroundColor),'settings tab contrast');`);
      const measure=await evaluate(`(() => {const d=document.querySelector('dialog[open]'),r=d.getBoundingClientRect();return {width:innerWidth,scroll:document.documentElement.scrollWidth,left:r.left,right:r.right,top:r.top,bottom:r.bottom};})()`);
      if(measure.scroll>width || measure.left<0 || measure.right>width || measure.top<0 || measure.bottom>801) throw Error('overflow '+name+JSON.stringify(measure));
      const png=await call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      fs.writeFileSync('.build/settings-'+name+'.png',Buffer.from(png.data,'base64'));checks.push({name,...measure});
    }
    await capture('diagnostics-light',1180,'light');
    await evaluate(`$('project-diagnostics-dialog').close();$('settings-dialog').showModal();SettingsUI.select(2);`);
    await capture('advanced-light',1180,'light');await capture('advanced-dark',1180,'dark');await capture('advanced-narrow',520,'light');
    await evaluate(`SettingsUI.select(3);`);await capture('tutorials-narrow',520,'light');
    await evaluate(`document.querySelector('#tutorial-catalog .tutorial-topic').click();check($('project-tutorial').open && !$('settings-dialog').open,'topic replay opens interactive practice');`);
    await capture('tutorial-card-narrow',520,'light');
    await evaluate(`ProjectTutorial.close();`);await delay(100);
    await evaluate(`check($('settings-dialog').open && !$('settings-panel-3').hidden,'closing replay returns to tutorial settings');$('updates-replay').click();check($('onboarding-title').textContent==='更新日誌','release replay opens history');`);
    await capture('release-history-narrow',520,'light');
    await evaluate(`$('onboarding-done').click();`);await delay(100);
    // 保留設定頁回歸，並用真正瀏覽器逐題操作教學。完成前不可先記完成，
    // 模擬輸入／選取必須可繼續；全部過程不能送出正式命令或改動真草稿。
    const tutorialChecks = await evaluate(`(async () => {
      $('settings-dialog').close();
      const originalDraft=$('prompt').value, originalCommands=testCommands.length;
      const finished=[], pause=()=>new Promise(resolve=>setTimeout(resolve,30));
      const clickText=(root,text)=>{
        const target=[...root.querySelectorAll('button')].find(button=>button.textContent===text && !button.disabled);
        check(target,'enabled tutorial button: '+text);target.click();
      };
      const coreSteps={
        'projects.practice.create':['＋ 新增專案','填入範例名稱','名稱填好了','選擇資料夾並建立','選擇這個範例資料夾','＋ 開始專案對話'],
        'projects.practice.files':['訂單小工具','▤ README.md','下一步','▸ src','▤ main.cpp'],
        'projects.practice.request':['下一步','填入範例要求','要求寫好了','送出（教學示範）','下一步']
      };
      check(ProjectTutorial.catalog.length===55,'runtime tutorial catalogue contains all 55 topics');
      for(const topic of ProjectTutorial.catalog){
        ProjectTutorial.open({topic:topic.id,onComplete:id=>finished.push(id)});
        await pause();
        if(topic.scenes){
          for(const scene of topic.scenes){
            const stage=$('project-tutorial').querySelector('.practice-stage');
            check(stage.querySelector('.practice-current'),'scene has spotlight target: '+topic.id);
            const action=scene.action;
            if(action.kind==='button')clickText(stage,action.label);
            else {
              const group=stage.querySelector('.practice-field'),apply=[...group.querySelectorAll('button')].find(button=>button.textContent==='繼續');
              check(apply.disabled,'empty action cannot continue: '+topic.id);
              if(action.kind==='input')clickText(group,'填入範例');
              else if(action.kind==='select'){
                const control=group.querySelector('select');control.selectedIndex=1;control.dispatchEvent(new Event('change'));
                check(document.activeElement===control,'select receives keyboard focus: '+topic.id);
              } else group.querySelector('input[type=checkbox]').click();
              check(!apply.disabled,'completed action enables continue: '+topic.id);apply.click();
            }
            await pause();
          }
        } else for(const label of coreSteps[topic.id]){clickText($('project-tutorial'),label);await pause();}
        check(!finished.includes(topic.id),'topic not completed before confirmation: '+topic.id);
        clickText($('project-tutorial'),'完成這個教學');
        check(finished.includes(topic.id),'topic completion callback: '+topic.id);
        ProjectTutorial.close();await pause();
      }
      check(new Set(finished).size===55,'all topics finish exactly once');
      check(testCommands.length===originalCommands && $('prompt').value===originalDraft,'practice does not send native commands or change real draft');
      ProjectTutorial.open();await pause();
      const search=$('practice-topic-search');search.value='不存在的教學詞';search.dispatchEvent(new Event('input'));
      check(!document.querySelector('.practice-topic'),'unknown search shows empty state');
      search.value='CAD';search.dispatchEvent(new Event('input'));
      check(document.querySelectorAll('.practice-topic').length===5,'search isolates five CAD tutorials');
      ProjectTutorial.close();await pause();
      $('settings-dialog').showModal();
      return {topics:finished.length,native_commands_added:testCommands.length-originalCommands,draft_unchanged:$('prompt').value===originalDraft};
    })()`);
    checks.push('all 55 tutorials completed in real browser; input/select/checkbox gates, focus, isolation and search');
    fs.writeFileSync('.build/tutorials-review.json',JSON.stringify({result:'PASS',...tutorialChecks},null,2));
    await evaluate(`SettingsUI.select(1);`);await capture('operations-narrow',520,'light');
    await evaluate(`SettingsUI.select(0);`);await capture('general-light',1180,'light');
    const chartPng = await evaluate(`(() => {
      const data={kind:'scatter',title:'參數切換點',x_label:'時間（秒）',y_label:'量測值',source:'合成資料',
        x:[0,100,200,300,400,500,600,700],series:[{name:'量測',values:[1,1.1,2,2.2,2.1,3,2.9,3.1]}],
        reference_lines:[{axis:'x',value:200,name:'參數1 → 2',color:'#d62728'},{axis:'x',value:500,name:'參數2 → 3',color:'#5470c6'}]};
      return ChartUI.exportPng(data);
    })()`);
    if(!chartPng.startsWith('data:image/png;base64,') || chartPng.length<1000) throw Error('real ECharts PNG missing');
    fs.writeFileSync('.build/settings-reference-lines.png',Buffer.from(chartPng.split(',')[1],'base64'));
    checks.push('real ECharts PNG with two parameter reference lines');
    fs.writeFileSync('.build/settings-review.json',JSON.stringify({result:'PASS',checks},null,2));console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
