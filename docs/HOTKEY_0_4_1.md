# 0.4.1：快捷鍵錄製與 PDF 選字

## 設定快捷鍵

齒輪 → 選取文字快捷鍵 → 點輸入框或「錄製」→ 按住 Win 再按 Esc → 放開 → 按「套用」。不用自行輸入 Win／Windows 等名稱。
全新設定預設 Win+Esc；既有使用者保留原本已保存的快捷鍵，需在設定錄製／套用新組合。

支援 Win、Ctrl、Alt、Shift 修飾鍵，至少包含 Win／Ctrl／Alt 之一；可搭配 A–Z、0–9、F1–F24（F12 除外）、Esc、Space、Enter、Tab、Backspace、Delete、Insert、Home、End、PageUp／PageDown、方向鍵、一般標點鍵與數字鍵盤。
只按修飾鍵不會完成錄製。單按 Esc、按取消、關閉設定、切換到其他程式或超過 15 秒，會結束錄製並恢復舊快捷鍵。
錄製期間僅攔截本程式 WebView2 的鍵盤事件，不安裝全域鍵盤 hook；原全域快捷鍵會暫時解除，以免擷取動作搶走同一組按鍵。
錄製結果先顯示，按套用且 Windows 註冊成功才儲存。Windows／其他程式占用的組合會提示並保留原設定。
F12 為 Windows 偵錯用途保留；Ctrl+C 是本程式擷取文字必須送出的組合，因此不允許把它設為觸發鍵。

## 擷取流程

1. 記住觸發時的來源視窗，等待快捷鍵放開，避免 Win／Alt 等修飾鍵混入 Ctrl+C。
2. 在送出前確認仍是來源視窗；不先切換至 LM_AI，也不嘗試強制切回來源。
3. 記錄剪貼簿序號，再透過 SendInput 送出 Ctrl+C。
4. 最多等待 3 秒，每 25 ms 檢查剪貼簿序號變更。只讀新的 Unicode 純文字，不採用舊剪貼簿。
5. 讀取時若剪貼簿又變更，在期限內重讀穩定版本。文字上限 16,000 個 UTF-16 code units。
6. 先把文字直接加入 LM_AI 草稿，再以一般 Windows API 請求顯示主視窗並聚焦 WebView2；不模擬 Ctrl+V。
7. Windows 不允許前景切換時，草稿仍已填入，顯示工作列提示，讓使用者自行點開。沒有 AttachThreadInput、假 Alt 按鍵或持續搶焦點。

0.4.0 已大致採上述複製順序，但在等待時另外檢查前景 HWND 與剪貼簿擁有者 PID。0.4.1 移除這兩個讀取階段的限制，以容許 Adobe 等程式由不同程序／隱藏視窗提供剪貼簿。
同時避免把較大的剪貼簿配置緩衝區誤判為長文字，只檢查界限內實際文字。
這不能證明等待期間取得的每份新文字都來自原選取；若同時在其他程式複製，可能取得那份新內容，因此仍必須在草稿核對後才送出 AI。
複製會改變系統剪貼簿。未送出草稿保留並附加；內容過長或擷取失敗不覆寫原稿。

## 驗證與限制

- Rust 測試驗證鍵名別名、Win+Esc、數字／數字鍵盤及特殊鍵、標準化名稱與不合法組合。
- WebView2 --self-check 經真正 Rust 訊息橋驗證錄製、標準名稱、待套用與取消；使用合成的前端事件，不讀使用者剪貼簿或送出系統按鍵。
- `cargo run --frozen --example hotkey_probe` 短暫註冊 Win+Esc 並立即解除，可檢查當前 Windows 是否可用；不產生按鍵或讀取剪貼簿。本次本機註冊已成功。
- Adobe PDF 的真實選字、延遲複製及焦點交接仍需公司實機驗收；加密／禁止複製的 PDF、掃描圖片或較高權限來源仍可能無法 Ctrl+C，不加入 OCR 或繞過保護。
- 網頁端不需改 API；版本、登入、Chat Completions、通知與 Outlook 契約不變。

參考：[RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)、[SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow)。Windows 鍵組合可能由系統保留；前景切換也可能被系統拒絕，應以當次註冊與使用結果為準。
