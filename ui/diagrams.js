/* Mermaid 全程使用內嵌引擎。原始碼與歷史索引由原生端保存；圖片不是下一輪編輯來源。 */
"use strict";
window.DiagramUI = (() => {
  let sequence = 0;
  let queue = Promise.resolve();
  const cache = new Map();
  let pendingSave = null;
  const MAX_SOURCE = 48000;
  const command = value => send({type: "project", command: value});
  document.getElementById("diagram-source-close").onclick=()=>document.getElementById("diagram-source-dialog").close();

  // 一個共用事件即可涵蓋所有卡片；使用者按Esc退出時也同步恢復按鈕文字。
  document.addEventListener("fullscreenchange", () => {
    for (const button of document.querySelectorAll("[data-diagram-fullscreen]")) {
      const active = document.fullscreenElement === button.closest(".mermaid-card");
      button.textContent = active ? "返回對話（Esc）" : "全畫面";
      button.setAttribute("aria-pressed", String(active));
    }
  });

  /** 固定安全設定；來源不允許透過 directive/frontmatter 降低 securityLevel。 */
  function validateSource(source) {
    if (typeof source !== "string" || !source.trim() || new TextEncoder().encode(source).length > MAX_SOURCE) {
      throw new Error("原始碼須為 1–48000 bytes，大型流程請拆成多張圖。");
    }
    if (source.includes("%%{") || source.trimStart().startsWith("---")) throw new Error("不支援來源中的設定指令或 YAML frontmatter。");
    if (/^\s*click\s|(?:https?:|file:|javascript:|data:)\/\/|\b(?:img|image)\s*:/im.test(source)) {
      throw new Error("圖形不支援外部圖片、連結或 click 指令。");
    }
  }

  /** 防止匯出的 SVG 帶入外部資源。保留引擎產生的片段參照及必要 CSS。 */
  function safeSvg(svg) {
    const clean = DOMPurify.sanitize(svg, {USE_PROFILES: {svg: true, svgFilters: true}, ADD_TAGS: ["style"], FORBID_TAGS: ["foreignObject", "a", "image", "script", "iframe", "animate", "set"]});
    const doc = new DOMParser().parseFromString(clean, "image/svg+xml");
    const root = doc.documentElement;
    if (root.localName !== "svg" || doc.querySelector("parsererror")) throw new Error("SVG 結構不合法。");
    const safeCss = value => !/\\|@import|@font-face/i.test(value)
      && [...value.matchAll(/url\(([^)]*)\)/gi)].every(match => match[1].trim().replace(/^['"]|['"]$/g, "").startsWith("#"));
    for (const element of root.querySelectorAll("*")) {
      for (const attribute of [...element.attributes]) {
        if (/href$/i.test(attribute.name) && !attribute.value.startsWith("#")) element.removeAttribute(attribute.name);
        if (attribute.name === "style" && !safeCss(attribute.value)) element.removeAttribute("style");
      }
      if (element.localName === "style" && !safeCss(element.textContent)) element.remove();
    }
    root.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    root.style.backgroundColor = "white";
    return new XMLSerializer().serializeToString(root);
  }

  /** Mermaid 共用內部狀態，排隊呈現可避免多張圖同時重設設定及重複 DOM ID。 */
  function svgFor(source) {
    if (cache.has(source)) return cache.get(source);
    const task = queue.catch(() => {}).then(async () => {
      validateSource(source);
      if (!window.mermaid) throw new Error("離線 Mermaid 引擎未載入。");
      mermaid.initialize({startOnLoad: false, securityLevel: "strict", layout: "dagre", theme: "default", htmlLabels: false,
        fontFamily: "Microsoft JhengHei, Microsoft YaHei, Arial, sans-serif", maxTextSize: MAX_SOURCE, maxEdges: 500,
        suppressErrorRendering: true, flowchart: {htmlLabels: false, useMaxWidth: false}, sequence: {useMaxWidth: false}});
      await document.fonts.ready;
      const id = `lm-mermaid-${++sequence}`;
      try {
        const result = await mermaid.render(id, source);
        return safeSvg(result.svg);
      } finally {
        // 部分語法錯誤會留下引擎建立的暫存節點，不放進正常對話。
        document.getElementById(id)?.remove();
        document.getElementById(`d${id}`)?.remove();
      }
    });
    queue = task.catch(() => {});
    cache.set(source, task);
    task.catch(() => cache.delete(source));
    if (cache.size > 32) cache.delete(cache.keys().next().value);
    return task;
  }

  /** 同一來源可同時出現在歷史與新版卡片；DOM ID及片段參照須各自獨立。 */
  function displaySvg(svg) {
    const root = new DOMParser().parseFromString(svg, "image/svg+xml").documentElement;
    const ids = new Map([...root.querySelectorAll("[id]"), root].filter(e => e.id).map(e => [e.id, `${e.id}-view-${++sequence}`]));
    const replace = value => value.replace(/#([\w-]+)/g, (whole, id) => ids.has(id) ? `#${ids.get(id)}` : whole);
    for (const element of [root, ...root.querySelectorAll("*")]) {
      if (ids.has(element.id)) element.id = ids.get(element.id);
      for (const attr of [...element.attributes]) if (attr.name !== "id") element.setAttribute(attr.name, replace(attr.value));
      if (element.localName === "style") element.textContent = replace(element.textContent);
    }
    return new XMLSerializer().serializeToString(root);
  }

  /** 另建有界畫布，不依畫面縮放；長圖超過限制時改用 SVG，避免瀏覽器耗盡記憶體。 */
  async function pngFor(svg) {
    const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
    const root = doc.documentElement;
    const box = (root.getAttribute("viewBox") || "").trim().split(/[ ,]+/).map(Number);
    const width = box[2] || parseFloat(root.getAttribute("width"));
    const height = box[3] || parseFloat(root.getAttribute("height"));
    if (!(width > 0 && height > 0 && Number.isFinite(width + height))) throw new Error("無法取得圖形尺寸。");
    const scale = Math.min(2, 4096 / Math.max(width, height));
    const w = Math.ceil(width * scale), h = Math.ceil(height * scale);
    if (Math.min(w, h) < 16 || w * h > 16000000) throw new Error("圖形比例不適合 PNG，請匯出 SVG 或拆圖。");
    root.setAttribute("width", String(w));root.setAttribute("height", String(h));
    root.removeAttribute("style");
    const image = new Image();
    const loaded = new Promise((resolve, reject) => {image.onload = resolve;image.onerror = () => reject(new Error("PNG 圖形載入失敗。"));});
    // 分段編碼，避免大型 SVG 使用展開參數超過 JavaScript 呼叫堆疊上限。
    const bytes = new TextEncoder().encode(new XMLSerializer().serializeToString(root));
    let binary = "";
    for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
    image.src = `data:image/svg+xml;base64,${btoa(binary)}`;
    await loaded;
    const canvas = document.createElement("canvas");canvas.width = w;canvas.height = h;
    const context = canvas.getContext("2d");context.fillStyle = "white";context.fillRect(0, 0, w, h);context.drawImage(image, 0, 0, w, h);
    return canvas.toDataURL("image/png");
  }

  /** 原生固定入口以 awaitPromise 等待；錯誤也是資料，讓 AI 取得真正的語法失敗原因。 */
  async function exportData(source, format) {
    try {
      const svg = await svgFor(source);
      return JSON.stringify({ok: true, data: format === "png" ? await pngFor(svg) : svg});
    } catch (error) {
      return JSON.stringify({ok: false, error: String(error.message || error).slice(0, 2000)});
    }
  }

  function render(container, diagrams, context) {
    for (const [diagram_index, diagram] of diagrams.entries()) {
      const card = document.createElement("section");card.className = "mermaid-card";
      const title = document.createElement("h3");title.textContent = diagram.name;
      const toolbar = document.createElement("div");toolbar.className = "mermaid-toolbar";
      const viewport = document.createElement("div");viewport.className = "mermaid-viewport";viewport.tabIndex = 0;
      viewport.setAttribute("aria-label", `Mermaid 圖形：${diagram.name}`);
      const plot = document.createElement("div");plot.className = "mermaid-plot";viewport.append(plot);
      const status = document.createElement("p");status.className = "mermaid-status";status.textContent = "正在呈現圖形…";status.setAttribute("role", "status");
      const details = document.createElement("details"), summary = document.createElement("summary"), code = document.createElement("pre");
      summary.textContent = "查看 Mermaid 原始碼";code.textContent = diagram.source;details.append(summary, code);
      const target = {...context, diagram_index};
      card.dataset.diagramIndex=String(diagram_index);card.dataset.diagramRequest=context.request_id||"";
      const showSource=origin=>{
        const dialog=document.getElementById("diagram-source-dialog");dialog.dataset.conversation=context.conversation;
        dialog.dataset.operation=origin.operation_id;dialog.dataset.node=origin.node_id;
        document.getElementById("diagram-source-location").textContent=`${origin.path} · ${origin.locator}`;
        document.getElementById("diagram-source-notice").textContent="正在核對來源版本…";
        document.getElementById("diagram-source-text").value="";
        if(!dialog.open)dialog.showModal();
        command({action:"diagram_source",target,node_id:origin.node_id});
      };
      let sourcesPanel=null;
      if(diagram.node_sources?.length){
        const sourceDetails=document.createElement("details"),sourceTitle=document.createElement("summary");sourceTitle.textContent="節點來源與詳細圖";sourceDetails.append(sourceTitle);
        for(const origin of diagram.node_sources){
          const link=document.createElement("button");link.type="button";link.textContent=`${origin.node_id}：${origin.path} ${origin.locator}`;link.onclick=()=>showSource(origin);sourceDetails.append(link);
          if(origin.detail_diagram_index!=null){const more=document.createElement("button");more.textContent="查看詳細圖";more.onclick=()=>{const target=[...document.querySelectorAll(".mermaid-card")].find(c=>c.dataset.diagramRequest===context.request_id&&Number(c.dataset.diagramIndex)===origin.detail_diagram_index);if(target)target.scrollIntoView({behavior:"smooth",block:"start"});else toast("詳細圖不在目前顯示的訊息中");};sourceDetails.append(more);}
        }
        sourcesPanel=sourceDetails;
      }
      const button = (label, action) => {const b = document.createElement("button");b.type = "button";b.textContent = label;b.onclick = action;toolbar.append(b);return b;};
      button("複製原始碼", () => send({type: "copy", text: diagram.source}));
      const save = format => {
        if (pendingSave) {toast("圖形正在儲存，請稍候。");return;}
        pendingSave = {target};command({action: "diagram_save", target, format});
        const pending = pendingSave;
        setTimeout(() => {if (pendingSave === pending) {pendingSave = null;toast("尚未收到存檔結果，請查看專案輸出資料夾。");}}, 65000);
      };
      button("儲存 .mmd", () => save("mmd"));
      const svgButton = button("儲存 SVG", () => save("svg"));
      const pngButton = button("儲存 PNG", () => save("png"));
      svgButton.disabled = pngButton.disabled = true;
      let zoom = 1, naturalWidth = 0, naturalHeight = 0;
      const zoomLabel = document.createElement("span");zoomLabel.className = "mermaid-zoom";
      zoomLabel.setAttribute("role", "status");zoomLabel.textContent = "100%";
      const setZoom = value => {
        zoom = Math.max(0.1, Math.min(8, value));
        const svg = plot.querySelector("svg");
        if (svg && naturalWidth && naturalHeight) {
          // 明訂實際像素尺寸，避免全域18px圖示規則或Mermaid百分比寬度縮小圖形。
          // 改尺寸而非transform，捲動範圍會與可見圖形一致。
          svg.style.width = `${naturalWidth * zoom}px`;
          svg.style.height = `${naturalHeight * zoom}px`;
        }
        zoomLabel.textContent = `${Math.round(zoom * 100)}%`;
      };
      button("縮小", () => setZoom(zoom / 1.25));button("放大", () => setZoom(zoom * 1.25));button("原始比例", () => setZoom(1));
      button("符合視窗", () => {
        if (naturalWidth && naturalHeight) {
          setZoom(Math.min((viewport.clientWidth - 24) / naturalWidth, (viewport.clientHeight - 24) / naturalHeight, 1));
          viewport.scrollTo(0, 0);
        }
      });
      toolbar.append(zoomLabel);
      const fullscreen = button("全畫面", async () => {
        try {
          if (document.fullscreenElement === card) await document.exitFullscreen();
          else await card.requestFullscreen();
        } catch (_) {toast("無法切換全畫面，仍可在圖形區捲動與放大。");}
      });
      fullscreen.dataset.diagramFullscreen = "true";fullscreen.setAttribute("aria-pressed", "false");
      fullscreen.title = "全畫面時可按返回對話或Esc離開";
      button("請 AI 修改此圖", () => {
        const prompt = `請修改 Mermaid 圖形「${diagram.name}」（message_index=${context.message_index}，diagram_index=${diagram_index}，revision=${diagram.revision}）。請先用 inspect_mermaid 讀取原始碼，保留其餘內容。修改要求：\n`;
        // 只填入草稿，保留使用者既有內容，不自動送出。
        const input = document.getElementById("prompt");
        if (input) {input.value = input.value ? `${input.value}\n\n${prompt}` : prompt;input.dispatchEvent(new Event("input", {bubbles: true}));input.focus();}
      });
      card.append(title, toolbar, viewport, status, details);container.append(card);
      if(sourcesPanel)card.append(sourcesPanel);
      svgFor(diagram.source).then(svg => {
        if (!card.isConnected) return;
        plot.innerHTML = displaySvg(svg);
        const root = plot.querySelector("svg");
        for(const origin of diagram.node_sources||[]){
          // safeSvg會加上圖形命名空間，displaySvg再加view尾碼。
          // 以完整節點ID及數字序號比對兩種形式，避免A誤綁A-1。
          const nodePattern=new RegExp(`(?:^|-)flowchart-${origin.node_id}-\\d+(?:-view-\\d+)?$`);
          const group=[...root.querySelectorAll("g.node")].find(g=>nodePattern.test(g.id));
          if(group){group.style.cursor="pointer";group.setAttribute("tabindex","0");group.setAttribute("role","button");group.setAttribute("aria-label",`查看 ${origin.node_id} 的來源`);group.onclick=()=>showSource(origin);group.onkeydown=event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();showSource(origin);}};}
        }
        const box = root.viewBox.baseVal;
        naturalWidth = box.width || parseFloat(root.getAttribute("width"));
        naturalHeight = box.height || parseFloat(root.getAttribute("height"));
        if (!(naturalWidth > 0 && naturalHeight > 0 && Number.isFinite(naturalWidth + naturalHeight))) {
          throw new Error("無法取得流程圖尺寸");
        }
        setZoom(1);svgButton.disabled = pngButton.disabled = false;
        status.textContent = diagram.based_on ? "已由先前版本修改；舊圖保留。" : "Mermaid 原始碼已保存，可在後續對話修改。";
      }).catch(error => {status.textContent = `圖形無法呈現：${error.message}。原始碼仍可複製或儲存。`;details.open = true;});
    }
  }

  function receive(message) {
    if(message.type==="diagram_source"){
      const dialog=document.getElementById("diagram-source-dialog");
      if(!dialog.open||dialog.dataset.conversation!==state.active_id||message.conversation!==state.active_id)return;
      if(!message.ok){document.getElementById("diagram-source-notice").textContent=message.text;return;}
      try{const data=JSON.parse(message.text);if(data.source.operation_id!==dialog.dataset.operation||data.source.node_id!==dialog.dataset.node)return;document.getElementById("diagram-source-location").textContent=`${data.source.path} · ${data.source.locator} · 版本 ${data.source.revision.slice(0,12)}`;document.getElementById("diagram-source-notice").textContent=data.notice;document.getElementById("diagram-source-text").value=data.text;}catch{document.getElementById("diagram-source-notice").textContent="來源紀錄無法解析";}
      return;
    }
    if (message.type !== "diagram_saved") return;
    pendingSave = null;toast(message.ok ? `圖形已儲存：${message.message}` : message.message);
  }
  return {render, exportData, validateSource, safeSvg, receive};
})();
