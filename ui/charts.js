/* 固定圖表呈現器。模型只提供資料；不接受 formatter、option 或可執行字串。 */
"use strict";
window.ChartUI = (() => {
  const charts = new Map();
  const observer = new ResizeObserver(entries => {
    for (const {target} of entries) charts.get(target)?.resize();
  });
  function cleanup() {
    for (const [element, instance] of charts) if (!element.isConnected) {
      observer.unobserve(element); instance.dispose(); charts.delete(element);
    }
  }
  // 畫面與 PNG 共用資料／座標規則；匯出另建畫布，避免跟隨聊天室縮放或隱藏狀態。
  function option(data, exporting = false) {
    return {animation:false, backgroundColor: exporting ? "#fff" : "transparent",
      aria:{enabled:true}, legend:{top:exporting ? 80 : 0},
      tooltip:{trigger:data.kind === "scatter" ? "item" : "axis", renderMode:"richText"},
      grid:{left:exporting ? 100 : 70,right:40,top:exporting ? 140 : 50,bottom:exporting ? 160 : 100},
      title:exporting ? {text:data.title,left:60,top:20,textStyle:{fontSize:20,width:1480,overflow:"break"}} : undefined,
      graphic:exporting ? [{type:"text",left:60,bottom:20,style:{text:`來源：${data.source}`,fontSize:12,fill:"#444",width:1480,overflow:"break"}}] : [],
      dataZoom:exporting ? [] : [{type:"inside",filterMode:"none"},{type:"slider",bottom:10,filterMode:"none"}],
      xAxis:{type:data.kind === "scatter" ? "value" : "category",name:data.x_label,nameLocation:"middle",nameGap:35,...(data.kind === "scatter" ? {} : {data:data.x})},
      yAxis:{type:"value",name:data.y_label},
      series:data.series.map(s => ({name:s.name,type:data.kind,connectNulls:false,progressive:0,showSymbol:data.x.length <= 300,
        data:s.values.map((v,i) => data.kind === "scatter" ? [data.x[i],v] : v)}))};
  }
  function exportPng(data) {
    if (!data || !["line","bar","scatter"].includes(data.kind) || !Array.isArray(data.x) ||
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
      instance.setOption(option(data, true), {notMerge:true,lazyUpdate:false});
      // ECharts 可能對大型系列分批繪製；匯出必須同步完成全圖。
      instance.getZr().flush();
      return instance.getDataURL({type:"png",pixelRatio:1,backgroundColor:"#fff"});
    } finally { instance?.dispose(); canvas.remove(); }
  }
  function render(container, values) {
    const signature = JSON.stringify(values || []);
    if (container.dataset.chartSignature === signature) return;
    container.dataset.chartSignature = signature;
    container.replaceChildren(); container.hidden = !values?.length; cleanup();
    for (const data of values || []) {
      const card = document.createElement("section"); card.className = "chart-card";
      const title = document.createElement("h3"); title.textContent = data.title;
      const plot = document.createElement("div"); plot.className = "chart-plot";
      plot.setAttribute("role", "img"); plot.setAttribute("aria-label", `${data.title}：${data.x_label} / ${data.y_label}`);
      const source = document.createElement("p"); source.className = "chart-source"; source.textContent = data.source;
      const expand = document.createElement("button"); expand.className = "text-button"; expand.textContent = "放大圖表";
      expand.onclick = () => { const large = card.classList.toggle("chart-expanded"); expand.textContent = large ? "縮小圖表" : "放大圖表"; };
      const details = document.createElement("details"), summary = document.createElement("summary"); summary.textContent = "查看資料表";
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
      details.append(summary, controls, table); card.append(title, expand, plot, source, details); container.append(card);
      requestAnimationFrame(() => {
        if (!plot.isConnected) return;
        try {
          const instance = echarts.init(plot, null, {renderer:"canvas"}); charts.set(plot, instance); observer.observe(plot);
          instance.setOption(option(data));
        } catch { plot.textContent = "圖表無法顯示，請展開資料表查看。"; }
      });
    }
  }
  return {render, cleanup, exportPng};
})();
