/* 每次只送一筆設定；連續調整時合併為最新完整偏好，避免舊回覆覆蓋新選擇。 */
"use strict";
window.AdvancedUI = (() => {
  const menu = $("advanced-dialog");
  const outlook = $("show-outlook");
  // 移動既有按鈕，沿用事件與 active 狀態，不建立重複 ID。
  const outlookHome = document.createComment("Outlook 原進階功能位置");
  outlook.before(outlookHome);
  $("show-advanced").onclick = () => {
    if (!state.config.extra_features_enabled) return;
    $("show-outlook").disabled = !state.logged_in;
    menu.showModal();
  };
  menu.querySelector("[data-advanced-close]").onclick = () => menu.close();
  menu.addEventListener(
    "click",
    (e) => {
      const b = e.target.closest("button");
      if (!b || b.hasAttribute("data-advanced-close")) return;
      if (b.id === "show-outlook" && !state.logged_in) {
        e.stopImmediatePropagation();
        return;
      }
      menu.close();
    },
    true,
  );
  return {render() {
    const enabled = !!state.config.extra_features_enabled;
    $("show-advanced").hidden = !enabled;
    if (enabled) outlookHome.after(outlook);
    else $("show-advanced").before(outlook);
    outlook.disabled = !state.logged_in;
    if (!enabled) {
      for (const id of ["advanced-dialog", "pet-dialog", "pet-generator-dialog", "tarot-dialog"]) {
        const dialog = $(id);
        if (dialog?.open) dialog.close();
      }
    }
  }};
})();
window.PetUI = (() => {
  const actions = {
    idle: "待機",
    thinking: "思考",
    working: "工作",
    waiting: "等待重連",
    success: "完成／通知",
    error: "錯誤",
  };
  const dialog = document.createElement("dialog");
  dialog.id = "pet-dialog";
  dialog.dataset.offline = "true";
  dialog.className = "pet-dialog";
  dialog.innerHTML = `<div class="tarot-heading"><div><h2>桌寵與通知</h2><p>調整後立即生效並自動保存。</p></div><button data-close aria-label="關閉桌寵設定">✕</button></div>
  <div class="tarot-controls"><label><input type="checkbox" data-enabled>顯示桌寵</label><label>角色 <select data-skin><option value="cat">奶油貓</option><option value="dog">可可犬</option><option value="custom">我的角色</option></select></label><label>大小 <input type="range" data-size min="72" max="200" step="8"><output data-size-value></output></label><label><input type="checkbox" data-motion>減少動畫</label></div>
  <fieldset class="pet-bubble-controls"><legend>通知泡泡</legend><div class="tarot-controls"><label>樣式 <select data-style><option value="round">圓角對話</option><option value="comic">漫畫對話</option><option value="thought">思考泡泡</option></select></label><label>顏色 <input type="color" data-color></label><label>透明度 <input type="range" data-transparency min="0" max="100"><output data-alpha-value></output></label><label>顯示時間 <input type="range" data-seconds min="0" max="5" step="1"><output data-seconds-value></output></label><label>文字大小 <input type="range" data-font-size min="12" max="28" step="1"><output data-font-size-value></output></label><button data-preview>試看桌面通知</button></div><div class="pet-bubble-sample" data-sample>工作已完成，點一下查看。<small>這是通知泡泡預覽</small></div></fieldset>
  <p class="subtle">字型沿用應用程式設定；文字大小可獨立調整為12～28 px，預設16 px，不受主介面字級與縮放比例影響。文字自動以黑字白邊或白字黑邊呈現。</p>
  <p class="subtle">0 秒不顯示泡泡，通知仍可從通知中心查看。隱藏角色後，啟用的泡泡仍會顯示；通知開關沿用一般設定。拖曳角色可移動位置。</p>
  <p><strong>只接受 PNG，建議先去背。</strong>每個動作一張圖，保留透明度、最長邊縮至256像素，每張小於500 KB；原圖不變。</p><div class="pet-images"></div><button data-generator class="secondary-button">用 AI 生成自己的角色…</button><p data-message role="status"></p>`;
  document.body.append(dialog);
  const navigation=document.createElement('fieldset');
  navigation.innerHTML='<legend>點擊與右鍵捷徑</legend><label>點擊桌寵開啟 <select data-open-action></select></label><p>右鍵最上方固定為「開啟主介面」「隱藏桌寵」。下面可加入最多8個捷徑；新對話只建立空白對話。</p><div data-shortcuts></div>';
  dialog.querySelector('.pet-bubble-controls').before(navigation);
  const generator = document.createElement("dialog");
  generator.id = "pet-generator-dialog";
  generator.dataset.offline = "true";
  generator.className = "pet-dialog";
  generator.innerHTML = `<div class="tarot-heading"><h2>生成自己的角色</h2><button data-generator-close aria-label="關閉角色提示詞">✕</button></div><p>可用市面上的 AI 圖片工具生成，再回桌寵設定替換 PNG。<strong>請先修改最前面的角色名稱／類型</strong>，例如貓、狗、米奇，並指定一個動作。</p><label>可編輯的生成提示詞<textarea data-prompt rows="10" maxlength="8000"></textarea></label><button data-copy class="primary-button">複製提示詞</button><p data-copy-status role="status"></p><p class="subtle">六張圖請保持外觀、比例與腳底位置一致，可使用第一張作為後續參考。真正透明背景的PNG才是去背；畫出棋盤格不等於去背。程式不會自動去背。</p>`;
  document.body.append(generator);
  const find = (selector) => dialog.querySelector(selector),
    prompt = generator.querySelector("[data-prompt]");
  prompt.value =
    "角色名稱／類型：【請先修改，例如貓、狗、米奇】\n本次動作：【請選一個：待機、思考、工作、等待、完成、錯誤】\n\n請幫我生成一張桌寵用的圖片，要求PNG格式、真正透明背景且已去背，不要白底、背景場景或畫上棋盤格。建議256×256像素，完整角色置中，四周保留少量空間，不裁切耳朵、尾巴或四肢，不加入文字、標誌或邊框。角色在小尺寸仍需清楚辨識。\n若有參考圖，請保持外觀、配色、比例、視角與腳底位置一致。每張只放一個角色與一個動作，不合併成多格圖；後續只替換動作，維持相同畫布與角色位置。";
  let busy = false,
    queued = null,
    ready = false,
    imageKey = "";
  function transmit(command) {
    busy = true;
    send({ type: "pet", command });
  }
  function flush() {
    if (!busy && queued) {
      const c = queued;
      queued = null;
      transmit(c);
    }
  }
  function values() {
    return {
      action: "configure",
      enabled: find("[data-enabled]").checked,
      skin: find("[data-skin]").value,
      size: Number(find("[data-size]").value),
      reduced_motion: find("[data-motion]").checked,
      bubble_style: find("[data-style]").value,
      bubble_color: find("[data-color]").value,
      bubble_opacity: 100 - Number(find("[data-transparency]").value),
      notice_seconds: Number(find("[data-seconds]").value),
      font_size: Number(find("[data-font-size]").value),
      open_action: find('[data-open-action]').value || 'main',
      shortcuts: [...find('[data-shortcuts]').querySelectorAll('input:checked')].map(input=>input.value),
    };
  }
  function sample() {
    const v = values(),
      box = find("[data-sample]");
    box.dataset.style = v.bubble_style;
    box.style.backgroundColor = v.bubble_color;
    box.style.opacity = v.bubble_opacity / 100;
    box.style.setProperty("--bubble-color", v.bubble_color);
    const rgb = v.bubble_color
      .slice(1)
      .match(/../g)
      .map((v) => parseInt(v, 16));
    const darkText = rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114 > 145000;
    box.style.color = darkText ? "#000000" : "#ffffff";
    box.style.fontSize = v.font_size + "px";
    // 與原生桌寵一致：固定以相反的黑／白色向八個方向描邊。
    const stroke = v.font_size >= 22 ? 2 : 1;
    const outline = darkText ? "#ffffff" : "#000000";
    box.style.textShadow = [[-1,-1],[0,-1],[1,-1],[-1,0],[1,0],[-1,1],[0,1],[1,1]]
      .map(([x,y]) => `${x*stroke}px ${y*stroke}px 0 ${outline}`).join(",");
    find("[data-size-value]").textContent = v.size + " px";
    find("[data-font-size-value]").textContent = v.font_size + " px";
    find("[data-alpha-value]").textContent = 100 - v.bubble_opacity + "%";
    find("[data-seconds-value]").textContent = v.notice_seconds
      ? v.notice_seconds + " 秒"
      : "0 秒（不顯示）";
  }
  for (const selector of [
    "[data-enabled]",
    "[data-skin]",
    "[data-size]",
    "[data-motion]",
    "[data-style]",
    "[data-color]",
    "[data-transparency]",
    "[data-seconds]",
    "[data-font-size]",
    "[data-open-action]",
  ])
    find(selector).addEventListener("input", () => {
      sample();
      if (ready) {
        queued = values();
        flush();
      }
    });
  function command(value) {
    if (!busy && !queued) transmit(value);
  }
  function render(current) {
    if (!queued) {
      const s = current.settings;
      const targets=new Map(current.navigation_targets||[['main','開啟主介面']]);
      const selected=s.open_action||'main', shortcuts=s.shortcuts||[];
      for(const id of [selected,...shortcuts])if(!targets.has(id))targets.set(id,`目前不可用：${id}`);
      const menu=find('[data-open-action]'), list=find('[data-shortcuts]');menu.replaceChildren();list.replaceChildren();
      for(const [id,label] of targets) {
        menu.add(new Option(label,id));
        const row=document.createElement('label'), checkbox=document.createElement('input');checkbox.type='checkbox';checkbox.value=id;checkbox.checked=shortcuts.includes(id);
        checkbox.onchange=()=>{
          if(list.querySelectorAll('input:checked').length>8){checkbox.checked=false;find('[data-message]').textContent='右鍵捷徑最多8項。';return;}
          if(ready){queued=values();flush();}
        };
        row.append(checkbox,document.createTextNode(label));list.append(row);
      }
      menu.value=selected;
      find("[data-enabled]").checked = s.enabled;
      find("[data-skin]").value = s.skin;
      find("[data-size]").value = s.size;
      find("[data-motion]").checked = s.reduced_motion;
      find("[data-style]").value = s.bubble_style || "round";
      find("[data-color]").value = s.bubble_color || "#fff8ed";
      find("[data-transparency]").value = 100 - (s.bubble_opacity ?? 100);
      find("[data-seconds]").value = s.notice_seconds ?? 5;
      find("[data-font-size]").value = s.font_size ?? 16;
      sample();
    }
    const key = JSON.stringify(current.images);
    if (Array.isArray(current.images) && key !== imageKey) {
      imageKey = key;
      const images = find(".pet-images");
      images.replaceChildren();
      for (const item of current.images) {
        const card = document.createElement("div"),
          image = document.createElement("img"),
          label = document.createElement("strong"),
          button = document.createElement("button"),
          size = document.createElement("small");
        image.src = item.url;
        image.alt = actions[item.action];
        label.textContent = actions[item.action];
        size.textContent = (item.bytes / 1000).toFixed(1) + " KB";
        button.textContent = "替換 PNG";
        button.onclick = () =>
          command({ action: "import", state: item.action });
        card.append(label, image, size, button);
        images.append(card);
      }
    }
    ready = true;
  }
  find("[data-close]").onclick = () => dialog.close();
  find("[data-preview]").onclick = () => command({ action: "preview" });
  find("[data-generator]").onclick = () => generator.showModal();
  generator.querySelector("[data-generator-close]").onclick = () =>
    generator.close();
  generator.querySelector("[data-copy]").onclick = () => {
    generator.querySelector("[data-copy-status]").textContent = "";
    command({ action: "copy_prompt", text: prompt.value });
  };
  $("show-pet").onclick = () => {
    if (!state.config.extra_features_enabled || !featureEnabled("pet")) return;
    if (!dialog.open) dialog.showModal();
    command({ action: "state" });
  };
  return {
    open: () => $("show-pet").click(),
    receive(message) {
      if (message.type === "show_pet_settings") {
        this.open();
        return;
      }
      if (message.type === "show_tarot") {
        window.TarotUI?.open();
        return;
      }
      if (message.type === "pet_state") {
        busy = false;
        render(message.state);
        flush();
      }
      if (message.type === "pet_error") {
        busy = false;
        find("[data-message]").textContent = message.message;
        generator.querySelector("[data-copy-status]").textContent =
          message.message;
        flush();
      }
      if (message.type === "pet_imported") {
        find("[data-message]").textContent = message.message;
        generator.querySelector("[data-copy-status]").textContent =
          message.message;
      }
    },
  };
})();
