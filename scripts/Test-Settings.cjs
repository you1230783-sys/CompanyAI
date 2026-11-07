/* 原始碼頁面的互動／版面回歸；不編譯、不代替 WebView2 原生驗收。從專案根目錄執行 node scripts/Test-Settings.cjs。 */
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {pathToFileURL} = require('node:url');
// 更新公告只保留使用者操作資訊；集中維護詞彙，避免日後再次加入內部細節。
const releaseNotes = JSON.parse(fs.readFileSync('assets/release-notes.json', 'utf8').replace(/^\uFEFF/, ''));
const excludedReleaseTerms = ['私聊', '好友', '聊天 ID', '塔羅', '桌寵', '牌義', '抽牌', '回歸', '候選', 'NSIS', 'v142', 'ThinLTO', 'codegen', 'panic', 'AppContainer', 'DPAPI', 'SHA256', 'JSON Pointer', 'tokens', 'Cargo', 'COM', 'GitHub', '簽章', '雜湊', 'Junction', 'ACL'];
for (const note of releaseNotes) {
  const text = [note.section, ...note.items].join('\n').toLowerCase();
  for (const term of excludedReleaseTerms) {
    if (text.includes(term.toLowerCase())) throw Error(`Release ${note.version} contains internal wording: ${term}`);
  }
  if (!note.section || note.items.length < 1 || note.items.length > 4 || note.items.some(item => !item || [...item].length > 60)) {
    throw Error(`Release ${note.version} must have a title and one to four short user-facing sentences`);
  }
}
for (const version of ['0.8.75', '0.8.76', '0.8.77', '0.8.78', '0.8.79', ...Array.from({length:27}, (_, i) => `0.9.${i}`)]) {
  if (releaseNotes.filter(note => note.version === version).length !== 1) throw Error(`Missing or duplicated release ${version}`);
}
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
        config:{...LMUI.getState().config,debug_mode:true,model:'quality',extra_features_enabled:true,vnc_enabled:true},
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
      check(tabs.map(t=>t.textContent.trim()).join(',')==='一般,通知,操作,進階,教學與更新日誌','notification tab follows general');
      tabs[0].dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
      const mode = $('notification-mode'), pet = mode.querySelector('[value=pet]');
      check(document.activeElement===tabs[1] && !$('settings-panel-notifications').hidden,'notification tab keyboard access');
      const toggles = [...document.querySelectorAll('[data-notice-category]')].filter(input => !input.closest('#private-chat-notices'));
      check(toggles.length===9 && new Set(toggles.map(input=>input.dataset.noticeCategory)).size===9,'all popup sources have independent toggles');
      testFixture.config.notification_preferences={mode:'legacy',categories:{},legacy_popups:false};
      testFixture.build_features={cad:true,tarot:true,vnc:true,pet:false};LMUI.receive(testFixture);
      check(mode.value==='legacy' && pet.hidden && pet.disabled,'legacy mode shown; missing pet feature hides option');
      check(toggles.filter(input=>input.checked).map(input=>input.dataset.noticeCategory).join(',')==='reminder,tray_hint','legacy disabled setting keeps reminders and tray hint');
      testFixture.build_features.pet=true;testFixture.config.extra_features_enabled=false;LMUI.receive(testFixture);
      check(pet.hidden && pet.disabled,'disabled extra features hide pet option');
      testFixture.config.extra_features_enabled=true;LMUI.receive(testFixture);
      check(!pet.hidden && !pet.disabled,'both flags expose pet option');
      mode.value='off';mode.dispatchEvent(new Event('change'));
      check(testCommands.at(-1).type==='notification_mode' && testCommands.at(-1).mode==='off','master preference routes to native code');
      for (const toggle of toggles) {
        toggle.click();
        const command = testCommands.at(-1);
        check(command.type==='notification_category' && command.category===toggle.dataset.noticeCategory
          && command.enabled===toggle.checked,'independent native preference: '+toggle.dataset.noticeCategory);
      }
      const chatState = {type:'private_chat_state',available:true,enabled:true};
      testFixture.private_chat_available=false;LMUI.receive(testFixture);PrivateChatUI.receive(chatState);
      check($('show-private-chat').hidden && $('private-chat-settings').hidden,'private chat requires native secure server capability');
      testFixture.private_chat_available=true;testFixture.config.extra_features_enabled=false;LMUI.receive(testFixture);
      check($('private-chat-settings').hidden,'private chat requires extra features');
      testFixture.config.extra_features_enabled=true;LMUI.receive(testFixture);
      PrivateChatUI.receive({...chatState,enabled:false});
      check(!$('private-chat-settings').hidden && $('show-private-chat').hidden,'setting is offered before enabling; entry is hidden');
      PrivateChatUI.receive(chatState);
      check(!$('show-private-chat').hidden && !$('private-chat-lock-settings').hidden,'enabled account exposes entry and lock settings');
      check(!$('private-chat-notices').hidden && $('private-chat-notice-preview').value==='generic','chat notices default to generic while chat is enabled');
      $('private-chat-notice-preview').value='detail';
      $('private-chat-notice-preview').dispatchEvent(new Event('change'));
      check(testCommands.at(-1).type==='chat_notification_preview' && testCommands.at(-1).enabled,'chat preview switch goes to native preferences');
      $('settings-dialog').close();
      PrivateChatUI.receive({...chatState,authenticate:true,has_password:false});
      const password = $('private-chat-password'), passwordInput=password.querySelector('[name=password]');
      check(password.open && $('private-chat-password-title').textContent==='設定聊天密碼','password setup blocks entry');
      passwordInput.value='abc中123';passwordInput.dispatchEvent(new Event('input'));
      check(password.querySelector('[data-password-status]').textContent.includes('中'),'invalid password character reported while typing');
      passwordInput.value='Aa1234';password.querySelector('[name=repeat]').value='Aa1234';
      password.querySelector('form').requestSubmit();
      check(testCommands.at(-1).command.action==='unlock' && passwordInput.value==='','native unlock and immediate input clearing');
      const contacts={profile:{chat_id:'0123-ABCD'},friends:[{friend_id:'friend',nickname:'<script>synthetic</script>',employee_no:'100',status:'active'}],
        invites:{incoming:[{invite_id:'sample-invite',nickname:'來電者',employee_no:'200'}],outgoing:[]},blocked:[]};
      PrivateChatUI.receive({...chatState,open:true,contacts});
      const pane=$('private-chat'), add=$('private-chat-contacts');
      check(pane.open && !password.open && !pane.querySelector('script'),'friend label is plain text in independent chat view');
      pane.querySelector('[data-add]').click();
      check(add.open && add.querySelector('[data-id]').textContent==='0123-ABCD','add friend opens with current chat ID');
      add.querySelector('[name=code]').value='4567-EFGH';add.querySelector('[data-lookup]').requestSubmit();
      check(testCommands.at(-1).command.action==='lookup' && add.querySelector('[data-invite]').hidden,'search does not send invitation');
      PrivateChatUI.receive({...chatState,open:true,contacts,lookup:{nickname:'已找到',employee_no:'300'}});
      add.querySelector('[data-invite]').click();
      check(testCommands.at(-1).command.action==='invite','only confirmed lookup sends invitation');
      add.querySelector('[aria-label="加入黑名單"]').click();
      check(testCommands.at(-1).command.response==='block','incoming invite can be blocked');
      add.close();
      pane.querySelector('[data-friends] button').click();
      check(testCommands.at(-1).command.command.action==='select','select friend binds private message thread');
      const composer=pane.querySelector('.private-chat-composer'), chatInput=composer.querySelector('textarea');
      chatInput.value='hello';chatInput.dispatchEvent(new Event('input'));composer.requestSubmit();
      check(testCommands.at(-1).command.command.action==='send_text' && testCommands.at(-1).command.command.peer==='friend','text composer never uses AI send');
      composer.querySelector('[data-choose]').click();
      check(testCommands.at(-1).command.command.action==='choose_image','file button uses native picker');
      const clipboard=new DataTransfer();
      clipboard.items.add(new File([Uint8Array.of(1,2,3)],'test.png',{type:'image/png'}));
      chatInput.dispatchEvent(new ClipboardEvent('paste',{clipboardData:clipboard,bubbles:true,cancelable:true}));
      check(testCommands.at(-1).command.command.action==='paste_image','Ctrl+V image paste routes to native clipboard');
      PrivateChatUI.receive({type:'private_chat_image_draft',peer:'friend',bytes:300});
      check(!composer.querySelector('[data-draft-image]').hidden,'image stays staged until explicit send');
      const literal='[x](https://example.test) <script>forged()</script>';
      PrivateChatUI.receive({type:'private_chat_timeline',peer:'friend',bytes:300,messages:[
        {id:'text',kind:'text',state:'delivered',outgoing:true,time:'2026-11-07T08:00:00Z',text:literal},
        {id:'failed',kind:'text',state:'rejected',outgoing:true,text:'wait',reason:'對方待收訊息已達上限'},
        {id:'expired',kind:'expired_image',state:'received',outgoing:false,time:'2026-11-01T08:00:00Z'},
        {id:'image',kind:'image',state:'received',outgoing:false},
      ]});
      const thread=pane.querySelector('[data-thread]');
      check(thread.textContent.includes(literal) && !thread.querySelector('a,script'),'HTML, URL and Markdown remain plain text');
      check(thread.textContent.includes('送達') && thread.textContent.includes('已達上限') && thread.textContent.includes('原類型：圖片'),'delivery, rejection and expiry labels rendered');
      thread.querySelector('[data-retry]').click();
      check(testCommands.at(-1).command.command.id==='failed','retry uses existing local message ID');
      pane.querySelector('footer > button').click();
      const space=$('private-chat-space');
      check(space.open && space.querySelector('[data-period]').options.length===6,'space manager offers all cleanup periods');
      space.querySelector('[data-only-images]').click();
      check(space.querySelector('[data-period]').options[0].disabled,'all period is restricted to image cleanup');
      space.querySelector('[data-clean]').click();$('confirm-ok').click();
      check(testCommands.at(-1).command.command.action==='cleanup' && testCommands.at(-1).command.command.confirmed,'cleanup asks for confirmation');
      space.close();
      contacts.friends[0].status='offline';PrivateChatUI.receive({...chatState,open:true,contacts});
      check(chatInput.disabled,'offline friend disables sending');
      PrivateChatUI.receive({...chatState,open:false});
      check(!pane.open && !add.open && !pane.textContent.includes('synthetic') && add.querySelector('[data-id]').textContent==='' && !thread.textContent,'lock clears dialogs, contact and message DOM');
      testFixture.private_chat_available=false;LMUI.receive(testFixture);
      $('settings-dialog').showModal();
      $('hotkey').value='Win+F11';tabs[2].click();tabs[3].click();tabs[0].click();
      check($('hotkey').value==='Win+F11','tabs preserve input drafts');
      tabs[0].dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));
      check(!$('settings-panel-3').hidden && $('settings-panel-0').hidden,'End selects the last tutorial tab');
      check($('tutorial-catalog').closest('#settings-panel-3') && $('updates-replay').closest('#settings-panel-3'),'tutorials and release history have their own panel');
      check(document.querySelectorAll('#tutorial-catalog .tutorial-topic').length===58,'all 58 interactive tutorial topics rendered');
      check(!$('release-history'),'no duplicate inline release history');
      tabs.at(-1).dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true}));
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
    await evaluate(`$('project-diagnostics-dialog').close();$('settings-dialog').showModal();SettingsUI.select('settings-tab-2');`);
    await capture('advanced-light',1180,'light');await capture('advanced-dark',1180,'dark');await capture('advanced-narrow',520,'light');
    await evaluate(`SettingsUI.select('settings-tab-3');`);await capture('tutorials-narrow',520,'light');
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
      check(ProjectTutorial.catalog.length===58,'runtime tutorial catalogue contains all 58 topics');
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
      check(new Set(finished).size===58,'all topics finish exactly once');
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
    checks.push('all 58 tutorials completed in real browser; input/select/checkbox gates, focus, isolation and search');
    fs.writeFileSync('.build/tutorials-review.json',JSON.stringify({result:'PASS',...tutorialChecks},null,2));
    await evaluate(`SettingsUI.select('settings-tab-1');`);await capture('operations-narrow',520,'light');
    await evaluate(`SettingsUI.select('settings-tab-notifications');`);await capture('notifications-narrow',520,'light');
    await evaluate(`SettingsUI.select('settings-tab-0');`);await capture('general-light',1180,'light');
    const chartPng = await evaluate(`(() => {
      const data={kind:'scatter',title:'參數切換點',x_label:'時間（秒）',y_label:'量測值',source:'合成資料',
        x:[0,100,200,300,400,500,600,700],series:[{name:'量測',values:[1,1.1,2,2.2,2.1,3,2.9,3.1]}],
        reference_lines:[{axis:'x',value:200,name:'參數1 → 2',color:'#d62728'},{axis:'x',value:500,name:'參數2 → 3',color:'#5470c6'}]};
      return ChartUI.exportPng(data);
    })()`);
    if(!chartPng.startsWith('data:image/png;base64,') || chartPng.length<1000) throw Error('real ECharts PNG missing');
    fs.writeFileSync('.build/settings-reference-lines.png',Buffer.from(chartPng.split(',')[1],'base64'));
    checks.push('real ECharts PNG with two parameter reference lines');
    const retryChecks = await evaluate(`(async () => {
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      const check=(ok,label)=>{if(!ok)throw Error(label);};
      window.testCommands=[];send=message=>testCommands.push(message);
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'review-retry',
        config:{...LMUI.getState().config,model:'quality'},
        conversations:[{id:'review-retry',project_id:'project',title:'重試測試'}],
        messages:[{role:'assistant',content:'可改為今晚批次處理',project_paused:true,request_id:'paused-run'}],
        retry:{enabled:true},projects:{items:[],running:false,batch:{entries:[]},retry_fast:true}};
      LMUI.receive(fixture);LMUI.showView('chat');
      const later=[...document.querySelectorAll('button')].find(b=>b.textContent==='稍後手動繼續');
      check(!!later,'稍後手動繼續按鈕存在');later.click();
      check(testCommands.length===0 && !document.querySelector('.fast-model-dialog'),'稍後不送出或確認任何任務');
      check(document.querySelector('.resume-project').textContent==='繼續','保留真正的手動續接入口');
      document.querySelector('.resume-project').click();
      check(!!document.querySelector('.fast-model-dialog') && testCommands.length===0,'原請求為快速模型時仍須確認，即使目前已改品質模型');
      document.querySelector('.fast-model-dialog .secondary-button').click();
      await new Promise(r=>setTimeout(r,30));
      check(testCommands.length===0,'取消快速模型確認維持暫停');
      document.querySelector('.resume-project').click();
      document.querySelector('.fast-model-dialog .primary-button').click();
      await new Promise(r=>setTimeout(r,30));
      check(testCommands.length===1 && testCommands[0].command.action==='resume' && testCommands[0].command.run_id==='paused-run','確認後才續接原任務');
      check(document.querySelector('#private-chat-settings').textContent.includes('只有程式意外結束時，才會在上鎖期限內免輸入'),'自動上鎖說明包含完整退出及意外復原規則');
      return '稍後不執行、原快速模型確認／取消／續接、完整退出設定說明';
    })()`);
    checks.push(retryChecks);
    fs.writeFileSync('.build/settings-review.json',JSON.stringify({result:'PASS',checks},null,2));console.log(JSON.stringify({result:'PASS',checks}));
  } finally {if(ws)ws.close();edge.kill();}
})().catch(error=>{process.stderr.write(String(error.stack||error));process.exitCode=1;});
