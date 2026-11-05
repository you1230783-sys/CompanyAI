/* 0.9.3文件、工作區與流程提醒的介面回歸；不代替原生Office與真模型驗證。 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const {spawn} = require("node:child_process");
const {pathToFileURL} = require("node:url");
const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

(async () => {
  const profile = path.resolve(".build", "schedule-review-" + Date.now());
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
      if(await evaluate("!!window.ScheduleUI && !!window.LMUI"))break;
      await delay(100);
    }

    const checks = await evaluate(`(() => {
      const checks=[],check=(ok,label)=>{if(!ok)throw Error(label);checks.push(label);};
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());window.testCommands=[];send=m=>testCommands.push(m);
      LMUI.receive({...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'chat'});
      const flow={id:'flow',name:'固定流程',parameters:[],steps:1};
      ScheduleUI.open(flow,'chat',{source:'source.md'});
      check($('schedule-confirm').disabled,'沒有原生預覽不能確認');
      $('schedule-preview').click();
      const request=testCommands.at(-1).command;
      check(request.action==='preview' && request.parameters.source==='source.md','預覽僅送固定參數與範本身分');
      ScheduleUI.receive({type:'schedule_preview',id:'old',request_id:'stale',grant:{},root:'C:/fixture',notice:'old'});
      check($('schedule-confirm').disabled,'過期預覽回覆不啟用確認');
      ScheduleUI.receive({type:'schedule_preview',id:'approved',request_id:request.request_id,grant:{template:{steps:[{request:{tool:'read_file',path:'source.md'}}]},parameters:request.parameters},root:'C:/fixture',notice:'核對固定範圍'});
      check(!$('schedule-confirm').disabled && $('schedule-plan').textContent.includes('source.md'),'原生完整步驟與來源可見');
      $('schedule-confirm').click();
      check(testCommands.at(-1).command.action==='confirm' && testCommands.at(-1).command.preview_id==='approved','確認只送原生一次性預覽ID');
      check($('schedule-confirm').disabled,'確認送出後避免重複提交');
      $('schedule-create').close();
      ScheduleUI.open(flow,'chat',{});$('schedule-preview').click();
      const second=testCommands.at(-1).command;
      ScheduleUI.receive({type:'schedule_preview',id:'second',request_id:second.request_id,grant:{},root:'C:/fixture',notice:'核對'});
      const due=$('schedule-create').querySelector('input');due.value='2099-01-01T12:00';due.dispatchEvent(new Event('input'));
      check($('schedule-confirm').disabled,'編輯時間使預覽失效');
      $('schedule-create').close();ScheduleUI.showList();
      ScheduleUI.receive({type:'schedule_state',running:true,entries:[{id:'schedule',grant:{template:{name:'固定流程'}},status:'執行中',enabled:true,next_due:2000,execution:'execution'}],audit:[]});
      const stop=[...$('schedule-list').querySelectorAll('button')].find(b=>b.textContent==='停止目前排程');stop.click();
      check(testCommands.at(-1).command.action==='stop','執行中的排程可以停止');
      ScheduleUI.receive({type:'schedule_state',running:false,entries:[{id:'schedule',grant:{template:{name:'固定流程'}},status:'未知',enabled:false,next_due:2000,execution:'execution'}],audit:[{schedule:'schedule',execution:'execution',status:'執行結果未知，禁止重播',started_at:1000,next_step:1,outputs:['_AI_Output/result.md']}]});
      check(![...$('schedule-list').querySelectorAll('button')].some(b=>b.textContent.includes('續接')),'未知結果不提供續接入口');
      check($('schedule-list').textContent.includes('_AI_Output/result.md'),'審計保留成果位置');
      ScheduleUI.receive({type:'show_schedule_audit'});
      ScheduleUI.receive({type:'schedule_state',running:false,entries:[{id:'schedule',grant:{template:{name:'固定流程'}},status:'已略過',enabled:true,next_due:87400,execution:null}],audit:[{schedule:'schedule',execution:'skipped',status:'執行前條件不符，已略過本次',started_at:1000,next_step:0,outputs:[],detail:'Office正在使用中'}]});
      check($('schedule-list').open && $('schedule-list').querySelector('details').open && $('schedule-list').textContent.includes('Office正在使用中'),'通知直接開啟略過原因審計並保留下次');
      return checks;
    })()`);
    for(const width of [1180,520]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});
      const bounds=await evaluate(`(() => {const d=$('schedule-list');return {width:d.getBoundingClientRect().width,scroll:d.scrollWidth,client:d.clientWidth};})()`);
      if(bounds.width>width||bounds.scroll>bounds.client+1)throw Error('Schedule UI overflow '+width);
      checks.push('排程清單版面 '+width);
    }
    fs.writeFileSync('.build/schedule-ui-verification.json',JSON.stringify({result:'PASS',scope:'source_ui_only',checks},null,2));
    console.log(JSON.stringify({passed:checks.length,checks},null,2));
  } finally {if(socket)socket.close();edge.kill();}
})().catch(error=>{console.error(error);process.exitCode=1;});
