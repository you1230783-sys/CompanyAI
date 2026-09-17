# LM_AI 0.6.0 — Windows 工作助理

Rust 桌面程式，提供公司瀏覽器登入、文字／附件對話、串流、持久背景任務、選字快捷鍵、網站通知及 Classic Outlook 唯讀助理。
介面使用 WebView2、內嵌 HTML/CSS 與離線 Markdown 套件；不需 Node、Python 或外部 CDN。

## 0.6 更新

- 卡住的任務可「停止追蹤」或「移除任務」，404／離線也能解除本機等待；另行嘗試 server 取消，對話保留。
- 切換模型重新查詢附件能力，保留不相容附件並提示原因，避免把圖片送到不支援的模型。
- 全站鈴鐺與 AI 通知共用通知中心。網站通知的已讀／刪除走 Token API，成功後才更新；AI 僅顯示成功回覆／失敗。App 前景時不跳系統氣泡。
- 設定可點視窗外部關閉，關閉時亦結束快捷鍵錄製。
- Outlook 支援多選及收件匣「所有／未讀 × 今天／三天內／本週」。確認後先傳基本資訊；可授權依 AI 請求自動匯出／上傳 MSG 並接續整理，不必手動存檔。

網站須配合 [0.6 契約](docs/DESKTOP_0_6_CONTRACT.md)。實際 Outlook／公司鈴鐺驗收與限制見 [0.6 驗收](docs/VALIDATION_0_6.md)。

## 直接使用

1. 複製 `dist/LM_AI.exe` 到公司 Windows 11 x64。`CompanyAI.exe` 是同版本的相容檔名，兩者擇一執行即可。
2. 若電腦缺少 WebView2，先執行隨附的 `dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe`；詳見 [離線安裝說明](dist/WEBVIEW2-OFFLINE.md)。
3. 按左下齒輪 → 瀏覽器登入，核對短碼並允許授權；登入最長保存 30 天。
4. 選擇後端提供的「快速／品質」等模型，輸入文字並送出。Enter 換行，Ctrl+Enter 送出。
5. 使用者訊息靠右、AI 靠左；支援表格、程式碼高亮／複製、數學公式、註腳、任務清單與一般 Markdown。

側欄可收合；齒輪設定包含登入／登出、字體大小（預設 14 px，可調 12–20）、快捷鍵、通知及更新。
輸入框會隨內容長高，上限 160 px。AI 回覆時，停在底部就跟隨新內容；往上閱讀時保留位置，可按「查看最新回覆」跳到底。
公式使用 KaTeX 支援的 TeX 語法，包括 `$...$`、`$$...$$`、`\(...\)`、`\[...\]`；不是完整 LaTeX 文件編譯器。
原始 HTML 不執行，遠端圖片只顯示替代文字；外部 HTTP(S) 連結經確認後用瀏覽器開啟。

## 附件、串流與背景任務

網站部署 [0.5 契約](docs/DESKTOP_0_5_CONTRACT.md) 後，登入會取得支援模式與附件規則。未部署時仍保留一般純文字聊天。
選「串流回覆」逐段閱讀，或「背景處理」先取得任務，完成後收到提示。按迴紋針選檔，也可直接在輸入框貼上圖片。
文件與圖片合計最多 20 個；副檔名、單檔及合計大小由後端提供，不需為一般規則調整更新 EXE。
附件先上傳網站轉換，卡片顯示排隊／轉檔／可送出／失敗；全部就緒後才可送出 AI 分析。桌面不處理 MD 轉換或 OCR。
可按「估算時間」查看排隊、處理與總時間；未知顯示未知，估算不是完成保證。

任務面板可查看狀態、回到對話或取消。可同時處理不同對話，同一對話先等當前工作結束；最多追蹤 8 個未完成聊天工作。
串流斷線保留片段、改查原任務；退出後重開並登入同帳號，也會查回結果。重試沿用 request_id，不直接新建重複工作。
最小化自動進入系統托盤，右鍵可還原或離開。離開 App 不會取消伺服器已接受的任務；退出期間沒有桌面即時通知。
詳見 [0.5 使用與驗收說明](docs/VALIDATION_0_5.md)。Icon 暫用 fallback，待提供圖片後可經 assets/app.ico 嵌入。

## 選取文字

保持 LM_AI 開啟，在一般權限的來源程式選取文字，按 **Win+Esc** 並放開按鍵；文字會接到草稿後面，確認後再按送出、翻譯、摘要或潤飾。
快捷鍵不自動送出、不監聽一般按鍵；可在設定修改。沿用一般 Ctrl+C 行為，會改變剪貼簿，不保留原圖片或富文字。
草稿上限 16,000 個 UTF-16 code units；擷取失敗、送出 Ctrl+C 前來源變動或內容過長時保留原稿。某些編輯器未選字也會複製整行，送出前請核對。

