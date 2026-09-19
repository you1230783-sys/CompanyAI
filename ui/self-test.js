/* 只由 EXE --self-check 明確觸發；測試使用虛構資料，不讀帳號或 Outlook。 */
"use strict";
// 先於 app.js 載入，讓啟動錯誤能使自我檢查立即失敗，而不是只得到逾時。
// 正常執行時 Rust 會忽略 self_test_result，不顯示或保存診斷內容。
window.addEventListener("error", (event) => {
  window.chrome?.webview?.postMessage({type: "self_test_result", ok: false,
    detail: `UI script error: ${event.message} (${event.filename}:${event.lineno})`});
});
window.runSelfTest = async () => {
  const checks = [];
  function check(condition, name) {
    if (!condition) throw new Error(name);
    checks.push(name);
  }
  const frame = () => new Promise((resolve) => setTimeout(resolve, 80));
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
    const statusBox = document.getElementById("work-status").getBoundingClientRect();
    check(statusBox.left >= modeBox.right && statusBox.top < modeBox.bottom,
      "work status shares the mode row");
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
    await frame();
    await frame();
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
    await frame();
    await frame();
    check(
      document.getElementById("hotkey").value === "Win+Esc",
      "recorded shortcut canonical name",
    );
    check(
      !document.getElementById("save-hotkey").disabled,
      "recording waits for explicit apply",
    );
    document.getElementById("record-hotkey").click();
    await frame();
    await frame();
    document.getElementById("cancel-hotkey").click();
    await frame();
    await frame();
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
      document.getElementById("batch-scope").value === "all_stores",
      "Outlook defaults to loaded stores and subfolders",
    );
    // 攔截前端命令，驗證三個範圍的六種按鈕都帶對參數，不觸發真實 Outlook。
    const originalMailSend = send;
    const mailCommands = [];
    try {
      send = (command) => mailCommands.push(command);
      for (const scope of ["all_stores", "current_folder", "inbox"]) {
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
      check(mailCommands.length === 18, "all scope and date combinations dispatched");
    } finally {
      send = originalMailSend;
      document.getElementById("batch-scope").value = "all_stores";
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
      "**Answer**\n\n主要回答。\n\n**Key points**\n\n重點。\n\n**Sources**\n\n來源內容。\n\n**Confidence**\n\nHigh\n\n**Limitations**\n\n限制內容。",
    ]) {
      const reply = document.createElement("div");
      reply.innerHTML = LMUI.renderAssistantReply(answer);
      const details = reply.querySelector("details.answer-details");
      check(details && !details.open && details.textContent.includes("限制內容"), "structured answer secondary sections collapsed");
      details.remove();
      check(reply.textContent.includes("主要回答") && reply.textContent.includes("重點") && !reply.textContent.includes("來源內容"), "Answer and Key points stay visible");
    }
    check(!LMUI.renderAssistantReply("```text\n1. Answer\n2. Key points\n3. Sources\n```\n\n一般回覆。").includes("answer-details"), "code and ordinary replies are not folded");
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
      fixture.conversations = [
        {id:"recent",title:"近期",updated_at:100,pinned:false},
        {id:"pinned",title:"重要",updated_at:1,pinned:true},
      ];
      LMUI.receive(fixture);
      check(document.querySelector("#history-list button")?.dataset.id === "pinned", "pinned conversation precedes newer ordinary conversation");
      check(document.getElementById("hotkey-hint").hidden, "disabled hotkey hint hidden");
      check(document.getElementById("enter-hint").textContent.includes("Shift + Enter"), "enter send mode hint");
      const prompt = document.getElementById("prompt");
      prompt.value = "中文測試";
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", isComposing:true, bubbles:true, cancelable:true}));
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", shiftKey:true, bubbles:true, cancelable:true}));
      check(!behaviorCommands.some(c=>c.type === "chat"), "IME confirmation and Shift Enter never send");
      prompt.dispatchEvent(new KeyboardEvent("keydown", {key:"Enter", bubbles:true, cancelable:true}));
      check(behaviorCommands.filter(c=>c.type === "chat").length === 1, "plain Enter sends when enabled");
      fixture.config.enter_sends = false;
      LMUI.receive(fixture);
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
