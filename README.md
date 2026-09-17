# Company AI 0.3.0 — Windows 工作助理

Rust 原生 Windows 桌面程式，提供瀏覽器登入、模型選單、文字對話，以及選取文字後的快捷處理。
採深色側欄、淺色對話區與底部輸入卡片，保留原生程式的快速啟動。一般使用不需要 Rust、Visual Studio、Node 或 WebView2。

## 直接使用

1. 將 `dist/CompanyAI.exe` 複製到公司 Windows 11 x64，雙擊開啟。
2. 按「瀏覽器登入」，核對網頁與程式的短碼並允許授權。登入最長保存 30 天。
3. 選取後端提供的「快速／品質」等選項，輸入文字後按「送出」。Enter 換行、Ctrl+Enter 送出。
4. 可接著追問，或按「複製回覆」。按「新對話」清除歷史，保留未送出的草稿。

本版已固定公司網址、聊天、登入碼、Token、版本與模型路由，介面不再提供網址或 Header 編輯。
更新前的設定檔只保留模型偏好；其中的舊網址不再採用。若舊憑證綁定不同路由，升級後請重新登入。
正式 API 使用 `Authorization: Bearer <個人 access_token>`，JSON 維持 Chat Completions、`stream: false`。

## 選取文字與快捷鍵

1. 保持 Company AI 開啟，可最小化到工作列。
2. 在瀏覽器、Word 或其他支援 Ctrl+C 的一般權限程式選取文字。
3. 按 **Ctrl+Alt+Q** 並放開按鍵，Company AI 會嘗試複製選字並帶入草稿。
4. 確認內容後，按「翻譯」「摘要」「潤飾」或「送出」。**快捷鍵本身不會傳送內容。**

側欄可改為例如 `Ctrl+Shift+F8`，再按「套用快捷鍵」。若組合已被占用會提示；不使用 Windows 保留快捷鍵。
翻譯預設外語轉繁體中文、中文轉英文；摘要使用繁體中文；潤飾保留原語言。

- 有舊草稿時，擷取內容接在後面，不直接覆蓋；超過 16,000 個 UTF-16 code units 時保留原稿。
- 擷取採一般 Ctrl+C，會改變系統剪貼簿，不保留原本圖片或富文字格式。
- 實際複製內容依來源程式的 Ctrl+C 行為；某些編輯器未選字也會複製整行，因此送出前仍需核對草稿。
- 沒有新複製結果、來源切換、其他程式改寫剪貼簿或權限不同時，不把舊剪貼簿當成這次選字。
- 不會背景監聽一般按鍵、持續讀取剪貼簿、掃描檔案或讀取郵件。擷取失敗可手動貼上。
- 關閉程式即停止全域快捷鍵；此版沒有系統列常駐。

## 版本與模型清單

啟動時與每 15 分鐘查詢服務，也能手動按「重新整理服務」。登入完成會重新取得有權使用的模型。
版本檢查連不上、回應錯誤或格式不正確時，**暫時允許使用**；已取得的最低版本門檻不會因同次執行中稍後斷線而解除。
低於最低版本時，停止新的登入與聊天，提示開啟固定下載頁。僅有較新版本但未低於最低版本時，仍可使用。
更新方式是下載新版、關閉舊程式並替換 EXE，沒有背景安裝或自動覆寫。

模型清單由網站提供，不把實際模型名稱寫死在桌面程式。若模型服務尚未完成、無可用模型或需要登入，會顯示原因；完成登入後可重新整理。
版本失敗的暫用政策不會略過登入驗證或憑空猜測模型。

## 後端文件

- **[WEB_INTEGRATION.md](docs/WEB_INTEGRATION.md)**：本次固定路由、登入、版本與模型 JSON、聊天 Header、錯誤及驗收順序。
- **[BACKEND_ROADMAP.md](docs/BACKEND_ROADMAP.md)**：依分享對話整理通知、Outlook、RAG 等後續項目；清楚區分本版與尚未實作範圍。
- **[VALIDATION_0_3.md](docs/VALIDATION_0_3.md)**：自動驗證範圍與公司實機測試表。

網站需實作上述契約才能使用真正公司服務。本版沒有串流、附件、Outlook 存取、通知推播、永久 Token 或 refresh_token。

## 本機示範

```powershell
.\dist\CompanyAI.exe --demo
```

按「瀏覽器登入」，在本機頁面核對短碼並允許；可用「快速／品質／Ultra」測試訊息往返。
回覆會標示「本機模擬回覆，非真實 AI」，不連公司服務。示範只監聽 loopback，程式結束即停止。

## 個人資料與連線

偏好存於 `%LOCALAPPDATA%\CompanyAI\settings.json`，只包含模型代號與快捷鍵。
憑證以目前 Windows 使用者的 DPAPI 加密存於同資料夾 `session.dpapi`；對話僅留在記憶體。
「登出」刪除本機憑證，不等於在伺服器撤銷；伺服器仍需檢查到期、權限與撤銷狀態。
目前依公司指定使用內網 HTTP，訊息與 Token 在傳輸中沒有 TLS 加密；若改成 HTTPS，需更新程式固定網址，憑證驗證使用 Windows 信任庫。

