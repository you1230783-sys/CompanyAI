/* 六種固定動作共用相同行為；自訂角色只替換PNG，不接受腳本或遠端網址。 */
"use strict";
window.PetUI = (() => {
  const actions = {
    idle: "待機", thinking: "思考", working: "工作",
    waiting: "等待重連", success: "完成／通知", error: "錯誤"
  };
  const dialog = document.createElement("dialog");
  dialog.id = "pet-dialog";
  dialog.dataset.offline = "true";
  dialog.className = "pet-dialog";
  dialog.innerHTML=`<div class="tarot-heading"><div><h2>桌寵與通知</h2><p>換上喜歡的角色，工作狀態與通知由程式自動切換。</p></div><button data-close aria-label="關閉桌寵設定">✕</button></div>
    <div class="tarot-controls"><label><input type="checkbox" data-enabled>顯示桌寵</label><label>角色 <select data-skin><option value="cat">奶油貓</option><option value="dog">可可犬</option><option value="custom">我的角色</option></select></label><label>大小 <input type="range" data-size min="72" max="200" step="8"></label><label><input type="checkbox" data-motion>減少動畫</label><button data-save class="primary-button">套用</button><button data-preview>試看通知</button></div>
    <p class="subtle">拖曳角色移動；點擊回主視窗，右鍵可開塔羅或設定。隱藏角色後，啟用的通知仍以氣泡顯示；通知開關沿用一般設定。</p>
    <p><strong>只接受 PNG，建議先去背。</strong>每個動作一張圖，匯入後保留透明度、最長邊縮至256像素，每張小於500 KB。原圖不會被修改；未替換的動作使用基本款。</p>
    <div class="pet-images"></div><details><summary>用市面上的 AI 圖片工具生成自己的角色</summary><p><strong>請先修改最前面的角色名稱／類型</strong>，例如貓、狗、米奇；再填入本次動作。複製以下提示詞到你慣用的 AI 圖片工具，一次生成一張 PNG。</p><textarea rows="8" readonly data-prompt></textarea><p class="subtle">六張圖片請保持角色外觀、比例與腳底位置一致，可將第一張圖作為後續生成的參考。分別儲存PNG後，按對應動作的「替換PNG」。請確認下載的是有透明背景的PNG，畫出棋盤格不等於去背；程式不會自動去背。</p></details><p data-message role="status"></p>`;
  document.body.append(dialog);
  const find = selector => dialog.querySelector(selector);
  let busy = false;
  find("[data-prompt]").value="角色名稱／類型：【請先修改，例如貓、狗、米奇】\n本次動作：【請選一個：待機、思考、工作、等待、完成、錯誤】\n\n請幫我生成一張桌寵用的圖片，要求PNG格式、真正透明背景且已去背，不要白底、背景場景或畫上棋盤格。建議256×256像素，完整角色置中，四周保留少量空間，不裁切耳朵、尾巴或四肢，不加入文字、標誌或邊框。角色在小尺寸仍需清楚辨識，動作符合上方指定狀態。\n若有參考圖，請保持角色外觀、配色、比例、視角與腳底位置一致。每張只放一個角色與一個動作，不要合併成多格圖；後續只替換動作，維持相同畫布大小與角色位置。";
  // 等待原生端回覆期間不重複開啟檔案選擇器或送出設定。
  function call(command) {
    if (busy) return;
    busy = true;
    find("[data-message]").textContent = "";
    send({type: "pet", command});
  }

  function render(current) {
    find("[data-enabled]").checked = current.settings.enabled;
    find("[data-skin]").value = current.settings.skin;
    find("[data-size]").value = current.settings.size;
    find("[data-motion]").checked = current.settings.reduced_motion;
    const images = find(".pet-images");
    images.replaceChildren();
    for (const item of current.images) {
      const card = document.createElement("div");
      const image = document.createElement("img");
      const label = document.createElement("strong");
      const button = document.createElement("button");
      const size = document.createElement("small");
      image.src = item.url;
      image.alt = actions[item.action];
      label.textContent = actions[item.action];
      size.textContent = (item.bytes / 1000).toFixed(1) + " KB";
      button.textContent = "替換 PNG";
      button.onclick = () => call({action: "import", state: item.action});
      card.append(label, image, size, button);
      images.append(card);
    }
  }

  find("[data-close]").onclick = () => dialog.close();
  find("[data-save]").onclick = () => call({
    action: "configure",
    enabled: find("[data-enabled]").checked,
    skin: find("[data-skin]").value,
    size: Number(find("[data-size]").value),
    reduced_motion: find("[data-motion]").checked
  });
  find("[data-preview]").onclick = () => call({action: "preview"});
  $("show-pet").onclick = () => {
    dialog.showModal();
    call({action: "state"});
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
      if (message.type === "pet_imported" || message.type === "pet_error") {
        busy = false;
        find("[data-message]").textContent = message.message;
      }
      if (message.type === "pet_state") {
        busy = false;
        render(message.state);
      }
    }
  };
})();
