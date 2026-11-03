/* 全部牌桌操作交本機原生牌局；問題只在使用者明確按下解析時送出。 */
"use strict";
window.TarotUI = (() => {
  const dialog = document.createElement("dialog");
  dialog.id = "tarot-dialog";
  dialog.dataset.offline = "true";
  dialog.className = "tarot-dialog";
  dialog.innerHTML = `<div class="tarot-heading"><div><h2>塔羅牌桌</h2><p>離線抽牌 · 自己選擇牌背，翻開本次指引</p></div><button type="button" data-close aria-label="關閉塔羅">✕</button></div>
    <div class="tarot-controls"><label>牌陣 <select data-count><option value="1">單張指引</option><option value="3">三張：現況／阻礙／建議</option></select></label><label><input type="checkbox" data-reverse checked>包含逆位</label><button data-shuffle class="primary-button">重新洗牌／開始新主題</button><span data-progress aria-live="polite"></span></div>
    <p class="subtle">同一副牌可以繼續提問；抽過的牌會保留在歷史中，重新洗牌前不會再出現。正逆位設定只在洗牌時生效。</p>
    <div class="tarot-deck" aria-label="78張牌背"></div><div class="tarot-results"></div><button data-continue class="secondary-button" disabled>繼續提問／從剩餘牌抽牌</button><details class="tarot-history"><summary>本副牌的抽牌紀錄</summary><div data-history></div></details><p class="subtle">牌面：1909 Rider–Waite–Smith／Pamela Colman Smith；圖像來源 mixvlad/TarotCards（Public Domain）。牌義為內建象徵解讀。</p>
    <div class="tarot-analysis"><button data-analyze class="secondary-button" disabled>給 AI 解析</button><div data-question-panel hidden><label>想詢問的問題<textarea data-question maxlength="3000" rows="3" placeholder="例如：面對目前的工作安排，我可以如何調整？"></textarea></label><p class="subtle">按送出後，問題與抽牌結果會送到普通對話，續問沿用同一對話並附上先前牌面紀錄。此步驟需要連線與登入。</p><button data-submit class="primary-button">送到一般對話</button></div></div><p data-error role="status"></p>`;
  document.body.append(dialog);
  const find = (selector) => dialog.querySelector(selector);
  let round = {
      id: "",
      count: 0,
      chosen: [],
      sent: false,
      history: [],
      used: [],
      remaining: 78,
    },
    busy = false;
  const call = (command) => {
    busy = true;
    find("[data-error]").textContent = "";
    render();
    send({ type: "tarot", command });
  };
  function render() {
    const deck = find(".tarot-deck");
    deck.replaceChildren();
    if (round.id)
      for (let position = 0; position < 78; position++) {
        const chosen = round.chosen.findIndex((c) => c.slot === position),
          used = (round.used || round.chosen.map((c) => c.slot)).includes(
            position,
          ),
          button = document.createElement("button");
        button.className = "tarot-back" + (used ? " picked" : "");
        button.disabled = busy || used || round.chosen.length >= round.count;
        button.setAttribute(
          "aria-label",
          `第 ${position + 1} 張牌背${used ? "，已抽過" : ""}`,
        );
        button.textContent = chosen >= 0 ? String(chosen + 1) : used ? "✓" : "";
        button.onclick = () => call({ action: "pick", id: round.id, position });
        deck.append(button);
      }
    find("[data-progress]").textContent = round.id
      ? `第 ${round.number || 1} 輪 · 已選 ${round.chosen.length}／${round.count} 張 · 剩餘 ${round.remaining ?? 78 - round.chosen.length} 張`
      : "選擇牌陣後開始";
    const results = find(".tarot-results");
    results.replaceChildren();
    for (const card of round.chosen) {
      const panel = document.createElement("article");
      panel.className = "tarot-card";
      const face = document.createElement("img");
      face.className = "tarot-face" + (card.reversed ? " reversed" : "");
      face.src = card.image;
      face.alt = card.name + "（" + (card.reversed ? "逆位" : "正位") + "）";
      face.width = 360;
      face.height = 600;
      panel.append(face);
      for (const [tag, text] of [
        ["small", card.position],
        ["h3", card.name],
        ["strong", card.reversed ? "逆位 ↓" : "正位 ↑"],
        ["p", card.meaning],
      ]) {
        const item = document.createElement(tag);
        item.textContent = text;
        panel.append(item);
      }
      results.append(panel);
    }
    const history = find("[data-history]");
    history.replaceChildren();
    for (const reading of round.history || []) {
      const block = document.createElement("section"),
        title = document.createElement("h4"),
        question = document.createElement("p");
      title.textContent = `第 ${reading.number} 輪`;
      question.textContent =
        reading.question || "本輪僅離線抽牌，尚未送AI解析。";
      block.append(title, question);
      for (const card of reading.chosen) {
        const line = document.createElement("p");
        line.textContent = `${card.position}：${card.name}（${card.reversed ? "逆位" : "正位"}）— ${card.meaning}`;
        block.append(line);
      }
      history.append(block);
    }
    find("[data-continue]").disabled =
      busy ||
      !round.count ||
      round.chosen.length !== round.count ||
      (round.remaining ?? 0) < Number(find("[data-count]").value);
    find("[data-analyze]").disabled =
      busy || round.sent || !round.count || round.chosen.length !== round.count;
    find("[data-submit]").disabled = busy || round.sent;
    find("[data-shuffle]").disabled = busy;
  }
  find("[data-close]").onclick = () => dialog.close();
  find("[data-count]").onchange = () => render();
  find("[data-continue]").onclick = () => {
    find("[data-question-panel]").hidden = true;
    call({
      action: "continue",
      id: round.id,
      count: Number(find("[data-count]").value),
    });
  };
  find("[data-shuffle]").onclick = () => {
    find("[data-question-panel]").hidden = true;
    find("[data-question]").value = "";
    call({
      action: "shuffle",
      count: Number(find("[data-count]").value),
      reversed: find("[data-reverse]").checked,
    });
  };
  find("[data-analyze]").onclick = () => {
    find("[data-question-panel]").hidden = false;
    find("[data-question]").focus();
  };
  find("[data-submit]").onclick = () => {
    const question = find("[data-question]").value.trim();
    if (!question) {
      find("[data-error]").textContent = "請先填寫問題。";
      return;
    }
    if (!state.logged_in) {
      find("[data-error]").textContent =
        "請先登入再使用AI解析；牌局與問題會保留。";
      return;
    }
    call({ action: "analyze", id: round.id, question });
  };
  $("show-tarot").onclick = () => {
    if (!dialog.open) dialog.showModal();
    call({ action: "state" });
  };
  return {
    open: () => $("show-tarot").click(),
    receive(message) {
      if (message.type === "tarot_state") {
        const completed =
          message.state.count > 0 &&
          message.state.chosen.length === message.state.count &&
          round.chosen.length < round.count;
        round = message.state;
        busy = false;
        render();
        // 抽齊才捲到牌義，避免第一張翻開後把其餘牌背移出視野。
        if (completed && dialog.open)
          find(".tarot-results").scrollIntoView({ block: "start" });
      }
      if (message.type === "tarot_error") {
        busy = false;
        render();
        find("[data-error]").textContent = message.message;
      }
      if (message.type === "tarot_sent") {
        dialog.close();
        showView("chat");
      }
    },
  };
})();
