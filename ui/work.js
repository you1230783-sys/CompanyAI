/* 附件 File 只來自使用者選檔／貼上；逐塊等待 Rust 確認，不傳本機路徑或憑證。 */
"use strict";
let fileBatchBusy = false;
let fileWaiter = null;
let taskSignature = "";
function workCommand(command) {
  send({ type: "work", command });
}
function fileStep(command) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      fileWaiter = null;
      reject(new Error("附件接收逾時，請重新選取。"));
    }, 30000);
    fileWaiter = { resolve, reject, timer };
    workCommand(command);
  });
}
function receiveFile(message) {
  if (!fileWaiter || !["file_ack", "file_error"].includes(message.type)) return;
  const waiter = fileWaiter;
  fileWaiter = null;
  clearTimeout(waiter.timer);
  if (message.type === "file_error") waiter.reject(new Error(message.message));
  else waiter.resolve(message);
}
function bytesLabel(bytes) {
  if (!Number.isFinite(bytes)) return "—";
  return bytes >= 1048576
    ? (bytes / 1048576).toFixed(1) + " MB"
    : (bytes / 1024).toFixed(1) + " KB";
}
function durationLabel(seconds) {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds))
    return "未知";
  if (seconds < 60) return Math.ceil(seconds) + " 秒";
  if (seconds < 3600) return Math.ceil(seconds / 60) + " 分鐘";
  return (seconds / 3600).toFixed(1) + " 小時";
}
function timingLabel(timing) {
  if (!timing) return "尚無估時";
  return `排隊約 ${durationLabel(timing.estimated_wait_seconds)} · 處理約 ${durationLabel(timing.estimated_processing_seconds)} · 合計約 ${durationLabel(timing.estimated_total_seconds)}`;
}
const workLabels = {
  reading: "接收檔案",
  upload_pending: "等待上傳",
  uploading: "上傳中",
  awaiting_upload: "等待上傳",
  uploaded: "已上傳",
  queued: "排隊中",
  processing: "轉檔中",
  ready: "可送出",
  expired: "附件已到期，請重新選取",
  submitting: "確認接收中",
  running: "AI 處理中",
  cancelling: "正在取消",
  completed: "已完成",
  failed: "失敗",
  cancelled: "已取消",
  stopped: "已停止追蹤",
};
async function addFiles(files) {
  if (fileBatchBusy) {
    toast("正在接收上一批附件，請稍候。");
    return;
  }
  const work = state.work || {},
    rules = work.rules;
  if (!state.logged_in || !rules?.enabled || work.mode === "sync") {
    toast("請先登入並選擇網站支援的串流／背景模式。");
    return;
  }
  const existing = work.attachments || [];
  const limit = Math.min(20, rules.max_count);
  if (files.length + existing.length > limit) {
    toast(`文件與圖片合計最多 ${limit} 個。`);
    return;
  }
  let total = existing.reduce((sum, file) => sum + file.size, 0);
  for (const file of files) {
    const extension = "." + file.name.split(".").pop().toLowerCase();
    if (!rules.allowed_extensions.some((e) => e.toLowerCase() === extension)) {
      toast(`網站目前不支援 ${extension}`);
      return;
    }
    if (
      !file.size ||
      file.size > rules.max_file_bytes ||
      file.size > 0xffffffff ||
      (total += file.size) > rules.max_total_bytes
    ) {
      toast("附件大小超過網站限制，或檔案為空。");
      return;
    }
  }
  fileBatchBusy = true;
  renderWork();
  try {
    for (const file of files) {
      let id;
      try {
        const response = await fileStep({
          action: "file_begin",
          name: file.name,
          size: file.size,
          mime_type: file.type || "application/octet-stream",
        });
        id = response.id;
        for (let offset = 0; offset < file.size; offset += 192 * 1024) {
          const bytes = new Uint8Array(
            await file.slice(offset, offset + 192 * 1024).arrayBuffer(),
          );
          let binary = "";
          for (const byte of bytes) binary += String.fromCharCode(byte);
          await fileStep({
            action: "file_chunk",
            id,
            offset,
            data: btoa(binary),
          });
        }
        await fileStep({ action: "file_finish", id });
      } catch (error) {
        if (id) workCommand({ action: "file_abort", id });
        throw error;
      }
    }
  } catch (error) {
    toast(error.message);
  } finally {
    fileBatchBusy = false;
    $("attachment-input").value = "";
    renderWork();
  }
}
$("add-attachment").onclick = () => $("attachment-input").click();
$("attachment-input").onchange = (event) =>
  addFiles(Array.from(event.target.files || []));
