/* 只由 EXE --self-check 明確觸發；測試使用虛構資料，不讀帳號或 Outlook。 */
"use strict";
// 先於 app.js 載入，讓啟動錯誤能使自我檢查立即失敗，而不是只得到逾時。
// 正常執行時 Rust 會忽略 self_test_result，不顯示或保存診斷內容。
window.addEventListener("error", (event) => {
  window.chrome?.webview?.postMessage({type: "self_test_result", ok: false,
    detail: `UI script error: ${event.message} (${event.filename}:${event.lineno})`});
});
window.runSelfTest = async (structuredFixture) => {
  const checks = [];
  function check(condition, name) {
    if (!condition) throw new Error(name);
    checks.push(name);
  }
  const frame = () => new Promise((resolve) => setTimeout(resolve, 80));
  // Rust 訊息橋為非同步；等實際狀態回覆，避免固定等兩幀就在慢速主機誤報失敗。
  // 有明確上限且保留原斷言，真正未回覆仍會使自檢失敗。
  async function waitFor(condition, name) {
    const deadline = performance.now() + 5000;
    while (!condition()) {
      if (performance.now() >= deadline) throw new Error(`Timed out: ${name}`);
      await frame();
    }
  }
  try {
    const sample =
      '# Markdown 測試\n\n| 名稱 | 數值 |\n| --- | --- |\n| 快速 | 42 |\n\n- [x] 已完成\n\n行內公式 $E=mc^2$\n\n$$\\int_0^1 x^2\\,dx=\\frac{1}{3}$$\n\n```rust\nfn main() { println!("hello"); }\n```\n\n註腳[^1]\n\n[^1]: 補充說明\n\n<script>alert(1)</script>\n\n![外部圖片](https://example.com/private.png)\n\n[危險連結](javascript:alert(1))';
    const parsed = document.createElement("div");
    parsed.innerHTML = LMUI.renderMarkdown(sample);
    check(!!parsed.querySelector("table"), "table");
    check(!!parsed.querySelector(".katex"), "math");
    check(!!parsed.querySelector(".hljs-keyword"), "code highlight");
    check(!!parsed.querySelector(".footnotes"), "footnotes");
    check(!!parsed.querySelector("input[type=checkbox]"), "task list");
    check(
      !parsed.querySelector("script,img,iframe"),
      "no executable HTML or remote images",
    );
    check(
      ![...parsed.querySelectorAll("a")].some((a) =>
        a.getAttribute("href")?.startsWith("javascript:"),
      ),
      "safe links",
    );
    const fixture = {
      ...LMUI.getState(),
      version: "0.4.0",
      config: {
        font_size: 14,
        hotkey: "Ctrl+Alt+Q",
        sidebar_collapsed: false,
        notification_popups: true,
        model: "fast",
      },
      models: [{ id: "fast", label: "快速" }],
      conversations: [{ id: "test", title: "測試對話", updated_at: 1 }],
      active_id: "test",
      draft_revision: 1,
      draft: "",
      messages: [
        { role: "user", content: "請整理測試資料" },
        {
          role: "assistant",
          content:
            sample +
            "\n\n" +
            Array(45).fill("這是一段用來確認捲動位置的測試內容。").join("\n\n"),
        },
      ],
      busy: "none",
      logged_in: true,
      can_send: true,
      notifications: [],
      version_status: "測試",
      focus_draft: false,
    };
    LMUI.receive(fixture);
    await frame();
    LMUI.bottom();
    check(LMUI.atBottom(), "follow bottom");
    const transcript = document.getElementById("transcript");
    transcript.scrollTop = 0;
    await frame();
    LMUI.setFollow(false);
    const before = transcript.scrollTop;
    fixture.messages.push({ role: "assistant", content: "新增的回覆內容。" });
    LMUI.receive(fixture);
    await frame();
    check(
      Math.abs(transcript.scrollTop - before) < 5,
      `preserve reading position: before=${before}, after=${transcript.scrollTop}, height=${transcript.scrollHeight}, viewport=${transcript.clientHeight}`,
    );
    LMUI.bottom();
    fixture.messages.push({ role: "assistant", content: "繼續回覆。" });
    LMUI.receive(fixture);
    await frame();
    check(LMUI.atBottom(), "follow new reply");
    fixture.config.sidebar_collapsed = true;
    fixture.config.font_size = 12;
    LMUI.receive(fixture);
    check(document.body.classList.contains("collapsed"), "sidebar collapse");
    check(
      getComputedStyle(document.documentElement)
        .getPropertyValue("--font-size")
        .trim() === "12px",
      "font preference",
    );
    check(
      document.querySelector(".message.user .message-meta")?.textContent ===
        "你",
      "user bubble",
    );
    check(
      document.querySelector(".message.assistant .message-meta")
        ?.textContent === "AI",
      "assistant label",
    );
    // 長任務 UI：伺服器規則、檔名安全顯示、Markdown 串流與閱讀位置。
    fixture.work = {
      rules: {
        enabled: true,
        max_count: 20,
        max_file_bytes: 10485760,
        max_total_bytes: 52428800,
        allowed_extensions: [".pdf", ".png"],
      },
      modes: ["stream", "background"],
      mode: "stream",
      status: "附件與執行模式由網站提供",
      attachments: [
        {
          id: "file1",
          name: "<script>file.pdf",
          size: 2048,
          state: "processing",
          progress: 42,
          queue_position: 2,
        },
      ],
      tasks: [
        {
          id: "task1",
          conversation_id: fixture.active_id,
          title: "長任務",
          state: "running",
          mode: "stream",
          active: true,
          partial: "**串流內容**",
          progress: 25,
        },
      ],
    };
    LMUI.receive(fixture);
    await frame();
    check(!document.getElementById("estimate-time"), "time estimate control removed");
    const modeBox = document.querySelector(".execution-switch").getBoundingClientRect();
    const hintBox = document.querySelector(".compose-hint").getBoundingClientRect();
    const statusBox = document.getElementById("work-status").getBoundingClientRect();
    check(hintBox.left >= modeBox.right && hintBox.top < modeBox.bottom,
      "keyboard hints share the mode row");
    check(statusBox.top >= modeBox.bottom,
      "work status stays below the mode and keyboard hints");
    check(
      document.querySelectorAll(".attachment-card").length === 1,
      "attachment processing card",
    );
    check(
      !document.querySelector(".attachment-card script"),
      "attachment filename is text",
    );
    check(
      document.querySelector("#live-task strong")?.textContent === "串流內容",
      "stream Markdown",
    );
    check(
      document.getElementById("attachment-rules").textContent.includes("20"),
      "server attachment rules",
    );
    transcript.scrollTop = 50;
    transcript.dispatchEvent(new Event("scroll"));
    await frame();
    const streamTop = transcript.scrollTop;
    fixture.work.tasks[0].partial += "\n\n新的串流段落。";
    LMUI.receive(fixture);
    await frame();
    check(
      Math.abs(transcript.scrollTop - streamTop) < 5,
      "stream preserves reading position",
    );
    LMUI.bottom();
    fixture.work.tasks[0].partial += "\n\n回覆完成。";
    LMUI.receive(fixture);
    await frame();
    check(LMUI.atBottom(), "stream follows bottom");
    fixture.work.attachments = Array.from({ length: 20 }, (_, i) => ({
      id: "limit" + i,
      name: "existing.png",
      size: 10,
      state: "ready",
    }));
    LMUI.receive(fixture);
    const clipboard = new DataTransfer();
    clipboard.items.add(
      new File([new Uint8Array([137, 80, 78, 71])], "paste.png", {
        type: "image/png",
      }),
    );
    const pasteImage = new ClipboardEvent("paste", {
      clipboardData: clipboard,
      bubbles: true,
      cancelable: true,
    });
    document.getElementById("prompt").dispatchEvent(pasteImage);
    await frame();
    check(pasteImage.defaultPrevented, "image paste is routed to attachments");
    check(
      document.getElementById("toast").textContent.includes("20"),
      "paste image shares the 20-file limit",
    );
    fixture.work.tasks = [];
    fixture.work.attachments = [];
    LMUI.receive(fixture);
    // 在已有文字對話的畫面，走真正的選檔接收流程；只模擬原生 ACK，不上傳使用者檔案。
    // 先前只驗證附件數量超限，未進入 fileBatchBusy，無法抓到已移除按鈕的殘留引用。
    const attachmentSend = send;
    const fileCommands = [];
    const controlLocks = [];
    let rejectFileAction = null;
    const sampleBytes = new Uint8Array(192 * 1024 + 17).map((_, i) => i % 251);
    const sampleFile = new File([sampleBytes], "upload-regression.pdf", {type: "application/pdf"});
    try {
      send = message => {
        if (message.type !== "work") return;
        const command = message.command;
        fileCommands.push(command);
        if (command.action === "file_abort") return;
        controlLocks.push(document.getElementById("new-chat").disabled &&
          document.getElementById("send").disabled &&
          [...document.querySelectorAll(".history-item,[data-history-action]")].every(button => button.disabled));
        queueMicrotask(() => WorkUI.receive(command.action === rejectFileAction
          ? {type: "file_error", message: "模擬附件接收失敗"}
          : {type: "file_ack", id: "test-upload", offset: command.offset || 0,
              finished: command.action === "file_finish"}));
      };
      const selection = new DataTransfer();
      selection.items.add(sampleFile);
      const picker = document.getElementById("attachment-input");
      picker.files = selection.files;
      await picker.onchange({target: picker});
      check(fileCommands.map(command => command.action).join(",") === "file_begin,file_chunk,file_chunk,file_finish",
        "selecting attachment in existing conversation completes native handoff");
      const chunks = fileCommands.filter(command => command.action === "file_chunk");
      const decoded = chunks.map(command => atob(command.data)).join("");
      check(chunks[0].offset === 0 && chunks[1].offset === 192 * 1024 &&
        decoded.length === sampleBytes.length && [...decoded].every((value, i) => value.charCodeAt(0) === sampleBytes[i]),
        "attachment handoff preserves bytes and ordered chunk offsets");
      check(controlLocks.every(Boolean), "file reception locks send and conversation row actions");
      check(!WorkUI.getFileBusy() && !picker.value && !document.getElementById("add-attachment").disabled &&
        !document.getElementById("new-chat").disabled && !document.getElementById("send").disabled &&
        [...document.querySelectorAll(".history-item,[data-history-action]")].every(button => !button.disabled),
        "file reception restores controls without waiting for state push");
      rejectFileAction = "file_chunk";
      fileCommands.length = 0;
      await WorkUI.addFiles([sampleFile]);
      check(fileCommands.map(command => command.action).join(",") === "file_begin,file_chunk,file_abort" &&
        !WorkUI.getFileBusy() && !document.getElementById("add-attachment").disabled &&
        document.getElementById("toast").textContent.includes("模擬附件接收失敗"),
        "failed reception aborts partial file and unlocks picker");
      rejectFileAction = null;
      fileCommands.length = 0;
      await WorkUI.addFiles([sampleFile]);
      check(fileCommands.at(-1).action === "file_finish" && !WorkUI.getFileBusy(),
        "same attachment can be selected again after failure");
    } finally { send = attachmentSend; }
    // 經過真正的 Rust 訊息橋錄製組合；測試不送出系統按鍵，也不改使用者的設定。
    const binding = LMUI.keyBindingFromEvent({ code: "Escape", metaKey: true });
    check(binding.modifiers === 8 && binding.key === 27, "Win+Esc key mapping");
    check(
      LMUI.keyBindingFromEvent({
        code: "Digit8",
        ctrlKey: true,
        shiftKey: true,
      }).key === 56,
      "number key mapping",
    );
    check(
      LMUI.keyBindingFromEvent({ code: "Numpad3", altKey: true }).key === 99,
      "numpad key mapping",
    );
    document.getElementById("settings-button").click();
    document.getElementById("record-hotkey").click();
    await waitFor(() => document.getElementById("hotkey").classList.contains("recording"), "recording start acknowledgement");
    check(
      document.getElementById("hotkey").classList.contains("recording"),
      "recording started through Rust bridge",
    );
    document.getElementById("hotkey").dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Escape",
        code: "Escape",
        metaKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    await waitFor(() => document.getElementById("hotkey").value === "Win+Esc" &&
      !document.getElementById("save-hotkey").disabled, "recorded shortcut acknowledgement");
    check(
      document.getElementById("hotkey").value === "Win+Esc",
      "recorded shortcut canonical name",
    );
    check(
      !document.getElementById("save-hotkey").disabled,
      "recording waits for explicit apply",
    );
    document.getElementById("record-hotkey").click();
    await waitFor(() => document.getElementById("hotkey").classList.contains("recording"), "second recording start acknowledgement");
    document.getElementById("cancel-hotkey").click();
    await waitFor(() => !document.getElementById("hotkey").classList.contains("recording"), "recording cancellation acknowledgement");
    check(
      !document.getElementById("hotkey").classList.contains("recording"),
      "recording cancel restores normal mode",
    );
    document.getElementById("settings-dialog").close();
    fixture.notifications = [
      {
        id: "kanban:123",
        source: "site",
        origin: "kanban",
        title: "<script>通知</script>",
        summary: "網站內容",
        created_at: "2026-09-17T00:00:00Z",
        is_read: false,
        url: "/lm_server/kanban/1",
      },
    ];
    fixture.site_status = "全站同步測試";
    fixture.mail_batch = {
      phase: "idle",
      busy: false,
      list: {
        mails: [
          {
            id: "m1",
            subject: "測試一",
            folder: "本機資料檔/<分類>",
            sender: "甲",
            to: "乙",
            received_at: "今天",
            unread: true,
          },
          {
            id: "m2",
            subject: "測試二",
            sender: "丙",
            to: "丁",
            received_at: "今天",
            unread: false,
          },
        ],
      },
      status: "基本資訊",
    };
    LMUI.receive(fixture);
    await frame();
    check(
      document.querySelectorAll("[data-mail-period]").length === 6,
      "six Outlook date/unread presets",
    );
    check(
      document.getElementById("batch-scope").value === "current_folder",
      "Outlook defaults to current folder and subfolders",
    );
    // 攔截前端命令，驗證兩個範圍的六種按鈕都帶對參數，不觸發真實 Outlook。
    const originalMailSend = send;
    const mailCommands = [];
    try {
      send = (command) => mailCommands.push(command);
      LMUI.showView("chat");
      mailCommands.length = 0;
      LMUI.showView("outlook");
      LMUI.showView("outlook");
      check(mailCommands.length === 1 && mailCommands[0].command.action === "refresh_models",
        "entering Outlook refreshes model catalog exactly once");
      check(document.getElementById("batch-auto-export").disabled,
        "automatic mail export disabled while refreshing models");
      fixture.mail_batch.quality_status = "unavailable";
      LMUI.receive(fixture);
      check(document.getElementById("batch-auto-export").disabled && !document.getElementById("batch-auto-export").checked &&
        document.getElementById("batch-quality-status").textContent.includes("品質模型維護中"), "missing quality model disables automatic export with maintenance notice");
      document.getElementById("batch-analyze").click();
      check(document.getElementById("confirm-message").textContent.includes("不讀取正文與任何附件"), "metadata analysis remains available without quality model");
      document.getElementById("confirm-ok").click();
      check(mailCommands.at(-1).command.allow_export === false, "metadata-only analysis never authorizes export");
      fixture.models.push({id: "quality", label: "品質"});
      fixture.mail_batch.quality_status = "available";
      LMUI.receive(fixture);
      check(!document.getElementById("batch-auto-export").disabled && !document.getElementById("batch-auto-export").checked,
        "quality recovery enables option without silently granting consent");
      document.getElementById("batch-auto-export").checked = true;
      document.getElementById("batch-analyze").click();
      check(document.querySelector("#confirm-message .consent-warning")?.textContent.includes("自動匯出本批郵件的完整內容") &&
        document.getElementById("confirm-message").textContent.split("\n").length === 3,
        "export consent has separate lines and highlighted authorization");
      // 在確認視窗尚未關閉時撤回模型，舊的同意不得繼續啟動匯出。
      fixture.models = fixture.models.filter(model => model.id !== "quality");
      fixture.mail_batch.quality_status = "unavailable";
      LMUI.receive(fixture);
      const countBeforeConfirm = mailCommands.length;
      document.getElementById("confirm-ok").click();
      check(mailCommands.length === countBeforeConfirm, "quality removed during confirmation prevents export command");
      fixture.models.push({id: "quality", label: "品質"});
      fixture.mail_batch.quality_status = "available";
      LMUI.receive(fixture);
      document.getElementById("batch-auto-export").checked = true;
      document.getElementById("batch-analyze").click();
      document.getElementById("confirm-ok").click();
      check(mailCommands.at(-1).command.allow_export === true, "explicit consent with available quality model authorizes export");
      fixture.mail_batch.quality_status = "error";
      LMUI.receive(fixture);
      check(document.getElementById("batch-auto-export").disabled && document.getElementById("batch-quality-status").textContent.includes("無法確認"),
        "catalog failure disables export without misreporting maintenance");
      fixture.mail_batch.quality_status = "available";
      LMUI.receive(fixture);
      mailCommands.length = 0;
      for (const scope of ["current_folder", "inbox"]) {
        document.getElementById("batch-scope").value = scope;
        for (const button of document.querySelectorAll("[data-mail-period]")) {
          button.click();
          const command = mailCommands[mailCommands.length - 1].command;
          check(
            command.scope === scope &&
              command.period === button.dataset.mailPeriod &&
              command.unread === (button.dataset.unread === "true"),
            "Outlook date command includes scope and unread filter",
          );
        }
      }
      check(mailCommands.length === 12, "all scope and date combinations dispatched");
    } finally {
      send = originalMailSend;
      document.getElementById("batch-scope").value = "current_folder";
    }
    check(
      document.getElementById("batch-mail-list").textContent.includes("本機資料檔/<分類>") &&
        !document.querySelector("#batch-mail-list 分類"),
      "Outlook source folder displayed as text",
    );
    check(
      document.querySelectorAll("#batch-mail-list input:checked").length === 2,
      "batch mail selection",
    );
    document.getElementById("batch-select-all").click();
    check(
      document.querySelectorAll("#batch-mail-list input:checked").length === 0,
      "batch deselect all",
    );
    check(
      !document.querySelector("#notification-list script"),
      "site notice escaped",
    );
    check(
      document
        .getElementById("notification-list")
        .textContent.includes("開啟通知"),
      "site mark-read then open action",
    );
    document.getElementById("settings-button").click();
    const settings = document.getElementById("settings-dialog"),
      box = settings.getBoundingClientRect();
    // 在真正 WebView2 排版後核對邊界，涵蓋大字體及較窄設定面板。
    const settingsContent = settings.querySelector(".settings-content");
    const originalFont = fixture.config.font_size;
    for (const font of [12, 14, 20]) {
      fixture.config.font_size = font;
      LMUI.receive(fixture);
      for (const width of [320, 520]) {
        settings.style.width = width + "px";
        await frame();
        check(settingsContent.scrollWidth <= settingsContent.clientWidth + 1,
          `settings has no horizontal overflow at ${width}px / ${font}px`);
        for (const row of settings.querySelectorAll(".option-row")) {
          const circle = row.querySelector("input").getBoundingClientRect();
          const text = row.querySelector("span").getBoundingClientRect();
          check(circle.right <= text.left + 0.5 && circle.top < text.top + parseFloat(getComputedStyle(row).lineHeight),
            `radio circle is beside the first text line: ${row.querySelector("input").id}, ${width}px / ${font}px`);
          check(row.scrollWidth <= row.clientWidth + 1,
            "radio label is not horizontally clipped");
        }
        for (const row of settings.querySelectorAll(".input-row,.button-row,fieldset")) {
          check(row.scrollWidth <= row.clientWidth + 1, "settings controls fit their row");
        }
      }
    }
    settings.style.width = "";
    fixture.config.font_size = originalFont;
    LMUI.receive(fixture);
    settings.dispatchEvent(
      new PointerEvent("pointerdown", {
        clientX: box.left - 2,
        clientY: box.top - 2,
        bubbles: true,
      }),
    );
    settings.dispatchEvent(
      new MouseEvent("click", {
        clientX: box.left - 2,
        clientY: box.top - 2,
        bubbles: true,
      }),
    );
    check(!settings.open, "settings backdrop dismiss");
    for (const answer of [
      "1. Answer\n主要回答。\n\n2. Key points\n重點。\n\n3. Sources\n來源內容。\n\n4. Confidence\nHigh\n\n5. Limitations\n限制內容。",
      "## 1. Answer\n主要回答。\n\n## 2. Key points\n重點。\n\n## 3. Sources\n來源內容。\n\n## 4. Confidence\nHigh\n\n## 5. Limitations\n限制內容。",
      "**1. Answer**\n\n主要回答。\n\n**2. Key points**\n\n重點。\n\n**3. Sources**\n\n來源內容。\n\n**4. Confidence**\n\nHigh\n\n**5. Limitations**\n\n限制內容。",
      "## 1. 回答\n主要回答。\n\n## 2. 回答重點\n重點。\n\n## 3. 來源\n來源內容。\n\n## 4. 信心度\nHigh\n\n## 5. 限制\n限制內容。",
      "## (1) ANSWER:\n主要回答。\n\n## (2) KEY   POINTS：\n重點。\n\n## (3) SOURCES:\n來源內容。\n\n## (4) CONFIDENCE:\nHigh\n\n## (5) LIMITATIONS:\n限制內容。",
      // 快速模型可能先給來源再補重點，或在來源後再次補上正文／重點。
      "## 1. Answer\n主要回答。\n\n## 2. Sources\n來源內容。\n\n## 3. Key points\n重點。\n\n## 4. Confidence\nHigh\n\n## 5. Limitations\n限制內容。",
      "## 1. Answer\n主要回答。\n\n## 2. Key points\n初步摘要。\n\n## 3. Sources\n來源內容。\n\n## 4. Key points\n重點。\n\n## 5. Answer\n補充正文。\n\n## 6. Confidence\nHigh\n\n## 7. Limitations\n限制內容。",
      "1. Answer\n主要回答。\n\n2. Key points\n初步摘要。\n\n3. Sources\n來源內容。\n\n4. Key points\n重點。\n\n5. Confidence\nHigh\n\n6. Limitations\n限制內容。",
      "**1. content：主要回答。**\n\n**2. keypoints：重點。**\n\n**3. references：來源內容。**\n\n**4. confidence：High**\n\n**5. limitation：限制內容。**",
    ]) {
      const reply = document.createElement("div");
      reply.innerHTML = LMUI.renderAssistantReply(answer);
      const details = reply.querySelector("details.answer-details");
      check(details && !details.open && ["來源內容", "High", "限制內容"].every((text) => details.textContent.includes(text)), "structured answer secondary sections collapsed");
      check(!details.textContent.includes("重點。") && !details.textContent.includes("補充正文"), "primary sections never enter collapsed details");
      details.open = true;
      check(details.open && details.querySelector(".answer-details-content"), "secondary sections can be expanded");
      details.remove();
      check(reply.textContent.includes("主要回答") && reply.textContent.includes("重點") && !reply.textContent.includes("來源內容"), "Answer and Key points stay visible");
    }
    // 逐一涵蓋網頁端提供的全部舊格式標籤，且冒號後同一行的內容也要保留。
    const legacyLabels = [
      ["answer", "content", "回答", "內容"],
      ["keypoint", "key points", "keypoints", "回答重點"],
      ["source", "sources", "references", "引用資料庫", "內容引用處", "來源", "來源摘要"],
      ["confidence", "信心度"],
      ["limitation", "limitations", "限制", "回答限制"],
    ];
    const legacyBodies = ["主要回答。", "重點。", "來源內容。", "High", "限制內容。"];
    for (const [sectionIndex, labels] of legacyLabels.entries()) {
      for (const label of labels) {
        const text = legacyLabels.map((names, index) =>
          `## ${index + 1}. ${index === sectionIndex ? label : names[0]}：${legacyBodies[index]}`
        ).join("\n\n");
        const reply = document.createElement("div");
        reply.innerHTML = LMUI.renderAssistantReply(text);
        const details = reply.querySelector("details");
        check(details && ["來源內容。", "High", "限制內容。"].every((body) => details.textContent.includes(body)), `legacy alias is recognized: ${label}`);
        details.remove();
        check(reply.textContent.includes("主要回答。") && reply.textContent.includes("重點。"), `legacy alias preserves primary text: ${label}`);
      }
    }
    check(!LMUI.renderAssistantReply("```text\n1. Answer\n2. Key points\n3. Sources\n```\n\n一般回覆。").includes("answer-details"), "code and ordinary replies are not folded");
    for (const ordinary of [
      "## Sources\n一般文章的來源章節。",
      "> Answer\n>\n> Key points\n>\n> Sources",
      "1. 一般清單\n   1. Answer\n   2. Key points\n   3. Sources",
      "**Answer**\n正文。\n\n**Key points**\n重點。",
      "## Answer\n正文。\n\n## Key points\n重點。\n\n## Sources\n來源。",
      "**Answer**\n正文。\n\n**Key points**\n重點。\n\n**Sources**\n來源。",
      "1. 回答包含 answer 關鍵字。\n\n2. 回答重點只是正文的一部分。\n\n3. sources 並非獨立標題。",
    ]) {
      check(LMUI.renderAssistantReply(ordinary) === LMUI.renderMarkdown(ordinary), "unrecognized structure retains original Markdown");
    }
    const structuredReply = "## 1. Answer\n主要回答。\n\n## 2. Key points\n初步摘要。\n\n## 3. Sources\n來源內容。\n\n### 資料庫\n來源的子章節。\n\n## 4. 回答重點\n重點。\n\n## 5. Confidence\nHigh\n\n## 6. Limitations\n限制內容。\n\n## 後續建議\n補充正文。";
    const boundaries = document.createElement("div");
    boundaries.innerHTML = LMUI.renderAssistantReply(structuredReply);
    const secondary = boundaries.querySelector("details");
    check(secondary.textContent.includes("來源的子章節") && !secondary.textContent.includes("補充正文"), "only subordinate headings stay in secondary sections");
    secondary.remove();
    check(boundaries.textContent.includes("重點。") && boundaries.textContent.includes("補充正文"), "later primary and unknown sections stay visible");

    // 經過正式的串流與完成訊息渲染入口，防止只測獨立函式卻漏掉完成後的重繪。
    // 前一組測試停在 Outlook；先切回對話，才能驗證展開內容實際可見。
    LMUI.showView("chat");
    const replyFixture = {
      ...fixture,
      active_id: "reply-sections",
      conversations: [{ id: "reply-sections", title: "章節回歸測試", updated_at: 1 }],
      messages: [],
      work: { ...fixture.work, attachments: [], tasks: [{
        id: "reply-task", conversation_id: "reply-sections", state: "running",
        active: true, partial: structuredReply,
      }] },
    };
    LMUI.receive(replyFixture);
    const liveReply = document.querySelector("#live-task .bubble");
    check(liveReply && !liveReply.querySelector("details").textContent.includes("重點。"), "stream keeps key points outside details");
    replyFixture.work.tasks = [];
    replyFixture.messages = [{ role: "assistant", content: structuredReply }];
    LMUI.receive(replyFixture);
    const savedReply = document.querySelector("#messages .bubble");
    check(savedReply.innerHTML === liveReply.innerHTML, "completed response preserves all streamed sections");
    const savedDetails = savedReply.querySelector("details");
    savedDetails.querySelector("summary").click();
    check(savedDetails.open, "completed response details expand by click");
    replyFixture.messages.push({ role: "user", content: "下一題" });
    LMUI.receive(replyFixture);
    check(document.querySelector("#messages details").open, "expanded sections survive message redraw");
    check(LMUI.getState().messages[0].content === structuredReply, "display folding preserves complete original message");

    // 與 Rust 協定測試共用使用者提供的 JSON 範例，核對結構化欄位優先及完成切換。
    check(structuredFixture?.payload && structuredFixture.message?.response_payload,
      "structured reply fixture is parsed by Rust and delivered through the bridge");
    const payload = structuredFixture.payload;
    const structured = document.createElement("div");
    structured.innerHTML = LMUI.renderAssistantReply("舊正文不應顯示", payload);
    const payloadDetails = structured.querySelector("details");
    check(payloadDetails && !payloadDetails.open && payloadDetails.textContent.includes("High (高)") &&
      !structured.textContent.includes("medium") && !structured.textContent.includes("舊正文不應顯示"),
      "structured sections override legacy content and top-level confidence");
    check(payloadDetails.textContent.includes(payload.sections.sources[0]) &&
      payloadDetails.textContent.includes(payload.sections.limitations[0]), "structured sources and limitations are retained");
    payloadDetails.remove();
    check(payload.sections.key_points.every((point) => structured.textContent.includes(point)) &&
      structured.querySelector("code")?.textContent === "robocopy", "structured answer and both key points remain visible");
    const emptyPayload = { answer: "只有正文", sections: {}, citations: [] };
    check(!LMUI.renderAssistantReply("", emptyPayload).includes("answer-details"), "empty sections produce no empty disclosure");
    const hostilePayload = {
      answer: "## 1. Sources\n這段仍屬於正文。", sections: { key_points: ["<script>bad()</script>"], confidence: "High" },
      citations: ["[不安全連結](javascript:alert(1))", { title: "<img src=x onerror=bad()>", page: 2 }],
    };
    const safeReply = document.createElement("div");
    safeReply.innerHTML = LMUI.renderAssistantReply("", hostilePayload);
    check(safeReply.querySelector(".answer-body").textContent.includes("這段仍屬於正文") &&
      !safeReply.querySelector("script,img,iframe") &&
      ![...safeReply.querySelectorAll("a")].some((a) => a.getAttribute("href")?.startsWith("javascript:")),
      "structured fields retain boundaries and sanitize Markdown and citation text");
    check(safeReply.querySelector("details").textContent.includes("引用文件") &&
      safeReply.querySelector(".citation-data").textContent.includes("page"), "citation objects are retained as inert text");

    const completeText = structuredFixture.message.content;
    check([...payload.sections.key_points, ...payload.sections.sources, payload.sections.confidence,
      ...payload.sections.limitations].every((value) => completeText.includes(value)),
      "Rust prepares complete copy text including every structured section");
    // 舊 answer payload 與新版 choices + 同層 sections 共用實際完成渲染／複製驗證。
    for (const completedMessage of [structuredFixture.message, structuredFixture.completion_message]) {
      const payload = completedMessage.response_payload;
      const completeText = completedMessage.content;
      check(payload && [...payload.sections.key_points, ...payload.sections.sources,
        payload.sections.confidence, ...payload.sections.limitations].every((text) => completeText.includes(text)),
        "both wire formats retain every field in copy text");
      for (const mode of ["background", "stream"]) {
        replyFixture.messages = [];
        replyFixture.work.tasks = [{ id: "payload-task", conversation_id: replyFixture.active_id,
          state: "running", active: true, mode, partial: mode === "stream" ? payload.answer : "" }];
        LMUI.receive(replyFixture);
        replyFixture.work.tasks = [];
        replyFixture.messages = [JSON.parse(JSON.stringify(completedMessage))];
        LMUI.receive(replyFixture);
        const completed = document.querySelector("#messages .bubble");
        check(completed.querySelector(".answer-body") && completed.querySelector("details"), `${mode} completion uses structured result`);
        const visible = completed.cloneNode(true);
        visible.querySelector("details").remove();
        check(payload.sections.key_points.every((point) => visible.textContent.includes(point)), `${mode} completion keeps every key point visible`);
        const details = completed.querySelector("details");
        check([...payload.sections.sources, payload.sections.confidence, ...payload.sections.limitations]
          .every((text) => details.textContent.includes(text)), `${mode} retains all metadata in disclosure`);
        details.querySelector("summary").click();
        check(details.open && details.querySelector(".answer-details-content").getBoundingClientRect().height > 0,
          `${mode} completed metadata opens visibly`);
        const originalCopySend = send;
        const copyCommands = [];
        try {
          send = (command) => copyCommands.push(command);
          document.querySelector("#messages .copy-message").click();
          check(copyCommands.length === 1 && copyCommands[0].type === "copy" && copyCommands[0].text === completeText,
            `${mode} copy includes the complete response`);
        } finally { send = originalCopySend; }
      }
    }
    // 使用回報中的單行全文，跨越串流、REST 完成與歷史重繪，確認資料不是被移除。
    const inlineText = structuredFixture.inline_text;
    const inlineReply = structuredFixture.inline_message;
    const compact = document.createElement("div");
    compact.innerHTML = LMUI.renderAssistantReply(inlineText);
    check(compact.querySelector("details")?.textContent.includes("100%"), "single-line numbered English headings are recognized");
    compact.querySelector("details").remove();
    check(compact.textContent.includes("系統回應正常") && compact.textContent.includes("已確認可接收輸入"),
      "single-line key points stay outside the disclosure");
    for (const finalMessage of [inlineReply, structuredFixture.changed_message]) {
      replyFixture.messages = [];
      replyFixture.work.tasks = [{ id: "inline-task", conversation_id: replyFixture.active_id,
        state: "running", active: true, mode: "stream", partial: inlineText }];
      LMUI.receive(replyFixture);
      check(document.querySelector("#live-task details")?.textContent.includes("100%"), "inline stream retains confidence");
      replyFixture.work.tasks = [];
      // 序列化再還原，模擬歷史資料經訊息橋重新載入。
      replyFixture.messages = JSON.parse(JSON.stringify([finalMessage]));
      LMUI.receive(replyFixture);
      const disclosure = document.querySelector("#messages details");
      check(disclosure && !disclosure.open && disclosure.textContent.includes("100%") &&
        disclosure.textContent.includes("未涉及任何實際的文件分析"), "body-only completion retains streamed metadata");
      disclosure.querySelector("summary").click();
      check(disclosure.open && disclosure.querySelector(".answer-details-content").getBoundingClientRect().height > 0,
        "retained metadata opens into a visible block");
      const originalCopySend = send;
      const commands = [];
      try {
        send = (command) => commands.push(command);
        document.querySelector("#messages .copy-message").click();
        check(commands[0]?.text.includes("系統回應正常") && commands[0]?.text.includes("100%") &&
          commands[0]?.text.includes("未涉及任何實際的文件分析"), "copy retains all inline reply sections");
      } finally { send = originalCopySend; }
      replyFixture.messages.push({ role: "user", content: "下一題" });
      LMUI.receive(replyFixture);
      check(document.querySelector("#messages details").open, "retained original stays expanded after redraw");
    }
    LMUI.receive(fixture);
    check(document.querySelector(".attachment-help").hidden, "attachment help hidden while validation remains active");
    check(document.querySelector(".topbar").getBoundingClientRect().height <= 50, "compact app header");

    const originalSend = send;
    const navigationCommands = [];
    try {
      send = (command) => navigationCommands.push(command);
      LMUI.showView("chat");
      navigationCommands.length = 0;
      LMUI.showView("tasks");
      LMUI.showView("tasks");
      check(navigationCommands.length === 0, "entering or redrawing tasks does not clear");
      LMUI.showView("notifications");
      LMUI.showView("notifications");
      check(navigationCommands.length === 1 && navigationCommands[0].command.action === "clear_completed", "leaving tasks clears completed cards once");
      LMUI.showView("chat");
      check(navigationCommands.length === 2 && navigationCommands[1].type === "notifications_left", "leaving notifications requests mark all read once");
      document.getElementById("dark-mode").checked = true;
      document.getElementById("dark-mode").dispatchEvent(new Event("change"));
      check(navigationCommands[2].dark_mode === true, "dark preference sent for persistence");
    } finally { send = originalSend; }
    fixture.config.dark_mode = true;
    LMUI.receive(fixture);
    check(getComputedStyle(document.body).backgroundColor === "rgb(29, 37, 48)", "dark mode uses grey blue instead of black");
    // 前面的附件測試已清空任務，這裡建立獨立的工具串流案例。
    fixture.work.tasks = [{ id: "tool-task", conversation_id: fixture.active_id,
      title: "工具測試", mode: "stream", state: "running", active: true, partial: "" }];
    fixture.work.tasks[0].tool_status = { tool_name: "search_session_documents", status: "started" };
    LMUI.receive(fixture);
    check(document.querySelector("#live-task .tool-status")?.textContent === "search_session_documents · started", "tool status visible before first answer delta");
    fixture.work.tasks[0].tool_status.status = "completed";
    LMUI.receive(fixture);
    check(document.querySelector("#live-task .tool-status")?.textContent.endsWith("completed"), "tool completion redraws without text delta");
    fixture.messages.push({role: "assistant", content: "這是斷線前已收到的內容", incomplete: true});
    LMUI.receive(fixture);
    check(document.querySelector(".incomplete-warning")?.textContent.includes("回覆中斷") && document.getElementById("messages").textContent.includes("這是斷線前已收到的內容"), "interrupted answer and warning remain together");
    fixture.config.dark_mode = false;
    LMUI.receive(fixture);
    // 使用者偏好：真正呼叫與日常使用相同的 DOM 事件處理器，不發出網路請求。
    const behaviorSend = send;
    const behaviorCommands = [];
    try {
      send = command => behaviorCommands.push(command);
      fixture.work.tasks = [];
      fixture.work.pending = false;
      fixture.can_send = true;
      fixture.config.enter_sends = true;
      fixture.config.hotkey_enabled = false;
      fixture.config.quick_actions_fast = false;
      fixture.config.sidebar_collapsed = false;
      fixture.conversations = [
        {id:"recent",title:"近期",updated_at:100,pinned:false},
        {id:"pinned",title:"重要",updated_at:1,pinned:true},
      ];
      LMUI.receive(fixture);
      check(document.querySelector("#history-list button")?.dataset.id === "pinned", "pinned conversation precedes newer ordinary conversation");
      check(document.querySelectorAll(".topbar-actions button").length === 2 &&
        document.getElementById("show-notifications").closest(".topbar-actions") &&
        document.getElementById("show-tasks").closest(".topbar-actions") &&
        document.getElementById("new-chat").closest(".section-caption"), "compact navigation places tools and new chat correctly");
      const logo = document.querySelector("img.brand-mark");
      check(logo.complete && logo.naturalWidth > 0 && logo.getAttribute("src") === "app.ico", "brand loads embedded application cat icon");
      const rowAction = action => document.querySelector(`[data-history-action="${action}"][data-id="recent"]`);
      rowAction("rename").focus();
      check(getComputedStyle(rowAction("rename").parentElement).opacity === "1", "keyboard focus reveals conversation actions");
      rowAction("rename").click();
      check(document.getElementById("rename-title").value === "近期", "rename opens clicked row rather than active conversation");
      document.getElementById("rename-title").value = "重新命名";
      document.getElementById("rename-save").click();
      check(behaviorCommands.at(-1).type === "rename_chat" && behaviorCommands.at(-1).id === "recent", "rename targets row id");
      rowAction("pin").click();
      check(behaviorCommands.at(-1).type === "pin_chat" && behaviorCommands.at(-1).id === "recent" && behaviorCommands.at(-1).pinned, "pin targets row id");
      rowAction("delete").click();
      document.getElementById("confirm-ok").click();
      check(behaviorCommands.at(-1).type === "delete_chat" && behaviorCommands.at(-1).id === "recent" &&
        !behaviorCommands.some(c => c.type === "select_chat"), "row actions never switch conversations and delete targets confirmed row");
      fixture.config.sidebar_collapsed = true;
      LMUI.receive(fixture);
      check(document.getElementById("new-chat").getBoundingClientRect().width > 0, "collapsed sidebar retains new chat access");
      fixture.config.sidebar_collapsed = false;
      LMUI.receive(fixture);
      check(document.getElementById("hotkey-hint").hidden, "disabled hotkey hint hidden");
      check(document.getElementById("compose-enter-hint").textContent === "Shift+Enter 換行，Enter 送出" &&
        document.getElementById("enter-hint").textContent === document.getElementById("compose-enter-hint").textContent &&
        document.getElementById("send").title === "送出（Enter）", "enter send hints follow applied preference");
      const prompt = document.getElementById("prompt");
      prompt.value = "中文測試";
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", isComposing:true, bubbles:true, cancelable:true}));
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", shiftKey:true, bubbles:true, cancelable:true}));
      check(!behaviorCommands.some(c=>c.type === "chat"), "IME confirmation and Shift Enter never send");
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", bubbles:true, cancelable:true}));
      check(behaviorCommands.filter(c=>c.type === "chat").length === 1, "plain Enter sends when enabled");
      fixture.config.enter_sends = false;
      fixture.config.hotkey_enabled = true;
      fixture.config.hotkey = "Ctrl+Shift+F9";
      LMUI.receive(fixture);
      check(document.getElementById("compose-enter-hint").textContent === "Enter 換行，Ctrl+Enter 送出" &&
        document.getElementById("send").title === "送出（Ctrl+Enter）", "newline hints follow applied preference");
      check(!document.getElementById("hotkey-hint").hidden &&
        document.getElementById("hotkey-hint").textContent === "Ctrl+Shift+F9 選字帶入", "custom capture hotkey hint follows applied preference");
      prompt.value = "換行模式";
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", bubbles:true, cancelable:true}));
      check(behaviorCommands.filter(c=>c.type === "chat").length === 1, "plain Enter preserves newline mode");
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter",ctrlKey:true,bubbles:true,cancelable:true}));
      check(behaviorCommands.filter(c=>c.type === "chat").length === 2, "Ctrl Enter sends in newline mode");
      check(!document.getElementById("execution-sync"), "old synchronous reply control removed");
      check(document.getElementById("execution-stream").textContent === "一般", "stream label is general");
      BehaviorUI.modelNotice("本次已自動切換為快速模型");
      check(!document.querySelector(".model-notice").hidden, "automatic model notice shown");
      await new Promise(resolve => setTimeout(resolve,1800));
      check(document.querySelector(".model-notice").hidden, "automatic model notice fades after 1.5 seconds");
    } finally { send = behaviorSend; }
    // VNC 只測試本機頁面命令；攔截傳送，不讀取真實機台檔、不啟動 Viewer。
    const vncSend = send;
    const vncCommands = [];
    try {
      send = value => vncCommands.push(value);
      fixture.config.vnc_enabled = false;
      LMUI.receive(fixture);
      check(document.getElementById("show-vnc").hidden, "VNC entry hidden by default");
      document.getElementById("show-vnc").click();
      check(vncCommands.length === 0, "disabled VNC does not load files");
      document.getElementById("vnc-enabled").checked = true;
      document.getElementById("vnc-enabled").dispatchEvent(new Event("change"));
      check(vncCommands.at(-1).type === "behavior" && vncCommands.at(-1).vnc_enabled, "VNC opt-in sent for persistence");
      fixture.config.vnc_enabled = true;
      fixture.vnc = { loaded: true, revision: 3, searching: false, status: "測試資料",
        viewer_path: "C:\\UltraVNC\\vncviewer.exe", machines_path: "C:\\LM_AI\\machines.json",
        options: { fullscreen: false, viewonly: false, autoscaling: true },
        groups: [{ name: "測試分類", machines: [
          { index: 0, name: "機台10<script>", ip: "192.0.2.10", has_password: true },
          { index: 1, name: "備用機", ip: "192.0.2.2", has_password: false },
        ] }] };
      LMUI.receive(fixture);
      check(!document.getElementById("show-vnc").hidden, "VNC entry visible after opt-in");
      document.getElementById("show-vnc").click();
      check(!document.getElementById("vnc-view").hidden && document.getElementById("chat-view").hidden, "VNC has independent page");
      const machineButtons = document.querySelectorAll(".vnc-machine");
      check(machineButtons.length === 2 && machineButtons[0].textContent.includes("機台10<script>") && !document.querySelector("#vnc-machines script"), "VNC keeps file order and treats names as text");
      machineButtons[0].click();
      const connection = vncCommands.at(-1).command;
      check(connection.action === "connect" && connection.group === "測試分類" && connection.index === 0 && connection.revision === 3 && !("password" in connection) && !("ip" in connection), "VNC connection refers only to native machine snapshot");
      document.getElementById("vnc-viewonly").checked = true;
      document.getElementById("vnc-viewonly").dispatchEvent(new Event("change"));
      check(vncCommands.at(-1).command.viewonly && vncCommands.at(-1).command.autoscaling, "VNC options sent independently of chat");
      document.getElementById("vnc-manage").click();
      const rowButtons = document.querySelectorAll("#vnc-manager-rows tr:first-child button");
      check(rowButtons[1].disabled && !rowButtons[2].disabled, "VNC manual move respects group boundaries");
      rowButtons[2].click();
      check(vncCommands.at(-1).command.action === "move_machine" && vncCommands.at(-1).command.direction === "down", "VNC manual order command");
      rowButtons[0].click();
      check(document.getElementById("vnc-password").value === "", "existing VNC password never filled into editor");
      document.getElementById("vnc-name").value = "改名保留位置";
      document.getElementById("vnc-machine-form").dispatchEvent(new Event("submit", { cancelable: true }));
      const edit = vncCommands.at(-1).command;
      check(edit.action === "save_machine" && edit.original.index === 0 && edit.password === null, "VNC edit retains original slot and password");
      document.getElementById("vnc-clear-password").checked = true;
      document.getElementById("vnc-machine-form").dispatchEvent(new Event("submit", { cancelable: true }));
      check(vncCommands.at(-1).command.password === "", "VNC password clear is explicit");
      fixture.config.vnc_enabled = false;
      LMUI.receive(fixture);
      check(document.getElementById("show-vnc").hidden && document.getElementById("vnc-view").hidden && !document.getElementById("vnc-manager-dialog").open, "VNC disabling closes and hides tools");
      check(!document.getElementById("vnc-manager-rows").children.length, "VNC disabling clears machine editor");
    } finally { send = vncSend; }
    fixture.update_required = true;
    fixture.update_busy = true;
    LMUI.receive(fixture);
    check(document.getElementById("required-update-dialog").open, "mandatory update blocks UI");
    check(document.getElementById("required-update-download").disabled, "duplicate download is disabled");
    const escapeUpdate = new Event("cancel", {cancelable:true});
    document.getElementById("required-update-dialog").dispatchEvent(escapeUpdate);
    check(escapeUpdate.defaultPrevented, "Escape cannot dismiss mandatory update");
    fixture.update_busy = false;
    fixture.update_ready = true;
    LMUI.receive(fixture);
    check(document.getElementById("required-update-download").textContent === "安裝並重新啟動", "downloaded update awaits second action");
    fixture.update_kind = "exe";
    LMUI.receive(fixture);
    check(document.getElementById("required-update-download").textContent === "開啟下載資料夾", "portable update offers manual replacement");
    check(document.getElementById("required-update-dialog").open, "portable download does not unlock mandatory update");
    fixture.update_kind = "nsis";
    fixture.update_required = false;
    LMUI.receive(fixture);
    check(!document.getElementById("required-update-dialog").open, "supported version has no update gate");
    window.chrome?.webview?.postMessage({
      type: "self_test_result",
      ok: true,
      detail: checks.join(", "),
    });
    window.selfTestResult = { ok: true, checks };
  } catch (error) {
    window.selfTestResult = { ok: false, detail: String(error) };
    window.chrome?.webview?.postMessage({
      type: "self_test_result",
      ok: false,
      detail: String(error),
    });
  }
};
