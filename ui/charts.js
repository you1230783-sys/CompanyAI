/* 固定圖表呈現器。模型只提供資料；不接受 formatter、option 或可執行字串。 */
"use strict";
window.ChartUI = (() => {
  const charts = new Map();
  const layouts = new Map();
  let pendingSave = null;
  const command=value=>send({type:"project",command:value});
  // 版本仍在原生資料保存，只在顯示／PNG拿掉完整版本碼。
  function sourceLabel(source) {return String(source || "").split("|").filter(s=>!/^\s*(?:excel:)?[a-f0-9]{64}\s*$/i.test(s)).map(s=>s.trim()).join(" | ");}
  // formatter 由程式固定提供；資料中的名稱／標籤須跳脫，不能被當成 HTML。
  const escapeTooltip = value => String(value).replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
  function tooltipFormatter(data, view, xCategory, yCategory) {
    return parameters => (Array.isArray(parameters) ? parameters : [parameters]).map(point => {
      const values = point.value || [];
      const coordinate = (axis, isCategory) => {
        const value = values[axis];
        return value == null ? "—" : isCategory ? (data.x[value] ?? "—") : value;
      };
      return `${escapeTooltip(point.seriesName || "")}<br>X（${escapeTooltip(view.x_label)}）：${escapeTooltip(coordinate(0, xCategory))}<br>Y（${escapeTooltip(view.y_label)}）：${escapeTooltip(coordinate(1, yCategory))}`;
    }).join("<br><br>");
  }
  const observer = new ResizeObserver(entries => {
    for (const {target} of entries) { charts.get(target)?.resize(); layouts.get(target)?.(); }
  });
  function cleanup() {
    for (const [element, instance] of charts) if (!element.isConnected) {
      observer.unobserve(element); instance.dispose(); charts.delete(element); layouts.delete(element);
    }
  }
  // 畫面與 PNG 共用資料／座標規則；匯出另建畫布，避免跟隨聊天室縮放或隱藏狀態。
  function option(data, exporting = false, style = null) {
    const view=style || ChartEditor.defaults(data), horizontal=view.kind==="horizontal_bar", scatter=view.kind==="scatter";
    data=ChartQuality.view(data,view.quality_policy);
    const transform=ChartTransform.settings(data,view), transformNote=ChartTransform.summary(data,view.kind,transform);
    // 自動量測範圍與編輯器共用算法；單邊手動設定仍保持使用者指定值。
    const measure=horizontal?"x":"y", range={x_min:view.x_min,x_max:view.x_max,y_min:view.y_min,y_max:view.y_max};
    if(range[`${measure}_min`]==null || range[`${measure}_max`]==null) {
      try {
        const [min,max]=ChartTransform.bounds(data,view.kind,transform,measure);
        range[`${measure}_min`]??=min;range[`${measure}_max`]??=max;
        // 手動單邊界線可能超出自動範圍；保留該側，另一側交由圖表引擎處理。
        if(range[`${measure}_min`]>=range[`${measure}_max`]) {
          if(view[`${measure}_min`]==null) range[`${measure}_min`]=null;
          if(view[`${measure}_max`]==null) range[`${measure}_max`]=null;
        }
      } catch { /* 全空值或浮點極端時，維持引擎預設與手動設定。 */ }
    }
    data=ChartTransform.view(data,view.kind,transform);
    const xCategory=ChartTransform.category(view.kind,"x",transform), yCategory=ChartTransform.category(view.kind,"y",transform);
    const legend={show:view.legend!=="hidden",type:"scroll",...(view.legend==="right"?{orient:"vertical",right:12,top:80,bottom:exporting?170:110}:view.legend==="top"?{top:55,left:"center"}:view.legend==="bottom_right"?{bottom:exporting?100:65,right:20}:{bottom:exporting?100:65,left:"center"})};
    const axis=(name,type,values,min,max,setting)=>({type,name,nameLocation:"middle",nameGap:40,...(values?{data:values}:{}),
      ...(type==="value" && setting.mode==="index"?{min:min??"dataMin",max:max??"dataMax",minInterval:Math.abs(setting.step)}:{...(min!=null?{min}:{}),...(max!=null?{max}:{})}),
      ...(type==="value" && setting.mode!=="original"?{scale:true}:{}),axisLabel:{hideOverlap:true}});
    const xa=axis(ChartTransform.axisLabel(view.x_label,transform.x),xCategory?"category":"value",xCategory?data.x:null,range.x_min,range.x_max,transform.x);
    const ya=axis(ChartTransform.axisLabel(view.y_label,transform.y),yCategory?"category":"value",yCategory?data.x:null,range.y_min,range.y_max,transform.y);
    return {animation:false, backgroundColor: exporting ? "#fff" : "transparent",
      aria:{enabled:true}, legend,
      tooltip:{trigger:scatter ? "item" : "axis", renderMode:"html",confine:true,formatter:tooltipFormatter(data,view,xCategory,yCategory)},
      grid:{left:exporting ? 110 : 75,right:view.legend==="right"?(exporting?240:170):40,top:view.legend==="top"?105:80,bottom:exporting ? 200 : 145,containLabel:true},
      title:{text:view.title,left:"center",top:16,textStyle:{fontSize:exporting?22:18,width:exporting?1450:550,overflow:"break"}},
      graphic:exporting ? [{type:"text",left:60,bottom:20,style:{text:`來源：${sourceLabel(data.source)}\n${[data.data_note,transformNote].filter(Boolean).join("\n")}`,fontSize:12,fill:"#444",width:1480,overflow:"break"}}] : [],
      dataZoom:exporting ? [] : [{type:"inside",filterMode:"none",...(horizontal?{yAxisIndex:0}:{xAxisIndex:0})},{type:"slider",bottom:10,filterMode:"none",...(horizontal?{yAxisIndex:0,orient:"vertical",right:0,top:80,bottom:145}:{xAxisIndex:0})}],
      xAxis:xa,yAxis:ya,
      series:data.series.map((s,index) => {
        const skip=new Set(s.skip_indices || []);
        // 明確 X 座標可略過單系列的異常點；真正空白仍保留 null，不能一起連線。
        const values=s.values.flatMap((v,i)=>skip.has(i) ? [] : [horizontal?[v,yCategory?i:data.x[i]]:[xCategory?i:data.x[i],v]]);
        return {name:view.series[index].name,type:["area","step"].includes(view.kind)?"line":horizontal?"bar":view.kind,
          ...(view.kind==="step"?{step:"end"}:{}),...(view.kind==="area"?{areaStyle:{opacity:0.2}}:{}),
          itemStyle:{color:view.series[index].color},lineStyle:{color:view.series[index].color},
          // 文字由共用排版器定位並加引線；markLine僅畫真正的數值線，不畫重複標籤。
          markLine:index===0?{symbol:["none","none"],silent:true,data:view.lines.map(l=>({name:l.name,[l.axis==="x"?"xAxis":"yAxis"]:l.value,lineStyle:{color:l.color,type:"dashed"},label:{show:false,formatter:()=>l.name,color:l.color}}))}:undefined,
          connectNulls:false,progressive:0,showSymbol:data.x.length <= 300,encode:{x:0,y:1},data:values};
      })};
  }
  function exportPng(data,style = null) {
    if (!data || !["line","bar","scatter","step","area","horizontal_bar"].includes(data.kind) || !Array.isArray(data.x) ||
        data.x.length < 1 || data.x.length > 10000 || !Array.isArray(data.series) ||
        data.series.length < 1 || data.series.length > 8 || data.series.some(s => s.values.length !== data.x.length)) {
      throw new Error("圖表資料不合法");
    }
    const canvas = document.createElement("div");
    Object.assign(canvas.style, {position:"fixed",left:"-20000px",top:"0",width:"1600px",height:"1000px",pointerEvents:"none"});
    document.body.append(canvas);
    let instance;
    try {
      instance = echarts.init(canvas, null, {renderer:"canvas",width:1600,height:1000,devicePixelRatio:1});
      const options=option(data,true,style);
      instance.setOption(options, {notMerge:true,lazyUpdate:false});
      ChartLayout.draw(instance,style || ChartEditor.defaults(data),true,options.graphic);
      // ECharts 可能對大型系列分批繪製；匯出必須同步完成全圖。
      instance.getZr().flush();
      return instance.getDataURL({type:"png",pixelRatio:1,backgroundColor:"#fff"});
    } finally { instance?.dispose(); canvas.remove(); }
  }
  // 異常明細使用原生預檢留下的原始列／格；分頁建立 DOM，避免大圖一次建立數萬列。
  // 所有來源內容只寫入 textContent，檔案中的 HTML 不會被當成介面執行。
  function renderIssues(data) {
    const details = document.createElement("details");
    details.className = "chart-issues";
    const summary = document.createElement("summary");
    summary.textContent = "查看異常值";
    const note = document.createElement("p");
    note.textContent = data.data_note || "這張圖沒有原始資料處理紀錄。";
    const table = document.createElement("table");
    const controls = document.createElement("div");
    const previous = document.createElement("button"), next = document.createElement("button");
    const label = document.createElement("span");
    previous.type = next.type = "button";
    previous.textContent = "上一頁";
    next.textContent = "下一頁";
    const rows = data.data_issues || [];
    let page = 0;

    function addRow(values, heading = false) {
      const row = table.insertRow();
      for (const value of values) {
        const cell = document.createElement(heading ? "th" : "td");
        cell.textContent = String(value ?? "—");
        row.append(cell);
      }
    }
    function showPage() {
      table.replaceChildren();
      addRow(["工作表／原始列", "儲存格", "系列", "X 原值", "Y 原值", "原始值／顯示值", "類型", "處理方式"], true);
      const start = page * 100, end = Math.min(start + 100, rows.length);
      for (let i = start; i < end; i++) {
        const row = rows[i];
        addRow([
          `${row.sheet}／${row.row}`, row.cell, row.series, row.x_value, row.y_value,
          `${JSON.stringify(row.original_value)}／${row.original_text}`, row.category, row.handling
        ]);
      }
      label.textContent = rows.length
        ? ` ${start + 1}–${end} / ${rows.length} 筆 `
        : " 沒有可列出的明細（舊版圖表可能僅有統計） ";
      previous.disabled = page === 0;
      next.disabled = end >= rows.length;
    }
    previous.onclick = () => { if (page > 0) { page--; showPage(); } };
    next.onclick = () => { if ((page + 1) * 100 < rows.length) { page++; showPage(); } };
    details.ontoggle = () => { if (details.open) showPage(); };
    controls.append(previous, label, next);
    details.append(summary, note, controls, table);
    return details;
  }

  function render(container, values, context = null) {
    const signature = JSON.stringify([values || [],context,ChartAppearance.palette()]);
    if (container.dataset.chartSignature === signature) return;
    container.dataset.chartSignature = signature;
    container.replaceChildren(); container.hidden = !values?.length; cleanup();
    for (const [chart_index,data] of (values || []).entries()) {
      let style=context?.styles?.[chart_index] || null;
      let layoutMode=false, beforeLayout=null, layoutBoxes=[], persistText=()=>{};
      const textPanel=document.createElement("div");textPanel.className="chart-text-panel";textPanel.hidden=true;
      function editText(index) {
        const current=(style || ChartEditor.defaults(data)).layout?.annotations?.[index];
        ChartText.open(current,annotation=>{
          style=structuredClone(style || ChartEditor.defaults(data));style.layout ??= ChartLayout.empty();style.layout.annotations ??= [];
          if(index==null) style.layout.annotations.push(annotation);else style.layout.annotations[index]=annotation;
          renderTextPanel();refreshLayout();if(!layoutMode)persistText();
        },()=>{
          style=structuredClone(style || ChartEditor.defaults(data));style.layout.annotations.splice(index,1);
          renderTextPanel();refreshLayout();if(!layoutMode)persistText();
        });
      }
      function renderTextPanel() {
        textPanel.replaceChildren();
        const annotations=(style || ChartEditor.defaults(data)).layout?.annotations || [];
        const add=document.createElement("button");add.type="button";add.textContent="新增文字";add.disabled=annotations.length>=20;add.onclick=()=>editText(null);textPanel.append(add);
        for(const [index,annotation] of annotations.entries()) {
          const row=document.createElement("div"),label=document.createElement("span"),edit=document.createElement("button");
          row.className="chart-text-row";label.textContent=annotation.text;edit.textContent="編輯文字";edit.type="button";
          edit.onclick=()=>editText(index);label.ondblclick=()=>editText(index);
          row.append(label,edit);textPanel.append(row);
        }
      }
      const card = document.createElement("section"); card.className = "chart-card";
      const title = document.createElement("h3"); title.textContent = data.title;title.className="chart-accessible-title";
      const plot = document.createElement("div"); plot.className = "chart-plot";
      plot.setAttribute("role", "img"); plot.setAttribute("aria-label", `${data.title}：${data.x_label} / ${data.y_label}`);
      const layoutHint=document.createElement("p");layoutHint.className="chart-layout-hint";layoutHint.setAttribute("role","status");layoutHint.hidden=true;
      function refreshLayout() {
        const instance=charts.get(plot);if(!instance)return;
        const result=ChartLayout.draw(instance,style || ChartEditor.defaults(data));
        layoutBoxes=result.boxes;
        layoutHint.hidden=!layoutMode&&!result.crowded;
        layoutHint.textContent=[layoutMode?"拖曳框線移動標題、圖例、參考線及自訂文字；方向鍵微調，Shift加速。按完成排版保存，取消則還原。":"",
          result.crowded?"文字空間不足，請放大圖表或縮短標籤。":""].filter(Boolean).join(" ");
        if(layoutMode) ChartLayout.handles(plot,result,(key,point)=>{
          style=structuredClone(style || ChartEditor.defaults(data));style.layout=ChartLayout.update(style.layout,key,point);
          refreshLayout();plot.querySelector(`[data-layout-key="${key}"]`)?.focus({preventScroll:true});
        },editText);
        else plot.querySelector(".chart-layout-layer")?.remove();
      }
      function redraw() {charts.get(plot)?.setOption(option(data,false,style),{notMerge:true});refreshLayout();}
      const source = document.createElement("p"); source.className = "chart-source";
      const updateSource=()=>{const shown=ChartQuality.view(data,(style || data.style)?.quality_policy);source.textContent=[sourceLabel(data.source),shown.data_note,ChartTransform.summary(shown,style?.kind || data.style?.kind || data.kind,ChartTransform.settings(data,style || data.style))].filter(Boolean).join("\n");};
      updateSource();
      const expand = document.createElement("button"); expand.className = "text-button"; expand.textContent = "放大圖表";
      expand.onclick = () => { const large = card.classList.toggle("chart-expanded"); expand.textContent = large ? "縮小圖表" : "放大圖表"; };
      const toolbar=document.createElement("div");toolbar.className="chart-toolbar";toolbar.append(expand);
      if(context?.request_id) {
        const target={conversation:context.conversation,message_index:context.message_index,request_id:context.request_id,chart_index};
        const edit=document.createElement("button"),reset=document.createElement("button"),save=document.createElement("button");
        edit.textContent="編輯圖表";reset.textContent="恢復原樣";save.textContent="儲存此圖片";
        const apply=value=>{style=value;const replacement=renderIssues(ChartQuality.view(data,(style || data.style)?.quality_policy));issueDetails.replaceWith(replacement);issueDetails=replacement;redraw();updateSource();command({action:"chart_customize",target,style});};
        persistText=()=>command({action:"chart_customize",target,style});
        edit.onclick=()=>ChartEditor.open(data,style,apply);
        // 以實際繪製後的框命中；一般檢視也能雙擊文字，單擊仍保留圖例與縮放功能。
        plot.ondblclick=event=>{
          if(layoutMode)return;
          const bounds=plot.getBoundingClientRect(),x=event.clientX-bounds.left,y=event.clientY-bounds.top;
          const box=layoutBoxes.find(b=>b.key.startsWith("text-") && x>=b.x && x<=b.x+b.width && y>=b.y && y<=b.y+b.height);
          if(box){event.stopPropagation();editText(Number(box.key.slice(5)));}else edit.onclick();
        };reset.onclick=()=>apply(null);
        const layout=document.createElement("button"),cancelLayout=document.createElement("button"),autoLayout=document.createElement("button");
        layout.textContent="調整排版";cancelLayout.textContent="取消排版";autoLayout.textContent="自動排版";
        cancelLayout.hidden=autoLayout.hidden=true;
        function toggleLayout(enabled) {
          layoutMode=enabled;textPanel.hidden=!enabled;if(enabled)renderTextPanel();layout.textContent=enabled?"完成排版":"調整排版";
          plot.setAttribute("role",enabled?"group":"img");
          cancelLayout.hidden=autoLayout.hidden=!enabled;edit.disabled=reset.disabled=save.disabled=enabled;
          refreshLayout();
        }
        layout.onclick=()=>{
          if(!layoutMode){beforeLayout=structuredClone(style);toggleLayout(true);}
          else {toggleLayout(false);command({action:"chart_customize",target,style});beforeLayout=null;}
        };
        cancelLayout.onclick=()=>{style=beforeLayout;beforeLayout=null;toggleLayout(false);redraw();};
        autoLayout.onclick=()=>{style=structuredClone(style || ChartEditor.defaults(data));const annotations=style.layout?.annotations || [];style.layout={...ChartLayout.empty(),annotations};refreshLayout();};
        save.onclick=()=>{
          if(pendingSave){toast("圖片正在儲存，請稍候。");return;}
          const pending={target,button:save};pendingSave=pending;save.disabled=true;
          command({action:"chart_save",target,style});
          // 登出或關閉頁面可能收不到ack；不永久鎖住按鈕，也不把逾時當作成功。
          setTimeout(()=>{if(pendingSave===pending){pendingSave=null;save.disabled=false;toast("尚未收到存圖結果，請先查看專案輸出資料夾。");}},65000);
        };
        toolbar.append(edit,layout,cancelLayout,autoLayout,reset,save);
      }
      const details = document.createElement("details"), summary = document.createElement("summary"); summary.textContent = "查看原始資料表";
      const table = document.createElement("table");
      const addRow = (values, header) => { const row = table.insertRow(); for (const value of values) { const cell = document.createElement(header ? "th" : "td"); cell.textContent = value == null ? "—" : String(value); row.append(cell); } };
      addRow([data.x_label, ...data.series.map(s => s.name)], true);
      // 一萬筆乘八個系列不可一次建立數萬個 DOM 節點；展開後每頁只建 100 列。
      let page = 0;
      const controls = document.createElement("div"), previous = document.createElement("button"), next = document.createElement("button"), label = document.createElement("span");
      previous.type = next.type = "button"; previous.textContent = "上一頁"; next.textContent = "下一頁";
      function showPage() {
        table.replaceChildren(); addRow([data.x_label, ...data.series.map(s => s.name)], true);
        const start = page * 100, end = Math.min(start + 100, data.x.length);
        for (let i = start; i < end; i++) addRow([data.x[i], ...data.series.map(s => s.values[i])], false);
        label.textContent = ` ${start + 1}–${end} / ${data.x.length} 筆 `;
        previous.disabled = page === 0; next.disabled = end >= data.x.length;
      }
      previous.onclick = () => { if (page > 0) { page--; showPage(); } };
      next.onclick = () => { if ((page + 1) * 100 < data.x.length) { page++; showPage(); } };
      details.ontoggle = () => { if (details.open) showPage(); };
      controls.append(previous, label, next);
      details.append(summary, controls, table); card.append(title, toolbar, textPanel, layoutHint, plot, source, details);
      let issueDetails=renderIssues(ChartQuality.view(data,(style || data.style)?.quality_policy));card.append(issueDetails); container.append(card);
      requestAnimationFrame(() => {
        if (!plot.isConnected) return;
        try {
          const instance = echarts.init(plot, null, {renderer:"canvas"}); charts.set(plot, instance); observer.observe(plot);
          instance.setOption(option(data,false,style));
          layouts.set(plot,refreshLayout);instance.on("datazoom",refreshLayout);refreshLayout();
        } catch { plot.textContent = "圖表無法顯示，請展開資料表查看。"; }
      });
    }
  }
  function receive(message) {
    if(message.type!=="chart_saved") return;
    if(pendingSave) pendingSave.button.disabled=false;
    const target=pendingSave?.target;pendingSave=null;
    toast(message.ok?`圖片已儲存：${message.message}`:message.message);
    if(message.ok && target && target.conversation===message.conversation){
      const notice=document.createElement("div");notice.className="chart-saved-notice";
      const text=document.createElement("span");text.textContent=`圖片已儲存：${message.message}`;
      const reveal=document.createElement("button");reveal.textContent="開啟所在資料夾";reveal.onclick=()=>command({action:"reveal",conversation:message.conversation,path:message.message});
      const close=document.createElement("button");close.textContent="關閉";close.onclick=()=>notice.remove();notice.append(text,reveal,close);document.body.append(notice);
    }
  }
  return {render, cleanup, exportPng, option, receive, sourceLabel};
})();
