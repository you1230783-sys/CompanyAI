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
    container.hidden = !coverage.length && !report;
    if (container.hidden) return;
    container.classList.add("analysis-panel");
    container.append(el("strong",finished ? "分析依據與處理範圍" : "分析概況"));
    if (state.review_required) container.append(el("p","已收到補充指示；以下舊結論等待重新核對。","analysis-attention"));
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
