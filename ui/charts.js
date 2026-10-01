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
      data.x.forEach((x, i) => addRow([x, ...data.series.map(s => s.values[i])], false));
      details.append(summary, table); card.append(title, expand, plot, source, details); container.append(card);
      requestAnimationFrame(() => {
        if (!plot.isConnected) return;
        try {
          const instance = echarts.init(plot, null, {renderer:"canvas"}); charts.set(plot, instance); observer.observe(plot);
          instance.setOption({animation:false, aria:{enabled:true}, legend:{top:0}, tooltip:{trigger:data.kind === "scatter" ? "item" : "axis", renderMode:"richText"},
            grid:{left:70,right:30,top:50,bottom:65},
            xAxis:{type:data.kind === "scatter" ? "value" : "category", name:data.x_label, nameLocation:"middle", nameGap:35, ...(data.kind === "scatter" ? {} : {data:data.x})},
            yAxis:{type:"value",name:data.y_label},
            series:data.series.map(s => ({name:s.name,type:data.kind,connectNulls:false,data:s.values.map((v,i) => data.kind === "scatter" ? [data.x[i],v] : v)}))});
        } catch { plot.textContent = "圖表無法顯示，請展開資料表查看。"; }
      });
    }
  }
  return {render, cleanup};
})();
