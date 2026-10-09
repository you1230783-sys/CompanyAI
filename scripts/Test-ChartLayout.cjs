/* 原始碼瀏覽器回歸：真實滑鼠拖曳、色票、取消／保存、縮放及PNG。禁止以此冒稱原生保存已驗收。 */
"use strict";
const fs=require("node:fs"), path=require("node:path"), {spawn}=require("node:child_process"), {pathToFileURL}=require("node:url");
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
(async()=>{
  const profile=path.resolve(".build","chart-layout-"+Date.now());fs.mkdirSync(profile,{recursive:true});
  const browser=spawn("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    ["--headless=new","--no-first-run","--disable-gpu","--remote-debugging-port=0","--user-data-dir="+profile,"about:blank"],{windowsHide:true,stdio:"ignore"});
  let ws;
  try {
    for(let i=0;i<100&&!fs.existsSync(path.join(profile,"DevToolsActivePort"));i++)await delay(100);
    const port=fs.readFileSync(path.join(profile,"DevToolsActivePort"),"utf8").split("\n")[0];
    const pages=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    ws=new WebSocket(pages.find(page=>page.type==="page").webSocketDebuggerUrl);
    await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
    let sequence=0;const pending=new Map();
    ws.onmessage=event=>{const message=JSON.parse(event.data);if(!message.id)return;const item=pending.get(message.id);pending.delete(message.id);message.error?item.reject(message.error):item.resolve(message.result);};
    const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
    const evaluate=async expression=>{const result=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true});if(result.exceptionDetails)throw Error(JSON.stringify(result.exceptionDetails));return result.result.value;};
    const capture=async name=>{const png=await call("Page.captureScreenshot",{format:"png",captureBeyondViewport:false});fs.writeFileSync(`.build/chart-layout-${name}.png`,Buffer.from(png.data,"base64"));};
    await call("Emulation.setDeviceMetricsOverride",{width:1180,height:850,deviceScaleFactor:1,mobile:false});
    await call("Page.navigate",{url:pathToFileURL(path.resolve("ui/index.html")).href});
    for(let i=0;i<100;i++){if(await evaluate("!!window.SettingsUI && !!window.ChartLayout"))break;await delay(100);}
    const checks=[];
    await evaluate(`(() => {
      window.check=(ok,message)=>{if(!ok)throw Error(message);};window.sent=[];send=message=>sent.push(message);
      window.fixture={...LMUI.getState(),logged_in:true,can_send:true,busy:'none',models:[],messages:[],
        config:{...LMUI.getState().config,chart_palette:ChartAppearance.defaults()}};
      LMUI.receive(fixture);document.querySelectorAll('dialog[open]').forEach(d=>d.close());$('settings-dialog').showModal();SettingsUI.select(0);
      const inputs=[...$('chart-palette').querySelectorAll('input')];check(inputs.length===8,'eight palette entries');
      inputs[0].value='#c02090';inputs[0].dispatchEvent(new Event('input'));LMUI.receive(fixture);
      check(inputs[0].value==='#c02090','incoming state preserves color draft');$('chart-palette-save').click();
      check(sent.at(-1).type==='chart_palette'&&sent.at(-1).colors[0]==='#c02090','color save command');
      fixture.config.chart_palette=sent.at(-1).colors;LMUI.receive(fixture);
      check(ChartAppearance.palette()[0]==='#c02090','saved palette becomes default');
      $('chart-palette-reset').click();check(ChartAppearance.palette()[0]==='#c02090','reset is a draft until saved');
    })()`);
    await capture("palette");checks.push("palette draft, persistence command and custom colors priority");
    await evaluate(`(() => {
      document.querySelectorAll('dialog[open]').forEach(d=>d.close());
      window.host=document.createElement('div');host.style.cssText='position:fixed;inset:0;overflow:auto;background:white;z-index:9999;padding:12px';document.body.append(host);
      window.data={kind:'scatter',title:'密集切換點與量測範圍',x_label:'時間（秒）',y_label:'量測',source:'合成測試資料',x:[0,100,200,300,400,500],
        series:[{name:'量測A',values:[90,99,100,100.3,102,110]},{name:'量測B',values:[91,97,102,101,104,109]}],reference_lines:[
        {axis:'x',value:200,name:'參數1 → 2，開始升溫',color:'#b52030'},
        {axis:'x',value:201,name:'參數2 → 3，增加速度',color:'#145fbd'},
        {axis:'x',value:202,name:'參數3 → 4，量測條件切換',color:'#26733b'},
        {axis:'y',value:100,name:'目標100',color:'#8048b0'},
        {axis:'y',value:100.1,name:'上限100.1',color:'#995500'},
        {axis:'y',value:100.2,name:'上限100.2',color:'#b52030'},
        {axis:'y',value:105,name:'',color:'#b52030'}]};
      window.original=JSON.stringify(data);window.chartContext={conversation:'fixture',request_id:'fixture-run',message_index:0,styles:{}};
      window.renderChart=style=>{chartContext.styles=style?{0:style}:{};host.dataset.chartSignature='';ChartUI.render(host,[data],chartContext);};
      window.button=text=>[...host.querySelectorAll('button')].find(button=>button.textContent===text);
      renderChart(null);
    })()`);
    await delay(250);
    // 用真正的訊息渲染入口確認：只有設定改變時，既有未自訂畫布也必須刷新。
    await evaluate(`fixture.active_id='palette-fixture';fixture.messages=[{role:'assistant',content:'配色測試',request_id:'palette-run',project_charts:[data],project_chart_styles:{}}];LMUI.receive(fixture);`);
    await delay(100);
    await evaluate(`fixture.config.chart_palette=[...fixture.config.chart_palette];fixture.config.chart_palette[0]='#2266cc';LMUI.receive(fixture);`);
    await delay(100);
    await evaluate(`check(echarts.getInstanceByDom(document.querySelector('#messages .chart-plot')).getOption().series[0].itemStyle.color==='#2266cc','existing message chart reacts to palette-only change');fixture.config.chart_palette[0]='#c02090';LMUI.receive(fixture);`);
    await delay(100);
    await evaluate(`(() => {
      window.plot=host.querySelector('.chart-plot');window.instance=echarts.getInstanceByDom(plot);
      window.result=ChartLayout.draw(instance,ChartEditor.defaults(data));
      check(!result.crowded && result.boxes.length===8,'all six visible labels retained');
      for(let i=0;i<result.boxes.length;i++)for(let j=i+1;j<result.boxes.length;j++)check(!ChartLayout.overlap(result.boxes[i],result.boxes[j],0),'labels do not overlap');
      check(instance.getOption().series[0].itemStyle.color==='#c02090','scatter default color');
      const custom=ChartEditor.defaults(data);custom.series[0].color='#123456';
      check(ChartUI.option(data,false,custom).series[0].itemStyle.color==='#123456','per-chart color wins');
      button('調整排版').click();check(host.querySelectorAll('.chart-layout-handle').length===8,'editing handles');
      window.commandCount=sent.length;
    })()`);
    await capture("handles");
    // CDP送真實滑鼠事件，驗證pointer capture及拖曳完成，而非直接呼叫保存函式。
    const box=await evaluate(`(() => {const r=host.querySelector('[data-layout-key="line-0"]').getBoundingClientRect();return {x:r.x+15,y:r.y+12};})()`);
    await call("Input.dispatchMouseEvent",{type:"mouseMoved",...box});
    await call("Input.dispatchMouseEvent",{type:"mousePressed",...box,button:"left",clickCount:1});
    await call("Input.dispatchMouseEvent",{type:"mouseMoved",x:box.x+90,y:box.y+110,button:"left",buttons:1});
    await call("Input.dispatchMouseEvent",{type:"mouseReleased",x:box.x+90,y:box.y+110,button:"left",clickCount:1});
    await evaluate(`(() => {
      check(sent.length===commandCount,'drag remains an unsaved draft');
      for(const key of ['title','legend','line-2'])host.querySelector('[data-layout-key="'+key+'"]').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',shiftKey:true,bubbles:true}));
      button('完成排版').click();window.saved=structuredClone(sent.at(-1).command.style);
      check(saved.layout.reference_labels[0] && saved.layout.reference_labels[2] && saved.layout.title && saved.layout.legend,'all three element types saved');
      check(JSON.stringify(data)===original,'drag does not change data or line values');
      check(!host.querySelector('.chart-layout-layer'),'ordinary interactions restored');
      button('調整排版').click();button('自動排版').click();button('取消排版').click();
      check(sent.at(-1).command.style.layout.reference_labels[0].x===saved.layout.reference_labels[0].x,'cancel sends no new save');
      renderChart(JSON.parse(JSON.stringify(saved)));
    })()`);
    await delay(200);
    await evaluate(`(() => {
      window.plot=host.querySelector('.chart-plot');window.instance=echarts.getInstanceByDom(plot);
      const restored=ChartLayout.draw(instance,saved);check(restored.boxes.length===8,'reloaded positions');
      check(instance.getOption().series[0].markLine.data[0].xAxis===200,'reference value unchanged');
      instance.dispatchAction({type:'dataZoom',start:60,end:100});
      const zoomed=ChartLayout.draw(instance,saved);check(!zoomed.boxes.some(box=>box.key==='line-0'),'offscreen reference hides its label');
      instance.dispatchAction({type:'dataZoom',start:0,end:100});
      check(ChartLayout.draw(instance,saved).boxes.some(box=>box.key==='line-0'),'zoom restore restores label');
      button('編輯圖表').click();const dialog=document.querySelector('.chart-edit-dialog');
      dialog.querySelector('.chart-line-row button').click();dialog.querySelector('form').requestSubmit();
      window.edited=sent.at(-1).command.style;
      check(edited.layout.reference_labels[1].x===saved.layout.reference_labels[2].x,'line removal keeps the matching label position');
      check(edited.layout.title.x===saved.layout.title.x,'form editing preserves title position');
      window.png=ChartUI.exportPng(data,saved);
      check(png.length>10000,'PNG render');
    })()`);
    fs.writeFileSync(".build/chart-layout-export.png",Buffer.from((await evaluate("png")).split(",")[1],"base64"));
    checks.push("real pointer drag, keyboard title/legend, completion/cancel, reload, line removal, zoom and PNG");
    await capture("saved");
    await call("Emulation.setDeviceMetricsOverride",{width:620,height:850,deviceScaleFactor:1,mobile:false});await delay(250);
    await evaluate(`(() => {const view=ChartLayout.draw(instance,edited);check(view.boxes.every(box=>box.x>=0 && box.y>=0 && box.x+box.width<=view.width+1 && box.y+box.height<=view.height),'resized boxes stay inside');button('調整排版').click();button('自動排版').click();button('完成排版').click();check(sent.at(-1).command.style.layout.reference_labels.length===0,'reset layout only');check(sent.at(-1).command.style.series[0].color==='#c02090','reset preserves colors');})()`);
    await capture("narrow");checks.push("narrow resize and layout-only reset");
    await evaluate(`(() => {
      const canvas=document.createElement('div');canvas.style.cssText='width:900px;height:640px';host.append(canvas);
      const chart=echarts.init(canvas);const dense=structuredClone(data);
      dense.reference_lines=Array.from({length:10},(_,i)=>({axis:'x',value:200+i*.01,name:'條件'+i+'：這是一段需要換行的參考線說明，請保留完整文字。',color:'#145fbd'}));
      chart.setOption(ChartUI.option(dense));const view=ChartLayout.draw(chart,ChartEditor.defaults(dense));
      check(view.boxes.length===12&&!view.crowded,'ten long labels can fit in an expanded chart');
      for(let i=0;i<view.boxes.length;i++)for(let j=i+1;j<view.boxes.length;j++)check(!ChartLayout.overlap(view.boxes[i],view.boxes[j],0),'dense labels do not overlap');
      window.densePng=chart.getDataURL({type:'png',backgroundColor:'#fff'});
      // 使用介面的最小圖框，搭配合法的100字長標籤製造確實放不下的情況。
      const crowded=structuredClone(dense);crowded.reference_lines.forEach(line=>line.name=(line.name.repeat(4)).slice(0,100));
      canvas.style.width='440px';canvas.style.height='440px';chart.resize();chart.setOption(ChartUI.option(crowded),{notMerge:true});
      const small=ChartLayout.draw(chart,ChartEditor.defaults(crowded));check(small.crowded&&small.boxes.length===12,'small frame warns without discarding labels');
      for(const kind of ['line','bar','step','area','horizontal_bar']) {
        const sample={...data,kind,reference_lines:[{axis:kind==='horizontal_bar'?'x':'y',value:100,name:'目標',color:'#145fbd'}]};
        chart.resize({width:900,height:640});chart.setOption(ChartUI.option(sample),{notMerge:true});
        check(ChartLayout.draw(chart,ChartEditor.defaults(sample)).boxes.some(box=>box.key==='line-0'),'reference label in '+kind);
      }
      canvas.style.display='none';chart.resize({width:0,height:0});
      check(ChartLayout.draw(chart,ChartEditor.defaults(data)).boxes.length===0,'zero-sized chart waits for its size');
      canvas.style.display='block';chart.resize({width:900,height:640});
      chart.setOption(ChartUI.option(data),{notMerge:true});check(ChartLayout.draw(chart,ChartEditor.defaults(data)).boxes.length===8,'visible chart restores annotations');
      chart.dispose();canvas.remove();
    })()`);
    fs.writeFileSync(".build/chart-layout-dense.png",Buffer.from((await evaluate("densePng")).split(",")[1],"base64"));
    checks.push("ten long labels, small frame crowding and all six chart kinds");
    await call("Emulation.setDeviceMetricsOverride",{width:1180,height:1000,deviceScaleFactor:1,mobile:false});
    await evaluate(`renderChart(null);host.scrollTop=0;`);await delay(200);
    await evaluate(`(() => {
      button('調整排版').click();button('新增文字').click();
      const dialog=document.querySelector('.chart-text-dialog'),field=key=>dialog.querySelector('[data-text-field="'+key+'"]');
      field('text').value='區間1：穩定量測\\n參數 = 1';field('font_family').value='Microsoft JhengHei';field('font_size').value='22';
      for(const key of ['bold','italic','underline','background_enabled'])field(key).checked=true;
      field('background_enabled').dispatchEvent(new Event('change'));field('background').value='#fff0c0';field('color').value='#163f80';field('x').value='22';field('y').value='45';
      field('font_size').value='73';dialog.querySelector('form').requestSubmit();check(dialog.open,'bad font size rejected');
      field('font_size').value='22';dialog.querySelector('form').requestSubmit();check(!document.querySelector('.chart-text-dialog'),'text applied');
      const handle=host.querySelector('[data-layout-key="text-0"]');check(handle,'annotation drag handle');handle.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',shiftKey:true,bubbles:true}));
      button('完成排版').click();window.textStyle=structuredClone(sent.at(-1).command.style);
      const a=textStyle.layout.annotations[0];check(a.bold&&a.italic&&a.underline&&a.background==='#fff0c0'&&a.color==='#163f80','formatting persisted');
      check(a.position.x!==.22,'text position changed');
      button('調整排版').click();button('刪除文字').click();button('取消排版').click();
      button('調整排版').click();button('自動排版').click();button('完成排版').click();
      check(sent.at(-1).command.style.layout.annotations.length===1,'reset layout retains user annotations');
      renderChart(JSON.parse(JSON.stringify(textStyle)));
    })()`);await delay(200);
    await evaluate(`(() => {
      const instance=echarts.getInstanceByDom(host.querySelector('.chart-plot'));
      const elements=instance.getOption().graphic[0].elements;
      check(elements.some(element=>element.id==='chart-text-0'),'annotation is in actual renderer');
      window.textPng=ChartUI.exportPng(data,textStyle);
      // AI持久樣式與使用者編輯共用預設入口；包含轉換而不更改來源點陣。
      const ai=structuredClone(data);ai.style=structuredClone(textStyle);ai.style.transform.x={mode:'offset',offset:10,start:1,step:1};
      check(ChartUI.option(ai).series[0].data[0][0]===10,'AI style transform applied');
      check(ChartEditor.defaults(ai).layout.annotations[0].underline,'AI annotations retained');
      check(JSON.stringify(data)===original,'annotations preserve original source');
    })()`);
    fs.writeFileSync('.build/chart-layout-text.png',Buffer.from((await evaluate('textPng')).split(',')[1],'base64'));
    await capture('text-editor');checks.push('custom text formatting, keyboard positioning, validation, cancel/delete, reset, reload, AI style and PNG');
    await evaluate(`(() => {
      host.remove();fixture.messages=[{role:'user',content:'測試訊息',local_time:'2026-10-09 23:59:50'},
        {role:'assistant',content:'完成',local_time:'2026-10-10 00:00:10',project_activity:[
          '舊版沒有時間',{text:'等待 AI 回覆（第 1 輪）',at:'2026-10-09 23:59:51'},
          {text:'AI 進度筆記：已完成核對',at:'2026-10-10 00:00:01'}]}];
      LMUI.receive(fixture);
      const times=[...document.querySelectorAll('#messages .message-meta time')].map(t=>t.textContent);
      check(JSON.stringify(times)===JSON.stringify(['2026-10-09 23:59:50','2026-10-10 00:00:10']),'message dates across midnight');
      const list=document.querySelector('.project-activity-history');check(!list.children[0].querySelector('time'),'legacy event no invented timestamp');
      check(list.children[1].querySelector('time').textContent==='23:59:51','activity HH:MM:SS');
      const live=document.createElement('details');document.body.append(live);live.open=true;
      renderProjectActivity(live,fixture.messages[1].project_activity);const first=live.querySelector('li');
      renderProjectActivity(live,[...fixture.messages[1].project_activity,{text:'工具完成',at:'2026-10-10 00:00:05'}]);check(live.querySelector('li')===first,'timed activity retains nodes');
      const notes=document.createElement('div');renderProjectNarration(notes,fixture.messages[1].project_activity);check(notes.textContent.includes('已完成核對'),'timed progress notes still detected');live.remove();
      check(toolStatusNode({tool_name:'read',status:'completed',local_time:'2026-10-10 00:00:05'}).querySelector('time').textContent==='00:00:05','ordinary chat tool timestamp');
    })()`);
    await capture('timestamps');checks.push('local timestamp persistence display, midnight, old history, narration and stable activity nodes');
    fs.writeFileSync(".build/chart-layout-review.json",JSON.stringify({result:"PASS",checks},null,2));console.log(JSON.stringify({result:"PASS",checks}));
  } finally {ws?.close();browser.kill();}
})().catch(error=>{console.error(error);process.exitCode=1;});
