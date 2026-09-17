# 專案級開發指引

適用於本專案及其子目錄的程式碼撰寫、編輯與維護。

## 專案位置與共用環境

- 本專案位於 `Y:\Rust\Project\CompanyAI`，是獨立的 Cargo workspace。
- 原始碼、依賴鎖定檔、.cargo 設定、文件、target 與 dist 留在本資料夾。
- 專案環境入口是 `scripts/Enter-DevShell.ps1`：優先使用解壓後的 `toolchain/`，否則使用外層 `Y:\Rust\.tools`。編譯輸出固定在本專案內。
- `Y:\Rust\.local-ai` 與外層安裝腳本是離線編譯環境的資源，不屬於此應用程式，不要搬入或任意修改。
- 離線 ZIP 自帶固定 Rust 工具鏈與 vendor 套件；解壓到新資料夾即可編譯，但電腦仍須預先安裝指定 MSVC 與 Windows SDK。一般開發也可用 `RUST_SHARED_ROOT` 指定共用工具。

## 使用者指定的程式碼風格

- **簡單好懂、註解充足、他人看得懂且改得了、容易維護，為最優先原則。**
- 使用直觀的控制流程與明確命名；避免難讀的一行式、過度泛型、複雜巨集及不必要的抽象。
- 函式與模組保持單一、清楚的責任，讓接手者能由上而下理解執行流程。
- 主動提供足夠的繁體中文註解，說明模組與函式用途、主要步驟、輸入輸出、限制及重要設計原因。
- 對較難理解的 Rust 所有權、生命週期、並行、錯誤處理及平台相容性設定，補上容易理解的說明。
- 公開介面優先使用 `///` 文件註解，必要時提供小型範例；註解應提供有用資訊，不只重述語法。
- 修改邏輯時同步更新相關註解與文件，避免過時資訊誤導維護者。
- 錯誤訊息交代失敗原因與處理方式；一般執行流程避免無理由的 `unwrap()` / `expect()`。
- 引入套件或增加架構複雜度前，先考慮標準函式庫與較簡單的做法，以實際需求為準。

## 編譯基準

- 公司目標：Windows 11 x64、VS2019、MSVC 14.2x / v142。
- 本機已驗證：Rust 1.98.1、MSVC 14.29.30133、Windows SDK 10.0.19041.0。
- Rust 版本由 `rust-toolchain.toml` 固定；不得因另裝新版 VS 就自行改用較新 MSVC。
- 使用 `scripts/Enter-DevShell.ps1` 載入環境，使用 `scripts/Build.ps1` 檢查格式、Clippy、release 編譯與基本執行。
- VS Code 使用 `scripts/Configure-VSCode.ps1` 產生本機 Rust / MSVC 設定，再重新載入視窗。不要把含本機路徑的 `.vscode/settings.json` 提交 Git 或打進 ZIP；離線工具鏈須保留 rust-src，以支援程式碼分析。
- 不加入 `target-cpu=native` 等綁定家用 CPU 的設定。
- 公司完整 MSVC 修補版本與 SDK 版本尚待核對，不宣稱兩邊環境已完全一致。

## 工作範圍

- 本專案目前開發 LM_AI Windows 測試版：瀏覽器授權登入、30 天登入保存、OpenAI 相容 Chat Completions 請求與回覆顯示；網頁端串接規格記錄在 docs/WEB_INTEGRATION.md。
- 網頁控制介面及共用套件編譯管理由另一個 Codex 任務處理。本專案只維護自己的 EXE、原始碼與離線交付包。

## 0.4.1 產品約定