快捷鍵設定可直接點輸入框／錄製，按下組合鍵後再套用；既有快捷鍵偏好保留，升級後可自行錄製 Win+Esc。PDF 相容流程與限制見 [0.4.1 快捷鍵說明](docs/HOTKEY_0_4_1.md)。

## 通知與 Classic Outlook

- 通知中心顯示網站事件；使用 WebSocket 喚醒及每 60 秒 REST 補查，支援已讀、去重、期限與加密快取。
- Windows 提示只在 App 不在前景時顯示，不搶焦點，點擊可開啟通知／任務面板。最小化後保留在系統托盤；關閉或選離開才退出，不另裝背景服務。
- Classic Outlook：選取一封信，按「讀取選取郵件」先預覽基本資訊；確認分析才送給 AI。
- 單封手動模式的正文需另外勾選並確認，不含附件；多封自動模式則依本批授權匯出 MSG。不寄信、刪信、移動或修改未讀狀態。
- 單封手動預覽仍需另外確認正文；多封自動模式使用本批授權，僅能匯出勾選的郵件。Skill 與工具由 App 限制，不寄信、不修改信箱。

## 本機保存與固定服務

偏好位於 `%LOCALAPPDATA%\CompanyAI\settings.json`；延續舊資料夾名稱以保留升級相容。
Token、對話與通知分別為 `session.dpapi`、`history.dpapi`、`notifications.dpapi`，以目前 Windows 使用者的 DPAPI 加密。
對話最多 200 個、每個最多 20 輪、未加密資料合計最多 32 MB；成功回覆後保存。磁碟錯誤會標示未保存，損毀原檔不被空紀錄覆寫。
草稿與未送出的郵件預覽只在記憶體。登出保留本機歷史，但新登入從新對話開始；再次送出舊對話才會把它交給目前帳號。
右上刪除可移除目前對話。登出只清除本機 Token，不等於伺服器撤銷。

公司主機、聊天、登入、版本、模型、下載與通知路由固定且不提供 UI 編輯。
聊天維持 `Authorization: Bearer <個人 Token>` 與 model alias。一般／背景使用 stream=false，串流使用 stream=true。
版本及模型啟動時、登入後、每 5 分鐘與手動重新整理時查詢。低於 minimum_version 才強制更新；僅低於 latest_version 可繼續使用。
版本檢查失敗暫時允許使用，但同次執行已確認的強制更新不因斷線解除。模型或登入失敗仍需處理，不能藉版本暫用政策略過。
更新由使用者下載並替換 EXE。依公司指定目前採內網 HTTP，沒有 TLS 傳輸加密；將來切 HTTPS 需同步更新固定設定。

## 後端契約與驗證

- [DESKTOP_0_6_CONTRACT.md](docs/DESKTOP_0_6_CONTRACT.md)：全站鈴鐺、模型規則與 Outlook MSG。
- [VALIDATION_0_6.md](docs/VALIDATION_0_6.md)：新版測試與公司驗收。

- [DESKTOP_0_5_CONTRACT.md](docs/DESKTOP_0_5_CONTRACT.md)：附件、模式、持久任務、估時與新通知契約。
- [VALIDATION_0_5.md](docs/VALIDATION_0_5.md)：新版操作、本機測試與公司驗收。
- [WEB_INTEGRATION.md](docs/WEB_INTEGRATION.md)：既有登入、聊天、版本與模型契約。
- [NOTIFICATIONS_AND_OUTLOOK.md](docs/NOTIFICATIONS_AND_OUTLOOK.md)：本次網站需新增的通知 API、WebSocket 與 Outlook 分析格式。
- [VALIDATION_0_4.md](docs/VALIDATION_0_4.md)：本機驗證範圍與公司驗收步驟。
- [BACKEND_ROADMAP.md](docs/BACKEND_ROADMAP.md)：已完成與後續範圍。

本次未加入 UNC 知識庫、skills 工具呼叫、RAG、桌面 OCR 或寄信。

## 本機示範

```powershell
.\dist\LM_AI.exe --demo
```

齒輪 → 登入，在本機頁面允許授權後可測附件狀態、串流、背景任務、估時、通知與已讀。模擬後端不做真實轉檔，狀態僅保存在記憶體。Outlook 助理會使用虛構郵件，不讀真實信箱。
只監聽 loopback，所有回覆明示本機模擬；不連公司服務。一般使用不需要 Rust 或 Visual Studio。

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

腳本會執行格式檢查、Clippy、測試、release 編譯、WebView2 介面自我檢查、DLL 依賴檢查，最後更新 `dist\LM_AI.exe`。
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

本資料夾包含原始碼、`dist\LM_AI.exe`、`offline\CompanyAI-offline.zip` 及相關文件／校驗記錄。
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
