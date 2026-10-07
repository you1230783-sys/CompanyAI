/* 使用者呈現設定及有限座標轉換；不允許 ECharts option 或腳本，不改原始點陣。 */
"use strict";
window.ChartEditor = (() => {
  const kinds={line:"折線圖",bar:"直條圖",scatter:"散佈圖",step:"階梯線",area:"面積圖",horizontal_bar:"水平長條圖"};
  const palette=["#5470c6","#91cc75","#fac858","#ee6666","#73c0de","#3ba272","#fc8452","#9a60b4"];
  function defaults(data) {
    return {title:data.title,x_label:data.kind==="horizontal_bar"?data.y_label:data.x_label,y_label:data.kind==="horizontal_bar"?data.x_label:data.y_label,kind:data.kind,legend:"right",x_min:null,x_max:null,y_min:null,y_max:null,
      series:data.series.map((s,i)=>({name:s.name,color:palette[i]})),lines:[],transform:ChartTransform.settings(data)};
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
    initial.transform=ChartTransform.settings(data,current);
    const initialData=ChartTransform.view(data,initial.kind,initial.transform);
    const dialog=document.createElement("dialog");dialog.className="chart-edit-dialog";
    const form=document.createElement("form"),heading=document.createElement("h2");heading.textContent="自訂圖表";
    const hint=document.createElement("p");hint.textContent="原始資料保留。先移除無值位置，再轉換座標；範圍及參考線使用轉換後座標。類別軸可填標籤或 #資料序號。";
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
    for(const axis of ["x","y"]) for(const bound of ["min","max"]){const key=`${axis}_${bound}`;controls[key]=field(grid,`${axis.toUpperCase()}軸${bound==="min"?"下限":"上限"}`,display(initialData,initial.kind,axis,initial[key],initial.transform));}
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
    const seriesInputs=initial.series.map(s=>{const row=document.createElement("div");row.className="chart-edit-grid";series.append(row);const name=field(row,"圖例名稱",s.name);name.maxLength=100;return {name,color:field(row,"顏色",s.color,"color")};});
    const lines=document.createElement("fieldset"),ll=document.createElement("legend");ll.textContent="參考線（最多10條）";lines.append(ll);const lineInputs=[];
    const add=document.createElement("button");add.type="button";add.textContent="新增參考線";
    function addLine(line={axis:"y",value:null,name:"上限",color:"#d62728"}) {
      if(lineInputs.length>=10) return;
      const row=document.createElement("div");row.className="chart-line-row";
      const entry={row,axis:field(row,"方向",line.axis,"text",{y:"水平線（Y）",x:"垂直線（X）"}),value:field(row,"座標",display(initialData,controls.kind.value,line.axis,line.value,initial.transform)),name:field(row,"標示文字",line.name),color:field(row,"顏色",line.color,"color")};
      entry.name.maxLength=100;entry.axis.onchange=()=>{entry.value.value="";};
      const remove=document.createElement("button");remove.type="button";remove.textContent="移除";remove.onclick=()=>{lineInputs.splice(lineInputs.indexOf(entry),1);row.remove();add.disabled=false;};row.append(remove);lines.append(row);lineInputs.push(entry);add.disabled=lineInputs.length>=10;
    }
    initial.lines.forEach(addLine);add.onclick=()=>addLine();
    function refreshTransform(reset=false) {
      for(const input of Object.values(transformControls)) {
        input.offset.disabled=input.mode.value!=="offset";
        input.start.disabled=input.step.disabled=input.mode.value!=="index";
      }
      if(reset){for(const key of ["x_min","x_max","y_min","y_max"]) controls[key].value="";for(const line of lineInputs) line.value.value="";}
      try {preview.textContent=ChartTransform.summary(data,controls.kind.value,readTransform()) || "保留全部原始座標。";preview.classList.remove("error");}
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
    form.onsubmit=event=>{event.preventDefault();try {
      const style={title:controls.title.value.trim(),x_label:controls.x_label.value,y_label:controls.y_label.value,kind:controls.kind.value,legend:controls.legend.value,
        series:seriesInputs.map(s=>({name:s.name.value.trim(),color:s.color.value})),lines:[],transform:readTransform()};
      const shown=ChartTransform.view(data,style.kind,style.transform);
      if(!style.title || style.series.some(s=>!s.name)) throw new Error("標題和系列名稱不可空白。");
      for(const axis of ["x","y"]){for(const bound of ["min","max"]){const key=`${axis}_${bound}`;style[key]=position(shown,style.kind,axis,controls[key].value,style.transform);}
        if(style[`${axis}_min`]!=null&&style[`${axis}_max`]!=null&&style[`${axis}_min`]>=style[`${axis}_max`]) throw new Error("下限必須小於上限。");}
      style.lines=lineInputs.map(l=>{const value=position(shown,style.kind,l.axis.value,l.value.value,style.transform);if(value==null) throw new Error("請填參考線座標，或移除不需要的參考線。");return {axis:l.axis.value,value,name:l.name.value,color:l.color.value};});
      apply(style);close();
    } catch(e){error.textContent=e.message;}};
    refreshTransform();form.append(heading,hint,grid,transforms,series,lines,add,error,actions);dialog.append(form);document.body.append(dialog);dialog.showModal();
  }
  return {open,defaults,position,category};
})();
