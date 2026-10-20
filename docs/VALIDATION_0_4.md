# 0.4.0 驗證範圍

## 自動檢查

建置以 `scripts/Build.ps1` 執行：v142 x64 實際 cl.exe 探針、cargo fmt、Clippy -D warnings、Rust 測試、release 建置、WebView2 介面自我檢查及 DLL 檢查。
測試覆蓋既有登入／版本／模型／選字規則，以及 DPAPI 對話讀寫、損毀保留、通知去重／過期／已讀、REST 補查、WebSocket 握手收訊及取消。
WebView2 自我檢查真正載入內嵌資源，檢查表格、KaTeX、程式碼高亮、註腳、任務清單、HTML／連結限制、聊天標籤、側欄／字體，以及向上閱讀保留位置和底部跟隨。

完整交付流程使用全新解壓資料夾、空 Cargo 快取及包內 Rust 工具鏈，再以 `cargo build --frozen` 和相同 Build 檢查驗證。最終結果與 SHA256 見 `offline/verification.json`、`offline/environment.txt`、`offline/manifest.json`。
WebView2 執行階段需已安裝；受限制的執行沙箱可能禁止啟動瀏覽器子程序，自我檢查必須在能正常啟動 WebView2 的 Windows 工作階段執行。

## 外觀與公司實機

以瀏覽器載入相同 HTML/CSS 與虛構資料檢查聊天、表格、公式、程式碼、設定、通知與 Outlook 預覽。瀏覽器預覽不能代替原生 EXE 或 Office 實機驗收。
本機未驗證真實公司 SSO／API、公司通知服務、Classic Outlook 實際信箱、Word 選字，以及從完全未安裝 WebView2 的公司電腦執行離線安裝。
離線安裝檔來源為 Microsoft 官方，已核對 Authenticode 簽章與 SHA256；未在現有電腦重裝執行階段。

公司驗收：

- 如缺少 WebView2，執行 dist 隨附的 x64 離線安裝程式，再啟動 LM_AI.exe。
- 登入、選模型、送出含表格／公式／程式碼的問題；驗證複製與長內容捲動。
- 調整字體、收合側欄，重啟確認偏好；重開舊對話、追問與刪除。
- 網站依 NOTIFICATIONS_AND_OUTLOOK.md 部署通知，測試即時、離線補查、已讀與期限。
- Classic Outlook 的實機步驟依該文件驗證，特別確認沒有改變郵件未讀狀態。
- 驗證 latest／minimum 更新政策及版本連線失敗暫用。