- 正式主機、聊天、device、token、version、models 與 download 路由固定於 `src/config.rs`，不提供 UI 編輯；偏好檔只保存模型代號、快捷鍵、字體大小、側欄收合及通知提示偏好。舊設定不得覆寫固定路由。
- 正式請求固定 Bearer 個人 Token，Chat Completions 維持 `stream: false`；模型選單從後端取得，UI 顯示 label、JSON 傳 id，真實模型由後端映射。
- 使用者指定：版本檢查失敗暫時允許使用；同次執行中已知的強制更新不得被後續網路失敗解除。這個政策不能略過登入驗證或模型權限。
- 全新設定快捷鍵預設 Win+Esc，既有偏好保留；支援直接按鍵錄製，套用成功才生效；必須在顯示主視窗之前取得來源選字。只放入草稿，不自動送出、不覆蓋舊草稿、不背景監控剪貼簿。
- 0.4.1 擷取完成後先填入草稿再請求前景；不依賴 Ctrl+V 或強制焦點技巧。等待新剪貼簿時不限制擁有者 PID／來源 HWND，以支援 Adobe 多程序；讀取新的穩定純文字並要求使用者確認。錄製時暫停原快捷鍵，取消／逾時／離開程式時恢復。
- 0.4.0 已加入內嵌 WebView2 介面、Markdown／KaTeX／高亮、本機 DPAPI 歷史、通知 REST／WebSocket，以及 Classic Outlook 唯讀預覽／確認分析。
- 公司一律使用 Classic Outlook；以 Rust windows COM 操作，不依賴 Python 或 pywin32。不寄信、不修改郵件、正文必須經使用者明確确认後讀取。
- 附件、RAG、UNC 知識庫、skills 工具呼叫與後續任務自動化尚未實作，不自行擴張本次範圍。
- 前端程式位於 ui/，build.rs 將資源嵌入 EXE；第三方資源及授權位於 ui/vendor，不使用 CDN，也不要求公司安裝 Node。
- 使用者同意附 WebView2 x64 離線安裝包，放在 dist/ 並納入 ZIP；更新時核對 Microsoft 簽章及 SHA256。
- 不向前端傳 Token、真實郵件 EntryID 或任意檔案／命令執行能力。通知不能直接觸發外部工具。
- 修改功能時同步更新 docs/WEB_INTEGRATION.md 與相應驗收說明。沒有實際操作 Word／Outlook 或取得外觀截圖時，不能宣稱這些驗收通過。

## 修改後的交付與 Git 規則

- 使用者要求專案包含原始碼、已編譯 EXE、Rust 離線編譯包及附屬文件，這些產物必須保持同一版本。
- 完成原始碼、依賴、設定、腳本或交付文件修改後，執行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Prepare-Delivery.ps1`。它會更新 vendor、執行 Build、製作 ZIP，並在全新解壓目錄使用包內工具鏈、空 Cargo 快取及 `--frozen` 再次驗證。
- 成功後交付 `dist/LM_AI.exe`、相同內容的相容檔名 `dist/CompanyAI.exe`、`dist/MicrosoftEdgeWebView2RuntimeInstallerX64.exe`、`offline/CompanyAI-offline.zip`、ZIP 的 `.sha256`、`offline/manifest.json`、`offline/verification.json`、`offline/environment.txt`，以及對應原始碼與文件。
- 檢查失敗時修正問題並重新執行；不要把舊 ZIP 配上新原始碼宣稱為完整交付，也不要略過驗證。
- 依賴變更須同步更新 Cargo.lock；先取得所需 registry 套件，再用 `Prepare-Delivery.ps1 -RefreshDependencies` 更新離線包。保持固定 Rust / MSVC 基準。
- Git 儲存庫以本專案資料夾為根，不把外層共用環境或 `.local-ai` 納入。
- `.gitattributes` 已為 `dist/*.exe` 與 `offline/*.zip` 設定 Git LFS。首次加入二進位檔前必須在儲存庫執行 `git lfs install --local`；確認 LFS 可用，避免把大型 ZIP 直接當一般 Git 物件提交。
- 使用者要求提交或上傳時，先完成上述交付流程，再將原始碼、EXE、離線包、校驗與文件一起提交；確認 LFS 物件亦成功上傳。提交前檢查暫存內容，不包含 API Key、登入憑證、個人設定、target、解壓後的 toolchain/vendor、快取或暫存目錄。
- 遠端儲存庫為 `https://github.com/you1230783-sys/CompanyAI.git`，主要分支為 `main`。使用者已授權首次完整交付上傳；後續依使用者的提交／推送指示操作，明確授權可沿用。不自行更換遠端、不強制推送覆蓋歷史。
- 操作細節見 `docs/OFFLINE_AND_GIT.md`。驗證記錄描述本機測試，不能代替公司實機驗收。
