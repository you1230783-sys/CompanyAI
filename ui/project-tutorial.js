/* 專案操作練習：所有控制項與資料均建立在獨立 dialog 中。
 * 不引用正式專案控制器、不呼叫 send，也不讀寫檔案；只有完成事件交由外層保存。
 */
"use strict";
window.ProjectTutorial = (() => {
  const catalog = [
    {id:"projects.practice.create", group:"專案操作", title:"建立專案", description:"命名、選擇資料夾，開始第一個專案對話。", steps:6},
    {id:"projects.practice.files", group:"專案操作", title:"認識檔案清單", description:"辨識資料夾、README 與程式碼，確認來源範圍。", steps:6},
    {id:"projects.practice.request", group:"專案操作", title:"提出讀檔要求", description:"說清楚閱讀範圍與交付內容，查看示範回覆。", steps:5},
    ...(window.InteractiveLessons || []),
  ];
  const sampleRoot = "D:\\教學範例\\訂單小工具";
  const samplePrompt = "請先閱讀 README.md，再參考 src/main.cpp，整理主要流程與來源行號。只需要架構總覽，不必逐行閱讀，也不要修改原始檔案。";
  const readme = "# 訂單小工具\n\n用途：讀取訂單、檢查數量，計算總價。\n\n資料夾架構\n  README.md      使用說明\n  src/main.cpp   主要流程\n  data/orders.csv 範例訂單\n\n主要流程：輸入訂單 → 檢查數量 → 計算總價 → 顯示結果。";
  const code = "1  int main() {\n2      Order order = readOrder();\n3      if (order.quantity <= 0) {\n4          showError();\n5          return 1;\n6      }\n7      double total = order.price * order.quantity;\n8      showTotal(total);\n9      return 0;\n10 }";
  let host = null, options = {}, topic = null, step = 0, previousFocus = null;
  let stage, coach, spotlight, target, frame = 0, resizeObserver;
  let practice = {}, completed = new Set(), menuGroup = "全部", menuQuery = "";

  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text != null) node.textContent = text;
    return node;
  }
  function button(text, action, className = "secondary-button") {
    const node = element("button", className, text);
    node.type = "button";
    node.onclick = action;
    return node;
  }
  function ensureHost() {
    if (host) return;
    host = element("dialog", "project-tutorial");
    host.id = "project-tutorial";
    host.setAttribute("aria-labelledby", "project-tutorial-title");
    document.body.append(host);
    // Escape、外部點擊與右上角離開都走相同清理；不把離開記成完成。
    host.addEventListener("close", () => {
      cancelAnimationFrame(frame);
      resizeObserver?.disconnect();
      target = null;
      const callback = options.onClose;
      options = {};
      callback?.();
      if (previousFocus?.isConnected) previousFocus.focus();
    });
    host.addEventListener("keydown", event => {
      // 練習中的 Enter 不冒泡給正式對話的快捷鍵。
      if (event.key !== "Escape") event.stopPropagation();
    });
    host.addEventListener("submit", event => event.preventDefault());
    host.addEventListener("scroll", schedulePosition, true);
    window.addEventListener("resize", schedulePosition);
    resizeObserver = new ResizeObserver(schedulePosition);
  }
  function close() { if (host?.open) host.close(); }
  function open(settings = {}) {
    ensureHost();
    if (host.open) return;
    previousFocus = document.activeElement;
    options = settings;
    completed = new Set(settings.completed || []);
    menuGroup = settings.group || "全部"; menuQuery = "";
    topic = catalog.find(item => item.id === settings.topic) || null;
    step = 0;
    practice = {name:"", folder:false, prompt:"", values:{}};
    host.showModal();
    if (topic) paintLesson(); else paintMenu();
  }
  function heading(title) {
    const header = element("header", "practice-header");
    const titleBlock = element("div");
    titleBlock.append(element("span", "practice-badge", "教學模式 · 範例資料"));
    const h2 = element("h2", "", title);
    h2.id = "project-tutorial-title";
    titleBlock.append(h2);
    header.append(titleBlock, button(topic ? "離開教學" : "全部跳過", close));
    return header;
  }
  function paintMenu() {
    topic = null;
    resizeObserver.disconnect();
    target = null;
    host.classList.add("practice-menu-mode");
    host.replaceChildren(heading("想先練習哪個功能？"));
    host.append(element("p", "practice-intro", "親手操作一次，認識按鈕與使用流程。全部使用假資料與示範回覆，不會呼叫 AI，也不消耗模型 tokens。"));
    const filter = element("div", "practice-menu-filter");
    const searchLabel = element("label", "", "搜尋教學");
    const search = element("input"); search.type = "search"; search.id = "practice-topic-search";
    search.placeholder = "例如：流程圖、週報、VNC、用量…"; search.value = menuQuery; searchLabel.htmlFor = search.id;
    const groups = element("div", "practice-group-buttons");
    groups.setAttribute("aria-label", "教學分類");
    let selectedGroup = menuGroup;
    const status = element("p", "subtle"); status.setAttribute("role", "status");
    const cards = element("div", "practice-topic-sections");
    const groupNames = ["全部", ...new Set(catalog.map(item => item.group))];
    if (!groupNames.includes(selectedGroup)) selectedGroup = "全部";
    for (const group of groupNames) {
      const control = button(group, () => {
        selectedGroup = group; menuGroup = group; renderCards();
      }, "secondary-button");
      control.setAttribute("aria-pressed", String(group === selectedGroup));
      groups.append(control);
    }
    function topicCard(item, index) {
      const card = button("", () => {
        topic = item; step = 0;
        practice = {name:"", folder:false, prompt:"", values:{}};
        paintLesson();
      }, "practice-topic");
      card.append(element("span", "practice-topic-number", String(index + 1).padStart(2, "0")),
        element("h3", "", item.title), element("p", "", item.description),
        element("span", "practice-topic-status", `${completed.has(item.id) ? "已完成 · 再練習" : "開始練習"}　／　${item.steps} 步`));
      return card;
    }
    function renderCards() {
      const query = search.value.trim().toLocaleLowerCase();
      const visible = catalog.filter(item => (selectedGroup === "全部" || item.group === selectedGroup)
        && `${item.title} ${item.description} ${item.group}`.toLocaleLowerCase().includes(query));
      cards.replaceChildren();
      for (const group of groupNames.slice(1)) {
        const items = visible.filter(item => item.group === group); if (!items.length) continue;
        const section = element("section"); section.append(element("h3", "practice-group-heading", group));
        const grid = element("div", "practice-topics");
        items.forEach(item => grid.append(topicCard(item, catalog.indexOf(item))));
        section.append(grid); cards.append(section);
      }
      for (const control of groups.children) control.setAttribute("aria-pressed", String(control.textContent === selectedGroup));
      status.textContent = `共 ${catalog.length} 個教學，已完成 ${catalog.filter(item => completed.has(item.id)).length} 個；目前顯示 ${visible.length} 個。`;
      if (!visible.length) cards.append(element("p", "", "找不到符合的教學，可換個關鍵字或切回全部分類。"));
    }
    search.oninput = () => { menuQuery = search.value; renderCards(); };
    filter.append(searchLabel, search, groups, status);
    host.append(filter, cards, element("p", "subtle", "跳過的教學可以在「設定 → 教學與更新日誌」中再次觀看。"), button("全部跳過，返回使用", close));
    renderCards(); search.focus();
  }

  // 聚焦框只在模擬區域內投影；說明欄永遠可操作。以實際 DOM 位置計算，
  // 不固定螢幕座標，讓捲動、字型變動及視窗縮放後仍對準目前控制項。
  function schedulePosition() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(positionSpotlight);
  }
  function positionSpotlight() {
    if (!target?.isConnected || !stage?.isConnected || !spotlight) return;
    const area = stage.getBoundingClientRect(), box = target.getBoundingClientRect();
    spotlight.style.left = `${box.left - area.left + stage.scrollLeft - 5}px`;
    spotlight.style.top = `${box.top - area.top + stage.scrollTop - 5}px`;
    spotlight.style.width = `${box.width + 10}px`;
    spotlight.style.height = `${box.height + 10}px`;
  }
  function focusOn(node) {
    target = node;
    target.classList.add("practice-current");
    // 其他可操作項目實際停用；遮罩不是權限或隔離的唯一防線。
    for (const control of stage.querySelectorAll("button,input,textarea,select")) {
      control.disabled = !(control === target || target.contains(control));
    }
    resizeObserver.observe(stage);
    resizeObserver.observe(target);
    target.scrollIntoView({block:"nearest", inline:"nearest"});
    const focusable = target.matches("button,input,textarea,select") ? target : target.querySelector("input,textarea,select,button");
    if (focusable) focusable.focus({preventScroll:true});
    else { target.tabIndex = -1; target.focus({preventScroll:true}); }
    schedulePosition();
  }
  function next() { step++; paintLesson(); }
  function instruction(title, description, hint, manual = false) {
    coach.append(element("span", "practice-step-count", `步驟 ${step + 1} / ${topic.steps}`),
      element("h3", "", title), element("p", "", description), element("p", "practice-action-hint", hint));
    const actions = element("div", "practice-coach-actions");
    const previous = button("上一步", () => { step--; paintLesson(); });
    previous.disabled = step === 0;
    actions.append(previous);
    if (manual) actions.append(button("下一步", next, "primary-button"));
    else actions.append(element("span", "subtle", "完成框內操作後繼續"));
    coach.append(actions, button("回教學選單", paintMenu, "text-button"));
  }
  function shell(projectExists = true) {
    const grid = element("div", "practice-layout");
    stage = element("section", "practice-stage");
    stage.setAttribute("aria-label", "專案操作模擬畫面");
    const screen = element("div", "practice-screen");
    const sidebar = element("aside", "practice-sidebar");
    sidebar.append(element("strong", "practice-brand", "LM_AI"));
    const create = button("＋ 新增專案", next);
    const project = button(practice.name || "訂單小工具", next, "practice-project-link");
    if (topic?.scenes) {
      for (const title of ["一般對話", "Outlook 助理", "VNC 快速連線", "專案", "設定"]) sidebar.append(element("p", "practice-nav-item", title));
      sidebar.append(element("p", "practice-badge", topic.group));
    } else {
      sidebar.append(element("p", "subtle", "專案"), create);
      if (projectExists) sidebar.append(project);
      sidebar.append(element("p", "subtle", "最近對話"));
    }
    sidebar.append(element("p", "subtle", "這裡只顯示教學內容"));
    const main = element("section", "practice-main");
    main.append(element("h3", "", projectExists ? (practice.name || "訂單小工具") : "新對話"));
    if (projectExists) main.append(element("p", "practice-path", sampleRoot));
    screen.append(sidebar, main);
    spotlight = element("div", "practice-spotlight");
    spotlight.setAttribute("aria-hidden", "true");
    stage.append(screen, spotlight);
    coach = element("aside", "practice-coach");
    coach.setAttribute("aria-live", "polite");
    grid.append(stage, coach);
    host.append(grid);
    return {create, project, main};
  }
  function finish(main, message) {
    const result = element("section", "practice-result");
    result.append(element("h3", "", "這一項練習完成了！"), element("p", "", message),
      element("p", "subtle", "正式操作時換成自己的資料與需求；實際 AI 回覆與等待時間會依任務而異。"));
    main.append(result);
    coach.append(element("span", "practice-step-count", `步驟 ${topic.steps} / ${topic.steps}`),
      element("h3", "", "完成並回到選單"), element("p", "", "你可以繼續其他練習，也可以離開教學。"));
    const done = button("完成這個教學", () => {
      const id = topic.id;
      if (!completed.has(id)) { completed.add(id); options.onComplete?.(id); }
      paintMenu();
    }, "primary-button");
    coach.append(done, button("再練習一次", () => { step = 0; practice = {name:"", folder:false, prompt:"", values:{}}; paintLesson(); }), button("回教學選單", paintMenu, "text-button"));
    focusOn(result);
    done.focus();
  }
  function paintCreate() {
    const {create, main} = shell(step >= 4);
    if (step === 0) {
      main.append(element("p", "subtle", "從左側建立一個專案，集中這次工作的檔案。"));
      instruction("找到專案入口", "左側的「新增專案」會開始建立專案工作區。", "請點擊亮框中的「＋ 新增專案」。");
      focusOn(create); return;
    }
    if (step <= 3) {
      const form = element("section", "practice-project-form");
      form.append(element("h3", "", "新增專案"), element("p", "subtle", "正式使用時，請確認資料適合提供給 AI 服務。"));
      const nameGroup = element("div", "practice-field");
      const label = element("label", "", "專案名稱");
      const input = element("input"); input.id = "practice-project-name";
      input.maxLength = 60; input.value = practice.name; input.placeholder = "例如：訂單小工具";
      label.htmlFor = input.id;
      const apply = button("名稱填好了", () => { if (input.value.trim()) { practice.name = input.value.trim(); next(); } });
      apply.disabled = !input.value.trim();
      input.oninput = () => { practice.name = input.value; apply.disabled = !input.value.trim(); schedulePosition(); };
      nameGroup.append(label, input, button("填入範例名稱", () => { input.value = "訂單小工具"; input.dispatchEvent(new Event("input")); input.focus(); }), apply);
      const choose = button("選擇資料夾並建立", () => { practice.folder = true; next(); }, "primary-button");
      form.append(nameGroup, choose);
      main.append(form);
      if (step === 1) {
        instruction("替專案命名", "名稱用來辨識工作，例如「訂單小工具」或「每週報表」。", "輸入名稱，或按「填入範例名稱」，再按「名稱填好了」。");
        focusOn(nameGroup); apply.disabled = !input.value.trim();
      } else if (step === 2) {
        instruction("選擇工作資料夾", "正式使用時，這個按鈕會讓你選擇電腦上的資料夾。練習只會開啟模擬選擇器。", "請點擊「選擇資料夾並建立」。");
        focusOn(choose);
      } else {
        const picker = element("section", "practice-picker");
        picker.append(element("h3", "", "選擇資料夾（模擬）"), element("p", "practice-path", sampleRoot),
          element("p", "", "內含 README.md、src 與 data；不會存取你的磁碟。"));
        const confirm = button("選擇這個範例資料夾", next, "primary-button"); picker.append(confirm); main.append(picker);
        instruction("確認資料範圍", "把本次需要的檔案放在同一個工作資料夾，較容易說清楚來源。", "請選擇範例資料夾，完成模擬建立。"); focusOn(picker);
      }
      return;
    }
    if (step === 4) {
      const start = button("＋ 開始專案對話", next, "primary-button");
      main.append(element("p", "", "範例專案已加入左側清單。接著在專案內開始對話。"), start);
      instruction("開始專案對話", "專案對話會帶著這個工作資料夾的範圍，供 AI 依需求搜尋與閱讀。", "請點擊「開始專案對話」。"); focusOn(start); return;
    }
    finish(main, "你已練習命名、選擇資料夾與開始專案對話。這次沒有建立真實專案或資料夾。");
  }
  function files(main, nested = false) {
    const area = element("section", "practice-files");
    area.append(element("h4", "", nested ? "檔案清單 ／ src" : "檔案清單 ／ 訂單小工具"));
    const items = nested ? [["↰ 返回上層", null], ["▤ main.cpp", "code"]] : [["▤ README.md", "readme"], ["▸ src", "folder"], ["▸ data", null]];
    const controls = {};
    for (const [name, key] of items) {
      const item = button(name, next, "practice-file-row");
      area.append(item); if (key) controls[key] = item;
    }
    main.append(area);
    return controls;
  }
  function preview(main, title, contents) {
    const area = element("section", "practice-preview");
    area.append(element("strong", "", `${title} · 教學內容預覽`), element("pre", "", contents));
    main.append(area); return area;
  }
  function paintFiles() {
    const {project, main} = shell();
    if (step === 0) {
      instruction("先確認目前專案", "同一台電腦可以有多個專案。開始前先確認自己選到正確的工作資料夾。", "請點擊左側的「訂單小工具」。"); focusOn(project); return;
    }
    const rows = files(main, step === 4);
    if (step === 1) {
      instruction("先認識 README", "README 通常說明專案用途、操作方式與資料夾架構，可以優先作為總覽線索。", "請點擊 README.md，查看教學範例。正式閱讀由 AI 工具依要求執行。"); focusOn(rows.readme);
    } else if (step === 2) {
      const view = preview(main, "README.md", readme);
      instruction("用說明文件掌握全貌", "這個範例處理訂單，主要程式在 src/main.cpp，資料放在 data。", "看過範例內容後，按「下一步」。", true); focusOn(view);
    } else if (step === 3) {
      instruction("資料夾與檔案不同", "src 是資料夾，先展開或列出內容，再挑需要閱讀的檔案。", "請點擊 src 資料夾。"); focusOn(rows.folder);
    } else if (step === 4) {
      instruction("找到主要程式", "main.cpp 是範例的主要流程。看到清單不等於 AI 已經讀完內容，仍要提出明確要求。", "請點擊 main.cpp，完成來源辨識練習。"); focusOn(rows.code);
    } else {
      preview(main, "src/main.cpp", code);
      finish(main, "你已認識 README、資料夾與程式碼的關係。正式使用時，可要求 AI 先列清單、讀 README，再定位需要的檔案。");
    }
  }
  function paintRequest() {
    const {main} = shell();
    const context = element("section", "practice-context");
    context.append(element("strong", "", "目前來源：訂單小工具"), element("p", "", "README.md　／　src/main.cpp　／　data/orders.csv"));
    main.append(context);
    if (step === 0) {
      instruction("先核對來源", "這次要了解程式架構，先看 README 與主要程式就足以開始，不需要預設讀完整個資料夾。", "確認範例來源後，按「下一步」。", true); focusOn(context); return;
    }
    if (step <= 2) {
      const compose = element("section", "practice-composer");
      const label = element("label", "", "輸入訊息");
      const input = element("textarea"); input.id = "practice-prompt"; input.rows = 5; input.maxLength = 2000;
      input.value = practice.prompt; input.placeholder = "說明來源、想了解什麼，以及希望的成果…"; label.htmlFor = input.id;
      const ready = button("要求寫好了", () => { if (input.value.trim()) next(); });
      const sample = button("填入範例要求", () => { input.value = samplePrompt; input.dispatchEvent(new Event("input")); input.focus(); });
      input.oninput = () => { practice.prompt = input.value; ready.disabled = !input.value.trim(); schedulePosition(); };
      compose.append(label, input, sample, ready);
      const sendDemo = button("送出（教學示範）", next, "primary-button");
      main.append(compose, sendDemo);
      if (step === 1) {
        instruction("把要求寫清楚", "包含三件事：讀哪些來源、想了解什麼、希望如何呈現。可以自行輸入，或先使用範例。", "填寫後按「要求寫好了」。教學回覆固定展示這個訂單範例，不分析你輸入的新問題。"); focusOn(compose); ready.disabled = !input.value.trim();
      } else {
        instruction("送出閱讀要求", "正式送出後 AI 才會依任務使用工具。這裡直接展示準備好的回覆，不呼叫模型或產生等待。", "請點擊「送出（教學示範）」。"); focusOn(sendDemo);
      }
      return;
    }
    const reply = element("section", "practice-reply");
    reply.append(element("strong", "practice-badge", "預先準備的示範回覆"), element("h4", "", "訂單小工具：主要流程"),
      element("p", "", "輸入訂單 → 檢查數量 → 計算總價 → 顯示結果"),
      element("p", "", "數量 ≤ 0 時顯示錯誤並結束；通過檢查才計算總價。"),
      element("p", "practice-source", "來源：README.md；src/main.cpp 第 2–9 行。"),
      element("p", "subtle", "範圍：架構總覽。沒有執行程式，也沒有修改檔案。"));
    main.append(reply);
    if (step === 3) {
      instruction("核對結果與閱讀範圍", "留意來源、條件分支，以及哪些事情沒有驗證。需要更細的內容，可以接著指定某個函式深入。", "這是示範答案，不能用來證明目前模型的分析能力。看過後按「下一步」。", true); focusOn(reply);
    } else finish(main, "你已練習指定來源、撰寫需求、送出與查看來源依據。正式使用時不必先要求逐行讀完，除非你的任務需要完整閱讀。");
  }
  // 共用場景只接受宣告式的教學資料。以 textContent 與固定 SVG 元件呈現，
  // 不解析任意 HTML、腳本、外部網址或正式業務命令。
  function renderScene(scene) {
    const panel = element("section", "practice-scene");
    const values = practice.values;
    const title = element("h4", "", values[scene.bindTitle] || scene.title);
    if (scene.bindPosition) title.style.textAlign = ({"上方置中":"center", "左上角":"left", "右上角":"right"})[values[scene.bindPosition]] || "center";
    panel.append(title);
    if (scene.kind === "table") {
      const wrap = element("div", "practice-table-scroll");
      const table = element("table"); const head = element("thead"), row = element("tr");
      scene.columns.forEach(text => { const cell = element("th", "", text); cell.scope = "col"; row.append(cell); });
      head.append(row); const body = element("tbody");
      scene.rows.forEach(cells => { const row = element("tr"); cells.forEach(value => row.append(element("td", "", value))); body.append(row); });
      table.append(head, body); wrap.append(table); panel.append(wrap);
    } else if (scene.kind === "diff") {
      panel.append(element("p", "practice-diff-before", `修改前：${scene.before}`), element("p", "practice-diff-after", `修改後：${scene.after}`));
    } else if (scene.kind === "flow") {
      const graph = element("div", "practice-flow");
      scene.nodes.forEach((node, index) => { if (index) graph.append(element("span", "practice-flow-arrow", "↓")); graph.append(element("div", "practice-flow-node", node)); });
      panel.append(graph, element("p", "subtle", "例外分支：數量 ≤ 0 → 顯示錯誤並結束。"));
    } else if (scene.kind === "chart" || scene.kind === "cad") {
      const ns = "http://www.w3.org/2000/svg";
      const svg = document.createElementNS(ns, "svg");
      svg.setAttribute("viewBox", scene.kind === "chart" ? "0 0 320 180" : "0 0 140 100");
      svg.setAttribute("role", "img"); svg.setAttribute("aria-label", scene.title);
      svg.classList.add("practice-vector");
      const shape = (name, attrs, content) => {
        const node = document.createElementNS(ns, name);
        for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
        if (content != null) node.textContent = content;
        svg.append(node);
      };
      if (scene.kind === "chart") {
        shape("line", {x1:24, y1:145, x2:310, y2:145, class:"practice-vector-line"});
        scene.values.forEach((value, index) => {
          const height = value / 40 * 110, x = 35 + index * 70;
          shape("rect", {x, y:145-height, width:36, height, class:"practice-bar"});
          shape("text", {x:x+18, y:138-height, "text-anchor":"middle", class:"practice-vector-text"}, String(value));
          shape("text", {x:x+18, y:164, "text-anchor":"middle", class:"practice-vector-text"}, scene.labels[index]);
        });
        if (scene.reference) {
          const y = 145 - scene.reference / 40 * 110;
          shape("line", {x1:24, y1:y, x2:310, y2:y, class:"practice-reference"});
          shape("text", {x:306, y:y-4, "text-anchor":"end", class:"practice-vector-text"}, `${scene.reference}°C`);
        }
      } else {
        shape("rect", {x:20, y:20, width:100, height:60, class:"practice-geometry"});
        if (scene.points) [40,70,100].forEach(cx => shape("circle", {cx, cy:50, r:5, class:"practice-geometry"}));
        else shape("circle", {cx:70, cy:50, r:(scene.diameter||20)/2, class:"practice-geometry"});
        if (scene.dimension) {
          shape("text", {x:70, y:15, "text-anchor":"middle", class:"practice-cad-text"}, "100 mm");
          shape("text", {x:70, y:93, "text-anchor":"middle", class:"practice-cad-text"}, `孔徑 Ø${scene.diameter||20} mm`);
        }
      }
      panel.append(svg);
      if (scene.note || (scene.bindNote && values[scene.bindNote])) {
        const note = element("p", "practice-chart-note", values[scene.bindNote] || scene.note);
        if (scene.bindTextStyle) {
          const style = values[scene.bindTextStyle];
          if (style === "粗體") note.style.fontWeight = "bold";
          if (style === "斜體") note.style.fontStyle = "italic";
          if (style === "底線") note.style.textDecoration = "underline";
        }
        panel.append(note);
      }
      panel.append(element("p", "subtle", scene.kind === "cad" ? "教學示意，非實際 CAD 成果。" : "固定範例數據，非目前模型計算結果。"));
    } else if (scene.kind === "code") panel.append(element("pre", "", scene.body));
    else panel.append(element("p", scene.kind === "conversation" ? "practice-sample-answer" : "", scene.body));
    if (scene.note && scene.kind !== "chart") panel.append(element("p", "subtle", scene.note));
    return panel;
  }
  function paintExtended() {
    const {main} = shell(false);
    main.querySelector("h3").textContent = topic.title;
    if (step >= topic.scenes.length) { finish(main, `你已完成「${topic.title}」練習。所有資料與操作都是模擬，沒有影響正式設定、檔案或外部程式。`); return; }
    const lesson = topic.scenes[step], action = lesson.action;
    const scene = renderScene(lesson.scene);
    main.append(scene);
    instruction(lesson.title, lesson.description, action.kind === "button" ? `請操作亮框中的「${action.label}」。` : "請完成亮框中的範例操作，再按繼續。這些值只用於本教學。");
    if (action.kind === "button") {
      const trigger = button(action.label, next, "primary-button");
      // 核對型場景讓內容與操作一起保持明亮，避免來源或差異被遮住。
      if (action.label === "已核對範例，繼續") { scene.append(trigger); focusOn(scene); }
      else { main.append(trigger); focusOn(trigger); }
      return;
    }
    const group = element("section", "practice-field");
    const label = element("label", "", action.label); label.htmlFor = "practice-action-value";
    const apply = button("繼續", next, "primary-button");
    let control;
    const key = action.key || `step-${step}`;
    if (action.kind === "select") {
      control = element("select");
      control.add(new Option("請選擇…", "")); action.choices.forEach(choice => control.add(new Option(choice, choice)));
    } else if (action.kind === "check") {
      control = element("input"); control.type = "checkbox"; control.checked = Boolean(practice.values[key]);
      group.classList.add("practice-check-field");
    } else {
      control = element("textarea"); control.rows = 3; control.maxLength = 2000; control.placeholder = action.sample;
    }
    control.id = label.htmlFor;
    if (action.kind !== "check") control.value = practice.values[key] || "";
    const update = () => {
      const value = action.kind === "check" ? control.checked : control.value.trim();
      practice.values[key] = value; apply.disabled = !value; schedulePosition();
    };
    control.oninput = update; control.onchange = update;
    group.append(label, control);
    if (action.kind === "input") group.append(button("填入範例", () => { control.value = action.sample; update(); control.focus(); }));
    group.append(apply); main.append(group); focusOn(group); update();
  }
  function paintLesson() {
    resizeObserver.disconnect(); target = null;
    host.classList.remove("practice-menu-mode");
    host.replaceChildren(heading(`${topic.group} ／ ${topic.title}`));
    if (topic.id === catalog[0].id) paintCreate();
    else if (topic.id === catalog[1].id) paintFiles();
    else if (topic.id === catalog[2].id) paintRequest();
    else paintExtended();
  }
  return {catalog, open, close};
})();
