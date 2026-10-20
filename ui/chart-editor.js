/* 使用者呈現設定及有限座標轉換；不允許 ECharts option 或腳本，不改原始點陣。 */
"use strict";
// 與 Rust quality::Policy 相同：只重建原生標記的缺值，保留有效數值及原始明細。
window.ChartQuality = {
  view(original, policy) {
    if(!original.quality || !policy || (!policy.blank && !policy.invalid)) return original;
    const data=structuredClone(original), labels={gap:"保留缺值（折線中斷）",skip:"略過此點（保留 X 位置，折線接續）",zero:"設為 0"};
    for(const cell of data.quality.cells) {
      const choice=cell.blank?policy.blank:policy.invalid;if(!choice) continue;
      if(!labels[choice]) throw new Error("不支援的缺值政策。");
      const series=data.series[cell.series];series.values[cell.row]=choice==="zero"?0:null;
      series.skip_indices=(series.skip_indices||[]).filter(i=>i!==cell.row);
      if(choice==="skip") series.skip_indices.push(cell.row);
      data.data_issues[cell.issue].handling=labels[choice];
    }
    for(const series of data.series) if(series.skip_indices) series.skip_indices.sort((a,b)=>a-b);
    data.data_note=`Y 空白：${labels[policy.blank]||"沿用建立時處理"}；Y 異常：${labels[policy.invalid]||"沿用建立時處理"}。原值與來源明細保留。`;
    return data;
  }
};
window.ChartEditor = (() => {
  const kinds={line:"折線圖",bar:"直條圖",scatter:"散佈圖",step:"階梯線",area:"面積圖",horizontal_bar:"水平長條圖"};
  let dialogNumber = 0;
  function defaults(data) {
    if(data.style) return structuredClone(data.style);
    return {title:data.title,x_label:data.kind==="horizontal_bar"?data.y_label:data.x_label,y_label:data.kind==="horizontal_bar"?data.x_label:data.y_label,kind:data.kind,legend:"right",x_min:null,x_max:null,y_min:null,y_max:null,
      series:data.series.map((s,i)=>({name:s.name,color:ChartAppearance.palette()[i]})),lines:structuredClone(data.reference_lines||[]),layout:{title:null,legend:null,reference_labels:[],annotations:[]},quality_policy:{blank:null,invalid:null},transform:ChartTransform.settings(data)};
  }
  const category=ChartTransform.category;
  // 類別軸輸入顯示標籤；重複標籤不猜位置，可用明確的 #資料序號（1起算）。
  function position(data,kind,axis,text,transform=ChartTransform.defaults()) {
    if(!text.trim()) return null;
    if(category(kind,axis,transform)) {
      if(/^#\d+$/.test(text)) {const i=Number(text.slice(1))-1;if(i>=0&&i<data.x.length) return i;}
      const hits=data.x.flatMap((x,i)=>String(x)===text ? [i] : []);
      if(hits.length===1) return hits[0];
      throw new Error("類別軸請填完整標籤；若標籤重複，請用 #資料序號，例如 #1。");
    }
    const n=Number(text);if(!Number.isFinite(n)) throw new Error("數值軸請填有效數字。");return n;
  }
  function display(data,kind,axis,value,transform=ChartTransform.defaults()) {return value==null ? "" : category(kind,axis,transform) ? `#${value+1}` : String(value);}
  function open(data,current,apply) {
    const initial=structuredClone(current || defaults(data));
    initial.layout ??= {title:null,legend:null,reference_labels:[],annotations:[]};
    initial.transform=ChartTransform.settings(data,initial);
    const initialData=ChartTransform.view(ChartQuality.view(data,initial.quality_policy),initial.kind,initial.transform);
    const dialog=document.createElement("dialog");dialog.className="chart-edit-dialog";
    const form=document.createElement("form"),heading=document.createElement("h2");heading.textContent="編輯圖表";
    // 分頁內的欄位可能不可見；由下方驗證集中切換到錯誤欄位，不讓瀏覽器聚焦隱藏輸入框。
    form.noValidate=true;
    const prefix=`chart-editor-${++dialogNumber}`;heading.id=`${prefix}-title`;dialog.setAttribute("aria-labelledby",heading.id);
    const hint=document.createElement("p");hint.className="chart-editor-hint";hint.textContent="原始資料保留；切換頁籤不會清除編輯內容，按「套用」一起生效。";
    const tabs=document.createElement("div"),body=document.createElement("div");
    tabs.className="chart-editor-tabs";tabs.setAttribute("role","tablist");tabs.setAttribute("aria-label","圖表設定");body.className="chart-editor-body";
    const pages=[],tabButtons=[];
    function selectPage(index,focus=false) {
      pages.forEach((page,i)=>{page.hidden=i!==index;tabButtons[i].setAttribute("aria-selected",String(i===index));tabButtons[i].tabIndex=i===index?0:-1;});
      body.scrollTop=0;if(focus) tabButtons[index].focus();
    }
    for(const [index,title] of ["基本設定","座標轉換","顏色與參考線"].entries()) {
      const tab=document.createElement("button"),page=document.createElement("section");
      tab.type="button";tab.textContent=title;tab.id=`${prefix}-tab-${index}`;tab.setAttribute("role","tab");
      page.id=`${prefix}-page-${index}`;page.setAttribute("role","tabpanel");page.setAttribute("aria-labelledby",tab.id);tab.setAttribute("aria-controls",page.id);
      tab.onclick=()=>selectPage(index);
      tab.onkeydown=event=>{
        const next={ArrowRight:(index+1)%3,ArrowLeft:(index+2)%3,Home:0,End:2}[event.key];
        if(next!==undefined){event.preventDefault();selectPage(next,true);}
      };
      tabs.append(tab);body.append(page);tabButtons.push(tab);pages.push(page);
    }
    function help(parent,title,paragraphs) {
      const details=document.createElement("details"),summary=document.createElement("summary");details.className="chart-editor-help";
      summary.textContent=`？ ${title}`;details.append(summary);
      for(const text of paragraphs){const p=document.createElement("p");p.textContent=text;details.append(p);}
      parent.append(details);
    }
    help(pages[0],"範圍與自動調整怎麼用？",[
      "上下限留白時自動決定；量測軸以所有有效點的兩倍標準差留白，限制為資料跨度的5%～40%，再依跨度向外取整。「自動調整」可填入計算值，手動上下限優先。",
      "範例：90～110、標準差3 → 80～120；0.25～0.35、標準差0.02 → 0.2～0.4。同值資料以值的5%留白；全部為0時使用−1～1。",
      "類別軸（例如文字或日期）會取第一到最後一筆。手動設定可填完整標籤，或用 #1、#5 指定第 1、第 5 個位置。上下限只調整可見範圍，不刪除資料。"
    ]);
    const grid=document.createElement("div");grid.className="chart-edit-grid";
    const controls={};
    function field(parent,label,value,type="text",options=null) {
      const wrap=document.createElement("label"),text=document.createElement("span");text.textContent=label;
      const input=document.createElement(options ? "select" : "input");
      if(options) for(const [key,name] of Object.entries(options)){const opt=document.createElement("option");opt.value=key;opt.textContent=name;input.append(opt);}
      else {input.type=type;if(type==="text") input.maxLength=200;if(type==="number") input.step="any";}
      input.value=value??"";wrap.append(text,input);parent.append(wrap);return input;
    }
    controls.title=field(grid,"標題",initial.title);controls.x_label=field(grid,"X軸標題",initial.x_label);controls.y_label=field(grid,"Y軸標題",initial.y_label);
    controls.kind=field(grid,"圖表類型",initial.kind,"text",kinds);
    controls.legend=field(grid,"圖例位置",initial.legend,"text",{right:"右側",bottom_right:"右下方",bottom:"下方置中",top:"標題下方",hidden:"隱藏"});
    pages[0].append(grid);
    const qualityControls={};
    if(data.quality?.cells?.length) {
      const choices={original:"沿用建立時的處理",gap:"保留缺值（折線中斷）",skip:"略過此點（折線接續）",zero:"設為 0"};
      for(const [key,label] of [["blank","Y 原始空白"],["invalid","Y 異常值"]]) {
        qualityControls[key]=field(grid,label,initial.quality_policy?.[key]||"original","text",choices);
        qualityControls[key].dataset.quality=key;
        qualityControls[key].onchange=()=>refreshTransform(true);
      }
      help(pages[0],"缺值處理可以隨時切換",["保留缺值不補數值；略過此點保留原 X 位置並接續折線；設為 0 會改變圖形與範圍。來源檔案與異常明細不變；原本有效的 0 不受影響。"]);
    }
    function readQuality() {
      const read=key=>qualityControls[key]?.value&&qualityControls[key].value!=="original"?qualityControls[key].value:null;
      return {blank:read("blank"),invalid:read("invalid")};
    }
    const qualityData=()=>ChartQuality.view(data,readQuality());
    const rangeStatus=document.createElement("p");rangeStatus.setAttribute("role","status");
    for(const axis of ["x","y"]) {
      const range=document.createElement("fieldset"),legend=document.createElement("legend"),row=document.createElement("div");
      legend.textContent=`${axis.toUpperCase()} 軸範圍`;row.className="chart-axis-range";range.append(legend,row);pages[0].append(range);
      for(const bound of ["min","max"]){const key=`${axis}_${bound}`;controls[key]=field(row,bound==="min"?"下限":"上限",display(initialData,initial.kind,axis,initial[key],initial.transform));controls[key].dataset.bound=key;}
      const auto=document.createElement("button");auto.type="button";auto.textContent="自動調整";auto.dataset.autoAxis=axis;row.append(auto);
      auto.onclick=()=>{try {
        const transform=readTransform(),shown=ChartTransform.view(qualityData(),controls.kind.value,transform);
        const [min,max]=ChartTransform.bounds(qualityData(),controls.kind.value,transform,axis);
        controls[`${axis}_min`].value=display(shown,controls.kind.value,axis,min,transform);
        controls[`${axis}_max`].value=display(shown,controls.kind.value,axis,max,transform);
        error.textContent="";rangeStatus.textContent=`${axis.toUpperCase()} 軸已依目前資料調整。`;
      } catch(e){error.textContent=e.message;}};
    }
    pages[0].append(rangeStatus);
    help(pages[1],"平移、重新編號、移除無值的差別",[
      "平移：每個座標加上同一個數。例如 10、20、30 平移 −10 後為 0、10、20，保留原本間距。",
      "重新編號：依資料順序指定起點與間距。例如 7001、7003、7008，以起點 1、間距 1 變成 1、2、3；間距可為負數，但不能為 0。",
      "五點範例：X＝7001、7002、7003、7004、7005；Y＝10、空白、20、空白、0。勾選移除無值，再將 X 從 1 編號，結果為 (1,10)、(2,20)、(3,0)。只有所有系列都無值的位置會移除，0 仍保留。",
      "X 是橫軸、Y 是縱軸，兩軸都能轉換。量測軸重新編號後會顯示序號，取代原量測值。變更轉換後會清除原上下限與參考線座標，請重新設定。"
    ]);
    const transforms=document.createElement("fieldset"),transformLegend=document.createElement("legend");
    transformLegend.textContent="座標轉換（X 為橫軸、Y 為縱軸）";transforms.append(transformLegend);
    const transformControls={};
    for(const axis of ["x","y"]) {
      const row=document.createElement("div");row.className="chart-transform-axis";transforms.append(row);
      const value=initial.transform[axis];
      const inputs={mode:field(row,`${axis.toUpperCase()} 軸處理`,value.mode,"text",{original:"保留原值",offset:"平移（加上固定值）",index:"重新編號"}),
        offset:field(row,"位移量",value.offset,"number"),start:field(row,"編號起點",value.start,"number"),step:field(row,"編號間距",value.step,"number")};
      for(const [name,input] of Object.entries(inputs)) input.dataset.transform=`${axis}-${name}`;
      transformControls[axis]=inputs;
    }
    const emptyLabel=document.createElement("label"),dropEmpty=document.createElement("input"),emptyText=document.createElement("span");
    emptyLabel.className="chart-transform-empty";dropEmpty.type="checkbox";dropEmpty.checked=initial.transform.drop_empty;dropEmpty.dataset.transform="drop-empty";
    emptyText.textContent="移除所有系列均無值的位置（0 保留；先移除，再重新編號）";emptyLabel.append(dropEmpty,emptyText);
    const transformHint=document.createElement("p");transformHint.textContent="平移保留原間距；重新編號改用資料顺序。量測軸重新編號後即不再表示原量測值。長條圖的類別位置保持等距。";
    const preview=document.createElement("p");preview.className="chart-transform-preview";preview.setAttribute("role","status");
    transforms.append(emptyLabel,transformHint,preview);
    pages[1].append(transforms);
    function readTransform() {
      const result={drop_empty:dropEmpty.checked};
      for(const axis of ["x","y"]) {
        const input=transformControls[axis], mode=input.mode.value;
        const read=name=>{if(!input[name].value.trim()) throw new Error("請填寫轉換數值。");return Number(input[name].value);};
        result[axis]={mode,offset:mode==="offset"?read("offset"):0,start:mode==="index"?read("start"):1,step:mode==="index"?read("step"):1};
      }
      return result;
    }
    const series=document.createElement("fieldset"),sl=document.createElement("legend");sl.textContent="系列名稱與顏色";series.append(sl);
    help(pages[2],"系列顏色與參考線",[
      "圖例名稱與顏色只改變呈現。參考線用來標示門檻：例如水平線 Y＝180，所有 X 位置都會畫在高度 180；垂直線 X＝5 則畫在第 5 個數值位置。",
      "座標必須填入；標示文字可以留白，此時只畫線、不顯示數字。若要顯示「180」或「上限」，請自行輸入。類別軸可用標籤或 #資料序號。"
    ]);
    const seriesInputs=initial.series.map(s=>{const row=document.createElement("div");row.className="chart-edit-grid";series.append(row);const name=field(row,"圖例名稱",s.name);name.maxLength=100;return {name,color:field(row,"顏色",s.color,"color")};});
    const lines=document.createElement("fieldset"),ll=document.createElement("legend");ll.textContent="參考線（最多10條）";lines.append(ll);const lineInputs=[];
    const add=document.createElement("button");add.type="button";add.textContent="新增參考線";
    function addLine(line={axis:"y",value:null,name:"上限",color:"#d62728"}, index=null) {
      if(lineInputs.length>=10) return;
      const row=document.createElement("div");row.className="chart-line-row";
      const entry={row,axis:field(row,"方向",line.axis,"text",{y:"水平線（Y）",x:"垂直線（X）"}),value:field(row,"座標",display(initialData,controls.kind.value,line.axis,line.value,initial.transform)),name:field(row,"標示文字",line.name),color:field(row,"顏色",line.color,"color")};
      entry.original=line;entry.labelPosition=index==null?null:initial.layout.reference_labels?.[index];
      entry.name.maxLength=100;entry.axis.onchange=()=>{entry.value.value="";};
      const remove=document.createElement("button");remove.type="button";remove.textContent="移除";remove.onclick=()=>{lineInputs.splice(lineInputs.indexOf(entry),1);row.remove();add.disabled=false;};row.append(remove);lines.append(row);lineInputs.push(entry);add.disabled=lineInputs.length>=10;
    }
    initial.lines.forEach(addLine);add.onclick=()=>addLine();pages[2].append(series,lines,add);
    function refreshTransform(reset=false) {
      for(const input of Object.values(transformControls)) {
        input.offset.disabled=input.mode.value!=="offset";
        input.start.disabled=input.step.disabled=input.mode.value!=="index";
      }
      if(reset){for(const key of ["x_min","x_max","y_min","y_max"]) controls[key].value="";for(const line of lineInputs) line.value.value="";rangeStatus.textContent="";}
      try {preview.textContent=ChartTransform.summary(qualityData(),controls.kind.value,readTransform()) || "保留全部原始座標。";preview.classList.remove("error");}
      catch(e){preview.textContent=e.message;preview.classList.add("error");}
    }
    for(const input of Object.values(transformControls)) for(const control of Object.values(input)) control.addEventListener("input",()=>refreshTransform(true));
    dropEmpty.onchange=()=>refreshTransform(true);
    let previousKind=initial.kind;
    controls.kind.onchange=()=>{
      if((previousKind==="horizontal_bar")!==(controls.kind.value==="horizontal_bar")) {
        const old=controls.x_label.value;controls.x_label.value=controls.y_label.value;controls.y_label.value=old;
        // 改變方向時讓轉換隨資料一起換軸，避免原類別的編號意外覆蓋量測值。
        for(const key of ["mode","offset","start","step"]){const value=transformControls.x[key].value;transformControls.x[key].value=transformControls.y[key].value;transformControls.y[key].value=value;}
      }
      previousKind=controls.kind.value;refreshTransform(true);
      hint.textContent="圖型已變更；轉換隨資料對調 X／Y。範圍已重設，請重新指定參考線。水平長條圖的 X 是量測值、Y 是原始類別。";
    };
    const error=document.createElement("p");error.className="error";error.setAttribute("role","alert");
    const actions=document.createElement("div");actions.className="dialog-actions";
    const cancel=document.createElement("button"),submit=document.createElement("button");cancel.type="button";cancel.textContent="取消";submit.type="submit";submit.className="primary";submit.textContent="套用";actions.append(cancel,submit);
    function close(){dialog.close();dialog.remove();}cancel.onclick=close;dialog.addEventListener("cancel",event=>{event.preventDefault();close();});
    dialog.addEventListener("close",()=>dialog.remove());
    let validationPage=0,validationField=null;
    form.onsubmit=event=>{event.preventDefault();try {
      validationPage=1;validationField=null;
      const style={title:controls.title.value.trim(),x_label:controls.x_label.value,y_label:controls.y_label.value,kind:controls.kind.value,legend:controls.legend.value,
        series:seriesInputs.map(s=>({name:s.name.value.trim(),color:s.color.value})),lines:[],layout:structuredClone(initial.layout),quality_policy:readQuality(),transform:readTransform()};
      const shown=ChartTransform.view(qualityData(),style.kind,style.transform);
      if(!style.title){validationPage=0;validationField=controls.title;throw new Error("標題不可空白。");}
      const unnamed=seriesInputs.find(s=>!s.name.value.trim());
      if(unnamed){validationPage=2;validationField=unnamed.name;throw new Error("系列名稱不可空白。");}
      validationPage=0;
      for(const axis of ["x","y"]){for(const bound of ["min","max"]){const key=`${axis}_${bound}`;style[key]=position(shown,style.kind,axis,controls[key].value,style.transform);}
        if(style[`${axis}_min`]!=null&&style[`${axis}_max`]!=null&&style[`${axis}_min`]>=style[`${axis}_max`]) throw new Error("下限必須小於上限。");}
      validationPage=2;
      style.lines=lineInputs.map(l=>{validationField=l.value;const value=position(shown,style.kind,l.axis.value,l.value.value,style.transform);if(value==null) throw new Error("請填參考線座標，或移除不需要的參考線。");return {axis:l.axis.value,value,name:l.name.value,color:l.color.value};});
      // 刪除／插入參考線時，位置隨原本那條線移動；改數值或換軸則重新自動定位。
      style.layout.reference_labels=lineInputs.map((entry,i)=>entry.original.axis===style.lines[i].axis && entry.original.value===style.lines[i].value ? entry.labelPosition??null : null);
      if(style.legend!==initial.legend) style.layout.legend=null;
      apply(style);close();
    } catch(e){selectPage(validationPage);validationField?.focus();error.textContent=e.message;}};
    refreshTransform();selectPage(0);form.append(heading,hint,tabs,body,error,actions);dialog.append(form);document.body.append(dialog);dialog.showModal();
  }
  return {open,defaults,position,category};
})();
