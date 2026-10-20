/* 離線介面的字型、排版、Mermaid及VNC預覽回歸；原生檔案與縮放另由EXE驗證。 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const {spawn} = require("node:child_process");
const {pathToFileURL} = require("node:url");
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

(async () => {
  const profile = path.resolve(".build", "preferences-ui-" + Date.now());
  fs.mkdirSync(profile, {recursive:true});
  const edge = spawn("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    ["--headless=new","--no-first-run","--disable-gpu","--remote-debugging-port=0","--user-data-dir="+profile,"about:blank"],
    {windowsHide:true,stdio:"ignore"});
  let socket;
  try {
    const portFile = path.join(profile,"DevToolsActivePort");
    for (let i=0;i<600 && !fs.existsSync(portFile);i++) {
      if (edge.exitCode !== null) throw Error("Offline test browser exited before initialization: "+edge.exitCode);
      await delay(100);
    }
    if (!fs.existsSync(portFile)) throw Error("Offline test browser did not publish its debugging port within 60 seconds.");
    const port = fs.readFileSync(portFile,"utf8").split("\n")[0];
    const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    socket = new WebSocket(pages.find(page => page.type === "page").webSocketDebuggerUrl);
    await new Promise((resolve,reject) => {socket.onopen=resolve;socket.onerror=reject;});
    let sequence=0;
    const pending=new Map();
    socket.onmessage=event=>{
      const message=JSON.parse(event.data), item=pending.get(message.id);
      if (item) {pending.delete(message.id);message.error?item.reject(message.error):item.resolve(message.result);}
    };
    const call=(method,params={})=>new Promise((resolve,reject)=>{
      const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));
    });
    const evaluate=async expression=>{
      const result=await call("Runtime.evaluate",{expression,awaitPromise:true,returnByValue:true,userGesture:true});
      if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
      return result.result.value;
    };
    await call("Emulation.setDeviceMetricsOverride",{width:1180,height:900,deviceScaleFactor:1,mobile:false});
    await call("Page.navigate",{url:pathToFileURL(path.resolve("ui/index.html")).href});
    for(let i=0;i<100 && !await evaluate("!!window.ProjectUI && !!window.DiagramUI");i++) await delay(100);
    const checks=await evaluate(`(async()=>{
      const checks=[],check=(ok,name)=>{if(!ok)throw Error(name);checks.push(name);};
      window.testCommands=[];send=message=>testCommands.push(message);
      window.preferenceFixture={...LMUI.getState(),logged_in:true,can_send:true,busy:"none",active_id:"ui-test",
        messages:[],models:[{id:"quality",label:"品質"},{id:"fast",label:"快速"}],notifications:[],onboarding:{enabled:false},
        config:{...LMUI.getState().config,model:"quality",vnc_enabled:true},
        conversations:[{id:"ui-test",title:"合成測試",project_id:"p",updated_at:1}],
        projects:{items:[{id:"p",name:"合成專案",root:"C:\\\\Synthetic",import_count:0}],running:false,status:""},
        vnc:{loaded:true,revision:7,groups:[],machines:[],options:{},sync_settings:{username:"",has_password:false,root:"https://example.invalid",home:"/",login:"/login",logout:"/logout",endpoints:[]},settings_revision:1}};
      LMUI.receive(preferenceFixture);LMUI.showView("chat");
      check($("minimize-to-taskbar").checked,"old preferences default to minimize in taskbar");
      $("minimize-to-taskbar").checked=false;$("minimize-to-taskbar").dispatchEvent(new Event("change"));
      check(testCommands.at(-1).minimize_to_taskbar===false,"unchecked window preference sends explicit false");
      LMUI.receive({...preferenceFixture,config:{...preferenceFixture.config,minimize_to_taskbar:false}});
      check(!$("minimize-to-taskbar").checked,"saved unchecked window preference is rendered");
      $("project-context").click();
      check(testCommands.at(-1).command.action==="open_folder" && testCommands.at(-1).command.id==="p" && !testCommands.at(-1).command.path,"project folder command uses native project id, not an arbitrary path");
      for(const family of ["LM Huninn","LM WenKai TC","LM WenKai Mono TC"]) {
        const faces=await document.fonts.load('16px "'+family+'"',"中文測試");
        check(faces.length===1 && faces[0].status==="loaded","bundled font loads offline: "+family);
      }
    // 真正檢查文字，避免把 value >= 10 誤畫成 value = 10。
    {
      const source = ['flowchart TD', ' A["value >= 10"] -->|"x > 5"| B{value <= 20}', ' B --> C["score &gt;= 80"]', ' C --> D["angle < 30"]'].join(String.fromCharCode(10));
      const result = JSON.parse(await DiagramUI.exportData(source, "svg"));
      check(result.ok, "Mermaid comparison fixture renders");
      const doc = new DOMParser().parseFromString(result.data, "image/svg+xml");
      const labels = [...doc.querySelectorAll("text")].map(node => node.textContent.trim());
      for (const expected of ["value >= 10", "x > 5", "value <= 20", "score >= 80", "angle < 30"]) {
        check(labels.includes(expected), "Mermaid preserves comparison label: " + expected);
      }
      const clean = new DOMParser().parseFromString(DiagramUI.safeSvg('<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><script>alert(1)</script><image href="https://example.invalid/x"/><foreignObject/><text>&gt;=</text></svg>'), "image/svg+xml");
      check(!clean.querySelector("script,image,foreignObject,[onload]") && clean.querySelector("text").textContent === ">=", "SVG sanitizer preserves operators and removes active/external content");
    }
      const wheel=new WheelEvent("wheel",{ctrlKey:true,cancelable:true});document.dispatchEvent(wheel);
      check(wheel.defaultPrevented,"Ctrl wheel browser scaling is prevented");
      $("project-dialog").showModal();
      check(!$("project-dialog").querySelector("details").open,"support types start collapsed");
      const label=document.querySelector('label[for="project-name"]').getBoundingClientRect();
      const input=$("project-name").getBoundingClientRect(), create=$("project-default-folder").getBoundingClientRect();
      check(label.bottom<=input.top && input.bottom+8<=create.top,"new project label, input and folder button have separate space");
      $("project-dialog").close();
      LMUI.showView("vnc");
      preferenceFixture.vnc.json_preview={id:"preview",revision:7,name:"synthetic.json",groups:1,machines:2,current_machines:1,
        add:{groups:1,machines:3,renamed:1},replace:{groups:1,machines:2,renamed:0}};
      LMUI.receive(preferenceFixture);
      const dialog=document.querySelector(".vnc-json-dialog"), apply=dialog.querySelector("[data-json-apply]");
      check(dialog.open && !apply.disabled && dialog.textContent.includes("自動改名 1 台"),"VNC JSON preview offers add and rename summary");
      preferenceFixture.vnc.revision=8;LMUI.receive(preferenceFixture);
      check(apply.disabled && dialog.textContent.includes("已變更"),"VNC stale preview cannot be committed");
      dialog.close();preferenceFixture.vnc.json_preview=null;LMUI.receive(preferenceFixture);LMUI.showView("chat");
      window.testDiagram=document.createElement("div");document.body.append(testDiagram);
      DiagramUI.render(testDiagram,[{name:"流程圖尺寸與返回",source:'flowchart LR\\n A["開始"] --> B["讀取設定"] --> C["判斷條件"] --> D["完成"]',revision:"r"}],{conversation:"ui-test",message_index:0});
      for(let i=0;i<200 && !testDiagram.querySelector("svg");i++)await new Promise(r=>setTimeout(r,50));
      const svg=testDiagram.querySelector("svg");
      check(!!svg && svg.getBoundingClientRect().width>100,"diagram renders at readable intrinsic size");
      return checks;
    })()`);
    // 真正的Fullscreen API與fullscreenchange事件，未偽造fullscreenElement。
    await evaluate(`testDiagram.querySelector('[data-diagram-fullscreen]').click()`);
    await delay(150);
    const fullscreen=await evaluate(`({active:document.fullscreenElement===testDiagram.querySelector('.mermaid-card'),label:testDiagram.querySelector('[data-diagram-fullscreen]').textContent})`);
    if(!fullscreen.active || fullscreen.label!=="返回對話（Esc）") throw Error("Actual fullscreen entry/return label failed: "+JSON.stringify(fullscreen));
    checks.push("actual fullscreen entry changes button to explicit return");
    await evaluate(`testDiagram.querySelector('[data-diagram-fullscreen]').click()`);
    await delay(100);
    if(!await evaluate(`!document.fullscreenElement && testDiagram.querySelector('[data-diagram-fullscreen]').textContent==='全畫面'`)) throw Error("Actual fullscreen return failed");
    checks.push("actual fullscreen return restores the normal button");
    const image=await call("Page.captureScreenshot",{format:"png"});
    fs.writeFileSync(".build/preferences-ui-mermaid.png",Buffer.from(image.data,"base64"));
    fs.writeFileSync(".build/preferences-ui-verification.json",JSON.stringify({result:"PASS_SOURCE_UI",native_webview_covered:false,scope:"offline Edge source UI; native WebView2 requires separate EXE validation",checks},null,2));
    console.log(JSON.stringify({result:"PASS_SOURCE_UI",checks}));
    await call("Browser.close");
  } finally {socket?.close();edge.kill();}
})().catch(error=>{console.error(error.stack||error);process.exitCode=1;});