$("prompt").addEventListener("paste", (event) => {
  const images = Array.from(event.clipboardData?.items || []).filter(
    (item) => item.kind === "file" && item.type.startsWith("image/"),
  );
  if (!images.length) return; // 一般文字維持正常貼上，不讀其他剪貼簿格式。
  event.preventDefault();
  const files = images.map((item) => item.getAsFile()).filter(Boolean);
  addFiles(files);
});
$("execution-sync").onclick = () =>
  workCommand({ action: "mode", mode: "sync" });
$("execution-stream").onclick = () =>
  workCommand({ action: "mode", mode: "stream" });
$("execution-background").onclick = () =>
  workCommand({ action: "mode", mode: "background" });
$("estimate-time").onclick = () =>
  workCommand({ action: "estimate", text: $("prompt").value });
$("refresh-tasks").onclick = () => workCommand({ action: "refresh" });
$("show-tasks").onclick = () => showView("tasks");
function actionButton(label, action) {
  const button = node("button", "secondary-button", label);
  button.type = "button";
  button.onclick = action;
  return button;
}
function renderWork() {
  const work = state.work || {},
    files = work.attachments || [],
    tasks = work.tasks || [],
    rules = work.rules;
  const available = state.logged_in && rules?.enabled && work.mode !== "sync";
  $("add-attachment").disabled =
    !available || fileBatchBusy || work.pending || state.update_required;
  $("attachment-input").accept = rules?.allowed_extensions?.join(",") || "";
  $("attachment-rules").textContent = rules?.enabled
    ? `最多 ${Math.min(20, rules.max_count)} 個 · 單檔 ${bytesLabel(Math.min(rules.max_file_bytes, 0xffffffff))} · 合計 ${bytesLabel(rules.max_total_bytes)} · ${rules.allowed_extensions.join("、")}`
    : "網站尚未啟用附件，仍可使用純文字聊天。";
  for (const mode of ["sync", "stream", "background"]) {
    $("execution-" + mode).hidden = !(work.modes || []).includes(mode);
    $("execution-" + mode).classList.toggle("selected", work.mode === mode);
    $("execution-" + mode).setAttribute(
      "aria-pressed",
      String(work.mode === mode),
    );
    $("execution-" + mode).disabled = fileBatchBusy;
  }
  $("estimate-time").hidden = !work.can_estimate;
  $("estimate-time").disabled = !state.can_send || fileBatchBusy;
  $("timing-estimate").textContent =
    work.draft_error ||
    (work.estimate
      ? timingLabel(work.estimate) + "（僅供參考）"
      : work.status || "");
  $("attachment-list").replaceChildren();
  for (const file of files) {
    const card = node("div", "attachment-card");
    const heading = node("div", "attachment-heading");
    const name = node("strong", "", file.name);
    name.title = file.name;
    heading.append(name, node("span", "subtle", bytesLabel(file.size)));
    card.append(
      heading,
      node(
        "div",
        "subtle",
        `${workLabels[file.state] || file.state}${file.queue_position ? " · 前方順位 " + file.queue_position : ""}`,
      ),
    );
    if (["uploading", "reading", "processing", "queued"].includes(file.state)) {
      const progress = node("progress", "work-progress");
      progress.max = 100;
      if (file.state === "uploading")
        progress.value = (100 * file.uploaded_bytes) / file.size;
      else if (file.progress !== null && file.progress !== undefined)
        progress.value = file.progress;
      progress.setAttribute("aria-label", file.name + " 處理進度");
      card.append(progress);
    }
    if (file.timing && ["queued", "processing"].includes(file.state))
      card.append(node("div", "subtle", timingLabel(file.timing)));
    if (file.message) card.append(node("div", "work-error", file.message));
    if (file.state === "failed")
      card.append(
        actionButton("重試上傳", () =>
          workCommand({ action: "retry_upload", id: file.id }),
        ),
      );
    const remove = actionButton("移除", () =>
      workCommand({ action: "remove_attachment", id: file.id }),
    );
    remove.disabled = fileBatchBusy || work.pending;
    card.append(remove);
    $("attachment-list").append(card);
  }
  if (fileBatchBusy) {
    $("send").disabled = true;
    $("new-chat").disabled = true;
    $("delete-chat").disabled = true;
    document
      .querySelectorAll(".history-item,[data-action]")
      .forEach((button) => (button.disabled = true));
  }
  $("task-count").textContent = tasks.filter((t) => t.active).length || "";
  const signature = JSON.stringify(
    tasks.map((t) => ({ ...t, partial: undefined })),
  );
  if (signature !== taskSignature) {
    taskSignature = signature;
    $("task-list").replaceChildren();
    if (!tasks.length)
      $("task-list").append(
        node("div", "empty-small", "尚無任務。串流與背景回覆會顯示在這裡。"),
      );
    for (const task of tasks) {
      const card = node("article", "task-card");
      card.append(
        node("h3", "", task.title || "附件分析"),
        node(
          "p",
          "",
          `${workLabels[task.state] || task.state}${task.queue_position ? " · 排隊順位 " + task.queue_position : ""}`,
        ),
      );
      if (task.active)
        card.append(node("p", "subtle", timingLabel(task.timing)));
      if (task.progress !== null && task.progress !== undefined) {
        const progress = node("progress", "work-progress");
        progress.max = 100;
        progress.value = task.progress;
        card.append(progress);
      }
      if (task.message) card.append(node("p", "subtle", task.message));
      const actions = node("div", "task-actions");
      actions.append(
        actionButton("開啟對話", () => {
          showView("chat");
          send({ type: "select_chat", id: task.conversation_id });
        }),
      );
      if (task.active && task.state !== "submitting")
        actions.append(
          actionButton("取消任務", () =>
            ask("取消工作", "伺服器會嘗試取消；若已完成，仍會取回結果。", () =>
              workCommand({ action: "cancel_task", id: task.id }),
            ),
          ),
        );
      if (task.can_retry)
        actions.append(
          actionButton("重試送出", () =>
            workCommand({ action: "retry_task", id: task.id }),
          ),
        );
      if (task.active)
        actions.append(
          actionButton("停止追蹤", () =>
            ask(
              "停止追蹤這個任務？",
              "立即解除本機等待並嘗試取消伺服器工作。404 或離線也能停止；無法保證伺服器已停止執行。",
              () => workCommand({ action: "stop_task", id: task.id }),
            ),
          ),
        );
      actions.append(
        actionButton("移除任務", () =>
          ask(
            "移除本機任務？",
            "保留對話內容，停止追蹤並移除任務卡片。若工作尚未完成，會嘗試取消；這不會刪除伺服器資料。",
            () => workCommand({ action: "remove_task", id: task.id }),
          ),
        ),
      );
      card.append(actions);
      $("task-list").append(card);
    }
  }
  const current = tasks.find(
    (t) => t.active && t.conversation_id === state.active_id,
  );
  const live = $("live-task");
  const partial = current?.partial || "";
  const liveSignature = JSON.stringify([
    current?.id,
    current?.state,
    partial,
    current?.queue_position,
  ]);
  if (live.dataset.signature !== liveSignature) {
    live.dataset.signature = liveSignature;
    live.replaceChildren();
    live.hidden = !current;
    if (current) {
      live.append(
        node(
          "div",
          "message-meta",
          "AI · " + (workLabels[current.state] || "處理中"),
        ),
      );
      const bubble = node("div", "bubble markdown");
      if (partial) bubble.innerHTML = renderMarkdown(partial);
      else
        bubble.textContent = current.queue_position
          ? `正在排隊，順位 ${current.queue_position}。可切換對話，完成後會通知。`
          : "正在等待伺服器，可切換對話或縮小到系統托盤。";
      live.append(bubble);
      if (stickToBottom) requestAnimationFrame(bottom);
    }
  }
}
window.WorkUI = {
  render: renderWork,
  receive: receiveFile,
  addFiles,
  durationLabel,
  timingLabel,
  getFileBusy: () => fileBatchBusy,
};
renderWork();
