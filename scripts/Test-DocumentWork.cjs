/* 0.9.3文件、工作區與流程提醒的介面回歸；不代替原生Office與真模型驗證。 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const {spawn} = require("node:child_process");
const {pathToFileURL} = require("node:url");
const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

(async () => {
  const profile = path.resolve(".build", "document-work-review-" + Date.now());
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

    const checks = await evaluate(`(async () => {
      const checks=[],check=(ok,label)=>{if(!ok)throw Error(label);checks.push(label);};
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());window.testCommands=[];send=m=>testCommands.push(m);
      const fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',active_id:'test-chat',
        conversations:[{id:'test-chat',title:'文件測試',project_id:'project',updated_at:1}],messages:[],
        models:[{id:'quality',label:'品質'}],config:{...LMUI.getState().config,model:'quality'},
        projects:{items:[{id:'project',name:'專案',root:'C:/fixture'}],running:false,status:'',activity:[]},
        work:{...LMUI.getState().work,mode:'stream',rules:{enabled:true,max_count:20,max_file_bytes:50000000,max_total_bytes:100000000,allowed_extensions:['.docx']},attachments:[],tasks:[]}};
      LMUI.receive(fixture);LMUI.showView('chat');
      const button=text=>[...document.querySelectorAll('button')].find(b=>b.textContent===text&&(!b.closest('dialog')||b.closest('dialog').open));
      check(!button('翻譯／比較／新建 Markdown').disabled,'專案可開啟文件工作');
      button('翻譯／比較／新建 Markdown').click();let dialog=document.querySelector('.workbench-dialog[open]');
      const inputs=dialog.querySelectorAll('input'),mode=dialog.querySelector('select');
      mode.value='compare';mode.dispatchEvent(new Event('change'));
      check([...dialog.querySelectorAll('p')].some(p=>!p.hidden&&p.textContent.includes('重複文字可能有多種對應')),'Word內容對齊與歧義限制在送出前直接顯示');
      mode.value='styled';inputs[0].value='source.docx';inputs[2].value='English';inputs[3].value='translated.docx';
      dialog.querySelector('textarea').value='LM=LM system';button('加入草稿，檢查後送出').click();
      check($('prompt').value.includes('preserve_styles')&&$('prompt').value.includes('LM system'),'格式翻譯與名詞表只填入可編輯草稿');
      check(!testCommands.some(m=>m.type==='send'),'文件表單不自動送出');
      button('翻譯／比較／新建 Markdown').click();dialog=document.querySelector('.workbench-dialog[open]');dialog.querySelector('select').value='markdown';dialog.querySelectorAll('input')[3].value='notes.md';button('加入草稿，檢查後送出').click();
      check($('prompt').value.includes('source為null')&&$('prompt').value.includes('受保護資料'),'新建Markdown保留原生副本與受保護資料限制');
      const selected=chooseFileDestination();dialog=[...document.querySelectorAll('dialog[open]')].find(d=>d.textContent.includes('選擇檔案處理位置'));
      check([...dialog.querySelectorAll('button')].find(b=>b.textContent==='上傳給公司網站').disabled,'專案不誤走網站附件');
      check(dialog.textContent.includes('100張')&&dialog.textContent.includes('公司模型'),'匯入目的地說明資料傳送與圖片上限');
      [...dialog.querySelectorAll('button')].find(b=>b.textContent==='取消').click();check(await selected===null,'取消位置選擇不傳檔');
      ReminderUI.schedule({id:'flow',name:'整理待辦'},'test-chat');ReminderUI.receive({type:'reminder_state',entries:[],acknowledgement:true});
      dialog=$('reminders-dialog');dialog.querySelector('select').value='7';dialog.querySelector('form').dispatchEvent(new Event('submit',{cancelable:true}));
      const reminder=testCommands.at(-1);check(reminder.type==='reminder'&&reminder.command.repeat_days===7&&reminder.command.workflow.template==='flow','流程提醒保存固定週期與範本身分');
      check(!('parameters' in reminder.command)&&!('execute' in reminder.command),'提醒不攜帶舊參數或自動執行權限');
      ReminderUI.receive({type:'reminder_state',acknowledgement:true,entries:[{id:1,title:'流程',due_at:2000000000,channel:'windows',notified_at:null,repeat_days:7,workflow:{conversation:'test-chat',template:'flow'}}]});button('載入流程草稿').click();
      check(testCommands.at(-1).command.action==='load_workflow','到期後需使用者載入流程');dialog.close();
      button('匯出整段對話 Word／PDF').click();dialog=document.querySelector('.workbench-dialog[open]');button('預覽匯出範圍').click();
      const preview=testCommands.at(-1);check(preview.command.conversation==='test-chat'&&preview.command.index===null&&!('content' in preview.command),'匯出讀原生歷史，不信任前端提供正文');
      WorkbenchUI.receive({type:'document_export_preview',conversation:'wrong',id:'stale',messages:1,blocks:1,warnings:[],folder:'wrong'});
      check(button('確認匯出副本').disabled,'錯誤對話的匯出預覽不能啟用保存');
      WorkbenchUI.receive({type:'document_export_preview',conversation:'test-chat',id:'native-preview',messages:2,blocks:8,warnings:['圖表未嵌入'],folder:'C:/fixture/_AI_Output',pdf:false});
      check(dialog.textContent.includes('圖表未嵌入')&&!button('確認匯出副本').disabled,'匯出前列出範圍、輸出目錄與省略內容');button('確認匯出副本').click();
      check(testCommands.at(-1).command.id==='native-preview'&&testCommands.at(-1).command.action==='save','保存需要原生預覽ID');
      check(button('確認匯出副本').disabled,'匯出進行中禁止重複保存');dialog.close();
      fixture.conversations[0].project_id=null;LMUI.receive(fixture);
      check(button('翻譯／比較／新建 Markdown').disabled&&!button('我的工作區').disabled,'一般對話可進入工作區但不能直接呼叫文件工具');
      check(!$('add-attachment').disabled,'一般對話可選擇網站或本機匯入');
      button('匯出整段對話 Word／PDF').click();
      return checks;
    })()`);
    for(const [name,width,theme] of [["light",1180,"light"],["dark",1180,"dark"],["narrow",520,"light"]]) {
      await call("Emulation.setDeviceMetricsOverride",{width,height:900,deviceScaleFactor:1,mobile:false});
      await evaluate("document.documentElement.dataset.theme="+JSON.stringify(theme));
      await delay(100);
      const bounds=await evaluate(`(() => {const d=document.querySelector('.workbench-dialog[open]');return {width:d.getBoundingClientRect().width,scroll:d.scrollWidth,client:d.clientWidth};})()`);
      if(bounds.width>width||bounds.scroll>bounds.client+1)throw Error("Document UI overflow: "+name);
      const shot=await call("Page.captureScreenshot",{format:"png",captureBeyondViewport:false});fs.writeFileSync(path.join(profile,name+".png"),Buffer.from(shot.data,"base64"));checks.push("版面 "+name);
    }
    fs.writeFileSync(path.join(profile,"result.json"),JSON.stringify({scope:"source_ui_only",checks},null,2));
    console.log(JSON.stringify({passed:checks.length,report:path.join(profile,"result.json")},null,2));
  } finally {if(socket)socket.close();edge.kill();}
})().catch(error=>{console.error(error);process.exitCode=1;});
