# Company AI — Windows 連線測試版

以 Rust 製作的原生 Windows 小工具，先驗證「網頁登入授權 → 帶 API Key 送出訊息 → 顯示 AI 回覆」。
編譯使用 VS2019 / MSVC v142，成品不需要安裝 Rust、Visual Studio、Node 或 WebView2。

## 直接使用 EXE

完成建置的程式位於 **`dist\CompanyAI.exe`**。複製這個 EXE 到公司 Win11 x64 後即可開啟。

1. 填入公司的網站網址，例如 `https://ai.company.example`（不用帶 `/v1`）。
2. API 路徑預設 `/v1/chat/completions`；如果公司用別的路由，可直接修改。
3. 模型名稱填公司服務實際支援的名稱。
4. Header 預設 `Authorization: Bearer`；若公司使用 `X-API-Key`，改選該項。
5. 儲存設定，按「瀏覽器登入」，在網站核對登入碼並授權。
6. 回到 EXE，輸入訊息、確認右側 JSON 預覽，再按「送出訊息」。

**網站需要先實作登入與驗證路由，EXE 才能連接真正公司服務。**
明天修改網站時，直接依照 [WEB_INTEGRATION.md](docs/WEB_INTEGRATION.md)；該文件包含請求、回應、Header、期限、錯誤與驗收流程。
如果公司目前只有固定 API Key 的模型路由，需要讓網站發出的個人 Key 可以被該入口驗證，或加一層代理閘道。

## 網站尚未完成時：本機示範

在此資料夾開啟 PowerShell：

```powershell
.\dist\CompanyAI.exe --demo
```

程式會自動填入 127.0.0.1 的臨時網站與 `demo-echo` 模型。按「瀏覽器登入」，在本機網頁核對代碼並允許，
再回 EXE 輸入文字送出，會收到明確標示「本機模擬回覆」的內容。**這不是真實 AI，也不會連線到公司。**
示範服務只監聽 loopback；隨 EXE 結束而停止，示範 Token 也隨之失效。它與正式設定使用不同資料夾。

## 已實作的範圍

- 可調整網站、API 路徑、模型與兩種驗證 Header。
- 一次性登入碼、瀏覽器核准／拒絕、可取消的背景輪詢。
- 網站核發最長 30 天的使用憑證，以 Windows DPAPI 加密保存。
- 關閉後重開，可沿用尚未到期的正式登入；修改網站／路由／Header 時會清除舊登入。
- Chat Completions 文字 JSON 預覽、非串流送出、最多 20 輪記憶體內對話。
- 中文錯誤提示、401 清除登入、網路逾時與重新導向處理。
- 本機示範及自動化協定／加密／保存測試。

本版不包含串流、附件、剪貼簿監控、系統列、快捷鍵、選取文字、永久登入、refresh_token 或安裝包。
本專案提供自己的離線交付 ZIP；共用的網頁套件編譯控制仍由另一個 Codex 任務處理。

## 憑證與設定位置

正式設定位於 `%LOCALAPPDATA%\CompanyAI\settings.json`，不含 API Key。
加密憑證位於同資料夾的 `session.dpapi`，綁定目前 Windows 使用者。
對話不寫入磁碟；關閉視窗、清除對話、切換網站或完成新的登入後會清除。
「清除本機登入」不等於伺服器撤銷，伺服器端管理方式見串接文件。

預設使用 HTTPS，沿用 Windows 信任庫；不會略過公司憑證錯誤。
僅在公司確實需要 HTTP 測試時，勾選「允許內網 HTTP」，此模式會明文傳送憑證與訊息。

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
| `src/main.rs` | EXE 入口及 --demo / --self-check |
| `ui.rs` | 原生 Windows 控制項與背景工作結果 |
| `config.rs` | 網址、路由、Header 設定與驗證 |
| `auth.rs` | 登入輪詢與帶 Key 的聊天請求 |
| `protocol.rs` | JSON 契約、解析與錯誤提示 |
| `transport.rs` | WinHTTP、TLS、代理、逾時與回應大小限制 |
| `storage.rs` | 設定保存與 DPAPI 憑證加密 |
| `demo.rs` | 明確標示的本機模擬伺服器 |

維護時遵循 [AGENTS.md](AGENTS.md)：簡單易懂、註解充足、方便接手修改。
