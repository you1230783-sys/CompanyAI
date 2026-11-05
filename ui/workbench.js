/* 工作條件、偏好、定點修改與流程表單。只送原生命令或填草稿，不自行執行AI工具。 */
"use strict";
window.WorkbenchUI = (() => {
  const make = (tag, text = "", className = "") => {
    const element = document.createElement(tag);
    element.textContent = text; element.className = className; return element;
  };
  const isProject = () => state.conversations.some(c => c.id === state.active_id && c.project_id);
  const command = value => send({type:"project", command:value});
  let owner = null, settings = null, preferenceForm = null, conditionInput = null, scope = null;
  let settingsBusy = false;
  let settingsRequest = null, requestSerial = 0, editingScope = null;
  let preferenceDrafts = {};
  const dialogs = new Set();

  function modal(title) {
    const dialog = make("dialog", "", "workbench-dialog");
    dialog.dataset.conversation = state.active_id || "";
    const heading = make("div", "", "dialog-heading"), close = make("button", "關閉");
    close.onclick = () => dialog.close(); heading.append(make("h2", title), close); dialog.append(heading);
    dialog.addEventListener("close", () => { dialogs.delete(dialog); dialog.remove(); });
    document.body.append(dialog); dialogs.add(dialog); return dialog;
  }
  function field(parent, label, input) {
    const row = make("label", label, "workbench-field"); row.append(input); parent.append(row); return input;
  }
  function input(value = "", maxLength = 1000) {
    const element = make("input"); element.value = value; element.maxLength = maxLength; return element;
  }
  function select(options, value) {
    const element = make("select");
    for (const [key, label] of options) { const option = make("option",label); option.value = key; element.append(option); }
    element.value = value ?? ""; return element;
  }
  function preferences(parent, value) {
    parent.replaceChildren();
    const language = field(parent,"回覆語言",input(value.language,40));
    const decimals = field(parent,"小數位數",select([["","依本次需求"], ...Array.from({length:9},(_,i)=>[String(i),String(i)])],value.decimals));
    const date = field(parent,"日期格式",select([["","依本次需求"],["YYYY-MM-DD","YYYY-MM-DD"],["YYYY/MM/DD","YYYY/MM/DD"],["DD/MM/YYYY","DD/MM/YYYY"]],value.date_format));
    const background = field(parent,"圖表背景",select([["","依本次需求"],["white","白色"],["transparent","透明"]],value.chart_background));
    const notes = make("textarea"); notes.maxLength = 1000; notes.rows = 4; notes.value = value.notes || "";
    field(parent,"其他呈現偏好",notes);
    return () => ({language:language.value.trim(), decimals:decimals.value===""?null:Number(decimals.value),
      date_format:date.value,chart_background:background.value,notes:notes.value.trim()});
  }
  const emptyPreferences = () => ({language:"",decimals:null,date_format:"",chart_background:"",notes:""});
  let settingsDialog = null, settingsStatus = null, readPreferences = null, saveButton = null;
  function openSettings() {
    if (!state.logged_in || !state.active_id || settingsDialog?.open) return;
    owner = state.active_id; settings = null; settingsBusy = true; preferenceDrafts = {};
    settingsRequest = `settings-${++requestSerial}`;
    settingsDialog = modal("工作偏好與條件");
    settingsDialog.append(make("p","偏好供新任務使用，本次明確要求優先。專案空欄位沿用個人偏好；條件卡變更會在執行中任務的下一輪生效。"));
    scope = field(settingsDialog,"設定範圍",select([["personal","個人偏好"],...(isProject()?[["project","本專案覆寫"]]:[])],isProject()?"project":"personal"));
    editingScope = scope.value; scope.disabled = true;
    preferenceForm = make("div"); settingsDialog.append(preferenceForm);
    const conditions = make("section"); conditions.hidden = !isProject();
    conditionInput = make("textarea"); conditionInput.maxLength = 4000; conditionInput.rows = 6;
    conditionInput.disabled = true;
    conditionInput.placeholder = "例如：分析日期、單位、排除規則、指定成果格式。每行一項。";
    field(conditions,"本對話條件",conditionInput); settingsDialog.append(conditions);
    const actions = make("div","","dialog-actions"); saveButton = make("button","保存"); saveButton.disabled = true;
    const clear = make("button","清除此範圍偏好"); clear.onclick = () => { if(settingsBusy)return;readPreferences = preferences(preferenceForm,emptyPreferences()); };
    actions.append(saveButton,clear); settingsDialog.append(actions);
    settingsStatus = make("p","正在讀取…"); settingsStatus.setAttribute("role","status"); settingsDialog.append(settingsStatus);
    scope.onchange = () => {
      if (!settings || settingsBusy) return;
      preferenceDrafts[editingScope] = readPreferences(); editingScope = scope.value;
      readPreferences = preferences(preferenceForm,preferenceDrafts[editingScope] || settings[editingScope] || emptyPreferences());
    };
    saveButton.onclick = () => {
      if (!settings || settingsBusy || owner !== state.active_id) return;
      settingsBusy = true; saveButton.disabled = true; scope.disabled = true; settingsStatus.textContent = "正在保存…";
      conditionInput.disabled = true;
      preferenceForm.querySelectorAll('input,select,textarea').forEach(element=>element.disabled=true);
      const values = readPreferences();
      preferenceDrafts[editingScope] = values; settingsRequest = `settings-${++requestSerial}`;
      command({action:"work_settings",conversation:owner,request_id:settingsRequest,expected:settings.fingerprint,
        personal:preferenceDrafts.personal||null,project:preferenceDrafts.project||null,
        conditions:settings.is_project?{revision:settings.conditions.revision,text:conditionInput.value}:null});
    };
    settingsDialog.addEventListener("close",()=>{owner=null;settings=null;settingsBusy=false;});
    settingsDialog.showModal(); command({action:"work_settings",conversation:owner,request_id:settingsRequest});
  }
  function receive(message) {
    if(exportReceive(message))return;
    if (message.type !== "work_settings" || message.request_id !== settingsRequest || !settingsDialog?.open || message.conversation !== owner || owner !== state.active_id) return;
    settingsBusy = false; scope.disabled = false; conditionInput.disabled = false;
    preferenceForm.querySelectorAll('input,select,textarea').forEach(element=>element.disabled=false);
    if (message.error) { settingsStatus.textContent = message.error; saveButton.disabled = !settings; return; }
    settings = message.data; preferenceDrafts = {}; readPreferences = preferences(preferenceForm,settings[scope.value]||emptyPreferences());
    conditionInput.value = settings.conditions.text; saveButton.disabled = false;
    settingsStatus.textContent = settings.saved ? "已保存。條件更動後，舊成果會提示重新核對。" : `條件版本：${settings.conditions.revision}`;
  }
  function appendDraft(text, conversation = state.active_id) {
    if (conversation !== state.active_id) { toast("已切換對話，未加入草稿"); return false; }
    const prompt = $("prompt"), next = prompt.value ? `${prompt.value}\n\n${text}` : text;
    if (next.length > prompt.maxLength) { toast("草稿過長，請先縮短目前草稿再加入"); return false; }
    prompt.value = next; prompt.dispatchEvent(new Event("input",{bubbles:true})); prompt.focus(); return true;
  }
  function correct(context) {
    if (!isProject() || context.conversation !== state.active_id) { toast("請回到原專案對話修改"); return; }
    const dialog = modal("修改選定內容");
    dialog.append(make("p","先核對選定位置與版本，再填寫修改要求。送出後建立新版成果。"));
    const location = [context.path, context.block_label, context.node_id && `節點：${context.node_id}`,
      context.revision && `版本：${context.revision.slice(0,12)}`, context.selected_text].filter(Boolean);
    const detail = make("pre",location.join("\n") || "目前選定的回覆"); detail.className="workbench-reference"; dialog.append(detail);
    const instruction = make("textarea"); instruction.rows=4; instruction.maxLength=1500;
    field(dialog,"希望如何修改",instruction);
    const add = make("button","加入提問草稿");
    add.onclick = () => {
      if (!instruction.value.trim()) { instruction.focus(); return; }
      const text = `請依以下選定位置修改，先核對成果版本與來源；若已變更，先說明差異。\n選定位置（歷史快照）：\n${JSON.stringify(context)}\n我的修改要求：${instruction.value.trim()}`;
      if (appendDraft(text,context.conversation)) { dialog.close(); closeSourceDialogs(); }
    };
    dialog.append(add); dialog.showModal(); instruction.focus();
  }
  function closeSourceDialogs() {
    for (const id of ["project-artifact-dialog", "review-window", "diagram-source-dialog"]) {
      const dialog = $(id); if(dialog?.open)dialog.close();
    }
    $("prompt").focus();
  }
  function attachReply(tools, message, index, content) {
    if(message.role==="assistant") {const exportButton=make("button","匯出 Word／PDF");exportButton.onclick=()=>exportDialog(index);tools.append(exportButton);}
    if (!isProject() || message.role !== "assistant") return;
    const conversation = state.active_id;
    const change = make("button","修改選取文字"); change.onmousedown = event => event.preventDefault();
    change.onclick = () => {
      const selection = window.getSelection();
      if (!selection?.rangeCount || selection.isCollapsed || !content.contains(selection.getRangeAt(0).commonAncestorContainer)) {
        toast("請先在這則回覆中選取要修改的文字"); return;
      }
      const text=selection.toString();
      if (text.length > 2500) { toast("請選取較短的段落（最多2500字）"); return; }
      correct({conversation,request_id:message.request_id,message_index:index,kind:"reply",selected_text:text});
    };
    tools.append(change);
    const pending = message.project_analysis?.deliverables?.items?.filter(i=>["pending","draft","blocked"].includes(i.status)) || [];
    const recovery = message.project_analysis?.recovery;
    if (pending.length || message.incomplete || message.project_paused || (recovery?.state && recovery.state !== "completed")) {
      const continueDraft = make("button","整理剩餘工作");
      continueDraft.title="先填入可修改草稿，送出後核對現況；不直接重播先前操作";
      continueDraft.onclick = () => appendDraft(`請先查回任務 ${message.request_id || "上一輪"} 的實際成果與未完成項目，保留已完成成果，只補做剩餘工作。結果未知的寫入先查明，不直接重試。\n${pending.map(i=>`- ${i.description}${i.blocked_reason?`（${i.blocked_reason}）`:""}`).join("\n")}`,conversation);
      tools.append(continueDraft);
    }
  }
  function delivery(container, snapshot) {
    if (!snapshot || (!snapshot.items?.length && !snapshot.conditions?.text)) return;
    const card=make("section","","delivery-card"); card.append(make("strong","交付清單與條件"));
    const labels={pending:"待處理",draft:"草稿，尚未保存最新版本",saved:"已保存，交付時再核對",delivered:"已交付，檔案已讀回核對",blocked:"未完成",answer_pending:"隨最終回答交付",answered:"已回答，內容仍需核對"};
    if(snapshot.review_required)card.append(make("p","條件已更動：以下成果需重新核對。","analysis-attention"));
    for(const item of snapshot.items||[]) {
      const row=make("p",`${item.description}：${labels[item.status]||item.status}${item.blocked_reason?`（${item.blocked_reason}）`:""}`);
      if(item.path)row.append(make("small",`　${item.path}`)); card.append(row);
    }
    if(snapshot.conditions?.text) {
      const conditions=make("details"), summary=make("summary",`使用者條件（第${snapshot.conditions.revision}版）`);
      conditions.append(summary,make("pre",snapshot.conditions.text));card.append(conditions);
    }
    card.append(make("small",snapshot.notice||""));container.append(card);
  }
  function workflow(flow, conversation) {
    const dialog=modal(`重跑流程：${flow.name}`), fields=[];
    dialog.append(make("p","填入這次參數後加入草稿。執行前會核對來源檔案、參數型別與各步驟。"));
    for(const parameter of flow.parameters||[]) {
      const control=parameter.kind==="boolean" ? select([["true","是"],["false","否"]],String(parameter.example))
        : ["object","array","json"].includes(parameter.kind)?make("textarea"):input("",4000);
      if(parameter.kind==="number")control.type="number";
      // 上次來源只作範例，避免未注意便重跑舊資料；布林則明確呈現選擇。
      if(parameter.kind!=="boolean")control.placeholder=typeof parameter.example==="string"?parameter.example:JSON.stringify(parameter.example);
      field(dialog,`${parameter.name}（${parameter.kind}）`,control);
      dialog.append(make("small",parameter.uses.join("；"))); fields.push([parameter,control]);
    }
    const plan=make("details");plan.append(make("summary",`檢視 ${flow.steps} 個步驟`),make("pre",(flow.plan||[]).map(s=>`${s.id}：${s.tool}${s.output_name?` → ${s.output_name}`:""}`).join("\n")));dialog.append(plan);
    const status=make("p");status.setAttribute("role","status");const add=make("button","加入提問草稿");
    add.onclick=()=>{
      try {
        const parameters=Object.create(null);
        for(const [parameter,control] of fields) {
          if(!control.value.trim())throw new Error(`請填寫 ${parameter.name}`);
          let value=control.value;
          if(parameter.kind==="number") { value=Number(value); if(!Number.isFinite(value))throw new Error(`${parameter.name} 需為有效數值`); }
          else if(parameter.kind==="boolean")value=value==="true";
          else if(["object","array","json"].includes(parameter.kind)) {
            value=JSON.parse(value);
            if(parameter.kind==="array"&&!Array.isArray(value))throw new Error(`${parameter.name} 需為陣列`);
            if(parameter.kind==="object"&&(!value||typeof value!=="object"||Array.isArray(value)))throw new Error(`${parameter.name} 需為物件`);
          }
          parameters[parameter.name]=value;
        }
        if(appendDraft(`請重跑工作流程「${flow.name}」（workflow_id=${flow.id}）。載入notes，檢視範本後，以以下參數prepare_workflow，核對來源及計畫，再逐步執行；保留已成功結果。\n本次參數：${JSON.stringify(parameters)}`,conversation)){dialog.close();closeSourceDialogs();}
      } catch(error) { status.textContent=error.message; }
    };
    dialog.append(status,add);dialog.showModal();
  }
  let exportView=null,exportStatus=null,exportSave=null,exportName=null,exportId=null;
  function exportDialog(index=null){
    if(exportView?.open)exportView.close();
    const conversation=state.active_id;exportView=modal(index===null?"匯出整段對話":"匯出這則回答");
    const format=field(exportView,"格式",select([["docx","Word"],["pdf","PDF"]],"docx"));
    exportName=field(exportView,"副本檔名",input("對話匯出.docx",180));
    exportStatus=make("p","先預覽匯出範圍。直接使用已保存內容，不經AI重寫。標題、表格、程式碼與本機PNG支援；其他Markdown語法原字保留。來源引用文字會保留。互動圖表、遠端圖片與附件本身不自動嵌入。");
    const preview=make("button","預覽匯出範圍");exportSave=make("button","確認匯出副本");exportSave.disabled=true;exportId=null;
    format.onchange=()=>{exportName.value="對話匯出."+format.value;exportId=null;exportSave.disabled=true;};
    preview.onclick=()=>{exportSave.disabled=true;exportStatus.textContent="核對內容中…";send({type:"document_export",command:{action:"preview",conversation,index,pdf:format.value==="pdf"}});};
    exportSave.onclick=()=>{if(!exportId)return;exportSave.disabled=true;preview.disabled=true;format.disabled=true;exportStatus.textContent="正在產生副本及讀回驗證…";send({type:"document_export",command:{action:"save",id:exportId,name:exportName.value}});exportId=null;};
    exportView.addEventListener("close",()=>send({type:"document_export",command:{action:"cancel"}}));
    exportView.append(exportStatus,preview,exportSave);exportView.showModal();
  }
  function exportReceive(message){
    if(!message.type?.startsWith("document_export_"))return false;
    if(message.type==="document_export_preview"&&exportView?.open&&message.conversation===state.active_id){exportId=message.id;exportSave.disabled=false;exportStatus.textContent=`${message.messages}則訊息、${message.blocks}個內容區塊。輸出位置：${message.folder}。同名檔案自動改名。${(message.warnings||[]).join("；")}`;}
    if(message.type==="document_export_done"){const text=`已匯出並讀回驗證：${message.result.path}。${message.result.notice}`;if(exportStatus)exportStatus.textContent=text;toast(text);}
    if(message.type==="document_export_error"){if(exportStatus)exportStatus.textContent=message.message;toast(message.message);}
    return true;
  }
  const exportAll=make("button","匯出整段對話 Word／PDF");$("project-tools-menu").append(exportAll);exportAll.onclick=()=>exportDialog();
  const documentButton=make("button","翻譯／比較／新建 Markdown");$("project-tools-menu").append(documentButton);
  documentButton.onclick=()=>{
    if(!isProject()){toast("請先選擇專案或我的工作區。");return;}
    const conversation=state.active_id,dialog=modal("文件工作");
    const mode=field(dialog,"工作",select([["translate","Word翻譯成新文件"],["styled","保留原樣式的DOCX副本翻譯"],["compare","Word／Excel差異比較"],["markdown","新建Markdown文件"]],"translate"));
    const source=field(dialog,"來源相對路徑（新建Markdown可空白）",input("",500));
    const second=field(dialog,"比較的第二份相對路徑",input("",500));
    const language=field(dialog,"翻譯目標語言",input("繁體中文",80));
    const name=field(dialog,"輸出副本檔名",input("翻譯.docx",180));
    const glossary=field(dialog,"專有名詞表：每行 原文=譯名",make("textarea"));glossary.maxLength=12000;
    const key=field(dialog,"Excel唯一鍵欄（可空白，例如A；第一列為標題）",input("",3));
    const details=field(dialog,"補充需求／Markdown文件內容",make("textarea"));details.maxLength=12000;
    const comparisonNotice=make("p","Word依原文順序對齊正文與表格段落，列出前後來源位置；重複文字可能有多種對應，候選替換需人工核對，不推論語意或移動。Excel依儲存格座標或指定唯一鍵欄對應。");dialog.append(comparisonNotice);
    const configureFields=()=>{
      const translating=["translate","styled"].includes(mode.value),comparing=mode.value==="compare",markdown=mode.value==="markdown";
      source.parentElement.hidden=markdown;second.parentElement.hidden=!comparing;language.parentElement.hidden=!translating;
      name.parentElement.hidden=comparing;glossary.parentElement.hidden=!translating;key.parentElement.hidden=!comparing;
      comparisonNotice.hidden=!comparing;
      if(markdown&&name.value==="翻譯.docx")name.value="新文件.md";
      if(translating&&name.value==="新文件.md")name.value="翻譯.docx";
    };mode.onchange=configureFields;configureFields();
    dialog.append(make("p","檔案和名詞表視為資料。原樣式翻譯涵蓋正文／表格的可編輯文字；特殊欄位、圖片段落、頁首頁尾、文字方塊等保持原樣，文字長度可能造成重排。比較以可讀範圍為準。新建Markdown仍遵守受保護資料不得轉存的限制。"));
    const add=make("button","加入草稿，檢查後送出");add.onclick=()=>{
      try{
        let prompt;
        if(mode.value==="markdown")prompt=`請載入text-edit，新建Markdown文件（create_working_copy的source為null），編輯後儲存副本並讀回驗證；遵守受保護資料轉存限制。檔名：${JSON.stringify(name.value.toLowerCase().endsWith(".md")?name.value:name.value+".md")}。內容需求：\n${details.value}`;
        else if(mode.value==="compare"){if(!source.value||!second.value)throw new Error("請填兩份文件路徑");prompt=`請載入office-edit，使用document_task compare比較以下文件，逐項引用location、原值／新值及原生範圍，列明未比較與截斷項目。${JSON.stringify({left:source.value,right:second.value,key_column:key.value.trim().toUpperCase()||null})}\n${details.value}`;}
        else{if(!source.value||!language.value)throw new Error("請填來源與目標語言");const terms=glossary.value.split(/\r?\n/).filter(v=>v.trim()).map(v=>{const i=v.indexOf("=");if(i<=0||!v.slice(i+1).trim())throw new Error("名詞表每行需為原文=譯名");return {source:v.slice(0,i).trim(),target:v.slice(i+1).trim()};});prompt=`請載入office-edit，使用document_task逐段翻譯、分頁及核對revision，所有可譯區段完成後才finish_translation，列出未涵蓋區域與人工校對需求。prepare_translation參數：${JSON.stringify({path:source.value,language:language.value,mode:mode.value==="styled"?"preserve_styles":"new_document",terms})}；輸出檔名：${JSON.stringify(name.value)}。\n${details.value}`;}
        if(appendDraft(prompt,conversation))dialog.close();
      }catch(error){toast(error.message);}
    };dialog.append(add);dialog.showModal();
  };
  const workspaceButton=make("button","我的工作區");$("project-tools-menu").append(workspaceButton);
  workspaceButton.onclick=async()=>{if(fileBatchBusy)return;fileBatchBusy=true;try{await fileStep({action:"open_default"},"workspace");}catch(error){toast(error.message);}finally{fileBatchBusy=false;renderWork();}};
  function sync() {
    exportAll.disabled=!state.logged_in||!state.active_id;
    documentButton.disabled=!state.logged_in||!isProject();workspaceButton.disabled=!state.logged_in;
    $("work-settings-open").hidden=!state.logged_in||!state.active_id;
    for(const dialog of dialogs) if(dialog.dataset.conversation!==state.active_id||!state.logged_in)dialog.close();
    const hint=$("project-capability-hint");hint.hidden=false;
    if(!isProject()) {
      hint.textContent="一般對話：使用公司網站服務與已加入的附件，未連結本機資料夾。";
      return;
    }
    if(!hint.hidden) {
      const text=$("prompt").value;
      const image=/圖片|照片|截圖|\.(png|jpe?g|bmp|tiff?)\b/i.test(text);
      const detail=image&&state.config.model!=="quality" ? "這個專案模型不支援圖片閱讀；請在模型選單切換品質模型後送出。"
        : /CAD|dwg|圖面/i.test(text) && !featureEnabled("cad") ? "此建置未包含CAD，無法讀取或修改圖面。請使用包含CAD功能的版本。"
        : /Word|Excel|報告|docx|xlsx/i.test(text) ? "可直接使用內建 Word／Excel 工具；實際 Office 可用性在執行時檢查。"
        : /Python|分析|計算/i.test(text) ? "資料分析使用隨程式提供的 Python 環境；可用套件以工具回報為準。"
        : "可指定輸出格式；多份成果會以交付清單追蹤。";
      hint.textContent="專案對話：可讀取授權資料夾、建立副本；必要內容仍送公司模型。"+detail;
    }
  }
  $("work-settings-open").onclick=openSettings;
  $("prompt").addEventListener("input",sync);
  return {sync,receive,appendDraft,correct,attachReply,delivery,workflow};
})();
