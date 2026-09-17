/* 郵件代號僅引用本次 Rust 快照；永遠不向介面提供 Outlook EntryID 或 StoreID。 */
"use strict";
let mailListSignature = "";
const selectedMails = new Set();
function mailCommand(command) {
  send({ type: "mail_batch", command });
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
  $("batch-auto-export").disabled = !!batch.busy;
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
  const allow = $("batch-auto-export").checked;
  ask(
    `分析勾選的 ${selectedMails.size} 封郵件？`,
    allow
      ? "先傳主旨、寄件者、收件者等基本資訊。你授權 App 依 AI 請求自動匯出本批郵件的完整 MSG（正文、圖片及附件），傳給公司網站轉檔，再由品質模型讀取文件並自動整理。不寄信、不修改信箱。"
      : "只傳本批勾選郵件的基本資訊，不讀取正文、不匯出 MSG。",
    () =>
      mailCommand({
        action: "analyze",
        ids: [...selectedMails],
        allow_export: allow,
      }),
  );
};
$("batch-stop").onclick = () => mailCommand({ action: "stop" });
$("batch-open").onclick = () => {
  showView("chat");
  send({ type: "select_chat", id: state.mail_batch.conversation_id });
};
window.MailUI = { render: renderMailBatch };
renderMailBatch();
