/* 九項改善的來源介面回歸。可獨立執行非CAD介面驗證；此腳本不代替原生／真模型驗證。 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const {spawn} = require("node:child_process");
const {pathToFileURL} = require("node:url");
const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

(async () => {
  const profile = path.resolve(".build", "workbench-review-" + Date.now());
  fs.mkdirSync(profile, {recursive:true});
  const edge = spawn("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe", [
    "--headless=new", "--no-first-run", "--disable-gpu", "--remote-debugging-port=0",
    "--user-data-dir=" + profile, "about:blank",
  ], {windowsHide:true, stdio:"ignore"});
  let socket;
  try {
    const endpoint = path.join(profile, "DevToolsActivePort");
    for (let index=0; index<100 && !fs.existsSync(endpoint); index++) await delay(100);
    const port = fs.readFileSync(endpoint, "utf8").split("\n")[0];
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    socket = new WebSocket(pages.find(page => page.type === "page").webSocketDebuggerUrl);
    await new Promise((resolve,reject) => { socket.onopen=resolve; socket.onerror=reject; });
    let sequence=0;
    const pending = new Map();
    socket.onmessage = event => {
      const message=JSON.parse(event.data), item=pending.get(message.id);
      if (!item) return;
      pending.delete(message.id);
      message.error ? item.reject(message.error) : item.resolve(message.result);
    };
    const call = (method, params={}) => new Promise((resolve,reject) => {
      const id=++sequence; pending.set(id,{resolve,reject});
      socket.send(JSON.stringify({id,method,params}));
    });
    const evaluate = async expression => {
      const result=await call("Runtime.evaluate",{expression,returnByValue:true,awaitPromise:true});
      if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails));
      return result.result.value;
    };
    await call("Page.navigate",{url:pathToFileURL(path.resolve("ui/index.html")).href});
    for(let index=0;index<100;index++) {
      if(await evaluate("!!window.WorkbenchUI && !!window.LMUI && !!window.ArtifactUI"))break;
      await delay(100);
    }
    // 所有命令攔在測試頁，不會登入、執行工具、改真偏好或寫入專案。
    const checks = await evaluate(`(() => {
      const checks=[],check=(condition,label)=>{if(!condition)throw Error(label);checks.push(label);};
      document.querySelectorAll('dialog[open]').forEach(dialog=>dialog.close());
      window.testCommands=[];send=message=>testCommands.push(message);
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'test-chat',
        conversations:[{id:'test-chat',title:'測試',project_id:'project',updated_at:1}],messages:[],
        models:[{id:'quality',label:'品質'}],config:{...LMUI.getState().config,model:'quality'},
        projects:{items:[],running:false,status:'',activity:[]}};
      LMUI.receive(fixture);LMUI.showView('chat');
      // 用實際訊息重繪核對三種資訊都在同一個預設收合區塊內。
      fixture.messages=[{role:'assistant',content:'已完成測試',project_activity:['測試工具完成'],
        project_analysis:{deliverables:{items:[{description:'測試交付',status:'delivered'}]}}}];
      LMUI.receive(fixture);
      const projectDetails=document.querySelector('#messages .message-project-details');
      check(projectDetails && !projectDetails.open && projectDetails.querySelector('.project-activity') &&
        projectDetails.querySelector('.delivery-card') && projectDetails.querySelector('.analysis-disclosure'),
        '工具紀錄、交付清單與分析共同預設收在詳情內容');
      projectDetails.open=true;
      fixture.messages[0].content='更新測試成果';LMUI.receive(fixture);
      check(document.querySelector('#messages .message-project-details').open,'訊息重繪保留使用者展開詳情的選擇');
      fixture.messages=[];LMUI.receive(fixture);
      fixture.projects.items=[{id:'project',name:'專案一',root:'C:/fixture'}, {id:'project-two',name:'專案二',root:'C:/fixture2'}];
      fixture.conversations.push({id:'chat-two',title:'第二個對話',project_id:'project-two',updated_at:1});
      LMUI.receive(fixture);
      document.querySelector('.project-title-toggle').click();
      check($('history-filter').value==='project:project' && !!document.querySelector('#history-list .history-item[data-id="test-chat"]'),
        '點擊專案名稱篩選統一對話清單');
      check(!document.querySelector('#project-list .history-item') && !document.querySelector('#history-list .history-item[data-id="chat-two"]') && LMUI.getState().active_id==='test-chat',
        '篩選不重複列出對話也不切換目前對話');
      fixture.projects.running=true;LMUI.receive(fixture);
      check(!document.querySelector('.project-title-toggle').disabled,'執行任務時仍可篩選對話');
      fixture.projects.running=false;LMUI.receive(fixture);filterHistory('all');

      const history=document.createElement('details');history.open=true;document.body.append(history);
      const events=Array.from({length:350},(_,index)=>({at:'2026-11-02 13:00:00',text:'紀錄 '+index}));
      renderProjectActivity(history,events);
      check(history.querySelectorAll('li').length===120 && history.querySelector('li').textContent.endsWith('紀錄 230'),
        '長歷史只渲染最新一頁而非丟棄原文');
      history.querySelector('[data-page="older"]').click();history.querySelector('[data-page="older"]').click();
      check(history.querySelector('li').textContent.endsWith('紀錄 0'),'可查閱超過120筆以前的第一筆紀錄');
      renderProjectActivity(history,[...events,{text:'新增紀錄'}]);
      check(history.querySelector('li').textContent.endsWith('紀錄 0'),'新進度不會把正在看的舊頁切回最後');
      history.querySelector('[data-page="latest"]').click();
      check(history.querySelector('li:last-child').textContent==='新增紀錄' && history.querySelectorAll('li').length===120,
        '可直接回到最新進度且DOM仍維持單頁');
      history.remove();
      const empty={language:'',decimals:null,date_format:'',chart_background:'',notes:''};
      document.getElementById('work-settings-open').click();
      const request=testCommands.at(-1).command;
      WorkbenchUI.receive({type:'work_settings',conversation:'test-chat',request_id:'stale',data:{}});
      check(document.querySelector('.workbench-dialog').textContent.includes('正在讀取'),'舊回應不能覆蓋設定');
      WorkbenchUI.receive({type:'work_settings',conversation:'test-chat',request_id:request.request_id,
        data:{personal:empty,project:empty,conditions:{revision:2,text:'排除缺值'},fingerprint:'version-2',is_project:true}});
      const dialog=document.querySelector('.workbench-dialog'),scope=dialog.querySelector('select');
      dialog.querySelector('input').value='繁體中文';scope.value='personal';scope.dispatchEvent(new Event('change'));
      dialog.querySelector('input').value='English';
      [...dialog.querySelectorAll('button')].find(button=>button.textContent==='保存').click();
      const saved=testCommands.at(-1).command;
      check(saved.project.language==='繁體中文' && saved.personal.language==='English','切換範圍保留兩份編輯');
      check(saved.expected==='version-2' && saved.conditions.revision===2,'保存帶原偏好與條件版本');
      dialog.close();

      const prompt=document.getElementById('prompt');prompt.value='保留這段';
      const commandCount=testCommands.length;
      WorkbenchUI.correct({conversation:'test-chat',kind:'document_block',path:'_AI_Output/run/report.docx',
        revision:'old-revision',block_label:'表1列2欄3',selected_text:'原數字'});
      const correction=document.querySelector('.workbench-dialog[open]');
      correction.querySelector('textarea').value='改成新的計算值';
      [...correction.querySelectorAll('button')].find(button=>button.textContent==='加入提問草稿').click();
      check(prompt.value.startsWith('保留這段') && prompt.value.includes('old-revision') && prompt.value.includes('表1列2欄3'),
        '定點修改保留原草稿、版本及位置');
      check(testCommands.length===commandCount,'修改內容只加入草稿，不執行工具');

      WorkbenchUI.workflow({id:'flow',name:'分析',steps:1,plan:[],parameters:[
        {name:'file',kind:'string',example:'old.csv',uses:['來源']},
        {name:'start',kind:'number',example:0,uses:['起始位置']}]},'test-chat');
      const workflow=document.querySelector('.workbench-dialog[open]');
      const add=[...workflow.querySelectorAll('button')].find(button=>button.textContent==='加入提問草稿');
      add.click();check(workflow.textContent.includes('請填寫 file'),'流程禁止空白來源');
      const inputs=workflow.querySelectorAll('input');inputs[0].value='new.csv';inputs[1].value='3';add.click();
      check(prompt.value.includes('"start":3') && prompt.value.includes('new.csv'),'流程參數保留數字型別');
      check(testCommands.length===commandCount,'流程表單不自動執行');

      const card=document.createElement('div');
      WorkbenchUI.delivery(card,{items:[{description:'報告',status:'blocked',blocked_reason:'檔案使用中'},
        {description:'圖表',status:'delivered',path:'chart.png'}],review_required:true});
      check(card.textContent.includes('未完成') && card.textContent.includes('檔案使用中') && card.textContent.includes('重新核對'),
        '部分完成與條件變更不被隱藏');
      document.getElementById('project-artifacts').click();
      const artifacts=testCommands.at(-1).command;
      ArtifactUI.receive({type:'project_artifact_center',conversation:'test-chat',request_id:artifacts.request_id,
        ok:true,text:JSON.stringify({offset:0,next_offset:0,total:0,artifacts:[],workflows:[],selected_versions:{},
          runs:[{id:'run',request:'尚未交付的任務',updated:1,state:'partial',delivery:{items:[{description:'Word',status:'draft'}]}}]})});
      check(document.getElementById('project-artifact-list').textContent.includes('尚未交付的任務'),
        '沒有成品的任務仍能看到草稿清單');
      LMUI.receive({...fixture,active_id:'other-chat',conversations:[{id:'other-chat',project_id:'project',title:'另個對話'}]});
      check(!document.getElementById('project-artifact-dialog').open,'切換對話關閉舊成果中心');
      return checks;
    })()`);
    // 補看新表單在明暗主題及窄視窗的實際渲染，不只驗證命令資料。
    await evaluate(`(() => {
      document.getElementById('work-settings-open').click();
      const request=testCommands.at(-1).command;
      const empty={language:'繁體中文',decimals:2,date_format:'YYYY-MM-DD',chart_background:'white',notes:'保留未完成項目'};
      WorkbenchUI.receive({type:'work_settings',conversation:request.conversation,request_id:request.request_id,
        data:{personal:empty,project:empty,conditions:{revision:2,text:'排除缺值，交付Word與Excel'},fingerprint:'visual',is_project:true}});
    })()`);
    for(const [name,width,theme] of [["light",1180,"light"],["dark",1180,"dark"],["narrow",520,"light"]]) {
      await call("Emulation.setDeviceMetricsOverride",{width,height:900,deviceScaleFactor:1,mobile:false});
      await evaluate("document.documentElement.dataset.theme="+JSON.stringify(theme));
      await delay(100);
      const bounds=await evaluate(`(() => {
        const dialog=document.querySelector('.workbench-dialog[open]');
        return {width:dialog.getBoundingClientRect().width,scroll:dialog.scrollWidth,client:dialog.clientWidth};
      })()`);
      if(bounds.width>width || bounds.scroll>bounds.client+1)throw Error("Workbench overflow: "+name);
      const screenshot=await call("Page.captureScreenshot",{format:"png",captureBeyondViewport:false});
      fs.writeFileSync(path.join(profile,name+".png"),Buffer.from(screenshot.data,"base64"));
      checks.push("表單版面 "+name);
    }
    fs.writeFileSync(path.join(profile,"result.json"),JSON.stringify({scope:"source_ui_only",checks},null,2));
    console.log(JSON.stringify({passed:checks.length,report:path.join(profile,"result.json")},null,2));
  } finally {
    if(socket)socket.close();
    edge.kill();
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