## 使用 VS Code 編輯

請以 VS Code 的「開啟資料夾」開啟本專案 `CompanyAI`，並使用 `rust-lang.rust-analyzer` 擴充套件。
第一次使用或搬移專案後，先在專案根目錄執行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Configure-VSCode.ps1
```

接著在 VS Code 按 `Ctrl+Shift+P`，執行 `Developer: Reload Window`（重新載入視窗）；既有終端機請關閉後重開。
腳本會讓 rust-analyzer 與新終端機取得正確的 Rust / MSVC 環境，解決 `cargo metadata ... program not found`。
已編譯的 EXE 可獨立執行；編輯器的提示、跳轉及檢查則需要 Cargo、rustc 和 rust-src。
電腦專用設定存於 `.vscode/settings.json`，不提交 Git 或放入離線 ZIP；包內含相同設定腳本，換電腦後重新產生即可。
如果設定檔含有 PowerShell 5.1 不支援的 JSON 註解，腳本會停止並保留原檔，請先備份再手動合併設定。

## 編譯與驗證

本專案根目錄為 `Y:\Rust\Project\CompanyAI`。以下指令都在這個資料夾執行：

```powershell
cd Y:\Rust\Project\CompanyAI
```

原始碼、Cargo.lock、編譯設定、target 與 dist 都屬於此專案。
`scripts\Enter-DevShell.ps1` 優先使用本專案的 `toolchain`，否則使用外層 `Y:\Rust\.tools`；套件由本專案 `vendor` 提供。
拿到 Git 專案後，要離線編譯請將 `offline\CompanyAI-offline.zip` 解壓到新資料夾，並在解壓後的根目錄執行下列指令。
包內包含 Rust 工具與依賴，不需要使用家裡的共用資料夾；公司電腦仍須預先安裝下列 MSVC 與 SDK。

固定 Rust/Cargo 1.98.1、`x86_64-pc-windows-msvc`、MSVC 14.29.30133（v142）、SDK 10.0.19041.0。
公司完整的 MSVC 修補版本與 SDK 版本仍待核對。

```powershell
.\scripts\Build.ps1
```

腳本會執行格式檢查、Clippy、測試、release 編譯、隱藏視窗控制項自我檢查、DLL 依賴檢查，最後更新 `dist\CompanyAI.exe`。
完整編譯記錄存於 `offline\environment.txt`。

若 Windows PowerShell 的執行原則阻擋腳本，可單次使用：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build.ps1
```

開發時先載入專案環境，再使用 Cargo：

```powershell
. .\scripts\Enter-DevShell.ps1
cargo run --frozen -- --demo
```

環境預設離線，透過 `vendor` 與 `--frozen` 建置；執行 EXE 不需要開發工具。

## 離線交付與 Git

本資料夾包含原始碼、`dist\CompanyAI.exe`、`offline\CompanyAI-offline.zip` 及相關文件／校驗記錄。
完整 ZIP 含相同版本的原始碼、EXE、Rust 工具鏈及所有依賴，方便整包帶到公司。

修改完成後，使用以下指令更新交付檔案並驗證全新解壓後的離線編譯：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1
```

Git LFS 規則已寫入 `.gitattributes`；遠端儲存庫為 [CompanyAI](https://github.com/you1230783-sys/CompanyAI)，主要分支為 `main`。
完整目錄說明、離線操作、校驗與 Git 工作流程見 [OFFLINE_AND_GIT.md](docs/OFFLINE_AND_GIT.md)。

## 開發用瀏覽器測試

真正瀏覽器的開發用整合測試：

```powershell
cargo run --example browser_smoke --frozen
```

開啟輸出的本機網址並按允許。程式會驗證 Token、DPAPI、中文訊息回覆及登入碼不可重複使用，成功時印出 PASS。

## 程式碼位置

| 檔案 | 責任 |
| --- | --- |
| `src/main.rs` | EXE 入口、--demo、--self-check |
| `src/ui.rs` | 控制項、草稿、使用狀態與背景結果 |
| `src/appearance.rs` | 原生介面配色、字型與卡片繪製 |
| `src/config.rs` | 固定路由、偏好序列化、同來源驗證 |
| `src/service.rs` | 版本門檻、失敗政策、模型清單 |
| `src/selection.rs` | 全域快捷鍵、明確觸發的複製、剪貼簿邊界 |
| `src/auth.rs` | 登入輪詢、帶 Token 的聊天請求 |
| `src/protocol.rs` | Chat Completions、登入 JSON 與錯誤提示 |
| `src/transport.rs` | WinHTTP、TLS、代理與逾時 |
| `src/storage.rs` | 偏好保存與 DPAPI |
| `src/demo.rs` | 本機模擬網站及往返測試 |

維護時遵循 [AGENTS.md](AGENTS.md)：簡單易懂、繁體中文註解充足、方便接手修改。
