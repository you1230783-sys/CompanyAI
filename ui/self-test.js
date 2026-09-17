/* 只由 EXE --self-check 明確觸發；測試使用虛構資料，不讀帳號或 Outlook。 */
"use strict";
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
      can_estimate: true,
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
    check(
      WorkUI.durationLabel(null) === "未知",
      "unknown estimate is not zero",
    );
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
