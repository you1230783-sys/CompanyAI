/* 郵件代號僅引用本次 Rust 快照；永遠不向介面提供 Outlook EntryID 或 StoreID。 */
"use strict";
let mailListSignature = "";
const selectedMails = new Set();
function mailCommand(command) {
  send({ type: "mail_batch", command });
}
function canAutoExportMail() {
  return state.mail_batch?.quality_status === "available" &&
    state.models.some(model => model.id === "quality");
}
function renderQualityAvailability(status = state.mail_batch?.quality_status) {
  const messages = {
    checking: "正在確認品質模型是否可用…",
    unavailable: "目前品質模型維護中，暫時停用自動補充內文功能。",
    error: "無法確認品質模型狀態，暫時停用自動補充內文。請重新進入 Outlook 助理或重新整理服務。",
    signed_out: "登入後可確認品質模型是否可用。",
  };
  const available = status === "available" && state.models.some(model => model.id === "quality");
  $("batch-auto-export").disabled = !!state.mail_batch?.busy || !available;
  // 查詢期間保留使用者選擇；確認停用或查詢失敗後清除，恢復時須由使用者重新勾選。
  if (["unavailable", "error", "signed_out"].includes(status)) $("batch-auto-export").checked = false;
  $("batch-quality-status").textContent = available ? "" : (messages[status] || messages.checking);
  $("batch-quality-status").hidden = available;
}
function renderMailBatch() {
  const batch = state.mail_batch || {},
    mails = batch.list?.mails || [];
  const signature = JSON.stringify(mails);
  if (signature !== mailListSignature) {
    mailListSignature = signature;
    selectedMails.clear();
    mails.forEach((mail) => selectedMails.add(mail.id));
    $("batch-mail-list").replaceChildren();
    for (const mail of mails) {
      const row = node("label", "batch-mail-row"),
        checkbox = node("input");
      checkbox.type = "checkbox";
      checkbox.checked = true;
      checkbox.dataset.mailId = mail.id;
      checkbox.onchange = () => {
        if (checkbox.checked) selectedMails.add(mail.id);
        else selectedMails.delete(mail.id);
        renderMailBatch();
      };
      const details = node("div");
      details.append(
        node("strong", "", mail.subject || "（無主旨）"),
        node("p", "subtle", mail.folder || ""),
        node("p", "subtle", `${mail.sender} → ${mail.to}`),
        node(
          "span",
          "subtle",
          `${mail.received_at} · ${mail.unread ? "未讀" : "已讀"}`,
        ),
      );
      row.append(checkbox, details);
      $("batch-mail-list").append(row);
    }
  }
  $("batch-status").textContent =
    batch.status || "讀取基本資訊後，勾選要分析的郵件。";
  $("batch-selected-count").textContent =
    `已勾選 ${selectedMails.size} / ${mails.length} 封`;
  $("batch-selected").disabled = !!batch.busy;
  $("batch-scope").disabled = !!batch.busy;
  document
    .querySelectorAll("[data-mail-period],#batch-mail-list input")
    .forEach((button) => (button.disabled = !!batch.busy));
  $("batch-select-all").disabled = !!batch.busy || !mails.length;
  renderQualityAvailability();
  $("batch-analyze").disabled =
    !!batch.busy || !selectedMails.size || !state.can_send;
  $("batch-stop").disabled = !batch.busy;
  $("batch-open").disabled = !batch.conversation_id;
}
$("batch-selected").onclick = () =>
  mailCommand({ action: "list", period: "selected", unread: false });
document.querySelectorAll("[data-mail-period]").forEach(
  (button) =>
    (button.onclick = () =>
      mailCommand({
        action: "list",
        period: button.dataset.mailPeriod,
        unread: button.dataset.unread === "true",
        scope: $("batch-scope").value,
      })),
);
$("batch-select-all").onclick = () => {
  const all =
    selectedMails.size === (state.mail_batch?.list?.mails || []).length;
  selectedMails.clear();
  document.querySelectorAll("#batch-mail-list input").forEach((input) => {
    input.checked = !all;
    if (!all) selectedMails.add(input.dataset.mailId);
  });
  renderMailBatch();
};
$("batch-analyze").onclick = () => {
  const allow = $("batch-auto-export").checked && canAutoExportMail();
  const selectedIds = [...selectedMails];
  ask(
    `分析勾選的 ${selectedMails.size} 封郵件？`,
    allow
      ? "第一次先傳主旨、寄件者、收件者等基本資訊，並交由AI自動判斷重要性。如果AI認為有必要，將在下一次對話自動匯出內文並再次傳出。"
      : "只傳本批勾選郵件的基本資訊，不讀取正文與任何附件。",
    () => {
      // 確認視窗開啟期間模型仍可能停用，送出前再確認；原生層亦獨立檢查。
      if (allow && !canAutoExportMail()) {
        toast("品質模型目前無法使用，請重新確認分析範圍。");
        return;
      }
      mailCommand({
        action: "analyze",
        ids: selectedIds,
        allow_export: allow,
      });
    },
  );
  if (allow) {
    // 僅以 DOM 與固定文字建立紅色授權段落，避免把郵件或伺服器文字當 HTML。
    const consent = node("strong", "consent-warning", "「你授權本 App 依據 AI 請求，自動匯出本批郵件的完整內容」");
    $("confirm-message").append(document.createTextNode("\n"), consent,
      document.createTextNode("，並交由 AI 再次讀取內文判斷重要性。\n本功能不會自動寄信、刪除郵件。"));
  }
};
$("batch-stop").onclick = () => mailCommand({ action: "stop" });
$("batch-open").onclick = () => {
  showView("chat");
  send({ type: "select_chat", id: state.mail_batch.conversation_id });
};
window.MailUI = {
  render: renderMailBatch,
  enter() {
    renderQualityAvailability("checking");
    mailCommand({ action: "refresh_models" });
  },
};
renderMailBatch();
