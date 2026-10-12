/* 原生分析快照只以文字呈現；證據連結不接受模型提供的 URL 或 HTML。 */
"use strict";
window.AnalysisUI = (() => {
  const el = (tag, text = "", className = "") => {
    const element = document.createElement(tag);
    element.textContent = text; element.className = className; return element;
  };
  const statuses = {listed:"僅列出",pending:"尚未處理",partial:"部分範圍",complete:"此範圍完成",provided:"已交給計算",failed:"讀取／計算失敗"};
  function group(parent, title, items) {
    if (!items?.length) return;
    const section = el("section"); section.append(el("strong", title));
    const list = el("ul"); for (const text of items) list.append(el("li", text));
    section.append(list); parent.append(section);
  }
  function excerpt(value) {
    if (typeof value === "string") return value;
    if (Array.isArray(value)) return value.map(excerpt).join("\n");
    if (value && typeof value === "object") {
      if (Array.isArray(value.lines)) return excerpt(value.lines);
      if (typeof value.text === "string") return (value.line ? "第 " + value.line + " 行　" : "") + value.text;
      if (value.excerpt) return "第 " + (value.line || "?") + " 行　" + excerpt(value.excerpt);
    }
    return JSON.stringify(value, null, 2);
  }
  function render(container, state, finished = false) {
    const signature = JSON.stringify([state,finished]);
    if (container.dataset.signature === signature) return;
    const opened = new Set([...container.querySelectorAll("details[open][data-key]")].map(e=>e.dataset.key));
    const scroll = container.scrollTop;
    container.dataset.signature = signature; container.replaceChildren();
    const coverage = Object.values(state?.coverage || {}), report = state?.report;
    container.hidden = !coverage.length && !report && !state?.timeline?.length && !state?.sources?.length;
    if (container.hidden) return;
    container.classList.add("analysis-panel");
    container.append(el("strong",finished ? "分析依據與處理範圍" : "分析概況"));
    const completion=state.completion;
    if(completion?.scopes) {
      const card=el("section","","analysis-completion");
      card.append(el("strong",finished?"本次處理完成情況":"目前處理情況"));
      card.append(el("p",`${completion.files} 份檔案／${completion.scopes} 個處理範圍；${completion.unresolved_scopes} 個範圍待處理或核對。`));
      card.append(el("p",Object.entries(completion.counts||{}).map(([key,count])=>`${statuses[key]||key} ${count}`).join(" · ")));
      if(completion.failed_checks)card.append(el("p",`${completion.failed_checks} 項筆數核對未通過。`,"analysis-attention"));
      if(!completion.checks)card.append(el("small","尚無結構化筆數核對；不能以工具成功推論資料完整。"));
      card.append(el("small",completion.notice));container.append(card);
    }
    if (state.review_required) container.append(el("p","來源版本或指示已變更；以下舊結論等待重新核對。","analysis-attention"));
    if (report) {
      container.append(el("p","目標：" + report.goal),el("p","目前步驟：" + report.current_step));
      group(container,"待確認／缺少的證據",report.open_questions);
      group(container,"已失效的舊結論",report.superseded);
      for (const [index,finding] of (report.findings || []).entries()) {
        const card=el("section","","analysis-finding");
        card.append(el("p",(({confirmed:"AI 已確認",hypothesis:"待驗證假設",rejected:"已否定"})[finding.status] || finding.status) + "：" + finding.claim));
        for (const [position,evidence] of (finding.evidence || []).entries()) {
          const details=el("details");details.dataset.key="evidence:" + index + ":" + position + ":" + evidence.operation_id;
          const label=({calculation:"查看計算結果",snapshot:"查看資料快照",source:"查看原文依據"})[evidence.kind] || "查看工具依據";
          details.append(el("summary",label + (evidence.path ? " · " + evidence.path : "")));
          details.append(el("p","當時工具回傳的快照；可核對引文，不代表現在檔案未變更。"),el("pre",excerpt(evidence.excerpt)));
          const view=el("button","開啟來源預覽");view.onclick=()=>window.ReviewUI.source(evidence);details.append(view);
          const source=el("details");source.append(el("summary","來源與範圍"),el("pre",JSON.stringify({operation:evidence.operation_id,revision:evidence.revision,selection:evidence.selection,sources:evidence.sources},null,2)));
          details.append(source);card.append(details);
        }
        container.append(card);
      }
      if (report.checks?.length) {
        const checks=el("section"); checks.append(el("strong","筆數與反例檢查"));
        for (const check of report.checks) {
          const equation=check.kind==="balance" ? check.total + " = " + check.parts.map(p=>p.value).join(" + ") : "預期 0，實際 " + check.total;
          checks.append(el("p",(check.passed ? "通過" : "需檢查") + " · " + check.label + "（" + equation + "）",check.passed ? "" : "analysis-attention"));
        }
        checks.append(el("small","程式核對工具輸出的數量關係；解析規則與結論仍需原文驗證。"));container.append(checks);
      } else container.append(el("p","本次尚未提供結構化筆數核對。","muted"));
      if (report.method_note) container.append(el("p","已存入專案記憶：" + report.method_note.title + "。下次需先確認適用條件。"));
    }
    if(state.timeline?.length) {
      const section=el("details");section.dataset.key="timeline";
      section.append(el("summary",`LOG事件時間表 · ${state.timeline.length} 筆`),el("p","時間與原文由工具取得；事件分類／分組為AI判斷，先後順序不代表因果。日期可能沿用前文或檔名；日期不明的事件不自動配對。"));
      const filter=el("input");filter.type="search";filter.placeholder="篩選機台／批次、事件或來源";filter.setAttribute("aria-label","篩選事件");
      const list=el("div","","analysis-timeline");
      const draw=()=>{
        list.replaceChildren();const table=el("table"),head=el("tr");
        for(const title of ["時間","分組／事件","配對狀態","來源"])head.append(el("th",title));table.append(head);
        for(const event of state.timeline.filter(e=>JSON.stringify([e.group,e.label,e.path]).toLowerCase().includes(filter.value.toLowerCase()))) {
          const row=el("tr"),millis=event.time_millis;
          const time=millis==null?"時間未知":`${String(Math.floor(millis/3600000)).padStart(2,"0")}:${String(Math.floor(millis/60000)%60).padStart(2,"0")}:${String(Math.floor(millis/1000)%60).padStart(2,"0")}.${String(millis%1000).padStart(3,"0")}`;
          row.append(el("td",`${event.date||"日期未知"} ${time}${event.time_inherited?"（沿用前行）":""}`),el("td",`${event.group||"未分組"} · ${event.label}`),el("td",({paired:"已配對",missing_start:"缺少起點",missing_end:"缺少終點",ambiguous:"多個起點，待核對",uncertain:"時間／分組待確認",not_applicable:"一般事件"})[event.pair_status]||event.pair_status));
          const cell=el("td"),button=el("button",`${event.path}:${event.line}`);button.onclick=()=>window.ReviewUI.source(event,"LOG事件原文");cell.append(button);row.append(cell);table.append(row);
        }
        list.append(table);
      };filter.oninput=draw;draw();section.append(filter,list);container.append(section);
    }
    if(state.sources?.length) {
      const section=el("details");section.dataset.key="sources";section.append(el("summary",`文件搜尋來源 · ${state.sources.length} 處`));
      if(state.sources_truncated)section.append(el("p","來源預覽已達200處上限；其餘請查閱工具紀錄，不能視為完整命中清單。","analysis-attention"));
      const filter=el("input");filter.type="search";filter.placeholder="篩選來源檔名或命中文字";filter.setAttribute("aria-label","篩選來源");
      const list=el("div","","analysis-source-list");
      const draw=()=>{
        list.replaceChildren();
        for(const source of state.sources.filter(s=>JSON.stringify([s.path,s.excerpt]).toLowerCase().includes(filter.value.toLowerCase()))) {
          const card=el("section"),button=el("button",`${source.path} · ${source.label||"原文"} · 第${source.line||"?"}行`);
          button.onclick=()=>window.ReviewUI.source(source);card.append(button,el("p",String(source.excerpt||"").slice(0,180)));
          if(state.sources.some(s=>s.path===source.path&&s.revision!==source.revision))card.append(el("small","此檔有不同版本的搜尋紀錄，請核對來源版本。","analysis-attention"));
          list.append(card);
        }
      };filter.oninput=draw;draw();section.append(el("p","保存搜尋當時的來源與版本。查詢是否完整請看處理紀錄；預覽不是目前檔案的即時內容。"),filter,list);container.append(section);
    }
    if (coverage.length) {
      const details=el("details");details.dataset.key="coverage";
      const paths=new Set(coverage.map(c=>c.path.replaceAll("\\","/").toLowerCase()));
      details.append(el("summary","處理紀錄 · " + paths.size + " 份檔案／" + coverage.length + " 個範圍"));
      details.append(el("p","以下為實際工具紀錄。同一份檔案可能有多個條件；此範圍完成不代表所有問題都已解答。"));
      const list=el("div","","analysis-coverage");
      for (const item of coverage) {
        const row=el("div");row.append(el("strong",item.path),el("p",(statuses[item.status] || item.status) + " · " + (item.scope.startsWith("LOG 查詢") ? "LOG 條件搜尋" : item.scope)),el("small",item.detail));list.append(row);
      }
      details.append(list);container.append(details);
    }
    if (state.coverage_truncated) container.append(el("p","概況超過保存上限；不能視為完整清單，請查閱工具紀錄。","analysis-attention"));
    for (const details of container.querySelectorAll("details[data-key]")) details.open=opened.has(details.dataset.key);
    container.scrollTop=scroll;
  }
  return {render,excerpt};
})();
