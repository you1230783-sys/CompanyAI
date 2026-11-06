/* 純文字、圖片與本機空間管理。私聊訊息完全不使用 AI 回覆的 Markdown 呈現器。 */
"use strict";
window.PrivateChatMessages = (() => {
  const statuses = {
    pending:"等待送出確認", rejected:"送出失敗", sent:"送出",
    delivered:"送達", expired:"已過期，對方未收到",
  };

  function mount(pane, request, askInChat) {
    const find = selector => pane.querySelector(selector);
    const thread = find("[data-thread]");
    const footer = pane.querySelector("footer");
    const composer = document.createElement("form");
    composer.className = "private-chat-composer";
    composer.hidden = true;
    composer.innerHTML = `
      <label>私聊訊息<textarea rows="2" aria-label="私聊訊息"></textarea></label>
      <div class="button-row">
        <button data-send type="submit">送出文字</button>
        <button data-choose type="button">選擇圖片</button>
        <button data-paste type="button">貼上剪貼簿圖片</button>
      </div>
      <div data-draft-image class="button-row" hidden>
        <span data-draft-size></span><button data-send-image type="button">送出圖片</button>
        <button data-discard-image type="button">移除待送圖片</button>
      </div>`;
    footer.prepend(composer);
    const input = composer.querySelector("textarea");
    const imageDraft = composer.querySelector("[data-draft-image]");
    const sendText = composer.querySelector("[data-send]");
    const search = selector => composer.querySelector(selector);
    let friend = null;
    let busy = false;
    let canSend = false;

    const page = document.createElement("nav");
    page.className = "button-row";
    page.setAttribute("aria-label", "私聊訊息分頁");
    page.innerHTML = '<button data-older type="button" hidden>較早訊息</button><button data-newer type="button" hidden>較新訊息</button>';
    thread.before(page);
    const older = page.querySelector("[data-older]");
    const newer = page.querySelector("[data-newer]");
    older.onclick = () => request({action:"page", older:true});
    newer.onclick = () => request({action:"page", older:false});

    const space = document.createElement("dialog");
    space.id = "private-chat-space";
    space.setAttribute("aria-labelledby", "private-chat-space-title");
    space.innerHTML = `
      <h2 id="private-chat-space-title">管理私聊空間</h2>
      <p data-space-used>正在讀取本機資料大小…</p>
      <label class="check-row"><input data-only-images type="checkbox" checked />優先刪除圖片，保留文字</label>
      <label>刪除範圍
        <select data-period>
          <option value="all">所有圖片</option><option value="year">一年前</option>
          <option value="half_year">半年前</option><option value="quarter">三個月前</option>
          <option value="month">一個月前</option><option value="before_yesterday">昨天以前（UTC）</option>
        </select>
      </label>
      <p class="subtle">僅刪除你這邊的紀錄。刪除後無法復原。</p>
      <div class="button-row"><button data-clean type="button">清理本機資料</button><button data-close type="button">關閉</button></div>`;
    document.body.append(space);
    const onlyImages = space.querySelector("[data-only-images]");
    const period = space.querySelector("[data-period]");
    const usage = space.querySelector("[data-space-used]");
    onlyImages.onchange = () => {
      period.options[0].hidden = period.options[0].disabled = !onlyImages.checked;
      if (!onlyImages.checked && period.value === "all") period.value = "year";
    };
    space.querySelector("[data-close]").onclick = () => space.close();
    space.querySelector("[data-clean]").onclick = () => {
      const choice = {action:"cleanup", period:period.value, only_images:onlyImages.checked, confirmed:true};
      askInChat("清理私聊空間", "這會永久刪除指定範圍的本機資料。確定要清理？", () => request(choice));
    };
    const spaceButton = document.createElement("button");
    spaceButton.type = "button";
    spaceButton.textContent = "管理私聊空間";
    spaceButton.onclick = () => {
      if (!space.open) space.showModal();
      request({action:"space"});
    };
    footer.append(spaceButton);

    const picture = document.createElement("dialog");
    picture.id = "private-chat-image";
    picture.setAttribute("aria-label", "私聊圖片");
    picture.innerHTML = '<img alt="私聊圖片" /><div class="button-row"><button data-close type="button">關閉</button></div>';
    document.body.append(picture);
    const image = picture.querySelector("img");
    picture.querySelector("button").onclick = () => picture.close();
    picture.addEventListener("close", () => image.removeAttribute("src"));

    function textOkay() {
      return input.value.trim() && [...input.value].length <= 4000;
    }
    function controls() {
      canSend = Boolean(friend?.status === "active" && !busy);
      composer.hidden = !friend;
      input.disabled = !canSend;
      sendText.disabled = !canSend || !textOkay();
      for (const button of composer.querySelectorAll("button[type=button]")) {
        button.disabled = !canSend && !button.hasAttribute("data-discard-image");
      }
      for (const retry of thread.querySelectorAll("[data-retry]")) retry.disabled = !canSend;
    }
    function forFriend(action) {
      if (canSend) request({action, peer:friend.friend_id});
    }
    input.oninput = controls;
    composer.onsubmit = event => {
      event.preventDefault();
      if (canSend && textOkay()) request({action:"send_text", peer:friend.friend_id, text:input.value});
    };
    search("[data-choose]").onclick = () => forFriend("choose_image");
    search("[data-paste]").onclick = () => forFriend("paste_image");
    search("[data-send-image]").onclick = () => forFriend("send_image");
    search("[data-discard-image]").onclick = () => {
      request({action:"discard_image"});
      imageDraft.hidden = true;
      search("[data-draft-size]").textContent = "";
    };
    input.addEventListener("paste", event => {
      const items = [...(event.clipboardData?.items || [])];
      if (canSend && items.some(item => item.kind === "file" && item.type.startsWith("image/"))) {
        event.preventDefault();
        forFriend("paste_image");
      }
    });

    function action(label, callback, retry = false) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      button.onclick = callback;
      if (retry) {
        button.dataset.retry = "";
        button.disabled = !canSend;
      }
      return button;
    }
    function renderTimeline(message) {
      usage.textContent = "本機訊息索引及內容佔用 " + Number(message.bytes || 0).toLocaleString() + " 位元組。";
      if (message.peer !== friend?.friend_id) return;
      const rows = [];
      for (const entry of message.messages || []) {
        const row = document.createElement("article");
        row.className = entry.outgoing ? "private-message outgoing" : "private-message";
        const byline = document.createElement("header");
        const name = entry.outgoing ? "我" : friend.nickname + " · " + friend.employee_no;
        const serverTime = entry.time ? new Date(entry.time).toLocaleString() : "";
        byline.textContent = [name, serverTime, statuses[entry.state]].filter(Boolean).join(" · ");
        const content = document.createElement("p");
        const placeholders = {
          image:"（圖片）", removed:"（此訊息已被移除）", image_removed:"（圖片已移除）",
          expired_text:"（已過期的訊息）原類型：文字", expired_image:"（已過期的訊息）原類型：圖片",
        };
        content.textContent = entry.kind === "text" ? entry.text ?? "（無法讀取文字）" : placeholders[entry.kind] || "";
        row.append(byline, content);
        if (entry.reason) {
          const reason = document.createElement("p");
          reason.textContent = entry.reason;
          row.append(reason);
        }
        if (entry.kind === "image") row.append(action("查看圖片", () => request({action:"show_image", id:entry.id})));
        if (entry.kind !== "removed") {
          if (entry.outgoing && ["text","image"].includes(entry.kind) && ["pending","rejected"].includes(entry.state)) {
            row.append(action("重送", () => request({action:"retry", id:entry.id}), true));
          }
          row.append(action("僅刪除本機", () => askInChat("刪除私聊訊息", "僅刪除你這邊的紀錄，刪除後無法復原。", () => {
            request({action:"remove", id:entry.id, confirmed:true});
          })));
        }
        rows.push(row);
      }
      thread.replaceChildren(...rows);
      older.hidden = !message.older;
      newer.hidden = !message.newer;
      thread.scrollTop = message.newer ? 0 : thread.scrollHeight;
    }

    function clear() {
      friend = null;
      thread.replaceChildren();
      input.value = "";
      imageDraft.hidden = true;
      search("[data-draft-size]").textContent = "";
      usage.textContent = "";
      older.hidden = newer.hidden = true;
      if (space.open) space.close();
      if (picture.open) picture.close();
      image.removeAttribute("src");
      controls();
    }

    function receive(message) {
      if (message.type === "private_chat_timeline") {
        renderTimeline(message);
        return;
      }
      if (message.peer !== friend?.friend_id) return;
      if (message.type === "private_chat_image_draft") {
        imageDraft.hidden = false;
        search("[data-draft-size]").textContent = "待送 JPEG · " + Number(message.bytes).toLocaleString() + " 位元組";
      } else if (message.type === "private_chat_composer_sent") {
        if (message.kind === "image") {
          imageDraft.hidden = true;
          search("[data-draft-size]").textContent = "";
        } else input.value = "";
        controls();
      } else if (message.type === "private_chat_image" && typeof message.url === "string"
          && message.url.startsWith("data:image/jpeg;base64,")) {
        image.src = message.url;
        if (!picture.open) picture.showModal();
      }
    }

    return {
      clear, receive,
      renderFriend(next, waiting) {
        if (next?.friend_id !== friend?.friend_id) clear();
        friend = next;
        busy = waiting;
        controls();
      },
      select(peer) { request({action:"select", peer}); },
    };
  }

  return {mount};
})();
